use std::fmt;
use std::str::{self};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum ArgFlags {
	DeviceName,
	MaxVolume,
	CustomIcon,
	Player,
	MonitorName,
	CustomProgressText,
	MinBrightness,
	Duration,
}

impl fmt::Display for ArgFlags {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let string = match self {
			ArgFlags::MaxVolume => "MAX-VOLUME",
			ArgFlags::DeviceName => "DEVICE-NAME",
			ArgFlags::CustomIcon => "CUSTOM-ICON",
			ArgFlags::Player => "PLAYER",
			ArgFlags::MonitorName => "MONITOR-NAME",
			ArgFlags::CustomProgressText => "CUSTOM-PROGRESS-TEXT",
			ArgFlags::MinBrightness => "MIN-BRIGHTNESS",
			ArgFlags::Duration => "DURATION",
		};
		write!(f, "{}", string)
	}
}

impl str::FromStr for ArgFlags {
	type Err = String;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		let result = match input {
			"MAX-VOLUME" => ArgFlags::MaxVolume,
			"DEVICE-NAME" => ArgFlags::DeviceName,
			"CUSTOM-ICON" => ArgFlags::CustomIcon,
			"PLAYER" => ArgFlags::Player,
			"MONITOR-NAME" => ArgFlags::MonitorName,
			"CUSTOM-PROGRESS-TEXT" => ArgFlags::CustomProgressText,
			"MIN-BRIGHTNESS" => ArgFlags::MinBrightness,
			"DURATION" => ArgFlags::Duration,
			other_type => return Err(other_type.to_owned()),
		};
		Ok(result)
	}
}
