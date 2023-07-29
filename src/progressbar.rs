use std::f64::consts::PI;
use std::hint::black_box;

use gtk::prelude::*;
use gtk::*;

use crate::utils::is_dark_mode;

const DEGREES: f64 = PI / 180.0;
const PROGRESSBAR_HEIGHT: i32 = 6;

#[derive(Shrinkwrap)]
pub struct ProgressBar {
	#[shrinkwrap(main_field)]
	pub bar: gtk::Box,
}

impl ProgressBar {
	pub fn new(fraction: f64) -> Self {
		let outer = cascade! {
			gtk::Box::new(gtk::Orientation::Horizontal, 0);
		};
		let inner = cascade! {
			gtk::Box::new(gtk::Orientation::Horizontal, 0);
			..set_height_request(PROGRESSBAR_HEIGHT);
			..style_context().add_class("progress");
		};

		outer.connect_size_allocate(move |outer, rect| {
			let width = (rect.width() as f64 * fraction) as i32;
			outer.children()[0].set_size_request(width, PROGRESSBAR_HEIGHT);
		});

		outer.add(&inner);
		Self { bar: outer }
	}
}
