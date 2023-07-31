mod application;
mod osd_window;
mod utils;

mod config;

#[macro_use]
extern crate shrinkwraprs;

#[macro_use]
extern crate cascade;

use application::SwayOSDApplication;
use gtk::prelude::*;
use gtk::{
	gdk::Screen,
	gio::{self, Resource},
	glib::Bytes,
	traits::IconThemeExt,
	CssProvider, IconTheme, StyleContext,
};
use utils::user_style_path;

const GRESOURCE_BASE_PATH: &str = "/org/erikreider/swayosd";

fn main() {
	if gtk::init().is_err() {
		eprintln!("failed to initialize GTK Application");
		std::process::exit(1);
	}

	// Load the compiled resource bundle
	let resources_bytes = include_bytes!("../data/swayosd.gresource");
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

	std::process::exit(SwayOSDApplication::new().start());
}
