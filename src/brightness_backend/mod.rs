use std::error::Error;

#[cfg(feature = "blight")]
mod blight_backend;

pub type BrightnessOperationResult<A> = Result<A, Box<dyn Error>>;
pub type BrightnessBackendResult = BrightnessOperationResult<Box<dyn BrightnessBackend>>;

pub trait BrightnessBackendConstructor: BrightnessBackend + Sized + 'static {
    fn try_new(device_name: Option<String>) -> BrightnessOperationResult<Self>;

    fn try_new_boxed(device_name: Option<String>) -> BrightnessBackendResult {
        let backend = Self::try_new(device_name);
        match backend {
            Ok(backend) => Ok(Box::new(backend)),
            Err(e) => Err(e),
        }
    }
}

pub trait BrightnessBackend {
    fn get_current(&self) -> u32;
    fn get_max(&self) -> u32;

    fn lower(&self, by: u32) -> BrightnessOperationResult<()>;
    fn raise(&self, by: u32) -> BrightnessOperationResult<()>;
    fn set(&self, val: u32) -> BrightnessOperationResult<()>;
}

pub fn get_preferred_backend(device_name: Option<String>) -> BrightnessBackendResult {
    #[cfg(feature = "blight")]
    blight_backend::Blight::try_new_boxed(device_name)
}
