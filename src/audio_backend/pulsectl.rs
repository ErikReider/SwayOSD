use pulse::volume::Volume;
use pulsectl::controllers::{
	types::DeviceInfo, DeviceControl, SinkController, SourceController,
};

use super::{AudioBackend, AudioBackendConstructor, AudioDeviceInfo, AudioDeviceType};

pub enum PulseController {
	Sink(SinkController),
	Source(SourceController),
}

pub struct PulseAudio {
	controller: PulseController,
	device_name: Option<String>,
}

impl AudioBackendConstructor for PulseAudio {
	fn try_new(device_type: AudioDeviceType, device_name: Option<String>) -> anyhow::Result<Self> {
		let controller = match device_type {
			AudioDeviceType::Sink => PulseController::Sink(SinkController::create()?),
			AudioDeviceType::Source => PulseController::Source(SourceController::create()?),
		};

		Ok(Self {
			controller,
			device_name,
		})
	}
}

impl PulseAudio {
	fn volume_to_f64(volume: &Volume) -> f64 {
		let tmp_vol = f64::from(volume.0 - Volume::MUTED.0);
		(100.0 * tmp_vol / f64::from(Volume::NORMAL.0 - Volume::MUTED.0)).round()
	}

	fn volume_from_f64(volume: f64) -> Volume {
		let tmp = f64::from(Volume::NORMAL.0 - Volume::MUTED.0) * volume / 100_f64;
		Volume((tmp + f64::from(Volume::MUTED.0)) as u32)
	}

	fn get_controller_and_device(
		&mut self,
	) -> anyhow::Result<(&mut dyn DeviceControl<DeviceInfo>, DeviceInfo)> {
		let (controller, default_name): (&mut dyn DeviceControl<DeviceInfo>, Option<String>) =
			match &mut self.controller {
				PulseController::Sink(ctrl) => {
					let server_info = ctrl.get_server_info()?;
					(ctrl, server_info.default_sink_name)
				}
				PulseController::Source(ctrl) => {
					let server_info = ctrl.get_server_info()?;
					(ctrl, server_info.default_source_name)
				}
			};

		// Get the device
		let device = if let Some(ref name) = self.device_name {
			controller.get_device_by_name(name)?
		} else {
			// Workaround the upstream issues in pulsectl-rs where getting the default source device
			// doesn't work...
			controller.get_device_by_name(&default_name.unwrap_or_default())?
		};

		Ok((controller, device))
	}
}

impl AudioBackend for PulseAudio {
	fn get_device_info(&mut self) -> anyhow::Result<AudioDeviceInfo> {
		let (_, device) = self.get_controller_and_device()?;

		Ok(AudioDeviceInfo {
			volume: Self::volume_to_f64(&device.volume.avg()),
			mute: device.mute,
		})
	}

	fn set_volume(&mut self, delta: f64, max_volume: u8) -> anyhow::Result<AudioDeviceInfo> {
		let (controller, device) = self.get_controller_and_device()?;

		let max_vol = Self::volume_from_f64(max_volume as f64);
		let delta_vol = Self::volume_from_f64(delta);

		if delta >= 0.0 {
			if let Some(volume) = device.volume.clone().inc_clamp(delta_vol, max_vol) {
				controller.set_device_volume_by_index(device.index, volume);
			}
		} else {
			// For negative delta, decrease volume
			if let Some(volume) = device.volume.clone().decrease(Self::volume_from_f64(-delta)) {
				controller.set_device_volume_by_index(device.index, volume);
			}
		}

		// Get updated device info
		let updated = controller.get_device_by_index(device.index)?;
		Ok(AudioDeviceInfo {
			volume: Self::volume_to_f64(&updated.volume.avg()),
			mute: updated.mute,
		})
	}

	fn toggle_mute(&mut self) -> anyhow::Result<AudioDeviceInfo> {
		let (controller, device) = self.get_controller_and_device()?;

		controller.set_device_mute_by_index(device.index, !device.mute);

		// Get updated device info
		let updated = controller.get_device_by_index(device.index)?;
		Ok(AudioDeviceInfo {
			volume: Self::volume_to_f64(&updated.volume.avg()),
			mute: updated.mute,
		})
	}
}
