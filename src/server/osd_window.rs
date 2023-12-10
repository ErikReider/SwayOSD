use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::{
	gdk,
	glib::{self, clone},
	prelude::*,
};
use pulsectl::controllers::types::DeviceInfo;

use crate::{
	brightness_backend::BrightnessBackend,
	utils::{get_max_volume, get_top_margin, volume_to_f64, KeysLocks, VolumeDeviceType},
};

const ICON_SIZE: i32 = 32;

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
		window.set_widget_name("osd");
		window
			.style_context()
			.add_class(&gtk::STYLE_CLASS_OSD.to_string());

		gtk_layer_shell::init_for_window(&window);
		gtk_layer_shell::set_monitor(&window, monitor);
		gtk_layer_shell::set_namespace(&window, "swayosd");

		gtk_layer_shell::set_exclusive_zone(&window, -1);
		gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay);
		gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Top, true);

		// Set up the widgets
		window.set_width_request(250);

		let container = cascade! {
			gtk::Box::new(gtk::Orientation::Horizontal, 12);
			..set_widget_name("container");
		};

		window.add(&container);

		// Set the window margin
		window.connect_map(clone!(@strong monitor => move |win| {
			let bottom = monitor.workarea().height() - win.allocated_height();
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

		let max_volume: f64 = get_max_volume().into();

		let icon = self.build_icon_widget(icon_name);
		let progress = self.build_progress_widget(volume / max_volume);

		progress.set_sensitive(!device.mute);

		self.container.add(&icon);
		self.container.add(&progress);

		self.run_timeout();
	}

	pub fn changed_brightness(&self, brightness_backend: &mut dyn BrightnessBackend) {
		self.clear_osd();

		let icon_name = "display-brightness-symbolic";
		let icon = self.build_icon_widget(icon_name);

		let brightness = brightness_backend.get_current() as f64;
		let max = brightness_backend.get_max() as f64;
		let progress = self.build_progress_widget(brightness / max);

		self.container.add(&icon);
		self.container.add(&progress);

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

		icon.set_sensitive(state);

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

	fn build_progress_widget(&self, fraction: f64) -> gtk::ProgressBar {
		cascade! {
			gtk::ProgressBar::new();
			..set_fraction(fraction);
			..set_valign(gtk::Align::Center);
			..set_expand(true);
		}
	}
}
