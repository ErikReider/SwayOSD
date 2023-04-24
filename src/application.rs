use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gio::{ApplicationFlags, Cancellable};
use gtk::glib::variant::DictEntry;
use gtk::glib::{clone, OptionArg, OptionFlags, SignalHandlerId, Variant, VariantTy};
use gtk::prelude::*;
use gtk::*;

use crate::osd_window::SwayosdWindow;
use crate::utils::*;

use gtk::gio::SimpleAction;

const ACTION_NAME: &str = "action";
const ACTION_FORMAT: &str = "(ss)";

#[derive(Debug, PartialEq)]
pub enum OsdTypes {
	None = 0,
	CapsLock = 1,
	SinkVolumeRaise = 2,
	SinkVolumeLower = 3,
	SinkVolumeMuteToggle = 4,
	SourceVolumeRaise = 5,
	SourceVolumeLower = 6,
	SourceVolumeMuteToggle = 7,
	BrightnessRaise = 8,
	BrightnessLower = 9,
}
impl OsdTypes {
	pub fn as_str(&self) -> &'static str {
		match self {
			OsdTypes::None => "NONE",
			OsdTypes::CapsLock => "CAPSLOCK",
			OsdTypes::SinkVolumeRaise => "SINK-VOLUME-RAISE",
			OsdTypes::SinkVolumeLower => "SINK-VOLUME-LOWER",
			OsdTypes::SinkVolumeMuteToggle => "SINK-VOLUME-MUTE-TOGGLE",
			OsdTypes::SourceVolumeRaise => "SOURCE-VOLUME-RAISE",
			OsdTypes::SourceVolumeLower => "SOURCE-VOLUME-LOWER",
			OsdTypes::SourceVolumeMuteToggle => "SOURCE-VOLUME-MUTE-TOGGLE",
			OsdTypes::BrightnessRaise => "BRIGHTNESS-RAISE",
			OsdTypes::BrightnessLower => "BRIGHTNESS-LOWER",
		}
	}

	pub fn parse(osd_type: Option<String>, value: Option<String>) -> (Self, Option<String>) {
		match osd_type {
			Some(osd_type) => match osd_type.as_str() {
				"CAPSLOCK" => (OsdTypes::CapsLock, value),
				"SINK-VOLUME-RAISE" => (OsdTypes::SinkVolumeRaise, value),
				"SINK-VOLUME-LOWER" => (OsdTypes::SinkVolumeLower, value),
				"SINK-VOLUME-MUTE-TOGGLE" => (OsdTypes::SinkVolumeMuteToggle, value),
				"SOURCE-VOLUME-RAISE" => (OsdTypes::SourceVolumeRaise, value),
				"SOURCE-VOLUME-LOWER" => (OsdTypes::SourceVolumeLower, value),
				"SOURCE-VOLUME-MUTE-TOGGLE" => (OsdTypes::SourceVolumeMuteToggle, value),
				"BRIGHTNESS-RAISE" => (OsdTypes::BrightnessRaise, value),
				"BRIGHTNESS-LOWER" => (OsdTypes::BrightnessLower, value),
				_ => (OsdTypes::None, None),
			},
			None => (OsdTypes::None, None),
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

		// Parse args
		app.connect_handle_local_options(|app, args| -> i32 {
			let variant = args.to_variant();
			if variant.n_children() > 1 {
				eprintln!("Only run with one arg at once!...");
				return 1;
			} else if variant.n_children() == 0 {
				return -1;
			}

			if !variant.is_container() {
				eprintln!("VariantDict isn't a container!...");
				return 1;
			}
			let child: DictEntry<String, Variant> = variant.child_get(0);
			let (osd_type, value): (OsdTypes, Option<String>) = match child.key().as_str() {
				"caps-lock" => (OsdTypes::CapsLock, None),
				"caps-lock-led" => match child.value().str() {
					Some(led) => (OsdTypes::CapsLock, Some(led.to_owned())),
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
								OsdTypes::SinkVolumeRaise
							} else {
								OsdTypes::SinkVolumeLower
							},
							Some(num.abs().to_string()),
						),
						("raise", _) => (OsdTypes::SinkVolumeRaise, None),
						("lower", _) => (OsdTypes::SinkVolumeLower, None),
						("mute-toggle", _) => (OsdTypes::SinkVolumeMuteToggle, None),
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
								OsdTypes::SourceVolumeRaise
							} else {
								OsdTypes::SourceVolumeLower
							},
							Some(num.abs().to_string()),
						),
						("raise", _) => (OsdTypes::SourceVolumeRaise, None),
						("lower", _) => (OsdTypes::SourceVolumeLower, None),
						("mute-toggle", _) => (OsdTypes::SourceVolumeMuteToggle, None),
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
								OsdTypes::BrightnessRaise
							} else {
								OsdTypes::BrightnessLower
							},
							Some(num.abs().to_string()),
						),
						("raise", _) => (OsdTypes::BrightnessRaise, None),
						("lower", _) => (OsdTypes::BrightnessLower, None),
						(e, _) => {
							eprintln!("Unknown brightness mode: \"{}\"!...", e);
							return 1;
						}
					}
				}
				e => {
					eprintln!("Unknown Variant Key: \"{}\"!...", e);
					return 1;
				}
			};
			let variant = Variant::tuple_from_iter([
				osd_type.as_str().to_variant(),
				value.unwrap_or(String::new()).to_variant(),
			]);
			app.activate_action(ACTION_NAME, Some(&variant));
			return 0;
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

		return self.app.run().into();
	}

	fn action_activated(&self, action: &SimpleAction, variant: Option<&Variant>) {
		if !self.started.get() {
			eprintln!("Please start the executable separately before running with args!...");
			return;
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
			match OsdTypes::parse(osd_type, value) {
				(OsdTypes::SinkVolumeRaise, step) => {
					if let Some(device) = change_sink_volume(VolumeChangeType::Raise, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, VolumeDeviceType::Sink);
						}
					}
				}
				(OsdTypes::SinkVolumeLower, step) => {
					if let Some(device) = change_sink_volume(VolumeChangeType::Lower, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, VolumeDeviceType::Sink);
						}
					}
				}
				(OsdTypes::SinkVolumeMuteToggle, _) => {
					if let Some(device) = change_sink_volume(VolumeChangeType::MuteToggle, None) {
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, VolumeDeviceType::Sink);
						}
					}
				}
				(OsdTypes::SourceVolumeRaise, step) => {
					if let Some(device) = change_source_volume(VolumeChangeType::Raise, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, VolumeDeviceType::Source);
						}
					}
				}
				(OsdTypes::SourceVolumeLower, step) => {
					if let Some(device) = change_source_volume(VolumeChangeType::Lower, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, VolumeDeviceType::Source);
						}
					}
				}
				(OsdTypes::SourceVolumeMuteToggle, _) => {
					if let Some(device) = change_source_volume(VolumeChangeType::MuteToggle, None) {
						for window in self.windows.borrow().to_owned() {
							window.changed_volume(&device, VolumeDeviceType::Source);
						}
					}
				}
				// TODO: Brightness
				(OsdTypes::BrightnessRaise, step) => {
					if let Ok(Some(device)) = change_brightness(BrightnessChangeType::Raise, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_brightness(&device);
						}
					}
				}
				(OsdTypes::BrightnessLower, step) => {
					if let Ok(Some(device)) = change_brightness(BrightnessChangeType::Lower, step) {
						for window in self.windows.borrow().to_owned() {
							window.changed_brightness(&device);
						}
					}
				}
				(OsdTypes::CapsLock, led) => {
					let state = get_caps_lock_state(led);
					for window in self.windows.borrow().to_owned() {
						window.changed_capslock(state)
					}
				}
				(OsdTypes::None, _) => {
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
			_self.init_windows(&d);
		}));

		display.connect_closed(clone!(@strong _self => move |_d, is_error| {
			if is_error {
				eprintln!("Display closed due to errors...");
			}
			_self.close_all_windows();
		}));

		display.connect_monitor_added(clone!(@strong _self => move |d, mon| {
			_self.add_window(&d, &mon);
		}));

		display.connect_monitor_removed(clone!(@strong _self => move |d, _mon| {
			_self.init_windows(&d);
		}));
	}

	fn add_window(&self, display: &gdk::Display, monitor: &gdk::Monitor) {
		let win = SwayosdWindow::new(&self.app, &display, &monitor);
		self.windows.borrow_mut().push(win);
	}

	fn init_windows(&self, display: &gdk::Display) {
		self.close_all_windows();

		for i in 0..display.n_monitors() {
			let monitor: gdk::Monitor = match display.monitor(i) {
				Some(x) => x,
				_ => continue,
			};
			self.add_window(&display, &monitor);
		}
	}

	fn close_all_windows(&self) {
		self.windows.borrow_mut().retain(|window| {
			window.close();
			false
		});
	}
}
