use std::{collections::HashMap, fs, path::Path};

use miniserde::json;
use smol::io;
use thiserror::Error;

use crate::game::{Game, GameError};

#[derive(Debug, Default)]
pub struct Listing(pub HashMap<String, Game>);

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

impl Listing {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn map(&self) -> &HashMap<String, Game> {
        &self.0
    }

    pub fn map_mut(&mut self) -> &mut HashMap<String, Game> {
        &mut self.0
    }

    fn add_file<P: AsRef<Path>>(&mut self, p: P) -> Result<(), ListingError> {
        let p = p.as_ref();
        if p.starts_with(".") {
            return Err(ListingError::NotArchive); // immediately exclude hidden directories
        }
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
            return Err(ListingError::BadName);
        };

        if !matches!(ext, "nsp" | "xci" | "nsz" | "nsx") {
            return Err(ListingError::NotArchive);
        }

        let game = Game::try_new(p)?;
        if let Some(g) = self.map_mut().get_mut(game.game_info().title_id())
            && g != &game
        {
            println!("Changed; {g:?}");
            *g = game;
        } else {
            self.map_mut()
                .insert(game.game_info().title_id().to_string(), game);
        }

        Ok(())
    }

    /// fill only fail for io errors
    fn add_file_nonfatal<P: AsRef<Path>>(&mut self, p: P) -> io::Result<()> {
        match self.add_file(&p) {
            Err(ListingError::IoError(e)) => return Err(e),
            Ok(_) | Err(ListingError::NotArchive) => (), // ignore this error so user isn't bombarded with errors
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

    pub fn get_game(&self, t_id: &str) -> Option<&Game> {
        self.map().get(t_id)
    }

    pub fn serialise(&self) -> String {
        json::to_string(
            &self
                .map()
                .values()
                .map(|g| g.game_info())
                .collect::<Vec<_>>(),
        )
    }
}
