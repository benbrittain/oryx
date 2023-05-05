use tonic::{Request, Response, Status};

pub struct OperationsService {}
impl OperationsService {
    pub fn new() -> Self {
        OperationsService {}
    }
}

#[tonic::async_trait]
impl protos::Operations for OperationsService {
    async fn list_operations(
        &self,
        request: Request<protos::longrunning::ListOperationsRequest>,
    ) -> Result<Response<protos::longrunning::ListOperationsResponse>, Status> {
        todo!()
    }

    async fn get_operation(
        &self,
        request: Request<protos::longrunning::GetOperationRequest>,
    ) -> Result<Response<protos::longrunning::Operation>, Status> {
        todo!()
    }

    async fn delete_operation(
        &self,
        request: Request<protos::longrunning::DeleteOperationRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn cancel_operation(
        &self,
        request: Request<protos::longrunning::CancelOperationRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn wait_operation(
        &self,
        request: Request<protos::longrunning::WaitOperationRequest>,
    ) -> Result<Response<protos::longrunning::Operation>, Status> {
        todo!()
    }
}
