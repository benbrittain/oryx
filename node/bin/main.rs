use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncReadExt};
use toml::Table;
use tracing::info;

use tracing_chrome::{ChromeLayerBuilder, TraceStyle};
use tracing_subscriber::{prelude::*, registry::Registry};

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
pub struct NodeConfig {
    instance: String,
    address: std::net::SocketAddr,
    storage_backend: node_lib::StorageBackend,
    execution_engine: node_lib::ExecutionEngine,
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

    node_lib::start_oryx(
        config.instance,
        node_lib::Connection::Tcp(config.address),
        config.storage_backend,
        config.execution_engine,
    )
    .await?;
    Ok(())
}
