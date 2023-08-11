#![feature(if_let_guard)]
mod application;
mod osd_window;
mod utils;

#[path = "../argtypes.rs"]
mod argtypes;
#[path = "../config.rs"]
mod config;
#[path = "../global_utils.rs"]
mod global_utils;

#[macro_use]
extern crate shrinkwraprs;

#[macro_use]
extern crate cascade;

use application::SwayOSDApplication;
use argtypes::ArgTypes;
use config::{DBUS_SERVER_NAME, DBUS_PATH};
use gtk::glib::{MainContext, Priority, Sender};
use gtk::prelude::*;
use gtk::{
	gdk::Screen,
	gio::{self, Resource},
	glib::Bytes,
	traits::IconThemeExt,
	CssProvider, IconTheme, StyleContext,
};
use std::future::pending;
use std::str::FromStr;
use utils::user_style_path;
use zbus::{dbus_interface, ConnectionBuilder};

struct DbusServer {
	sender: Sender<(ArgTypes, String)>,
}

#[dbus_interface(name = "org.erikreider.swayosd")]
impl DbusServer {
	pub fn handle_action(&self, arg_type: String, data: String) -> bool {
		let arg_type = match ArgTypes::from_str(&arg_type) {
			Ok(arg_type) => arg_type,
			Err(other_type) => {
				eprintln!("Unknown action in Dbus handle_action: {:?}", other_type);
				return false;
			}
		};
		if let Err(error) = self.sender.send((arg_type, data)) {
			eprintln!("Channel Send error: {}", error);
			return false;
		}
		true
	}
}

impl DbusServer {
	async fn new(sender: Sender<(ArgTypes, String)>) -> zbus::Result<()> {
		let _connection = ConnectionBuilder::session()?
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
	let resources_bytes = include_bytes!("../../data/swayosd.gresource");
	let resource_data = Bytes::from(&resources_bytes[..]);
	let res = Resource::from_data(&resource_data).unwrap();
	gio::resources_register(&res);

	// Load the icon theme
	let theme = IconTheme::default().expect("Could not get IconTheme");
	theme.add_resource_path(&format!("{}/icons", GRESOURCE_BASE_PATH));

	// Load the CSS themes
	let screen = Screen::default().expect("Failed getting the default screen");

	// Load the provided default CSS theme
	let provider = CssProvider::new();
	provider.load_from_resource(&format!("{}/style/style.css", GRESOURCE_BASE_PATH));
	StyleContext::add_provider_for_screen(
		&screen,
		&provider,
		gtk::STYLE_PROVIDER_PRIORITY_APPLICATION as u32,
	);

	// Try loading the users CSS theme
	if let Some(user_config_path) = user_style_path() {
		let user_provider = CssProvider::new();
		user_provider
			.load_from_path(&user_config_path)
			.expect("Failed loading user defined style.css");
		StyleContext::add_provider_for_screen(
			&screen,
			&user_provider,
			gtk::STYLE_PROVIDER_PRIORITY_APPLICATION as u32,
		);
		println!("Loaded user defined CSS file");
	}

	let (sender, receiver) = MainContext::channel::<(ArgTypes, String)>(Priority::default());
	// Start the DBus Server
	async_std::task::spawn(DbusServer::new(sender));
	// Start the GTK Application
	std::process::exit(SwayOSDApplication::new(receiver).start());
}
