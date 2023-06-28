use crate::config::{self, DBUS_SERVER_NAME};
use crate::osd_window::SwayosdWindow;
use crate::utils::*;
use gtk::gio::{ApplicationFlags, BusNameWatcherFlags, BusType, Cancellable};
use gtk::gio::{SignalSubscriptionId, SimpleAction};
use gtk::glib::variant::DictEntry;
use gtk::glib::{
	clone, MainContext, OptionArg, OptionFlags, Priority, SignalHandlerId, Variant, VariantTy,
};
use gtk::prelude::*;
use gtk::*;
use pulsectl::controllers::{SinkController, SourceController};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const ACTION_NAME: &str = "action";
const ACTION_FORMAT: &str = "(ss)";

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum ArgTypes {
	None = 0,
	CapsLock = 1,
	MaxVolume = 2,
	SinkVolumeRaise = 3,
	SinkVolumeLower = 4,
	SinkVolumeMuteToggle = 5,
	SourceVolumeRaise = 6,
	SourceVolumeLower = 7,
	SourceVolumeMuteToggle = 8,
	BrightnessRaise = 9,
	BrightnessLower = 10,
	NumLock = 11,
	ScrollLock = 12,
	// should always be first to set a global variable before executing related functions
	DeviceName = isize::MIN,
}

impl ArgTypes {
	pub fn as_str(&self) -> &'static str {
		match self {
			ArgTypes::None => "NONE",
			ArgTypes::CapsLock => "CAPSLOCK",
			ArgTypes::MaxVolume => "MAX-VOLUME",
			ArgTypes::SinkVolumeRaise => "SINK-VOLUME-RAISE",
			ArgTypes::SinkVolumeLower => "SINK-VOLUME-LOWER",
			ArgTypes::SinkVolumeMuteToggle => "SINK-VOLUME-MUTE-TOGGLE",
			ArgTypes::SourceVolumeRaise => "SOURCE-VOLUME-RAISE",
			ArgTypes::SourceVolumeLower => "SOURCE-VOLUME-LOWER",
			ArgTypes::SourceVolumeMuteToggle => "SOURCE-VOLUME-MUTE-TOGGLE",
			ArgTypes::BrightnessRaise => "BRIGHTNESS-RAISE",
			ArgTypes::BrightnessLower => "BRIGHTNESS-LOWER",
			ArgTypes::NumLock => "NUM-LOCK",
			ArgTypes::ScrollLock => "SCROLL-LOCK",
			ArgTypes::DeviceName => "DEVICE-NAME",
		}
	}

	pub fn parse(osd_type: Option<String>, value: Option<String>) -> (Self, Option<String>) {
		match osd_type {
			Some(osd_type) => match osd_type.as_str() {
				"CAPSLOCK" => (ArgTypes::CapsLock, value),
				"SINK-VOLUME-RAISE" => (ArgTypes::SinkVolumeRaise, value),
				"SINK-VOLUME-LOWER" => (ArgTypes::SinkVolumeLower, value),
				"SINK-VOLUME-MUTE-TOGGLE" => (ArgTypes::SinkVolumeMuteToggle, value),
				"SOURCE-VOLUME-RAISE" => (ArgTypes::SourceVolumeRaise, value),
				"SOURCE-VOLUME-LOWER" => (ArgTypes::SourceVolumeLower, value),
				"SOURCE-VOLUME-MUTE-TOGGLE" => (ArgTypes::SourceVolumeMuteToggle, value),
				"BRIGHTNESS-RAISE" => (ArgTypes::BrightnessRaise, value),
				"BRIGHTNESS-LOWER" => (ArgTypes::BrightnessLower, value),
				"MAX-VOLUME" => (ArgTypes::MaxVolume, value),
				"NUM-LOCK" => (ArgTypes::NumLock, value),
				"SCROLL-LOCK" => (ArgTypes::ScrollLock, value),
				"DEVICE-NAME" => (ArgTypes::DeviceName, value),
				_ => (ArgTypes::None, None),
			},
			None => (ArgTypes::None, None),
		}
	}
}

#[derive(Clone, Shrinkwrap)]
pub struct SwayOSDApplication {
	#[shrinkwrap(main_field)]
	app: gtk::Application,
	started: Rc<Cell<bool>>,
	action_id: Rc<RefCell<Option<SignalHandlerId>>>,
	windows: Rc<RefCell<Vec<SwayosdWindow>>>,
}

impl SwayOSDApplication {
	pub fn new() -> Self {
		let app = Application::new(Some("org.erikreider.swayosd"), ApplicationFlags::FLAGS_NONE);

		// Capslock cmdline arg
		app.add_main_option(
			"caps-lock",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::None,
			"Shows capslock osd. Note: Doesn't toggle CapsLock, just display the status",
			None,
		);
		app.add_main_option(
			"num-lock",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::None,
			"Shows numlock osd. Note: Doesn't toggle NumLock, just display the status",
			None,
		);
		app.add_main_option(
			"scroll-lock",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::None,
			"Shows scrolllock osd. Note: Doesn't toggle ScrollLock, just display the status",
			None,
		);
		// Capslock with specific LED cmdline arg
		app.add_main_option(
			"caps-lock-led",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows capslock osd. Uses LED class name. Note: Doesn't toggle CapsLock, just display the status",
			Some("LED class name (/sys/class/leds/NAME)"),
		);
		app.add_main_option(
			"num-lock-led",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows numlock osd. Uses LED class name. Note: Doesn't toggle NumLock, just display the status",
			Some("LED class name (/sys/class/leds/NAME)"),
		);
		app.add_main_option(
			"scroll-lock-led",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows scrolllock osd. Uses LED class name. Note: Doesn't toggle ScrollLock, just display the status",
			Some("LED class name (/sys/class/leds/NAME)"),
		);
		// Sink volume cmdline arg
		app.add_main_option(
			"output-volume",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows volume osd and raises, loweres or mutes default sink volume",
			Some("raise|lower|mute-toggle|(±)number"),
		);
		// Sink volume cmdline arg
		app.add_main_option(
			"input-volume",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows volume osd and raises, loweres or mutes default source volume",
			Some("raise|lower|mute-toggle|(±)number"),
		);

		// Sink brightness cmdline arg
		app.add_main_option(
			"brightness",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows brightness osd and raises or loweres all available sources of brightness device",
			Some("raise|lower|(±)number"),
		);
		app.add_main_option(
			"max-volume",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Sets the maximum Volume",
			Some("(+)number"),
		);
		app.add_main_option(
			"device",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"For which device to increase/decrease audio",
			Some("Pulseaudio device name (pactl list short sinks|sources)"),
		);

		// Parse args
		app.connect_handle_local_options(|app, args| -> i32 {
			let variant = args.to_variant();

			if variant.n_children() == 0 {
				return -1;
			}

			if !variant.is_container() {
				eprintln!("VariantDict isn't a container!...");
				return 1;
			}
			let mut actions = Vec::new();

			for i in 0..variant.n_children() {
				let child: DictEntry<String, Variant> = variant.child_get(i);

				let (option, value): (ArgTypes, Option<String>) = match child.key().as_str() {
					"caps-lock" => (ArgTypes::CapsLock, None),
					"caps-lock-led" => match child.value().str() {
						Some(led) => (ArgTypes::CapsLock, Some(led.to_owned())),
						None => {
							eprintln!("Value for caps-lock-led isn't a string!...");
							return 1;
						}
					},
					"output-volume" => {
						let value = child.value().str().unwrap_or("");
						let parsed = volume_parser(false, value);
						match parsed {
							Ok(p) => p,
							Err(e) => return e,
						}
					}
					"input-volume" => {
						let value = child.value().str().unwrap_or("");
						let parsed = volume_parser(true, value);
						match parsed {
							Ok(p) => p,
							Err(e) => return e,
						}
					}
					"brightness" => {
						let value = child.value().str().unwrap_or("");
						match (value, value.parse::<i8>()) {
							// Parse custom step values
							(_, Ok(num)) => (
								if num.is_positive() {
									ArgTypes::BrightnessRaise
								} else {
									ArgTypes::BrightnessLower
								},
								Some(num.abs().to_string()),
							),
							("raise", _) => (ArgTypes::BrightnessRaise, None),
							("lower", _) => (ArgTypes::BrightnessLower, None),
							(e, _) => {
								eprintln!("Unknown brightness mode: \"{}\"!...", e);
								return 1;
							}
						}
					}
					"max-volume" => {
						let value = child.value().str().unwrap_or("").trim();
						match value.parse::<u8>() {
							Ok(_) => (ArgTypes::MaxVolume, Some(value.to_string())),
							Err(_) => {
								eprintln!("{} is not a number between 0 and {}!", value, u8::MAX);
								return 1;
							}
						}
					}
					"device" => {
						let value = match child.value().str() {
							Some(v) => v.to_string(),
							None => {
								eprintln!("--device found but no name given");
								return 1;
							}
						};
						(ArgTypes::DeviceName, Some(value))
					}
					e => {
						eprintln!("Unknown Variant Key: \"{}\"!...", e);
						return 1;
					}
				};
				if option != ArgTypes::None {
					actions.push((option, value));
				}
			}

			// sort actions so that they always get executed in the correct order
			for i in 0..actions.len() - 1 {
				for j in i + 1..actions.len() {
					if actions[i].0 > actions[j].0 {
						let temp = actions[i].clone();
						actions[i] = actions[j].clone();
						actions[j] = temp;
					}
				}
			}

			// execute the sorted actions
			for action in actions {
				let variant = Variant::tuple_from_iter([
					action.0.as_str().to_variant(),
					action.1.unwrap_or(String::new()).to_variant(),
				]);
				app.activate_action(ACTION_NAME, Some(&variant));
			}
			0
		});

		// Listen to the LibInput Backend and activate the Application action
		let (sender, receiver) = MainContext::channel::<(u16, i32)>(Priority::default());
		receiver.attach(
			None,
			clone!(@strong app => @default-return Continue(false), move |(key_code, state)| {
				Self::key_pressed_cb(&app, key_code, state);
				Continue(true)
			}),
		);
		// Start watching for the LibInput Backend
		let signal_id: Arc<Mutex<Option<SignalSubscriptionId>>> = Arc::new(Mutex::new(None));
		gio::bus_watch_name(
			BusType::System,
			DBUS_SERVER_NAME,
			BusNameWatcherFlags::NONE,
			clone!(@strong sender, @strong signal_id => move |connection, _, _| {
				println!("Connecting to the SwayOSD LibInput Backend");
				let mut mutex = match signal_id.lock() {
					Ok(mut mutex) => match mutex.as_mut() {
						Some(_) => return,
						None => mutex,
					},
					Err(error) => return println!("Mutex lock Error: {}", error),
				};
				mutex.replace(connection.signal_subscribe(
					Some(config::DBUS_SERVER_NAME),
					Some(config::DBUS_SERVER_NAME),
					Some("KeyPressed"),
					Some(config::DBUS_SERVER_PATH),
					None,
					gio::DBusSignalFlags::NONE,
					clone!(@strong sender => move |_, _, _, _, _, variant| {
						let key_code = variant.try_child_get::<u16>(0);
						let state = variant.try_child_get::<i32>(1);
						match (key_code, state) {
							(Ok(Some(key_code)), Ok(Some(state))) => {
								if let Err(error) = sender.send((key_code, state)) {
									eprintln!("Channel Send error: {}", error);
								}
							},
							variables => return eprintln!("Variables don't match: {:?}", variables),
						};
					}),
				));
			}),
			clone!(@strong signal_id => move|connection, _| {
				eprintln!("SwayOSD LibInput Backend isn't available, waiting...");
				match signal_id.lock() {
					Ok(mut mutex) => if let Some(sig_id) = mutex.take() {
						connection.signal_unsubscribe(sig_id);
					},
					Err(error) => println!("Mutex lock Error: {}", error),
				}
			}),
		);

		SwayOSDApplication {
			app,
			started: Rc::new(Cell::new(false)),
			action_id: Rc::new(RefCell::new(None)),
			windows: Rc::new(RefCell::new(Vec::new())),
		}
	}

	pub fn start(&self) -> i32 {
		let s = self.clone();
		self.app.connect_activate(move |_| {
			if s.started.get() {
				return;
			}
			s.started.set(true);

			s.initialize();
		});

		match VariantTy::new(ACTION_FORMAT) {
			Ok(variant_ty) => {
				let action = SimpleAction::new(ACTION_NAME, Some(variant_ty));
				let s = self.clone();
				self.action_id.replace(Some(
					action.connect_activate(move |sa, v| s.action_activated(sa, v)),
				));
				self.app.add_action(&action);
				let _ = self.app.register(Cancellable::NONE);
			}
			Err(x) => {
				eprintln!("VARIANT TYPE ERROR: {}", x.message);
				std::process::exit(1);
			}
		}

		self.app.run().into()
	}

	fn key_pressed_cb(app: &gtk::Application, key_code: u16, state: i32) {
		let (option, value): (ArgTypes, Option<String>) =
			match evdev_rs::enums::int_to_ev_key(key_code as u32) {
				Some(evdev_rs::enums::EV_KEY::KEY_CAPSLOCK) => {
					(ArgTypes::CapsLock, Some(state.to_string()))
				}
				e => {
					eprintln!("Unknown Key in signal: \"{:?}\"!...", e);
					return;
				}
			};
		let variant = Variant::tuple_from_iter([
			option.as_str().to_variant(),
			value.unwrap_or(String::new()).to_variant(),
		]);
		app.activate_action(ACTION_NAME, Some(&variant));
	}

	fn action_activated(&self, action: &SimpleAction, variant: Option<&Variant>) {
		if !self.started.get() {
			self.started.set(true);
			self.initialize();
		}
		match self.action_id.take() {
			Some(action_id) => action.disconnect(action_id),
			None => return,
		}

		if let Some(variant) = variant {
			let osd_type = variant.try_child_get::<String>(0);
			let value = variant.try_child_get::<String>(1);
			let (osd_type, value) = match (osd_type, value) {
				(Ok(osd_type), Ok(Some(value))) => {
					(osd_type, if value.is_empty() { None } else { Some(value) })
				}
				_ => (None, None),
			};
			match ArgTypes::parse(osd_type, value) {
				(ArgTypes::SinkVolumeRaise, step) => {
					let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
					if let Some(device) =
						change_device_volume(&mut device_type, VolumeChangeType::Raise, step)
					{
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, &device_type);
						}
					}
				}
				(ArgTypes::SinkVolumeLower, step) => {
					let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
					if let Some(device) =
						change_device_volume(&mut device_type, VolumeChangeType::Lower, step)
					{
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, &device_type);
						}
					}
				}
				(ArgTypes::SinkVolumeMuteToggle, _) => {
					let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
					if let Some(device) =
						change_device_volume(&mut device_type, VolumeChangeType::MuteToggle, None)
					{
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, &device_type);
						}
					}
				}
				(ArgTypes::SourceVolumeRaise, step) => {
					let mut device_type =
						VolumeDeviceType::Source(SourceController::create().unwrap());
					if let Some(device) =
						change_device_volume(&mut device_type, VolumeChangeType::Raise, step)
					{
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, &device_type);
						}
					}
				}
				(ArgTypes::SourceVolumeLower, step) => {
					let mut device_type =
						VolumeDeviceType::Source(SourceController::create().unwrap());
					if let Some(device) =
						change_device_volume(&mut device_type, VolumeChangeType::Lower, step)
					{
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, &device_type);
						}
					}
				}
				(ArgTypes::SourceVolumeMuteToggle, _) => {
					let mut device_type =
						VolumeDeviceType::Source(SourceController::create().unwrap());
					if let Some(device) =
						change_device_volume(&mut device_type, VolumeChangeType::MuteToggle, None)
					{
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, &device_type);
						}
					}
				}
				// TODO: Brightness
				(ArgTypes::BrightnessRaise, step) => {
					if let Ok(Some(device)) = change_brightness(BrightnessChangeType::Raise, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_brightness(&device);
						}
					}
				}
				(ArgTypes::BrightnessLower, step) => {
					if let Ok(Some(device)) = change_brightness(BrightnessChangeType::Lower, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_brightness(&device);
						}
					}
				}
				(ArgTypes::CapsLock, value) => {
					let i32_value = value.clone().unwrap_or("-1".to_owned());
					let state = match i32_value.parse::<i32>() {
						Ok(value) if value >= 0 && value <= 1 => value == 1,
						_ => get_caps_lock_state(value),
					};
					for window in self.windows.borrow().to_owned() {
						window.changed_capslock(state)
					}
				}
				(ArgTypes::NumLock, value) => {
					let i32_value = value.clone().unwrap_or("-1".to_owned());
					let state = match i32_value.parse::<i32>() {
						Ok(value) if value >= 0 && value <= 1 => value == 1,
						_ => get_caps_lock_state(value),
					};
					for window in self.windows.borrow().to_owned() {
						window.changed_capslock(state)
					}
				}
				(ArgTypes::ScrollLock, value) => {
					let i32_value = value.clone().unwrap_or("-1".to_owned());
					let state = match i32_value.parse::<i32>() {
						Ok(value) if value >= 0 && value <= 1 => value == 1,
						_ => get_caps_lock_state(value),
					};
					for window in self.windows.borrow().to_owned() {
						window.changed_capslock(state)
					}
				}
				(ArgTypes::MaxVolume, max) => set_max_volume(max),
				(ArgTypes::DeviceName, name) => set_device_name(name.unwrap()),
				(ArgTypes::None, _) => {
					eprintln!("Failed to parse variant: {}!...", variant.print(true))
				}
			};
		}

		let s = self.clone();
		let id = action.connect_activate(move |sa, v| s.action_activated(sa, v));
		self.action_id.replace(Some(id));
	}

	fn initialize(&self) {
		let display: gdk::Display = match gdk::Display::default() {
			Some(x) => x,
			_ => return,
		};

		self.init_windows(&display);

		let _self = self;

		display.connect_opened(clone!(@strong _self => move |d| {
			_self.init_windows(d);
		}));

		display.connect_closed(clone!(@strong _self => move |_d, is_error| {
			if is_error {
				eprintln!("Display closed due to errors...");
			}
			_self.close_all_windows();
		}));

		display.connect_monitor_added(clone!(@strong _self => move |d, mon| {
			_self.add_window(d, mon);
		}));

		display.connect_monitor_removed(clone!(@strong _self => move |d, _mon| {
			_self.init_windows(d);
		}));
	}

	fn add_window(&self, display: &gdk::Display, monitor: &gdk::Monitor) {
		let win = SwayosdWindow::new(&self.app, display, monitor);
		self.windows.borrow_mut().push(win);
	}

	fn init_windows(&self, display: &gdk::Display) {
		self.close_all_windows();

		for i in 0..display.n_monitors() {
			let monitor: gdk::Monitor = match display.monitor(i) {
				Some(x) => x,
				_ => continue,
			};
			self.add_window(display, &monitor);
		}
	}

	fn close_all_windows(&self) {
		self.windows.borrow_mut().retain(|window| {
			window.close();
			false
		});
	}
}
