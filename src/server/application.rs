use crate::argtypes::ArgTypes;
use crate::config::{self, APPLICATION_NAME, DBUS_BACKEND_NAME};
use crate::global_utils::{handle_application_args, HandleLocalStatus};
use crate::osd_window::SwayosdWindow;
use crate::playerctl::*;
use crate::utils::{self, *};
use async_channel::Receiver;
use gtk::{
	gdk,
	gio::{
		self, ApplicationFlags, BusNameWatcherFlags, BusType, DBusSignalFlags, SignalSubscriptionId,
	},
	glib::{
		clone, variant::ToVariant, Char, ControlFlow::Break, MainContext, OptionArg, OptionFlags,
	},
	prelude::*,
	Application,
};
use pulsectl::controllers::{SinkController, SourceController};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::config::user::ServerConfig;

#[derive(Clone, Shrinkwrap)]
pub struct SwayOSDApplication {
	#[shrinkwrap(main_field)]
	app: gtk::Application,
	windows: Rc<RefCell<Vec<SwayosdWindow>>>,
	_hold: Rc<gio::ApplicationHoldGuard>,
}

impl SwayOSDApplication {
	pub fn new(
		server_config: Arc<ServerConfig>,
		action_receiver: Receiver<(ArgTypes, String)>,
	) -> Self {
		let app = Application::new(Some(APPLICATION_NAME), ApplicationFlags::FLAGS_NONE);
		let hold = Rc::new(app.hold());

		app.add_main_option(
			"config",
			Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			"Use a custom config file instead of looking for one.",
			Some("<CONFIG FILE PATH>"),
		);

		app.add_main_option(
			"style",
			Char::from('s' as u8),
			OptionFlags::NONE,
			OptionArg::String,
			"Use a custom Stylesheet file instead of looking for one",
			Some("<CSS FILE PATH>"),
		);

		app.add_main_option(
			"top-margin",
			Char::from(0),
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
			_hold: hold,
		};

		// Apply Server Config
		if let Some(margin) = server_config.top_margin {
			if (0_f32..1_f32).contains(&margin) {
				set_top_margin(margin);
			}
		}
		if let Some(max_volume) = server_config.max_volume {
			set_default_max_volume(max_volume);
		}
		if let Some(show) = server_config.show_percentage {
			set_show_percentage(show);
		}

		let server_config_shared = server_config.clone();

		// Parse args
		app.connect_handle_local_options(clone!(
			#[strong]
			osd_app,
			move |_app, args| {
				let actions = match handle_application_args(args.to_variant()) {
					(HandleLocalStatus::SUCCESS | HandleLocalStatus::CONTINUE, actions) => actions,
					(status @ HandleLocalStatus::FAILURE, _) => return status as i32,
				};
				for (arg_type, data) in actions {
					match (arg_type, data) {
						(ArgTypes::TopMargin, margin) => {
							let margin: Option<f32> = margin
								.and_then(|margin| margin.parse().ok())
								.and_then(|margin| {
									(0_f32..1_f32).contains(&margin).then_some(margin)
								});

							if let Some(margin) = margin {
								set_top_margin(margin)
							}
						}
						(ArgTypes::MaxVolume, max) => {
							let max: Option<u8> = max.and_then(|max| max.parse().ok());

							if let Some(max) = max {
								set_default_max_volume(max);
							}
						}
						(arg_type, data) => Self::action_activated(
							&osd_app,
							server_config_shared.clone(),
							arg_type,
							data,
						),
					}
				}

				HandleLocalStatus::CONTINUE as i32
			}
		));

		let server_config_shared = server_config.clone();

		MainContext::default().spawn_local(clone!(
			#[strong]
			osd_app,
			async move {
				while let Ok((arg_type, data)) = action_receiver.recv().await {
					Self::action_activated(
						&osd_app,
						server_config_shared.clone(),
						arg_type,
						(!data.is_empty()).then_some(data),
					);
				}
				Break
			}
		));

		let server_config_shared = server_config.clone();

		// Listen to the LibInput Backend and activate the Application action
		let (sender, receiver) = async_channel::bounded::<(u16, i32)>(1);
		MainContext::default().spawn_local(clone!(
			#[strong]
			osd_app,
			async move {
				while let Ok((key_code, state)) = receiver.recv().await {
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
							_ => continue,
						};
					Self::action_activated(&osd_app, server_config_shared.clone(), arg_type, data);
				}
				Break
			}
		));
		// Start watching for the LibInput Backend
		let signal_id: Arc<Mutex<Option<SignalSubscriptionId>>> = Arc::new(Mutex::new(None));
		gio::bus_watch_name(
			BusType::System,
			DBUS_BACKEND_NAME,
			BusNameWatcherFlags::NONE,
			clone!(
				#[strong]
				sender,
				#[strong]
				signal_id,
				move |connection, _, _| {
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
						DBusSignalFlags::NONE,
						clone!(
							#[strong]
							sender,
							move |_, _, _, _, _, variant| {
								let key_code = variant.try_child_get::<u16>(0);
								let state = variant.try_child_get::<i32>(1);
								match (key_code, state) {
									(Ok(Some(key_code)), Ok(Some(state))) => {
										MainContext::default().spawn_local(clone!(
											#[strong]
											sender,
											async move {
												if let Err(error) =
													sender.send((key_code, state)).await
												{
													eprintln!("Channel Send error: {}", error);
												}
											}
										));
									}
									variables => {
										return eprintln!("Variables don't match: {:?}", variables)
									}
								};
							}
						),
					));
				}
			),
			clone!(
				#[strong]
				signal_id,
				move |connection, _| {
					eprintln!("SwayOSD LibInput Backend isn't available, waiting...");
					match signal_id.lock() {
						Ok(mut mutex) => {
							if let Some(sig_id) = mutex.take() {
								connection.signal_unsubscribe(sig_id);
							}
						}
						Err(error) => println!("Mutex lock Error: {}", error),
					}
				}
			),
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

	fn choose_windows(osd_app: &SwayOSDApplication) -> Vec<SwayosdWindow> {
		let mut selected_windows = Vec::new();

		match get_monitor_name() {
			Some(monitor_name) => {
				for window in osd_app.windows.borrow().to_owned() {
					if let Some(monitor_connector) = window.monitor.connector() {
						if monitor_name == monitor_connector {
							selected_windows.push(window);
						}
					}
				}
			}
			None => return osd_app.windows.borrow().to_owned(),
		}

		if selected_windows.is_empty() {
			eprintln!("Specified monitor name, but found no matching output");
			return osd_app.windows.borrow().to_owned();
		} else {
			return selected_windows;
		}
	}

	fn action_activated(
		osd_app: &SwayOSDApplication,
		server_config: Arc<ServerConfig>,
		arg_type: ArgTypes,
		value: Option<String>,
	) {
		match (arg_type, value) {
			(ArgTypes::SinkVolumeRaise, step) => {
				let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Raise, step)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
				reset_monitor_name();
			}
			(ArgTypes::SinkVolumeLower, step) => {
				let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Lower, step)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
				reset_monitor_name();
			}
			(ArgTypes::SinkVolumeMuteToggle, _) => {
				let mut device_type = VolumeDeviceType::Sink(SinkController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::MuteToggle, None)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
				reset_monitor_name();
			}
			(ArgTypes::SourceVolumeRaise, step) => {
				let mut device_type = VolumeDeviceType::Source(SourceController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Raise, step)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
				reset_monitor_name();
			}
			(ArgTypes::SourceVolumeLower, step) => {
				let mut device_type = VolumeDeviceType::Source(SourceController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::Lower, step)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
				reset_monitor_name();
			}
			(ArgTypes::SourceVolumeMuteToggle, _) => {
				let mut device_type = VolumeDeviceType::Source(SourceController::create().unwrap());
				if let Some(device) =
					change_device_volume(&mut device_type, VolumeChangeType::MuteToggle, None)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_volume(&device, &device_type);
					}
				}
				reset_max_volume();
				reset_device_name();
				reset_monitor_name();
			}
			// TODO: Brightness
			(ArgTypes::BrightnessRaise, step) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Raise, step)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
				reset_monitor_name();
			}
			(ArgTypes::BrightnessLower, step) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Lower, step)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
				reset_monitor_name();
			}
			(ArgTypes::BrightnessSet, value) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Set, value)
				{
					for window in Self::choose_windows(osd_app) {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
				reset_monitor_name();
			}
			(ArgTypes::CapsLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if value >= 0 && value <= 1 => value == 1,
					_ => get_key_lock_state(KeysLocks::CapsLock, value),
				};
				for window in Self::choose_windows(osd_app) {
					window.changed_keylock(KeysLocks::CapsLock, state)
				}
				reset_monitor_name();
			}
			(ArgTypes::NumLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if value >= 0 && value <= 1 => value == 1,
					_ => get_key_lock_state(KeysLocks::NumLock, value),
				};
				for window in Self::choose_windows(osd_app) {
					window.changed_keylock(KeysLocks::NumLock, state)
				}
				reset_monitor_name();
			}
			(ArgTypes::ScrollLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if value >= 0 && value <= 1 => value == 1,
					_ => get_key_lock_state(KeysLocks::ScrollLock, value),
				};
				for window in Self::choose_windows(osd_app) {
					window.changed_keylock(KeysLocks::ScrollLock, state)
				}
				reset_monitor_name();
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
			(ArgTypes::Player, name) => set_player(name.unwrap_or("".to_string())),
			(ArgTypes::Playerctl, value) => {
				let value = &value.unwrap_or("".to_string());

				let action = PlayerctlAction::from(value).unwrap();
				if let Ok(mut player) = Playerctl::new(action, server_config) {
					match player.run() {
						Ok(_) => {
							let (icon, label) = (player.icon.unwrap(), player.label.unwrap());
							for window in Self::choose_windows(osd_app) {
								window.changed_player(&icon, &label)
							}
							reset_monitor_name();
						}
						Err(x) => {
							eprintln!("couldn't run player change: \"{:?}\"!", x)
						}
					}
				} else {
					eprintln!("Unable to get players! are any opened?")
				}

				reset_player();
			}
			(ArgTypes::DeviceName, name) => {
				set_device_name(name.unwrap_or(DEVICE_NAME_DEFAULT.to_string()))
			}
			(ArgTypes::MonitorName, name) => {
				if let Some(name) = name {
					set_monitor_name(name)
				}
			}
			(ArgTypes::CustomMessage, message) => {
				if let Some(message) = message {
					for window in Self::choose_windows(osd_app) {
						window.custom_message(message.as_str(), get_icon_name().as_deref());
					}
				}
				reset_icon_name();
				reset_monitor_name();
			}
			(ArgTypes::CustomIcon, icon) => {
				set_icon_name(icon.unwrap_or(ICON_NAME_DEFAULT.to_string()))
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

		display.connect_opened(clone!(
			#[strong]
			_self,
			move |d| {
				_self.init_windows(d);
			}
		));

		display.connect_closed(clone!(
			#[strong]
			_self,
			move |_d, is_error| {
				if is_error {
					eprintln!("Display closed due to errors...");
				}
				_self.close_all_windows();
			}
		));

		display.monitors().connect_items_changed(clone!(
			#[strong]
			_self,
			move |monitors, position, removed, added| {
				if removed != 0 {
					_self.init_windows(&display);
				} else if added != 0 {
					for i in 0..added {
						if let Some(mon) = monitors
							.item(position + i)
							.and_then(|obj| obj.downcast::<gdk::Monitor>().ok())
						{
							_self.add_window(&display, &mon);
						}
					}
				}
			}
		));
	}

	fn add_window(&self, display: &gdk::Display, monitor: &gdk::Monitor) {
		let win = SwayosdWindow::new(&self.app, display, monitor);
		self.windows.borrow_mut().push(win);
	}

	fn init_windows(&self, display: &gdk::Display) {
		self.close_all_windows();

		let monitors = display.monitors();
		for i in 0..monitors.n_items() {
			let monitor = match monitors
				.item(i)
				.and_then(|obj| obj.downcast::<gdk::Monitor>().ok())
			{
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
