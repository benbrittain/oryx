use clap::Parser;
use log::info;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncReadExt};
use toml::Table;
use tonic::transport::Server;

use services::*;
use protos::*;

#[derive(Parser, Debug)]
#[command(name = "oryx-node")]
#[command(author = "Benjamin Brittain. <ben@brittain.org>")]
#[command(version = "0.1")]
#[command(about = "Oryx Remote Build Execution node", long_about = None)]
struct Args {
    #[arg(long)]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
enum StorageBackend {
    #[serde(alias = "memory")]
    InMemory,
}


#[derive(Debug, Deserialize)]
struct Config {
    address: std::net::SocketAddr,
    storage_backend: StorageBackend,
}

/// Read the oryx node config
async fn read_config(config_file: PathBuf) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(config_file).await?;
    let mut contents = vec![];
    file.read_to_end(&mut contents).await?;
    let contents = std::str::from_utf8(&contents)?;
    Ok(toml::from_str(contents)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let args = Args::parse();

    let config = read_config(args.config).await?;

    let action_cache = ActionCacheService::default();
    let bytestream = BytestreamService::new();
    let capabilities = CapabilitiesService::default();
    let cas = ContentStorageService::new(match config.storage_backend {
        StorageBackend::InMemory => cas::InMemory::default(),
    });
    let execute = ExecutionService::new();
    let ops = OperationsService::new();

    let address = config.address;
    info!("Serving on {}", address);
    Server::builder()
        .add_service(ActionCacheServer::new(action_cache))
        .add_service(ByteStreamServer::new(bytestream))
        .add_service(CapabilitiesServer::new(capabilities))
        .add_service(ContentAddressableStorageServer::new(cas))
        .add_service(ExecutionServer::new(execute))
        .add_service(OperationsServer::new(ops))
        .serve(address)
        .await?;

    Ok(())
}
