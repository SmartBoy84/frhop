use std::{io::SeekFrom, mem};

use bytemuck::{bytes_of, from_bytes};
use smol::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::device::{
    SwitchCommError, SwitchHostImpl,
    hosts::sphaira::{
        CmdType, SphairaError, SphairaInterface,
        packet::{CMD_MAGIC, CmdPacket, FileRangePacket},
    },
    writer::{ChunkStatus, SwitchHostWriterExt},
};

impl SphairaInterface {
    pub async fn poll_command(&mut self) -> Result<CmdType, SphairaError> {
        let mut buf = [0u8; mem::size_of::<CmdPacket>()];
        self.get_interface_mut()
            .get_rx()
            .read_exact(&mut buf)
            .await
            .map_err(SwitchCommError::from)?;

        let cmd_packet: &CmdPacket = from_bytes(&buf);

        if cmd_packet.magic != CMD_MAGIC {
            return Err(SwitchCommError::BadMagic)?;
        }

        let Some(cmd_type): Option<CmdType> = CmdType::try_from(cmd_packet.cmd_id).ok() else {
            return Err(SwitchCommError::UnknownCmd)?;
        };

        match cmd_type {
            CmdType::FileRange => self.serve_file(cmd_packet.data_size).await?,
            CmdType::Exit => (),
        }

        Ok(cmd_type)
    }

    pub async fn serve_file(&mut self, datasize: u64) -> Result<(), SphairaError> {
        let mut buf = [0u8; mem::size_of::<FileRangePacket>()];
        let rx = self.get_interface_mut().get_rx();
        rx.read_exact(&mut buf)
            .await
            .map_err(SwitchCommError::from)?;

        let file_range_packet: &FileRangePacket = from_bytes(&buf);

        let mut name = vec![0u8; file_range_packet.name_len as usize];
        rx.read_exact(&mut name)
            .await
            .map_err(SwitchCommError::from)?;

        let name = String::from_utf8(name).map_err(|_| SphairaError::BadFileName)?;

        let mut f = fs::File::open(&name).await?;
        f.seek(SeekFrom::Start(file_range_packet.range_offset))
            .await?;

        let mut f = f.take(file_range_packet.range_size);

        if Some(&name) != self.current_f.as_ref() {
            println!("\"{name}\" requested");
            self.current_f.replace(name);
        }

        let file_range_header = CmdPacket::new(CmdType::FileRange.into(), datasize);
        let tx = self.get_interface_mut().get_tx();

        tx.write(bytes_of(&file_range_header))
            .await
            .map_err(SwitchCommError::from)?;
        tx.flush().await?;

        // write the file
        loop {
            if self.write_next_chunk(&mut f).await? == ChunkStatus::End {
                break;
            }
        }
        Ok(())
    }
}
