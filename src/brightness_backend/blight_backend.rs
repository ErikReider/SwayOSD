use blight::{Device, Direction};

use super::{BrightnessBackend, BrightnessBackendConstructor, BrightnessOperationResult};

pub struct Blight {
    device: Device,
}

impl BrightnessBackendConstructor for Blight {
    fn try_new(device_name: Option<String>) -> BrightnessOperationResult<Self> {
        Ok(Self {
            device: Device::new(device_name.map(Into::into))?,
        })
    }
}

impl BrightnessBackend for Blight {
    fn get_current(&self) -> u32 {
        self.device.current()
    }

    fn get_max(&self) -> u32 {
        self.device.max()
    }

    fn lower(&self, by: u32) -> BrightnessOperationResult<()> {
        let val = self.device.calculate_change(by, Direction::Dec);
        Ok(self.device.write_value(val)?)
    }

    fn raise(&self, by: u32) -> BrightnessOperationResult<()> {
        let val = self.device.calculate_change(by, Direction::Inc);
        Ok(self.device.write_value(val)?)
    }

    fn set(&self, val: u32) -> BrightnessOperationResult<()> {
        Ok(self.device.write_value(val)?)
    }
}
