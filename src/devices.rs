use anyhow::Result;

use std::{
    fs::{read_to_string, write},
    path::PathBuf,
};

use crate::{DeviceRead, DeviceWrite};

pub struct Backlight {
    pub path: PathBuf,
    pub max: u32,
    pub requested: u32,
}

impl Backlight {
    pub fn changed(&self) -> Result<bool> {
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

pub struct Ambient {
    pub path: PathBuf,
    pub max: u32,
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
