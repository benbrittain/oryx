use cas::ContentAddressableStorage;
use execution_engine::ExecutionEngine;
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

    pub async fn get_proto<P: prost::Message + Default>(
        &self,
        digest: common::Digest,
    ) -> Result<P, Status> {
        let blob = self
            .cas
            .read_blob(&digest.into())
            .await
            .map_err(|_| Status::failed_precondition("Failed to fetch blob from CAS."))?;
        // TODO should return a precondition failure / violation setup here as well.
        // https://github.com/googleapis/googleapis/blob/master/google/rpc/error_details.proto#L139
        let proto = P::decode(&mut std::io::Cursor::new(blob))
            .map_err(|_| Status::internal("Failed to decode Action proto: {action_digest}."))?;
        Ok(proto)
    }
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
        let action: protos::re::Action = self.get_proto(action_digest.into()).await?;
        log::info!("{action:#?}");

        let command_digest = action.command_digest.ok_or(Status::invalid_argument(
            "Invalid Action: no command digest specified.",
        ))?;
        let command: protos::re::Command = self.get_proto(command_digest.into()).await?;
        log::info!("{command:#?}");

        let root_digest = action.input_root_digest.ok_or(Status::invalid_argument(
            "Invalid Action: no root digest specified.",
        ))?;
        let root_directory: protos::re::Directory = self.get_proto(root_digest.into()).await?;
        log::info!("{root_directory:#?}");

        let cmd = execution_engine::Command {
            arguments: command.arguments,
        };
        let dir = execution_engine::DirectoryLayout::default();
        self.engine
            .run_command(cmd, dir)
            .await
            .map_err(|e| Status::invalid_argument("Invalid Action: no root digest specified."))?;

        Err(Status::not_found("Execute: Not yet implemented"))
    }

    type WaitExecutionStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    async fn wait_execution(
        &self,
        request: Request<protos::re::WaitExecutionRequest>,
    ) -> Result<Response<Self::WaitExecutionStream>, Status> {
        Err(Status::not_found("WaitExecution: Not yet implemented"))
    }
}
