use zbus::{dbus_interface, Connection, ConnectionBuilder, SignalContext};

use crate::config::{DBUS_BACKEND_NAME, DBUS_PATH};

pub struct DbusServer;

#[dbus_interface(name = "org.erikreider.swayosd")]
impl DbusServer {
	#[dbus_interface(signal)]
	pub async fn key_pressed(
		signal_ctxt: &SignalContext<'_>,
		key_code: u16,
		state: i32,
	) -> zbus::Result<()>;
}

impl DbusServer {
	async fn get_connection(&self) -> zbus::Result<Connection> {
		let conn = ConnectionBuilder::system()?
			.name(DBUS_BACKEND_NAME)?
			.serve_at(DBUS_PATH, DbusServer)?
			.build()
			.await?;

		Ok(conn)
	}

	pub async fn init(&self) -> Connection {
		match self.get_connection().await {
			Ok(conn) => conn,
			Err(error) => {
				eprintln!("Error: {}", error);
				std::process::exit(1)
			}
		}
	}
}
