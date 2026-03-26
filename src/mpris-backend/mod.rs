use playerctld::PlayerctldProxyBlocking;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

use super::config::user::ServerConfig;
use crate::utils::get_player as get_player_raw;
use std::{error::Error, sync::Arc, thread::sleep, time::Duration};
use PlayerctlAction::*;

pub enum PlayerctlAction {
	PlayPause,
	Play,
	Pause,
	Stop,
	Next,
	Prev,
	Shuffle,
}

#[derive(Clone, Debug)]
pub enum PlayerctlDeviceRaw {
	None,
	All,
	Some(String),
	Shift,
	Unshift,
}

// A thin zbus-based wrapper around an MPRIS player bus name.
pub struct MprisPlayer {
	bus_name: String,
	conn: Connection,
}

impl MprisPlayer {
	fn new(bus_name: String) -> Result<Self, Box<dyn Error>> {
		let conn = Connection::session()?;
		Ok(Self { bus_name, conn })
	}

	fn player_proxy(&self) -> Result<Proxy<'_>, Box<dyn Error>> {
		Ok(Proxy::new(
			&self.conn,
			self.bus_name.as_str(),
			"/org/mpris/MediaPlayer2",
			"org.mpris.MediaPlayer2.Player",
		)?)
	}

	fn call(&self, method: &str) -> Result<(), Box<dyn Error>> {
		self.player_proxy()?.call_method(method, &())?;
		Ok(())
	}

	fn get_property_string(&self, prop: &str) -> Result<String, Box<dyn Error>> {
		let proxy = self.player_proxy()?;
		let val: Value = proxy.get_property(prop)?;
		Ok(value_to_string(val))
	}

	fn get_property_bool(&self, prop: &str) -> Result<bool, Box<dyn Error>> {
		let proxy = self.player_proxy()?;
		let val: Value = proxy.get_property(prop)?;
		match val {
			Value::Bool(b) => Ok(b),
			Value::Value(inner) => match *inner {
				Value::Bool(b) => Ok(b),
				_ => Err("not a bool".into()),
			},
			_ => Err("not a bool".into()),
		}
	}

	fn set_property_bool(&self, prop: &str, val: bool) -> Result<(), Box<dyn Error>> {
		self.player_proxy()?.set_property(prop, val)?;
		Ok(())
	}

	fn get_playback_status(&self) -> String {
		self.get_property_string("PlaybackStatus").unwrap_or_default()
	}

	fn get_metadata_map(&self) -> Option<std::collections::HashMap<String, String>> {
		let proxy = self.player_proxy().ok()?;
		let val: Value = proxy.get_property("Metadata").ok()?;
		let mut map = std::collections::HashMap::new();
		if let Value::Dict(dict) = unwrap_value(val) {
			for (k, v) in dict.iter() {
				let key = value_to_string(k.try_clone().ok()?);
				let val_str = value_to_string(v.clone());
				map.insert(key, val_str);
			}
		}
		Some(map)
	}
}

fn unwrap_value(val: Value) -> Value {
	if let Value::Value(inner) = val {
		unwrap_value(*inner)
	} else {
		val
	}
}

fn value_to_string(val: Value) -> String {
	match unwrap_value(val) {
		Value::Str(s) => s.to_string(),
		Value::Array(arr) => arr
			.iter()
			.map(|v| value_to_string(v.clone()))
			.collect::<Vec<_>>()
			.join(", "),
		other => format!("{:?}", other),
	}
}

pub enum PlayerctlDevice {
	All(Vec<MprisPlayer>),
	Some(MprisPlayer),
}

pub struct Playerctl {
	player: PlayerctlDevice,
	action: PlayerctlAction,
	pub icon: Option<String>,
	pub label: Option<String>,
	fmt_str: Option<String>,
}

fn get_player(player: PlayerctlDeviceRaw) -> Result<PlayerctlDevice, Box<dyn Error>> {
	fn get_playerctld<'a>() -> Result<PlayerctldProxyBlocking<'a>, Box<dyn Error>> {
		Ok(PlayerctldProxyBlocking::new(&Connection::session()?)?)
	}

	fn get_playerctld_devices() -> Result<Vec<String>, Box<dyn Error>> {
		Ok(get_playerctld()?.player_names()?)
	}

	fn get_single_player(bus_name: String) -> Result<PlayerctlDevice, Box<dyn Error>> {
		Ok(PlayerctlDevice::Some(MprisPlayer::new(bus_name)?))
	}

	fn get_all_players() -> Result<PlayerctlDevice, Box<dyn Error>> {
		let names = get_playerctld_devices()?;
		let players: Vec<MprisPlayer> = names
			.into_iter()
			.filter_map(|n| MprisPlayer::new(n).ok())
			.collect();
		if players.is_empty() {
			return Err("No players found".into());
		}
		Ok(PlayerctlDevice::All(players))
	}

	match player {
		PlayerctlDeviceRaw::None => {
			let Ok(players) = get_playerctld_devices() else {
				return Err("playerctld not available and no player specified".into());
			};
			let Some(first) = players.into_iter().next() else {
				return Err("No players found".into());
			};
			get_single_player(first)
		}
		PlayerctlDeviceRaw::Some(name) => {
			if name.starts_with("org.mpris.") {
				get_single_player(name)
			} else {
				let names = get_playerctld_devices()?;
				let matched = names.into_iter().find(|n| n.contains(&name));
				match matched {
					Some(bus) => get_single_player(bus),
					None => get_single_player(format!("org.mpris.MediaPlayer2.{}", name)),
				}
			}
		}
		PlayerctlDeviceRaw::All => get_all_players(),
		PlayerctlDeviceRaw::Shift => get_single_player(get_playerctld()?.shift()?),
		PlayerctlDeviceRaw::Unshift => get_single_player(get_playerctld()?.unshift()?),
	}
}

impl Playerctl {
	pub fn new(
		action: PlayerctlAction,
		config: Arc<ServerConfig>,
	) -> Result<Playerctl, Box<dyn Error>> {
		let player = get_player_raw();
		let player = get_player(player)?;
		let fmt_str = config.playerctl_format.clone();
		Ok(Self {
			player,
			action,
			icon: None,
			label: None,
			fmt_str,
		})
	}

	pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
		let mut metadata = None;
		let mut icon = Err::<&str, &str>("no icon");

		match &self.player {
			PlayerctlDevice::Some(player) => {
				icon = Ok(self.run_single(player)?);
				metadata = self.get_metadata(player);
			}
			PlayerctlDevice::All(players) => {
				for player in players {
					let icon_new = self.run_single(player);
					if let Ok(icon_new) = icon_new
						&& icon.is_err()
					{
						icon = Ok(icon_new);
					};
					if metadata.is_none() {
						metadata = self.get_metadata(player);
					}
				}
			}
		};

		self.icon = Some(icon.unwrap_or("").to_string());
		let label = metadata.map(|m| self.fmt_string(m));
		self.label = label;
		Ok(())
	}

	fn run_single(&self, player: &MprisPlayer) -> Result<&str, Box<dyn Error>> {
		let out = match self.action {
			PlayPause => {
				if player.get_playback_status() == "Playing" {
					player.call("Pause")?;
					"pause-large-symbolic"
				} else {
					player.call("Play")?;
					"play-large-symbolic"
				}
			}
			Shuffle => {
				let shuffle = player.get_property_bool("Shuffle").unwrap_or(false);
				player.set_property_bool("Shuffle", !shuffle)?;
				if shuffle {
					"playlist-consecutive-symbolic"
				} else {
					"playlist-shuffle-symbolic"
				}
			}
			Play => {
				player.call("Play")?;
				"play-large-symbolic"
			}
			Pause => {
				player.call("Pause")?;
				"pause-large-symbolic"
			}
			Stop => {
				player.call("Stop")?;
				"stop-large-symbolic"
			}
			Next => {
				player.call("Next")?;
				"media-seek-forward-symbolic"
			}
			Prev => {
				player.call("Previous")?;
				"media-seek-backward-symbolic"
			}
		};
		Ok(out)
	}

	fn get_metadata(
		&self,
		player: &MprisPlayer,
	) -> Option<std::collections::HashMap<String, String>> {
		match self.action {
			Next | Prev => {
				let map1 = player.get_metadata_map()?;
				let url1 = map1.get("xesam:url").cloned().unwrap_or_default();
				let mut counter = 0;
				while counter < 1000 {
					sleep(Duration::from_millis(5));
					let map2 = player.get_metadata_map()?;
					let url2 = map2.get("xesam:url").cloned().unwrap_or_default();
					if url1 != url2 {
						return Some(map2);
					}
					counter += 1;
				}
				Some(map1)
			}
			_ => player.get_metadata_map(),
		}
	}

	fn fmt_string(&self, metadata: std::collections::HashMap<String, String>) -> String {
		use std::collections::HashMap;
		use strfmt::Format;

		let mut vars = HashMap::new();
		let artist = metadata
			.get("xesam:artist")
			.map(|s| s.as_str())
			.unwrap_or("");
		let artist_album = metadata
			.get("xesam:albumArtist")
			.map(|s| s.as_str())
			.unwrap_or("");
		let title = metadata
			.get("xesam:title")
			.map(|s| s.as_str())
			.unwrap_or("");
		let album = metadata
			.get("xesam:album")
			.map(|s| s.as_str())
			.unwrap_or("");
		let track_num = metadata
			.get("xesam:trackNumber")
			.cloned()
			.unwrap_or_default();
		let disc_num = metadata
			.get("xesam:discNumber")
			.cloned()
			.unwrap_or_default();
		let autorating = metadata
			.get("xesam:autoRating")
			.cloned()
			.unwrap_or_default();

		vars.insert("artist".to_string(), artist);
		vars.insert("albumArtist".to_string(), artist_album);
		vars.insert("title".to_string(), title);
		vars.insert("album".to_string(), album);
		vars.insert("trackNumber".to_string(), &track_num);
		vars.insert("discNumber".to_string(), &disc_num);
		vars.insert("autoRating".to_string(), &autorating);

		self.fmt_str
			.clone()
			.unwrap_or("{artist} - {title}".into())
			.format(&vars)
			.unwrap_or_else(|e| {
				eprintln!("error: {}. using default string", e);
				"{artist} - {title}".format(&vars).unwrap()
			})
	}
}

impl PlayerctlAction {
	pub fn from(action: &str) -> Result<Self, String> {
		use PlayerctlAction::*;
		match action {
			"play-pause" => Ok(PlayPause),
			"play" => Ok(Play),
			"pause" => Ok(Pause),
			"stop" => Ok(Stop),
			"next" => Ok(Next),
			"prev" | "previous" => Ok(Prev),
			"shuffle" => Ok(Shuffle),
			x => Err(x.to_string()),
		}
	}
}

impl PlayerctlDeviceRaw {
	pub fn from(player: String) -> Result<Self, ()> {
		use PlayerctlDeviceRaw::*;
		match player.as_str() {
			"auto" | "" => Ok(None),
			"all" => Ok(All),
			"shift" => Ok(Shift),
			"unshift" => Ok(Unshift),
			_ => Ok(Some(player)),
		}
	}
}
