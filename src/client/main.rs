#[path = "../argtypes.rs"]
mod argtypes;
mod client;
#[path = "../config.rs"]
mod config;
#[path = "../global_utils.rs"]
mod global_utils;

use config::APPLICATION_NAME;
use global_utils::{handle_application_args, HandleLocalStatus};
use gtk::glib::{OptionArg, OptionFlags};
use gtk::{gio::ApplicationFlags, Application};
use gtk::{glib, prelude::*};

fn main() -> Result<(), glib::Error> {
	// Make sure that the server is running
	let proxy = match client::get_proxy() {
		Ok(proxy) => proxy,
		Err(err) => {
			eprintln!("Could not connect to server with error: {}", err);
			std::process::exit(1);
		}
	};

	let app = Application::new(Some(APPLICATION_NAME), ApplicationFlags::FLAGS_NONE);

	// Capslock cmdline arg
	app.add_main_option(
		"caps-lock",
		glib::Char::from(0),
		OptionFlags::NONE,
		OptionArg::None,
		"Shows capslock osd. Note: Doesn't toggle CapsLock, just displays the status",
		None,
	);
	app.add_main_option(
		"num-lock",
		glib::Char::from(0),
		OptionFlags::NONE,
		OptionArg::None,
		"Shows numlock osd. Note: Doesn't toggle NumLock, just displays the status",
		None,
	);
	app.add_main_option(
		"scroll-lock",
		glib::Char::from(0),
		OptionFlags::NONE,
		OptionArg::None,
		"Shows scrolllock osd. Note: Doesn't toggle ScrollLock, just displays the status",
		None,
	);
	// Capslock with specific LED cmdline arg
	app.add_main_option(
		"caps-lock-led",
		glib::Char::from(0),
		OptionFlags::NONE,
		OptionArg::String,
		"Shows capslock osd. Uses LED class name. Note: Doesn't toggle CapsLock, just displays the status",
		Some("LED class name (/sys/class/leds/NAME)"),
	);
	app.add_main_option(
		"num-lock-led",
		glib::Char::from(0),
		OptionFlags::NONE,
		OptionArg::String,
		"Shows numlock osd. Uses LED class name. Note: Doesn't toggle NumLock, just displays the status",
		Some("LED class name (/sys/class/leds/NAME)"),
	);
	app.add_main_option(
		"scroll-lock-led",
		glib::Char::from(0),
		OptionFlags::NONE,
		OptionArg::String,
		"Shows scrolllock osd. Uses LED class name. Note: Doesn't toggle ScrollLock, just displays the status",
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
	// Source volume cmdline arg
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
	app.connect_handle_local_options(move |_app, args| {
		let actions = match handle_application_args(args) {
			(HandleLocalStatus::SUCCESS, actions) => actions,
			(status @ HandleLocalStatus::FAILIURE, _) => return status as i32,
			(status @ HandleLocalStatus::CONTINUE, _) => return status as i32,
		};
		// execute the sorted actions
		for (arg_type, data) in actions {
			let _ = proxy.handle_action(arg_type.to_string(), data.unwrap_or(String::new()));
		}

		HandleLocalStatus::SUCCESS as i32
	});

	std::process::exit(app.run().into());
}
