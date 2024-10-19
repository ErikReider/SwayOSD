use mpris::{
    PlayerFinder,
    Player,
    PlaybackStatus,
};

use std::error::Error;
use crate::utils::get_player;

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
}

impl Playerctl {
    pub fn new(action: PlayerctlAction) -> Result<Playerctl, Box<dyn Error>> {
        let playerfinder = PlayerFinder::new()?;
        let player = get_player();
        let player = match player {
            PlayerctlDeviceRaw::None => PlayerctlDevice::Some(playerfinder.find_active()?),
            PlayerctlDeviceRaw::Some(name) => {
                PlayerctlDevice::Some(playerfinder.find_by_name(name.as_str())?)
            },
            PlayerctlDeviceRaw::All => PlayerctlDevice::All(playerfinder.find_all()?),
        };
        Ok(Self {
            player,
            action,
            icon: None,
            label: None,
        })
    }
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        use PlayerctlAction::*;
        use PlaybackStatus::*;
        let run_single = |player: &Player| -> Result<&str, Box<dyn Error>> {
            let out = match self.action {
                PlayPause => {
                    match player.get_playback_status()? {
                        Playing => {player.pause()?; "pause-large -symbolic"},
                        Paused | Stopped => {player.play()?;"play-large-symbolic"}
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
                },
                Play => {player.play()?; "play-large-symbolic"},
                Pause => {player.pause()?; "pause-large-symbolic"},
                Stop => {player.stop()?; "stop-large-symbolic"},
                Next => {player.next()?; "media-seek-forward-symbolic"},
                Prev => {player.previous()?; "media-seek-backward-symbolic"},
            };
            Ok(out)
        };
        let mut metadata = Err("some errro");
        let icon = match &self.player {
            PlayerctlDevice::Some(player) => {
                metadata = player.get_metadata().or_else(|_| Err(""));
                run_single(player)?
            },
            PlayerctlDevice::All(players) => {
                let mut icon = Err("couldn't change any players!");
                for player in players {
                    let icon_new = run_single(player);
                    if let Ok(icon_new) = icon_new {
                        if icon.is_err() {
                            icon = Ok(icon_new);
                        }
                    };
                    if let Err(_) = metadata {
                        metadata = player.get_metadata().or_else(|_| Err(""));
                    }
                }
                icon?
            },
        };

        self.icon = Some(icon.to_string());
        let label = if let Ok(metadata) = metadata {
            let artist = metadata.artists().and_then(|x| {
                if x.len() != 0 {
                    Some(x[0].to_string())
                } else {
                    None
                }
            });
            let title = metadata.title().and_then(|x| Some(x.to_string()));
            if title.is_none() {
                if artist.is_none() {
                    None
                } else {
                    artist
                }
            } else {
                if artist.is_none() {
                    title
                } else {
                    Some(format!("{} - {}", title.unwrap(), artist.unwrap()))
                }
            }
        } else {
            None
        };
        self.label = label;
        Ok(())
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
            x => Err(x.to_string())
        }
    }
}

impl PlayerctlDeviceRaw {
    pub fn from(player: String) -> Result<Self, ()> {
        use PlayerctlDeviceRaw::*;
        match player.as_str() {
            "auto" | "" => Ok(None),
            "all" => Ok(All),
            _ => Ok(Some(player))
        }
    }
}
