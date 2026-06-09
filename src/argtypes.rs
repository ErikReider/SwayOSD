use std::fmt;
use std::str::{self};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum ArgTypes {
	CapsLock,
	SinkVolumeRaise,
	SinkVolumeLower,
	SinkVolumeMuteToggle,
	SinkVolumeMute,
	SinkVolumeUnMute,
	SourceVolumeRaise,
	SourceVolumeLower,
	SourceVolumeMuteToggle,
	SourceVolumeMute,
	SourceVolumeUnMute,
	BrightnessRaise,
	BrightnessLower,
	BrightnessSet,
	NumLock,
	ScrollLock,
	CustomMessage,
	Playerctl,
	CustomProgress,
	CustomSegmentedProgress,
	KbdBacklight,
}

impl fmt::Display for ArgTypes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let string = match self {
			ArgTypes::CapsLock => "CAPSLOCK",
			ArgTypes::SinkVolumeRaise => "SINK-VOLUME-RAISE",
			ArgTypes::SinkVolumeLower => "SINK-VOLUME-LOWER",
			ArgTypes::SinkVolumeMuteToggle => "SINK-VOLUME-MUTE-TOGGLE",
			ArgTypes::SinkVolumeMute => "SINK-VOLUME-MUTE",
			ArgTypes::SinkVolumeUnMute => "SINK-VOLUME-UNMUTE",
			ArgTypes::SourceVolumeRaise => "SOURCE-VOLUME-RAISE",
			ArgTypes::SourceVolumeLower => "SOURCE-VOLUME-LOWER",
			ArgTypes::SourceVolumeMuteToggle => "SOURCE-VOLUME-MUTE-TOGGLE",
			ArgTypes::SourceVolumeMute => "SOURCE-VOLUME-MUTE",
			ArgTypes::SourceVolumeUnMute => "SOURCE-VOLUME-UNMUTE",
			ArgTypes::BrightnessRaise => "BRIGHTNESS-RAISE",
			ArgTypes::BrightnessLower => "BRIGHTNESS-LOWER",
			ArgTypes::BrightnessSet => "BRIGHTNESS-SET",
			ArgTypes::NumLock => "NUM-LOCK",
			ArgTypes::ScrollLock => "SCROLL-LOCK",
			ArgTypes::CustomMessage => "CUSTOM-MESSAGE",
			ArgTypes::Playerctl => "PLAYERCTL",
			ArgTypes::CustomProgress => "CUSTOM-PROGRESS",
			ArgTypes::CustomSegmentedProgress => "CUSTOM-SEGMENTED-PROGRESS",
			ArgTypes::KbdBacklight => "KBD-BACKLIGHT",
		};
		write!(f, "{}", string)
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
			"SINK-VOLUME-MUTE" => ArgTypes::SinkVolumeMute,
			"SINK-VOLUME-UNMUTE" => ArgTypes::SinkVolumeUnMute,
			"SOURCE-VOLUME-RAISE" => ArgTypes::SourceVolumeRaise,
			"SOURCE-VOLUME-LOWER" => ArgTypes::SourceVolumeLower,
			"SOURCE-VOLUME-MUTE-TOGGLE" => ArgTypes::SourceVolumeMuteToggle,
			"SOURCE-VOLUME-MUTE" => ArgTypes::SourceVolumeMute,
			"SOURCE-VOLUME-UNMUTE" => ArgTypes::SourceVolumeUnMute,
			"BRIGHTNESS-RAISE" => ArgTypes::BrightnessRaise,
			"BRIGHTNESS-LOWER" => ArgTypes::BrightnessLower,
			"BRIGHTNESS-SET" => ArgTypes::BrightnessSet,
			"NUM-LOCK" => ArgTypes::NumLock,
			"SCROLL-LOCK" => ArgTypes::ScrollLock,
			"CUSTOM-MESSAGE" => ArgTypes::CustomMessage,
			"PLAYERCTL" => ArgTypes::Playerctl,
			"CUSTOM-PROGRESS" => ArgTypes::CustomProgress,
			"CUSTOM-SEGMENTED-PROGRESS" => ArgTypes::CustomSegmentedProgress,
			"KBD-BACKLIGHT" => ArgTypes::KbdBacklight,
			other_type => return Err(other_type.to_owned()),
		};
		Ok(result)
	}
}
