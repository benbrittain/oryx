use cas::*;
use log::info;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;

#[derive(Debug)]
pub struct ContentStorageService<T> {
    cas: T,
}

impl<T> ContentStorageService<T> {
    pub fn new(cas: T) -> Self {
        ContentStorageService { cas }
    }
}

type CasResult<T> = Result<tonic::Response<T>, tonic::Status>;

#[tonic::async_trait]
impl<T: ContentAddressableStorage> protos::ContentAddressableStorage for ContentStorageService<T> {
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
        let mut missing_blob_digests = vec![];
        for digest in request.into_inner().blob_digests {
            if !self
                .cas
                .has_blob(&digest.clone().into())
                .await
                .map_err(|e| tonic::Status::unknown(e.to_string()))?
            {
                missing_blob_digests.push(digest);
            }
        }

        info!("Missing {} blobs", missing_blob_digests.len());
        let resp = protos::re::FindMissingBlobsResponse {
            missing_blob_digests,
        };

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
            let digest = request.digest.clone().unwrap_or_default().into();
            self.cas
                .write_blob(digest, &request.data)
                .await
                .map_err(|e| tonic::Status::unknown(e.to_string()))?;
            responses.push(Response {
                digest: request.digest.clone(),
                status: Some(Status::default()),
            });
        }

        let resp = protos::re::BatchUpdateBlobsResponse { responses };
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
