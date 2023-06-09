use futures::Future;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Endpoint, Uri};

mod cas;
mod execute;

pub async fn oryx_test<F, FRet>(client_test_fut: F)
where
    F: FnOnce(Channel) -> FRet,
    FRet: Future<Output = ()>,
{
    // Create a new UDS file
    let socket = NamedTempFile::new().unwrap();
    let socket = Arc::new(socket.into_temp_path());
    std::fs::remove_file(&*socket).unwrap();

    let uds = UnixListener::bind(&*socket).unwrap();
    let stream = UnixListenerStream::new(uds);

    // Create a new oryx instance
    let server_fut = async {
        let result = node_lib::start_oryx(
            String::from(""),
            node_lib::Connection::Uds(stream),
            node_lib::StorageBackend::InMemory,
            node_lib::ExecutionEngine::Insecure,
        )
        .await;
        assert!(result.is_ok());
    };

    // Create a UDS connection to the oryx instance
    let socket = Arc::clone(&socket);
    let channel = Endpoint::try_from("http://oryx.build")
        .unwrap()
        .connect_with_connector(tower::service_fn(move |_: Uri| {
            let socket = Arc::clone(&socket);
            async move { UnixStream::connect(&*socket).await }
        }))
        .await
        .unwrap();

    // Run the client future
    let client_fut = async move { client_test_fut(channel).await };

    // Run both futures to completion
    tokio::select! {
        _ = server_fut => panic!("Server ended execution before client."),
        _ = client_fut => (),
    }
}
