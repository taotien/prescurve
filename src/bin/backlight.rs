use std::{
    collections::{BTreeMap, VecDeque},
    fs::{read_to_string, File},
    io::Write,
    path::PathBuf,
    process::exit,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use log::{debug, info, trace};
use tokio::{task::JoinHandle, time::sleep, try_join};

use prescurve::devices::{Ambient, Backlight};
use prescurve::Config;
use prescurve::{Curve, DeviceRead, Interpolate, Monotonic, Smooth};

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
    Ignore,
    // Save,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // TODO prompt user for setup (sensor_max)
    let mut config_path = dirs::config_dir().context("couldn't find config dir")?;
    config_path.push("prescurve.toml");
    let config: Config = toml::from_str(
        read_to_string(&config_path)
            .context("couldn't find config")?
            .as_ref(),
    )
    .context("couldn't parse config")?;
    info!("Loaded config.");
    debug!("{:?}", config);

    let mut backlight = Backlight {
        path: PathBuf::from(&config.device_path),
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
    info!("Init backlight.");
    debug!("{:?}", backlight);

    let ambient = Ambient {
        path: PathBuf::from(&config.sensor_path),
        // TODO read this from config and/or prompt user (setup wizard?)
        max: if let Some(max) = config.sensor_max {
            max
        } else {
            read_to_string(config.sensor_max_path.clone().unwrap())
                .context("couldn't read backlight value")?
                .trim()
                .parse()?
        },
    };
    let ambient_arc = Arc::new(ambient);
    info!("Init ambient light sensor.");
    debug!("{:?}", ambient_arc);

    let mut points = BTreeMap::new();
    let config_clone = config.clone();
    if let (Some(k), Some(v)) = (config_clone.curve_keys, config_clone.curve_values) {
        for (k, v) in k.iter().zip(v.iter()) {
            points.insert(*k, *v);
        }
    } else {
        points.insert(0, 1);
        points.insert(ambient_arc.max, backlight.max);
    }
    let mut curve = Curve {
        points,
        cache: Box::new([0]),
    };
    let cache: Vec<u32> = (0..=ambient_arc.max)
        .map(|x| curve.search_interpolate(&x))
        .collect();
    curve.cache = cache.into_boxed_slice();

    let average = Arc::new(AtomicU32::new(0));
    let average_arc = Arc::clone(&average);

    let ambient = Arc::clone(&ambient_arc);
    let average = Arc::clone(&average_arc);
    let sample: JoinHandle<Result<()>> = tokio::spawn(async move {
        trace!("enter sample block");
        let duration = Duration::from_millis(config.sample_frequency.unwrap_or(1000).into());
        let mut samples: VecDeque<u32> =
            VecDeque::with_capacity(config.sample_size.unwrap_or(10).into());
        for _ in 0..10 {
            samples.push_back(ambient.get()?);
        }
        loop {
            trace!("sample loop");
            let sensor = ambient.get()?;
            samples.pop_front();
            samples.push_back(sensor);
            average.store(
                samples.iter().sum::<u32>() / samples.len() as u32,
                Ordering::Relaxed,
            );
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
            trace!("adj retain");
            match backlight.changed()? {
                false => {
                    trace!("bl unchanged");
                    let sensor = average.load(Ordering::Relaxed);
                    trace!("load sensor");
                    let target = curve.cache[sensor as usize];
                    if backlight.get()? != target {
                        let diff = target as i32 - backlight.get()? as i32;
                        Curve::adjust(diff, &mut backlight, fps)?;
                        sleep(Duration::from_millis(1000 / fps as u64)).await;
                    } else {
                        sleep(Duration::from_millis(freq as u64 * sample_size as u64)).await;
                    }
                }
                // backlight was manually/externally adjusted
                true => {
                    trace!("bl changed");
                    // TODO read this from config
                    sleep(Duration::from_secs(config.manual_adjust_wait.unwrap_or(10))).await;
                    let current = backlight.get()?;

                    // user requested black screen temporarily, don't save
                    if current == 0 {
                        continue;
                    }

                    backlight.requested = current;
                    curve.add(average.load(Ordering::Acquire), current, ambient.max);

                    let cache: Vec<u32> = (0..=ambient_arc.max)
                        .map(|x| curve.search_interpolate(&x))
                        .collect();
                    curve.cache = cache.into_boxed_slice();

                    let mut config = config.clone();
                    let curve = curve.points.clone();
                    let keys = curve.keys().copied().collect();
                    let values = curve.values().copied().collect();

                    config.curve_keys = Some(keys);
                    config.curve_values = Some(values);
                    let toml = toml::to_string(&config)?;
                    let mut f = File::create(&config_path)?;
                    f.write_all(toml.as_bytes())?;
                }
            }
        }
    });

    let res = try_join![sample, adjust_retain];
    // FIXME this doesn't seem to actually handle panics in loop?
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
