use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Duration;

use gtk::{
	cairo, gdk,
	glib::{self, clone},
	prelude::*,
};
use pulsectl::controllers::types::DeviceInfo;

use crate::utils::{volume_to_f64, KeysLocks, VolumeDeviceType, get_top_margin};
use blight::Device;

const DISABLED_OPACITY: f64 = 0.5;
const ICON_SIZE: i32 = 32;
const WINDOW_MARGIN: i32 = 16;
const DEGREES: f64 = PI / 180.0;

/// A window that our application can open that contains the main project view.
#[derive(Clone, Debug)]
pub struct SwayosdWindow {
	pub window: gtk::ApplicationWindow,
	pub display: gdk::Display,
	pub monitor: gdk::Monitor,
	container: gtk::Box,
	timeout_id: Rc<RefCell<Option<glib::SourceId>>>,
}

impl SwayosdWindow {
	/// Create a new window and assign it to the given application.
	pub fn new(app: &gtk::Application, display: &gdk::Display, monitor: &gdk::Monitor) -> Self {
		let window = gtk::ApplicationWindow::new(app);
		window
			.style_context()
			.add_class(&gtk::STYLE_CLASS_OSD.to_string());

		gtk_layer_shell::init_for_window(&window);
		gtk_layer_shell::set_monitor(&window, monitor);
		gtk_layer_shell::set_namespace(&window, "swayosd");

		gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay);
		gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Top, true);

		// Set up a widget
		let container = cascade! {
			gtk::Box::new(gtk::Orientation::Horizontal, 12);
			..set_margin(WINDOW_MARGIN);
		};
		window.add(&container);
		window.set_width_request(250);
		let style = window.style_context();
		window.connect_draw(move |win, ctx| {
			let width = f64::from(win.allocated_width());
			let height = f64::from(win.allocated_height());
			let radius: f64 = height * 0.5;

			let bg = style
				.style_property_for_state(
					gtk::STYLE_PROPERTY_BACKGROUND_COLOR.to_string().as_str(),
					gtk::StateFlags::NORMAL,
				)
				.get::<gdk::RGBA>();
			let bg = match bg {
				Ok(bg) => bg,
				Err(_) => gdk::RGBA::new(1.0, 1.0, 1.0, 1.0),
			};

			ctx.save().expect("Couldn't save OSD window!...");
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

			ctx.set_operator(cairo::Operator::Source);
			ctx.set_source_rgba(bg.red(), bg.green(), bg.blue(), bg.alpha());
			ctx.clip();
			ctx.paint().expect("Couldn't paint OSD window!...");
			ctx.restore().expect("Couldn't restore OSD window!...");

			ctx.save().expect("Couldn't save OSD window!...");
			ctx.translate(f64::from(WINDOW_MARGIN), f64::from(WINDOW_MARGIN));
			win.child().unwrap().draw(ctx);
			ctx.restore().expect("Couldn't restore OSD window!...");
			gtk::Inhibit(true)
		});

		// Set the window margin
		window.connect_map(clone!(@strong monitor => move |win| {
			let bottom = monitor.workarea().height() - win.height_request();
			let margin = (bottom as f32 * get_top_margin()).round() as i32;
			gtk_layer_shell::set_margin(win, gtk_layer_shell::Edge::Top, margin);
		}));

		Self {
			window,
			container,
			display: display.clone(),
			monitor: monitor.clone(),
			timeout_id: Rc::new(RefCell::new(None)),
		}
	}

	pub fn close(&self) {
		self.window.close();
	}

	pub fn changed_volume(&self, device: &DeviceInfo, device_type: &VolumeDeviceType) {
		self.clear_osd();

		let volume = volume_to_f64(&device.volume.avg());
		let icon_prefix = match device_type {
			VolumeDeviceType::Sink(_) => "sink",
			VolumeDeviceType::Source(_) => "source",
		};
		let icon_state = &match (device.mute, volume) {
			(true, _) => "muted",
			(_, x) if x == 0.0 => "muted",
			(false, x) if x > 0.0 && x <= 33.0 => "low",
			(false, x) if x > 33.0 && x <= 66.0 => "medium",
			(false, x) if x > 66.0 && x <= 100.0 => "high",
			(false, x) if x > 100.0 => match device_type {
				VolumeDeviceType::Sink(_) => "high",
				VolumeDeviceType::Source(_) => "overamplified",
			},
			(_, _) => "high",
		};
		let icon_name = &format!("{}-volume-{}-symbolic", icon_prefix, icon_state);

		let icon = self.build_icon_widget(icon_name);
		let progress = self.build_progress_widget(volume / 100.0);

		if device.mute {
			progress.set_opacity(DISABLED_OPACITY);
		} else {
			progress.set_opacity(1.0);
		}

		self.container.add(&icon);
		self.container.add(&progress.bar);

		self.run_timeout();
	}

	pub fn changed_brightness(&self, device: &Device) {
		self.clear_osd();

		let icon_name = "display-brightness-symbolic";
		let icon = self.build_icon_widget(icon_name);

		let brightness = device.current() as f64;
		let max = device.max() as f64;
		let progress = self.build_progress_widget(brightness / max);

		self.container.add(&icon);
		self.container.add(&progress.bar);

		self.run_timeout();
	}

	pub fn changed_keylock(&self, key: KeysLocks, state: bool) {
		self.clear_osd();

		let label = self.build_text_widget(None);

		let on_off_text = match state {
			true => "On",
			false => "Off",
		};

		let (label_text, symbol) = match key {
			KeysLocks::CapsLock => {
				let symbol = "caps-lock-symbolic";
				let text = "Caps Lock ".to_string() + on_off_text;
				(text, symbol)
			}
			KeysLocks::NumLock => {
				let symbol = "num-lock-symbolic";
				let text = "Num Lock ".to_string() + on_off_text;
				(text, symbol)
			}
			KeysLocks::ScrollLock => {
				let symbol = "scroll-lock-symbolic";
				let text = "Scroll Lock ".to_string() + on_off_text;
				(text, symbol)
			}
		};

		label.set_text(&label_text);
		let icon = self.build_icon_widget(symbol);

		if !state {
			icon.set_opacity(DISABLED_OPACITY);
		} else {
			icon.set_opacity(1.0);
		}

		self.container.add(&icon);
		self.container.add(&label);

		self.run_timeout();
	}

	/// Clear all container children
	fn clear_osd(&self) {
		for widget in self.container.children() {
			self.container.remove(&widget);
		}
	}

	fn run_timeout(&self) {
		// Hide window after timeout
		if let Some(timeout_id) = self.timeout_id.take() {
			timeout_id.remove()
		}
		let s = self.clone();
		self.timeout_id.replace(Some(glib::timeout_add_local_once(
			Duration::from_millis(1000),
			move || {
				s.window.hide();
				s.timeout_id.replace(None);
			},
		)));

		self.window.show_all();
	}

	fn build_icon_widget(&self, icon_name: &str) -> gtk::Image {
		let icon_name = match gtk::IconTheme::default() {
			Some(theme) if theme.has_icon(icon_name) => icon_name,
			_ => "missing-symbolic",
		};

		cascade! {
			gtk::Image::from_icon_name(Some(icon_name), gtk::IconSize::Invalid);
			..set_pixel_size(ICON_SIZE);
		}
	}

	fn build_text_widget(&self, text: Option<&str>) -> gtk::Label {
		cascade! {
			gtk::Label::new(text);
			..set_halign(gtk::Align::Center);
			..set_hexpand(true);
			..style_context().add_class("title-4");
		}
	}

	fn build_progress_widget(&self, fraction: f64) -> crate::progressbar::ProgressBar {
		cascade! {
			crate::progressbar::ProgressBar::new(fraction);
			..set_valign(gtk::Align::Center);
			..set_expand(true);
		}
	}
}
