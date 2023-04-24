mod application;
mod osd_window;
mod progressbar;
mod utils;

#[macro_use]
extern crate shrinkwraprs;

#[macro_use]
extern crate cascade;

use application::SwayOSDApplication;

fn main() {
	if gtk::init().is_err() {
		eprintln!("failed to initialize GTK Application");
		std::process::exit(1);
	}
	std::process::exit(SwayOSDApplication::new().start());
}
