use bytemuck::bytes_of;

use crate::device::{
    SwitchHost, SwitchHostImpl, hosts::sphaira::packet::ListPacketResponse,
    interface::SwitchInterface, writer::SwitchHostWriterExt,
};

mod packet;

pub struct SphairaInterface {
    inner: SwitchInterface,
}

impl SwitchHostWriterExt for SphairaInterface {}

impl SwitchHost for SphairaInterface {
    async fn start_talkin_buddy(self) -> Result<(), crate::device::SwitchCommError> {
        // let mut buff = self.inner.get_buff().await;
        // let listing = self.inner.get_listing().await;
        // let file_id_map = listing.file_map();
        // let list_header = ListPacketResponse::new(file_id_map.len() as u32);
        // println!("Writing header ");
        // buff = self
        //     .write_from_reader(
        //         bytes_of(&list_header),
        //         size_of::<ListPacketResponse>(),
        //         buff,
        //     )
        //     .await?;
        // for (file, _) in file_id_map {
        //     println!("Writing");
        //     let n = format!("{file}\n");
        //     buff = self.write_from_reader(n.as_bytes(), n.len(), buff).await?;
        // }
        // println!("Here?");
        todo!()
    }
}

impl From<SwitchInterface> for SphairaInterface {
    fn from(inner: SwitchInterface) -> Self {
        Self { inner }
    }
}

impl SwitchHostImpl for SphairaInterface {
    fn get_interface(&self) -> &SwitchInterface {
        &self.inner
    }

    async fn listen_response(&mut self) -> Result<(), crate::device::SwitchCommError> {
        todo!()
    }
    
    fn get_interface_mut(&mut self) -> &mut SwitchInterface {
        &mut self.inner
    }
}
