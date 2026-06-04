use std::process::Command;

use anyhow::{bail, Context};

use super::{AudioBackend, AudioBackendConstructor, AudioDeviceInfo, AudioDeviceType};

const DEFAULT_SINK: &str = "@DEFAULT_AUDIO_SINK@";
const DEFAULT_SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";

pub struct WirePlumber {
	device_type: AudioDeviceType,
	device_name: Option<String>,
}

impl AudioBackendConstructor for WirePlumber {
	fn try_new(device_type: AudioDeviceType, device_name: Option<String>) -> anyhow::Result<Self> {
		let backend = Self {
			device_type,
			device_name,
		};

		// Check if wpctl is available and WirePlumber is running
		let output = backend
			.wpctl_command()
			.arg("status")
			.output()
			.context("Failed to run wpctl status")?;

		if !output.status.success() {
			bail!("wpctl status failed - WirePlumber may not be running");
		}

		Ok(backend)
	}
}

impl WirePlumber {
	fn wpctl_command(&self) -> Command {
		Command::new("wpctl")
	}

	fn get_device_target(&self) -> String {
		if let Some(ref name) = self.device_name {
			name.clone()
		} else {
			match self.device_type {
				AudioDeviceType::Sink => DEFAULT_SINK.to_string(),
				AudioDeviceType::Source => DEFAULT_SOURCE.to_string(),
			}
		}
	}

	fn parse_volume_output(output: &str) -> anyhow::Result<f64> {
		// Output format: "Volume: 0.45" or "Volume: 0.45 [MUTED]"
		for line in output.lines() {
			let line = line.trim();
			if let Some(vol_str) = line.strip_prefix("Volume:") {
				let vol_str = vol_str.trim();
				// Take only the numeric part (before any brackets or spaces)
				let vol_str = vol_str.split_whitespace().next().unwrap_or("0");
				let volume: f64 = vol_str.parse().context("Failed to parse volume")?;
				// Convert from 0.0-1.0 to 0-100
				return Ok(volume * 100.0);
			}
		}
		bail!("Could not parse volume from wpctl output: {}", output)
	}

	fn parse_mute_status(output: &str) -> bool {
		// Check if output contains [MUTED]
		output.contains("[MUTED]")
	}
}

impl AudioBackend for WirePlumber {
	fn get_device_info(&mut self) -> anyhow::Result<AudioDeviceInfo> {
		let target = self.get_device_target();

		let output = self
			.wpctl_command()
			.args(["get-volume", &target])
			.output()
			.context("Failed to run wpctl get-volume")?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			bail!("wpctl get-volume failed: {}", stderr);
		}

		let stdout = String::from_utf8_lossy(&output.stdout);
		let volume = Self::parse_volume_output(&stdout)?;
		let mute = Self::parse_mute_status(&stdout);

		Ok(AudioDeviceInfo { volume, mute })
	}

	fn set_volume(&mut self, delta: f64, max_volume: u8) -> anyhow::Result<AudioDeviceInfo> {
		let target = self.get_device_target();
		let current = self.get_device_info()?;
		let max_vol = max_volume as f64;

		// Calculate new volume with clamping
		let new_volume = (current.volume + delta).clamp(0.0, max_vol);

		// wpctl uses 0.0-1.0 range
		let vol_arg = format!("{:.3}", new_volume / 100.0);

		let output = self
			.wpctl_command()
			.args(["set-volume", &target, &vol_arg])
			.output()
			.context("Failed to run wpctl set-volume")?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			bail!("wpctl set-volume failed: {}", stderr);
		}

		self.get_device_info()
	}

	fn toggle_mute(&mut self) -> anyhow::Result<AudioDeviceInfo> {
		let target = self.get_device_target();

		let output = self
			.wpctl_command()
			.args(["set-mute", &target, "toggle"])
			.output()
			.context("Failed to run wpctl set-mute")?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			bail!("wpctl set-mute toggle failed: {}", stderr);
		}

		self.get_device_info()
	}
}
