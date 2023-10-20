#![feature(async_fn_in_trait)]

use std::{
    collections::{BTreeMap, VecDeque},
    fs::{read_to_string, write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::{join, task::JoinHandle, time::sleep};

trait DeviceRead {
    fn get(&self) -> Result<u32>;
    fn max(&self) -> u32;
}

trait DeviceWrite {
    fn set(&mut self, value: u32) -> Result<()>;
}

trait Monotonic {
    fn add(&mut self, key: u32, value: u32, sensor_max: u32);
}

trait Interpolate {
    fn search_interpolate(&self, x: &u32) -> u32;
}

trait Smooth {
    async fn adjust(
        diff: i32,
        device: &mut (impl DeviceWrite + DeviceRead),
        fps: u8,
    ) -> Result<()> {
        let mut step = diff / fps as i32;
        if step == 0 {
            // for when 0 < step < 1
            step = if diff > 0 { 1 } else { -1 }
        }
        let value = TryInto::<i32>::try_into(device.get()?)? + step;
        if value < 0 {
            return Ok(());
        }
        device.set(TryInto::<u32>::try_into(value)?)?;

        Ok(())
    }
}

struct Backlight {
    path: PathBuf,
    max: u32,
    requested: u32,
}

impl Backlight {
    fn changed(&self) -> Result<bool> {
        Ok((self.get().unwrap() - self.requested) != 0)
    }
}

impl DeviceRead for Backlight {
    fn get(&self) -> Result<u32> {
        Ok(read_to_string(&self.path)?.parse()?)
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
        Ok(read_to_string(&self.path)?.parse()?)
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

struct Curve {
    points: BTreeMap<u32, u32>,
}

impl Smooth for Curve {}

impl Interpolate for Curve {
    fn search_interpolate(&self, x: &u32) -> u32 {
        if self.points.contains_key(x) {
            self.points[x]
        } else {
            let (x0, y0) = self.points.range(..x).next_back().unwrap();
            let (x1, y1) = self.points.range(x..).next().unwrap();

            (y0 * (x1 - x) + y1 * (x - x0)) / (x1 - x0)
        }
    }
}

impl Monotonic for Curve {
    fn add(&mut self, key: u32, value: u32, sensor_max: u32) {
        self.points.insert(key, value);
        self.points.iter_mut().for_each(|(k, v)| {
            if (*k != 0 && *k != sensor_max)
                && ((*k < key && *v > value) || (*k > key && *v < value))
            {
                *v = value
            }
        })
    }
}

#[derive(Deserialize, Serialize)]
struct Config {
    device_path: String,
    device_max_path: Option<String>,
    device_max: Option<u32>,

    sensor_path: String,
    sensor_max_path: Option<String>,
    sensor_max: Option<u32>,

    fps: Option<u8>,
    sample_frequency: Option<u8>,
    sample_size: Option<u8>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // TODO prompt user for setup (sensor_max)
    let mut config_path = dirs::config_dir().unwrap();
    config_path.push("/prescurve.toml");
    let config: Config = toml::from_str(read_to_string(config_path)?.as_ref())?;

    let mut backlight = Backlight {
        path: PathBuf::from(config.device_path),
        requested: 0,
        max: if let Some(max) = config.device_max {
            max
        } else {
            read_to_string(config.device_max_path.clone().unwrap())?.parse()?
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
    let mut curve = Curve { points };

    let average = Arc::new(AtomicU32::new(0));
    let average_arc = Arc::clone(&average);

    let ambient = Arc::clone(&ambient_arc);
    let average = Arc::clone(&average_arc);
    let sample: JoinHandle<Result<()>> = tokio::spawn(async move {
        let duration = Duration::from_millis(config.sample_frequency.unwrap_or(100).into());
        let mut samples: VecDeque<u32> =
            VecDeque::with_capacity(config.sample_size.unwrap_or(10).into());
        loop {
            let sensor = ambient.get()?;
            samples.pop_front();
            samples.push_back(sensor);
            average.store(samples.iter().sum(), Ordering::Release);
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
                        Curve::adjust(diff, &mut backlight, fps).await?;
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

    let _ = join![sample, adjust_retain];
    unreachable!()
}
