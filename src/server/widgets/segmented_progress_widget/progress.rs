use gtk::{
	glib::{self, Object, Properties},
	prelude::*,
	subclass::prelude::*,
	Orientable,
};
use std::cell::RefCell;

glib::wrapper! {
	pub struct SegmentWidget(ObjectSubclass<imp::SegmentWidget>)
		@extends gtk::Widget,
		@implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, Orientable;
}

impl SegmentWidget {
	pub fn new() -> Self {
		Object::new()
	}
}

mod imp {
	use super::*;

	#[derive(Properties, Debug, Default)]
	#[properties(wrapper_type = super::SegmentWidget)]
	pub struct SegmentWidget {
		#[property(set = Self::set_value)]
		active: RefCell<bool>,
	}

	impl SegmentWidget {
		fn set_value(&self, val: bool) {
			*self.active.borrow_mut() = val;
			if val {
				self.obj().set_css_classes(&["active"]);
			} else {
				self.obj().set_css_classes(&[]);
			}
		}
	}

	#[glib::object_subclass]
	impl ObjectSubclass for SegmentWidget {
		const NAME: &'static str = "SegmentWidget";
		type Type = super::SegmentWidget;
		type ParentType = gtk::Widget;

		fn class_init(klass: &mut Self::Class) {
			klass.set_layout_manager_type::<gtk::BinLayout>();
			klass.set_css_name("segment");
		}
	}

	#[glib::derived_properties]
	impl ObjectImpl for SegmentWidget {
		fn constructed(&self) {
			self.parent_constructed();
			self.obj().set_vexpand(true);
			self.obj().set_hexpand(true);
		}
	}

	impl WidgetImpl for SegmentWidget {}
}
