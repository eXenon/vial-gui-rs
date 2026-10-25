// SPDX-License-Identifier: GPL-2.0-or-later
// Utility helpers ported from vial-gui's util.py.

use hidapi::HidDevice;

use crate::protocol::constants::MSG_LEN;

/// Send a message to a raw HID device and read back a response (vial's `hid_send`).
pub fn hid_send(dev: &HidDevice, msg: &[u8], retries: usize) -> Result<Vec<u8>, String> {
    if msg.len() > MSG_LEN {
        return Err("message must be less than 32 bytes".to_string());
    }
    let mut padded = msg.to_vec();
    padded.resize(MSG_LEN, 0u8);

    // Prepend report id 0x00 for hidapi.
    let mut out = vec![0u8];
    out.extend_from_slice(&padded);

    let mut retries = retries;
    let mut first = true;
    let mut data: Vec<u8> = Vec::new();

    while retries > 0 {
        retries -= 1;
        if !first {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        first = false;

        match dev.write(&out) {
            Ok(n) if n == MSG_LEN + 1 => {}
            Ok(_) => continue,
            Err(_) => continue,
        }

        let mut buf = vec![0u8; MSG_LEN];
        match dev.read_timeout(&mut buf, 500) {
            Ok(n) if n > 0 => {
                buf.truncate(n);
                data = buf;
                break;
            }
            _ => continue,
        }
    }

    if data.is_empty() {
        return Err("failed to communicate with the device".to_string());
    }
    Ok(data)
}

/// Pads a message to the VIBL fixed 64-byte length.
pub fn pad_for_vibl(msg: &[u8]) -> Vec<u8> {
    if msg.len() > 64 {
        panic!("vibl message too long");
    }
    let mut v = msg.to_vec();
    v.resize(64, 0u8);
    v
}

/// Yield successive `sz`-sized chunks of `data`.
pub fn chunks(data: &[u8], sz: usize) -> impl Iterator<Item = &[u8]> {
    data.chunks(sz)
}

// Big-endian pack/unpack helpers (mirroring Python's struct.pack/unpack ">...").
pub fn be_u16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

pub fn be_u32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

pub fn le_u32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

pub fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

pub fn unpack_be_u16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

pub fn unpack_be_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

pub fn unpack_le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

pub fn unpack_le_u64(b: &[u8]) -> u64 {
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

pub fn unpack_le_u16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
