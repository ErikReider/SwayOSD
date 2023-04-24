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
	let args: Vec<String> = std::env::args().collect();
	if args.len() > 1 {
		let max_volume: u8 = args[1].parse().unwrap_or(150 as u8);
		utils::set_max_volume(max_volume);
	}

	if gtk::init().is_err() {
		eprintln!("failed to initialize GTK Application");
		std::process::exit(1);
	}
	std::process::exit(SwayOSDApplication::new().start());
}
