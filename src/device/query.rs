/*
info, queue, search, download
*/

use std::{
    fs::File,
    io::{Seek, SeekFrom},
};

use thiserror::Error;

use crate::{
    device::{TinfoilDevice, TinfoilDeviceCommError},
    game::entry::GameEntry,
};

#[derive(Error, Debug)]
pub enum QueryError {
    #[error("malformed cmd")]
    UnsupportedCmd(String),
    #[error("unsupported endpoint")]
    UnsupportedEndpoint(String),
    #[error("unsupported req type")]
    UnsupportedReqType(String),
    #[error("no title id in info query")]
    NoIdInfoQuery,
    #[error("game not found")]
    GameNotFound(String),
    #[error("bad download range")]
    BadRange,
    #[error("io error")]
    IoError(#[from] smol::io::Error),
}

// &str's lifetime is shorter than TinfoilDevice but Query doesn't care about TinfoilDevice longer than it cares about the &str so it works
pub struct Query<'a> {
    buff: Vec<u8>,
    device: &'a TinfoilDevice,
    endpoint: &'a str,
    req_type: &'a str,
    query: Option<&'a str>,
}

impl<'a> Query<'a> {
    /// buff MUST contain payload at this point
    pub fn from_payload(
        device: &'a TinfoilDevice,
        buff: Vec<u8>,
        payload: &'a str,
    ) -> Result<Self, QueryError> {
        let mut args = payload.split('/');
        if args.next() != Some("") {
            return Err(QueryError::UnsupportedCmd(payload.to_string()).into());
            // be somewhat strict; tinfoil over usb will always have '/' at start
        }

        // tinfoil's usb args are: `/{req_name}/{req_type}/{query url}`
        let (Some(endpoint), Some(req_type), query) = (
            args.next(),
            args.next(),
            args.next(), // may not exist
        ) else {
            return Err(QueryError::UnsupportedCmd(payload.to_string()).into());
        };

        Ok(Self {
            buff,
            device,
            endpoint,
            req_type,
            query,
        })
    }

    pub async fn handle_query(self) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        match self.endpoint {
            "api" => self.route_api().await,
            _ => return Err(QueryError::UnsupportedEndpoint(self.endpoint.to_string()).into()),
        }
    }
}

impl Query<'_> {
    async fn write_str(self, res: &str) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        let b = res.as_bytes();
        self.device.write_from_reader(b, b.len(), self.buff).await
    }

    #[inline] // tho compiler will prob already inline it
    fn map_io(e: smol::io::Error) -> QueryError {
        QueryError::IoError(e)
    }
}

impl Query<'_> {
    async fn route_api(self) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        println!(
            "API request - req: {}, queries: {:?}",
            self.req_type, self.query
        );

        match self.req_type.trim_end_matches('?') // residue of html query impl in tinfoil it seems
        {
            "queue" => self.handle_queue().await,
            "search" => self.handle_search().await,
            "info" => self.handle_info().await,
            "download" => self.handle_download().await, // because my QueryError also maps from io::Error
            _ => Err(QueryError::UnsupportedReqType(self.req_type.to_string()).into()),
        }
    }

    async fn handle_download(self) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        let Some(mut args) = self.query.map(|q| q.split('/')) else {
            return Err(QueryError::NoIdInfoQuery)?;
        };

        let Some(t_id) = args.next() else {
            return Err(QueryError::NoIdInfoQuery)?;
        };

        let listing = self.device.get_listing().await;
        let Some(game) = listing.get_game(t_id) else {
            return Err(QueryError::GameNotFound(t_id.to_string()))?;
        };

        let mut args = args.map(|v| v.parse::<u64>());
        let (Ok(start), Ok(end)) = (
            args.next().unwrap_or(Ok(0)),
            args.next().unwrap_or(Ok(game.size())),
        ) else {
            return Err(QueryError::BadRange)?;
        };

        let mut f = File::open(game.path()).map_err(Self::map_io)?;

        f.seek(SeekFrom::Start(start))
            .map_err(Self::map_io)
            .unwrap();

        println!("Requested file: {:?}, range {start}-{end}", game.path());
        self.device
            .write_chunked_with_caching(f, end - start, self.buff)
            .await
    }

    async fn handle_info(self) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        let Some(t_id) = self.query.and_then(|q| q.split('/').next()) else {
            return Err(QueryError::NoIdInfoQuery)?;
        };

        if let Some(game) = self.device.get_listing().await.get_game(t_id) {
            self.write_str(&miniserde::json::to_string(&GameEntry::try_from(game)?))
                .await
        } else {
            Err(QueryError::GameNotFound(t_id.to_string()))?
        }
    }

    async fn handle_search(self) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        let s = self.device.get_listing().await.serialise();
        self.write_str(&s).await
    }

    async fn handle_queue(self) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        self.write_str("[]").await // another half-baked feature by Blawar
    }
}
