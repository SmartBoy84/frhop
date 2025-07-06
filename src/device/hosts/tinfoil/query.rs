/*
info, queue, search, download
*/
use smol::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};
use std::io::{self, SeekFrom};

use bytemuck::bytes_of;
use miniserde::json;
use smol::io::AsyncWriteExt;
use thiserror::Error;

use crate::{
    device::{
        CHUNK_SIZE, SwitchCommError, SwitchHostImpl,
        hosts::tinfoil::{DEFAULT_CMD, TinfoilInterface, packet::CommandPacket},
        writer::{ChunkStatus, SwitchHostWriterExt},
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
    #[error("bad query: {}", .0)]
    BadQuery(#[from] TinfoilQueryErrorKind),
    #[error("{0}")]
    CommError(#[from] SwitchCommError),
}

// &str's lifetime is shorter than TinfoilDevice but Query doesn't care about TinfoilDevice longer than it cares about the &str so it works
pub struct TinfoilQuery<'a> {
    device: &'a mut TinfoilInterface,
    endpoint: &'a str,
    req_type: &'a str,
    query: Option<&'a str>,
}

impl<'a> TinfoilQuery<'a> {
    pub async fn process_query(
        device: &'a mut TinfoilInterface,
        payload: &'a str,
    ) -> Result<(), TinfoilQueryError> {
        Self::from_payload(device, payload)?.handle_query().await
    }

    /// buff MUST contain payload at this point
    fn from_payload(
        device: &'a mut TinfoilInterface,
        payload: &'a str,
    ) -> Result<Self, TinfoilQueryError> {
        let mut args = payload.split('/');
        if args.next() != Some("") {
            return Err(TinfoilQueryErrorKind::UnsupportedCmd(payload.to_string()))?;
            // be somewhat strict; tinfoil over usb will always have '/' at start
        }

        // tinfoil's usb args are: `/{req_name}/{req_type}/{query url}`
        let (Some(endpoint), Some(req_type), query) = (
            args.next(),
            args.next(),
            args.next(), // may not exist
        ) else {
            return Err(TinfoilQueryErrorKind::UnsupportedCmd(payload.to_string()))?;
        };

        Ok(Self {
            device,
            endpoint,
            req_type,
            query,
        })
    }

    async fn handle_query(self) -> Result<(), TinfoilQueryError> {
        match self.endpoint {
            "api" => self.route_api().await,
            _ => {
                return Err(TinfoilQueryErrorKind::UnsupportedEndpoint(
                    self.endpoint.to_string(),
                ))?;
            }
        }
    }
}

impl TinfoilQuery<'_> {
    async fn write_bytes(&mut self, buf: &[u8]) -> Result<(), TinfoilQueryError> {
        let tx = self.device.get_interface_mut().get_tx();
        tx.write(buf).await.map_err(SwitchCommError::from)?;
        tx.flush().await.map_err(SwitchCommError::from)?;
        Ok(())
    }

    async fn write_str(&mut self, res: &str) -> Result<(), TinfoilQueryError> {
        self.write_bytes(&bytes_of(&CommandPacket::new(
            DEFAULT_CMD,
            res.len() as u64,
        )))
        .await?;
        self.write_bytes(res.as_bytes()).await?;
        Ok(())
    }
}

impl TinfoilQuery<'_> {
    async fn route_api(mut self) -> Result<(), TinfoilQueryError> {
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
            _ => Err(TinfoilQueryErrorKind::UnsupportedReqType(self.req_type.to_string()))?,
        }?;
        Ok(())
    }

    async fn handle_download(&mut self) -> Result<(), TinfoilQueryError> {
        let Some(mut args) = self.query.map(|q| q.split('/')) else {
            return Err(TinfoilQueryErrorKind::NoIdInfoQuery)?;
        };

        let Some(t_id) = args.next() else {
            return Err(TinfoilQueryErrorKind::NoIdInfoQuery)?;
        };

        let listing = self.device.get_interface().get_listing().await;
        let Some(game) = listing.get_game(ListingIndex::TitleId(t_id)) else {
            return Err(TinfoilQueryErrorKind::GameNotFound(t_id.to_string()))?;
        };

        let mut args = args.map(|v| v.parse::<u64>());
        let (Ok(start), Ok(end)) = (
            args.next().unwrap_or(Ok(0)),
            args.next().unwrap_or(Ok(game.size())),
        ) else {
            return Err(TinfoilQueryErrorKind::BadRange)?;
        };

        if start > end {
            return Err(TinfoilQueryErrorKind::BadRange)?;
        }

        let mut f = File::open(game.path())
            .await
            .map_err(TinfoilQueryErrorKind::from)?;

        f.seek(SeekFrom::Start(start))
            .await
            .map_err(TinfoilQueryErrorKind::from)?;

        println!("Requested file: {:?}, range {start}-{end}", game.path());
        drop(listing);

        let header = CommandPacket::new(DEFAULT_CMD, CHUNK_SIZE);

        let mut f = f.take(end - start);

        loop {
            self.write_bytes(&bytes_of(&header)).await?;
            if self.device.write_next_chunk(&mut f).await? == ChunkStatus::End {
                break;
            }
        }
        Ok(())
    }

    async fn handle_info(&mut self) -> Result<(), TinfoilQueryError> {
        let Some(t_id) = self.query.and_then(|q| q.split('/').next()) else {
            return Err(TinfoilQueryErrorKind::NoIdInfoQuery)?;
        };

        let listing = self.device.get_interface().get_listing().await;

        let Some(game) = listing.get_game(ListingIndex::TitleId(t_id)) else {
            return Err(TinfoilQueryErrorKind::GameNotFound(t_id.to_string()))?;
        };
        let game_entry = GameEntry::try_from(game).map_err(TinfoilQueryErrorKind::from)?;

        let s = miniserde::json::to_string(&game_entry);

        drop(listing);
        self.write_str(&s).await
    }

    async fn handle_search(&mut self) -> Result<(), TinfoilQueryError> {
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

    async fn handle_queue(&mut self) -> Result<(), TinfoilQueryError> {
        self.write_str("[]").await // another half-baked feature by Blawar
    }
}
