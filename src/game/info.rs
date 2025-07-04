use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::game::{GameError, nsp::Nsp};

// kept separate to make serialisation easy
// rename macro used to enforce that name is FIXED

#[derive(Debug, miniserde::Serialize, PartialEq)]
pub struct GameInfo {
    #[serde(rename = "id")]
    id: String,
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "name")]
    size: u64,
    #[serde(rename = "version")]
    version: String,
}

// just so I don't have to keep track of tuple order from return
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

        let mut ex = Extractor::from_name(f_base, &p); // try to extract from filename first
        if let Err(e) = ex {
            println!(
                "Warning; failed to extract info from name [{f_base}]: {e:?} - trying to extract from binary..."
            );
            ex = Extractor::from_nsp(&p); // fallback option
        }
        let Extractor { title_id, version } = ex?;

        let metadata = fs::metadata(path)?;

        Ok(GameInfo {
            id: title_id,
            size: metadata.len(),
            version,
            name: f_base.to_string(),
        })
    }

    pub fn title_id(&self) -> &str {
        &self.id
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
        // Right now, this is the fall back - all I can get is the titleid without decryption
        let title_id = Nsp::from_file(path)?.title_id()?;

        let ex = Extractor {
            title_id: title_id,       // internally u64, sent as hex
            version: "0".to_string(), // unfortunately no parsing
        };

        Ok(ex)
    }
}
