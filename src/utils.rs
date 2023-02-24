use gtk::gdk;

use std::{
	fs::{self, File},
	io::{prelude::*, BufReader, Result},
};

use pulse::volume::Volume;
use pulsectl::controllers::{types::DeviceInfo, DeviceControl, SinkController, SourceController};

pub enum LightDevice {
	LED,
	BACKLIGHT,
}

impl LightDevice {
	pub fn get_path(&self) -> String {
		format!(
			"/sys/class/{}",
			match self {
				LightDevice::LED => "leds",
				LightDevice::BACKLIGHT => "backlight",
			}
		)
	}
}

#[derive(Debug)]
pub struct LightProps {
	brightness: i32,
	max_brightness: i32,
}

impl LightProps {
	pub fn get_binary_device_state(&self) -> Option<bool> {
		if self.max_brightness > 1 {
			return None;
		}
		return Some(self.brightness == 1);
	}
}

pub fn get_light_state(
	device_type: LightDevice,
	light: Option<String>,
	find: &str,
) -> Option<LightProps> {
	let base_path: &str = &device_type.get_path();
	match fs::read_dir(base_path) {
		Ok(paths) => {
			let mut paths: Vec<String> = paths
				.map_while(|path| {
					path.map_or_else(|_| None, |p| Some(p.path().display().to_string()))
				})
				.collect();

			if let Some(light) = light {
				let light = format!("{}/{}", base_path, light);
				if paths.ends_with(&[light.clone()]) {
					paths.insert(0, light);
				} else {
					eprintln!("LED device {light} does not exist!... Trying other LEDs");
				}
			}

			for path in paths {
				if !path.contains(find) {
					continue;
				}

				match (
					read_file(format!("{}/brightness", &path)).map(|x| x.trim().parse::<i32>()),
					read_file(format!("{}/max_brightness", &path)).map(|x| x.trim().parse::<i32>()),
				) {
					(Ok(Ok(a)), Ok(Ok(b))) => {
						return Some(LightProps {
							brightness: a,
							max_brightness: b,
						});
					}
					_ => continue,
				};
			}
			return None;
		}
		Err(_) => {
			eprintln!("No LEDS found!...");
			None
		}
	}
}

fn read_file(path: String) -> Result<String> {
	let file = File::open(path)?;
	let mut buf_reader = BufReader::new(file);
	let mut contents = String::new();
	buf_reader.read_to_string(&mut contents)?;
	Ok(contents)
}

pub enum VolumeChangeType {
	Raise,
	Lower,
	MuteToggle,
}

pub enum VolumeDeviceType {
	Sink,
	Source,
}

pub fn change_sink_volume(change_type: VolumeChangeType) -> Option<DeviceInfo> {
	let mut controller = SinkController::create().unwrap();

	let server_info = controller.get_server_info();
	let device_name = &match server_info {
		Ok(info) => info.default_sink_name.unwrap_or("".to_string()),
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			return None;
		}
	};
	let device = match controller.get_device_by_name(device_name) {
		Ok(device) => device,
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			return None;
		}
	};

	const VOLUME_CHANGE_DELTA: f64 = 0.05;
	match change_type {
		VolumeChangeType::Raise => {
			controller.increase_device_volume_by_percent(device.index, VOLUME_CHANGE_DELTA)
		}
		VolumeChangeType::Lower => {
			controller.decrease_device_volume_by_percent(device.index, VOLUME_CHANGE_DELTA)
		}
		VolumeChangeType::MuteToggle => {
			let op = controller.handler.introspect.set_sink_mute_by_index(
				device.index,
				!device.mute,
				None,
			);
			controller.handler.wait_for_operation(op).ok();
		}
	}

	match controller.get_device_by_name(device_name) {
		Ok(device) => Some(device),
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			None
		}
	}
}

pub fn change_source_volume(change_type: VolumeChangeType) -> Option<DeviceInfo> {
	let mut controller = SourceController::create().unwrap();

	let server_info = controller.get_server_info();
	let device_name = &match server_info {
		Ok(info) => info.default_source_name.unwrap_or("".to_string()),
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			return None;
		}
	};
	let device = match controller.get_device_by_name(device_name) {
		Ok(device) => device,
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			return None;
		}
	};

	const VOLUME_CHANGE_DELTA: f64 = 0.05;
	match change_type {
		VolumeChangeType::Raise => {
			controller.increase_device_volume_by_percent(device.index, VOLUME_CHANGE_DELTA)
		}
		VolumeChangeType::Lower => {
			controller.decrease_device_volume_by_percent(device.index, VOLUME_CHANGE_DELTA)
		}
		VolumeChangeType::MuteToggle => {
			let op = controller.handler.introspect.set_source_mute_by_index(
				device.index,
				!device.mute,
				None,
			);
			controller.handler.wait_for_operation(op).ok();
		}
	}

	match controller.get_device_by_name(device_name) {
		Ok(device) => Some(device),
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			None
		}
	}
}

pub fn volume_to_f64(volume: &Volume) -> f64 {
	let tmp_vol = f64::from(volume.0 - Volume::MUTED.0);
	(100.0 * tmp_vol / f64::from(Volume::NORMAL.0 - Volume::MUTED.0)).round()
}

pub fn is_dark_mode(fg: &gdk::RGBA, bg: &gdk::RGBA) -> bool {
	let text_avg = fg.red() / 256.0 + fg.green() / 256.0 + fg.blue() / 256.0;
	let bg_avg = bg.red() / 256.0 + bg.green() / 256.0 + bg.blue() / 256.0;
	text_avg > bg_avg
}
