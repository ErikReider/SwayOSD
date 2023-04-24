use std::f64::consts::PI;

use gtk::prelude::*;
use gtk::*;

use crate::utils::is_dark_mode;

const DEGREES: f64 = PI / 180.0;
const PROGRESSBAR_HEIGHT: i32 = 6;

#[derive(Shrinkwrap)]
pub struct ProgressBar {
	#[shrinkwrap(main_field)]
	pub bar: gtk::DrawingArea,
}

impl ProgressBar {
	pub fn new(fraction: f64) -> Self {
		let bar = gtk::DrawingArea::new();
		bar.set_height_request(PROGRESSBAR_HEIGHT);

		bar.connect_draw(move |area, ctx| {
			let width = f64::from(area.allocated_width());
			let height: f64 = f64::from(PROGRESSBAR_HEIGHT);
			let radius: f64 = height * 0.5;

			// Gets the themes bg and fg colors for the OSD window.
			// Compares the fg and bg to determine if the theme is a
			// dark or a light theme.
			let def_fg = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
			let def_bg = gdk::RGBA::new(1.0, 1.0, 1.0, 1.0);
			let (fg, bg, dark_mode) = match area.toplevel() {
				Some(win) => {
					let style = win.style_context();
					let fg = style
						.style_property_for_state(gtk::STYLE_PROPERTY_COLOR, StateFlags::NORMAL)
						.get::<gdk::RGBA>();
					let bg = style
						.style_property_for_state(
							gtk::STYLE_PROPERTY_BACKGROUND_COLOR,
							StateFlags::NORMAL,
						)
						.get::<gdk::RGBA>();
					let (fg, bg) = match (fg, bg) {
						(Ok(fg), Ok(bg)) => (fg, bg),
						_ => (def_fg, def_bg),
					};
					(fg, bg, is_dark_mode(&fg, &bg))
				}
				None => (def_fg, def_bg, is_dark_mode(&def_fg, &def_bg)),
			};

			// Background
			if dark_mode {
				ctx.set_source_rgb(fg.red() * 0.5, fg.green() * 0.5, fg.blue() * 0.5);
			} else {
				ctx.set_source_rgb(bg.red() * 0.75, bg.green() * 0.75, bg.blue() * 0.75);
			}
			ProgressBar::draw_rounded_rectangle(width, height, radius, ctx);

			// Progress
			ctx.set_source_rgb(fg.red(), fg.green(), fg.blue());
			ProgressBar::draw_rounded_rectangle(width * fraction, height, radius, ctx);

			gtk::Inhibit(true)
		});

		Self { bar }
	}

	fn draw_rounded_rectangle(width: f64, height: f64, radius: f64, ctx: &cairo::Context) {
		ctx.new_sub_path();
		ctx.arc(
			width - radius,
			radius,
			radius,
			-90.0 * DEGREES,
			0.0 * DEGREES,
		);
		ctx.arc(
			width - radius,
			height - radius,
			radius,
			0.0 * DEGREES,
			90.0 * DEGREES,
		);
		ctx.arc(
			radius,
			height - radius,
			radius,
			90.0 * DEGREES,
			180.0 * DEGREES,
		);
		ctx.arc(radius, radius, radius, 180.0 * DEGREES, 270.0 * DEGREES);
		ctx.close_path();

		ctx.clip();
		ctx.paint().expect("Couldn't paint OSD ProgressBar!...");
	}
}
