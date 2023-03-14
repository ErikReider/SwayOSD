use gtk::gdk;

use std::{
	fs::{self, File},
	io::{prelude::*, BufReader},
};

use blight::{change_bl, err::BlibError, Change, Device, Direction};
use pulse::volume::Volume;
use pulsectl::controllers::{types::DeviceInfo, DeviceControl, SinkController, SourceController};

pub fn get_caps_lock_state(led: Option<String>) -> bool {
	const BASE_PATH: &str = "/sys/class/leds";
	match fs::read_dir(BASE_PATH) {
		Ok(paths) => {
			let mut paths: Vec<String> = paths
				.map_while(|path| {
					path.map_or_else(|_| None, |p| Some(p.path().display().to_string()))
				})
				.collect();

			if let Some(led) = led {
				let led = format!("{}/{}", BASE_PATH, led);
				if paths.contains(&led) {
					paths.insert(0, led);
				} else {
					eprintln!("LED device {led} does not exist!... Trying other LEDs");
				}
			}

			for path in paths {
				if !path.contains("capslock") {
					continue;
				}
				if let Ok(content) = read_file(path + "/brightness") {
					if content.trim().eq("1") {
						return true;
					}
				}
			}
			return false;
		}
		Err(_) => {
			eprintln!("No LEDS found!...");
			false
		}
	}
}

fn read_file(path: String) -> std::io::Result<String> {
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

pub enum BrightnessChangeType {
	Raise,
	Lower,
}

pub fn change_sink_volume(
	change_type: VolumeChangeType,
	step: Option<String>,
) -> Option<DeviceInfo> {
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

	const VOLUME_CHANGE_DELTA: u8 = 5;
	let volume_delta = step
		.unwrap_or(String::new())
		.parse::<u8>()
		.unwrap_or(VOLUME_CHANGE_DELTA) as f64
		* 0.01;
	match change_type {
		VolumeChangeType::Raise => {
			controller.increase_device_volume_by_percent(device.index, volume_delta)
		}
		VolumeChangeType::Lower => {
			controller.decrease_device_volume_by_percent(device.index, volume_delta)
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

pub fn change_source_volume(
	change_type: VolumeChangeType,
	step: Option<String>,
) -> Option<DeviceInfo> {
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

	const VOLUME_CHANGE_DELTA: u8 = 5;
	let volume_delta = step
		.unwrap_or(String::new())
		.parse::<u8>()
		.unwrap_or(VOLUME_CHANGE_DELTA) as f64
		* 0.01;
	match change_type {
		VolumeChangeType::Raise => {
			controller.increase_device_volume_by_percent(device.index, volume_delta)
		}
		VolumeChangeType::Lower => {
			controller.decrease_device_volume_by_percent(device.index, volume_delta)
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

pub fn change_brightness(
	change_type: BrightnessChangeType,
	step: Option<String>,
) -> Result<Option<Device>, BlibError> {
	const BRIGHTNESS_CHANGE_DELTA: u8 = 5;
	let brightness_delta: u16 = step
		.unwrap_or(String::new())
		.parse::<u8>()
		.unwrap_or(BRIGHTNESS_CHANGE_DELTA) as u16;
	let direction = match change_type {
		BrightnessChangeType::Raise => Direction::Inc,
		BrightnessChangeType::Lower => {
			let device = Device::new(None)?;
			let change = device.calculate_change(brightness_delta, Direction::Dec) as f64;
			let max = device.max() as f64;
			// Limits the lowest brightness to 5%
			if change / max < (brightness_delta as f64) * 0.01 {
				return Ok(Some(device));
			}
			Direction::Dec
		}
	};
	match change_bl(brightness_delta, Change::Regular, direction, None) {
		Err(e) => {
			eprintln!("Brightness Error: {}", e);
			Err(e)
		}
		_ => Ok(Some(Device::new(None)?)),
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
