use zbus::{proxy, Connection};

#[proxy(
	default_service = "org.freedesktop.UPower",
	default_path = "/org/freedesktop/UPower/KbdBacklight",
	interface = "org.freedesktop.UPower.KbdBacklight"
)]
pub trait KbdBacklight {
	#[zbus(signal, name = "BrightnessChangedWithSource")]
	async fn brightness_changed_with_source(&self, value: i32, source: String) -> zbus::Result<()>;

	#[zbus(name = "GetMaxBrightness")]
	async fn get_max_brightness(&self) -> zbus::Result<i32>;
}

pub struct KbdBacklight {}

impl KbdBacklight {
	pub async fn init<'a>() -> zbus::Result<KbdBacklightProxy<'a>> {
		let connection = Connection::system().await?;
		let proxy = KbdBacklightProxy::builder(&connection).build().await?;

		Ok(proxy)
	}
}
