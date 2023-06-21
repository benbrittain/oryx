use crate::*;
use cas::ContentAddressableStorage;
use futures::future::{BoxFuture, FutureExt};
use prost::Message;
use tokio::io::AsyncReadExt;
use tokio::{fs::File, io::AsyncWriteExt, process};
use tracing::instrument;


#[derive(Debug, Clone)]
pub struct Hermetic<C> {
    cas: C,
}

impl<C: ContentAddressableStorage> Hermetic<C> {
    pub fn new(cas: C) -> Result<Self, ExecuteError> {
        Ok(Hermetic { cas })
    }
}

#[async_trait]
impl<C: ContentAddressableStorage> ExecutionBackend for Hermetic<C> {
    #[instrument(skip_all)]
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<ExecuteResponse, ExecuteError> {
        todo!();
    }
}
