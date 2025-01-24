use mpris::{Metadata, PlaybackStatus, Player, PlayerFinder};

use super::config::user::ServerConfig;
use crate::utils::get_player;
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

impl Playerctl {
	pub fn new(
		action: PlayerctlAction,
		config: Arc<ServerConfig>,
	) -> Result<Playerctl, Box<dyn Error>> {
		let playerfinder = PlayerFinder::new()?;
		let player = get_player();
		let player = match player {
			PlayerctlDeviceRaw::None => PlayerctlDevice::Some(playerfinder.find_active()?),
			PlayerctlDeviceRaw::Some(name) => {
				PlayerctlDevice::Some(playerfinder.find_by_name(name.as_str())?)
			}
			PlayerctlDeviceRaw::All => PlayerctlDevice::All(playerfinder.find_all()?),
		};
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
		let mut metadata = Err("some errro");
		let mut icon = Err("some errro");
		match &self.player {
			PlayerctlDevice::Some(player) => {
				icon = Ok(self.run_single(player)?);
				metadata = self.get_metadata(player).or_else(|_| Err(""));
			}
			PlayerctlDevice::All(players) => {
				for player in players {
					let icon_new = self.run_single(player);
					if let Ok(icon_new) = icon_new {
						if icon.is_err() {
							icon = Ok(icon_new);
						}
					};
					if let Err(_) = metadata {
						metadata = self.get_metadata(player).or_else(|_| Err(""));
					}
				}
			}
		};

		self.icon = Some(icon.unwrap_or("").to_string());
		let label = if let Ok(metadata) = metadata {
			Some(self.fmt_string(metadata))
		} else {
			None
		};
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
	fn get_metadata(&self, player: &Player) -> Result<Metadata, mpris::DBusError> {
		match self.action {
			Next | Prev => {
				if let Ok(track_list) = player.get_track_list() {
					if let Some(track) = track_list.get(0) {
						return player.get_track_metadata(track);
					}
				}
				let metadata = player.get_metadata()?;
				let name1 = metadata.url().unwrap();
				let mut counter = 0;
				while counter < 20 {
					std::thread::sleep(std::time::Duration::from_millis(5));
					counter += 1;
					let metadata = player.get_metadata()?;
					let name2 = metadata.url().unwrap();
					if name1 != name2 {
						return Ok(metadata);
					}
				}
				Ok(metadata)
			}
			_ => player.get_metadata(),
		}
	}
	fn fmt_string(&self, metadata: mpris::Metadata) -> String {
		use std::collections::HashMap;
		use strfmt::Format;

		let mut vars = HashMap::new();
		let artists = metadata.artists().unwrap_or(vec![""]);
		let artists_album = metadata.album_artists().unwrap_or(vec![""]);
		let artist = artists.get(0).map_or("", |v| v);
		let artist_album = artists_album.get(0).map_or("", |v| v);

		let title = metadata.title().unwrap_or("");
		let album = metadata.album_name().unwrap_or("");
		let track_num = metadata
			.track_number()
			.and_then(|x| Some(x.to_string()))
			.unwrap_or(String::new());
		let disc_num = metadata
			.disc_number()
			.and_then(|x| Some(x.to_string()))
			.unwrap_or(String::new());
		let autorating = metadata
			.auto_rating()
			.and_then(|x| Some(x.to_string()))
			.unwrap_or(String::new());

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
			_ => Ok(Some(player)),
		}
	}
}
