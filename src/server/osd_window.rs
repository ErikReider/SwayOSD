use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration; // new imports

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
		get_max_volume, get_show_percentage, get_top_margin, volume_to_f64, KeysLocks,
		VolumeDeviceType,
	},
};
use gtk_layer_shell::LayerShell;

const ICON_SIZE: i32 = 32;

/// Type of progress bar to display
#[derive(Clone, Debug)]
enum ProgressType {
	Normal(f64),
	Segmented(u32, u32),
}

/// Holds all optional content and flags for the OSD window.
#[derive(Clone, Debug, Default)]
struct ContentSlots {
	icon_name: Option<String>,
	label_text: Option<String>,
	progress_type: Option<ProgressType>,
	percentage_text: Option<String>,

	// Style flags
	label_hexpand: bool,
	icon_sensitive: bool,
	progress_sensitive: bool,
	apply_icon_margin_to_label: bool,

	// Character width hints
	label_min_chars: Option<u32>,
	percentage_min_chars: Option<u32>,
}

/// A window that our application can open that contains the main project view.
#[derive(Clone, Debug)]
pub struct SwayosdWindow {
	pub window: gtk::ApplicationWindow,
	pub monitor: gdk::Monitor,
	container: gtk::Box,
	timeout_id: Rc<RefCell<Option<glib::SourceId>>>,
	slots: Arc<Mutex<ContentSlots>>, // changed to Arc<Mutex>
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
		// Anchor to bottom edge for better reliability with rotated/transformed displays
		window.set_anchor(gtk_layer_shell::Edge::Bottom, true);

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
			slots: Arc::new(Mutex::new(ContentSlots::default())), // initialize with Mutex
		}
	}

	pub fn close(&self) {
		self.window.close();
	}

	pub fn changed_volume(&self, device: &DeviceInfo, device_type: &VolumeDeviceType) {
		// Reset slots by assigning a new default under lock
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
		}

		let volume = volume_to_f64(&device.volume.avg());
		let icon_prefix = match device_type {
			VolumeDeviceType::Sink(_) => "sink",
			VolumeDeviceType::Source(_) => "source",
		};
		let icon_state = match (device.mute, volume) {
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
		let icon_name = format!("{}-volume-{}-symbolic", icon_prefix, icon_state);

		{
			let mut slots = self.slots.lock().unwrap();
			slots.icon_name = Some(icon_name);
			let max_volume: f64 = get_max_volume().into();
			slots.progress_type = Some(ProgressType::Normal(volume / max_volume));
			if get_show_percentage() {
				slots.percentage_text = Some(format!("{}%", volume));
				slots.percentage_min_chars = Some(4);
			}
			slots.progress_sensitive = !device.mute;
		}

		self.run_timeout();
	}

	pub fn changed_brightness(&self, brightness_backend: &mut dyn BrightnessBackend) {
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
		}

		let brightness = brightness_backend.get_current() as f64;
		let max = brightness_backend.get_max() as f64;

		{
			let mut slots = self.slots.lock().unwrap();
			slots.icon_name = Some("display-brightness-symbolic".to_string());
			slots.progress_type = Some(ProgressType::Normal(brightness / max));
			if get_show_percentage() {
				slots.percentage_text =
					Some(format!("{}%", (brightness / max * 100.).round() as i32));
				slots.percentage_min_chars = Some(4);
			}
		}

		self.run_timeout();
	}

	pub fn changed_player(&self, icon: &str, label: Option<&str>) {
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
			slots.icon_name = Some(icon.to_string());
			slots.label_text = label.map(|s| s.to_string());
			slots.label_hexpand = true;
		}

		self.run_timeout();
	}

	pub fn changed_kbd_backlight(&self, value: u32, max: u32) {
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
		}

		let value = value.min(max);
		let icon_name = match value {
			0 => "keyboard-brightness-off-symbolic",
			v if v == max => "keyboard-brightness-high-symbolic",
			_ => "keyboard-brightness-medium-symbolic",
		};

		{
			let mut slots = self.slots.lock().unwrap();
			slots.icon_name = Some(icon_name.to_string());
			if max < 5 {
				slots.progress_type = Some(ProgressType::Segmented(value, max));
			} else {
				slots.progress_type = Some(ProgressType::Normal(value as f64 / max as f64));
			}
		}

		self.run_timeout();
	}

	pub fn changed_keylock(&self, key: KeysLocks, state: bool) {
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
		}

		let on_off_text = if state { "On" } else { "Off" };
		let (label_text, symbol) = match key {
			KeysLocks::CapsLock => (format!("Caps Lock {}", on_off_text), "caps-lock-symbolic"),
			KeysLocks::NumLock => (format!("Num Lock {}", on_off_text), "num-lock-symbolic"),
			KeysLocks::ScrollLock => (
				format!("Scroll Lock {}", on_off_text),
				"scroll-lock-symbolic",
			),
		};

		{
			let mut slots = self.slots.lock().unwrap();
			slots.icon_name = Some(symbol.to_string());
			slots.label_text = Some(label_text);
			slots.label_hexpand = true;
			slots.icon_sensitive = state;
		}

		self.run_timeout();
	}

	pub fn custom_progress(&self, fraction: f64, text: Option<String>, icon_name: Option<&str>) {
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
			slots.icon_name = icon_name.map(|s| s.to_string());
			slots.progress_type = Some(ProgressType::Normal(fraction.clamp(0.0, 1.0)));
			slots.label_text = text;
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
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
		}

		let value = value.min(n_segments);
		{
			let mut slots = self.slots.lock().unwrap();
			slots.icon_name = icon_name.map(|s| s.to_string());
			slots.progress_type = Some(ProgressType::Segmented(value, n_segments));
			slots.label_text = text;
		}

		self.run_timeout();
	}

	pub fn custom_message(&self, message: &str, icon_name: Option<&str>) {
		{
			let mut slots = self.slots.lock().unwrap();
			*slots = ContentSlots::default();
			slots.label_text = Some(message.to_string());
			slots.label_hexpand = true;
			if let Some(icon) = icon_name {
				slots.icon_name = Some(icon.to_string());
				slots.apply_icon_margin_to_label = true;
			}
		}

		self.run_timeout();
	}

	/// Remove all children from the container.
	fn clear_osd(&self) {
		let mut next = self.container.first_child();
		while let Some(widget) = next {
			next = widget.next_sibling();
			self.container.remove(&widget);
		}
	}

	/// Build the UI from current slots, show the window, and schedule hiding.
	fn run_timeout(&self) {
		// Cancel any existing timeout
		if let Some(timeout_id) = self.timeout_id.take() {
			timeout_id.remove()
		}

		// Clear previous content
		self.clear_osd();

		// Borrow slots – lock and read
		let slots = self.slots.lock().unwrap();

		// Build icon
		let icon_widget = slots.icon_name.as_ref().map(|name| {
			let icon = self.build_icon_widget(name);
			icon.set_sensitive(slots.icon_sensitive);
			icon
		});

		// Build label and progress into a vertical box if either exists
		let mut vbox: Option<gtk::Box> = None;
		let mut label_widget: Option<gtk::Label> = None; // Stored for margin callback

		if slots.label_text.is_some() || slots.progress_type.is_some() {
			let vbox_child = gtk::Box::new(gtk::Orientation::Vertical, 6);
			vbox_child.set_hexpand(true);
			vbox_child.set_valign(gtk::Align::Center);

			if let Some(text) = &slots.label_text {
				let label = self.build_text_widget(Some(text), slots.label_min_chars);
				label.set_hexpand(slots.label_hexpand);
				label_widget = Some(label.clone());
				vbox_child.append(&label);
			}

			if let Some(progress_type) = &slots.progress_type {
				let progress: gtk::Widget = match progress_type {
					ProgressType::Normal(fraction) => {
						self.build_progress_widget(*fraction).upcast()
					}
					ProgressType::Segmented(value, n_segments) => self
						.build_segmented_progress_widget(*value, *n_segments)
						.upcast(),
				};
				progress.set_sensitive(slots.progress_sensitive);
				vbox_child.append(&progress);
			}

			vbox = Some(vbox_child);
		}

		// Build percentage label
		let percentage_widget = slots
			.percentage_text
			.as_ref()
			.map(|text| self.build_text_widget(Some(text), slots.percentage_min_chars));

		// Add widgets to container in order: icon, vertical box, percentage
		if let Some(icon) = &icon_widget {
			self.container.append(icon);
		}
		if let Some(vbox) = &vbox {
			self.container.append(vbox);
		}
		if let Some(percentage) = &percentage_widget {
			self.container.append(percentage);
		}

		// Apply special margin for custom_message if needed
		if slots.apply_icon_margin_to_label
			&& let (Some(icon), Some(label)) = (icon_widget.as_ref(), label_widget.as_ref()) {
				let box_spacing = self.container.spacing();
				let _icon_clone = icon.clone();
				let label_clone = label.clone();
				// If icon is already realized, set margin immediately; otherwise wait for realize.
				if icon.is_realized() {
					let margin = icon.allocation().width()
						+ icon.margin_start()
						+ icon.margin_end()
						+ box_spacing;
					label.set_margin_end(margin);
				} else {
					icon.connect_realize(move |icon| {
						let margin = icon.allocation().width()
							+ icon.margin_start() + icon.margin_end()
							+ box_spacing;
						label_clone.set_margin_end(margin);
					});
				}
			}

		// Release the lock before scheduling the timeout
		drop(slots);

		// Show window and schedule hide
		self.window.show();
		let s = self.clone();
		self.timeout_id.replace(Some(glib::timeout_add_local_once(
			Duration::from_millis(1000),
			move || {
				s.window.hide();
				s.timeout_id.replace(None);
			},
		)));
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
