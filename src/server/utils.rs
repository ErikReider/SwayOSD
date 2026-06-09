use gtk::glib::{system_config_dirs, user_config_dir};
use pulse::volume::Volume;
use std::{
	fmt::Debug,
	fs::{self, File},
	io::{prelude::*, BufReader},
	path::{Path, PathBuf},
};

use crate::actions::{
	brightness_backend::{self, BrightnessBackendResult},
	pulse::{DeviceInfo, DeviceKind, VolumeController},
};

#[derive(Clone, Debug)]
pub struct ActionField<T: Clone + Debug> {
	value: Option<T>,
	default: T,
}

#[allow(unused)]
impl<T: Clone + Debug> ActionField<T> {
	pub fn new(default: T) -> Self {
		Self {
			value: None,
			default,
		}
	}

	pub fn get(&self) -> &T {
		match self.value {
			Some(ref value) => value,
			None => &self.default,
		}
	}

	pub fn set(&mut self, value: Option<T>) {
		self.value = value;
	}

	pub fn reset(&mut self) {
		self.set(None)
	}

	pub fn set_default(&mut self, value: T) {
		self.default = value;
	}
	pub fn get_default(&self) -> T {
		self.default.clone()
	}
}

#[derive(Clone, Debug)]
pub struct ActionOptionalField<T: Clone + Debug> {
	value: Option<T>,
	default: Option<T>,
}

#[allow(unused)]
impl<T: Clone + Debug> ActionOptionalField<T> {
	pub fn new(default: Option<T>) -> Self {
		Self {
			value: None,
			default,
		}
	}

	pub fn get(&self) -> &Option<T> {
		match self.value {
			Some(_) => &self.value,
			None => &self.default,
		}
	}

	pub fn set(&mut self, value: Option<T>) {
		self.value = value;
	}

	pub fn reset(&mut self) {
		self.set(None)
	}

	pub fn set_default(&mut self, value: Option<T>) {
		self.default = value;
	}
	pub fn get_default(&self) -> Option<T> {
		self.default.clone()
	}
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy)]
pub enum KeysLocks {
	CapsLock,
	NumLock,
	ScrollLock,
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
				if let Ok(content) = read_file(path + "/brightness")
					&& content.trim() == "1"
				{
					return true;
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

pub enum BrightnessChangeType {
	Raise,
	Lower,
	Set,
}

pub fn volume_to_f64(volume: &Volume) -> f64 {
	let tmp_vol = f64::from(volume.0 - Volume::MUTED.0);
	(100.0 * tmp_vol / f64::from(Volume::NORMAL.0 - Volume::MUTED.0)).round()
}

fn volume_from_f64(volume: f64) -> Volume {
	let tmp = f64::from(Volume::NORMAL.0 - Volume::MUTED.0) * volume / 100_f64;
	Volume((tmp + f64::from(Volume::MUTED.0)) as u32)
}

pub fn change_device_volume(
	ctrl: &mut VolumeController,
	kind: DeviceKind,
	change_type: VolumeChangeType,
	device_name: &Option<String>,
	max_volume: f64,
	step: Option<String>,
) -> Option<DeviceInfo> {
	let device = match device_name {
		Some(name) => ctrl.get_device_by_name(kind, name),
		None => ctrl.get_default_device(kind),
	};
	let device = match device {
		Ok(d) => d,
		Err(e) => {
			eprintln!("Error getting device: {}", e);
			return None;
		}
	};

	const VOLUME_CHANGE_DELTA: f64 = 5_f64;
	let delta = volume_from_f64(
		step.unwrap_or_default()
			.parse::<f64>()
			.unwrap_or(VOLUME_CHANGE_DELTA),
	);
	match change_type {
		VolumeChangeType::Raise => {
			let max_volume = volume_from_f64(max_volume);
			if let Some(volume) = device.volume.clone().inc_clamp(delta, max_volume) {
				ctrl.set_volume_by_index(kind, device.index, volume);
			}
		}
		VolumeChangeType::Lower => {
			if let Some(volume) = device.volume.clone().decrease(delta) {
				ctrl.set_volume_by_index(kind, device.index, volume);
			}
		}
		VolumeChangeType::MuteToggle => {
			ctrl.set_mute_by_index(kind, device.index, !device.mute);
		}
	}

	match ctrl.get_device_by_index(kind, device.index) {
		Ok(d) => Some(d),
		Err(e) => {
			eprintln!("Pulse Error: {}", e);
			None
		}
	}
}

pub fn change_brightness(
	change_type: BrightnessChangeType,
	device_name: &Option<String>,
	min_brightness: u32,
	step: Option<String>,
) -> BrightnessBackendResult {
	const BRIGHTNESS_CHANGE_DELTA: u8 = 5;
	let value = step.unwrap_or_default().parse::<u8>();

	let mut backend = brightness_backend::get_preferred_backend(device_name.clone())?;

	match change_type {
		BrightnessChangeType::Raise => backend.raise(
			value.unwrap_or(BRIGHTNESS_CHANGE_DELTA) as u32,
			min_brightness,
		)?,
		BrightnessChangeType::Lower => backend.lower(
			value.unwrap_or(BRIGHTNESS_CHANGE_DELTA) as u32,
			min_brightness,
		)?,
		BrightnessChangeType::Set => backend.set(value? as u32, min_brightness)?,
	};

	Ok(backend)
}

pub fn get_system_css_path() -> Option<PathBuf> {
	let mut paths: Vec<PathBuf> = Vec::new();
	for path in system_config_dirs() {
		paths.push(path.join("swayosd").join("style.css"));
	}
	// Fallback for Debian/Ubuntu-based distros
	paths.push(Path::new("/usr/local/etc/xdg/swaync/style.css").to_path_buf());

	for try_path in paths {
		if try_path.exists() {
			return Some(try_path.clone());
		}
	}
	None
}

pub fn user_style_path(custom_path: Option<PathBuf>) -> Option<String> {
	let path = user_config_dir().join("swayosd").join("style.css");
	if let Some(custom_path) = custom_path
		&& custom_path.exists()
	{
		return custom_path.to_str().map(|s| s.to_string());
	}
	if path.exists() {
		return path.to_str().map(|s| s.to_string());
	}
	None
}
