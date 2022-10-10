mod application;
mod osd_window;
mod utils;

#[macro_use] extern crate shrinkwraprs;

use application::SwayOSDApplication;

fn main() {
    if gtk::init().is_err() {
        eprintln!("failed to initialize GTK Application");
        std::process::exit(1);
    }
    std::process::exit(SwayOSDApplication::new().start());
}
