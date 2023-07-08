use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::signal;
use tokio::{fs::File, io::AsyncReadExt};
use toml::Table;
use tracing::{info, span};

use opentelemetry::global;
use tracing_subscriber::{fmt, layer::SubscriberExt, registry::Registry, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "oryx-node")]
#[command(author = "Benjamin Brittain. <ben@brittain.org>")]
#[command(version = "0.1")]
#[command(about = "Oryx Remote Build Execution node", long_about = None)]
struct Args {
    /// Path to oryx configuration file
    #[arg(long)]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct NodeConfig {
    instance: String,
    address: std::net::SocketAddr,
    storage_backend: node_lib::StorageBackend,
    execution_engine: node_lib::ExecutionEngine,
    trace: bool,
}

/// Read the oryx node config
async fn read_config(config_file: PathBuf) -> Result<NodeConfig, Box<dyn std::error::Error>> {
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

    if config.trace {
        global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
        let tracer = opentelemetry_jaeger::new_agent_pipeline()
            .with_auto_split_batch(true)
            .with_service_name("oryx")
            .install_batch(opentelemetry::runtime::Tokio)?;
        let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        tracing_subscriber::registry()
            .with(opentelemetry)
            .with(fmt::Layer::default())
            .try_init()?;
        Some(())
    } else {
        tracing_subscriber::fmt::init();
        None
    };
    let root = span!(tracing::Level::TRACE, "oryx", work_units = 2);
    info!("Initialized");

    let oryx_fut = node_lib::start_oryx(
        config.instance,
        node_lib::Connection::Tcp(config.address),
        config.storage_backend,
        config.execution_engine,
    );

    tokio::select! {
        _ = signal::ctrl_c() => (),
        _ = oryx_fut => unreachable!(),
    }

    global::shutdown_tracer_provider();
    Ok(())
}
