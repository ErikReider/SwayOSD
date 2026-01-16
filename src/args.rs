use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help(false))]
pub struct ArgsServer {
	/// Use a custom config file instead of looking for one.
	#[arg(long, value_name = "Config File Path")]
	pub config: Option<PathBuf>,

	/// Use a custom Stylesheet file instead of looking for one
	#[arg(long, short, value_name = "CSS File Path")]
	pub style: Option<PathBuf>,

	/// OSD margin from top edge (0.5 would be screen center). Default is 0.85
	#[arg(long, value_name = "from 0.0 to 1.0")]
	pub top_margin: Option<String>,

	/// OSD display duration in milliseconds (200-60000)
	#[arg(long, short = 'd', value_name = "200-60000", default_value = "1000", value_parser = clap::value_parser!(u64).range(200..=60000))]
	pub duration: u64,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help(true))]
pub struct ArgsClient {
	/// Use a custom config file instead of looking for one.
	#[arg(long, value_name = "Config File Path")]
	pub config: Option<PathBuf>,

	/// Which monitor to display osd on
	#[arg(long, value_name = "Monitor identifier (e.g., HDMI-A-1, DP-1)")]
	pub monitor: Option<String>,

	/// Shows capslock osd. Note: Doesn't toggle CapsLock, just displays the status
	#[arg(long, default_value_t = false)]
	pub caps_lock: bool,

	/// Shows capslock osd. Uses LED class NAME.
	/// Note: Doesn't toggle CapsLock, just displays the status
	#[arg(long, value_name = "LED class name (/sys/class/leds/NAME)")]
	pub caps_lock_led: Option<String>,

	/// Shows numlock osd. Note: Doesn't toggle NumLock, just displays the status
	#[arg(long, default_value_t = false)]
	pub num_lock: bool,

	/// Shows numlock osd. Uses LED class NAME.
	/// Note: Doesn't toggle NumLock, just displays the status
	#[arg(long, value_name = "LED class name (/sys/class/leds/NAME)")]
	pub num_lock_led: Option<String>,

	/// Shows scrolllock osd. Note: Doesn't toggle ScrollLock, just displays the status
	#[arg(long, default_value_t = false)]
	pub scroll_lock: bool,

	/// Shows scrolllock osd. Uses LED class NAME.
	/// Note: Doesn't toggle ScrollLock, just displays the status",
	#[arg(long, value_name = "LED class name (/sys/class/leds/NAME)")]
	pub scroll_lock_led: Option<String>,

	/// Shows volume osd and raises, loweres or mutes default sink volume
	#[arg(
		long,
		value_name = "raise|lower|mute-toggle|(±)number",
		allow_negative_numbers = true
	)]
	pub output_volume: Option<String>,

	/// Shows volume osd and raises, loweres or mutes default source volume
	#[arg(
		long,
		value_name = "raise|lower|mute-toggle|(±)number",
		allow_negative_numbers = true
	)]
	pub input_volume: Option<String>,

	/// Sets the maximum Volume
	#[arg(long, value_name = "(+)number")]
	pub max_volume: Option<String>,

	/// For which device to increase/decrease audio/brightness.
	/// Can be wildcard for brightness.
	/// Ex: (pactl list short sinks|sources)/(brightnessctl -l -c backlight).
	#[arg(long, value_name = "Pulseaudio/BrightnessCtl device name")]
	pub device: Option<String>,

	/// Shows brightness osd and raises or loweres all available sources of brightness device
	#[arg(
		long,
		value_name = "raise|lower|(±)number",
		allow_negative_numbers = true
	)]
	pub brightness: Option<String>,

	/// Sets the minimum Brightness
	#[arg(long, value_name = "(+)number")]
	pub min_brightness: Option<String>,

	/// Shows Playerctl osd and runs the playerctl command
	#[arg(long, value_name = "play-pause|play|pause|stop|next|prev|shuffle")]
	pub playerctl: Option<String>,

	/// For which player to run the playerctl commands
	#[arg(long, value_name = "auto|all|(playerctl -l)")]
	pub player: Option<String>,

	/// Message to display
	#[arg(long, value_name = "text")]
	pub custom_message: Option<String>,

	/// Icon to display when using custom-message/custom-progress.
	/// Icon name is from Freedesktop specification
	/// (https://specifications.freedesktop.org/icon-naming-spec/latest/)
	#[arg(long, value_name = "Icon name")]
	pub custom_icon: Option<String>,

	/// Progress to display (0.0 <-> 1.0)
	#[arg(long, value_name = "Progress from 0.0 to 1.0")]
	pub custom_progress: Option<String>,

	/// Segmented progress to display (value:num-segments). Ex: 2:4
	#[arg(long, value_name = "Progress from 0 to num-segments")]
	pub custom_segmented_progress: Option<String>,

	/// Text to display when using custom-progress or custom-segmented-progress
	#[arg(long, value_name = "Progress text")]
	pub custom_progress_text: Option<String>,
}
