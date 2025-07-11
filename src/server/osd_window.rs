use std::rc::Rc;
use std::time::Duration;
use std::{cell::RefCell, ops::Deref};

use gtk::{
	gdk,
	glib::{self, clone},
	prelude::*,
};
use pulsectl::controllers::types::DeviceInfo;

use crate::{
	brightness_backend::BrightnessBackend,
	utils::{
		get_max_volume, get_show_percentage, get_top_margin, volume_to_f64, KeysLocks,
		VolumeDeviceType,
	},
};

use gtk_layer_shell::LayerShell;

const ICON_SIZE: i32 = 32;

/// A window that our application can open that contains the main project view.
#[derive(Clone, Debug)]
pub struct SwayosdWindow {
	pub window: gtk::ApplicationWindow,
	pub monitor: gdk::Monitor,
	container: gtk::Box,
	timeout_id: Rc<RefCell<Option<glib::SourceId>>>,
}

impl SwayosdWindow {
	/// Create a new window and assign it to the given application.
	pub fn new(app: &gtk::Application, monitor: &gdk::Monitor) -> Self {
		let window = gtk::ApplicationWindow::new(app);
		window.set_widget_name("osd");
		window.add_css_class("osd");

		window.init_layer_shell();
		window.set_monitor(Some(monitor));
		window.set_namespace(Some("swayosd"));

		window.set_exclusive_zone(-1);
		window.set_layer(gtk_layer_shell::Layer::Overlay);
		window.set_anchor(gtk_layer_shell::Edge::Top, true);

		// Set up the widgets
		window.set_width_request(250);

		let container = cascade! {
			gtk::Box::new(gtk::Orientation::Horizontal, 12);
			..set_widget_name("container");
		};

		window.set_child(Some(&container));

		// Disable mouse input
		window.connect_map(|window| {
			if let Some(surface) = window.surface() {
				let region = gtk::cairo::Region::create();
				surface.set_input_region(&region);
			}
		});

		let update_margins = |window: &gtk::ApplicationWindow, monitor: &gdk::Monitor| {
			// Monitor scale factor is not always correct
			// Transform monitor height into coordinate system of window
			let mon_height =
				monitor.geometry().height() * monitor.scale_factor() / window.scale_factor();
			// Calculate new margin
			let bottom = mon_height - window.allocated_height();
			let margin = (bottom as f32 * get_top_margin()).round() as i32;
			window.set_margin(gtk_layer_shell::Edge::Top, margin);
		};

		// Set the window margin
		update_margins(&window, monitor);
		// Ensure window margin is updated when necessary
		window.connect_scale_factor_notify(clone!(
			#[weak]
			monitor,
			move |window| update_margins(window, &monitor)
		));
		monitor.connect_scale_factor_notify(clone!(
			#[weak]
			window,
			move |monitor| update_margins(&window, monitor)
		));
		monitor.connect_geometry_notify(clone!(
			#[weak]
			window,
			move |monitor| update_margins(&window, monitor)
		));

		Self {
			window,
			container,
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
		let label = self.build_text_widget(Some(&format!("{}%", volume)));

		progress.set_sensitive(!device.mute);

		self.container.append(&icon);
		self.container.append(&progress);
		if get_show_percentage() {
			self.container.append(&label);
		}

		self.run_timeout();
	}

	pub fn changed_brightness(&self, brightness_backend: &mut dyn BrightnessBackend) {
		self.clear_osd();

		let icon_name = "display-brightness-symbolic";
		let icon = self.build_icon_widget(icon_name);

		let brightness = brightness_backend.get_current() as f64;
		let max = brightness_backend.get_max() as f64;
		let progress = self.build_progress_widget(brightness / max);
		let label = self.build_text_widget(Some(&format!("{}%", (brightness / max * 100.) as i32)));

		self.container.append(&icon);
		self.container.append(&progress);
		if get_show_percentage() {
			self.container.append(&label);
		}

		self.run_timeout();
	}

	pub fn changed_player(&self, icon: &str, label: Option<&str>) {
		self.clear_osd();

		let icon = self.build_icon_widget(icon);
		let label = self.build_text_widget(label);

		self.container.append(&icon);
		self.container.append(&label);

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

		self.container.append(&icon);
		self.container.append(&label);

		self.run_timeout();
	}

	pub fn custom_progress(&self, fraction: f64, text: Option<String>, icon_name: Option<&str>) {
		self.clear_osd();

		if let Some(icon_name) = icon_name {
			let icon = self.build_icon_widget(icon_name);
			self.container.append(&icon);
		}

		let progress = self.build_progress_widget(fraction.clamp(0.0, 1.0));
		self.container.append(&progress);

		if let Some(text) = text {
			let label = self.build_text_widget(Some(text.deref()));
			self.container.append(&label);
		}

		self.run_timeout();
	}

	pub fn custom_message(&self, message: &str, icon_name: Option<&str>) {
		self.clear_osd();

		let label = self.build_text_widget(Some(message));

		if let Some(icon_name) = icon_name {
			let icon = self.build_icon_widget(icon_name);
			self.container.append(&icon);
			self.container.append(&label);
			let box_spacing = self.container.spacing();
			icon.connect_realize(move |icon| {
				label.set_margin_end(
					icon.allocation().width()
						+ icon.margin_start()
						+ icon.margin_end()
						+ box_spacing,
				);
			});
		} else {
			self.container.append(&label);
		}

		self.run_timeout();
	}

	/// Clear all container children
	fn clear_osd(&self) {
		let mut next = self.container.first_child();
		while let Some(widget) = next {
			next = widget.next_sibling();
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

		self.window.show();
	}

	fn build_icon_widget(&self, icon_name: &str) -> gtk::Image {
		let icon = gtk::gio::ThemedIcon::from_names(&[icon_name, "missing-symbolic"]);

		cascade! {
			gtk::Image::from_gicon(&icon.upcast::<gtk::gio::Icon>());
			..set_pixel_size(ICON_SIZE);
		}
	}

	fn build_text_widget(&self, text: Option<&str>) -> gtk::Label {
		cascade! {
			gtk::Label::new(text);
			..set_halign(gtk::Align::Center);
			..set_hexpand(true);
			..add_css_class("title-4");
		}
	}

	fn build_progress_widget(&self, fraction: f64) -> gtk::ProgressBar {
		cascade! {
			gtk::ProgressBar::new();
			..set_fraction(fraction);
			..set_valign(gtk::Align::Center);
			..set_hexpand(true);
		}
	}
}
