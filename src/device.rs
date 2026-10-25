// SPDX-License-Identifier: GPL-2.0-or-later
// Port of vial-gui's vial_device.py and util.find_vial_devices — device
// discovery and the device abstraction.

use hidapi::{DeviceInfo, HidApi, HidDevice};
use serde_json::Value;

use crate::protocol::constants::{VIAL_SERIAL_NUMBER_MAGIC, VIBL_SERIAL_NUMBER_MAGIC};
use crate::protocol::keyboard::Keyboard;
use crate::via::ViaStack;

#[derive(Clone, Debug)]
pub struct DeviceDescriptor {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: String,
    pub manufacturer_string: String,
    pub product_string: String,
    pub path: String,
    pub usage_page: u16,
    pub usage: u16,
}

impl DeviceDescriptor {
    pub fn from_hid(info: &DeviceInfo) -> Self {
        DeviceDescriptor {
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            serial_number: info.serial_number().unwrap_or_default().to_string(),
            manufacturer_string: info.manufacturer_string().unwrap_or_default().to_string(),
            product_string: info.product_string().unwrap_or_default().to_string(),
            path: info.path().to_string_lossy().to_string(),
            usage_page: info.usage_page(),
            usage: info.usage(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Bootloader,
    Dummy,
}

/// A discovered device. Opening a keyboard produces a `Keyboard` via reload.
#[derive(Clone)]
pub struct Device {
    pub desc: DeviceDescriptor,
    pub kind: DeviceKind,
    pub sideload: bool,
    pub via_stack: bool,
    /// For sideload/VIA devices, the definition JSON used to reload.
    pub definition: Option<Value>,
}

impl Device {
    pub fn title(&self) -> String {
        let s = format!(
            "{} {}",
            self.desc.manufacturer_string, self.desc.product_string
        )
        .trim()
        .to_string();
        if self.sideload {
            format!("{} [sideload]", s)
        } else if self.via_stack {
            format!("{} [VIA]", s)
        } else {
            s
        }
    }

    /// Open the device, returning a fully loaded `Keyboard` when applicable.
    pub fn open(&self) -> Result<Option<Keyboard>, String> {
        match self.kind {
            DeviceKind::Dummy => Ok(None),
            DeviceKind::Bootloader => Ok(None),
            DeviceKind::Keyboard => {
                let dev = open_path(&self.desc.path).map_err(|e| {
                    if cfg!(target_os = "linux") {
                        format!(
                            "{} (on Linux you need a udev rule to access Vial keyboards — see https://get.vial.today/manual/linux-udev.html)",
                            e
                        )
                    } else {
                        e
                    }
                })?;
                let mut kb = Keyboard::new(dev);
                kb.reload(self.definition.as_ref())?;
                Ok(Some(kb))
            }
        }
    }
}

fn open_path(path: &str) -> Result<HidDevice, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
    api.open_path(cpath.as_c_str()).map_err(|e| e.to_string())
}

fn is_rawhid(desc: &DeviceDescriptor) -> bool {
    desc.usage_page == 0xFF60 && desc.usage == 0x61
}

/// Enumerate connected Vial/VIA devices.
pub fn find_vial_devices(
    api: &HidApi,
    via_stack: &ViaStack,
    sideload_vid: u16,
    sideload_pid: u16,
) -> Vec<Device> {
    let mut filtered = Vec::new();
    for info in api.device_list() {
        let desc = DeviceDescriptor::from_hid(info);
        let vid = desc.vendor_id;
        let pid = desc.product_id;

        if vid == sideload_vid && pid == sideload_pid {
            if is_rawhid(&desc) {
                filtered.push(Device {
                    desc,
                    kind: DeviceKind::Keyboard,
                    sideload: true,
                    via_stack: false,
                    definition: None,
                });
            }
        } else if desc.serial_number.contains(VIAL_SERIAL_NUMBER_MAGIC) {
            if is_rawhid(&desc) {
                filtered.push(Device {
                    desc,
                    kind: DeviceKind::Keyboard,
                    sideload: false,
                    via_stack: false,
                    definition: None,
                });
            }
        } else if desc.serial_number.contains(VIBL_SERIAL_NUMBER_MAGIC) {
            filtered.push(Device {
                desc,
                kind: DeviceKind::Bootloader,
                sideload: false,
                via_stack: false,
                definition: None,
            });
        } else if via_stack.contains(vid, pid) {
            if is_rawhid(&desc) {
                filtered.push(Device {
                    desc,
                    kind: DeviceKind::Keyboard,
                    sideload: false,
                    via_stack: true,
                    definition: via_stack.definition(vid, pid),
                });
            }
        }
    }
    filtered
}
