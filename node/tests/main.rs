use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tonic::Request;

#[tokio::test]
async fn simple_query() {
    let socket = NamedTempFile::new().unwrap();
    let socket = Arc::new(socket.into_temp_path());
    std::fs::remove_file(&*socket).unwrap();

    let uds = UnixListener::bind(&*socket).unwrap();
    let stream = UnixListenerStream::new(uds);

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

    let socket = Arc::clone(&socket);
    let channel = Endpoint::try_from("http://oryx.build")
        .unwrap()
        .connect_with_connector(tower::service_fn(move |_: Uri| {
            let socket = Arc::clone(&socket);
            async move { UnixStream::connect(&*socket).await }
        }))
        .await
        .unwrap();

    let mut client = protos::ContentAddressableStorageClient::new(channel);

    let client_fut = async {
        let response = client
            .find_missing_blobs(Request::new(protos::re::FindMissingBlobsRequest {
                blob_digests: vec![],
                instance_name: "".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.missing_blob_digests, vec![]);
    };

    tokio::select! {
        _ = server_fut => panic!("Server ended execution before client."),
        _ = client_fut => (),
    }
}
