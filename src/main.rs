// SPDX-License-Identifier: GPL-2.0-or-later
//! Vial GUI — a Rust port of vial-gui for configuring QMK/Vial keyboards.

// Hide the console window on Windows in release builds (keep it in debug for logging).
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

// WIP: the full protocol/keycode surface is implemented ahead of its UI, so a
// number of items are intentionally not yet referenced. Remove once all editors
// (macros, RGB, tap dance, etc.) are wired up.
#![allow(dead_code)]

mod any_keycode;
mod app;
mod constants;
mod device;
mod display_defs;
mod keycodes;
mod kle;
mod protocol;
mod rgb;
mod ui;
mod util;
mod via;

fn main() -> eframe::Result {
    // Diagnostic: list all HID devices as seen by hidapi.
    if std::env::args().any(|a| a == "--list-devices") {
        list_devices();
        return Ok(());
    }
    app::run()
}

fn list_devices() {
    let api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("HID init failed: {}", e);
            return;
        }
    };
    for info in api.device_list() {
        let serial = info.serial_number().unwrap_or_default();
        let is_vial = serial.contains(protocol::constants::VIAL_SERIAL_NUMBER_MAGIC);
        let is_vibl = serial.contains(protocol::constants::VIBL_SERIAL_NUMBER_MAGIC);
        let is_rawhid = info.usage_page() == 0xFF60 && info.usage() == 0x61;
        let tag = if is_vial {
            "  <-- VIAL KEYBOARD"
        } else if is_vibl {
            "  <-- VIAL BOOTLOADER"
        } else if is_rawhid {
            "  <-- raw HID (candidate)"
        } else {
            ""
        };
        println!(
            "{:04X}:{:04X}  usage={:04X}:{:02X}  iface={}  serial=\"{}\"  {} {}{}",
            info.vendor_id(),
            info.product_id(),
            info.usage_page(),
            info.usage(),
            info.interface_number(),
            serial,
            info.manufacturer_string().unwrap_or_default(),
            info.product_string().unwrap_or_default(),
            tag,
        );
    }

    // Also exercise the app's actual detection + open path.
    println!("\n=== app detection (find_vial_devices) ===");
    let stack = via::ViaStack::default();
    let devices = device::find_vial_devices(&api, &stack, 0, 0);
    println!("found {} device(s)", devices.len());
    for d in &devices {
        println!("  - {}  [path={}]", d.title(), d.desc.path);
        match d.open() {
            Ok(Some(kb)) => {
                println!("    OPENED OK: {}x{} matrix, {} layers, {} keys",
                    kb.rows, kb.cols, kb.layers, kb.keys.len());
                println!("    lighting: rgblight={} backlight={} vialrgb={}",
                    kb.lighting_qmk_rgblight, kb.lighting_qmk_backlight, kb.lighting_vialrgb);
                if kb.lighting_vialrgb {
                    println!("    VialRGB version={} max_brightness={} layer_support={} effects={:?}",
                        kb.rgb_version, kb.rgb_maximum_brightness, kb.rgb_layer_support,
                        kb.rgb_supported_effects.len());
                    if kb.rgb_layer_support {
                        println!("    per-layer configs: {:?}", kb.rgb_layer_configs);
                    } else {
                        println!("    global config: mode={} speed={} hsv={:?}", kb.rgb_mode, kb.rgb_speed, kb.rgb_hsv);
                    }
                }
                match kb.save_layout() {
                    Ok(bytes) => {
                        println!("    .vil2 JSON ({} bytes):", bytes.len());
                        let s = String::from_utf8_lossy(&bytes);
                        println!("{}", s.lines().take(24).collect::<Vec<_>>().join("\n"));
                    }
                    Err(e) => println!("    save_layout FAILED: {}", e),
                }
            }
            Ok(None) => println!("    open returned None (bootloader/dummy)"),
            Err(e) => println!("    OPEN FAILED: {}", e),
        }
    }
}
