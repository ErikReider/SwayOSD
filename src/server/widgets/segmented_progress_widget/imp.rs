use std::{cell::RefCell, collections::LinkedList};

use gtk::{
	glib::{self, Properties},
	prelude::*,
	subclass::prelude::*,
};

use crate::widgets::segmented_progress_widget::progress::SegmentWidget;

#[derive(Properties, Debug, Default)]
#[properties(wrapper_type = super::SegmentedProgressWidget)]
pub struct SegmentedProgressWidget {
	#[property(get, set = Self::set_n_segments)]
	n_segments: RefCell<u32>,

	#[property(get, set = Self::set_value)]
	value: RefCell<u32>,

	children: RefCell<LinkedList<SegmentWidget>>,
}

impl SegmentedProgressWidget {
	fn set_n_segments(&self, new_n_segments: u32) {
		{
			*self.n_segments.borrow_mut() = new_n_segments;

			// Make sure that the value isn't larger
			let mut value = self.value.borrow_mut();
			*value = value.min(new_n_segments);
		}

		self.obj().update_property(&[
			gtk::accessible::Property::ValueMin(0_f64),
			gtk::accessible::Property::ValueMax(new_n_segments as f64),
		]);

		self.update_children();
	}

	fn set_value(&self, new_value: u32) {
		{
			let n_segments = *self.n_segments.borrow();
			let value = new_value.min(n_segments);

			*self.value.borrow_mut() = value;

			self.obj()
				.update_property(&[gtk::accessible::Property::ValueNow(value as f64)]);
		}

		self.update_children();
	}

	fn update_children(&self) {
		let n_segments = *self.n_segments.borrow();
		let value = *self.value.borrow();
		let mut children = self.children.borrow_mut();

		if children.len() as u32 == n_segments {
			// Update the state of all segments
			for (i, segment) in children.iter().enumerate() {
				segment.set_active((i as u32) < value);
			}
		} else if children.len() as u32 != n_segments {
			// Remove all previous segments
			while let Some(segment) = children.pop_front() {
				segment.unparent();
			}
			// Add the new number of segments
			for i in 0..n_segments {
				let segment = cascade! {
					SegmentWidget::new();
					..set_active(i < value);
					..set_parent(&*self.obj());
				};
				children.push_back(segment);
			}
		}
	}
}

#[glib::object_subclass]
impl ObjectSubclass for SegmentedProgressWidget {
	const NAME: &'static str = "SegmentedProgressWidget";
	type Type = super::SegmentedProgressWidget;
	type ParentType = gtk::Widget;

	fn class_init(klass: &mut Self::Class) {
		klass.set_layout_manager_type::<gtk::BoxLayout>();
		klass.set_css_name("segmentedprogress");
		klass.set_accessible_role(gtk::AccessibleRole::ProgressBar);
	}
}

#[glib::derived_properties]
impl ObjectImpl for SegmentedProgressWidget {
	fn constructed(&self) {
		self.parent_constructed();

		self.update_children();
	}

	fn dispose(&self) {
		// Remove all children
		let mut children = self.children.borrow_mut();
		while let Some(segment) = children.pop_front() {
			segment.unparent();
		}
	}
}

impl WidgetImpl for SegmentedProgressWidget {}
