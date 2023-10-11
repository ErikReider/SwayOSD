#[cfg(feature = "blight")]
mod blight;

#[cfg(feature = "brightnessctl")]
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

pub fn get_preferred_backend(device_name: Option<String>) -> BrightnessBackendResult {
    #[cfg(feature = "blight")]
    return blight::Blight::try_new_boxed(device_name);

    #[cfg(feature = "brightnessctl")]
    return brightnessctl::BrightnessCtl::try_new_boxed(device_name);
}
