use async_std::task::{self, sleep};
use config::DBUS_PATH;
use dbus_server::DbusServer;
use evdev_rs::enums::{int_to_ev_key, EventCode, EV_KEY, EV_LED};
use evdev_rs::DeviceWrapper;
use input::event::keyboard::KeyboardEventTrait;
use input::event::tablet_pad::KeyState;
use input::event::{EventTrait, KeyboardEvent};
use input::{Event, Libinput, LibinputInterface};
use libc::{O_RDONLY, O_RDWR};
use nix::poll::{poll, PollFd, PollFlags};
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::{fs::OpenOptionsExt, io::OwnedFd};
use std::path::Path;
use std::time::Duration;
use zbus::InterfaceRef;

#[path = "../config.rs"]
mod config;
mod dbus_server;

struct EventInfo {
	device_path: String,
	ev_key: EV_KEY,
}

struct Interface;

impl LibinputInterface for Interface {
	fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
		OpenOptions::new()
			.custom_flags(flags)
			.read((flags & O_RDONLY != 0) | (flags & O_RDWR != 0))
			.open(path)
			.map(|file| file.into())
			.map_err(|err| err.raw_os_error().unwrap())
	}
	fn close_restricted(&mut self, fd: OwnedFd) {
		drop(File::from(fd));
	}
}

fn main() -> Result<(), zbus::Error> {
	// Create DBUS server
	let connection = task::block_on(DbusServer.init());
	let object_server = connection.object_server();
	let iface_ref = task::block_on(object_server.interface::<_, DbusServer>(DBUS_PATH))?;

	// Init libinput
	let mut input = Libinput::new_with_udev(Interface);
	input
		.udev_assign_seat("seat0")
		.expect("Could not assign seat0");

	let pollfd = PollFd::new(input.as_raw_fd(), PollFlags::POLLIN);
	while poll(&mut [pollfd], -1).is_ok() {
		event(&mut input, &iface_ref);
	}

	Ok(())
}

fn event(input: &mut Libinput, iface_ref: &InterfaceRef<DbusServer>) {
	input.dispatch().unwrap();
	for event in input.into_iter() {
		if let Event::Keyboard(KeyboardEvent::Key(event)) = event {
			if event.key_state() == KeyState::Pressed {
				continue;
			}
			let device = match unsafe { event.device().udev_device() } {
				Some(device) => device,
				None => continue,
			};

			let ev_key = match int_to_ev_key(event.key()) {
				// Basic Lock keys
				Some(key @ EV_KEY::KEY_CAPSLOCK) |
				Some(key @ EV_KEY::KEY_NUMLOCK) |
				Some(key @ EV_KEY::KEY_SCROLLLOCK) |
				// Display Brightness
				Some(key @ EV_KEY::KEY_BRIGHTNESSUP) |
				Some(key @ EV_KEY::KEY_BRIGHTNESSDOWN) |
				Some(key @ EV_KEY::KEY_BRIGHTNESS_MIN) |
				Some(key @ EV_KEY::KEY_BRIGHTNESS_MAX) |
				Some(key @ EV_KEY::KEY_BRIGHTNESS_AUTO) |
				Some(key @ EV_KEY::KEY_BRIGHTNESS_CYCLE) |
				// Keyboard Illumination
				Some(key @ EV_KEY::KEY_KBDILLUMUP) |
				Some(key @ EV_KEY::KEY_KBDILLUMDOWN) |
				Some(key @ EV_KEY::KEY_KBDILLUMTOGGLE) => key,
				// Keyboard Layout
				Some(key @ EV_KEY::KEY_KBD_LAYOUT_NEXT) => key,
				// Audio Keys
				Some(key @ EV_KEY::KEY_VOLUMEUP) |
				Some(key @ EV_KEY::KEY_VOLUMEDOWN) |
				Some(key @ EV_KEY::KEY_MUTE) |
				Some(key @ EV_KEY::KEY_UNMUTE) |
				Some(key @ EV_KEY::KEY_MICMUTE) => key,
				// Touchpad
				Some(key @ EV_KEY::KEY_TOUCHPAD_ON) |
				Some(key @ EV_KEY::KEY_TOUCHPAD_OFF) |
				Some(key @ EV_KEY::KEY_TOUCHPAD_TOGGLE) |
				// Media Keys
				Some(key @ EV_KEY::KEY_PREVIOUSSONG) |
				Some(key @ EV_KEY::KEY_PLAYPAUSE) |
				Some(key @ EV_KEY::KEY_PLAY) |
				Some(key @ EV_KEY::KEY_PAUSE) |
				Some(key @ EV_KEY::KEY_NEXTSONG) => key,
				_ => continue,
			};

			if let Some(path) = device.devnode() {
				if let Some(path) = path.to_str() {
					let event_info = EventInfo {
						device_path: path.to_owned(),
						ev_key,
					};
					task::spawn(call(event_info, iface_ref.clone()));
				}
			}
		}
	}
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
		iface_ref.signal_context(),
		event_info.ev_key as u16,
		lock_state.unwrap_or(-1),
	)
	.await;

	if let Err(error) = signal_result {
		eprintln!("Signal Error: {}", error)
	}
}
