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
    fn adjust(diff: i32, device: &mut (impl DeviceWrite + DeviceRead), fps: u8) -> Result<()> {
        let mut step = diff / fps as i32;
        if step == 0 {
            // for when 0 < step < 1 to reach target
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

struct Curve {
    points: BTreeMap<u32, u32>,
}

impl Smooth for Curve {}

impl Interpolate for Curve {
    fn search_interpolate(&self, x: &u32) -> u32 {
        // println!("{x}");
        // println!("{:?}", self.points);
        if self.points.contains_key(x) {
            self.points[x]
        } else {
            let (x0, y0) = self.points.range(..x).next_back().unwrap();
            // let (x0, y0) = match self.points.range(..x).next_back() {
            //     Some((x, y)) => (x, y),
            //     None => self.points.first_key_value().unwrap(),
            // };
            let (x1, y1) = self.points.range(x..).next().unwrap();
            // let (x1, y1) = match self.points.range(x..).next() {
            //     Some((x, y)) => (x, y),
            //     None => self.points.last_key_value().unwrap(),
            // };

            // println!("({x0}, {y0}), ({x1}, {y1})");
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
