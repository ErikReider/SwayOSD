use self::pulsectl::PulseAudio;
use self::wireplumber::WirePlumber;

mod pulsectl;
mod wireplumber;

pub type AudioBackendResult = anyhow::Result<Box<dyn AudioBackend>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioDeviceType {
	Sink,   // Output
	Source, // Input
}

#[derive(Clone, Debug)]
pub struct AudioDeviceInfo {
	pub volume: f64, // 0.0 - 100.0+
	pub mute: bool,
}

pub trait AudioBackendConstructor: AudioBackend + Sized + 'static {
	fn try_new(device_type: AudioDeviceType, device_name: Option<String>) -> anyhow::Result<Self>;

	fn try_new_boxed(device_type: AudioDeviceType, device_name: Option<String>) -> AudioBackendResult {
		let backend = Self::try_new(device_type, device_name);
		match backend {
			Ok(backend) => Ok(Box::new(backend)),
			Err(e) => Err(e),
		}
	}
}

pub trait AudioBackend {
	fn get_device_info(&mut self) -> anyhow::Result<AudioDeviceInfo>;
	fn set_volume(&mut self, delta: f64, max_volume: u8) -> anyhow::Result<AudioDeviceInfo>;
	fn toggle_mute(&mut self) -> anyhow::Result<AudioDeviceInfo>;
}

pub fn get_preferred_backend(
	device_type: AudioDeviceType,
	device_name: Option<String>,
) -> AudioBackendResult {
	// Try WirePlumber first (native PipeWire)
	println!("Trying WirePlumber Backend...");
	WirePlumber::try_new_boxed(device_type, device_name.clone()).or_else(|e| {
		println!("...WirePlumber failed: {e}! Falling back to PulseAudio");
		PulseAudio::try_new_boxed(device_type, device_name)
	})
}
