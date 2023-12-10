use crate::argtypes::ArgTypes;
use crate::config::{self, APPLICATION_NAME, DBUS_BACKEND_NAME};
use crate::global_utils::{handle_application_args, HandleLocalStatus};
use crate::osd_window::SwayosdWindow;
use crate::utils::{self, *};
use gtk::gio::SignalSubscriptionId;
use gtk::gio::{ApplicationFlags, BusNameWatcherFlags, BusType};
use gtk::glib::{clone, MainContext, OptionArg, OptionFlags, Priority, Receiver};
use gtk::prelude::*;
use gtk::*;
use pulsectl::controllers::{SinkController, SourceController};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone, Shrinkwrap)]
pub struct SwayOSDApplication {
	#[shrinkwrap(main_field)]
	app: gtk::Application,
	windows: Rc<RefCell<Vec<SwayosdWindow>>>,
}

impl SwayOSDApplication {
	pub fn new(action_receiver: Receiver<(ArgTypes, String)>) -> Self {
		let app = Application::new(Some(APPLICATION_NAME), ApplicationFlags::FLAGS_NONE);

		app.add_main_option(
			"style",
			glib::Char::from('s' as u8),
			OptionFlags::NONE,
			OptionArg::String,
			"Use a custom Stylesheet file instead of looking for one",
			Some("<CSS FILE PATH>"),
		);

		app.add_main_option(
			"top-margin",
			glib::Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			&format!(
				"OSD margin from top edge (0.5 would be screen center). Default is {}",
				*utils::TOP_MARGIN_DEFAULT
			),
			Some("<from 0.0 to 1.0>"),
		);

		let osd_app = SwayOSDApplication {
			app: app.clone(),
			windows: Rc::new(RefCell::new(Vec::new())),
		};

		// Parse args
		app.connect_handle_local_options(clone!(@strong osd_app => move |_app, args| {
			let actions = match handle_application_args(args.to_variant()) {
				(HandleLocalStatus::SUCCESS | HandleLocalStatus::CONTINUE, actions) => actions,
				(status @ HandleLocalStatus::FAILURE, _) => return status as i32,
			};
			for (arg_type, data) in actions {
				match (arg_type, data) {
					(ArgTypes::TopMargin, margin) => {
						let margin: Option<f32> = match margin {
							Some(margin) => match margin.parse::<f32>() {
								Ok(margin) => (0_f32..1_f32).contains(&margin).then_some(margin),
								_ => None,
							},
							_ => None,
						};
						set_top_margin(margin.unwrap_or(*TOP_MARGIN_DEFAULT))
					},
					(ArgTypes::MaxVolume, max) => {
						let volume: u8 = match max {
								Some(max) => match max.parse() {
									Ok(max) => max,
									_ => get_default_max_volume(),
								}
								_ => get_default_max_volume(),
							};
						set_default_max_volume(volume);
					},
					(arg_type, data) => Self::action_activated(&osd_app, arg_type, data),
				}
			}

			HandleLocalStatus::CONTINUE as i32
		}));

		// Listen to any Client actions
		action_receiver.attach(
			None,
			clone!(@strong osd_app => @default-return Continue(false), move |(arg_type, data)| {
				Self::action_activated(&osd_app, arg_type, (!data.is_empty()).then_some(data));
				Continue(true)
			}),
		);

		// Listen to the LibInput Backend and activate the Application action
		let (sender, receiver) = MainContext::channel::<(u16, i32)>(Priority::default());
		receiver.attach(
			None,
			clone!(@strong osd_app => @default-return Continue(false), move |(key_code, state)| {
				let (arg_type, data): (ArgTypes, Option<String>) =
					match evdev_rs::enums::int_to_ev_key(key_code as u32) {
						Some(evdev_rs::enums::EV_KEY::KEY_CAPSLOCK) => {
							(ArgTypes::CapsLock, Some(state.to_string()))
						}
						Some(evdev_rs::enums::EV_KEY::KEY_NUMLOCK) => {
							(ArgTypes::NumLock, Some(state.to_string()))
						}
						Some(evdev_rs::enums::EV_KEY::KEY_SCROLLLOCK) => {
							(ArgTypes::ScrollLock, Some(state.to_string()))
						}
						_ => return Continue(true),
					};
				Self::action_activated(&osd_app, arg_type, data);
				Continue(true)
			}),
		);
		// Start watching for the LibInput Backend
		let signal_id: Arc<Mutex<Option<SignalSubscriptionId>>> = Arc::new(Mutex::new(None));
		gio::bus_watch_name(
			BusType::System,
			DBUS_BACKEND_NAME,
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
					Some(config::DBUS_BACKEND_NAME),
					Some(config::DBUS_BACKEND_NAME),
					Some("KeyPressed"),
					Some(config::DBUS_PATH),
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

		return osd_app;
	}

	pub fn start(&self) -> i32 {
		let s = self.clone();
		self.app.connect_activate(move |_| {
			s.initialize();
		});

		let _ = self.app.register(gio::Cancellable::NONE);
		self.app.run().into()
	}

	fn action_activated(osd_app: &SwayOSDApplication, arg_type: ArgTypes, value: Option<String>) {
		match (arg_type, value) {
			(ArgTypes::SinkVolumeRaise, step) => {
				let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Raise, step)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
			}
			(ArgTypes::SinkVolumeLower, step) => {
				let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Lower, step)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
			}
			(ArgTypes::SinkVolumeMuteToggle, _) => {
				let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::MuteToggle, None)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
			}
			(ArgTypes::SourceVolumeRaise, step) => {
				let mut device_type = VolumeDeviceType::Source(SourceController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Raise, step)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
			}
			(ArgTypes::SourceVolumeLower, step) => {
				let mut device_type = VolumeDeviceType::Source(SourceController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Lower, step)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
			}
			(ArgTypes::SourceVolumeMuteToggle, _) => {
				let mut device_type = VolumeDeviceType::Source(SourceController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::MuteToggle, None)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
			}
			// TODO: Brightness
			(ArgTypes::BrightnessRaise, step) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Raise, step)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
			}
			(ArgTypes::BrightnessLower, step) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Lower, step)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
			}
			(ArgTypes::BrightnessSet, value) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Set, value)
				{
					for window in osd_app.windows.borrow().to_owned() {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
			}
			(ArgTypes::CapsLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if value >= 0 && value <= 1 => value == 1,
					_ => get_key_lock_state(KeysLocks::CapsLock, value),
				};
				for window in osd_app.windows.borrow().to_owned() {
					window.changed_keylock(KeysLocks::CapsLock, state)
				}
			}
			(ArgTypes::NumLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if value >= 0 && value <= 1 => value == 1,
					_ => get_key_lock_state(KeysLocks::NumLock, value),
				};
				for window in osd_app.windows.borrow().to_owned() {
					window.changed_keylock(KeysLocks::NumLock, state)
				}
			}
			(ArgTypes::ScrollLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if value >= 0 && value <= 1 => value == 1,
					_ => get_key_lock_state(KeysLocks::ScrollLock, value),
				};
				for window in osd_app.windows.borrow().to_owned() {
					window.changed_keylock(KeysLocks::ScrollLock, state)
				}
			}
			(ArgTypes::MaxVolume, max) => {
				let volume: u8 = match max {
					Some(max) => match max.parse() {
						Ok(max) => max,
						_ => get_default_max_volume(),
					},
					_ => get_default_max_volume(),
				};
				set_max_volume(volume)
			}
			(ArgTypes::DeviceName, name) => {
				set_device_name(name.unwrap_or(DEVICE_NAME_DEFAULT.to_string()))
			}
			(arg_type, data) => {
				eprintln!(
					"Failed to parse command... Type: {:?}, Data: {:?}",
					arg_type, data
				)
			}
		};
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
