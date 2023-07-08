mod application;
mod osd_window;
mod progressbar;
mod utils;

#[macro_use]
extern crate shrinkwraprs;

#[macro_use]
extern crate cascade;

use application::SwayOSDApplication;
use gtk::{
	gio::{self, Resource},
	glib::Bytes,
	traits::IconThemeExt,
	IconTheme,
};

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
	theme.add_resource_path("/org/erikreider/swayosd/icons");

	std::process::exit(SwayOSDApplication::new().start());
}
