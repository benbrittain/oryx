use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use cas::ContentAddressableStorage;
use std::str::FromStr;
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use tracing::{info, error, instrument};
use common::Digest;

#[derive(Debug)]
pub struct BytestreamService<T> {
    cas: T,
}

impl<T> BytestreamService<T> {
    pub fn new(cas: T) -> Self {
        BytestreamService {
            cas,
        }
    }
}

#[tonic::async_trait]
impl<T: ContentAddressableStorage> protos::ByteStream for BytestreamService<T> {
    type ReadStream = ReceiverStream<Result<protos::bytestream::ReadResponse, Status>>;

    #[instrument(skip(self))]
    async fn read(
        &self,
        request: Request<protos::bytestream::ReadRequest>,
    ) -> Result<Response<Self::ReadStream>, Status> {
        let request = request.into_inner();

        // TODO support other offsets
        assert_eq!(request.read_offset, 0);

        let digest = Digest::from_blob_str(&request.resource_name).map_err(|e| {
            tonic::Status::invalid_argument(format!("Invalid digest: {e:?}"))
        })?;

        let (tx, rx) = mpsc::channel(32);
        let cas = self.cas.clone();

        tokio::spawn(async move {
            info!("digest: {digest}");
            // TODO Don't load whole blob into memory, stream from CAS.
            match cas.read_blob(digest).await {
                Ok(blob) => {
                    for chunk in blob.chunks(1024) {
                        tx.send(Ok(protos::bytestream::ReadResponse {
                            data: chunk.to_vec(),
                        }));
                    }
                },
                Err(_) => {
                    tx.send(Err(tonic::Status::not_found(format!("Blob not found"))));
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx) as Self::ReadStream))
    }

    #[instrument(skip(self))]
    async fn write(
        &self,
        request: Request<tonic::Streaming<protos::bytestream::WriteRequest>>,
    ) -> Result<Response<protos::bytestream::WriteResponse>, Status> {
        let mut blob = vec![];
        let mut stream = request.into_inner();

        let mut digest = None;
        while let Some(req) = stream.next().await {
            let req = req?;
            if digest.is_none() {
                digest = Some(Digest::from_blob_str(&req.resource_name)
                    .map_err(|e| {
                    tonic::Status::invalid_argument(format!("Invalid digest: {e:?}"))
                })?);
            }
            assert_eq!(req.write_offset, blob.len() as i64);
            blob.extend(req.data);

            if req.finish_write {
                assert!(digest.is_some());
                self.cas.write_blob(&blob, digest).await.map_err(|e| {
                    tonic::Status::internal(format!("Invalid blob write: {e:?}"))
                })?;
                return Ok(Response::new(protos::bytestream::WriteResponse {
                    committed_size: blob.len() as i64,
                }));
            }
        }
        Err(tonic::Status::unknown(format!("Write did not succeed")))
    }

    #[instrument(skip_all)]
    async fn query_write_status(
        &self,
        _request: Request<protos::bytestream::QueryWriteStatusRequest>,
    ) -> Result<Response<protos::bytestream::QueryWriteStatusResponse>, Status> {
        Err(tonic::Status::unknown(format!("Not Supported at this time.")))
    }
}
