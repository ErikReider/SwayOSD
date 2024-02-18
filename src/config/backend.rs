use gtk::glib::system_config_dirs;
use serde_derive::Deserialize;
use std::error::Error;
use std::path::PathBuf;

#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct InputBackendConfig {}

#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct BackendConfig {
	#[serde(default)]
	pub input: InputBackendConfig,
}

fn find_backend_config() -> Option<PathBuf> {
	for path in system_config_dirs() {
		let path = path.join("swayosd").join("backend.toml");
		if path.exists() {
			return Some(path);
		}
	}

	None
}

pub fn read_backend_config() -> Result<BackendConfig, Box<dyn Error>> {
	let path = match find_backend_config() {
		Some(path) => path,
		None => return Ok(Default::default()),
	};

	let config_file = std::fs::read_to_string(path)?;
	let config: BackendConfig = toml::from_str(&config_file)?;
	Ok(config)
}
