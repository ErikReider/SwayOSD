mod application;
mod osd_window;
mod upower;
mod utils;
mod widgets;

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

#[path = "../mpris-backend/mod.rs"]
mod playerctl;

#[macro_use]
extern crate shrinkwraprs;

#[macro_use]
extern crate cascade;

use application::SwayOSDApplication;
use argtypes::ArgTypes;
use async_channel::Sender;
use clap::Parser;
use config::{DBUS_PATH, DBUS_SERVER_NAME};
use gtk::{
	gdk::Display,
	gio::{self, Resource},
	glib::Bytes,
	CssProvider, IconTheme,
};
use std::{future::pending, str::FromStr, sync::Arc};
use utils::{get_system_css_path, user_style_path};
use zbus::{connection, interface};

struct DbusServer {
	sender: Sender<(ArgTypes, String)>,
}

#[interface(name = "org.erikreider.swayosd")]
impl DbusServer {
	pub async fn handle_action(&self, arg_type: String, data: String) -> bool {
		let arg_type = match ArgTypes::from_str(&arg_type) {
			Ok(arg_type) => arg_type,
			Err(other_type) => {
				eprintln!("Unknown action in Dbus handle_action: {:?}", other_type);
				return false;
			}
		};
		if let Err(error) = self.sender.send((arg_type, data)).await {
			eprintln!("Channel Send error: {}", error);
			return false;
		}
		true
	}
}

impl DbusServer {
	async fn init(sender: Sender<(ArgTypes, String)>) -> zbus::Result<()> {
		let _connection = connection::Builder::session()?
			.name(DBUS_SERVER_NAME)?
			.serve_at(DBUS_PATH, DbusServer { sender })?
			.build()
			.await?;
		pending::<()>().await;
		Ok(())
	}
}

const GRESOURCE_BASE_PATH: &str = "/org/erikreider/swayosd";

fn main() {
	if gtk::init().is_err() {
		eprintln!("failed to initialize GTK Application");
		std::process::exit(1);
	}

	// Load the compiled resource bundle
	let resources_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/swayosd.gresource"));
	let resource_data = Bytes::from(&resources_bytes[..]);
	let res = Resource::from_data(&resource_data).unwrap();
	gio::resources_register(&res);

	// Load the icon theme
	let theme = IconTheme::default();
	theme.add_resource_path(&format!("{}/icons", GRESOURCE_BASE_PATH));

	// Load the CSS themes
	let display = Display::default().expect("Failed getting the default screen");

	// Load the provided default CSS theme
	let provider = CssProvider::new();
	provider.connect_parsing_error(|_provider, _section, error| {
		eprintln!("Could not load default CSS stylesheet: {}", error);
	});
	match get_system_css_path() {
		Some(path) => {
			provider.load_from_path(path.to_str().unwrap());
			gtk::style_context_add_provider_for_display(
				&display,
				&provider,
				gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
			);
		}
		None => eprintln!("Could not find the system CSS file..."),
	}

	let args = Arc::new(args::ArgsServer::parse());

	// Parse Config
	let server_config = Arc::new(
		config::user::read_user_config(args.config.as_deref())
			.expect("Failed to parse config file")
			.server,
	);

	// Try loading the users CSS theme
	if let Some(user_config_path) = user_style_path(args.style.clone()) {
		let user_provider = CssProvider::new();
		user_provider.connect_parsing_error(|_provider, _section, error| {
			eprintln!("Failed loading user defined style.css: {}", error);
		});
		user_provider.load_from_path(&user_config_path);
		gtk::style_context_add_provider_for_display(
			&display,
			&user_provider,
			gtk::STYLE_PROVIDER_PRIORITY_USER,
		);
		println!("Loaded user defined CSS file");
	}

	let (sender, receiver) = async_channel::bounded::<(ArgTypes, String)>(1);
	// Start the DBus Server
	async_std::task::spawn(DbusServer::init(sender));
	// Start the GTK Application
	std::process::exit(SwayOSDApplication::new(server_config, args, receiver).start());
}
