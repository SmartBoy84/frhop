// all this crap just to dynamically get version and title id 😭

use core::str;
use std::{
    fmt::Debug,
    fs,
    io::{self, Read, Seek},
    mem,
    path::PathBuf,
};

use bytemuck::{Pod, Zeroable};
use thiserror::Error;

const HEADER: &[u8; 4] = b"PFS0"; // nsp/nca/... header
const TITLE_ID_WIDTH: usize = 16;

#[derive(Pod, Clone, Copy, Zeroable, Debug)]
#[repr(C)]
pub struct PFS0Header {
    tag: [u8; 4],
    n_files: u32, // lendian
    s_table_size: u32,
    _padding: u32,
}

#[derive(Pod, Clone, Copy, Zeroable, Debug)]
#[repr(C)]
pub struct FileHeader {
    offset: u64,
    size: u64,
    name_offset: u32,
    _padding: u32,
}

// #[repr(C)]
// enum TitleType {
//     Base = 0x80,
//     Patch = 0x81,
//     DLC = 0x82,
// }

pub struct File {
    name: String,
    file_header: FileHeader,
}

pub struct Files(Vec<File>);

#[derive(Pod, Clone, Copy, Zeroable, Debug)]
#[repr(C)]
pub struct Cnmt {
    pub title_id: u64,
    pub version: u32, // other stuff as well but whatever
    pub title_type: u8,
    pub _other_data: [u8; 19], // other data we don't care about
}

pub struct NspHeader {
    pub pfs0_header: PFS0Header,
    str_table: Vec<u8>,
    files: Files,
}

pub struct Nsp {
    pub name: String,
    pub path: PathBuf,
    pub file_size: u64,
    pub nsp_header: NspHeader,
    pub cnmt: Cnmt,
}

#[derive(Error, Debug)]
pub enum NspParsingError {
    #[error("Bad filename")]
    BadName,
    #[error("File read error")]
    FileError(#[from] io::Error),
    #[error("Header missing/malformed")]
    MalformedHeader(String),
    #[error("non utf-8 encoding in string table")]
    BadString(String),
    #[error("missiing ticket")]
    NoTicket,
    #[error("missing cnmt")]
    NoCnmt,
}

impl Debug for Nsp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}: {}", self.path.to_string_lossy(), self.cnmt.title_id)
    }
}

impl Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Files {
    fn from_vec(files: Vec<File>) -> Self {
        Self(files)
    }
    fn find_extension(&self, extension: &str) -> Option<&File> {
        self.0.iter().find(|f| f.name.ends_with(".tik"))
    }
}

impl Nsp {
    pub fn from_file(path: PathBuf) -> Result<Self, NspParsingError> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(NspParsingError::BadName)?
            .to_string();

        let file_size = std::fs::metadata(&path)?.len();

        let mut f = fs::OpenOptions::new().read(true).open(&path)?;

        // honestly; all this just to get filename + id
        // maybe picture sometime in future?

        // first up; header
        let mut pfs0_header = [0u8; mem::size_of::<PFS0Header>()];
        f.read_exact(&mut pfs0_header)?;
        let pfs0_header: PFS0Header = bytemuck::cast(pfs0_header);

        if &pfs0_header.tag != HEADER {
            return Err(NspParsingError::MalformedHeader(
                path.to_string_lossy().to_string(),
            ));
        }

        // 2 - read file headers
        let mut files = Vec::with_capacity(pfs0_header.n_files as usize);
        for _i in 0..pfs0_header.n_files {
            let mut file_header = [0u8; size_of::<FileHeader>()];
            f.read_exact(&mut file_header)?;
            let file_header: FileHeader = bytemuck::cast(file_header);
            files.push(File {
                name: String::new(),
                file_header,
            });
        }

        // 3 - read string table
        let mut str_table = vec![0u8; pfs0_header.s_table_size as usize];
        f.read_exact(&mut str_table)?;

        // 4 - get filenames
        for i in 0..files.len() {
            let str_data = &str_table[files[i].file_header.name_offset as usize
                ..if i == pfs0_header.n_files as usize - 1 {
                    str_table.len()
                } else {
                    files[i + 1].file_header.name_offset as usize
                }];
            match std::str::from_utf8(str_data) {
                Ok(s) => files[i]
                    .name
                    .push_str(s.trim_end_matches(|c| matches!(c, '\0' | ' '))),
                _ => {
                    return Err(NspParsingError::BadString(
                        String::from_utf8_lossy(str_data).to_string(),
                    ));
                }
            }
        }
        println!("{files:?}");

        let files = Files::from_vec(files);

        // 5 - get title id (redundant - use cnmt now)
        let Some(File { name: tik_id, .. }) = files.find_extension(".tik") else {
            return Err(NspParsingError::NoTicket);
        };
        let title_id = tik_id[..TITLE_ID_WIDTH].to_uppercase().to_string();

        // 5 - extract cnmt
        let Some(File {
            file_header: cnmt_header,
            ..
        }) = files.find_extension(".cnmt.nca")
        else {
            return Err(NspParsingError::NoCnmt);
        };
        let mut cnmt_buff = [0u8; size_of::<Cnmt>()];

        f.seek_relative(cnmt_header.offset as i64)?; // offset from end of header, which f file pointer already at, at this point
        f.read_exact(&mut cnmt_buff)?;

        let cnmt: Cnmt = bytemuck::cast(cnmt_buff);
        println!(
            "cnmt size: {}, cnmt offset: {}, cnmt: {cnmt:?}",
            cnmt_header.size, cnmt_header.offset
        );

        let nsp_header = NspHeader {
            pfs0_header,
            str_table,
            files,
        };

        Ok(Self {
            name,
            file_size,
            path,
            nsp_header,
            cnmt,
        })
    }
}
