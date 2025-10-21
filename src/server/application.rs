use crate::argtypes::ArgTypes;
use crate::config::{self, APPLICATION_NAME, DBUS_BACKEND_NAME};
use crate::global_utils::{handle_application_args, HandleLocalStatus};
use crate::osd_window::SwayosdWindow;
use crate::playerctl::*;
use crate::utils::{self, *};
use async_channel::{Receiver, Sender};
use gtk::gio::{DBusConnection, ListModel};
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
			Char::from(b's'),
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
					(status @ HandleLocalStatus::FAILURE, _) => return status.as_return_code(),
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

				HandleLocalStatus::CONTINUE.as_return_code()
			}
		));

		// Listen for any actions sent from swayosd-client
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
		let (sender, receiver) = async_channel::bounded::<(u16, i32)>(1);
		// Listen to the LibInput Backend and activate the Application action
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
				move |connection, _, _| Self::libinput_backend_appeared(
					&sender, &signal_id, connection
				)
			),
			clone!(
				#[strong]
				signal_id,
				move |connection, _| Self::libinput_backend_vanished(&signal_id, connection)
			),
		);

		osd_app
	}

	fn libinput_backend_appeared(
		sender: &Sender<(u16, i32)>,
		signal_id: &Arc<Mutex<Option<SignalSubscriptionId>>>,
		connection: DBusConnection,
	) {
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
									if let Err(error) = sender.send((key_code, state)).await {
										eprintln!("Channel Send error: {}", error);
									}
								}
							));
						}
						variables => return eprintln!("Variables don't match: {:?}", variables),
					};
				}
			),
		));
	}

	fn libinput_backend_vanished(
		signal_id: &Arc<Mutex<Option<SignalSubscriptionId>>>,
		connection: DBusConnection,
	) {
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

	pub fn start(&self) -> i32 {
		let osd_app = self.clone();
		self.app.connect_activate(move |_| {
			osd_app.initialize();
		});

		self.app
			.register(gio::Cancellable::NONE)
			.expect("Could not register swayosd-server");
		if self.app.is_remote() {
			eprintln!("An instance of SwayOSD is already running!\n");
			std::process::exit(1);
		}
		self.app.run().into()
	}

	fn initialize(&self) {
		let display: gdk::Display = gdk::Display::default().expect("Could not get GDK Display!");
		let monitors = display.monitors();

		let osd_app = self.clone();
		monitors.connect_items_changed(clone!(
			#[strong]
			osd_app,
			move |monitors, position, removed, added| osd_app
				.monitors_changed(monitors, position, removed, added)
		));
		osd_app.monitors_changed(&monitors, 0, 0, monitors.n_items());
	}

	fn monitors_changed(&self, monitors: &ListModel, position: u32, removed: u32, added: u32) {
		let mut windows = self.windows.borrow_mut();

		for i in 0..removed {
			let window = windows.remove((position + i) as usize);
			window.close();
		}

		for i in 0..added {
			if let Some(monitor) = monitors
				.item(position + i)
				.and_then(|obj| obj.downcast::<gdk::Monitor>().ok())
			{
				let window = SwayosdWindow::new(&self.app, &monitor);
				windows.push(window);
			}
		}
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
		}

		selected_windows
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
					Ok(value) if (0..=1).contains(&value) => value == 1,
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
					Ok(value) if (0..=1).contains(&value) => value == 1,
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
					Ok(value) if (0..=1).contains(&value) => value == 1,
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
							let (icon, label) = (player.icon.unwrap_or_default(), &player.label);
							for window in Self::choose_windows(osd_app) {
								window.changed_player(&icon, label.as_deref())
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
			(ArgTypes::CustomProgress, fraction) => {
				if let Some(fraction) = fraction {
					let fraction: f64 = fraction.parse::<f64>().unwrap_or(1.0);
					for window in Self::choose_windows(osd_app) {
						window.custom_progress(
							fraction,
							get_progress_text(),
							get_icon_name().as_deref(),
						);
					}
				}
				reset_progress_text();
				reset_icon_name();
				reset_monitor_name();
			}
			(ArgTypes::CustomProgressText, text) => set_progress_text(text),
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
}
