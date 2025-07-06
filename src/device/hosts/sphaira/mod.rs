use std::io;

use bytemuck::bytes_of;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use smol::io::AsyncWriteExt;
use thiserror::Error;

use crate::device::{
    SwitchCommError, SwitchHost, SwitchHostImpl, hosts::sphaira::packet::ListPacketResponse,
    interface::SwitchInterface, writer::SwitchHostWriterExt,
};

mod packet;
mod server;

pub struct SphairaInterface {
    inner: SwitchInterface,
    current_f: Option<String>,
}

#[derive(Error, Debug)]
pub enum SphairaError {
    #[error("switch comm erorr")]
    SwitchComm(#[from] SwitchCommError),
    #[error("non utf-8 char in name")]
    BadFileName,
    #[error("io erro")]
    IoError(#[from] io::Error),
}

#[derive(PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum CmdType {
    Exit = 0,
    FileRange = 1,
}

impl SwitchHostWriterExt for SphairaInterface {}

impl SwitchHost for SphairaInterface {
    async fn start_talkin_buddy(mut self) -> Result<(), crate::device::SwitchCommError> {
        let listing = self.inner.get_listing().await;
        let file_id_map = listing
            .file_map()
            .keys()
            .map(|s| format!("{s}\n"))
            .collect::<Vec<_>>();
        drop(listing);

        let total_len = file_id_map.iter().map(|s| s.len()).sum::<usize>();
        let list_header = ListPacketResponse::new(total_len as u32);

        let tx = self.get_interface_mut().get_tx();

        println!("writing list header");
        tx.write(bytes_of(&list_header)).await?;
        tx.flush().await?;

        println!("sending list");
        for f in file_id_map {
            tx.write(f.as_bytes()).await?;
        }
        tx.flush().await?;

        // start main loop
        loop {
            let cmd_type = match self.poll_command().await {
                Ok(p) => p,
                Err(SphairaError::SwitchComm(s_e)) => return Err(s_e),
                Err(e) => {
                    eprintln!("{e:?}");
                    continue;
                }
            };

            if cmd_type == CmdType::Exit {
                return Ok(());
            }
        }
    }
}

impl From<SwitchInterface> for SphairaInterface {
    fn from(inner: SwitchInterface) -> Self {
        Self {
            inner,
            current_f: None,
        }
    }
}

impl SwitchHostImpl for SphairaInterface {
    fn get_interface(&self) -> &SwitchInterface {
        &self.inner
    }

    fn get_interface_mut(&mut self) -> &mut SwitchInterface {
        &mut self.inner
    }
}
