#![allow(unused)]

use std::{collections::BTreeMap, path::PathBuf};

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
    ListDevices,
    Reset,
}

#[derive(Serialize, Deserialize)]
struct Config {
    sensor: Vec<Device>,
    target: Vec<Device>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    match args.command {
        Commands::Init => todo!(),
        Commands::Reset => todo!(),
        Commands::ListDevices => {
            select_devices()?;
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Device {
    r#type: DeviceType,

    max: Option<u32>,
    min: Option<u32>,

    rate: Option<u8>,
    sample_size: Option<u8>,
    wait: Option<u8>,

    curve: Option<Vec<u32>>,
}

#[derive(Serialize, Deserialize)]
enum DeviceType {
    Iio {
        name: String,
        channel: String,
        attribute: String,
    },
    Path {
        path: PathBuf,
        max: Option<PathBuf>,
    },
    Command {
        command: String,
        args: Vec<String>,
    },
}

fn select_devices() -> anyhow::Result<Vec<Device>> {
    // TODO better help messages
    let ctx = iio::context::Context::new()
        .context("Couldn't create iio context. Do you have iio enabled in kernel?")?;

    let (sensors, _targets): (Vec<_>, Vec<_>) = ctx
        .devices()
        .filter(|d| !d.is_trigger())
        .partition(|d| d.channels().all(|c| c.is_input()));

    println!("Found sensors:");

    for (index, device) in sensors.iter().enumerate() {
        println!(
            "{index}, name: {}\nid: {}\nchannels:",
            device.name().unwrap_or("N/A".into()),
            device.id().unwrap_or("N/A".into())
        );
        // println!("\tname\tid\tdirection",);
        println!("\tname\tid",);
        for channel in device
            .channels()
            .filter(|c| c.id().is_some_and(|n| n != "timestamp"))
        {
            println!(
                "{index}, \t{}\t{}",
                channel.name().unwrap_or("N/A".into()),
                channel.id().unwrap_or("N/A".into()),
                // channel.direction()
            );
            println!("\t\t\tattribute\tvalue");
            for (attribute, value) in BTreeMap::from_iter(channel.attr_read_all()?.iter()) {
                println!("\t\t\t{}\t{}", attribute, value);
            }
        }
    }

    todo!()
    // Ok()
}
