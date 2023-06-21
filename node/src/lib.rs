use serde::Deserialize;
use tonic::transport::server::Router;
use tonic::transport::Server;

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

fn add_exec_service<C: cas::ContentAddressableStorage>(
    s: Router,
    instance: &str,
    execution_engine: ExecutionEngine,
    cas: C,
) -> Result<Router, Box<dyn std::error::Error>> {
    Ok(match execution_engine {
        ExecutionEngine::Insecure => {
            let backend = execution_engine::insecure::Insecure::new(cas.clone())?;
            let execution_engine = execution_engine::ExecutionEngine::new(backend);
            let server =
                ExecutionServer::new(ExecutionService::new(&instance, cas, execution_engine));
            s.add_service(server)
        }
        ExecutionEngine::Hermetic => {
            let backend = execution_engine::hermetic::Hermetic::new(cas.clone())?;
            let execution_engine = execution_engine::ExecutionEngine::new(backend);
            let server =
                ExecutionServer::new(ExecutionService::new(&instance, cas, execution_engine));
            s.add_service(server)
        }
    })
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

    let server = Server::builder()
        .trace_fn(|event| tracing::info_span!("gRPC Request", api = event.uri().path()))
        .add_service(ActionCacheServer::new(ActionCacheService::default()))
        .add_service(ByteStreamServer::new(BytestreamService::new(cas.clone())))
        .add_service(CapabilitiesServer::new(CapabilitiesService::default()))
        .add_service(ContentAddressableStorageServer::new(
            ContentStorageService::new(cas.clone()),
        ))
        .add_service(OperationsServer::new(OperationsService::new()));
    let server = add_exec_service(server, &instance, execution_engine, cas)?;

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
