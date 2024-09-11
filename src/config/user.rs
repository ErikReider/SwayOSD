use gtk::glib::system_config_dirs;
use gtk::glib::user_config_dir;
use serde_derive::Deserialize;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {}

#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
	pub style: Option<PathBuf>,
	pub top_margin: Option<f32>,
	pub max_volume: Option<u8>,
	pub show_percentage: Option<bool>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
	#[serde(default)]
	pub server: ServerConfig,
	#[serde(default)]
	pub client: ClientConfig,
}

fn find_user_config() -> Option<PathBuf> {
	let path = user_config_dir().join("swayosd").join("config.toml");
	if path.exists() {
		return Some(path);
	}

	for path in system_config_dirs() {
		let path = path.join("swayosd").join("config.toml");
		if path.exists() {
			return Some(path);
		}
	}

	None
}

pub fn read_user_config(path: Option<&Path>) -> Result<UserConfig, Box<dyn Error>> {
	let path = match path.map(Path::to_owned).or_else(find_user_config) {
		Some(path) => path,
		None => return Ok(Default::default()),
	};

	let config_file = std::fs::read_to_string(path)?;
	let config: UserConfig = toml::from_str(&config_file)?;
	Ok(config)
}
