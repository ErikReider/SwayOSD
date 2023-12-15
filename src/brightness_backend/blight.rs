use blight::{Device, Direction};

use super::{BrightnessBackend, BrightnessBackendConstructor};

pub(super) struct Blight {
	device: Device,
}

impl BrightnessBackendConstructor for Blight {
	fn try_new(device_name: Option<String>) -> anyhow::Result<Self> {
		Ok(Self {
			device: Device::new(device_name.map(Into::into))?,
		})
	}
}

impl BrightnessBackend for Blight {
	fn get_current(&mut self) -> u32 {
		self.device.reload();
		self.device.current()
	}

	fn get_max(&mut self) -> u32 {
		self.device.max()
	}

	fn lower(&mut self, by: u32) -> anyhow::Result<()> {
		let val = self.device.calculate_change(by, Direction::Dec);
		Ok(self.device.write_value(val)?)
	}

	fn raise(&mut self, by: u32) -> anyhow::Result<()> {
		let val = self.device.calculate_change(by, Direction::Inc);
		Ok(self.device.write_value(val)?)
	}

	fn set(&mut self, val: u32) -> anyhow::Result<()> {
		Ok(self.device.write_value(val)?)
	}
}
