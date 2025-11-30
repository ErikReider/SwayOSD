#[path = "../args.rs"]
mod args;
#[path = "../argtypes.rs"]
mod argtypes;
#[path = "../config.rs"]
mod config;
#[path = "../global_utils.rs"]
mod global_utils;

#[path = "../brightness_backend/mod.rs"]
mod brightness_backend;

use clap::Parser;
use config::APPLICATION_NAME;
use gtk::{gio::ApplicationFlags, Application};
use gtk::{glib, prelude::*};
use zbus::{blocking::Connection, proxy};

use crate::args::ArgsClient;
use crate::argtypes::ArgTypes;

#[proxy(
	interface = "org.erikreider.swayosd",
	default_service = "org.erikreider.swayosd-server",
	default_path = "/org/erikreider/swayosd"
)]
trait Server {
	async fn handle_action(&self, arg_type: String, data: String) -> zbus::Result<bool>;
}

fn get_proxy() -> zbus::Result<ServerProxyBlocking<'static>> {
	let connection = Connection::session()?;
	ServerProxyBlocking::new(&connection)
}

fn main() -> Result<(), glib::Error> {
	let args = args::ArgsClient::parse();

	// Parse Config
	let _client_config = config::user::read_user_config(args.config.as_deref())
		.expect("Failed to parse config file")
		.client;

	// Make sure that the server is running
	let proxy = match get_proxy() {
		Ok(proxy) => match proxy.0.introspect() {
			Ok(_) => proxy,
			Err(err) => {
				eprintln!("Could not connect to SwayOSD Server with error: {}", err);
				std::process::exit(1);
			}
		},
		Err(err) => {
			eprintln!("Dbus error: {}", err);
			std::process::exit(1);
		}
	};

	let app = Application::new(Some(APPLICATION_NAME), ApplicationFlags::FLAGS_NONE);

	parse_args(&args, &proxy);

	let empty_args: Vec<String> = vec![];
	std::process::exit(app.run_with_args(&empty_args).into());
}

fn parse_args(args: &ArgsClient, proxy: &ServerProxyBlocking<'_>) {
	let mut actions: Vec<(ArgTypes, Option<String>)> = Vec::new();

	//
	// Parse flags. Should always be first to set a global variable before executing related functions
	//

	// Pulse Device
	if let Some(value) = args.device.to_owned() {
		actions.push((ArgTypes::DeviceName, Some(value)));
	}
	// Max volume
	if let Some(value) = args.max_volume.to_owned() {
		match value.parse::<u8>() {
			Ok(_) => actions.push((ArgTypes::MaxVolume, Some(value))),
			Err(_) => eprintln!("{} is not a number between 0 and {}!", value, u8::MAX),
		}
	}
	// Custom icon
	if let Some(value) = args.custom_icon.to_owned() {
		actions.push((ArgTypes::MaxVolume, Some(value)));
	}
	// Player name
	if let Some(value) = args.player.to_owned() {
		actions.push((ArgTypes::Player, Some(value)));
	}
	// Monitor name
	if let Some(value) = args.monitor.to_owned() {
		actions.push((ArgTypes::MonitorName, Some(value)));
	}
	// Custom progress text
	if let Some(value) = args.custom_progress_text.to_owned() {
		actions.push((ArgTypes::CustomProgressText, Some(value)));
	}
	// Min Brightness
	if let Some(value) = args.min_brightness.to_owned() {
		match value.parse::<u8>() {
			Ok(value @ 0u8..=100u8) => {
				actions.push((ArgTypes::MinBrightness, Some(value.to_string())))
			}
			_ => eprintln!("{} is not a number between 0 and {}!", value, 100),
		}
	}

	//
	// Main options
	//

	// Caps lock
	if args.caps_lock {
		actions.push((ArgTypes::CapsLock, None));
	}
	// Caps lock LED
	if let Some(value) = args.caps_lock_led.to_owned() {
		actions.push((ArgTypes::CapsLock, Some(value)));
	}
	// Num lock
	if args.num_lock {
		actions.push((ArgTypes::NumLock, None));
	}
	// Num lock LED
	if let Some(value) = args.num_lock_led.to_owned() {
		actions.push((ArgTypes::NumLock, Some(value)));
	}
	// Scroll lock
	if args.scroll_lock {
		actions.push((ArgTypes::ScrollLock, None));
	}
	// Scroll lock LED
	if let Some(value) = args.scroll_lock_led.to_owned() {
		actions.push((ArgTypes::ScrollLock, Some(value)));
	}
	// Output volume
	if let Some(value) = args.output_volume.as_deref() {
		if let Ok(parsed) = volume_parser(false, value) {
			actions.push(parsed);
		}
	}
	// Input volume
	if let Some(value) = args.input_volume.as_deref() {
		if let Ok(parsed) = volume_parser(true, value) {
			actions.push(parsed);
		}
	}
	// Brightness
	if let Some(value) = args.brightness.as_deref() {
		// let value: &str = value.as_str();
		let value = match (value, value.parse::<i8>()) {
			// Parse custom step values
			(_, Ok(num)) => match value.get(..1) {
				Some("+") => Some((ArgTypes::BrightnessRaise, Some(num.to_string()))),
				Some("-") => Some((ArgTypes::BrightnessLower, Some(num.abs().to_string()))),
				_ => Some((ArgTypes::BrightnessSet, Some(num.to_string()))),
			},

			("raise", _) => Some((ArgTypes::BrightnessRaise, None)),
			("lower", _) => Some((ArgTypes::BrightnessLower, None)),
			(e, _) => {
				eprintln!("Unknown brightness mode: \"{}\"!...", e);
				None
			}
		};
		if let Some(value) = value {
			actions.push(value);
		}
	}
	// Playerctl
	if let Some(value) = args.playerctl.as_deref() {
		match value {
			"play-pause" | "play" | "pause" | "next" | "prev" | "previous" | "shuffle" | "stop" => {
				actions.push((ArgTypes::Playerctl, Some(value.to_string())));
			}
			x => eprintln!("Unknown Playerctl command: \"{}\"!...", x),
		}
	}
	// Custom message
	if let Some(value) = args.custom_message.to_owned() {
		actions.push((ArgTypes::CustomMessage, Some(value)));
	}
	// Custom progress
	if let Some(value) = args.custom_progress.as_deref() {
		match value.parse::<f64>() {
			Ok(_) => actions.push((ArgTypes::CustomProgress, Some(value.to_string()))),
			Err(_) => eprintln!("{} is not a number between 0.0 and 1.0!", value),
		}
	}
	// Custom segmented progress
	if let Some(value) = args.custom_segmented_progress.as_deref() {
		match global_utils::segmented_progress_parser(value) {
			Ok((value, n_segments)) => actions.push((
				ArgTypes::CustomSegmentedProgress,
				Some(format!("{}:{}", value, n_segments)),
			)),
			Err(msg) => eprintln!("{}", msg),
		}
	}

	// execute the sorted actions
	for (arg_type, data) in actions {
		let _ = proxy.handle_action(arg_type.to_string(), data.unwrap_or(String::new()));
	}
}

fn volume_parser(is_sink: bool, value: &str) -> Result<(ArgTypes, Option<String>), i32> {
	let mut v = match (value, value.parse::<i8>()) {
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
			return Err(1);
		}
	};
	if is_sink {
		if v.0 == ArgTypes::SinkVolumeRaise {
			v.0 = ArgTypes::SourceVolumeRaise;
		} else if v.0 == ArgTypes::SinkVolumeLower {
			v.0 = ArgTypes::SourceVolumeLower;
		} else {
			v.0 = ArgTypes::SourceVolumeMuteToggle;
		}
	}
	Ok(v)
}
