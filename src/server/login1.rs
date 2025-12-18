use zbus::{proxy, Connection};

#[proxy(
	default_service = "org.freedesktop.login1",
	default_path = "/org/freedesktop/login1",
	interface = "org.freedesktop.login1.Manager"
)]
pub trait Login1 {
	#[zbus(signal, name = "PrepareForSleep")]
	async fn prepare_for_sleep(&self, value: bool) -> zbus::Result<()>;
}

pub struct Login1 {}

impl Login1 {
	pub async fn init<'a>() -> zbus::Result<Login1Proxy<'a>> {
		let connection = Connection::system().await?;
		let proxy = Login1Proxy::builder(&connection).build().await?;

		Ok(proxy)
	}
}
