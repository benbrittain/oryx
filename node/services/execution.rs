use cas::ContentAddressableStorage;
use prost::Message;
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

#[tonic::async_trait]
impl<T: Sync + Send + 'static, C: Sync + Send + 'static + ContentAddressableStorage>
    protos::Execution for ExecutionService<T, C>
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

        let action_digest = request.action_digest.ok_or(Status::invalid_argument(
            "An Action Digest was not specified in the ExecuteRequest.",
        ))?;

        let action: Result<protos::re::Action, prost::DecodeError> = self
            .cas
            .read_blob(&action_digest.into())
            .await
            .map_err(|_| Status::invalid_argument("Failed to fetch blob from CAS."))
            .map(|buf| protos::re::Action::decode(&mut std::io::Cursor::new(buf)))?;

        let action = action.map_err(|_| {
            Status::invalid_argument("Failed to decode Action proto: {action_digest}.")
        })?;

        log::info!("{action:#?}");

        let command_digest = action.command_digest.ok_or(Status::invalid_argument(
            "Invalid Action: no command digest specified.",
        ))?;
        let root_digest = action
            .input_root_digest
            .ok_or(Status::invalid_argument("Invalid Action: no root digest specified."))?;

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
