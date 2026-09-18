use async_std::task::{self, sleep};
use config::DBUS_PATH;
use dbus_server::DbusServer;
use evdev_rs::enums::{int_to_ev_key, EventCode, EV_KEY, EV_LED};
use evdev_rs::DeviceWrapper;
use input::event::device::DeviceEvent;
use input::event::keyboard::KeyboardEventTrait;
use input::event::{EventTrait, KeyboardEvent};
use input::{Event, Libinput, LibinputInterface};
use input_state::InputState;
use libc::O_RDWR;
use nix::poll::{poll, PollFd, PollFlags};
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::unix::{fs::OpenOptionsExt, io::OwnedFd};
use std::path::Path;
use std::time::Duration;
use zbus::object_server::InterfaceRef;

#[path = "../config.rs"]
mod config;
mod dbus_server;
mod input_state;

struct EventInfo {
	device_path: String,
	ev_key: EV_KEY,
}

struct Interface;

impl LibinputInterface for Interface {
	fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
		OpenOptions::new()
			.custom_flags(flags)
			.read(flags & O_RDWR != 0)
			.open(path)
			.map(|file| file.into())
			.map_err(|err| err.raw_os_error().unwrap())
	}
	fn close_restricted(&mut self, fd: OwnedFd) {
		drop(File::from(fd));
	}
}

fn main() -> Result<(), zbus::Error> {
	// Parse Config
	let input_config = config::backend::read_backend_config()
		.expect("Failed to parse config file")
		.input;

	// Create DBUS server
	let connection = task::block_on(DbusServer.init());
	let object_server = connection.object_server();
	let iface_ref = task::block_on(object_server.interface::<_, DbusServer>(DBUS_PATH))?;

	// Init libinput
	let mut input = Libinput::new_with_udev(Interface);
	input
		.udev_assign_seat("seat0")
		.expect("Could not assign seat0");
	let fd = input.as_raw_fd();
	assert!(fd != -1);
	let borrowed_fd = unsafe { BorrowedFd::borrow_raw(fd) };
	let pollfd = PollFd::new(borrowed_fd, PollFlags::POLLIN);
	let mut input_state = InputState::default();
	while poll(&mut [pollfd.clone()], None::<u8>).is_ok() {
		if let Err(error) = event(&input_config, &mut input_state, &mut input, &iface_ref) {
			eprintln!("Event error: {:?}", error);
		}
	}

	Ok(())
}

fn event(
	input_config: &config::backend::InputBackendConfig,
	input_state: &mut InputState,
	input: &mut Libinput,
	iface_ref: &InterfaceRef<DbusServer>,
) -> Result<(), Box<dyn std::error::Error>> {
	input.dispatch()?;
	for event in input {
		let event = match event {
			Event::Device(DeviceEvent::Added(event)) => {
				initialize_device_state(input_state, event.device());
				continue;
			}
			Event::Device(DeviceEvent::Removed(event)) => {
				input_state.remove_device(event.device().sysname());
				continue;
			}
			Event::Keyboard(KeyboardEvent::Key(event)) => event,
			_ => continue,
		};
		let device = event.device();
		let device_id = device.sysname().to_owned();
		let Some(ev_key) = int_to_ev_key(event.key()) else {
			continue;
		};
		if !input_state.handle_key_event(input_config, &device_id, ev_key, event.key_state()) {
			continue;
		}

		let ev_key = match ev_key {
				// Basic Lock keys
				key @ EV_KEY::KEY_CAPSLOCK |
				key @ EV_KEY::KEY_NUMLOCK |
				key @ EV_KEY::KEY_SCROLLLOCK |
				// Display Brightness
				key @ EV_KEY::KEY_BRIGHTNESSUP |
				key @ EV_KEY::KEY_BRIGHTNESSDOWN |
				key @ EV_KEY::KEY_BRIGHTNESS_MIN |
				key @ EV_KEY::KEY_BRIGHTNESS_MAX |
				key @ EV_KEY::KEY_BRIGHTNESS_AUTO |
				key @ EV_KEY::KEY_BRIGHTNESS_CYCLE |
				// Keyboard Illumination
				key @ EV_KEY::KEY_KBDILLUMUP |
				key @ EV_KEY::KEY_KBDILLUMDOWN |
				key @ EV_KEY::KEY_KBDILLUMTOGGLE => key,
				// Keyboard Layout
				key @ EV_KEY::KEY_KBD_LAYOUT_NEXT => key,
				// Audio Keys
				key @ EV_KEY::KEY_VOLUMEUP |
				key @ EV_KEY::KEY_VOLUMEDOWN |
				key @ EV_KEY::KEY_MUTE |
				key @ EV_KEY::KEY_UNMUTE |
				key @ EV_KEY::KEY_MICMUTE => key,
				// Touchpad
				key @ EV_KEY::KEY_TOUCHPAD_ON |
				key @ EV_KEY::KEY_TOUCHPAD_OFF |
				key @ EV_KEY::KEY_TOUCHPAD_TOGGLE |
				// Media Keys
				key @ EV_KEY::KEY_PREVIOUSSONG |
				key @ EV_KEY::KEY_PLAYPAUSE |
				key @ EV_KEY::KEY_PLAY |
				key @ EV_KEY::KEY_PAUSE |
				key @ EV_KEY::KEY_NEXTSONG => key,
				_ => continue,
			};

		if let Some(device) = unsafe { device.udev_device() }
			&& let Some(path) = device.devnode()
			&& let Some(path) = path.to_str()
		{
			let event_info = EventInfo {
				device_path: path.to_owned(),
				ev_key,
			};
			task::spawn(call(event_info, iface_ref.clone()));
		}
	}
	Ok(())
}

fn initialize_device_state(input_state: &mut InputState, device: input::Device) {
	let Some(udev_device) = (unsafe { device.udev_device() }) else {
		return;
	};
	let Some(device_path) = udev_device.devnode() else {
		return;
	};
	let Ok(evdev_device) = evdev_rs::Device::new_from_path(device_path) else {
		return;
	};

	let pressed_shift_keys = [EV_KEY::KEY_LEFTSHIFT, EV_KEY::KEY_RIGHTSHIFT]
		.into_iter()
		.filter(|key| evdev_key_is_pressed(evdev_device.event_value(&EventCode::EV_KEY(*key))));
	input_state.add_device(device.sysname(), pressed_shift_keys);
}

fn evdev_key_is_pressed(value: Option<i32>) -> bool {
	matches!(value, Some(value) if value > 0)
}

async fn call(event_info: EventInfo, iface_ref: InterfaceRef<DbusServer>) {
	// Wait for the LED value to change
	sleep(Duration::from_millis(50)).await;

	let Ok(device) = evdev_rs::Device::new_from_path(event_info.device_path) else {
		return;
	};

	let lock_state = match event_info.ev_key {
		EV_KEY::KEY_CAPSLOCK => device.event_value(&EventCode::EV_LED(EV_LED::LED_CAPSL)),
		EV_KEY::KEY_NUMLOCK => device.event_value(&EventCode::EV_LED(EV_LED::LED_NUML)),
		EV_KEY::KEY_SCROLLLOCK => device.event_value(&EventCode::EV_LED(EV_LED::LED_SCROLLL)),
		_ => None,
	};

	// Send signal
	let signal_result = DbusServer::key_pressed(
		iface_ref.signal_emitter(),
		event_info.ev_key as u16,
		lock_state.unwrap_or(-1),
	)
	.await;

	if let Err(error) = signal_result {
		eprintln!("Signal Error: {}", error)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn evdev_zero_value_means_key_is_released() {
		assert!(!evdev_key_is_pressed(Some(0)));
	}

	#[test]
	fn evdev_one_value_means_key_is_pressed() {
		assert!(evdev_key_is_pressed(Some(1)));
	}

	#[test]
	fn missing_evdev_value_means_key_is_unsupported() {
		assert!(!evdev_key_is_pressed(None));
	}
}
