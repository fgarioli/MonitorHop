use crate::TriggerEvent;
use anyhow::{anyhow, Result};
use rusb::{Context, Device, HotplugBuilder, Registration, UsbContext};
use std::sync::mpsc::{self, Sender};

fn device_id<T: UsbContext>(device: &Device<T>) -> Option<String> {
    device
        .device_descriptor()
        .map(|d| format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()))
        .ok()
}

pub struct MacosHotplugTrigger {
    usb_device: String,
}

impl MacosHotplugTrigger {
    pub fn new(usb_device: &str) -> Self {
        Self {
            usb_device: usb_device.to_lowercase(),
        }
    }
}

impl crate::TriggerSource for MacosHotplugTrigger {
    fn watch(&self) -> mpsc::Receiver<TriggerEvent> {
        let (tx, rx) = mpsc::channel();
        let usb_device = self.usb_device.clone();
        std::thread::spawn(move || {
            if let Err(err) = run_hotplug_loop(usb_device, tx) {
                log::error!("USB hotplug detection failed: {:?}", err);
            }
        });
        rx
    }
}

struct HotplugHandler {
    usb_device: String,
    sender: Sender<TriggerEvent>,
}

impl<T: UsbContext> rusb::Hotplug<T> for HotplugHandler {
    fn device_arrived(&mut self, device: Device<T>) {
        if device_id(&device).as_deref() == Some(self.usb_device.as_str()) {
            let _ = self.sender.send(TriggerEvent::HostGainedFocus);
        }
    }
    fn device_left(&mut self, device: Device<T>) {
        if device_id(&device).as_deref() == Some(self.usb_device.as_str()) {
            let _ = self.sender.send(TriggerEvent::HostLostFocus);
        }
    }
}

fn run_hotplug_loop(usb_device: String, sender: Sender<TriggerEvent>) -> Result<()> {
    if !rusb::has_hotplug() {
        return Err(anyhow!("libusb hotplug api unsupported on this platform"));
    }
    let context = Context::new()?;
    let handler = HotplugHandler { usb_device, sender };
    let _registration: Registration<Context> =
        HotplugBuilder::new().enumerate(true).register(&context, Box::new(handler))?;
    loop {
        context.handle_events(None)?;
    }
}
