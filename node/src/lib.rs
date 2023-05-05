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

pub enum Connection {
    // Default gRPC over TCP
    Tcp(std::net::SocketAddr),
    // Unix Domain Socket. Used for testing.
    Uds(tokio_stream::wrappers::UnixListenerStream),
}

pub async fn start_oryx(
    instance: String,
    conn: Connection,
    storage_backend: StorageBackend,
    execution_engine: ExecutionEngine,
) -> Result<(), Box<dyn std::error::Error>> {
    let cas = match storage_backend {
        StorageBackend::InMemory => cas::InMemory::default(),
    };
    let execution_engine = execution_engine::ExecutionEngine::new(match execution_engine {
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
    let conn = async {
        match conn {
            Connection::Tcp(address) => {
                server.serve(address.clone()).await?;
            }
            Connection::Uds(uds_stream) => {
                server.serve_with_incoming(uds_stream).await?;
            }
        }
        Ok(())
    };
    conn.await
}
