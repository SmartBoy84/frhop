/*
info, queue, search, download
*/

use std::{
    fs::File,
    io::{self, Seek, SeekFrom},
};

use miniserde::json;
use thiserror::Error;

use crate::{
    device::{
        SwitchCommError, SwitchHostImpl, hosts::tinfoil::TinfoilInterface,
        writer::SwitchHostWriterExt,
    },
    game::entry::GameEntry,
    listing::ListingIndex,
};

#[derive(Error, Debug)]
pub enum TinfoilQueryErrorKind {
    #[error("unsupported cmd: {0}")]
    UnsupportedCmd(String),
    #[error("unsupported endpoint: {0}")]
    UnsupportedEndpoint(String),
    #[error("unsupported req type: {0}")]
    UnsupportedReqType(String),
    #[error("no title id in info query")]
    NoIdInfoQuery,
    #[error("game not found: {0}")]
    GameNotFound(String),
    #[error("bad download range")]
    BadRange,
    #[error("failed to read file: {0}")]
    FileRead(#[from] io::Error),
}

#[derive(Error, Debug)]
pub enum TinfoilQueryError {
    #[error("bad query: {}", .0.0)]
    BadQuery((TinfoilQueryErrorKind, Vec<u8>)),
    #[error("{0}")]
    CommError(#[from] SwitchCommError),
}

impl TinfoilQueryErrorKind {
    fn to_error(self, b: Vec<u8>) -> TinfoilQueryError {
        TinfoilQueryError::BadQuery((self, b))
    }
}

// &str's lifetime is shorter than TinfoilDevice but Query doesn't care about TinfoilDevice longer than it cares about the &str so it works
pub struct TinfoilQuery<'a> {
    buff: Vec<u8>,
    device: &'a TinfoilInterface,
    endpoint: &'a str,
    req_type: &'a str,
    query: Option<&'a str>,
}

impl<'a> TinfoilQuery<'a> {
    pub async fn process_query(
        device: &'a TinfoilInterface,
        buff: Vec<u8>,
        payload: &'a str,
    ) -> Result<Vec<u8>, TinfoilQueryError> {
        Self::from_payload(device, buff, payload)?
            .handle_query()
            .await
    }

    /// buff MUST contain payload at this point
    fn from_payload(
        device: &'a TinfoilInterface,
        buff: Vec<u8>,
        payload: &'a str,
    ) -> Result<Self, TinfoilQueryError> {
        let mut args = payload.split('/');
        if args.next() != Some("") {
            return Err(TinfoilQueryErrorKind::UnsupportedCmd(payload.to_string()).to_error(buff));
            // be somewhat strict; tinfoil over usb will always have '/' at start
        }

        // tinfoil's usb args are: `/{req_name}/{req_type}/{query url}`
        let (Some(endpoint), Some(req_type), query) = (
            args.next(),
            args.next(),
            args.next(), // may not exist
        ) else {
            return Err(TinfoilQueryErrorKind::UnsupportedCmd(payload.to_string()).to_error(buff));
        };

        Ok(Self {
            buff,
            device,
            endpoint,
            req_type,
            query,
        })
    }

    async fn handle_query(self) -> Result<Vec<u8>, TinfoilQueryError> {
        match self.endpoint {
            "api" => self.route_api().await,
            _ => {
                return Err(
                    TinfoilQueryErrorKind::UnsupportedEndpoint(self.endpoint.to_string())
                        .to_error(self.buff),
                );
            }
        }
    }
}

impl TinfoilQuery<'_> {
    async fn write_str(self, res: &str) -> Result<Vec<u8>, TinfoilQueryError> {
        Ok(self.device.write_str(res, self.buff).await?)
    }
}

impl TinfoilQuery<'_> {
    async fn route_api(self) -> Result<Vec<u8>, TinfoilQueryError> {
        println!(
            "API request - req: {}, queries: {:?}",
            self.req_type, self.query
        );

        match self.req_type.trim_end_matches('?') // residue of html query impl in tinfoil it seems
        {
            "queue" => self.handle_queue().await,
            "search" => self.handle_search().await,
            "info" => self.handle_info().await,
            "download" => self.handle_download().await, // because my TinfoilQueryError also maps from io::Error
            _ => Err(TinfoilQueryErrorKind::UnsupportedReqType(self.req_type.to_string()).to_error(self.buff))?,
        }
    }

    async fn handle_download(self) -> Result<Vec<u8>, TinfoilQueryError> {
        let Some(mut args) = self.query.map(|q| q.split('/')) else {
            return Err(TinfoilQueryErrorKind::NoIdInfoQuery.to_error(self.buff))?;
        };

        let Some(t_id) = args.next() else {
            return Err(TinfoilQueryErrorKind::NoIdInfoQuery.to_error(self.buff))?;
        };

        let listing = self.device.get_interface().get_listing().await;
        let Some(game) = listing.get_game(ListingIndex::TitleId(t_id)) else {
            return Err(TinfoilQueryErrorKind::GameNotFound(t_id.to_string()).to_error(self.buff))?;
        };

        let mut args = args.map(|v| v.parse::<u64>());
        let (Ok(start), Ok(end)) = (
            args.next().unwrap_or(Ok(0)),
            args.next().unwrap_or(Ok(game.size())),
        ) else {
            return Err(TinfoilQueryErrorKind::BadRange.to_error(self.buff))?;
        };

        let mut f = match File::open(game.path()) {
            Ok(f) => f,
            Err(e) => return Err(TinfoilQueryErrorKind::from(e).to_error(self.buff))?,
        };

        if let Err(e) = f.seek(SeekFrom::Start(start)) {
            return Err(TinfoilQueryErrorKind::from(e).to_error(self.buff))?;
        };

        println!("Requested file: {:?}, range {start}-{end}", game.path());
        Ok(self
            .device
            .write_chunked_with_caching(f, end - start, self.buff)
            .await?)
    }

    async fn handle_info(self) -> Result<Vec<u8>, TinfoilQueryError> {
        let Some(t_id) = self.query.and_then(|q| q.split('/').next()) else {
            return Err(TinfoilQueryErrorKind::NoIdInfoQuery.to_error(self.buff))?;
        };

        if let Some(game) = self
            .device
            .get_interface()
            .get_listing()
            .await
            .get_game(ListingIndex::TitleId(t_id))
        {
            let game_entry = match GameEntry::try_from(game) {
                Ok(g_e) => g_e,
                Err(e) => return Err(TinfoilQueryErrorKind::from(e).to_error(self.buff))?,
            };
            self.write_str(&miniserde::json::to_string(&game_entry))
                .await
        } else {
            Err(TinfoilQueryErrorKind::GameNotFound(t_id.to_string()).to_error(self.buff))?
        }
    }

    async fn handle_search(self) -> Result<Vec<u8>, TinfoilQueryError> {
        // slighly inefficient due to allocation but worth it for the simiplicity in my opinion (im lazy)
        let s = json::to_string(
            &self
                .device
                .get_interface()
                .get_listing()
                .await
                .id_map()
                .values()
                .map(|g| g.game_info())
                .collect::<Vec<_>>(),
        );
        self.write_str(&s).await
    }

    async fn handle_queue(self) -> Result<Vec<u8>, TinfoilQueryError> {
        self.write_str("[]").await // another half-baked feature by Blawar
    }
}
