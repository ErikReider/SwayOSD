use crate::args::ArgsServer;
use crate::argtypes::ArgTypes;
use crate::config::{self, APPLICATION_NAME, DBUS_BACKEND_NAME};
use crate::global_utils::segmented_progress_parser;
use crate::osd_window::SwayosdWindow;
use crate::utils::{self, *};
use crate::{login1, playerctl::*, upower};
use async_channel::{Receiver, Sender};
use async_std::stream::StreamExt;
use gtk::gio::{DBusConnection, ListModel};
use gtk::glib::ControlFlow;
use gtk::{
	gdk,
	gio::{
		self, ApplicationFlags, BusNameWatcherFlags, BusType, DBusSignalFlags, SignalSubscriptionId,
	},
	glib::{clone, Char, ControlFlow::Break, MainContext, OptionArg, OptionFlags},
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
	activated: Rc<RefCell<bool>>,
	_hold: Rc<gio::ApplicationHoldGuard>,
}

impl SwayOSDApplication {
	pub fn new(
		server_config: Arc<ServerConfig>,
		args: Arc<ArgsServer>,
		action_receiver: Receiver<(ArgTypes, String)>,
	) -> Self {
		let app = Application::new(Some(APPLICATION_NAME), ApplicationFlags::FLAGS_NONE);
		let hold = Rc::new(app.hold());

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
			activated: Rc::new(RefCell::new(false)),
			_hold: hold,
		};

		// Apply Server Config
		if let Some(margin) = server_config.top_margin
			&& (0_f32..1_f32).contains(&margin)
		{
			set_top_margin(margin);
		}
		if let Some(max_volume) = server_config.max_volume {
			set_default_max_volume(max_volume);
			reset_max_volume();
		}
		if let Some(min_brightness) = server_config.min_brightness {
			set_default_min_brightness(min_brightness);
			reset_min_brightness();
		}
		if let Some(show) = server_config.show_percentage {
			set_show_percentage(show);
		}

		Self::parse_args(&args);

		// Listen for any actions sent from swayosd-client
		MainContext::default().spawn_local(clone!(
			#[strong]
			osd_app,
			#[strong]
			server_config,
			async move {
				while let Ok((arg_type, data)) = action_receiver.recv().await {
					osd_app.action_activated(
						server_config.clone(),
						arg_type,
						(!data.is_empty()).then_some(data),
					);
				}
				Break
			}
		));

		// Listen for UPower keyboard backlight changes
		if server_config.keyboard_backlight.unwrap_or(true) {
			MainContext::default().spawn_local(clone!(
				#[strong]
				osd_app,
				#[strong]
				server_config,
				async move { osd_app.listen_to_upower_kbd_backlight(&server_config).await }
			));
		}

		let (sender, receiver) = async_channel::bounded::<(u16, i32)>(1);
		// Listen to the LibInput Backend and activate the Application action
		MainContext::default().spawn_local(clone!(
			#[strong]
			osd_app,
			#[strong]
			server_config,
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
					osd_app.action_activated(server_config.clone(), arg_type, data);
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

	fn parse_args(args: &ArgsServer) {
		// Top Margin
		if let Some(value) = args.top_margin.to_owned() {
			match value.parse::<f32>() {
				Ok(top_margin @ 0.0f32..=1.0f32) => {
					set_top_margin(top_margin);
				}
				_ => {
					eprintln!("{} is not a number between 0.0 and 1.0!", value);
				}
			}
		}
	}

	async fn listen_to_upower_kbd_backlight(
		&self,
		server_config: &Arc<ServerConfig>,
	) -> zbus::Result<ControlFlow> {
		// TODO: Support other UPower KdbBacklights with version 1.91
		let proxy = upower::KbdBacklight::init().await?;
		let max_brightness = proxy.get_max_brightness().await?;
		let mut changed_stream = proxy.receive_brightness_changed_with_source().await?;
		while let Some(msg) = changed_stream.next().await {
			if let Ok(args) = msg.args() {
				if args.source != "internal" {
					// Only display the OSD if the hardware changed the keyboard brightness itself
					// (automatically or through a firmware-handled hotkey being pressed)
					continue;
				}
				self.action_activated(
					server_config.clone(),
					ArgTypes::KbdBacklight,
					Some(format!("{}:{}", args.value, max_brightness)),
				);
			} else {
				eprintln!("UPower args aren't valid {:?}", msg.args());
			}
		}
		eprintln!("UPower stream ended unexpectedly");
		zbus::Result::Ok(Break)
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

	async fn listen_to_prepare_for_sleep(&self) -> zbus::Result<ControlFlow> {
		let proxy = login1::Login1::init().await?;
		let mut changed_stream = proxy.receive_prepare_for_sleep().await?;
		while let Some(msg) = changed_stream.next().await {
			if let Ok(args) = msg.args() {
				if !args.value {
					// Re-add all windows
					let windows_len: u32 = {
						let windows = self.windows.borrow();
						windows.len() as u32
					};
					let display: gdk::Display =
						gdk::Display::default().expect("Could not get GDK Display!");
					let monitors = display.monitors();
					self.monitors_changed(&monitors, 0, windows_len, monitors.n_items());
				}
			} else {
				eprintln!("Login1 args aren't valid {:?}", msg.args());
			}
		}
		eprintln!("Login1 stream ended unexpectedly");
		zbus::Result::Ok(Break)
	}

	pub fn start(&self) -> i32 {
		let osd_app = self.clone();
		self.app.connect_activate(move |_| {
			if let Ok(mut is_activated) = osd_app.activated.try_borrow_mut() {
				if *is_activated {
					return;
				}
				*is_activated = true;
				osd_app.initialize();
			}
		});

		self.app
			.register(gio::Cancellable::NONE)
			.expect("Could not register swayosd-server");
		if self.app.is_remote() {
			eprintln!("An instance of SwayOSD is already running!\n");
			std::process::exit(1);
		}
		let empty_args: Vec<String> = vec![];
		self.app.run_with_args(&empty_args).into()
	}

	fn initialize(&self) {
		let display: gdk::Display = gdk::Display::default().expect("Could not get GDK Display!");
		let monitors = display.monitors();

		let osd_app = self.clone();

		// Refresh the created windows if a monitor got unplugged when suspended
		MainContext::default().spawn_local(clone!(
			#[strong]
			osd_app,
			async move {
				let result = osd_app.listen_to_prepare_for_sleep().await;
				eprintln!("Login1 signal ended unexpectedly: {:?}", result);
				result
			}
		));

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

		for _ in 0..removed {
			let window = windows.remove(position as usize);
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

	fn choose_windows(&self) -> Vec<SwayosdWindow> {
		let mut selected_windows = Vec::new();

		match get_monitor_name() {
			Some(monitor_name) => {
				for window in self.windows.borrow().to_owned() {
					if let Some(monitor_connector) = window.monitor.connector()
						&& monitor_name == monitor_connector
					{
						selected_windows.push(window);
					}
				}
			}
			None => return self.windows.borrow().to_owned(),
		}

		if selected_windows.is_empty() {
			eprintln!("Specified monitor name, but found no matching output");
			return self.windows.borrow().to_owned();
		}

		selected_windows
	}

	fn action_activated(
		&self,
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
					for window in self.choose_windows() {
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
					for window in self.choose_windows() {
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
					for window in self.choose_windows() {
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
					for window in self.choose_windows() {
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
					for window in self.choose_windows() {
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
					for window in self.choose_windows() {
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
					for window in self.choose_windows() {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
				reset_min_brightness();
				reset_monitor_name();
			}
			(ArgTypes::BrightnessLower, step) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Lower, step)
				{
					for window in self.choose_windows() {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
				reset_min_brightness();
				reset_monitor_name();
			}
			(ArgTypes::BrightnessSet, value) => {
				if let Ok(mut brightness_backend) =
					change_brightness(BrightnessChangeType::Set, value)
				{
					for window in self.choose_windows() {
						window.changed_brightness(brightness_backend.as_mut());
					}
				}
				reset_min_brightness();
				reset_monitor_name();
			}
			(ArgTypes::CapsLock, value) => {
				let i32_value = value.clone().unwrap_or("-1".to_owned());
				let state = match i32_value.parse::<i32>() {
					Ok(value) if (0..=1).contains(&value) => value == 1,
					_ => get_key_lock_state(KeysLocks::CapsLock, value),
				};
				for window in self.choose_windows() {
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
				for window in self.choose_windows() {
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
				for window in self.choose_windows() {
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
			(ArgTypes::MinBrightness, min) => {
				let brightness: u32 = match min {
					Some(min) => match min.parse() {
						Ok(min) => min,
						_ => get_default_min_brightness(),
					},
					_ => get_default_min_brightness(),
				};
				set_min_brightness(brightness)
			}
			(ArgTypes::Player, name) => set_player(name.unwrap_or("".to_string())),
			(ArgTypes::Playerctl, value) => {
				let value = &value.unwrap_or("".to_string());

				let action = PlayerctlAction::from(value).unwrap();
				if let Ok(mut player) = Playerctl::new(action, server_config) {
					match player.run() {
						Ok(_) => {
							let (icon, label) = (player.icon.unwrap_or_default(), &player.label);
							for window in self.choose_windows() {
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
			(ArgTypes::KbdBacklight, values) => {
				if let Some(values) = values
					&& let Ok((value, n_segments)) = segmented_progress_parser(&values)
				{
					for window in self.choose_windows() {
						window.changed_kbd_backlight(value, n_segments);
					}
				}
				reset_monitor_name();
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
					for window in self.choose_windows() {
						window.custom_message(message.as_str(), get_icon_name().as_deref());
					}
				}
				reset_icon_name();
				reset_monitor_name();
			}
			(ArgTypes::CustomProgress, fraction) => {
				if let Some(fraction) = fraction {
					let fraction: f64 = fraction.parse::<f64>().unwrap_or(1.0);
					for window in self.choose_windows() {
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
			(ArgTypes::CustomSegmentedProgress, values) => {
				if let Some(values) = values
					&& let Ok((value, n_segments)) = segmented_progress_parser(&values)
				{
					for window in self.choose_windows() {
						window.custom_segmented_progress(
							value,
							n_segments,
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
