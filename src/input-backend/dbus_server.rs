use zbus::object_server::SignalEmitter;
use zbus::{connection, interface, Connection};

use crate::config::{DBUS_BACKEND_NAME, DBUS_PATH};

pub struct DbusServer;

#[interface(name = "org.erikreider.swayosd")]
impl DbusServer {
	#[zbus(signal)]
	pub async fn key_pressed(
		signal_ctxt: &SignalEmitter<'_>,
		key_code: u16,
		state: i32,
	) -> zbus::Result<()>;
}

impl DbusServer {
	async fn get_connection(&self) -> zbus::Result<Connection> {
		let conn = connection::Builder::system()?
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
