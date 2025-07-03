pub mod interface;
mod packet;
mod query;
mod writer;

use std::{
    cell::Cell,
    io,
    sync::{Arc, RwLock, mpsc},
};

use nusb::{Device, DeviceInfo, Interface, transfer::TransferError};
use thiserror::Error;

use crate::{device::query::QueryError, game::listing::Listing};

const DEFAULT_CMD: u32 = 1; // tinfoil only every has 1
const CHUNK_SIZE: usize = 0x400000; // ~4mb - good chunk size
const FILE_CACHE_N: usize = 50; // keep n chunk sizes in memory to reduce disk reads

#[derive(Error, Debug)]
pub enum TinfoilDeviceCommError {
    #[error("recv error")]
    RecvError(#[from] TransferError),
    #[error("bad magic id")]
    BadMagic,
    #[error("unknown cmd")]
    UnknownCmd, // tinfoil only has command == 1
    #[error("bad utf-8 in cmd")]
    CorruptedCmd,
    // following should be non-fatal
    #[error("bad query")]
    BadQuery(#[from] QueryError),
    #[error("payload r/w failed")]
    PayloadTransferFailed(#[from] io::Error),
}

struct DeviceId {
    vendor: u16,
    prod: u16,
}
const DEVICES: [DeviceId; 2] = [
    DeviceId {
        vendor: 0x16C0,
        prod: 0x27E2,
    },
    DeviceId {
        vendor: 0x057E,
        prod: 0x3000,
    },
];

pub struct TinfoilDevice {
    device: Device,
    device_info: DeviceInfo,
    interface: Interface, // tinfoil's interface - there's only one really...
    in_ep: u8,
    out_ep: u8,
    recv_buff: Vec<u8>, // bit of interior mutability
    listing: Arc<RwLock<Listing>>,
}
