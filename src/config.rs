#![allow(dead_code)]

#[path = "config/backend.rs"]
pub mod backend;
#[path = "config/user.rs"]
pub mod user;

pub const DBUS_PATH: &str = "/org/erikreider/swayosd";
pub const DBUS_BACKEND_NAME: &str = "org.erikreider.swayosd";
pub const DBUS_SERVER_NAME: &str = "org.erikreider.swayosd-server";

pub const APPLICATION_NAME: &str = "org.erikreider.swayosd";
