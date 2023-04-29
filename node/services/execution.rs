use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub struct ExecutionService<T> {
    engine: T,
}

impl<T> ExecutionService<T> {
    pub fn new(engine: T) -> Self {
        ExecutionService { engine }
    }
}

#[tonic::async_trait]
impl<T: Sync + Send + 'static> protos::Execution for ExecutionService<T> {
    type ExecuteStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    async fn execute(
        &self,
        request: Request<protos::re::ExecuteRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
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
