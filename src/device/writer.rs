/*
Unsafe *fully localised here
It's to get performance past what's possible with libusb in python
*/

use std::io::{Cursor, Read};

use crate::device::{CHUNK_SIZE, FILE_CACHE_N, SwitchCommError, SwitchHost};

/*
Look here to optimise file transfer speeds
*/

pub trait SwitchHostWriterExt: SwitchHost {
    async fn write_str(&self, res: &str, buff: Vec<u8>) -> Result<Vec<u8>, SwitchCommError> {
        let b = res.as_bytes();
        self.write_from_reader(b, b.len(), buff).await
    }

    /// transfer n bytes *exactly (err if not) from a reader -> tinfoil
    async fn write_from_reader<R: Read>(
        &self,
        mut reader: R,
        n: usize,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, SwitchCommError> {
        buff = self.write_header(n, buff).await?;

        buff.reserve(n); // slightly inefficient - sets capacity on top of pre-existing TODO; fix
        unsafe {
            buff.set_len(n);
        }

        reader.read_exact(&mut buff[..])?;

        self.get_interface().write(buff).await
    }

    #[allow(unused)]
    async fn write_from_vec<R: Read>(
        &self,
        payload: Vec<u8>,
        mut buff: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<u8>), SwitchCommError> {
        buff = self.write_header(payload.len(), buff).await?;
        buff = self.get_interface().write(buff).await?;
        let payload = self.get_interface().write(payload).await?;
        Ok((payload, buff))
    }

    #[allow(unused)]
    async fn write_chunked_no_caching<R: Read>(
        &self,
        mut reader: R,
        size: u64,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, SwitchCommError> {
        /* Tried to make this as fast as possible with caching to minimise file reads */

        let chunk_n = size / CHUNK_SIZE as u64;

        for _ in 0..chunk_n {
            buff = self
                .write_from_reader(&mut reader, CHUNK_SIZE, buff)
                .await?;
        }
        if size % CHUNK_SIZE as u64 > 0 {
            buff = self
                .write_from_reader(&mut reader, size as usize % CHUNK_SIZE, buff)
                .await?;
        }
        Ok(buff)
    }

    #[allow(unused)]
    async fn write_chunked_with_caching<R: Read>(
        &self,
        mut reader: R,
        size: u64,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, SwitchCommError> {
        /* Tried to make this as fast as possible with caching to minimise file reads but doesn't make that big a difference...*/
        let size = (FILE_CACHE_N * CHUNK_SIZE).min(size as usize);

        let mut cache_buff = Vec::with_capacity(size); // allocate a file cache
        unsafe {
            cache_buff.set_len(size);
        }

        loop {
            let n = reader.read(&mut cache_buff[..])?;
            if n == 0 {
                break;
            }
            let mut cursor = Cursor::new(&cache_buff[..n]);
            for _ in 0..n / CHUNK_SIZE {
                buff = self
                    .write_from_reader(&mut cursor, CHUNK_SIZE, buff)
                    .await?;
            }
            if n % CHUNK_SIZE > 0 {
                buff = self
                    .write_from_reader(&mut cursor, n % CHUNK_SIZE, buff)
                    .await?;
                break;
            }
        }
        Ok(buff)

        // my other scuffed hyper-optimised implementation which was just as fast...

        // loop {
        //     let n = reader.read(&mut cache_buff[..])?;
        //     if n == 0 {
        //         break;
        //     }
        //     let mut i = 0;
        //     for _ in 0..n / CHUNK_SIZE {
        //         let ptr = cache_buff[i..i + CHUNK_SIZE].as_ptr() as *mut u8;
        //         let mut payload = unsafe { Vec::from_raw_parts(ptr, CHUNK_SIZE, CHUNK_SIZE) };

        //         (payload, buff) = self.write_from_vec(payload, buff).await?;
        //         mem::forget(payload);
        //         i += CHUNK_SIZE;
        //     }
        //     if n % CHUNK_SIZE != 0 {
        //         let ptr = cache_buff[i..i + n % CHUNK_SIZE].as_ptr() as *mut u8;
        //         let mut payload = unsafe { Vec::from_raw_parts(ptr, CHUNK_SIZE, CHUNK_SIZE) };
        //         (payload, buff) = self.write_from_vec(payload, buff).await?;
        //         mem::forget(payload);
        //         break;
        //     }
        // }
    }
}
