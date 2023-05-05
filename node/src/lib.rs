use serde::Deserialize;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncReadExt};
use tonic::transport::Server;
use tracing::info;

mod services;

use protos::*;
use services::*;

#[derive(Debug, Deserialize)]
pub enum StorageBackend {
    #[serde(alias = "memory")]
    InMemory,
}

#[derive(Debug, Deserialize)]
pub enum ExecutionEngine {
    #[serde(rename = "insecure")]
    Insecure,
    #[serde(rename = "hermetic")]
    Hermetic,
}

#[derive(Debug, Deserialize)]
pub struct NodeConfig {
    instance: String,
    address: std::net::SocketAddr,
    storage_backend: StorageBackend,
    execution_engine: ExecutionEngine,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    node: NodeConfig,
}

pub async fn start_oryx(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let address = &config.node.address;
    let instance = &config.node.instance;
    let cas = match &config.node.storage_backend {
        StorageBackend::InMemory => cas::InMemory::default(),
    };
    let execution_engine =
        execution_engine::ExecutionEngine::new(match config.node.execution_engine {
            ExecutionEngine::Insecure => execution_engine::insecure::Insecure::new(cas.clone())?,
            ExecutionEngine::Hermetic => todo!(),
        });

    let server = Server::builder()
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
        .add_service(OperationsServer::new(OperationsService::new()));
    server
        .serve(address.clone())
        .await?;

    Ok(())
}
