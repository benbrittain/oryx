use clap::Parser;
use log::info;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncReadExt};
use toml::Table;
use tonic::transport::Server;

#[derive(Parser, Debug)]
#[command(name = "oryx-node")]
#[command(author = "Benjamin Brittain. <ben@brittain.org>")]
#[command(version = "0.1")]
#[command(about = "Oryx Remote Build Execution node", long_about = None)]
struct Args {
    #[arg(long)]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Config {
    address: std::net::SocketAddr,
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
    pretty_env_logger::init();
    let args = Args::parse();
    let config = read_config(args.config).await?;

    let capabilities = capabilities::CapabilitiesService::default();

    let address = config.address;
    info!("Serving on {}", address);
    Server::builder()
        .add_service(protos::CapabilitiesServer::new(capabilities))
        .serve(address)
        .await?;

    Ok(())
}
