use std::{
    fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use thiserror::Error;

use crate::game::{entry::GameEntry, info::GameInfo, nsp::NspParsingError};

pub mod entry;
mod info;
pub mod nsp;

#[derive(Debug)]
pub struct Game {
    pub info: GameInfo,
    path: PathBuf,
}

#[derive(Error, Debug)]
pub enum GameError {
    #[error("io error")]
    IoError(#[from] io::Error),
    #[error("nsp error")]
    NspError(#[from] NspParsingError),
    #[error("malformed file name")]
    MalformedName,
    #[error("badly formated name")]
    BadNameFormat(String),
}

fn get_mtime(path: &PathBuf) -> io::Result<f64> {
    let metadata = fs::metadata(&path)?;

    let mtime = metadata.modified()?;
    let duration = mtime.duration_since(UNIX_EPOCH).unwrap_or_default(); // fallback if system clock is earlier than epoch

    Ok(duration.as_secs_f64())
}

impl TryFrom<&Game> for GameEntry {
    type Error = io::Error;
    fn try_from(value: &Game) -> Result<Self, Self::Error> {
        let mtime = get_mtime(value.path())?; // not hardcoded in struct, since it may change + file might get deleted
        let info = value.game_info();
        Ok(GameEntry::plain_new(info.title_id(), info.size(), mtime))
    }
}

impl Game {
    pub fn try_new<P: AsRef<Path>>(path: P) -> Result<Self, GameError> {
        let path = path.as_ref(); // not always free
        Ok(Self {
            info: GameInfo::try_new(path)?, // game info also runs as_ref, but we make that free here
            path: path.to_path_buf(),
        })
    }

    pub fn size(&self) -> u64 {
        self.game_info().size()
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn game_info(&self) -> &GameInfo {
        &self.info
    }
}
