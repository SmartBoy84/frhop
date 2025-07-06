use futures_io::AsyncRead;
use smol::io::{self, AsyncReadExt, AsyncWriteExt};

use crate::device::{CHUNK_SIZE, SwitchCommError, SwitchHost};

/*
Look here to optimise file transfer speeds
*/

#[derive(PartialEq)]
pub enum ChunkStatus {
    Remaining,
    End,
}

pub trait SwitchHostWriterExt: SwitchHost {
    async fn write_next_chunk<R: AsyncRead>(
        &mut self,
        reader: R,
    ) -> Result<ChunkStatus, SwitchCommError> {
        let n = io::copy(reader.take(CHUNK_SIZE), self.get_interface_mut().get_tx()).await?;
        // don't need to flush as tx's internal buffer size == CHUNK_SIZE

        if n < CHUNK_SIZE as u64 {
            self.get_interface_mut().get_tx().flush().await?;
            return Ok(ChunkStatus::End);
        } else {
            return Ok(ChunkStatus::Remaining);
        }
    }
}
