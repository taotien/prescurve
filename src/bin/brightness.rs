use std::{
    collections::{BTreeMap, VecDeque},
    fs::{read_to_string, write},
    path::PathBuf,
    process::exit,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinHandle, time::sleep, try_join};

use prescurve::{Curve, DeviceRead, DeviceWrite, Interpolate, Monotonic, Smooth};

#[derive(Deserialize, Serialize)]
struct Config {
    device_path: String,
    device_max_path: Option<String>,
    device_max: Option<u32>,

    sensor_path: String,
    sensor_max_path: Option<String>,
    sensor_max: Option<u32>,

    fps: Option<u8>,
    sample_frequency: Option<u16>,
    sample_size: Option<u8>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // TODO prompt user for setup (sensor_max)
    // let mut config_path = dirs::config_dir().context("couldn't find config dir")?;
    let mut config_path = PathBuf::from("/home/tao/.config/");
    config_path.push("prescurve.toml");
    let config: Config = toml::from_str(
        read_to_string(config_path)
            .context("couldn't find config")?
            .as_ref(),
    )
    .context("couldn't parse config")?;

    let mut backlight = Backlight {
        path: PathBuf::from(config.device_path),
        requested: 0,
        max: if let Some(max) = config.device_max {
            max
        } else {
            read_to_string(config.device_max_path.clone().unwrap())
                .context("couldn't read backlight value")?
                .trim()
                .parse()?
        },
    };
    backlight.requested = backlight.get()?;

    let ambient = Ambient {
        path: PathBuf::from(config.sensor_path),
        // TODO read this from config and/or prompt user (setup wizard?)
        max: 3355,
    };
    let ambient_arc = Arc::new(ambient);

    let mut points = BTreeMap::new();
    points.insert(0, 1);
    points.insert(ambient_arc.max, backlight.max);
    // points.insert(ambient_arc.get()?, backlight.get()?);
    let mut curve = Curve { points };

    let average = Arc::new(AtomicU32::new(0));
    let average_arc = Arc::clone(&average);

    let ambient = Arc::clone(&ambient_arc);
    let average = Arc::clone(&average_arc);
    let sample: JoinHandle<Result<()>> = tokio::spawn(async move {
        let duration = Duration::from_millis(config.sample_frequency.unwrap_or(1000).into());
        let mut samples: VecDeque<u32> =
            VecDeque::with_capacity(config.sample_size.unwrap_or(10).into());
        for _ in 0..10 {
            samples.push_back(ambient.get()?);
        }
        loop {
            let sensor = ambient.get()?;
            samples.pop_front();
            samples.push_back(sensor);
            // println!("{:?}", samples);
            average.store(
                samples.iter().sum::<u32>() / samples.len() as u32,
                Ordering::Release,
            );
            // println!("{}", average.load(Ordering::Relaxed));
            sleep(duration).await;
        }
    });

    let ambient = Arc::clone(&ambient_arc);
    let average = Arc::clone(&average_arc);
    let fps = config.fps.unwrap_or(60);
    let freq = config.sample_frequency.unwrap_or(100);
    let sample_size = config.sample_size.unwrap_or(10);
    let adjust_retain: JoinHandle<Result<()>> = tokio::spawn(async move {
        loop {
            let target = curve.search_interpolate(&average.load(Ordering::Acquire));
            let diff = target as i32 - backlight.get()? as i32;
            match backlight.changed()? {
                false => {
                    if backlight.get()? != target {
                        Curve::adjust(diff, &mut backlight, fps)?;
                        sleep(Duration::from_millis(1000 / fps as u64)).await;
                    } else {
                        sleep(Duration::from_millis(freq as u64 * sample_size as u64)).await;
                    }
                }
                // backlight was manually/externally adjusted
                true => {
                    // TODO read this from config
                    sleep(Duration::from_secs(5)).await;
                    let current = backlight.get()?;

                    // user requested black screen temporarily, don't save
                    if current == 0 {
                        continue;
                    }

                    backlight.requested = current;
                    curve.add(average.load(Ordering::Acquire), current, ambient.max);
                }
            }
        }
    });

    let res = try_join![sample, adjust_retain];
    match res {
        Err(e) => {
            eprintln!("Error occured: {e}");
            exit(1);
        }
        Ok(_) => {
            unreachable!()
        }
    }
}
struct Backlight {
    path: PathBuf,
    max: u32,
    requested: u32,
}

impl Backlight {
    fn changed(&self) -> Result<bool> {
        Ok((self.get().unwrap() as i32 - self.requested as i32) != 0)
    }
}

impl DeviceRead for Backlight {
    fn get(&self) -> Result<u32> {
        Ok(read_to_string(&self.path)?.trim().parse()?)
    }
    fn max(&self) -> u32 {
        self.max
    }
}

impl DeviceWrite for Backlight {
    fn set(&mut self, value: u32) -> Result<()> {
        write(&self.path, value.to_string())?;
        self.requested = value;
        Ok(())
    }
}

struct Ambient {
    path: PathBuf,
    max: u32,
}

impl DeviceRead for Ambient {
    fn get(&self) -> Result<u32> {
        Ok(read_to_string(&self.path)?.trim().parse()?)
    }
    fn max(&self) -> u32 {
        self.max
    }
}

impl DeviceWrite for Ambient {
    fn set(&mut self, value: u32) -> Result<()> {
        write(&self.path, value.to_string())?;
        Ok(())
    }
}
