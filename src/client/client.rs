use zbus::{dbus_proxy, blocking::Connection};

#[dbus_proxy(
	interface = "org.erikreider.swayosd",
	default_service = "org.erikreider.swayosd-server",
	default_path = "/org/erikreider/swayosd"
)]
trait Server {
	async fn handle_action(&self, arg_type: String, data: String) -> zbus::Result<bool>;
}

pub fn get_proxy() -> zbus::Result<ServerProxyBlocking<'static>> {
	let connection = Connection::session()?;
	Ok(ServerProxyBlocking::new(&connection)?)
}
