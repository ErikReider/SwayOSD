use std::fmt;
use std::str::{self};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum ArgTypes {
	// should always be first to set a global variable before executing related functions
	DeviceName = isize::MIN,
	TopMargin = isize::MIN + 1,
	MaxVolume = isize::MIN + 2,
	CustomIcon = isize::MIN + 3,
	Player = isize::MIN + 4,
	MonitorName = isize::MIN + 5,
	CustomProgressText = isize::MIN + 6,
	// Other
	None = 0,
	CapsLock = 1,
	SinkVolumeRaise = 2,
	SinkVolumeLower = 3,
	SinkVolumeMuteToggle = 4,
	SourceVolumeRaise = 5,
	SourceVolumeLower = 6,
	SourceVolumeMuteToggle = 7,
	BrightnessRaise = 8,
	BrightnessLower = 9,
	BrightnessSet = 12,
	NumLock = 10,
	ScrollLock = 11,
	CustomMessage = 13,
	Playerctl = 14,
	CustomProgress = 15,
}

impl fmt::Display for ArgTypes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let string = match self {
			ArgTypes::None => "NONE",
			ArgTypes::CapsLock => "CAPSLOCK",
			ArgTypes::MaxVolume => "MAX-VOLUME",
			ArgTypes::SinkVolumeRaise => "SINK-VOLUME-RAISE",
			ArgTypes::SinkVolumeLower => "SINK-VOLUME-LOWER",
			ArgTypes::SinkVolumeMuteToggle => "SINK-VOLUME-MUTE-TOGGLE",
			ArgTypes::SourceVolumeRaise => "SOURCE-VOLUME-RAISE",
			ArgTypes::SourceVolumeLower => "SOURCE-VOLUME-LOWER",
			ArgTypes::SourceVolumeMuteToggle => "SOURCE-VOLUME-MUTE-TOGGLE",
			ArgTypes::BrightnessRaise => "BRIGHTNESS-RAISE",
			ArgTypes::BrightnessLower => "BRIGHTNESS-LOWER",
			ArgTypes::BrightnessSet => "BRIGHTNESS-SET",
			ArgTypes::NumLock => "NUM-LOCK",
			ArgTypes::ScrollLock => "SCROLL-LOCK",
			ArgTypes::DeviceName => "DEVICE-NAME",
			ArgTypes::TopMargin => "TOP-MARGIN",
			ArgTypes::CustomMessage => "CUSTOM-MESSAGE",
			ArgTypes::CustomIcon => "CUSTOM-ICON",
			ArgTypes::Playerctl => "PLAYERCTL",
			ArgTypes::Player => "PLAYER",
			ArgTypes::MonitorName => "MONITOR-NAME",
			ArgTypes::CustomProgress => "CUSTOM-PROGRESS",
			ArgTypes::CustomProgressText => "CUSTOM-PROGRESS-TEXT",
		};
		return write!(f, "{}", string);
	}
}

impl str::FromStr for ArgTypes {
	type Err = String;

	fn from_str(input: &str) -> Result<Self, Self::Err> {
		let result = match input {
			"CAPSLOCK" => ArgTypes::CapsLock,
			"SINK-VOLUME-RAISE" => ArgTypes::SinkVolumeRaise,
			"SINK-VOLUME-LOWER" => ArgTypes::SinkVolumeLower,
			"SINK-VOLUME-MUTE-TOGGLE" => ArgTypes::SinkVolumeMuteToggle,
			"SOURCE-VOLUME-RAISE" => ArgTypes::SourceVolumeRaise,
			"SOURCE-VOLUME-LOWER" => ArgTypes::SourceVolumeLower,
			"SOURCE-VOLUME-MUTE-TOGGLE" => ArgTypes::SourceVolumeMuteToggle,
			"BRIGHTNESS-RAISE" => ArgTypes::BrightnessRaise,
			"BRIGHTNESS-LOWER" => ArgTypes::BrightnessLower,
			"BRIGHTNESS-SET" => ArgTypes::BrightnessSet,
			"MAX-VOLUME" => ArgTypes::MaxVolume,
			"NUM-LOCK" => ArgTypes::NumLock,
			"SCROLL-LOCK" => ArgTypes::ScrollLock,
			"DEVICE-NAME" => ArgTypes::DeviceName,
			"TOP-MARGIN" => ArgTypes::TopMargin,
			"CUSTOM-MESSAGE" => ArgTypes::CustomMessage,
			"CUSTOM-ICON" => ArgTypes::CustomIcon,
			"PLAYERCTL" => ArgTypes::Playerctl,
			"PLAYER" => ArgTypes::Player,
			"MONITOR-NAME" => ArgTypes::MonitorName,
			"CUSTOM-PROGRESS" => ArgTypes::CustomProgress,
			"CUSTOM-PROGRESS-TEXT" => ArgTypes::CustomProgressText,
			other_type => return Err(other_type.to_owned()),
		};
		Ok(result)
	}
}
