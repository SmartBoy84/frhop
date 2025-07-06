use bytemuck::{Pod, Zeroable};

type PacketType = [u8; 4];

const LIST_MAGIC: [u8; 4] = *b"TUL0";
pub const CMD_MAGIC: [u8; 4] = *b"TUC0";

#[repr(C, packed)]
#[derive(Default, Pod, Clone, Copy, Zeroable)]
pub struct ListPacketResponse {
    packet_type: PacketType,
    len: u32,
    _padding: [u8; 8],
}

#[repr(C, packed)]
#[derive(Default, Pod, Clone, Copy, Zeroable)]
pub struct CmdPacket {
    pub magic: [u8; 4], // b'TUC0'
    pub cmd_type: u8,
    pub _padding0: [u8; 3],
    pub cmd_id: u32,    // le
    pub data_size: u64, // le
    pub _padding1: [u8; 12],
}

#[repr(C, packed)]
#[derive(Default, Pod, Clone, Copy, Zeroable)]
pub struct FileRangePacket {
    pub range_size: u64,
    pub range_offset: u64,
    pub name_len: u64,
    pub _padding0: [u8; 8]
}

impl ListPacketResponse {
    pub fn new(len: u32) -> Self {
        Self {
            packet_type: LIST_MAGIC,
            len,
            ..Default::default()
        }
    }
}

impl CmdPacket {
    pub fn new(cmd_id: u32, data_size: u64) -> Self {
        Self {
            cmd_id,
            data_size,
            ..Default::default()
        }
    }
}
