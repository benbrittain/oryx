use crate::MetadataMap;
use cas::*;
use opentelemetry::global;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;
use tracing::{event, span, Level};
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
        todo!()
    }

    async fn find_missing_blobs(
        &self,
        request: tonic::Request<protos::re::FindMissingBlobsRequest>,
    ) -> CasResult<protos::re::FindMissingBlobsResponse> {
        let span = span!(Level::TRACE, "gRPC find_missing_blobs");
        let mut missing_blob_digests = vec![];
        for digest in request.into_inner().blob_digests {
            let cdigest = digest.clone().into();
            let span = span!(Level::TRACE, "Find missing blob", digest = %&cdigest);
            if !self
                .cas
                .has_blob(&cdigest)
                .await
                .map_err(|e| tonic::Status::unknown(e.to_string()))?
            {
                event!(Level::INFO, digest = ?digest, "digest missing");
                missing_blob_digests.push(digest);
            }
        }

        let resp = protos::re::FindMissingBlobsResponse {
            missing_blob_digests,
        };

        Ok(tonic::Response::new(resp))
    }

    async fn batch_update_blobs(
        &self,
        request: tonic::Request<protos::re::BatchUpdateBlobsRequest>,
    ) -> CasResult<protos::re::BatchUpdateBlobsResponse> {
        let span = span!(Level::TRACE, "gRPC batch_update_blobs");
        use protos::re::batch_update_blobs_response::Response;
        use protos::rpc::{Code, Status};

        let mut responses = vec![];
        for request in &request.get_ref().requests {
            let digest = request.digest.clone().unwrap_or_default().into();
            let span = span!(Level::TRACE, "Update blob", digest = %&digest);
            match self.cas.write_blob(&request.data, Some(digest)).await {
                Ok(_) => {
                    event!(parent: &span, Level::INFO, "wrote blob");
                    responses.push(Response {
                        digest: request.digest.clone(),
                        status: Some(Status::default()),
                    });
                }
                Err(CasError::InvalidDigest(d1, d2)) => {
                    event!(parent: &span, Level::INFO, "invalid digest");
                    responses.push(Response {
                        digest: request.digest.clone(),
                        status: Some(Status {
                            code: Code::InvalidArgument.into(),
                            ..Default::default()
                        }),
                    });
                }
                Err(e) => return Err(tonic::Status::unknown(e.to_string())),
            }
        }

        let resp = protos::re::BatchUpdateBlobsResponse { responses };
        Ok(tonic::Response::new(resp))
    }

    async fn batch_read_blobs(
        &self,
        request: tonic::Request<protos::re::BatchReadBlobsRequest>,
    ) -> CasResult<protos::re::BatchReadBlobsResponse> {
        let span = span!(Level::TRACE, "gRPC batch_read_blobs");
        let request = request.into_inner();

        let mut responses = vec![];
        for digest in &request.digests {
            let cdigest = digest.clone().into();
            let span = span!(Level::TRACE, "Read blob", digest = %&cdigest);
            let blob = self
                .cas
                .read_blob(cdigest)
                .await
                .map_err(|e| tonic::Status::unknown(format!("Reading: {}", e)))?;
            responses.push(protos::re::batch_read_blobs_response::Response {
                digest: Some(digest.clone()),
                data: blob,
                compressor: protos::re::compressor::Value::Identity as i32,
                status: Default::default(),
            });
        }
        let resp = protos::re::BatchReadBlobsResponse { responses };
        Ok(tonic::Response::new(resp))
    }
}
