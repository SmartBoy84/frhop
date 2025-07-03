/*
Unsafe *fully localised here
It's to get performance past what's possible with libusb in python
*/

use std::{io::Read, mem};

use bytemuck::bytes_of;
use std::io::Cursor;

use crate::device::{
    CHUNK_SIZE, DEFAULT_CMD, FILE_CACHE_N, TinfoilDevice, TinfoilDeviceCommError,
    packet::CommandPacket,
};

/*
Look here to optimise file transfer speeds
*/

impl TinfoilDevice {
    // these don't need to be hyper-optimised - for file writes, buff taken out at start not on every chunk write
    pub async fn get_buff(&self) -> Vec<u8> {
        let mut v = self.recv_buff.lock().await;
        std::mem::take(&mut *v)
    }

    pub async fn return_buff(&self, buff: Vec<u8>) {
        let mut v = self.recv_buff.lock().await;
        *v = buff;
    }

    /// transfer n bytes *exactly (err if not) from a reader -> tinfoil
    pub async fn write_from_reader<R: Read>(
        &self,
        mut reader: R,
        n: usize,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        buff = self.write_header(n, buff).await?;
        // buff.resize(n, 0); // highly inefficient - calloc's

        buff.reserve(n); // slightly inefficient - sets capacity on top of pre-existing TODO; fix
        unsafe {
            buff.set_len(n);
        }

        reader.read_exact(&mut buff[..])?;

        self.write(buff).await
    }

    #[allow(unused)]
    pub async fn write_from_vec(
        &self,
        payload: Vec<u8>,
        mut buff: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<u8>), TinfoilDeviceCommError> {
        buff = self.write_header(payload.len(), buff).await?;
        let payload = self.write(payload).await?;
        Ok((payload, buff))
    }

    /// copies packet header into buff (can't combine with payload - needs to be separate packet)
    pub async fn write_header(
        &self,
        size: usize,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        let header = CommandPacket::new(DEFAULT_CMD, size as u64); // slight overhead, but must be done..
        // fiddle with hardcoding this struct as in struct
        unsafe {
            // guarantee; initialised to have least capacity == size_of::<CommandPacket>()
            buff.set_len(mem::size_of::<CommandPacket>());
        }
        buff.copy_from_slice(bytes_of(&header));
        self.write(buff).await
    }

    #[allow(unused)]
    pub async fn write_chunked_no_caching<R: Read>(
        &self,
        mut reader: R,
        size: u64,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, TinfoilDeviceCommError> {
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
    pub async fn write_chunked_with_caching<R: Read>(
        &self,
        mut reader: R,
        size: u64,
        mut buff: Vec<u8>,
    ) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        /* Tried to make this as fast as possible with caching to minimise file reads but doesn't make that big a difference...*/

        let size = (FILE_CACHE_N * CHUNK_SIZE).min(size as usize);

        let mut cache_buff = Vec::with_capacity(size); // allocate a file cache
        unsafe {
            cache_buff.set_len(size);
        }

        loop {
            let n = reader.read(&mut cache_buff[..]).unwrap();
            if n == 0 {
                break;
            }
            let mut cursor = Cursor::new(&cache_buff[..n]);
            for _ in 0..n / CHUNK_SIZE {
                buff = self
                    .write_from_reader(&mut cursor, CHUNK_SIZE, buff)
                    .await
                    .unwrap();
            }
            if n % CHUNK_SIZE > 0 {
                buff = self
                    .write_from_reader(&mut cursor, n % CHUNK_SIZE, buff)
                    .await
                    .unwrap();
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
