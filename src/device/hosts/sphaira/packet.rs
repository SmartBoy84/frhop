use bytemuck::{Pod, Zeroable};

type PacketType = [u8; 4];

const LIST_MAGIC: [u8; 4] = *b"TUL0";

#[repr(C, packed)]
#[derive(Default, Pod, Clone, Copy, Zeroable)]
pub struct ListPacketResponse {
    packet_type: PacketType,
    nsp_n: u32,
    _padding: [u8; 8],
}

#[repr(C, packed)]
#[derive(Default, Pod, Clone, Copy, Zeroable)]
struct CmdPacket {
    pub magic: [u8; 4], // b'TUC0'
    pub cmd_type: u8,
    pub _padding0: [u8; 3],
    pub cmd_id: u32,    // le
    pub data_size: u64, // le
    pub _padding1: [u8; 12],
}

impl ListPacketResponse {
    pub fn new(nsp_n: u32) -> Self {
        Self {
            packet_type: LIST_MAGIC,
            nsp_n,
            ..Default::default()
        }
    }
}
