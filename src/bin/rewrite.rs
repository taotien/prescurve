use std::{collections::VecDeque, fs::read_to_string, iter::Sum, path::PathBuf, time::Duration};

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use num_traits::Num;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::watch::{self, Sender},
    time,
};
// use toml::Value;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Config {
    settings: Settings,
    devices: Vec<Device>,
    curve: Curve,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            devices: Vec::new(),
            curve: Curve::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Settings {
    fps: Option<u8>,
    sample_frequency: Option<u16>,
    sample_size: Option<u8>,
    manual_adjust_wait: Option<u8>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fps: Some(60),
            sample_frequency: Some(1000),
            sample_size: Some(5),
            manual_adjust_wait: Some(5),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Curve {
    keys: Vec<toml::Value>,
    values: Vec<toml::Value>,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Device {
    device_type: DeviceType,
    value_type: ValueType,
    path: PathBuf,
    max_path: Option<PathBuf>,
    max: Option<toml::Value>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum ValueType {
    Float64,
    Int64,
}

// #[derive(Clone, Serialize, Deserialize, PartialEq)]
// enum Value {
//     Float64(f64),
//     Int64(i64),
//     Zero,
// }

#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum DeviceType {
    Sensor,
    Target,
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    subcommand: Commands,
    debug: Option<bool>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Fork,
    Visualize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut config = Config::load_from_file()
        .await
        .context("Could not load config!")?;

    // let (config_tx, config_rx) = watch::channel(config.clone());

    // // monitor the config file
    // tokio::spawn(async move {
    //     loop {
    //         time::sleep(Duration::from_secs(1)).await;

    //         let new_config = Config::load_from_file()
    //             .await
    //             .expect("Could not load config!");

    //         if new_config != config {
    //             config_tx.send(new_config.clone()).unwrap();
    //             config = new_config;
    //         }
    //     }
    // });

    let sensor = config
        .devices
        .iter()
        .find(|device| device.device_type == DeviceType::Sensor)
        .context("No sensor device found!")?;

    // let mut samples = VecDeque::with_capacity(config.settings.sample_size.unwrap().into());
    // for _ in 0..10 {
    //     samples.push_back(sensor.get()?);
    // }
    // let
    let current = sensor.get()?;
    let (average_tx, average_rx) = watch::channel(current);

    // monitor the sensor
    tokio::spawn(async move {});

    // match args.subcommand {
    //     Commands::Init => {
    //         todo!();
    //     }
    //     Commands::Fork => {
    //         todo!();
    //     }
    //     Commands::Visualize => {
    //         todo!();
    //     }
    // }

    Ok(())
}

impl Config {
    async fn load_from_file() -> anyhow::Result<Self> {
        let mut config_path =
            dirs::config_dir().context("Could not find user config directory!")?;
        config_path.push("prescurve.toml");
        let config_text = read_to_string(config_path);
        let config = if let Ok(text) = config_text {
            toml::from_str(&text).context("Failed to parse config at {config_path}!")?
        } else {
            Config::default()
            // TODO prompt user to init
        };

        Ok(config)
    }
}

impl Device {
    fn get(&self) -> anyhow::Result<impl Num> {
        let value = read_to_string(&self.path)?;
        // match self.value_type {
        //     ValueType::Int64 => Ok(Num::from_str_radix(&value.trim(), 10)),
        //     ValueType::Float64 => {
        //         Ok(Value::Float64(value))
        //     }
        // }
        let result = Num::from_str_radix(&value.trim(), 10)?;
        Ok(result)
    }
}

async fn watch_sensor(
    sample_size: u8,
    sensor: &Device,
    tx: Sender<impl Num>,
) -> anyhow::Result<()> {
    let mut samples = VecDeque::with_capacity(sample_size.into());
    let mut average = sensor.get()?;
    loop {
        samples.push_back(sensor.get()?);
        samples.pop_front();

        let new_average = samples.iter().sum::<impl Num>() / sample_size;

        // if new_average != average {
        //     tx.send(new_average).unwrap();
        //     average = new_average;
        // }
        time::sleep(Duration::from_secs(1)).await;
    }
}
