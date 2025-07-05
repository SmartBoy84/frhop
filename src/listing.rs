use std::{collections::HashMap, fs, path::Path};

use smol::io;
use thiserror::Error;

use crate::game::{Game, GameError};

#[derive(Debug, Default)]
pub struct Listing {
    id_to_game: HashMap<String, Game>,   // game id -> game
    file_to_id: HashMap<String, String>, // file name -> id
}

#[derive(Error, Debug)]
pub enum ListingError {
    #[error("not a switch archive")]
    NotArchive,
    #[error("non utf-8 filename")]
    BadName,
    #[error("io error")]
    IoError(#[from] io::Error),
    #[error("game error")]
    GameError(#[from] GameError),
}

pub enum ListingIndex<'a> {
    TitleId(&'a str),
    FileName(&'a str),
}

impl Listing {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn id_map(&self) -> &HashMap<String, Game> {
        &self.id_to_game
    }

    pub fn file_map(&self) -> &HashMap<String, String> {
        &self.file_to_id
    }

    fn add_file<P: AsRef<Path>>(&mut self, p: P) -> Result<(), ListingError> {
        let p = p.as_ref();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .ok_or(ListingError::BadName)?;

        let p_str = p.to_str().ok_or(ListingError::BadName)?;

        if !matches!(ext, "nsp" | "xci" | "nsz" | "nsx") {
            return Err(ListingError::NotArchive);
        }

        let game = Game::try_new(p)?;
        let id = game.game_info().title_id().to_string();
        if let Some(g) = self.id_to_game.get_mut(&id)
            && g != &game
        {
            println!("Changed; {g:?}");
            *g = game;
        } else {
            self.id_to_game
                .insert(game.game_info().title_id().to_string(), game);
        }
        self.file_to_id.insert(p_str.to_string(), id);

        Ok(())
    }

    /// fill only fail for io errors
    fn add_file_nonfatal<P: AsRef<Path>>(&mut self, p: P) -> io::Result<()> {
        match self.add_file(&p) {
            Err(ListingError::IoError(e)) => return Err(e),
            Ok(_) | Err(ListingError::NotArchive | ListingError::BadName) => (), // ignore this error so user isn't bombarded with errors
            Err(e) => {
                let p = p.as_ref();
                println!("Failed to add {p:?}: {e:?}")
            }
        }
        Ok(())
    }

    fn add_dir<P: AsRef<Path>>(&mut self, p: P) -> io::Result<()> {
        for f in fs::read_dir(p)? {
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

            self.add_file_nonfatal(dir_entry.path())?;
        }
        Ok(())
    }

    /// Provide either file path OR dir path to scan at top-level
    pub fn add<P: AsRef<Path>>(&mut self, p: P) -> Result<(), ListingError> {
        let f = fs::metadata(&p)?;
        if f.is_dir() {
            self.add_dir(&p)?;
        } else {
            self.add_file_nonfatal(&p)?;
        }
        Ok(())
    }

    pub fn get_game(&self, index: ListingIndex) -> Option<&Game> {
        match index {
            ListingIndex::FileName(f_name) => self
                .file_map()
                .get(f_name)
                .and_then(|id| self.id_map().get(id)),
            ListingIndex::TitleId(t_id) => self.id_map().get(t_id),
        }
    }
}
