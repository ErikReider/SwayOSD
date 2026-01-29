use crate::global_utils::div_round_u32;

use super::{BrightnessBackend, BrightnessBackendConstructor};

use anyhow::bail;
use ddc_hi::{Ddc, Display};
use std::{cell::RefCell, rc::Rc};
use thiserror::Error;

/// VCP feature code to get and set the brightness of the monitor via DDC/CI
const VCP_BRIGHTNESS_FEATURE: u8 = 0x10;

#[derive(Error, Debug)]
#[error("Requested device '{device_name}' does not exist ")]
pub struct DeviceDoesntExistError {
	device_name: String,
}

struct DdcDevice {
	display: Rc<RefCell<Display>>,
	current: u32,
	max: u32,
}

#[allow(unused)]
impl DdcDevice {
	fn try_new(device_name: Option<String>) -> anyhow::Result<Self> {
		let mut displays = Display::enumerate();

		// Try to find the exact display if it was specified
		if let Some(ref name) = device_name {
			if let Some(n) = displays
				.iter()
				.position(|d| d.info.model_name.as_deref() == Some(name))
			{
				// Test if the display supports the Brightness feature
				let mut display = displays.swap_remove(n);
				if let Ok(vcp_response) = display.handle.get_vcp_feature(VCP_BRIGHTNESS_FEATURE) {
					Ok(Self {
						display: Rc::new(RefCell::new(display)),
						current: vcp_response.value() as u32,
						max: vcp_response.maximum() as u32,
					})
				} else {
					// The device was found, but doesn't support brightness control via DDC/CI
					// NOTE: perhaps a fallback to the first useable display is better instead?
					bail!(DeviceDoesntExistError {
						device_name: device_name.unwrap_or("Device name unknown".to_string())
					})
				}
			} else {
				// The device couldn't be found
				// NOTE: perhaps a fallback to the first useable display is better instead?
				bail!(DeviceDoesntExistError {
					device_name: device_name.unwrap_or("Device name unknown".to_string())
				})
			}
		} else {
			// Search for the first display responsive to the Brightness feature
			for i in 0..displays.len() {
				if let Ok(vcp_response) = displays
					.get_mut(i)
					.unwrap() // Careful: `i` is always valid here
					.handle
					.get_vcp_feature(VCP_BRIGHTNESS_FEATURE)
				{
					let display = displays.swap_remove(i);
					return Ok(Self {
						display: Rc::new(RefCell::new(display)),
						current: vcp_response.value() as u32,
						max: vcp_response.maximum() as u32,
					});
				}
			}

			// There are no displays that can be used, at all
			bail!(DeviceDoesntExistError {
				device_name: "N/A".to_string()
			})
		}
	}

	fn get_current(&mut self) -> u32 {
		self.current
	}

	fn get_max(&mut self) -> u32 {
		self.max
	}

	fn get_percent(&mut self) -> u32 {
		let cur = self.get_current();
		let max = self.get_max();
		div_round_u32(cur * 100, max)
	}

	fn set_raw(&mut self, val: u32) -> anyhow::Result<()> {
		let max = self.get_max();
		let clamped_val = val.clamp(0, max);

		// Try to update the Brightness
		self.display
			.borrow_mut()
			.handle
			.set_vcp_feature(VCP_BRIGHTNESS_FEATURE, clamped_val as u16)
			.expect("DdcDevice failed to set brightness");

		self.current = clamped_val;
		Ok(())
	}

	fn set_percent(&mut self, val: u32) -> anyhow::Result<()> {
		// The monitor should accept everything in percentages
		// but if it doesn't, scale the percentage to the expected value
		let clamped_val = val.clamp(0, 100);
		let max = self.get_max();
		let raw_val = div_round_u32(clamped_val * max, 100);

		self.set_raw(raw_val)
	}
}

#[allow(unused)]
pub(super) struct Ddcci {
	device: DdcDevice,
}

impl BrightnessBackendConstructor for Ddcci {
	fn try_new(device_name: Option<String>) -> anyhow::Result<Self> {
		Ok(Self {
			device: DdcDevice::try_new(device_name)?,
		})
	}
}

impl BrightnessBackend for Ddcci {
	fn get_current(&mut self) -> u32 {
		self.device.get_current()
	}

	fn get_max(&mut self) -> u32 {
		self.device.get_max()
	}

	fn lower(&mut self, by: u32, min: u32) -> anyhow::Result<()> {
		let max = self.device.get_max();
		let cur = self.device.get_current();
		let step = div_round_u32(by * max, 100);
		let new_val = cur.saturating_sub(step);
		let min_raw = div_round_u32(min * max, 100);
		self.device.set_raw(new_val.max(min_raw))
	}

	fn raise(&mut self, by: u32, min: u32) -> anyhow::Result<()> {
		let max = self.device.get_max();
		let curr = self.device.get_current();
		let step = div_round_u32(by * max, 100);
		let new_val = (curr + step).min(max);
		let min_raw = div_round_u32(min * max, 100);
		self.device.set_raw(new_val.max(min_raw))
	}

	fn set(&mut self, val: u32, min: u32) -> anyhow::Result<()> {
		let max = self.device.get_max();
		let raw_val = div_round_u32(val.max(min) * max, 100);
		self.device.set_raw(raw_val)
	}
}
