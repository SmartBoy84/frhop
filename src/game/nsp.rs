// all this crap just to dynamically get version and title id 😭
use core::str;
use std::{
    fmt::Debug,
    fs,
    io::{self, Read},
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
pub struct FileEntry {
    offset: u64,
    size: u64,
    s_table_off: u32,
    _reserved: u32,
}

// #[repr(C)]
// enum TitleType {
//     Base = 0x80,
//     Patch = 0x81,
//     DLC = 0x82,
// }

pub struct File {
    name: String,
    file_header: FileEntry,
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
    pub nsp_header: NspHeader,
    // pub cnmt: Cnmt,
}

#[derive(Error, Debug)]
pub enum NspParsingError {
    #[error("File read error")]
    FileError(#[from] io::Error),
    #[error("Header missing/malformed")]
    MalformedHeader,
    #[error("non utf-8 encoding in string table")]
    BadString(String),
    #[error("missiing ticket")]
    NoTicket,
    #[error("missing cnmt")]
    NoCnmt,
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
        self.0.iter().find(|f| f.name.ends_with(extension))
    }
}

impl Nsp {
    pub fn from_file(path: &PathBuf) -> Result<Self, NspParsingError> {
        // the following process is sequential - the f cursor is automatically advanced behind the scenes
        let mut f = fs::OpenOptions::new().read(true).open(&path)?;

        // honestly; all this just to get filename + id
        // maybe picture sometime in future?

        // first up; header
        let mut pfs0_header = [0u8; mem::size_of::<PFS0Header>()];
        f.read_exact(&mut pfs0_header)?;
        let pfs0_header: PFS0Header = bytemuck::cast(pfs0_header);

        if &pfs0_header.tag != HEADER {
            return Err(NspParsingError::MalformedHeader);
        }

        // 2 - read file headers
        let mut files = Vec::with_capacity(pfs0_header.n_files as usize);
        for _i in 0..pfs0_header.n_files {
            let mut file_header = [0u8; size_of::<FileEntry>()];
            f.read_exact(&mut file_header)?;
            let file_header: FileEntry = bytemuck::cast(file_header);
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
            let str_data = &str_table[files[i].file_header.s_table_off as usize
                ..if i == pfs0_header.n_files as usize - 1 {
                    str_table.len()
                } else {
                    files[i + 1].file_header.s_table_off as usize
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

        let files = Files::from_vec(files);

        let nsp_header = NspHeader {
            pfs0_header,
            str_table,
            files,
        };

        Ok(Self { nsp_header })
    }

    pub fn title_id(&self) -> Result<String, NspParsingError> {
        // 5 - extract cnmt - AHHHH, IT'S FOCKIN' ENCRYPTED MATE
        // let Some(File {
        //     file_header: cnmt_header,
        //     name,
        //     ..
        // }) = files.find_extension(".cnmt.nca")
        // else {
        //     return Err(NspParsingError::NoCnmt);
        // };
        // println!("{:?}", name);
        // let mut cnmt_buff = [0u8; size_of::<Cnmt>()];

        // f.seek_relative(cnmt_header.offset as i64)?; // offset from end of header, which f file pointer already at, at this point
        // f.read_exact(&mut cnmt_buff)?;

        // let cnmt: Cnmt = bytemuck::cast(cnmt_buff); // little endian -> native endianess (eh, all modern computers are LE anyways...)

        // I would've preferred to parse the cnmt header, but its encrypted
        // maybe in the future I could implement decrypting...
        let Some(File { name: tik_id, .. }) = self.nsp_header.files.find_extension(".tik") else {
            return Err(NspParsingError::NoTicket);
        };
        let title_id = tik_id[..TITLE_ID_WIDTH].to_uppercase().to_string();
        Ok(title_id)
    }
}
