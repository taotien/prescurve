use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use industrial_io as iio;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Reset,
}

#[derive(Serialize, Deserialize)]
struct Config {
    sensor: Vec<Device>,
    target: Vec<Device>,
}

/// pick one of iio_device, paths, or command
#[derive(Serialize, Deserialize)]
struct Device {
    /// iio_device
    iio_device: Option<String>,
    /// paths
    path: Option<PathBuf>,
    max_path: Option<PathBuf>,
    // /// command
    // command: Option<String>,
    // args: Option<Vec<String>>,
    max: Option<u32>,
    min: Option<u32>,

    rate: Option<u8>,
    sample_size: Option<u8>,
    wait: Option<u8>,

    curve: Option<Vec<u32>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    match args.command {
        Commands::Init => todo!(),
        Commands::Reset => todo!(),
    }

    Ok(())
}

fn find_device() -> anyhow::Result<Device> {
    // TODO better help messages
    let ctx = iio::context::Context::new()
        .context("Couldn't create iio context. Do you have iio enabled in kernel?")?;

    todo!()
}
