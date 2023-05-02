use tonic::{Request, Response, Status};
use tracing::instrument;

#[derive(Debug, Default)]
pub struct ActionCacheService {}

#[tonic::async_trait]
impl protos::ActionCache for ActionCacheService {
    #[instrument(skip_all)]
    async fn get_action_result(
        &self,
        _request: Request<protos::re::GetActionResultRequest>,
    ) -> Result<Response<protos::re::ActionResult>, Status> {
        Err(Status::not_found("GetActionResult: Not yet implemented"))
    }

    #[instrument(skip_all)]
    async fn update_action_result(
        &self,
        _request: Request<protos::re::UpdateActionResultRequest>,
    ) -> Result<Response<protos::re::ActionResult>, Status> {
        Err(Status::not_found("UpdateActionResult: Not yet implemented"))
    }
}
