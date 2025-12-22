use gtk::glib::{system_config_dirs, user_config_dir};
use lazy_static::lazy_static;

use std::{
	fs::{self, File},
	io::{prelude::*, BufReader},
	path::{Path, PathBuf},
	sync::Mutex,
};

use pulse::volume::Volume;
use pulsectl::controllers::{types::DeviceInfo, DeviceControl, SinkController, SourceController};

use crate::brightness_backend;
use crate::playerctl::PlayerctlDeviceRaw;

static PRIV_MAX_VOLUME_DEFAULT: u8 = 100_u8;
static PRIV_MIN_BRIGHTNESS_DEFAULT: u32 = 5_u32;

lazy_static! {
	static ref MAX_VOLUME_DEFAULT: Mutex<u8> = Mutex::new(PRIV_MAX_VOLUME_DEFAULT);
	static ref MAX_VOLUME: Mutex<u8> = Mutex::new(PRIV_MAX_VOLUME_DEFAULT);
	static ref MIN_BRIGHTNESS_DEFAULT: Mutex<u32> = Mutex::new(PRIV_MIN_BRIGHTNESS_DEFAULT);
	static ref MIN_BRIGHTNESS: Mutex<u32> = Mutex::new(PRIV_MIN_BRIGHTNESS_DEFAULT);
	pub static ref DEVICE_NAME_DEFAULT: &'static str = "default";
	static ref DEVICE_NAME: Mutex<Option<String>> = Mutex::new(None);
	static ref MONITOR_NAME: Mutex<Option<String>> = Mutex::new(None);
	pub static ref ICON_NAME_DEFAULT: &'static str = "text-x-generic";
	static ref ICON_NAME: Mutex<Option<String>> = Mutex::new(None);
	static ref PROGRESS_TEXT: Mutex<Option<String>> = Mutex::new(None);
	static ref PLAYER_NAME: Mutex<PlayerctlDeviceRaw> = Mutex::new(PlayerctlDeviceRaw::None);
	pub static ref TOP_MARGIN_DEFAULT: f32 = 0.85_f32;
	static ref TOP_MARGIN: Mutex<f32> = Mutex::new(*TOP_MARGIN_DEFAULT);
	pub static ref SHOW_PERCENTAGE: Mutex<bool> = Mutex::new(false);
}

#[allow(clippy::enum_variant_names)]
pub enum KeysLocks {
	CapsLock,
	NumLock,
	ScrollLock,
}

pub fn get_default_max_volume() -> u8 {
	*MAX_VOLUME_DEFAULT.lock().unwrap()
}

pub fn set_default_max_volume(volume: u8) {
	let mut vol = MAX_VOLUME_DEFAULT.lock().unwrap();
	*vol = volume;
}

pub fn get_max_volume() -> u8 {
	*MAX_VOLUME.lock().unwrap()
}

pub fn set_max_volume(volume: u8) {
	let mut vol = MAX_VOLUME.lock().unwrap();
	*vol = volume;
}

pub fn reset_max_volume() {
	let mut vol = MAX_VOLUME.lock().unwrap();
	*vol = *MAX_VOLUME_DEFAULT.lock().unwrap();
}

pub fn get_default_min_brightness() -> u32 {
	*MIN_BRIGHTNESS_DEFAULT.lock().unwrap()
}

pub fn set_default_min_brightness(brightness: u32) {
	let mut min = MIN_BRIGHTNESS_DEFAULT.lock().unwrap();
	*min = brightness;
}

pub fn get_min_brightness() -> u32 {
	*MIN_BRIGHTNESS.lock().unwrap()
}

pub fn set_min_brightness(brightness: u32) {
	let mut min = MIN_BRIGHTNESS.lock().unwrap();
	*min = brightness;
}

pub fn reset_min_brightness() {
	let mut min = MIN_BRIGHTNESS.lock().unwrap();
	*min = *MIN_BRIGHTNESS_DEFAULT.lock().unwrap();
}

pub fn get_top_margin() -> f32 {
	*TOP_MARGIN.lock().unwrap()
}

pub fn set_top_margin(margin: f32) {
	let mut margin_mut = TOP_MARGIN.lock().unwrap();
	*margin_mut = margin;
}

pub fn get_show_percentage() -> bool {
	*SHOW_PERCENTAGE.lock().unwrap()
}

pub fn set_show_percentage(show: bool) {
	let mut show_mut = SHOW_PERCENTAGE.lock().unwrap();
	*show_mut = show;
}

pub fn get_device_name() -> Option<String> {
	(*DEVICE_NAME.lock().unwrap()).clone()
}

pub fn set_device_name(name: String) {
	let mut global_name = DEVICE_NAME.lock().unwrap();
	*global_name = Some(name);
}

pub fn reset_device_name() {
	let mut global_name = DEVICE_NAME.lock().unwrap();
	*global_name = None;
}

pub fn get_monitor_name() -> Option<String> {
	(*MONITOR_NAME.lock().unwrap()).clone()
}

pub fn set_monitor_name(name: String) {
	let mut monitor_name = MONITOR_NAME.lock().unwrap();
	*monitor_name = Some(name);
}

pub fn reset_monitor_name() {
	let mut monitor_name = MONITOR_NAME.lock().unwrap();
	*monitor_name = None;
}

pub fn get_progress_text() -> Option<String> {
	(*PROGRESS_TEXT.lock().unwrap()).clone()
}

pub fn set_progress_text(name: Option<String>) {
	let mut progress_text = PROGRESS_TEXT.lock().unwrap();
	*progress_text = name;
}

pub fn reset_progress_text() {
	let mut progress_text = PROGRESS_TEXT.lock().unwrap();
	*progress_text = None;
}

pub fn get_icon_name() -> Option<String> {
	(*ICON_NAME.lock().unwrap()).clone()
}

pub fn set_icon_name(name: String) {
	let mut icon_name = ICON_NAME.lock().unwrap();
	*icon_name = Some(name);
}

pub fn reset_icon_name() {
	let mut icon_name = ICON_NAME.lock().unwrap();
	*icon_name = None;
}

pub fn set_player(name: String) {
	let mut global_player = PLAYER_NAME.lock().unwrap();
	*global_player = PlayerctlDeviceRaw::from(name).unwrap_or(PlayerctlDeviceRaw::None);
}

pub fn reset_player() {
	let mut global_name = PLAYER_NAME.lock().unwrap();
	*global_name = PlayerctlDeviceRaw::None;
}

pub fn get_player() -> PlayerctlDeviceRaw {
	let player = PLAYER_NAME.lock().unwrap();
	player.clone()
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
	device_type: &mut VolumeDeviceType,
	change_type: VolumeChangeType,
	step: Option<String>,
) -> Option<DeviceInfo> {
	// Get the sink/source controller
	let controller: &mut dyn DeviceControl<DeviceInfo> = match device_type {
		VolumeDeviceType::Sink(controller) => controller,
		VolumeDeviceType::Source(controller) => controller,
	};

	// Get the device
	let device: DeviceInfo = if let Some(name) = get_device_name()
		&& let Ok(device) = controller.get_device_by_name(&name)
	{
		device
	} else {
		match controller.get_default_device() {
			Ok(device) => device,
			Err(e) => {
				eprintln!("Error getting the default device: {}", e);
				return None;
			}
		}
	};

	// Adjust the volume / mute state
	const VOLUME_CHANGE_DELTA: f64 = 5_f64;
	let delta = volume_from_f64(
		step.unwrap_or_default()
			.parse::<f64>()
			.unwrap_or(VOLUME_CHANGE_DELTA),
	);
	match change_type {
		VolumeChangeType::Raise => {
			let max_volume = volume_from_f64(get_max_volume() as f64);
			if let Some(volume) = device.volume.clone().inc_clamp(delta, max_volume) {
				controller.set_device_volume_by_index(device.index, volume);
				controller.set_device_mute_by_index(device.index, false);
			}
		}
		VolumeChangeType::Lower => {
			if let Some(volume) = device.volume.clone().decrease(delta) {
				controller.set_device_volume_by_index(device.index, volume);
				controller.set_device_mute_by_index(device.index, false);
			}
		}
		VolumeChangeType::MuteToggle => {
			controller.set_device_mute_by_index(device.index, !device.mute);
		}
	}

	match controller.get_device_by_index(device.index) {
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
) -> brightness_backend::BrightnessBackendResult {
	let min_brightness = get_min_brightness();
	const BRIGHTNESS_CHANGE_DELTA: u8 = 5;
	let value = step.unwrap_or_default().parse::<u8>();

	let mut backend = brightness_backend::get_preferred_backend(get_device_name())?;

	match change_type {
		BrightnessChangeType::Raise => backend.raise(
			value.unwrap_or(BRIGHTNESS_CHANGE_DELTA) as u32,
			min_brightness,
		)?,
		BrightnessChangeType::Lower => backend.lower(
			value.unwrap_or(BRIGHTNESS_CHANGE_DELTA) as u32,
			min_brightness,
		)?,
		BrightnessChangeType::Set => backend.set(value.unwrap() as u32, min_brightness)?,
	};

	Ok(backend)
}

pub fn get_system_css_path() -> Option<PathBuf> {
	let mut paths: Vec<PathBuf> = Vec::new();
	for path in system_config_dirs() {
		paths.push(path.join("swayosd").join("style.css"));
	}

	paths.push(Path::new("/usr/local/etc/xdg/swaync/style.css").to_path_buf());

	let mut path: Option<PathBuf> = None;
	for try_path in paths {
		if try_path.exists() {
			path = Some(try_path);
			break;
		}
	}

	path
}

pub fn user_style_path(custom_path: Option<PathBuf>) -> Option<String> {
	let path = user_config_dir().join("swayosd").join("style.css");
	if let Some(custom_path) = custom_path {
		if custom_path.exists() {
			return custom_path.to_str().map(|s| s.to_string());
		}
	}
	if path.exists() {
		return path.to_str().map(|s| s.to_string());
	}
	None
}
