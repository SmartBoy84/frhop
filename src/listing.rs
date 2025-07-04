use std::{collections::HashMap, fs};

use miniserde::json;
use smol::io;

use crate::game::Game;

#[derive(Debug)]
pub struct Listing(pub HashMap<String, Game>);

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

            match Game::try_new(p) {
                Ok(game) => {
                    h.insert(game.game_info().size().to_string(), game);
                }
                Err(e) => eprintln!("{e:?}"),
            };
        }
        Ok(Listing(h))
    }

    pub fn serialise(&self) -> String {
        json::to_string(&self.0.values().map(|g| g.game_info()).collect::<Vec<_>>())
    }
}
