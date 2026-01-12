use std::rc::Rc;
use std::time::Duration;
use std::{cell::RefCell, ops::Deref};

use gtk::{
	gdk,
	glib::{self, clone},
	prelude::*,
};
use pulsectl::controllers::types::DeviceInfo;

use crate::widgets::segmented_progress_widget::SegmentedProgressWidget;
use crate::{
	brightness_backend::BrightnessBackend,
	utils::{
		get_max_volume, get_show_percentage, get_theme, get_top_margin, volume_to_f64, KeysLocks,
		VolumeDeviceType,
	},
};

use gtk_layer_shell::LayerShell;

const ICON_SIZE: i32 = 32;
const MACOS_WINDOW_SIZE: i32 = 150;
const MACOS_ICON_SIZE: i32 = 100;
fn icons_dir() -> std::path::PathBuf {
	std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons")
}
fn round_to_step(value: f64, step: f64, min: f64, max: f64) -> u32 {
	(((value / step).round() * step).clamp(min, max)) as u32
}
fn build_macos_icon_from_path(path: &std::path::Path) -> gtk::Image {
	let img = gtk::Image::from_file(path);
	img.set_pixel_size(MACOS_ICON_SIZE);
	img
}
fn resolve_macos_volume_icon(device: &DeviceInfo) -> Option<gtk::Image> {
	let base = icons_dir().join("volume");
	// Muted or zero volume -> muted icon
	let vol = volume_to_f64(&device.volume.avg());
	if device.mute || vol == 0.0 {
		let p = base.join("muted.png");
		return Some(build_macos_icon_from_path(&p));
	}

	// Round to nearest 5 and cap to 150
	let v = round_to_step(vol, 5.0, 0.0, 150.0);
	// Above 100, only 125 and 150 icons exist; fallback to previous available step
	let mapped = if v <= 100 {
		v
	} else if v < 125 {
		100
	} else if v < 150 {
		125
	} else {
		150
	};
	let p = base.join(format!("vol-{}.png", mapped));
	Some(build_macos_icon_from_path(&p))
}
fn resolve_macos_brightness_icon(percent: f64) -> Option<gtk::Image> {
	let base = icons_dir().join("brightness");
	let v = round_to_step(percent, 5.0, 0.0, 100.0);
	let p_svg = base.join(format!("brightness-{}.svg", v));
	if p_svg.exists() {
		return Some(build_macos_icon_from_path(&p_svg));
	}
	let p_png = base.join(format!("br-{}.png", v));
	Some(build_macos_icon_from_path(&p_png))
}

/// A window that our application can open that contains the main project view.
#[derive(Clone, Debug)]
pub struct SwayosdWindow {
	pub window: gtk::ApplicationWindow,
	pub monitor: gdk::Monitor,
	container: gtk::Box,
	timeout_id: Rc<RefCell<Option<glib::SourceId>>>,
}

// TODO: Use custom widget
// - Use start, center, and end children
//   - Always center the centered widget (both left and right sides are the same width)
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
		// Anchor to bottom edge for better reliability with rotated/transformed displays
		window.set_anchor(gtk_layer_shell::Edge::Bottom, true);

		// Set up the widgets (branch by theme)
		let container = match get_theme() {
			crate::config::Theme::MacOS => {
				window.set_width_request(MACOS_WINDOW_SIZE);
				window.set_height_request(MACOS_WINDOW_SIZE);
				let b = cascade! {
					gtk::Box::new(gtk::Orientation::Vertical, 0);
					..set_widget_name("container");
					..set_halign(gtk::Align::Center);
					..set_valign(gtk::Align::Center);
					..set_hexpand(true);
					..set_vexpand(true);
				};
				b
			}
			_ => {
				window.set_width_request(250);
				cascade! {
					gtk::Box::new(gtk::Orientation::Horizontal, 12);
					..set_widget_name("container");
				}
			}
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
			let mon_height = monitor.geometry().height() / window.scale_factor();
			// Calculate margin from bottom while preserving top_margin semantics:
			// top_margin=0.85 means window should be at 85% from top, which equals
			// 15% from bottom. By anchoring to bottom, we avoid issues with
			// window.allocated_height() being 0 or incorrect during initialization.
			let margin = (mon_height as f32 * (1.0 - get_top_margin())).round() as i32;
			window.set_margin(gtk_layer_shell::Edge::Bottom, margin);
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

		match get_theme() {
			crate::config::Theme::MacOS => {
				if let Some(icon) = resolve_macos_volume_icon(device) {
					self.container.append(&icon);
				}
			}
			_ => {
				let volume = volume_to_f64(&device.volume.avg());
				let icon_prefix = match device_type {
					VolumeDeviceType::Sink(_) => "sink",
					VolumeDeviceType::Source(_) => "source",
				};
				let icon_state = &match (device.mute, volume) {
					(true, _) => "muted",
					(_, 0.0) => "muted",
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
				let label = self.build_text_widget(Some(&format!("{}%", volume)), Some(4));

				progress.set_sensitive(!device.mute);

				self.container.append(&icon);
				self.container.append(&progress);
				if get_show_percentage() {
					self.container.append(&label);
				}
			}
		}

		self.run_timeout();
	}

	pub fn changed_brightness(&self, brightness_backend: &mut dyn BrightnessBackend) {
		self.clear_osd();

		match get_theme() {
			crate::config::Theme::MacOS => {
				let brightness = brightness_backend.get_current() as f64;
				let max = brightness_backend.get_max() as f64;
				let percent = (brightness / max * 100.).clamp(0.0, 100.0);
				if let Some(icon) = resolve_macos_brightness_icon(percent) {
					self.container.append(&icon);
				}
			}
			_ => {
				let icon_name = "display-brightness-symbolic";
				let icon = self.build_icon_widget(icon_name);

				let brightness = brightness_backend.get_current() as f64;
				let max = brightness_backend.get_max() as f64;
				let progress = self.build_progress_widget(brightness / max);
				let label = self.build_text_widget(
					Some(&format!("{}%", (brightness / max * 100.).round() as i32)),
					Some(4),
				);

				self.container.append(&icon);
				self.container.append(&progress);
				if get_show_percentage() {
					self.container.append(&label);
				}
			}
		}

		self.run_timeout();
	}

	pub fn changed_player(&self, icon: &str, label: Option<&str>) {
		self.clear_osd();

		let icon = self.build_icon_widget(icon);
		let label = self.build_text_widget(label, None);
		label.set_hexpand(true);

		self.container.append(&icon);
		self.container.append(&label);

		self.run_timeout();
	}

	pub fn changed_kbd_backlight(&self, value: u32, max: u32) {
		self.clear_osd();

		let value = value.min(max);

		let icon_name = match value {
			0 => "keyboard-brightness-off-symbolic",
			v if (v == max) => "keyboard-brightness-high-symbolic",
			_ => "keyboard-brightness-medium-symbolic",
		};
		let icon = self.build_icon_widget(icon_name);
		self.container.append(&icon);

		// A segmented progress bar looks cramped when there are too many segments
		if max < 5 {
			let progress = self.build_segmented_progress_widget(value, max);
			self.container.append(&progress);
		} else {
			let progress = self.build_progress_widget((value / max) as f64);
			self.container.append(&progress);
		}

		self.run_timeout();
	}

	pub fn changed_keylock(&self, key: KeysLocks, state: bool) {
		self.clear_osd();

		let label = self.build_text_widget(None, None);
		label.set_hexpand(true);

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
			let label = self.build_text_widget(Some(text.deref()), None);
			self.container.append(&label);
		}

		self.run_timeout();
	}

	pub fn custom_segmented_progress(
		&self,
		value: u32,
		n_segments: u32,
		text: Option<String>,
		icon_name: Option<&str>,
	) {
		self.clear_osd();

		if let Some(icon_name) = icon_name {
			let icon = self.build_icon_widget(icon_name);
			self.container.append(&icon);
		}

		let value = value.min(n_segments);
		let progress = self.build_segmented_progress_widget(value, n_segments);
		self.container.append(&progress);

		if let Some(text) = text {
			let label = self.build_text_widget(Some(text.deref()), None);
			self.container.append(&label);
		}

		self.run_timeout();
	}

	pub fn custom_message(&self, message: &str, icon_name: Option<&str>) {
		self.clear_osd();

		let label = self.build_text_widget(Some(message), None);
		label.set_hexpand(true);

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

	fn build_text_widget(&self, text: Option<&str>, min_chars: Option<u32>) -> gtk::Label {
		cascade! {
			gtk::Label::new(text);
			// width-chars is based off of the average font width, so we add 1
			// to make sure that it's wide enough.
			..set_width_chars(min_chars.map_or(-1, |v| (v + 1) as i32));
			..set_halign(gtk::Align::Center);
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

	fn build_segmented_progress_widget(
		&self,
		value: u32,
		n_segments: u32,
	) -> SegmentedProgressWidget {
		cascade! {
			SegmentedProgressWidget::new(n_segments);
			..set_value(value);
			..set_valign(gtk::Align::Center);
			..set_hexpand(true);
		}
	}
}
