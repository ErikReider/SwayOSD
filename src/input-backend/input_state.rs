use crate::config::backend::InputBackendConfig;
use evdev_rs::enums::EV_KEY;
use input::event::tablet_pad::KeyState;
use std::collections::HashSet;

#[derive(Default)]
pub struct InputState {
	pressed_shift_keys: HashSet<(String, EV_KEY)>,
	suppressed_caps_lock_keys: HashSet<String>,
}

impl InputState {
	pub fn add_device(&mut self, device_id: &str, pressed_keys: impl IntoIterator<Item = EV_KEY>) {
		self.remove_device(device_id);
		self.pressed_shift_keys.extend(
			pressed_keys
				.into_iter()
				.filter(|key| matches!(key, EV_KEY::KEY_LEFTSHIFT | EV_KEY::KEY_RIGHTSHIFT))
				.map(|key| (device_id.to_owned(), key)),
		);
	}

	pub fn handle_key_event(
		&mut self,
		input_config: &InputBackendConfig,
		device_id: &str,
		ev_key: EV_KEY,
		key_state: KeyState,
	) -> bool {
		if matches!(ev_key, EV_KEY::KEY_LEFTSHIFT | EV_KEY::KEY_RIGHTSHIFT) {
			let shift_key = (device_id.to_owned(), ev_key);
			match key_state {
				KeyState::Pressed => self.pressed_shift_keys.insert(shift_key),
				KeyState::Released => self.pressed_shift_keys.remove(&shift_key),
			};
			return false;
		}

		if ev_key != EV_KEY::KEY_CAPSLOCK {
			return key_state == KeyState::Released;
		}

		if input_config.ignore_caps_lock_key.unwrap_or(false) {
			if key_state == KeyState::Released {
				self.suppressed_caps_lock_keys.remove(device_id);
			}
			return false;
		}

		match key_state {
			KeyState::Pressed => {
				self.suppressed_caps_lock_keys.remove(device_id);
				if input_config
					.ignore_caps_lock_key_without_shift
					.unwrap_or(false)
					&& self.pressed_shift_keys.is_empty()
				{
					self.suppressed_caps_lock_keys.insert(device_id.to_owned());
				}
				false
			}
			KeyState::Released => !self.suppressed_caps_lock_keys.remove(device_id),
		}
	}

	pub fn remove_device(&mut self, device_id: &str) {
		self.pressed_shift_keys
			.retain(|(pressed_device_id, _)| pressed_device_id != device_id);
		self.suppressed_caps_lock_keys.remove(device_id);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Scenario {
		config: InputBackendConfig,
		state: InputState,
	}

	impl Scenario {
		fn conditional_filter() -> Self {
			Self {
				config: InputBackendConfig {
					ignore_caps_lock_key: Some(false),
					ignore_caps_lock_key_without_shift: Some(true),
				},
				state: InputState::default(),
			}
		}

		fn event(&mut self, device_id: &str, key: EV_KEY, state: KeyState) -> bool {
			self.state
				.handle_key_event(&self.config, device_id, key, state)
		}

		fn caps_lock(&mut self, device_id: &str) -> bool {
			assert!(!self.event(device_id, EV_KEY::KEY_CAPSLOCK, KeyState::Pressed));
			self.event(device_id, EV_KEY::KEY_CAPSLOCK, KeyState::Released)
		}
	}

	#[test]
	fn plain_caps_lock_is_suppressed() {
		let mut scenario = Scenario::conditional_filter();

		assert!(!scenario.caps_lock("keyboard"));
	}

	#[test]
	fn caps_lock_with_either_shift_is_forwarded() {
		for shift in [EV_KEY::KEY_LEFTSHIFT, EV_KEY::KEY_RIGHTSHIFT] {
			let mut scenario = Scenario::conditional_filter();
			scenario.event("keyboard", shift, KeyState::Pressed);

			assert!(scenario.caps_lock("keyboard"));
		}
	}

	#[test]
	fn releasing_one_of_two_shift_keys_keeps_shift_active() {
		let mut scenario = Scenario::conditional_filter();
		scenario.event("keyboard", EV_KEY::KEY_LEFTSHIFT, KeyState::Pressed);
		scenario.event("keyboard", EV_KEY::KEY_RIGHTSHIFT, KeyState::Pressed);
		scenario.event("keyboard", EV_KEY::KEY_LEFTSHIFT, KeyState::Released);

		assert!(scenario.caps_lock("keyboard"));
	}

	#[test]
	fn shift_release_before_caps_release_still_forwards() {
		let mut scenario = Scenario::conditional_filter();
		scenario.event("keyboard", EV_KEY::KEY_LEFTSHIFT, KeyState::Pressed);
		assert!(!scenario.event("keyboard", EV_KEY::KEY_CAPSLOCK, KeyState::Pressed));
		scenario.event("keyboard", EV_KEY::KEY_LEFTSHIFT, KeyState::Released);

		assert!(scenario.event("keyboard", EV_KEY::KEY_CAPSLOCK, KeyState::Released));
	}

	#[test]
	fn shift_press_after_caps_press_does_not_change_suppression() {
		let mut scenario = Scenario::conditional_filter();
		assert!(!scenario.event("keyboard", EV_KEY::KEY_CAPSLOCK, KeyState::Pressed));
		scenario.event("keyboard", EV_KEY::KEY_LEFTSHIFT, KeyState::Pressed);

		assert!(!scenario.event("keyboard", EV_KEY::KEY_CAPSLOCK, KeyState::Released));
	}

	#[test]
	fn shift_on_another_keyboard_is_recognized() {
		let mut scenario = Scenario::conditional_filter();
		scenario.event("external", EV_KEY::KEY_LEFTSHIFT, KeyState::Pressed);

		assert!(scenario.caps_lock("built-in"));
	}

	#[test]
	fn added_keyboard_initializes_held_shift() {
		let mut scenario = Scenario::conditional_filter();
		scenario
			.state
			.add_device("keyboard", [EV_KEY::KEY_LEFTSHIFT]);

		assert!(scenario.caps_lock("keyboard"));
	}

	#[test]
	fn removed_keyboard_does_not_leave_shift_pressed() {
		let mut scenario = Scenario::conditional_filter();
		scenario.event("external", EV_KEY::KEY_LEFTSHIFT, KeyState::Pressed);
		scenario.state.remove_device("external");

		assert!(!scenario.caps_lock("built-in"));
	}

	#[test]
	fn disabled_filter_forwards_plain_caps_lock() {
		let mut scenario = Scenario {
			config: InputBackendConfig::default(),
			state: InputState::default(),
		};

		assert!(scenario.caps_lock("keyboard"));
	}

	#[test]
	fn full_suppression_takes_precedence() {
		let mut scenario = Scenario {
			config: InputBackendConfig {
				ignore_caps_lock_key: Some(true),
				ignore_caps_lock_key_without_shift: Some(false),
			},
			state: InputState::default(),
		};
		scenario.event("keyboard", EV_KEY::KEY_LEFTSHIFT, KeyState::Pressed);

		assert!(!scenario.caps_lock("keyboard"));
	}
}
