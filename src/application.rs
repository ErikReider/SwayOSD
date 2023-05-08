use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gio::{ApplicationFlags, Cancellable};
use gtk::glib::variant::DictEntry;
use gtk::glib::{clone, OptionArg, OptionFlags, SignalHandlerId, Variant, VariantTy};
use gtk::prelude::*;
use gtk::*;
use pulsectl::controllers::{SinkController, SourceController};

use crate::osd_window::SwayosdWindow;
use crate::utils::*;

use gtk::gio::SimpleAction;

const ACTION_NAME: &str = "action";
const ACTION_FORMAT: &str = "(ss)";

#[derive(Debug, PartialEq)]
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
		// Capslock with specific LED cmdline arg
		app.add_main_option(
			"caps-lock-led",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Shows capslock osd. Uses LED class name. Note: Doesn't toggle CapsLock, just display the status",
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
						match (value, value.parse::<i8>()) {
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
								return 1;
							}
						}
					}
					"input-volume" => {
						let value = child.value().str().unwrap_or("");
						match (value, value.parse::<i8>()) {
							// Parse custom step values
							(_, Ok(num)) => (
								if num.is_positive() {
									ArgTypes::SourceVolumeRaise
								} else {
									ArgTypes::SourceVolumeLower
								},
								Some(num.abs().to_string()),
							),
							("raise", _) => (ArgTypes::SourceVolumeRaise, None),
							("lower", _) => (ArgTypes::SourceVolumeLower, None),
							("mute-toggle", _) => (ArgTypes::SourceVolumeMuteToggle, None),
							(e, _) => {
								eprintln!("Unknown input volume mode: \"{}\"!...", e);
								return 1;
							}
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
					e => {
						eprintln!("Unknown Variant Key: \"{}\"!...", e);
						return 1;
					}
				};
				if option != ArgTypes::None {
					let variant = Variant::tuple_from_iter([
						option.as_str().to_variant(),
						value.unwrap_or(String::new()).to_variant(),
					]);
					actions.push((ACTION_NAME, Some(variant)));
				}
			}

			// sort actions so that they always get executed in the correct order
			for i in 0..actions.len() - 1 {
				for j in i + 1..actions.len() {
					if i < j {
						if actions[i].0 > actions[j].0 {
							let temp = actions[i].clone();
							actions[i] = actions[j].clone();
							actions[j] = temp;
						}
					}
				}
			}

			// execute the sorted actions
			for action in actions {
				app.activate_action(action.0, Some(&action.1.unwrap()));
			}
			0
		});

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
				(ArgTypes::CapsLock, led) => {
					let state = get_caps_lock_state(led);
					for window in self.windows.borrow().to_owned() {
						window.changed_capslock(state)
					}
				}
				(ArgTypes::MaxVolume, max) => set_max_volume(max),
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
