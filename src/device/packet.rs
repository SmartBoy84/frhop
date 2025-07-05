use bytemuck::{Pod, Zeroable, from_bytes};

// tinfoil's magic header
const MAGIC_HEADER: [u8; 4] = [0x12, 0x12, 0x12, 0x12];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Default)]
pub struct CommandPacket {
    pub magic: [u8; 4],
    pub cmd: u32,
    pub size: u64,
    thread_id: u32,
    packet_i: u16,
    packet_n: u16,
    timestamp: u64,
}

#[derive(miniserde::Serialize, Debug)]
pub struct StatusResponse {
    #[serde(rename = "success")]
    success: bool,
    #[serde(rename = "message")]
    message: String,
}

impl StatusResponse {
    pub fn new(success: bool, message: String) -> Self {
        Self { success, message }
    }
}

impl CommandPacket {
    /// from_bytes panics for size-mismatch but not possible to call this function without a buffer of right size
    /// None if magic header mismatch
    pub fn from_raw(buff: &[u8]) -> Option<&Self> {
        let p = from_bytes::<Self>(buff); // awesome! no copying, I return a reference whose lifetime is linked to buff!
        if p.magic != MAGIC_HEADER {
            None
        } else {
            Some(p)
        }
    }

    pub fn new(cmd: u32, size: u64) -> Self {
        Self {
            magic: MAGIC_HEADER,
            cmd,
            size,
            ..Default::default() // ig these were features that were forgotten about...
        }
    }
}
