use mpris::{Metadata, PlaybackStatus, Player, PlayerFinder};
use playerctld::PlayerctldProxyBlocking;
use zbus::blocking::Connection;

use super::config::user::ServerConfig;
use crate::utils::get_player as get_player_raw;
use std::{error::Error, sync::Arc, thread::sleep, time::Duration};
use PlaybackStatus::*;
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

pub enum PlayerctlDevice {
	All(Vec<Player>),
	Some(Player),
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

	fn get_single_player(player: String) -> Result<PlayerctlDevice, Box<dyn Error>> {
		let possible_player = PlayerFinder::new()?.find_all()?.into_iter().find(|p| {
			let bus = p.bus_name();
			bus.contains(&player)
		});
		match possible_player {
			Some(player) => Ok(PlayerctlDevice::Some(player)),
			None => Err(From::from(mpris::FindingError::NoPlayerFound)),
		}
	}

	match player {
		PlayerctlDeviceRaw::None => {
			let fallback = || -> Result<PlayerctlDevice, Box<dyn Error>> {
				Ok(PlayerctlDevice::Some(PlayerFinder::new()?.find_active()?))
			};
			let Ok(players) = get_playerctld_devices() else {
				return fallback();
			};
			let Some(player) = players.first() else {
				return fallback();
			};
			get_single_player(player.to_string())
		}
		PlayerctlDeviceRaw::Some(name) => get_single_player(name),
		PlayerctlDeviceRaw::All => Ok(PlayerctlDevice::All(PlayerFinder::new()?.find_all()?)),

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
		let mut icon = Err("some errro");
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
		let label = metadata.map(|metadata| self.fmt_string(metadata));
		self.label = label;
		Ok(())
	}
	fn run_single(&self, player: &Player) -> Result<&str, Box<dyn Error>> {
		let out = match self.action {
			PlayPause => match player.get_playback_status()? {
				Playing => {
					player.pause()?;
					"pause-large-symbolic"
				}
				Paused | Stopped => {
					player.play()?;
					"play-large-symbolic"
				}
			},
			Shuffle => {
				let shuffle = player.get_shuffle()?;
				player.set_shuffle(!shuffle)?;
				if shuffle {
					"playlist-consecutive-symbolic"
				} else {
					"playlist-shuffle-symbolic"
				}
			}
			Play => {
				player.play()?;
				"play-large-symbolic"
			}
			Pause => {
				player.pause()?;
				"pause-large-symbolic"
			}
			Stop => {
				player.stop()?;
				"stop-large-symbolic"
			}
			Next => {
				player.next()?;
				"media-seek-forward-symbolic"
			}
			Prev => {
				player.previous()?;
				"media-seek-backward-symbolic"
			}
		};
		Ok(out)
	}
	fn get_metadata(&self, player: &Player) -> Option<Metadata> {
		match self.action {
			Next | Prev => {
				if let Ok(track_list) = player.get_track_list()
					&& let Some(track) = track_list.get(0)
				{
					return player.get_track_metadata(track).ok();
				}
				let metadata = player.get_metadata().ok()?;
				let name1 = metadata.url()?;
				let mut counter = 0;
				while counter < 1000 {
					// 1000 * 5ms = 5s
					let metadata = player.get_metadata().ok()?;
					let name2 = metadata.url()?;
					if name1 != name2 {
						return Some(metadata);
					}
					sleep(Duration::from_millis(5));
					counter += 1;
				}
				Some(metadata)
			}
			_ => player.get_metadata().ok(),
		}
	}
	fn fmt_string(&self, metadata: mpris::Metadata) -> String {
		use std::collections::HashMap;
		use strfmt::Format;

		let mut vars = HashMap::new();
		let artists = metadata.artists().unwrap_or(vec![""]);
		let artists_album = metadata.album_artists().unwrap_or(vec![""]);
		let artist = artists.first().map_or("", |v| v);
		let artist_album = artists_album.first().map_or("", |v| v);

		let title = metadata.title().unwrap_or("");
		let album = metadata.album_name().unwrap_or("");
		let track_num = metadata
			.track_number()
			.map(|x| x.to_string())
			.unwrap_or_default();
		let disc_num = metadata
			.disc_number()
			.map(|x| x.to_string())
			.unwrap_or_default();
		let autorating = metadata
			.auto_rating()
			.map(|x| x.to_string())
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
