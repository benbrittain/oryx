use anyhow::{anyhow, Error};
use cas::ContentAddressableStorage;
use execution_engine::ExecutionEngine;
use futures::future::BoxFuture;
use std::path::PathBuf;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub struct ExecutionService<T, C> {
    instance: String,
    cas: C,
    engine: T,
}

impl<T, C: ContentAddressableStorage> ExecutionService<T, C> {
    pub fn new(instance: &str, cas: C, engine: T) -> Self {
        ExecutionService {
            instance: instance.to_string(),
            cas,
            engine,
        }
    }
}

pub async fn get_proto<P: prost::Message + Default, C: ContentAddressableStorage>(
    cas: C,
    digest: common::Digest,
) -> Result<P, Status> {
    let blob = cas
        .read_blob(&digest.into())
        .await
        .map_err(|_| Status::failed_precondition("Failed to fetch blob from CAS."))?;
    // TODO should return a precondition failure / violation setup here as well.
    // https://github.com/googleapis/googleapis/blob/master/google/rpc/error_details.proto#L139
    let proto = P::decode(&mut std::io::Cursor::new(blob))
        .map_err(|_| Status::internal("Failed to decode Action proto: {action_digest}."))?;
    Ok(proto)
}

fn create_mapping<'a, C: ContentAddressableStorage>(
    mapping: &'a mut execution_engine::DirectoryLayout,
    dir: protos::re::Directory,
    cas: C,
    root: PathBuf,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        assert_eq!(dir.symlinks.len(), 0);
        for file in dir.files {
            let mut path = root.clone();
            path.push(&file.name);
            mapping.files.push(execution_engine::Entry {
                digest: file.digest.unwrap().into(),
                path,
                executable: file.is_executable,
            });
        }
        for directory_node in &dir.directories {
            let node_digest = directory_node.digest.clone().ok_or(anyhow!(
                "No digest in node for directory: {}",
                root.display()
            ))?;
            let dir: protos::re::Directory = get_proto(cas.clone(), node_digest.into()).await?;
            let mut new_root = root.clone();
            new_root.push(&directory_node.name);
            create_mapping(mapping, dir, cas.clone(), new_root).await?;
        }
        Ok(())
    })
}

#[tonic::async_trait]
impl<T: ExecutionEngine, C: ContentAddressableStorage> protos::Execution
    for ExecutionService<T, C>
{
    type ExecuteStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    async fn execute(
        &self,
        request: Request<protos::re::ExecuteRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let request = request.into_inner();
        if request.instance_name != self.instance {
            return Err(Status::permission_denied(
                "Request sent to invalid instance: {self.instance}.",
            ));
        }

        // Get all the information needed to build a command for execution
        let action_digest = request.action_digest.ok_or(Status::invalid_argument(
            "Invalid ExecuteRequest: no action digest specified.",
        ))?;
        let action: protos::re::Action = get_proto(self.cas.clone(), action_digest.into()).await?;
        log::trace!("{action:#?}");

        let command_digest = action.command_digest.ok_or(Status::invalid_argument(
            "Invalid Action: no command digest specified.",
        ))?;
        let command: protos::re::Command =
            get_proto(self.cas.clone(), command_digest.into()).await?;
        log::trace!("{command:#?}");

        let root_digest = action.input_root_digest.ok_or(Status::invalid_argument(
            "Invalid Action: no root digest specified.",
        ))?;
        let root_directory: protos::re::Directory =
            get_proto(self.cas.clone(), root_digest.into()).await?;
        log::trace!("{root_directory:#?}");

        // Collect a command for the execution engine
        let cmd = execution_engine::Command {
            arguments: command.arguments,
            env_vars: command
                .environment_variables
                .iter()
                .map(|ev| (ev.name.clone(), ev.value.clone()))
                .collect(),
        };

        // Collect the filesystem information for the execution engine
        let mut dir_layout = execution_engine::DirectoryLayout::default();
        create_mapping(
            &mut dir_layout,
            root_directory,
            self.cas.clone(),
            PathBuf::default(),
        )
        .await
        .map_err(|e| Status::unknown(format!("Failed to execute mapping collection: {e}")))?;


        let result = self
            .engine
            .run_command(cmd, dir_layout)
            .await
            .map_err(|e| Status::unknown(format!("Failed to execute command: {e}")))?;

        // Setup response stream
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // TODO next, set up a longrunning response of the result

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(output_stream as Self::ExecuteStream))
    }

    type WaitExecutionStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    async fn wait_execution(
        &self,
        request: Request<protos::re::WaitExecutionRequest>,
    ) -> Result<Response<Self::WaitExecutionStream>, Status> {
        Err(Status::not_found("WaitExecution: Not yet implemented"))
    }
}
