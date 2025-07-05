use std::mem;

use bytemuck::bytes_of;
use miniserde::json;

use crate::device::{
    hosts::tinfoil::{
        packet::{CommandPacket, StatusResponse},
        query::{TinfoilQuery, TinfoilQueryError},
    }, interface::SwitchInterface, writer::SwitchHostWriterExt, SwitchCommError, SwitchHost, SwitchHostImpl
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
    fn get_interface(&self) -> &SwitchInterface {
        &self.inner
    }

    // write the packet header
    async fn write_header(
        &self,
        n: usize,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, crate::device::SwitchCommError> {
        let header = CommandPacket::new(DEFAULT_CMD, n as u64); // slight overhead, but must be done..
        // fiddle with hardcoding this struct as in struct
        unsafe {
            // guarantee; initialised to have least capacity == size_of::<CommandPacket>()
            buff.set_len(mem::size_of::<CommandPacket>());
        }
        buff.copy_from_slice(bytes_of(&header));
        self.inner.write(buff).await
    }

    // listens and responds to requests
    async fn listen_req(&self) -> Result<(), SwitchCommError> {
        // doesn't really make sense for the external user why recv takes a mutable reference - that's why I use refcell

        // recv_buff only taken here, so no issue with this
        let interface = self.get_interface();
        let mut recv_buff = self.get_interface().get_buff().await;

        /*
        Sphaira waits for USB list rather than request queue itself
        */

        recv_buff = interface
            .read(recv_buff, mem::size_of::<CommandPacket>())
            .await?;

        // from_raw panics for size-mismatch but bulk_in already errs if incomplete read - always will be 32 bits
        // extract all relevant deets - tinfoil makes everything else 0 - so we can reuse recv_buff
        let &CommandPacket { cmd, size, .. } =
            CommandPacket::from_raw(&recv_buff).ok_or(SwitchCommError::BadMagic)?;

        if cmd != DEFAULT_CMD {
            return Err(SwitchCommError::UnknownCmd);
        }

        recv_buff = interface.read(recv_buff, size as usize).await?;

        let payload = std::str::from_utf8(&recv_buff)
            .map_err(|_| SwitchCommError::CorruptedCmd)?
            .trim()
            .to_string(); // must copy since Query will be writing to recv_buff 

        // handle and respond to query
        let b = match TinfoilQuery::process_query(self, recv_buff, &payload).await {
            Ok(b) => b,
            Err(TinfoilQueryError::CommError(comm_e)) => return Err(comm_e),
            Err(TinfoilQueryError::BadQuery((bq_e, b))) => {
                // query erorrs are non-fatal by design
                let e = bq_e.to_string();
                println!("Query error; {}", &e);
                self.write_str(&json::to_string(&StatusResponse::new(false, e)), b)
                    .await?
            }
        };
        interface.return_buff(b).await;
        Ok(())
        // all single-threaded baby! read mod.rs
    }
}

impl SwitchHost for TinfoilInterface {
    async fn start_talkin_buddy(self) -> Result<(), SwitchCommError> {
        // println!("Hey twitcho! Waiting for requests...");
        // man this is so nice! tinfoil comms are ping-pong style (i.e., no continuation - request then response, repeat)
        loop {
            self.listen_req().await?; // fail on TinfoilDeviceCommErorr - that's why I consume it, after this function errs, struct is in undefined state (no buff Vec)
        }
    }
}
