use anyhow::Error;
use async_trait::async_trait;

pub mod insecure;

#[derive(Default, Debug)]
pub struct Command {
    pub arguments: Vec<String>,
}

#[derive(Default, Debug)]
pub struct DirectoryLayout {}

pub struct CommandResponse {}

#[async_trait]
pub trait ExecutionEngine: Send + Sync + 'static {
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<CommandResponse, Error>;
}
