//! Traits for creating smooth hardware curves.
//!
//! API is not stable! Currently only used for prescurve-backlight. Hopefully in
//! the future these can be generic over any type that a sensor and device exposes.

use std::collections::BTreeMap;

use anyhow::Result;

pub mod devices;

pub trait DeviceRead {
    fn get(&self) -> Result<u32>;
    fn max(&self) -> u32;
}

pub trait DeviceWrite {
    fn set(&mut self, value: u32) -> Result<()>;
}

/// Make sure that the curve does not have dips or peaks.
pub trait Monotonic {
    fn add(&mut self, key: u32, value: u32, sensor_max: u32);
}

pub trait Interpolate {
    fn search_interpolate(&self, x: &u32) -> u32;
}

pub trait Smooth {
    fn adjust(diff: i32, device: &mut (impl DeviceWrite + DeviceRead), fps: u8) -> Result<()> {
        let mut step = diff / fps as i32;
        if step == 0 {
            // for when 0 < step < 1 to reach target
            step = if diff > 0 { 1 } else { -1 }
        }
        let value = device.get()?.saturating_add_signed(step);
        device.set(TryInto::<u32>::try_into(value)?)?;

        Ok(())
    }
}

pub struct Curve {
    pub points: BTreeMap<u32, u32>,
    pub cache: Box<[u32]>,
}

impl Smooth for Curve {}

enum Algorithm {
    Linear,
    Logarithmic,
}

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
