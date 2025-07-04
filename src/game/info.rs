use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::game::{
    GameError,
    nsp::{Cnmt, Nsp},
};

// kept separate to make serialisation easy

#[derive(Debug, miniserde::Serialize)]
pub struct GameInfo {
    title_id: String,
    name: String,
    size: u64,
    version: String,
}

// just so I don't have to keep track of tupule order from return
#[derive(Debug)]
struct Extractor {
    title_id: String,
    version: String,
}

impl GameInfo {
    pub fn try_new<P: AsRef<Path>>(path: P) -> Result<Self, GameError> {
        let p = path.as_ref().to_path_buf();

        let f_base = p
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or(GameError::MalformedName)?;

        let Extractor { title_id, version } = match f_base.rsplit_once('.') {
            // Some((_, "nsp")) | None => Extractor::from_nsp(&p), // .nsp OR default for no extension
            _ => Extractor::from_name(f_base, &p),              // unsupported extension
        }?;

        let metadata = fs::metadata(path)?;

        Ok(GameInfo {
            title_id,
            size: metadata.len(),
            version,
            name: f_base.to_string(),
        })
    }

    pub fn title_id(&self) -> &str {
        &self.title_id
    }

    pub fn size(&self) -> u64 {
        self.size
    }
}

impl Extractor {
    fn from_name(name: &str, path: &PathBuf) -> Result<Self, GameError> {
        let mut result = Vec::new();
        let mut start = None;

        for (i, c) in name.char_indices() {
            match c {
                '[' => start = Some(i + 1),
                ']' => {
                    if let Some(s) = start {
                        result.push(&name[s..i]);
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

        let title_id = result
            .iter()
            .find(|s| !s.starts_with('v') && matches!(s.len(), 6..=16)) // titleIds are 6-16 characters long
            .map(|s| s.to_string())
            .ok_or(GameError::BadNameFormat(path.to_string_lossy().to_string()))?;

        Ok(Extractor { title_id, version })
    }

    fn from_nsp(path: &PathBuf) -> Result<Self, GameError> {
        let Nsp {
            cnmt: Cnmt {
                title_id, version, ..
            },
            ..
        } = Nsp::from_file(path)?;
        println!("Using nsp extraction for {path:?}");
        let ex = Extractor {
            title_id: format!("{:x}", title_id), // internally u64, sent as hex
            version: version.to_string(),
        };

        println!("{ex:?}");
        Ok(ex)
    }
}
