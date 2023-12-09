use super::{BrightnessBackend, BrightnessBackendConstructor};

const EXPECT_STR: &str = "VirtualDevice didn't test the command during initialization";

use anyhow::bail;
use std::{error::Error, process::Command, str::FromStr};
use thiserror::Error;

enum CliArg<'arg> {
	Simple(&'arg str),
	KeyValue { key: &'arg str, value: &'arg str },
}

impl<'arg> From<&'arg str> for CliArg<'arg> {
	fn from(value: &'arg str) -> Self {
		CliArg::Simple(value)
	}
}

impl<'arg> From<(&'arg str, &'arg str)> for CliArg<'arg> {
	fn from((key, value): (&'arg str, &'arg str)) -> Self {
		CliArg::KeyValue { key, value }
	}
}

#[derive(Default)]
struct VirtualDevice {
	name: Option<String>,
	current: Option<u32>,
	max: Option<u32>,
}

pub(super) struct BrightnessCtl {
	device: VirtualDevice,
}

#[derive(Error, Debug)]
#[error("Requested device '{device_name}' does not exist ")]
pub struct DeviceDoesntExistError {
	device_name: String,
}

impl VirtualDevice {
	fn try_new(device_name: Option<String>) -> anyhow::Result<Self> {
		let s = Self {
			name: device_name.clone(),
			..Default::default()
		};

		// Check if the command is available to us before running it in other occasions
		let exit_code = s.command(CliArg::Simple("info")).output()?.status;

		if exit_code.success() {
			Ok(s)
		} else {
			bail!(DeviceDoesntExistError {
				device_name: device_name.unwrap()
			})
		}
	}

	fn command(&self, arg: CliArg) -> Command {
		let mut cmd = Command::new("brightnessctl");

		if let Some(name) = &self.name {
			cmd.arg("--device").arg(name);
		}

		match arg {
			CliArg::Simple(arg) => cmd.arg(arg),
			CliArg::KeyValue { key, value } => cmd.arg(key).arg(value),
		};

		cmd
	}

	fn run<'arg, T: FromStr, A: Into<CliArg<'arg>>>(&self, arg: A) -> anyhow::Result<T>
	where
		<T as FromStr>::Err: Error + Send + Sync + 'static,
	{
		let cmd_output = self.command(arg.into()).output()?.stdout;

		let cmd_output = String::from_utf8_lossy(&cmd_output);

		Ok(cmd_output.trim().parse()?)
	}

	fn get_current(&mut self) -> u32 {
		match self.current {
			Some(val) => val,
			None => {
				let val = self.run("get").expect(EXPECT_STR);
				self.current = Some(val);
				val
			}
		}
	}

	fn get_max(&mut self) -> u32 {
		match self.max {
			Some(val) => val,
			None => {
				let val = self.run("max").expect(EXPECT_STR);
				self.max = Some(val);
				val
			}
		}
	}

	fn set_percent(&mut self, mut val: u32) -> anyhow::Result<()> {
		val = val.clamp(0, 100);
		self.current = self.max.map(|max| val * max / 100);
		let _: String = self.run(("set", &*format!("{val}%")))?;
		Ok(())
	}
}

impl BrightnessBackendConstructor for BrightnessCtl {
	fn try_new(device_name: Option<String>) -> anyhow::Result<Self> {
		Ok(Self {
			device: VirtualDevice::try_new(device_name)?,
		})
	}
}

impl BrightnessBackend for BrightnessCtl {
	fn get_current(&mut self) -> u32 {
		self.device.get_current()
	}

	fn get_max(&mut self) -> u32 {
		self.device.get_max()
	}

	fn lower(&mut self, by: u32) -> anyhow::Result<()> {
		let curr = self.get_current();
		let max = self.get_max();

		let curr = curr * 100 / max;

		self.device.set_percent(curr.saturating_sub(by))
	}

	fn raise(&mut self, by: u32) -> anyhow::Result<()> {
		let curr = self.get_current();
		let max = self.get_max();

		let curr = curr * 100 / max;

		self.device.set_percent(curr + by)
	}

	fn set(&mut self, val: u32) -> anyhow::Result<()> {
		self.device.set_percent(val)
	}
}
