use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gemsbok::*;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Endpoint, Uri};

async fn upload_digest(mut client: Gemsbok) {
    let _digest_of_upload = client
        .add_command(&["/bin/sh", "-c", "echo 'rbe'"], &[])
        .await
        .unwrap();
}

fn upload_digest_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime initialization");

    // Set up the stream
    let (stream, socket) = rt.block_on(async {
        // Create a new UDS file
        let socket = NamedTempFile::new().unwrap();
        let socket = Arc::new(socket.into_temp_path());
        std::fs::remove_file(&*socket).unwrap();

        let uds = UnixListener::bind(&*socket).unwrap();
        (UnixListenerStream::new(uds), socket)
    });

    // Start the server
    rt.spawn(async {
        let result = node_lib::start_oryx(
            String::from(""),
            node_lib::Connection::Uds(stream),
            node_lib::StorageBackend::InMemory,
            node_lib::ExecutionEngine::Insecure,
        )
        .await;
        assert!(result.is_ok());
    });

    // Create a channel for a client to connect on
    let channel = rt.block_on(async {
        // Create a UDS connection to the oryx instance
        let socket = Arc::clone(&socket);
        Endpoint::try_from("http://oryx.build")
            .unwrap()
            .connect_with_connector(tower::service_fn(move |_: Uri| {
                let socket = Arc::clone(&socket);
                async move { UnixStream::connect(&*socket).await }
            }))
            .await
            .unwrap()
    });

    let mut client = Gemsbok::new(channel);

    c.bench_function("upload_command", |b| {
        b.to_async(&rt).iter(|| upload_digest(client.clone()))
    });
}

criterion_group!(benches, upload_digest_benchmark);
criterion_main!(benches);
