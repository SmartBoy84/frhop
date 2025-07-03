use std::{collections::HashMap, fs, path::PathBuf, time::UNIX_EPOCH};

use smol::io;
use miniserde::json;
use thiserror::Error;

use crate::game::entry::GameEntry;

/*
Tinfoil extracts titleid and version directly from filename
Maybe I could do this dynamically?
Nsp is easy enough; issue is handling other filetypes
*/

#[derive(Debug)]
pub struct Game {
    pub info: GameInfo,
    mtime: f64,
    file: PathBuf,
}

#[derive(Debug, miniserde::Serialize)]
pub struct GameInfo {
    id: String,
    name: String,
    size: u64,
    version: String,
}

impl From<&Game> for GameEntry {
    fn from(value: &Game) -> Self {
        GameEntry::plain_new(value.info.id.clone(), value.info.size, value.mtime)
    }
}

#[derive(Error, Debug)]
enum GameError {
    #[error("io error")]
    IoError(#[from] io::Error),
    #[error("malformed file name")]
    MalformedName,
    #[error("badly formated name")]
    BadNameFormat(String),
}

#[derive(Debug)]
pub struct Listing(pub HashMap<String, Game>);

impl Game {
    pub fn size(&self) -> u64 {
        self.info.size
    }

    pub fn path(&self) -> &PathBuf {
        &self.file
    }

    fn from_path(path: PathBuf) -> Result<Self, GameError> {
        /*
        this is the bit to change if I want dynamic titleids
        */

        let mut result = Vec::new();
        let mut start = None;

        let f_base = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or(GameError::MalformedName)?;
        for (i, c) in f_base.char_indices() {
            match c {
                '[' => start = Some(i + 1),
                ']' => {
                    if let Some(s) = start {
                        result.push(&f_base[s..i]);
                        start = None;
                    }
                }
                _ => {}
            }
        }

        // this is a very shoddy way of doing this...
        let version = result
            .iter()
            .find(|s| s.starts_with('v'))
            .map(|s| s[1..].to_string())
            .unwrap_or("0".to_string()); // this is optional

        let id = result
            .iter()
            .find(|s| !s.starts_with('v') && matches!(s.len(), 6..=16)) // titleIds are 6-16 characters long
            .map(|s| s.to_string())
            .ok_or(GameError::BadNameFormat(path.to_string_lossy().to_string()))?;

        let metadata = fs::metadata(&path)?;
        let mtime = metadata.modified()?;
        let duration = mtime.duration_since(UNIX_EPOCH).unwrap_or_default(); // fallback if system clock is earlier than epoch

        Ok(Self {
            info: GameInfo {
                id,
                version,
                size: metadata.len(),
                name: f_base.to_string(),
            },
            mtime: duration.as_secs_f64(),
            file: path,
        })
    }
}

impl Listing {
    pub fn get_game(&self, t_id: &str) -> Option<&Game> {
        self.0.get(t_id)
    }

    pub fn from_dir(dir: &str) -> io::Result<Self> {
        let mut h = HashMap::new();
        for f in fs::read_dir(dir)? {
            // bit verbose but can be lax this way - bad files don't crash program
            let dir_entry = match f {
                Err(ref e) => {
                    eprintln!("{e:?}");
                    continue;
                }
                Ok(f) => f,
            };

            match dir_entry.file_type() {
                Err(ref e) => {
                    eprintln!("{e:?}");
                    continue;
                }
                Ok(f) if !f.is_file() => continue,
                _ => (),
            }

            let p = dir_entry.path();
            let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
                continue;
            };

            if !matches!(ext, "nsp" | "xci" | "nsz" | "nsx") {
                continue;
            }

            match Game::from_path(p) {
                Ok(game) => {
                    h.insert(game.info.id.clone(), game);
                }
                Err(e) => eprintln!("{e:?}"),
            };
        }
        Ok(Listing(h))
    }

    pub fn serialise(&self) -> String {
        json::to_string(&self.0.values().map(|g| &g.info).collect::<Vec<_>>())
    }
}
