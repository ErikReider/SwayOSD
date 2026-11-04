use std::collections::HashMap;
use std::ffi::CStr;
use xkbregistry::*;

/// Error types for xkbregistry operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum XkbregistryError {
	#[error("xkbregistry context creation error")]
	CreateContext,
	#[error("xkbregistry parse error")]
	Parse,
}

pub struct KbdLayoutDescRegistry {
	name_description_table: HashMap<String, String>,
}

impl KbdLayoutDescRegistry {
	pub fn create() -> Result<Self, XkbregistryError> {
		let mut name_description_table = HashMap::new();
		unsafe {
			let context = rxkb_context_new(0);

			if context.is_null() {
				return Err(XkbregistryError::CreateContext);
			}

			if !rxkb_context_parse_default_ruleset(context) {
				rxkb_context_unref(context);
				return Err(XkbregistryError::Parse);
			}

			let mut layout = rxkb_layout_first(context);
			while !layout.is_null() {
				let cstr_name = rxkb_layout_get_name(layout);
				let cstr_description = rxkb_layout_get_description(layout);

				if !cstr_name.is_null() && !cstr_description.is_null() {
					let name = CStr::from_ptr(cstr_name).to_string_lossy().to_string();
					let description = CStr::from_ptr(cstr_description)
						.to_string_lossy()
						.to_string();
					// The first result is usually the desired one, so we have to check if there's
					// already an entry under the same name to keep what was put there first
					name_description_table
						.entry(name)
						.or_insert_with(|| description);
				}

				layout = rxkb_layout_next(layout);
			}

			rxkb_context_unref(context);
		}
		Ok(Self {
			name_description_table,
		})
	}

	/// Gets descriptions of layouts from their name
	///
	/// A "description" in xkbd is a human readable name for the layout,
	/// e.g. if layout is "us" its "description" would be "English (US)"
	pub fn description_from_name(&self, name: &str) -> String {
		self.name_description_table
			.get(name)
			.cloned()
			.unwrap_or(name.to_string())
	}
}
