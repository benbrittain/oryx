use log::info;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;

#[derive(Debug)]
pub struct ContentStorageService {}

impl ContentStorageService {
    pub fn new() -> Self {
        ContentStorageService {}
    }
}

type CasResult<T> = Result<tonic::Response<T>, tonic::Status>;

#[tonic::async_trait]
impl protos::ContentAddressableStorage for ContentStorageService {
    type GetTreeStream = ReceiverStream<Result<protos::re::GetTreeResponse, Status>>;

    async fn get_tree(
        &self,
        _request: tonic::Request<protos::re::GetTreeRequest>,
    ) -> CasResult<Self::GetTreeStream> {
        info!("");
        todo!()
    }

    async fn find_missing_blobs(
        &self,
        request: tonic::Request<protos::re::FindMissingBlobsRequest>,
    ) -> CasResult<protos::re::FindMissingBlobsResponse> {
        let resp = protos::re::FindMissingBlobsResponse {
            missing_blob_digests: request.get_ref().blob_digests.clone(),
        };
        info!("Find all blobs");
        Ok(tonic::Response::new(resp))
    }

    async fn batch_update_blobs(
        &self,
        request: tonic::Request<protos::re::BatchUpdateBlobsRequest>,
    ) -> CasResult<protos::re::BatchUpdateBlobsResponse> {
        use protos::re::batch_update_blobs_response::Response;
        use protos::rpc::Status;

        let mut responses = vec![];
        for request in &request.get_ref().requests {
            responses.push(Response {
                digest: request.digest.clone(),
                status: Some(Status::default()),
            });
        }

        let resp = protos::re::BatchUpdateBlobsResponse { responses };
        info!("{resp:#?}");
        Ok(tonic::Response::new(resp))
    }

    async fn batch_read_blobs(
        &self,
        _request: tonic::Request<protos::re::BatchReadBlobsRequest>,
    ) -> CasResult<protos::re::BatchReadBlobsResponse> {
        info!("");
        todo!()
    }
}
