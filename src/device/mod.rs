pub mod hosts;
pub mod interface;
mod writer;
// mod hosts;

use std::{fmt::Display, io, sync::LazyLock};

use nusb::{DeviceId, transfer::TransferError};
use smol::lock::Mutex;
use thiserror::Error;

use crate::device::{
    hosts::{sphaira::SphairaInterface, tinfoil::TinfoilInterface},
    interface::SwitchInterface,
};

const CHUNK_SIZE: u64 = 0x400000; // ~4mb - good chunk size
const FILE_CACHE_N: usize = 50; // keep n chunk sizes in memory to reduce disk reads

// TX/RX
const TX_BUFF_N: usize = CHUNK_SIZE as usize;
const RX_BUFF_N: usize = 100; // 100 bytes more than sufficient I'd say

struct HostId {
    vendor: u16,
    prod: u16,
}
const DEVICES: [HostId; 2] = [
    HostId {
        vendor: 0x16C0,
        prod: 0x27E2,
    },
    HostId {
        vendor: 0x057E,
        prod: 0x3000,
    },
];

// purely an internal variable so not difficult to reason about this
static CONNECTED_IDS: LazyLock<Mutex<Vec<DeviceId>>> = LazyLock::new(|| Mutex::new(vec![]));

#[derive(Error, Debug)]
pub enum SwitchCommError {
    #[error("recv error")]
    RecvError(#[from] TransferError),
    #[error("bad magic id")]
    BadMagic,
    #[error("unknown cmd")]
    UnknownCmd, // tinfoil only has command == 1
    #[error("bad utf-8 in cmd")]
    CorruptedCmd,
    // following should be fatal
    #[error("payload r/w failed")]
    SwitchRw(#[from] io::Error),
}

#[derive(PartialEq, Clone, Copy)]
pub enum UsbClient {
    Tinfoil,
    Sphaira,
}

impl Display for UsbClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Tinfoil => "Tinfoil",
                Self::Sphaira => "Sphaira",
            }
        )
    }
}

impl TryFrom<&str> for UsbClient {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "s" => Self::Sphaira,
            "t" => Self::Tinfoil,
            _ => return Err(()),
        })
    }
}

impl Default for UsbClient {
    fn default() -> Self {
        Self::Tinfoil
    }
}

impl UsbClient {
    pub async fn start_interface(&self, device: SwitchInterface) -> Result<(), SwitchCommError> {
        match self {
            Self::Tinfoil => TinfoilInterface::from(device).start_talkin_buddy().await,
            Self::Sphaira => SphairaInterface::from(device).start_talkin_buddy().await,
        }
    }
}

trait SwitchHostImpl: From<SwitchInterface> {
    async fn listen_response(&mut self) -> Result<(), SwitchCommError>;
    fn get_interface_mut(&mut self) -> &mut SwitchInterface;
    fn get_interface(&self) -> &SwitchInterface;
}

#[allow(private_bounds)] // hide SwitchHostImpl methods
pub trait SwitchHost: SwitchHostImpl + Send {
    /// loops infinitely; waiting for request and responding accordingly
    fn start_talkin_buddy(self) -> impl Future<Output = Result<(), SwitchCommError>> + Send;
}
