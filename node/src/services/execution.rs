use anyhow::{anyhow, Error};
use cas::ContentAddressableStorage;
use execution_engine::{Entry, ExecuteStage, ExecuteStatus, ExecutionBackend, ExecutionEngine};
use futures::future::BoxFuture;
use futures::StreamExt;
use prost::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, instrument, trace};

pub static EXEC_OP_METADATA: &'static str =
    "type.googleapis.com/build.bazel.remote.execution.v2.ExecuteOperationMetadata";
pub static EXEC_RESP: &'static str =
    "type.googleapis.com/build.bazel.remote.execution.v2.ExecuteResponse";

pub struct ExecutionService<C, B> {
    instance: String,
    cas: C,
    engine: ExecutionEngine<B>,
}

impl<C: ContentAddressableStorage, B> ExecutionService<C, B> {
    pub fn new(instance: &str, cas: C, engine: ExecutionEngine<B>) -> Self {
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
        .read_blob(digest.into())
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
impl<C: ContentAddressableStorage, B: ExecutionBackend> protos::Execution
    for ExecutionService<C, B>
{
    type ExecuteStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    #[instrument(skip_all)]
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

        let cas = self.cas.clone();
        let cas2 = self.cas.clone();
        let mut exec_events = self
            .engine
            .execute(
                move || {
                    let request = request.clone();
                    let cas = cas.clone();
                    async move {
                        let action_digest =
                            request.action_digest.ok_or(Status::invalid_argument(
                                "Invalid ExecuteRequest: no action digest specified.",
                            ))?;
                        let action: protos::re::Action =
                            get_proto(cas.clone(), action_digest.clone().into()).await?;
                        trace!("{action:#?}");

                        let command_digest =
                            action.command_digest.ok_or(Status::invalid_argument(
                                "Invalid Action: no command digest specified.",
                            ))?;
                        let command: protos::re::Command =
                            get_proto(cas.clone(), command_digest.into()).await?;
                        trace!("{command:#?}");

                        let root_digest = action.input_root_digest.ok_or(
                            Status::invalid_argument("Invalid Action: no root digest specified."),
                        )?;
                        let root_directory: protos::re::Directory =
                            get_proto(cas.clone(), root_digest.into()).await?;
                        trace!("{root_directory:#?}");

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
                            cas.clone(),
                            PathBuf::default(),
                        )
                        .await
                        .map_err(|e| {
                            Status::unknown(format!("Failed to execute mapping collection: {e}"))
                        })?;

                        // Collect output path information
                        if !command.output_paths.is_empty() {
                            for path in command.output_paths {
                                dir_layout.output_paths.push(path.into());
                            }
                        } else {
                            todo!();
                        };

                        Ok((action_digest.into(), cmd, dir_layout))
                    }
                },
                move |output_paths| {
                    let cas = cas2.clone();
                    async move {
                        info!("## Verifying ## ");
                        let mut entries = vec![];

                        for (entry_path, file_path) in output_paths {
                            let digest = cas.add_from_file(&file_path).await?;
                            entries.push(Entry {
                                path: entry_path,
                                digest,
                                executable: false,
                            });
                        }
                        Ok(entries)
                    }
                },
            )
            .map_err(|e| Status::unknown(format!("Failed to execute: {e}")))?;

        // TODO is there an easy way to map over this instead of making another channel?
        let (tx, rx) = mpsc::channel(32);
        while let Some(event) = exec_events.recv().await {
            info!("{:?}", event);
            tx.send(convert_to_op(event))
                .await
                .map_err(|e| Status::unknown(format!("Failed to execute command: {e}")))?;
        }
        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(output_stream as Self::ExecuteStream))
    }

    type WaitExecutionStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    #[instrument(skip_all)]
    async fn wait_execution(
        &self,
        request: Request<protos::re::WaitExecutionRequest>,
    ) -> Result<Response<Self::WaitExecutionStream>, Status> {
        Err(Status::not_found("WaitExecution: Not yet implemented"))
    }
}

fn convert_to_op(
    exec_status: ExecuteStatus,
) -> Result<protos::longrunning::Operation, tonic::Status> {
    // Assemble the metadata
    let metadata = protos::re::ExecuteOperationMetadata {
        stage: match exec_status.stage {
            ExecuteStage::Queued => protos::re::execution_stage::Value::Queued,
            ExecuteStage::Running => protos::re::execution_stage::Value::Executing,
            ExecuteStage::Done(_) => protos::re::execution_stage::Value::Completed,
        }
        .into(),
        action_digest: Some(exec_status.action_digest.into()),
        // TODO stream metadata
        stdout_stream_name: String::from(""),
        stderr_stream_name: String::from(""),
    };

    let (done, result) = match exec_status.stage {
        ExecuteStage::Queued => (false, None),
        ExecuteStage::Running => (false, None),
        ExecuteStage::Done(resp) => {
            let execution_metadata = protos::re::ExecutedActionMetadata {
                ..Default::default()
            };

            // Collect output files from the finished execution
            let mut output_files = vec![];
            for entry in resp.output_paths {
                output_files.push(protos::re::OutputFile {
                    path: entry.path.display().to_string(),
                    digest: Some(entry.digest.into()),
                    is_executable: entry.executable,
                    // The contents of the file if inlining was requested. The server SHOULD NOT inline
                    // file contents unless requested by the client in the
                    // [GetActionResultRequest][build.bazel.remote.execution.v2.GetActionResultRequest]
                    // message. The server MAY omit inlining, even if requested, and MUST do so if inlining
                    // would cause the response to exceed message size limits.
                    contents: vec![],
                    node_properties: None,
                });
            }

            info!("output: {:#?}", output_files);

            let response = protos::re::ExecuteResponse {
                result: Some(protos::re::ActionResult {
                    output_files,
                    output_file_symlinks: vec![],
                    output_symlinks: vec![],
                    output_directories: vec![],
                    output_directory_symlinks: vec![],
                    exit_code: resp.exit_status,
                    execution_metadata: Some(execution_metadata),
                    stdout_digest: None,
                    stderr_digest: None,
                    stdout_raw: resp.stdout,
                    stderr_raw: resp.stderr,
                }),
                cached_result: false,
                status: Some(protos::rpc::Status {
                    code: 0,
                    message: "".to_string(),
                    details: vec![],
                }),
                server_logs: HashMap::new(),
                message: String::from(""),
            };
            (true, Some(response))
        }
    };
    // Name pattern defined by longrunning proto & in remote_execution.proto under WaitExecution
    let name = format!("operations/{}", exec_status.uuid);
    Ok(protos::longrunning::Operation {
        name,
        done,
        metadata: Some(prost_types::Any {
            type_url: EXEC_OP_METADATA.to_string(),
            value: metadata.encode_to_vec(),
        }),
        result: result.map(|result| {
            // There should never be a scenario where we are attempting to return a response
            // to an unfinished execution.
            assert!(done);
            protos::longrunning::operation::Result::Response(prost_types::Any {
                type_url: EXEC_RESP.to_string(),
                value: result.encode_to_vec(),
            })
        }),
    })
}