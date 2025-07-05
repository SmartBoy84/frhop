use std::{io, sync::Arc, time::Duration};

use nusb::{
    Device, DeviceInfo, Interface,
    hotplug::HotplugEvent,
    transfer::{Direction, RequestBuffer},
};
use smol::{
    Timer,
    lock::{Mutex, RwLock, RwLockReadGuard},
    stream::StreamExt,
};
use thiserror::Error;

use crate::{
    device::{CONNECTED_IDS, DEVICES, SwitchCommError},
    listing::Listing,
};

#[derive(Error, Debug)]
pub enum SwitchInitError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("endpoint not found")]
    EpNotFound,
    #[error("interface not found")]
    NoInterface,
}

pub struct SwitchInterface {
    device: Device,
    device_info: DeviceInfo,
    interface: Interface, // tinfoil's interface - there's only one really...
    in_ep: u8,
    out_ep: u8,
    recv_buff: Mutex<Vec<u8>>, // bit of interior mutability
    listing: Arc<smol::lock::RwLock<Listing>>,
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

pub async fn get_conn() -> Result<(Device, DeviceInfo), SwitchInitError> {
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

impl SwitchInterface {
    pub async fn get_buff(&self) -> Vec<u8> {
        let mut v = self.recv_buff.lock().await;
        std::mem::take(&mut *v)
    }

    pub async fn return_buff(&self, buff: Vec<u8>) {
        let mut v = self.recv_buff.lock().await;
        *v = buff;
    }

    pub async fn wait_new(listing: Arc<RwLock<Listing>>) -> Result<Self, SwitchInitError> {
        let (device, device_info) = get_conn().await?;

        // following not important as tinfoil interface has one config anyways - these aren't supported on windows/WinUSB
        // device.reset()?;
        // let (device, device_info) = get_conn().await?;

        // tinfoil's usb interface - one config, 2 interfaces but still try to be dynamic...
        // device.set_configuration(1)?;

        let interface = match device.claim_interface(0) {
            Ok(i) => i,
            Err(_) => {
                // for windows, interfaces might only be available after a small delay
                Timer::after(Duration::from_millis(500)).await;
                device.claim_interface(0)?
            }
        }; // why are there 2 interfaces anyways...

        let a_set = interface
            .descriptors()
            .find(|d| d.alternate_setting() == 0)
            .ok_or(SwitchInitError::NoInterface)?;

        // could've hardcoded addresses but future-proofing
        let out_ep = a_set
            .endpoints()
            .find(|ep| ep.direction() == Direction::Out)
            .ok_or(SwitchInitError::EpNotFound)?
            .address();

        let in_ep = a_set
            .endpoints()
            .find(|ep| ep.direction() == Direction::In)
            .ok_or(SwitchInitError::EpNotFound)?
            .address();

        Ok(Self {
            device,
            device_info,
            interface,
            in_ep,
            out_ep,
            recv_buff: Mutex::new(Vec::new()), // this buff will grow as we receive other commands
            listing,
        })
    }

    pub async fn get_listing(&self) -> RwLockReadGuard<'_, Listing> {
        self.listing.read().await
    }

    pub async fn read(&self, buff: Vec<u8>, size: usize) -> Result<Vec<u8>, SwitchCommError> {
        // I wish ::reuse took &Vec - could've neatened up the code...
        Ok(self
            .interface
            .bulk_in(self.in_ep, RequestBuffer::reuse(buff, size))
            .await
            .into_result()?)
    }

    /// REMEMBER; fully drain the Vec as api uses .len() method to determine size of out payload
    pub async fn write(&self, buff: Vec<u8>) -> Result<Vec<u8>, SwitchCommError> {
        Ok(self
            .interface
            .bulk_out(self.out_ep, buff)
            .await
            .into_result()?
            .reuse())
    }
}
