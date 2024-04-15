use gtk::glib::{variant::DictEntry, Variant};

use crate::argtypes::ArgTypes;

pub enum HandleLocalStatus {
	FAILURE = 1,
	SUCCESS = 0,
	CONTINUE = -1,
}

pub(crate) fn handle_application_args(
	variant: Variant,
) -> (HandleLocalStatus, Vec<(ArgTypes, Option<String>)>) {
	let mut actions: Vec<(ArgTypes, Option<String>)> = Vec::new();

	if variant.n_children() == 0 {
		return (HandleLocalStatus::CONTINUE, actions);
	}

	if !variant.is_container() {
		eprintln!("VariantDict isn't a container!...");
		return (HandleLocalStatus::FAILURE, actions);
	}

	for i in 0..variant.n_children() {
		let child: DictEntry<String, Variant> = variant.child_get(i);

		let (option, value): (ArgTypes, Option<String>) = match child.key().as_str() {
			"caps-lock" => (ArgTypes::CapsLock, None),
			"num-lock" => (ArgTypes::NumLock, None),
			"scroll-lock" => (ArgTypes::ScrollLock, None),
			"caps-lock-led" => match child.value().str() {
				Some(led) => (ArgTypes::CapsLock, Some(led.to_owned())),
				None => {
					eprintln!("Value for caps-lock-led isn't a string!...");
					return (HandleLocalStatus::FAILURE, actions);
				}
			},
			"num-lock-led" => match child.value().str() {
				Some(led) => (ArgTypes::NumLock, Some(led.to_owned())),
				None => {
					eprintln!("Value for num-lock-led isn't a string!...");
					return (HandleLocalStatus::FAILURE, actions);
				}
			},
			"scroll-lock-led" => match child.value().str() {
				Some(led) => (ArgTypes::ScrollLock, Some(led.to_owned())),
				None => {
					eprintln!("Value for scroll-lock-led isn't a string!...");
					return (HandleLocalStatus::FAILURE, actions);
				}
			},
			"output-volume" => {
				let value = child.value().str().unwrap_or("");
				let parsed = volume_parser(false, value);
				match parsed {
					Ok(p) => p,
					Err(_) => return (HandleLocalStatus::FAILURE, actions),
				}
			}
			"input-volume" => {
				let value = child.value().str().unwrap_or("");
				let parsed = volume_parser(true, value);
				match parsed {
					Ok(p) => p,
					Err(_) => return (HandleLocalStatus::FAILURE, actions),
				}
			}
			"brightness" => {
				let value = child.value().str().unwrap_or("");

				match (value, value.parse::<i8>()) {
					// Parse custom step values
					(_, Ok(num)) => match value.get(..1) {
						Some("+") => (ArgTypes::BrightnessRaise, Some(num.to_string())),
						Some("-") => (ArgTypes::BrightnessLower, Some(num.abs().to_string())),
						_ => (ArgTypes::BrightnessSet, Some(num.to_string())),
					},

					("raise", _) => (ArgTypes::BrightnessRaise, None),
					("lower", _) => (ArgTypes::BrightnessLower, None),
					(e, _) => {
						eprintln!("Unknown brightness mode: \"{}\"!...", e);
						return (HandleLocalStatus::FAILURE, actions);
					}
				}
			}
			"max-volume" => {
				let value = child.value().str().unwrap_or("").trim();
				match value.parse::<u8>() {
					Ok(_) => (ArgTypes::MaxVolume, Some(value.to_string())),
					Err(_) => {
						eprintln!("{} is not a number between 0 and {}!", value, u8::MAX);
						return (HandleLocalStatus::FAILURE, actions);
					}
				}
			}
			"device" => {
				let value = match child.value().str() {
					Some(v) => v.to_string(),
					None => {
						eprintln!("--device found but no name given");
						return (HandleLocalStatus::FAILURE, actions);
					}
				};
				(ArgTypes::DeviceName, Some(value))
			}
			"top-margin" => {
				let value = child.value().str().unwrap_or("").trim();
				match value.parse::<f32>() {
					Ok(top_margin) if (0.0f32..=1.0f32).contains(&top_margin) => {
						(ArgTypes::TopMargin, Some(value.to_string()))
					}
					_ => {
						eprintln!("{} is not a number between 0.0 and 1.0!", value);
						return (HandleLocalStatus::FAILURE, actions);
					}
				}
			}
			"style" => continue,
			e => {
				eprintln!("Unknown Variant Key: \"{}\"!...", e);
				return (HandleLocalStatus::FAILURE, actions);
			}
		};
		if option != ArgTypes::None {
			actions.push((option, value));
		}
	}

	// sort actions so that they always get executed in the correct order
	if actions.len() > 0 {
		for i in 0..actions.len() - 1 {
			for j in i + 1..actions.len() {
				if actions[i].0 > actions[j].0 {
					let temp = actions[i].clone();
					actions[i] = actions[j].clone();
					actions[j] = temp;
				}
			}
		}
	}
	(HandleLocalStatus::SUCCESS, actions)
}

fn volume_parser(is_sink: bool, value: &str) -> Result<(ArgTypes, Option<String>), i32> {
	let mut v = match (value, value.parse::<i8>()) {
		// Parse custom step values
		(_, Ok(num)) => (
			if num.is_positive() {
				ArgTypes::SinkVolumeRaise
			} else {
				ArgTypes::SinkVolumeLower
			},
			Some(num.abs().to_string()),
		),
		("raise", _) => (ArgTypes::SinkVolumeRaise, None),
		("lower", _) => (ArgTypes::SinkVolumeLower, None),
		("mute-toggle", _) => (ArgTypes::SinkVolumeMuteToggle, None),
		(e, _) => {
			eprintln!("Unknown output volume mode: \"{}\"!...", e);
			return Err(1);
		}
	};
	if is_sink {
		if v.0 == ArgTypes::SinkVolumeRaise {
			v.0 = ArgTypes::SourceVolumeRaise;
		} else if v.0 == ArgTypes::SinkVolumeLower {
			v.0 = ArgTypes::SourceVolumeLower;
		} else {
			v.0 = ArgTypes::SourceVolumeMuteToggle;
		}
	}
	Ok(v)
}
