// SPDX-License-Identifier: GPL-2.0-or-later
// RGB lighting configurator (port of editor/rgb_configurator.py).

use egui::Ui;

use crate::protocol::keyboard::Keyboard;
use crate::rgb;

/// Draw the RGB lighting panel. `selected_layer` tracks the chosen VialRGB layer.
pub fn show(ui: &mut Ui, kb: &mut Keyboard, selected_layer: &mut usize) {
    if kb.lighting_vialrgb {
        show_vialrgb(ui, kb, selected_layer);
        ui.separator();
    }
    if kb.lighting_qmk_rgblight {
        show_qmk_rgblight(ui, kb);
        ui.separator();
    }
    if kb.lighting_qmk_backlight {
        show_qmk_backlight(ui, kb);
        ui.separator();
    }

    save_button(ui, kb);
}

/// A "Save" button (with hint) that persists RGB config to the keyboard's EEPROM.
/// Reused on both the Lighting and Keymap tabs.
pub fn save_button(ui: &mut Ui, kb: &Keyboard) {
    ui.horizontal(|ui| {
        ui.colored_label(
            egui::Color32::from_rgb(0xcc, 0xa8, 0x50),
            "Changes apply live — press “Save” to persist them to the keyboard.",
        );
        if ui.button("Save").clicked() {
            if let Err(e) = kb.save_rgb() {
                log::warn!("save_rgb failed: {}", e);
            }
        }
    });
}

fn show_vialrgb(ui: &mut Ui, kb: &mut Keyboard, selected_layer: &mut usize) {
    ui.heading("VialRGB");

    let has_layer = kb.rgb_layer_support;
    let max_brightness = kb.rgb_maximum_brightness.max(1) as u8;

    // Layer selector (only for per-layer firmware: VialRGB protocol >= 2).
    if has_layer {
        ui.horizontal(|ui| {
            ui.label("RGB Layer:");
            for l in 0..kb.layers {
                if ui.selectable_label(*selected_layer == l, format!("{}", l)).clicked() {
                    *selected_layer = l;
                }
            }
        });
    } else {
        *selected_layer = 0;
    }

    let layer = *selected_layer;
    let cfg = kb.rgb_config(layer);

    // Effect dropdown.
    let mut effects: Vec<u16> = kb.rgb_supported_effects.iter().copied().collect();
    effects.sort_unstable();
    let names: Vec<String> = effects.iter().map(|e| rgb::vialrgb_name(*e)).collect();
    let mut current_idx = effects
        .iter()
        .position(|e| *e == cfg.mode)
        .unwrap_or(0);

    let mut effect_changed = false;
    let selected_name = names.get(current_idx).cloned().unwrap_or_default();
    egui::ComboBox::from_label("Effect")
        .selected_text(selected_name)
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                if ui.selectable_label(current_idx == i, name).clicked() {
                    current_idx = i;
                    effect_changed = true;
                }
            }
        });
    if effect_changed {
        let mode = effects[current_idx];
        let _ = if has_layer {
            kb.set_vialrgb_layer_mode(layer, mode)
        } else {
            kb.set_vialrgb_mode(mode)
        };
    }

    // Color swatch (hue/saturation), brightness kept separate.
    ui.horizontal(|ui| {
        ui.label("Color:");
        let mut hsva = egui::ecolor::Hsva {
            h: cfg.hue as f32 / 255.0,
            s: cfg.sat as f32 / 255.0,
            v: 1.0,
            a: 1.0,
        };
        if ui.color_edit_button_hsva(&mut hsva).changed() {
            let h = (hsva.h.clamp(0.0, 1.0) * 255.0) as u8;
            let s = (hsva.s.clamp(0.0, 1.0) * 255.0) as u8;
            let v = cfg.val;
            let _ = if has_layer {
                kb.set_vialrgb_layer_color(layer, h, s, v)
            } else {
                kb.set_vialrgb_color(h, s, v)
            };
        }
    });

    // Brightness slider.
    let mut val = cfg.val;
    if ui
        .add(egui::Slider::new(&mut val, 0..=max_brightness).text("Brightness"))
        .changed()
    {
        let _ = if has_layer {
            kb.set_vialrgb_layer_brightness(layer, val)
        } else {
            kb.set_vialrgb_brightness(val)
        };
    }

    // Speed slider.
    let mut speed = cfg.speed;
    if ui
        .add(egui::Slider::new(&mut speed, 0..=255u8).text("Speed"))
        .changed()
    {
        let _ = if has_layer {
            kb.set_vialrgb_layer_speed(layer, speed)
        } else {
            kb.set_vialrgb_speed(speed)
        };
    }
}

fn show_qmk_rgblight(ui: &mut Ui, kb: &mut Keyboard) {
    ui.heading("Underglow (QMK rgblight)");

    // Effect dropdown.
    let current_idx = kb.underglow_effect.clamp(0, 36) as usize;
    let names: Vec<String> = rgb::QMK_RGBLIGHT_EFFECTS
        .iter()
        .map(|(_, n, _)| n.to_string())
        .collect();
    let mut idx = current_idx;
    let mut effect_changed = false;
    egui::ComboBox::from_label("Underglow Effect")
        .selected_text(names[idx].clone())
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                if ui.selectable_label(idx == i, name).clicked() {
                    idx = i;
                    effect_changed = true;
                }
            }
        });
    if effect_changed {
        let _ = kb.set_qmk_rgblight_effect(idx as u8);
    }

    // Brightness.
    let mut brightness = kb.underglow_brightness.clamp(0, 255) as u8;
    if ui
        .add(egui::Slider::new(&mut brightness, 0..=255u8).text("Brightness"))
        .changed()
    {
        let _ = kb.set_qmk_rgblight_brightness(brightness);
    }

    // Color (only for effects that support it).
    if rgb::QMK_RGBLIGHT_EFFECTS
        .get(idx)
        .map(|(_, _, cp)| *cp)
        .unwrap_or(false)
    {
        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut hsva = egui::ecolor::Hsva {
                h: kb.underglow_color.0 as f32 / 255.0,
                s: kb.underglow_color.1 as f32 / 255.0,
                v: 1.0,
                a: 1.0,
            };
            if ui.color_edit_button_hsva(&mut hsva).changed() {
                let h = (hsva.h.clamp(0.0, 1.0) * 255.0) as u8;
                let s = (hsva.s.clamp(0.0, 1.0) * 255.0) as u8;
                let v = kb.underglow_brightness.clamp(0, 255) as u8;
                let _ = kb.set_qmk_rgblight_color(h, s, v);
            }
        });
    }
}

fn show_qmk_backlight(ui: &mut Ui, kb: &mut Keyboard) {
    ui.heading("Backlight (QMK backlight)");

    let mut brightness = kb.backlight_brightness.clamp(0, 255) as u8;
    if ui
        .add(egui::Slider::new(&mut brightness, 0..=255u8).text("Brightness"))
        .changed()
    {
        let _ = kb.set_qmk_backlight_brightness(brightness);
    }

    let mut breathing = kb.backlight_effect == 1;
    if ui.checkbox(&mut breathing, "Breathing").changed() {
        let _ = kb.set_qmk_backlight_effect(if breathing { 1 } else { 0 });
    }
}
