use crate::*;
use anyhow::Error;
use tokio::process;

#[derive(Default, Debug)]
pub struct Insecure {}

#[async_trait]
impl ExecutionEngine for Insecure {
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<CommandResponse, Error> {
        let binary = &command.arguments[0];
        let args = &command.arguments[1..];
        let mut command = process::Command::new(binary);
        command.args(args);
        todo!()
    }
}
