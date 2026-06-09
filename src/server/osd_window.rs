use gtk::{
	gdk,
	glib::{self, clone},
	prelude::*,
};
use gtk_layer_shell::LayerShell;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::utils::{volume_to_f64, KeysLocks};
use crate::widgets::segmented_progress_widget::SegmentedProgressWidget;
use crate::{
	actions::{
		brightness_backend::BrightnessBackend,
		pulse::{DeviceInfo, DeviceKind},
	},
	application::ActionOptions,
};

const ICON_SIZE: i32 = 32;

/// A window that our application can open that contains the main project view.
#[derive(Clone, Debug)]
pub struct SwayosdWindow {
	pub window: gtk::ApplicationWindow,
	pub monitor: gdk::Monitor,
	container: gtk::Box,
	timeout_id: Rc<RefCell<Option<glib::SourceId>>>,

	top_margin: f32,
}

// TODO: Use custom widget
// - Use start, center, and end children
//   - Always center the centered widget (both left and right sides are the same width)
impl SwayosdWindow {
	/// Create a new window and assign it to the given application.
	pub fn new(app: &gtk::Application, monitor: &gdk::Monitor, top_margin: &f32) -> Self {
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

		let osd_window = Self {
			window: window.clone(),
			container,
			monitor: monitor.clone(),
			timeout_id: Rc::new(RefCell::new(None)),

			top_margin: *top_margin,
		};

		// Set the window margin
		osd_window.update_margins(&window, monitor);
		// Ensure window margin is updated when necessary
		window.connect_scale_factor_notify(clone!(
			#[strong]
			osd_window,
			#[weak]
			monitor,
			move |window| osd_window.update_margins(window, &monitor)
		));
		monitor.connect_scale_factor_notify(clone!(
			#[strong]
			osd_window,
			#[weak]
			window,
			move |monitor| osd_window.update_margins(&window, monitor)
		));
		monitor.connect_geometry_notify(clone!(
			#[strong]
			osd_window,
			#[weak]
			window,
			move |monitor| osd_window.update_margins(&window, monitor)
		));

		osd_window
	}

	fn update_margins(&self, window: &gtk::ApplicationWindow, monitor: &gdk::Monitor) {
		let mon_height = monitor.geometry().height();
		// Calculate margin from bottom while preserving top_margin semantics:
		// top_margin=0.85 means window should be at 85% from top, which equals
		// 15% from bottom. By anchoring to bottom, we avoid issues with
		// window.allocated_height() being 0 or incorrect during initialization.
		let margin = (mon_height as f32 * (1.0 - self.top_margin)).round() as i32;
		window.set_margin(gtk_layer_shell::Edge::Bottom, margin);
	}

	pub fn close(&self) {
		self.window.close();
	}

	pub fn changed_volume(
		&self,
		action_options: &ActionOptions,
		device: &DeviceInfo,
	) {
		let max_volume: f64 = (*action_options.max_volume.get()).into();
		let show_percentage = action_options.show_percentage.get();
		let duration = action_options.duration.get();

		self.clear_osd();

		let volume = volume_to_f64(&device.volume.avg());
		let icon_prefix = match device.kind {
			DeviceKind::Sink => "sink",
			DeviceKind::Source => "source",
		};
		let icon_state = &match (device.mute, volume) {
			(true, _) => "muted",
			(_, 0.0) => "muted",
			(false, x) if x > 0.0 && x <= 33.0 => "low",
			(false, x) if x > 33.0 && x <= 66.0 => "medium",
			(false, x) if x > 66.0 && x <= 100.0 => "high",
			(false, x) if x > 100.0 => match device.kind {
				DeviceKind::Sink => "high",
				DeviceKind::Source => "overamplified",
			},
			(_, _) => "high",
		};
		let icon_name = format!("{}-volume-{}-symbolic", icon_prefix, icon_state);

		let icon = self.build_icon_widget(&icon_name);
		let progress = self.build_progress_widget(volume / max_volume);
		let label = self.build_text_widget(&Some(format!("{}%", volume)), Some(4));

		progress.set_sensitive(!device.mute);

		self.container.append(&icon);
		self.container.append(&progress);
		if *show_percentage {
			self.container.append(&label);
		}

		self.run_timeout(duration);
	}

	pub fn changed_brightness(
		&self,
		action_options: &ActionOptions,
		brightness_backend: &mut dyn BrightnessBackend,
	) {
		let show_percentage = action_options.show_percentage.get();
		let duration = action_options.duration.get();

		self.clear_osd();

		let icon_name = "display-brightness-symbolic";
		let icon = self.build_icon_widget(icon_name);

		let brightness = brightness_backend.get_current() as f64;
		let max = brightness_backend.get_max() as f64;
		let progress = self.build_progress_widget(brightness / max);
		let label = self.build_text_widget(
			&Some(format!("{}%", (brightness / max * 100.).round() as i32)),
			Some(4),
		);

		self.container.append(&icon);
		self.container.append(&progress);
		if *show_percentage {
			self.container.append(&label);
		}

		self.run_timeout(duration);
	}

	pub fn changed_player(
		&self,
		action_options: &ActionOptions,
		icon: &str,
		label: &Option<String>,
	) {
		let duration = action_options.duration.get();

		self.clear_osd();

		let icon = self.build_icon_widget(icon);
		let label = self.build_text_widget(label, None);
		label.set_hexpand(true);

		self.container.append(&icon);
		self.container.append(&label);

		self.run_timeout(duration);
	}

	pub fn changed_kbd_backlight(&self, action_options: &ActionOptions, value: u32, max: u32) {
		let duration = action_options.duration.get();

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
			let progress = self.build_progress_widget(value as f64 / max as f64);
			self.container.append(&progress);
		}

		self.run_timeout(duration);
	}

	pub fn changed_keylock(&self, action_options: &ActionOptions, key: KeysLocks, state: bool) {
		let duration = action_options.duration.get();

		self.clear_osd();

		let label = self.build_text_widget(&None, None);
		label.set_hexpand(true);

		let on_off_text = match state {
			true => "On",
			false => "Off",
		};

		let (label_text, symbol) = match key {
			KeysLocks::CapsLock => {
				let text = format!("Caps Lock {on_off_text}");
				let symbol = "caps-lock-symbolic";
				(text, symbol)
			}
			KeysLocks::NumLock => {
				let text = format!("Num Lock {on_off_text}");
				let symbol = "num-lock-symbolic";
				(text, symbol)
			}
			KeysLocks::ScrollLock => {
				let text = format!("Scroll Lock  {on_off_text}");
				let symbol = "scroll-lock-symbolic";
				(text, symbol)
			}
		};

		label.set_text(&label_text);
		let icon = self.build_icon_widget(symbol);

		icon.set_sensitive(state);

		self.container.append(&icon);
		self.container.append(&label);

		self.run_timeout(duration);
	}

	pub fn custom_progress(&self, action_options: &ActionOptions, fraction: f64) {
		let duration = action_options.duration.get();
		let icon_name = action_options.icon_name.get();
		let progress_text = action_options.progress_text.get();

		self.clear_osd();

		if let Some(icon_name) = icon_name {
			let icon = self.build_icon_widget(icon_name);
			self.container.append(&icon);
		}

		let progress = self.build_progress_widget(fraction.clamp(0.0, 1.0));
		self.container.append(&progress);

		if progress_text.is_some() {
			let label = self.build_text_widget(progress_text, None);
			self.container.append(&label);
		}

		self.run_timeout(duration);
	}

	pub fn custom_segmented_progress(
		&self,
		action_options: &ActionOptions,
		value: u32,
		n_segments: u32,
	) {
		let duration = action_options.duration.get();
		let icon_name = action_options.icon_name.get();
		let progress_text = action_options.progress_text.get();

		self.clear_osd();

		if let Some(icon_name) = icon_name {
			let icon = self.build_icon_widget(icon_name);
			self.container.append(&icon);
		}

		let value = value.min(n_segments);
		let progress = self.build_segmented_progress_widget(value, n_segments);
		self.container.append(&progress);

		if progress_text.is_some() {
			let label = self.build_text_widget(progress_text, None);
			self.container.append(&label);
		}

		self.run_timeout(duration);
	}

	pub fn custom_message(&self, action_options: &ActionOptions, message: &String) {
		let icon_name = action_options.icon_name.get();
		let duration = action_options.duration.get();

		self.clear_osd();

		let label = self.build_text_widget(&Some(message.into()), None);
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

		self.run_timeout(duration);
	}

	/// Clear all container children
	fn clear_osd(&self) {
		let mut next = self.container.first_child();
		while let Some(widget) = next {
			next = widget.next_sibling();
			self.container.remove(&widget);
		}
	}

	fn run_timeout(&self, duration: &u64) {
		// Hide window after timeout
		if let Some(timeout_id) = self.timeout_id.take() {
			timeout_id.remove()
		}
		self.timeout_id.replace(Some(glib::timeout_add_local_once(
			Duration::from_millis(*duration),
			clone!(
				#[strong(rename_to = this)]
				self,
				move || {
					this.window.hide();
					this.timeout_id.replace(None);
				}
			),
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

	fn build_text_widget(&self, text: &Option<String>, min_chars: Option<u32>) -> gtk::Label {
		cascade! {
			gtk::Label::new(text.as_deref());
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
