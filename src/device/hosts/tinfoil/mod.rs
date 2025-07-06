use std::{io::Write, mem};

use smol::io::AsyncReadExt;

use miniserde::json;

use crate::device::{
    SwitchCommError, SwitchHost, SwitchHostImpl,
    hosts::tinfoil::{
        packet::{CommandPacket, StatusResponse},
        query::{TinfoilQuery, TinfoilQueryError},
    },
    interface::SwitchInterface,
    writer::SwitchHostWriterExt,
};

mod packet;
mod query;

const DEFAULT_CMD: u32 = 1; // tinfoil only every has 1

pub struct TinfoilInterface {
    inner: SwitchInterface,
}

impl From<SwitchInterface> for TinfoilInterface {
    fn from(inner: SwitchInterface) -> Self {
        Self { inner }
    }
}

impl SwitchHostWriterExt for TinfoilInterface {}

impl SwitchHostImpl for TinfoilInterface {
    fn get_interface_mut(&mut self) -> &mut SwitchInterface {
        &mut self.inner
    }

    fn get_interface(&self) -> &SwitchInterface {
        &self.inner
    }
}

impl TinfoilInterface {
    // listens and responds to requests
    async fn listen_response(&mut self) -> Result<(), SwitchCommError> {
        let interface = self.get_interface_mut();

        let mut recv_buff = [0u8; mem::size_of::<CommandPacket>()];
        interface.get_rx().read(&mut recv_buff).await?;

        // from_raw panics for size-mismatch but bulk_in already errs if incomplete read - always will be 32 bits
        // extract all relevant deets - tinfoil makes everything else 0 - so we can reuse recv_buff
        let &CommandPacket { cmd, size, .. } =
            CommandPacket::from_raw(&recv_buff).ok_or(SwitchCommError::BadMagic)?;

        if cmd != DEFAULT_CMD {
            return Err(SwitchCommError::UnknownCmd);
        }

        let mut p = vec![0u8; size as usize];
        interface.get_rx().read(&mut p).await?;

        let payload = String::from_utf8(p).map_err(|_| SwitchCommError::CorruptedCmd)?;
        // handle and respond to query
        if let Err(e) = TinfoilQuery::process_query(self, &payload).await {
            match e {
                TinfoilQueryError::CommError(com_e) => return Err(com_e),
                TinfoilQueryError::BadQuery(bq_e) => {
                    let e = bq_e.to_string();
                    println!("Query error; {}", &e);
                    self.get_interface_mut()
                        .get_tx()
                        .write(&json::to_string(&StatusResponse::new(false, e)).as_bytes())?;
                }
            }
        };
        Ok(())
        // all single-threaded baby! read mod.rs
    }
}

impl SwitchHost for TinfoilInterface {
    async fn start_talkin_buddy(mut self) -> Result<(), SwitchCommError> {
        // println!("Hey twitcho! Waiting for requests...");
        // man this is so nice! tinfoil comms are ping-pong style (i.e., no continuation - request then response, repeat)
        loop {
            self.listen_response().await?; // fail on TinfoilDeviceCommErorr - that's why I consume it, after this function errs, struct is in undefined state (no buff Vec)
        }
    }
}
