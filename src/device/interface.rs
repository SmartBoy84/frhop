use std::{sync::Arc, time::Duration};

use nusb::{
    Device, DeviceInfo, Interface,
    hotplug::HotplugEvent,
    io::{EndpointRead, EndpointWrite},
    transfer::{Bulk, Direction},
};
use smol::{
    Timer,
    lock::{RwLock, RwLockReadGuard},
    stream::StreamExt,
};
use thiserror::Error;

use crate::{
    device::{CONNECTED_IDS, DEVICES, RX_BUFF_N, TX_BUFF_N},
    listing::Listing,
};

#[derive(Error, Debug)]
pub enum SwitchInitError {
    #[error("usb error: {0}")]
    UsbError(#[from] nusb::Error),
    #[error("endpoint not found")]
    EpNotFound,
    #[error("interface not found")]
    NoInterface,
}

pub struct SwitchInterface {
    device: Device,
    device_info: DeviceInfo,
    interface: Interface, // tinfoil's interface - there's only one really...
    rx: EndpointRead<Bulk>,
    tx: EndpointWrite<Bulk>,
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
            .await
            .inspect_err(|e| eprintln!("Usb error: {e:?}"))
            .ok()?;

        connected_ids.push(device_info.id());

        Some(device)
    }
}

pub async fn get_conn() -> Result<(Device, DeviceInfo), SwitchInitError> {
    for d_info in nusb::list_devices().await? {
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
    pub async fn wait_new(listing: Arc<RwLock<Listing>>) -> Result<Self, SwitchInitError> {
        let (device, device_info) = get_conn().await?;

        // following not important as tinfoil interface has one config anyways - these aren't supported on windows/WinUSB
        // device.reset()?;
        // let (device, device_info) = get_conn().await?;

        // tinfoil's usb interface - one config, 2 interfaces but still try to be dynamic...
        // device.set_configuration(1)?;

        let interface = match device.claim_interface(0).await {
            Ok(i) => i,
            Err(_) => {
                // for windows, interfaces might only be available after a small delay
                Timer::after(Duration::from_millis(500)).await;
                device.claim_interface(0).await?
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

        let tx = EndpointWrite::new(interface.endpoint::<Bulk, _>(out_ep)?, TX_BUFF_N);
        let rx = EndpointRead::new(interface.endpoint::<Bulk, _>(in_ep)?, RX_BUFF_N);

        Ok(Self {
            device,
            device_info,
            interface,
            tx,
            rx,
            listing,
        })
    }

    pub async fn get_listing(&self) -> RwLockReadGuard<'_, Listing> {
        self.listing.read().await
    }

    pub fn get_rx(&mut self) -> &mut EndpointRead<Bulk> {
        &mut self.rx
    }

    pub fn get_tx(&mut self) -> &mut EndpointWrite<Bulk> {
        &mut self.tx
    }
}
