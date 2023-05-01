use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::error;

#[derive(Debug)]
pub struct BytestreamService {}

impl BytestreamService {
    pub fn new() -> Self {
        BytestreamService {}
    }
}

#[tonic::async_trait]
impl protos::ByteStream for BytestreamService {
    type ReadStream = ReceiverStream<Result<protos::bytestream::ReadResponse, Status>>;

    async fn read(
        &self,
        request: Request<protos::bytestream::ReadRequest>,
    ) -> Result<Response<Self::ReadStream>, Status> {
        todo!();
    }

    async fn write(
        &self,
        request: Request<tonic::Streaming<protos::bytestream::WriteRequest>>,
    ) -> Result<Response<protos::bytestream::WriteResponse>, Status> {
        error!("{request:#?}");
        todo!();
    }

    async fn query_write_status(
        &self,
        _request: Request<protos::bytestream::QueryWriteStatusRequest>,
    ) -> Result<Response<protos::bytestream::QueryWriteStatusResponse>, Status> {
        todo!();
    }
}
