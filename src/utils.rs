use gtk::gdk;
use lazy_static::lazy_static;
use substring::Substring;

use std::{
	env,
	fs::{self, File},
	io::{prelude::*, BufReader},
	path::Path,
	sync::Mutex,
};

use blight::{change_bl, err::BlibError, Change, Device, Direction};
use pulse::volume::Volume;
use pulsectl::controllers::{types::DeviceInfo, DeviceControl, SinkController, SourceController};

use crate::application::ArgTypes;

lazy_static! {
	static ref MAX_VOLUME: Mutex<u8> = Mutex::new(100_u8);
	static ref DEVICE_NAME: Mutex<String> = Mutex::new("default".to_string());
	static ref TOP_MARGIN: Mutex<f32> = Mutex::new(0.85_f32);
}

pub enum KeysLocks {
	CapsLock,
	NumLock,
	ScrollLock,
}

pub fn volume_parser(is_sink: bool, value: &str) -> Result<(ArgTypes, Option<String>), i32> {
	let mut v = match (value, value.parse::<i8>()) {
		// Parse custom step values
		(_, Ok(num)) => (
			if num.is_positive() {
				ArgTypes::SinkVolumeRaise
			} else {
				ArgTypes::SinkVolumeLower
			},
			Some(num.abs().to_string()),
		),
		("raise", _) => (ArgTypes::SinkVolumeRaise, None),
		("lower", _) => (ArgTypes::SinkVolumeLower, None),
		("mute-toggle", _) => (ArgTypes::SinkVolumeMuteToggle, None),
		(e, _) => {
			eprintln!("Unknown output volume mode: \"{}\"!...", e);
			return Err(1);
		}
	};
	if is_sink {
		if v.0 == ArgTypes::SinkVolumeRaise {
			v.0 = ArgTypes::SourceVolumeRaise;
		} else if v.0 == ArgTypes::SinkVolumeLower {
			v.0 = ArgTypes::SourceVolumeLower;
		} else {
			v.0 = ArgTypes::SourceVolumeMuteToggle;
		}
	}
	Ok(v)
}

pub fn get_max_volume() -> u8 {
	*MAX_VOLUME.lock().unwrap()
}

pub fn set_max_volume(volume: Option<String>) {
	let setter: u8 = volume.unwrap().parse().unwrap();

	let mut vol = MAX_VOLUME.lock().unwrap();
	*vol = setter;
}

pub fn get_top_margin() -> f32 {
	*TOP_MARGIN.lock().unwrap()
}

pub fn set_top_margin(volume: Option<String>) {
	let setter: f32 = volume.unwrap().parse().unwrap();

	let mut vol = TOP_MARGIN.lock().unwrap();
	*vol = setter;
}

pub fn get_device_name() -> String {
	(*DEVICE_NAME.lock().unwrap()).clone()
}

pub fn set_device_name(name: String) {
	let mut global_name = DEVICE_NAME.lock().unwrap();
	*global_name = name;
}

pub fn get_key_lock_state(key: KeysLocks, led: Option<String>) -> bool {
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

			let key_name = match key {
				KeysLocks::CapsLock => "capslock",
				KeysLocks::NumLock => "numlock",
				KeysLocks::ScrollLock => "scrolllock",
			};

			for path in paths {
				if !path.contains(key_name) {
					continue;
				}
				if let Ok(content) = read_file(path + "/brightness") {
					if content.trim().eq("1") {
						return true;
					}
				}
			}
			false
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
	Sink(SinkController),
	Source(SourceController),
}

pub enum BrightnessChangeType {
	Raise,
	Lower,
}

pub fn change_device_volume(
	device_type: &mut VolumeDeviceType,
	change_type: VolumeChangeType,
	step: Option<String>,
) -> Option<DeviceInfo> {
	let (device, device_name): (DeviceInfo, String) = match device_type {
		VolumeDeviceType::Sink(controller) => {
			let server_info = controller.get_server_info();
			let global_name = get_device_name();
			let device_name: String = if global_name == "default" {
				match server_info {
					Ok(info) => info.default_sink_name.unwrap_or("".to_string()),
					Err(e) => {
						eprintln!("Error getting default_sink: {}", e);
						return None;
					}
				}
			} else {
				set_device_name("default".to_string());
				global_name
			};
			match controller.get_device_by_name(&device_name) {
				Ok(device) => (device, device_name.clone()),
				Err(_) => {
					eprintln!("No device with name: '{}' found!", device_name);
					return None;
				}
			}
		}
		VolumeDeviceType::Source(controller) => {
			let server_info = controller.get_server_info();
			let global_name = get_device_name();
			let device_name: String = if global_name == "default" {
				match server_info {
					Ok(info) => info.default_source_name.unwrap_or("".to_string()),
					Err(e) => {
						eprintln!("Error getting default_source: {}", e);
						return None;
					}
				}
			} else {
				set_device_name("default".to_string());
				global_name
			};
			match controller.get_device_by_name(&device_name) {
				Ok(device) => (device, device_name.clone()),
				Err(_) => {
					eprintln!("No device with name: '{}' found!", device_name);
					return None;
				}
			}
		}
	};

	const VOLUME_CHANGE_DELTA: u8 = 5;
	let volume_delta = step
		.clone()
		.unwrap_or(String::new())
		.parse::<u8>()
		.unwrap_or(VOLUME_CHANGE_DELTA) as f64
		* 0.01;
	match change_type {
		VolumeChangeType::Raise => {
			let max_volume = get_max_volume();
			// if we are already exactly at or over the max volume
			let mut at_max_volume = false;
			// if we are under the next volume but increasing by the given amount would be over the max
			let mut over_max_volume = false;

			let mut volume_percent = max_volume;
			// iterate through all devices in the volume group
			for v in device.volume.get() {
				// the string looks like this: ' NUMBER% '
				let volume_string = v.to_string();
				// trim it to remove the empty space 'NUMBER%'
				let mut volume_string = volume_string.trim();
				// remove the '%'
				volume_string = volume_string.substring(0, volume_string.len() - 1);

				// parse the string to a u8, we do it this convoluted to get the % and I haven't found another way
				volume_percent = volume_string.parse::<u8>().unwrap();

				if volume_percent >= max_volume {
					at_max_volume = true;
					break;
				}

				if volume_percent + VOLUME_CHANGE_DELTA > max_volume {
					over_max_volume = true;
					break;
				}
			}
			// if we are exactle at max volume
			if at_max_volume {
				// only show the OSD
				match device_type {
					VolumeDeviceType::Sink(controller) => {
						controller.increase_device_volume_by_percent(device.index, 0.0)
					}
					VolumeDeviceType::Source(controller) => {
						controller.increase_device_volume_by_percent(device.index, 0.0)
					}
				}
			}
			// if we would increase over the max step exactly to the max
			else if over_max_volume {
				let delta_to_max = max_volume - volume_percent;
				let volume_delta = step
					.unwrap_or(String::new())
					.parse::<u8>()
					.unwrap_or(delta_to_max) as f64
					* 0.01;
				match device_type {
					VolumeDeviceType::Sink(controller) => {
						controller.increase_device_volume_by_percent(device.index, volume_delta)
					}
					VolumeDeviceType::Source(controller) => {
						controller.increase_device_volume_by_percent(device.index, volume_delta)
					}
				}
			}
			// if neither of the above are true increase normally
			else {
				match device_type {
					VolumeDeviceType::Sink(controller) => {
						controller.increase_device_volume_by_percent(device.index, volume_delta)
					}
					VolumeDeviceType::Source(controller) => {
						controller.increase_device_volume_by_percent(device.index, volume_delta)
					}
				}
			}
		}
		VolumeChangeType::Lower => match device_type {
			VolumeDeviceType::Sink(controller) => {
				controller.decrease_device_volume_by_percent(device.index, volume_delta)
			}
			VolumeDeviceType::Source(controller) => {
				controller.decrease_device_volume_by_percent(device.index, volume_delta)
			}
		},
		VolumeChangeType::MuteToggle => match device_type {
			VolumeDeviceType::Sink(controller) => {
				let op = controller.handler.introspect.set_sink_mute_by_index(
					device.index,
					!device.mute,
					None,
				);
				controller.handler.wait_for_operation(op).ok();
			}
			VolumeDeviceType::Source(controller) => {
				let op = controller.handler.introspect.set_source_mute_by_index(
					device.index,
					!device.mute,
					None,
				);
				controller.handler.wait_for_operation(op).ok();
			}
		},
	}

	match device_type {
		VolumeDeviceType::Sink(controller) => match controller.get_device_by_name(&device_name) {
			Ok(device) => Some(device),
			Err(e) => {
				eprintln!("Pulse Error: {}", e);
				None
			}
		},
		VolumeDeviceType::Source(controller) => match controller.get_device_by_name(&device_name) {
			Ok(device) => Some(device),
			Err(e) => {
				eprintln!("Pulse Error: {}", e);
				None
			}
		},
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

pub fn user_style_path() -> Option<String> {
	match env::var("HOME") {
		Ok(home) => {
			let path = Path::new(&home).join(".config/swayosd/style.css");
			if path.exists() {
				return path.to_str().map(|s| s.to_string());
			}
			None
		}
		Err(_) => {
			println!("$HOME not set, not reading user style.css config");
			None
		}
	}
}
