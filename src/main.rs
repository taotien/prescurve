use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
struct Args {
    config: Option<PathBuf>,

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

#[derive(Serialize, Deserialize)]
struct Device {
    path: Option<PathBuf>,
    max_path: Option<PathBuf>,

    command: Option<String>,
    args: Option<Vec<String>>,

    max: Option<u32>,
    min: Option<u32>,

    rate: Option<u8>,
    sample_size: Option<u8>,
    wait: Option<u8>,

    curve: Option<Vec<u32>>,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    match args.command {
        Commands::Init => todo!(),
        Commands::Reset => todo!(),
    }

    Ok(())
}
