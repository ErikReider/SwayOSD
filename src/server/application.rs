use async_channel::{Receiver, Sender};
use async_std::stream::StreamExt;
use gtk::{
	gdk,
	gio::{
		self, ApplicationFlags, BusNameWatcherFlags, BusType, DBusConnection, DBusSignalFlags,
		ListModel, SignalSubscriptionId,
	},
	glib::{clone, Char, ControlFlow, ControlFlow::Break, MainContext, OptionArg, OptionFlags},
	prelude::*,
	Application,
};
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::actions::mpris::{Playerctl, PlayerctlAction, PlayerctlDeviceRaw};
use crate::actions::pulse::{DeviceKind, VolumeController};
use crate::argflags::ArgFlags;
use crate::args::ArgsServer;
use crate::argtypes::ArgTypes;
use crate::config::{self, user::ServerConfig, APPLICATION_NAME, DBUS_BACKEND_NAME};
use crate::global_utils;
use crate::osd_window::SwayosdWindow;
use crate::utils::*;
use crate::{login1, upower, DbusSenderFlagsType, DbusSenderType};

#[derive(Clone)]
pub struct ActionOptions {
	pub max_volume: ActionField<u8>,
	pub min_brightness: ActionField<u32>,
	pub device_name: ActionOptionalField<String>,
	pub monitor_name: ActionOptionalField<String>,
	pub icon_name: ActionOptionalField<String>,
	pub progress_text: ActionOptionalField<String>,
	pub player_name: ActionOptionalField<PlayerctlDeviceRaw>,
	pub top_margin: ActionField<f32>,
	pub duration: ActionField<u64>,
	pub show_percentage: ActionField<bool>,
}

impl ActionOptions {
	pub fn new() -> Self {
		Self {
			max_volume: ActionField::new(100_u8),
			min_brightness: ActionField::new(5_u32),
			device_name: ActionOptionalField::new(None),
			monitor_name: ActionOptionalField::new(None),
			icon_name: ActionOptionalField::new(None),
			progress_text: ActionOptionalField::new(None),
			player_name: ActionOptionalField::new(None),
			top_margin: ActionField::new(0.85_f32),
			duration: ActionField::new(1000),
			show_percentage: ActionField::new(false),
		}
	}
}

#[derive(Clone, Shrinkwrap)]
pub struct SwayOSDApplication {
	#[shrinkwrap(main_field)]
	app: gtk::Application,
	windows: Rc<RefCell<Vec<SwayosdWindow>>>,
	activated: Rc<RefCell<bool>>,
	_hold: Rc<gio::ApplicationHoldGuard>,
	action_options: Rc<ActionOptions>,

	volume_ctrl: Rc<RefCell<Option<VolumeController>>>,
}

/// Iterate the "correct" monitors
macro_rules! iter_windows {
	($self:expr, $action_options:expr, ($window:ident), $do:block) => {{
		let monitor_name = $action_options.monitor_name.get();

		if let Some(monitor_name) = monitor_name {
			let mut found = false;
			for $window in $self.windows.borrow().to_owned() {
				if $window
					.monitor
					.connector()
					.is_some_and(|c| c == *monitor_name)
				{
					found = true;
					$do
				}
			}
			if !found {
				eprintln!("Specified monitor name, but found no matching output");
			}
		} else {
			for $window in $self.windows.borrow().to_owned() {
				$do
			}
		}
	}};
}

impl SwayOSDApplication {
	pub fn new(
		server_config: Arc<ServerConfig>,
		args: Arc<ArgsServer>,
		action_receiver: Receiver<DbusSenderType>,
	) -> Self {
		let app = Application::new(Some(APPLICATION_NAME), ApplicationFlags::FLAGS_NONE);
		let hold = Rc::new(app.hold());

		let mut action_options = ActionOptions::new();

		app.add_main_option(
			"top-margin",
			Char::from(0),
			OptionFlags::NONE,
			OptionArg::String,
			&format!(
				"OSD margin from top edge (0.5 would be screen center). Default is {}",
				action_options.top_margin.get_default()
			),
			Some("<from 0.0 to 1.0>"),
		);

		// Apply Server Config
		if let Some(margin) = server_config.top_margin
			&& (0_f32..1_f32).contains(&margin)
		{
			action_options.top_margin.set_default(margin);
		}
		if let Some(max_volume) = server_config.max_volume {
			action_options.max_volume.set_default(max_volume);
		}
		if let Some(min_brightness) = server_config.min_brightness {
			action_options.min_brightness.set_default(min_brightness)
		}
		if let Some(show_percentage) = server_config.show_percentage {
			action_options.show_percentage.set_default(show_percentage);
		}
		if let Some(duration) = server_config.duration {
			action_options.duration.set_default(duration);
		}

		Self::parse_args(&args, &mut action_options);

		let osd_app = SwayOSDApplication {
			app: app.clone(),
			windows: Rc::new(RefCell::new(Vec::new())),
			activated: Rc::new(RefCell::new(false)),
			_hold: hold,
			action_options: Rc::new(action_options),

			volume_ctrl: Rc::new(RefCell::new(None)),
		};

		// Listen for any actions sent from swayosd-client
		MainContext::default().spawn_local(clone!(
			#[strong]
			osd_app,
			#[strong]
			server_config,
			async move {
				while let Ok((arg_type, data, flags)) = action_receiver.recv().await {
					if let Err(error) =
						osd_app.action_activated(server_config.clone(), arg_type, data, Some(flags))
					{
						eprintln!("Could not activate action: {:?}", error)
					}
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
					if let Err(error) =
						osd_app.action_activated(server_config.clone(), arg_type, data, None)
					{
						eprintln!("Could not activate action: {:?}", error)
					}
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

	fn parse_args(args: &ArgsServer, action_options: &mut ActionOptions) {
		// Top Margin
		if let Some(top_margin) = args.top_margin {
			action_options.top_margin.set_default(top_margin);
		}

		// Duration
		if let Some(duration) = args.duration {
			action_options.duration.set_default(duration);
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
				if let Err(error) = self.action_activated(
					server_config.clone(),
					ArgTypes::KbdBacklight,
					Some(format!("{}:{}", args.value, max_brightness)),
					None,
				) {
					eprintln!("Could not activate action: {:?}", error)
				}
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

		let top_margin = self.action_options.top_margin.get();

		for i in 0..added {
			if let Some(monitor) = monitors
				.item(position + i)
				.and_then(|obj| obj.downcast::<gdk::Monitor>().ok())
			{
				let window = SwayosdWindow::new(&self.app, &monitor, top_margin);
				windows.push(window);
			}
		}
	}

	fn adjust_volume(
		&self,
		action_options: &ActionOptions,
		kind: DeviceKind,
		change_type: VolumeChangeType,
		step: Option<String>,
	) -> Result<(), Box<dyn Error>> {
		let max_volume: f64 = (*action_options.max_volume.get()).into();
		let device_name = action_options.device_name.get();

		let mut ctrl = self.volume_ctrl.try_borrow_mut()?;
		let ctrl = ctrl.get_or_insert(VolumeController::create()?);

		if let Some(device) =
			change_device_volume(ctrl, kind, change_type, device_name, max_volume, step)
		{
			iter_windows!(self, action_options, (window), {
				window.changed_volume(action_options, &device);
			});
		}
		Ok(())
	}

	fn adjust_brightness(
		&self,
		action_options: &ActionOptions,
		change_type: BrightnessChangeType,
		step: Option<String>,
	) -> Result<(), Box<dyn Error>> {
		let min_brightness = action_options.min_brightness.get();
		let device_name = action_options.device_name.get();

		let mut brightness_backend =
			change_brightness(change_type, device_name, *min_brightness, step)?;
		iter_windows!(self, action_options, (window), {
			window.changed_brightness(action_options, brightness_backend.as_mut());
		});
		Ok(())
	}

	fn adjust_keylock(
		&self,
		action_options: &ActionOptions,
		keylock_type: KeysLocks,
		value: Option<String>,
	) -> Result<(), Box<dyn Error>> {
		let i32_value = value.clone().unwrap_or("-1".to_owned());
		let state = match i32_value.parse::<i32>() {
			Ok(value) if (0..=1).contains(&value) => value == 1,
			_ => get_key_lock_state(keylock_type, value),
		};
		iter_windows!(self, action_options, (window), {
			window.changed_keylock(action_options, keylock_type, state)
		});
		Ok(())
	}

	fn action_activated(
		&self,
		server_config: Arc<ServerConfig>,
		arg_type: ArgTypes,
		value: Option<String>,
		flags: Option<DbusSenderFlagsType>,
	) -> Result<(), Box<dyn Error>> {
		let mut action_options: ActionOptions = (*self.action_options).clone();

		// Parse flags
		for (flag, value) in flags.unwrap_or_default() {
			match (flag, value) {
				(ArgFlags::MaxVolume, max) => {
					let volume: Option<u8> = max.and_then(|max| max.parse().ok());
					action_options.max_volume.set(volume);
				}
				(ArgFlags::MinBrightness, min) => {
					let brightness: Option<u32> = min.and_then(|min| min.parse().ok());
					action_options.min_brightness.set(brightness);
				}
				(ArgFlags::Player, name) => {
					action_options
						.player_name
						.set(PlayerctlDeviceRaw::from(name));
				}
				(ArgFlags::DeviceName, name) => {
					action_options.device_name.set(name);
				}
				(ArgFlags::MonitorName, name) => {
					action_options.monitor_name.set(name);
				}
				(ArgFlags::CustomProgressText, text) => {
					action_options.progress_text.set(text);
				}
				(ArgFlags::CustomIcon, icon) => {
					action_options.icon_name.set(icon);
				}
				(ArgFlags::Duration, duration) => {
					let duration: Option<u64> = duration.and_then(|d| d.parse().ok());
					action_options.duration.set(duration);
				}
			};
		}

		// Execute the action
		match (arg_type, value) {
			// Pulse Sink
			(ArgTypes::SinkVolumeRaise, step) => self.adjust_volume(
				&action_options,
				DeviceKind::Sink,
				VolumeChangeType::Raise,
				step,
			)?,
			(ArgTypes::SinkVolumeLower, step) => self.adjust_volume(
				&action_options,
				DeviceKind::Sink,
				VolumeChangeType::Lower,
				step,
			)?,
			(ArgTypes::SinkVolumeMuteToggle, _) => self.adjust_volume(
				&action_options,
				DeviceKind::Sink,
				VolumeChangeType::MuteToggle,
				None,
			)?,

			// Pulse Source
			(ArgTypes::SourceVolumeRaise, step) => self.adjust_volume(
				&action_options,
				DeviceKind::Source,
				VolumeChangeType::Raise,
				step,
			)?,
			(ArgTypes::SourceVolumeLower, step) => self.adjust_volume(
				&action_options,
				DeviceKind::Source,
				VolumeChangeType::Lower,
				step,
			)?,
			(ArgTypes::SourceVolumeMuteToggle, _) => self.adjust_volume(
				&action_options,
				DeviceKind::Source,
				VolumeChangeType::MuteToggle,
				None,
			)?,

			// Brightness
			(ArgTypes::BrightnessRaise, step) => {
				self.adjust_brightness(&action_options, BrightnessChangeType::Raise, step)?
			}
			(ArgTypes::BrightnessLower, step) => {
				self.adjust_brightness(&action_options, BrightnessChangeType::Lower, step)?
			}
			(ArgTypes::BrightnessSet, value) => {
				self.adjust_brightness(&action_options, BrightnessChangeType::Set, value)?
			}

			// Keystates
			(ArgTypes::CapsLock, value) => {
				self.adjust_keylock(&action_options, KeysLocks::CapsLock, value)?
			}
			(ArgTypes::NumLock, value) => {
				self.adjust_keylock(&action_options, KeysLocks::NumLock, value)?
			}
			(ArgTypes::ScrollLock, value) => {
				self.adjust_keylock(&action_options, KeysLocks::ScrollLock, value)?
			}

			// Playerctrl
			(ArgTypes::Playerctl, value) => {
				let player_name = action_options.player_name.get();

				let value = &value.unwrap_or("".to_string());
				let action = PlayerctlAction::from(value)?;
				if let Ok(mut player) = Playerctl::new(action, player_name.clone(), server_config) {
					match player.run() {
						Ok(_) => {
							let (icon, label) = (player.icon.unwrap_or_default(), &player.label);
							iter_windows!(self, action_options, (window), {
								window.changed_player(&action_options, &icon, label)
							});
						}
						Err(x) => {
							eprintln!("couldn't run player change: \"{:?}\"!", x)
						}
					}
				} else {
					eprintln!("Unable to get players! are any opened?")
				}
			}

			// Keyboard backlight
			(ArgTypes::KbdBacklight, values) => {
				if let Some(values) = values
					&& let Ok((value, n_segments)) =
						global_utils::segmented_progress_parser(&values)
				{
					iter_windows!(self, action_options, (window), {
						window.changed_kbd_backlight(&action_options, value, n_segments);
					});
				}
			}

			// Custom actions
			(ArgTypes::CustomMessage, message) => {
				if let Some(message) = message {
					iter_windows!(self, action_options, (window), {
						window.custom_message(&action_options, &message);
					});
				}
			}
			(ArgTypes::CustomProgress, fraction) => {
				if let Some(fraction) = fraction {
					let fraction: f64 = fraction.parse::<f64>().unwrap_or(1.0);
					iter_windows!(self, action_options, (window), {
						window.custom_progress(&action_options, fraction);
					});
				}
			}
			(ArgTypes::CustomSegmentedProgress, values) => {
				if let Some(values) = values
					&& let Ok((value, n_segments)) =
						global_utils::segmented_progress_parser(&values)
				{
					iter_windows!(self, action_options, (window), {
						window.custom_segmented_progress(&action_options, value, n_segments);
					});
				}
			}
		};
		Ok(())
	}
}
