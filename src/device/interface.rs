use std::{io, mem, sync::Arc};

use nusb::{
    Device, DeviceInfo,
    hotplug::HotplugEvent,
    transfer::{Direction, RequestBuffer},
};
use smol::{
    lock::{Mutex, RwLock, RwLockReadGuard},
    stream::StreamExt,
};
use thiserror::Error;

use crate::{device::{
    packet::CommandPacket, query::Query, TinfoilDevice, TinfoilDeviceCommError, CONNECTED_IDS, DEFAULT_CMD, DEVICES
}, listing::Listing};

#[derive(Error, Debug)]
pub enum TinfoilDeviceInitError {
    #[error("io error")]
    Io(#[from] io::Error),
    #[error("endpoint not found")]
    EpNotFound,
    #[error("interface not found")]
    NoInterface,
}

async fn open_device(device_info: &DeviceInfo) -> Option<Device> {
    let mut connected_ids = CONNECTED_IDS.lock().await; // lock at the start to prevent weird races

    if connected_ids.contains(&device_info.id()) {
        return None;
    }

    if DEVICES
        .iter()
        .find(|d| d.prod == device_info.product_id() && d.vendor == device_info.vendor_id())
        .is_none()
    {
        None
    } else {
        // usb errors aren't fatal
        let device = device_info
            .open()
            .inspect_err(|e| eprintln!("Usb error: {e:?}"))
            .ok()?;

        connected_ids.push(device_info.id());

        Some(device)
    }
}

pub async fn get_conn() -> Result<(Device, DeviceInfo), TinfoilDeviceInitError> {
    for d_info in nusb::list_devices()? {
        if let Some(d) = open_device(&d_info).await {
            return Ok((d, d_info));
        }
    }

    let mut watcher = nusb::watch_devices()?;

    // wait for connection
    loop {
        if let Some(HotplugEvent::Connected(d_info)) = watcher.next().await
            && let Some(d) = open_device(&d_info).await
        {
            return Ok((d, d_info));
        }
    }
}

impl TinfoilDevice {
    pub async fn wait_new(listing: Arc<RwLock<Listing>>) -> Result<Self, TinfoilDeviceInitError> {
        let (device, device_info) = get_conn().await.unwrap();

        // following not important as tinfoil interface has one config anyways - these aren't supported on windows/WinUSB
        // device.reset()?;
        // let device = get_conn().await.unwrap();

        // tinfoil's usb interface - one config, 2 interfaces but still try to be dynamic...
        // device.set_configuration(1).unwrap();

        let interface = device.claim_interface(0)?; // why are there 2 interfaces anyways...

        let a_set = interface
            .descriptors()
            .find(|d| d.alternate_setting() == 0)
            .ok_or(TinfoilDeviceInitError::NoInterface)?;

        // could've hardcoded addresses but future-proofing
        let out_ep = a_set
            .endpoints()
            .find(|ep| ep.direction() == Direction::Out)
            .ok_or(TinfoilDeviceInitError::EpNotFound)?
            .address();

        let in_ep = a_set
            .endpoints()
            .find(|ep| ep.direction() == Direction::In)
            .ok_or(TinfoilDeviceInitError::EpNotFound)?
            .address();

        Ok(Self {
            device,
            device_info,
            interface,
            in_ep,
            out_ep,
            recv_buff: Mutex::new(Vec::with_capacity(mem::size_of::<CommandPacket>())), // this buff will grow as we receive other commands
            listing,
        })
    }

    pub async fn get_listing(&self) -> RwLockReadGuard<'_, Listing> {
        self.listing.read().await
    }

    pub async fn read(
        &self,
        buff: Vec<u8>,
        size: usize,
    ) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        // I wish ::reuse took &Vec - could've neatened up the code...
        Ok(self
            .interface
            .bulk_in(self.in_ep, RequestBuffer::reuse(buff, size))
            .await
            .into_result()?)
    }

    /// REMEMBER; fully drain the Vec as api uses .len() method to determine size of out payload
    pub async fn write(&self, buff: Vec<u8>) -> Result<Vec<u8>, TinfoilDeviceCommError> {
        Ok(self
            .interface
            .bulk_out(self.out_ep, buff)
            .await
            .into_result()?
            .reuse())
    }

    // listens and responds to requests
    pub async fn listen_req(&self) -> Result<(), TinfoilDeviceCommError> {
        // doesn't really make sense for the external user why recv takes a mutable reference - that's why I use refcell

        // recv_buff only taken here, so no issue with this
        let mut recv_buff = self.get_buff().await;

        recv_buff = self
            .read(recv_buff, mem::size_of::<CommandPacket>())
            .await?;

        // from_raw panics for size-mismatch but bulk_in already errs if incomplete read - always will be 32 bits
        // extract all relevant deets - tinfoil makes everything else 0 - so we can reuse recv_buff
        let &CommandPacket { cmd, size, .. } =
            CommandPacket::from_raw(&recv_buff).ok_or(TinfoilDeviceCommError::BadMagic)?;

        if cmd != DEFAULT_CMD {
            return Err(TinfoilDeviceCommError::UnknownCmd);
        }

        recv_buff = self.read(recv_buff, size as usize).await?;

        let payload = std::str::from_utf8(&recv_buff)
            .map_err(|_| TinfoilDeviceCommError::CorruptedCmd)?
            .trim()
            .to_string(); // must copy since Query will be writing to recv_buff 

        // handle and respond to query
        let query = Query::from_payload(self, recv_buff, &payload)?;
        recv_buff = query.handle_query().await?; // all single-threaded baby! read mod.rs

        self.return_buff(recv_buff).await; // place back the Vec
        Ok(())
    }

    /// loops infinitely; waiting for request and responding accordingly
    pub async fn start_talkin_buddy(self) -> Result<(), TinfoilDeviceCommError> {
        // println!("Hey twitcho! Waiting for requests...");
        // man this is so nice! tinfoil comms are ping-pong style (i.e., no continuation - request then response, repeat)
        loop {
            self.listen_req().await?;
        }
    }
}
