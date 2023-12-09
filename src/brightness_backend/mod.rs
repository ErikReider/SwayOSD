use self::{blight::Blight, brightnessctl::BrightnessCtl};

mod blight;

mod brightnessctl;

pub type BrightnessBackendResult = anyhow::Result<Box<dyn BrightnessBackend>>;

pub trait BrightnessBackendConstructor: BrightnessBackend + Sized + 'static {
	fn try_new(device_name: Option<String>) -> anyhow::Result<Self>;

	fn try_new_boxed(device_name: Option<String>) -> BrightnessBackendResult {
		let backend = Self::try_new(device_name);
		match backend {
			Ok(backend) => Ok(Box::new(backend)),
			Err(e) => Err(e),
		}
	}
}

pub trait BrightnessBackend {
	fn get_current(&mut self) -> u32;
	fn get_max(&mut self) -> u32;

	fn lower(&mut self, by: u32) -> anyhow::Result<()>;
	fn raise(&mut self, by: u32) -> anyhow::Result<()>;
	fn set(&mut self, val: u32) -> anyhow::Result<()>;
}

#[allow(dead_code)]
pub fn get_preferred_backend(device_name: Option<String>) -> BrightnessBackendResult {
	println!("Trying BrightnessCtl Backend...");
	BrightnessCtl::try_new_boxed(device_name.clone()).or_else(|_| {
		println!("...Command failed! Falling back to Blight");
		Blight::try_new_boxed(device_name)
	})
}
