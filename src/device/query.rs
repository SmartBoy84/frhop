/*
info, queue, search, download
*/

use std::{
    fs::File,
    io::{self, Seek, SeekFrom},
};

use thiserror::Error;

use crate::{
    device::{TinfoilDevice, TinfoilDeviceCommError},
    game::entry::GameEntry,
};

#[derive(Error, Debug)]
pub enum QueryErrorKind {
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
pub enum QueryError {
    #[error("bad query: {}", .0.0)]
    BadQuery((QueryErrorKind, Vec<u8>)),
    #[error("{0}")]
    CommError(#[from] TinfoilDeviceCommError),
}

impl QueryErrorKind {
    fn to_error(self, b: Vec<u8>) -> QueryError {
        QueryError::BadQuery((self, b))
    }
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
    pub async fn process_query(
        device: &'a TinfoilDevice,
        buff: Vec<u8>,
        payload: &'a str,
    ) -> Result<Vec<u8>, QueryError> {
        Self::from_payload(device, buff, payload)?
            .handle_query()
            .await
    }

    /// buff MUST contain payload at this point
    fn from_payload(
        device: &'a TinfoilDevice,
        buff: Vec<u8>,
        payload: &'a str,
    ) -> Result<Self, QueryError> {
        let mut args = payload.split('/');
        if args.next() != Some("") {
            return Err(QueryErrorKind::UnsupportedCmd(payload.to_string()).to_error(buff));
            // be somewhat strict; tinfoil over usb will always have '/' at start
        }

        // tinfoil's usb args are: `/{req_name}/{req_type}/{query url}`
        let (Some(endpoint), Some(req_type), query) = (
            args.next(),
            args.next(),
            args.next(), // may not exist
        ) else {
            return Err(QueryErrorKind::UnsupportedCmd(payload.to_string()).to_error(buff));
        };

        Ok(Self {
            buff,
            device,
            endpoint,
            req_type,
            query,
        })
    }

    async fn handle_query(self) -> Result<Vec<u8>, QueryError> {
        match self.endpoint {
            "api" => self.route_api().await,
            _ => {
                return Err(
                    QueryErrorKind::UnsupportedEndpoint(self.endpoint.to_string())
                        .to_error(self.buff),
                );
            }
        }
    }
}

impl Query<'_> {
    async fn write_str(self, res: &str) -> Result<Vec<u8>, QueryError> {
        Ok(self.device.write_str(res, self.buff).await?)
    }
}

impl Query<'_> {
    async fn route_api(self) -> Result<Vec<u8>, QueryError> {
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
            _ => Err(QueryErrorKind::UnsupportedReqType(self.req_type.to_string()).to_error(self.buff))?,
        }
    }

    async fn handle_download(self) -> Result<Vec<u8>, QueryError> {
        let Some(mut args) = self.query.map(|q| q.split('/')) else {
            return Err(QueryErrorKind::NoIdInfoQuery.to_error(self.buff))?;
        };

        let Some(t_id) = args.next() else {
            return Err(QueryErrorKind::NoIdInfoQuery.to_error(self.buff))?;
        };

        let listing = self.device.get_listing().await;
        let Some(game) = listing.get_game(t_id) else {
            return Err(QueryErrorKind::GameNotFound(t_id.to_string()).to_error(self.buff))?;
        };

        let mut args = args.map(|v| v.parse::<u64>());
        let (Ok(start), Ok(end)) = (
            args.next().unwrap_or(Ok(0)),
            args.next().unwrap_or(Ok(game.size())),
        ) else {
            return Err(QueryErrorKind::BadRange.to_error(self.buff))?;
        };

        let mut f = match File::open(game.path()) {
            Ok(f) => f,
            Err(e) => return Err(QueryErrorKind::from(e).to_error(self.buff))?,
        };

        if let Err(e) = f.seek(SeekFrom::Start(start)) {
            return Err(QueryErrorKind::from(e).to_error(self.buff))?;
        };

        println!("Requested file: {:?}, range {start}-{end}", game.path());
        Ok(self
            .device
            .write_chunked_with_caching(f, end - start, self.buff)
            .await?)
    }

    async fn handle_info(self) -> Result<Vec<u8>, QueryError> {
        let Some(t_id) = self.query.and_then(|q| q.split('/').next()) else {
            return Err(QueryErrorKind::NoIdInfoQuery.to_error(self.buff))?;
        };

        if let Some(game) = self.device.get_listing().await.get_game(t_id) {
            let game_entry = match GameEntry::try_from(game) {
                Ok(g_e) => g_e,
                Err(e) => return Err(QueryErrorKind::from(e).to_error(self.buff))?,
            };
            self.write_str(&miniserde::json::to_string(&game_entry))
                .await
        } else {
            Err(QueryErrorKind::GameNotFound(t_id.to_string()).to_error(self.buff))?
        }
    }

    async fn handle_search(self) -> Result<Vec<u8>, QueryError> {
        let s = self.device.get_listing().await.serialise();
        self.write_str(&s).await
    }

    async fn handle_queue(self) -> Result<Vec<u8>, QueryError> {
        self.write_str("[]").await // another half-baked feature by Blawar
    }
}
