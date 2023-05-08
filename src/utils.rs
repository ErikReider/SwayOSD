use gtk::gdk;
use lazy_static::lazy_static;
use substring::Substring;

use std::{
	fs::{self, File},
	io::{prelude::*, BufReader},
	sync::Mutex,
};

use blight::{change_bl, err::BlibError, Change, Device, Direction};
use pulse::volume::Volume;
use pulsectl::controllers::{types::DeviceInfo, DeviceControl, SinkController, SourceController};

lazy_static! {
	static ref MAX_VOLUME: Mutex<u8> = Mutex::new(100_u8);
}

pub fn get_max_volume() -> u8 {
	*MAX_VOLUME.lock().unwrap()
}

pub fn set_max_volume(volume: Option<String>) {
	let setter: u8 = volume.unwrap().parse().unwrap();

	let mut vol = MAX_VOLUME.lock().unwrap();
	*vol = setter;
}

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
			let device_name = &match server_info {
				Ok(info) => info.default_sink_name.unwrap_or("".to_string()),
				Err(e) => {
					eprintln!("Pulse Error: {}", e);
					return None;
				}
			};
			match controller.get_device_by_name(device_name) {
				Ok(device) => (device, device_name.clone()),
				Err(e) => {
					eprintln!("Pulse Error: {}", e);
					return None;
				}
			}
		}
		VolumeDeviceType::Source(controller) => {
			let server_info = controller.get_server_info();
			let device_name = &match server_info {
				Ok(info) => info.default_sink_name.unwrap_or("".to_string()),
				Err(e) => {
					eprintln!("Pulse Error: {}", e);
					return None;
				}
			};
			match controller.get_device_by_name(device_name) {
				Ok(device) => (device, device_name.clone()),
				Err(e) => {
					eprintln!("Pulse Error: {}", e);
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
				let op = controller.handler.introspect.set_sink_mute_by_index(
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
