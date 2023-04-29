use tonic::{Request, Response, Status};

#[derive(Debug, Default)]
pub struct ActionCacheService {}

#[tonic::async_trait]
impl protos::ActionCache for ActionCacheService {
    async fn get_action_result(
        &self,
        _request: Request<protos::re::GetActionResultRequest>,
    ) -> Result<Response<protos::re::ActionResult>, Status> {
        Err(Status::not_found("GetActionResult: Not yet implemented"))
    }

    async fn update_action_result(
        &self,
        _request: Request<protos::re::UpdateActionResultRequest>,
    ) -> Result<Response<protos::re::ActionResult>, Status> {
        Err(Status::not_found("UpdateActionResult: Not yet implemented"))
    }
}
