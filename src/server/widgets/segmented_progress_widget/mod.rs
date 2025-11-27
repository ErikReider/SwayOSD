mod imp;
mod progress;

use gtk::glib::{self, Object};

glib::wrapper! {
	pub struct SegmentedProgressWidget(ObjectSubclass<imp::SegmentedProgressWidget>)
		@extends gtk::Widget,
		@implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SegmentedProgressWidget {
	pub fn new(n_segments: u32) -> Self {
		Object::builder().property("n-segments", n_segments).build()
	}
}

impl Default for SegmentedProgressWidget {
	fn default() -> Self {
		Self::new(5)
	}
}
