use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncReadExt};
use toml::Table;
use tonic::transport::Server;
use tracing::info;

use tracing_chrome::{ChromeLayerBuilder, TraceStyle};
use tracing_subscriber::{prelude::*, registry::Registry};

mod services;

use protos::*;
use services::*;

#[derive(Parser, Debug)]
#[command(name = "oryx-node")]
#[command(author = "Benjamin Brittain. <ben@brittain.org>")]
#[command(version = "0.1")]
#[command(about = "Oryx Remote Build Execution node", long_about = None)]
struct Args {
    /// Path to oryx configuration file
    #[arg(long)]
    config: PathBuf,

    /// Enable traces for visualizing in https://ui.perfetto.dev
    #[arg(long)]
    trace: bool,
}

#[derive(Debug, Deserialize)]
enum StorageBackend {
    #[serde(alias = "memory")]
    InMemory,
}

#[derive(Debug, Deserialize)]
enum ExecutionEngine {
    #[serde(rename = "insecure")]
    Insecure,
    #[serde(rename = "hermetic")]
    Hermetic,
}

#[derive(Debug, Deserialize)]
struct NodeConfig {
    instance: String,
    address: std::net::SocketAddr,
    storage_backend: StorageBackend,
    execution_engine: ExecutionEngine,
}

#[derive(Debug, Deserialize)]
struct Config {
    node: NodeConfig,
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
    let args = Args::parse();

    let config = read_config(args.config).await?;

    let _guard = if args.trace {
        let (chrome_layer, guard) = ChromeLayerBuilder::new()
            .include_args(true)
            .trace_style(TraceStyle::Async)
            .build();
        tracing_subscriber::registry().with(chrome_layer).init();
        Some(guard)
    } else {
        tracing_subscriber::fmt::init();
        None
    };

    let address = config.node.address;
    let instance = config.node.instance;
    let cas = match config.node.storage_backend {
        StorageBackend::InMemory => cas::InMemory::default(),
    };
    let execution_engine =
        execution_engine::ExecutionEngine::new(match config.node.execution_engine {
            ExecutionEngine::Insecure => execution_engine::insecure::Insecure::new(cas.clone())?,
            ExecutionEngine::Hermetic => todo!(),
        });

    eprintln!("Serving instance '{instance}' on {address}");

    Server::builder()
        .trace_fn(|event| tracing::info_span!("gRPC Request", api = event.uri().path()))
        .add_service(ActionCacheServer::new(ActionCacheService::default()))
        .add_service(ByteStreamServer::new(BytestreamService::new()))
        .add_service(CapabilitiesServer::new(CapabilitiesService::default()))
        .add_service(ContentAddressableStorageServer::new(
            ContentStorageService::new(cas.clone()),
        ))
        .add_service(ExecutionServer::new(ExecutionService::new(
            &instance,
            cas,
            execution_engine,
        )))
        .add_service(OperationsServer::new(OperationsService::new()))
        .serve(address)
        .await?;

    Ok(())
}
