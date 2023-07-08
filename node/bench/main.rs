use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gemsbok::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
use tokio::runtime::Runtime;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Endpoint, Uri};

fn setup_oryx_benchmark() -> (Runtime, Channel) {
    let rt = Runtime::new().expect("tokio runtime initialization");

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

    (rt, channel)
}

async fn upload_blob(mut client: Gemsbok, blob: &[u8]) {
    let _digest_of_upload = client.upload_blob(blob).await.unwrap();
}

fn upload_blob_benchmark(c: &mut Criterion) {
    let (rt, channel) = setup_oryx_benchmark();
    let mut client = Gemsbok::new(channel);

    static KB: usize = 1024;
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let blob: Vec<u8> = (0..64 * KB).map(|_| rng.gen()).collect();

    let mut group = c.benchmark_group("upload_blob");
    for size in [32, KB, 2 * KB, 8 * KB, 32 * KB, 64 * KB].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let blob = &blob[..size];
            b.to_async(&rt).iter(|| upload_blob(client.clone(), blob))
        });
    }
    group.finish();
}

criterion_group!(benches, upload_blob_benchmark);
criterion_main!(benches);
