// SPDX-License-Identifier: GPL-2.0-or-later
// Port of vial-gui's protocol/keyboard_comm.py — low-level communication with a
// Vial-enabled keyboard.

use std::collections::{HashMap, HashSet};

use hidapi::HidDevice;
use serde_json::Value;

use super::constants::*;
use crate::keycodes::{CustomKeycode, KeycodeDb};
use crate::kle::{self, Key};
use crate::util::{self, be_u16, be_u32, le_u16, le_u32, unpack_be_u16, unpack_be_u32, unpack_le_u32, unpack_le_u64};

#[derive(Debug, Clone)]
pub struct ProtocolError;

/// Low-level communication with a Vial-enabled keyboard.
pub struct Keyboard {
    dev: HidDevice,

    pub definition: Value,
    pub via_protocol: i32,
    pub vial_protocol: i32,
    pub keyboard_id: u64,

    pub rows: usize,
    pub cols: usize,
    pub layers: usize,
    pub layout_labels: Option<Value>,
    pub layout_options: i32,

    pub keys: Vec<Key>,
    pub encoders: Vec<Key>,
    pub encoder_count: usize,
    pub rowcol: Vec<(usize, usize)>,
    pub encoderpos: Vec<usize>,

    pub layout: HashMap<(usize, usize, usize), String>,
    pub encoder_layout: HashMap<(usize, usize, usize), String>,

    pub custom_keycodes: Option<Vec<CustomKeycode>>,
    pub midi: Option<String>,
    pub vibl: bool,

    pub macro_count: usize,
    pub macro_memory: usize,
    pub macro_data: Vec<u8>,

    pub tap_dance_count: usize,
    pub combo_count: usize,
    pub key_override_count: usize,
    pub alt_repeat_key_count: usize,
    pub supported_features: HashSet<String>,

    pub keycode_db: KeycodeDb,

    // RGB state (populated later; present for API completeness).
    pub lighting_qmk_rgblight: bool,
    pub lighting_qmk_backlight: bool,
    pub lighting_vialrgb: bool,

    // QMK rgblight (underglow)
    pub underglow_brightness: i32,
    pub underglow_effect: i32,
    pub underglow_effect_speed: i32,
    pub underglow_color: (u8, u8),

    // QMK backlight
    pub backlight_brightness: i32,
    pub backlight_effect: i32,

    // VialRGB
    pub rgb_version: i32,
    pub rgb_maximum_brightness: i32,
    pub rgb_layer_support: bool,
    pub rgb_mode: i32,
    pub rgb_speed: i32,
    pub rgb_hsv: (u8, u8, u8),
    pub rgb_supported_effects: HashSet<u16>,
    pub rgb_layer_configs: Vec<RgbLayerConfig>,

    // VialRGB per-key "no light" mask (protocol v3)
    pub rgb_key_disable_support: bool,
    pub rgb_number_leds: usize,
    pub rgb_led_matrix: Vec<Option<(usize, usize)>>, // LED index -> matrix (row, col)
    pub rgb_key_disable: Vec<Vec<u8>>,               // per layer, one bit per LED
}

/// Per-layer VialRGB config (matches `vialrgb_layer_config_t` in the firmware).
#[derive(Clone, Copy, Debug, Default)]
pub struct RgbLayerConfig {
    pub mode: u16,
    pub speed: u8,
    pub hue: u8,
    pub sat: u8,
    pub val: u8,
}

impl Keyboard {
    pub fn new(dev: HidDevice) -> Self {
        Keyboard {
            dev,
            definition: Value::Null,
            via_protocol: -1,
            vial_protocol: -1,
            keyboard_id: 0,
            rows: 0,
            cols: 0,
            layers: 0,
            layout_labels: None,
            layout_options: -1,
            keys: Vec::new(),
            encoders: Vec::new(),
            encoder_count: 0,
            rowcol: Vec::new(),
            encoderpos: Vec::new(),
            layout: HashMap::new(),
            encoder_layout: HashMap::new(),
            custom_keycodes: None,
            midi: None,
            vibl: false,
            macro_count: 0,
            macro_memory: 0,
            macro_data: Vec::new(),
            tap_dance_count: 0,
            combo_count: 0,
            key_override_count: 0,
            alt_repeat_key_count: 0,
            supported_features: HashSet::new(),
            keycode_db: KeycodeDb::default(),
            lighting_qmk_rgblight: false,
            lighting_qmk_backlight: false,
            lighting_vialrgb: false,
            underglow_brightness: -1,
            underglow_effect: -1,
            underglow_effect_speed: -1,
            underglow_color: (0, 0),
            backlight_brightness: -1,
            backlight_effect: -1,
            rgb_version: -1,
            rgb_maximum_brightness: -1,
            rgb_layer_support: false,
            rgb_mode: -1,
            rgb_speed: -1,
            rgb_hsv: (0, 0, 0),
            rgb_supported_effects: HashSet::new(),
            rgb_layer_configs: Vec::new(),
            rgb_key_disable_support: false,
            rgb_number_leds: 0,
            rgb_led_matrix: Vec::new(),
            rgb_key_disable: Vec::new(),
        }
    }

    fn usb_send(&self, msg: &[u8], retries: usize) -> Result<Vec<u8>, String> {
        util::hid_send(&self.dev, msg, retries)
    }

    /// Load information about the keyboard.
    pub fn reload(&mut self, sideload_json: Option<&Value>) -> Result<(), String> {
        self.reload_layout(sideload_json)?;
        self.reload_layers()?;
        self.reload_macros_early()?;
        self.reload_persistent_rgb()?;
        self.reload_rgb()?;
        self.reload_dynamic()?;

        // Rebuild the keycode database based on keyboard capabilities.
        self.keycode_db = KeycodeDb::for_keyboard(
            self.vial_protocol,
            self.layers,
            self.macro_count,
            self.tap_dance_count,
            self.custom_keycodes.as_deref(),
            self.midi.as_deref(),
            &self.supported_features,
        );

        self.reload_keymap()?;
        self.reload_macros_late()?;
        Ok(())
    }

    fn reload_via_protocol(&mut self) -> Result<(), String> {
        let data = self.usb_send(&[CMD_VIA_GET_PROTOCOL_VERSION], 20)?;
        self.via_protocol = unpack_be_u16(&data[1..3]) as i32;
        Ok(())
    }

    fn check_protocol_version(&self) -> Result<(), ProtocolError> {
        if SUPPORTED_VIA_PROTOCOL.contains(&self.via_protocol)
            && SUPPORTED_VIAL_PROTOCOL.contains(&self.vial_protocol)
        {
            Ok(())
        } else {
            Err(ProtocolError)
        }
    }

    fn reload_layout(&mut self, sideload_json: Option<&Value>) -> Result<(), String> {
        self.reload_via_protocol()?;

        let payload: Value = if let Some(json) = sideload_json {
            json.clone()
        } else {
            // Get keyboard identification.
            let data = self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_KEYBOARD_ID], 20)?;
            self.vial_protocol = unpack_le_u32(&data[0..4]) as i32;
            self.keyboard_id = unpack_le_u64(&data[4..12]);

            // Get the definition size.
            let data = self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_SIZE], 20)?;
            let mut sz = unpack_le_u32(&data[0..4]) as usize;

            // Fetch the compressed payload in MSG_LEN-sized blocks.
            let mut payload = Vec::new();
            let mut block = 0u32;
            while sz > 0 {
                let mut msg = vec![CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_DEFINITION];
                msg.extend_from_slice(&le_u32(block));
                let mut data = self.usb_send(&msg, 20)?;
                if sz < MSG_LEN {
                    data.truncate(sz);
                }
                payload.extend_from_slice(&data);
                block += 1;
                sz = sz.saturating_sub(MSG_LEN);
            }

            let decompressed = decompress_xz(&payload)
                .map_err(|e| format!("failed to decompress definition: {}", e))?;
            serde_json::from_slice(&decompressed).map_err(|e| format!("bad definition JSON: {}", e))?
        };

        self.check_protocol_version().map_err(|_| {
            "unsupported protocol version".to_string()
        })?;

        self.definition = payload.clone();

        if let Some(vial) = payload.get("vial") {
            self.vibl = vial.get("vibl").and_then(Value::as_bool).unwrap_or(false);
            self.midi = vial.get("midi").and_then(Value::as_str).map(str::to_string);
        }

        self.layout_labels = payload.get("layouts").and_then(|l| l.get("labels")).cloned();

        self.rows = payload["matrix"]["rows"].as_u64().unwrap_or(0) as usize;
        self.cols = payload["matrix"]["cols"].as_u64().unwrap_or(0) as usize;

        self.custom_keycodes = payload.get("customKeycodes").and_then(Value::as_array).map(|arr| {
            arr.iter()
                .map(|v| CustomKeycode {
                    short_name: v.get("shortName").and_then(Value::as_str).map(str::to_string),
                    title: v.get("title").and_then(Value::as_str).map(str::to_string),
                    name: v.get("name").and_then(Value::as_str).map(str::to_string),
                })
                .collect()
        });

        let kbd = kle::Serial::deserialize(&payload["layouts"]["keymap"])?;

        self.keys.clear();
        self.encoders.clear();
        self.rowcol.clear();
        self.encoderpos.clear();
        self.encoder_count = 0;

        for mut key in kbd.keys {
            if key.labels[4].as_deref() == Some("e") {
                if let Some(lbl) = &key.labels[0] {
                    if let Some((idx_s, dir_s)) = lbl.split_once(',') {
                        if let (Ok(idx), Ok(dir)) = (idx_s.parse::<usize>(), dir_s.parse::<usize>()) {
                            key.encoder_idx = Some(idx);
                            key.encoder_dir = Some(dir);
                            if !self.encoderpos.contains(&idx) {
                                self.encoderpos.push(idx);
                            }
                            self.encoder_count = self.encoder_count.max(idx + 1);
                            self.encoders.push(key);
                            continue;
                        }
                    }
                }
            }

            if key.decal || (key.labels[0].as_deref().map(|l| l.contains(',')).unwrap_or(false)) {
                let mut row = 0usize;
                let mut col = 0usize;
                if let Some(lbl) = &key.labels[0] {
                    if let Some((r_s, c_s)) = lbl.split_once(',') {
                        if let (Ok(r), Ok(c)) = (r_s.parse::<usize>(), c_s.parse::<usize>()) {
                            row = r;
                            col = c;
                        }
                    }
                }
                key.row = Some(row);
                key.col = Some(col);
                if !self.rowcol.contains(&(row, col)) {
                    self.rowcol.push((row, col));
                }
                self.keys.push(key);
                continue;
            }

            // Keys without annotations are ignored (decorative).
            self.keys.push(key);
        }

        // Assign layout index/option from labels[8].
        for key in self.keys.iter_mut().chain(self.encoders.iter_mut()) {
            if let Some(lbl) = &key.labels[8] {
                if let Some((idx_s, opt_s)) = lbl.split_once(',') {
                    if let (Ok(idx), Ok(opt)) = (idx_s.parse::<i32>(), opt_s.parse::<i32>()) {
                        key.layout_index = idx;
                        key.layout_option = opt;
                    }
                }
            }
        }

        Ok(())
    }

    fn reload_layers(&mut self) -> Result<(), String> {
        let data = self.usb_send(&[CMD_VIA_GET_LAYER_COUNT], 20)?;
        self.layers = data[1] as usize;
        Ok(())
    }

    fn reload_macros_early(&mut self) -> Result<(), String> {
        let data = self.usb_send(&[CMD_VIA_MACRO_GET_COUNT], 20)?;
        self.macro_count = data[1] as usize;
        let data = self.usb_send(&[CMD_VIA_MACRO_GET_BUFFER_SIZE], 20)?;
        self.macro_memory = unpack_be_u16(&data[1..3]) as usize;
        Ok(())
    }

    fn reload_macros_late(&mut self) -> Result<(), String> {
        self.macro_data = Vec::new();
        if self.macro_memory == 0 {
            return Ok(());
        }
        for x in (0..self.macro_memory).step_by(BUFFER_FETCH_CHUNK) {
            let sz = BUFFER_FETCH_CHUNK.min(self.macro_memory - x);
            let mut msg = vec![CMD_VIA_MACRO_GET_BUFFER];
            msg.extend_from_slice(&be_u16(x as u16));
            msg.push(sz as u8);
            let data = self.usb_send(&msg, 20)?;
            self.macro_data.extend_from_slice(&data[4..4 + sz]);
            if self.macro_data.iter().filter(|b| **b == 0).count() > self.macro_count {
                break;
            }
        }
        // Normalize to macro_count NUL-separated entries.
        let macros: Vec<&[u8]> = self.macro_data.split(|b| *b == 0).collect();
        let mut normalized: Vec<u8> = Vec::new();
        for i in 0..self.macro_count {
            if i > 0 {
                normalized.push(0);
            }
            if let Some(m) = macros.get(i) {
                normalized.extend_from_slice(m);
            }
        }
        normalized.push(0);
        self.macro_data = normalized;
        Ok(())
    }

    fn reload_dynamic(&mut self) -> Result<(), String> {
        self.supported_features = HashSet::new();
        if self.vial_protocol < VIAL_PROTOCOL_DYNAMIC {
            self.tap_dance_count = 0;
            self.combo_count = 0;
            self.key_override_count = 0;
            self.alt_repeat_key_count = 0;
            return Ok(());
        }
        let data = self.usb_send(
            &[
                CMD_VIA_VIAL_PREFIX,
                CMD_VIAL_DYNAMIC_ENTRY_OP,
                DYNAMIC_VIAL_GET_NUMBER_OF_ENTRIES,
            ],
            20,
        )?;
        self.tap_dance_count = *data.first().unwrap_or(&0) as usize;
        self.combo_count = *data.get(1).unwrap_or(&0) as usize;
        self.key_override_count = *data.get(2).unwrap_or(&0) as usize;
        self.alt_repeat_key_count = *data.get(3).unwrap_or(&0) as usize;

        if let Some(&last) = data.last() {
            if last & (1 << 0) != 0 {
                self.supported_features.insert("caps_word".to_string());
            }
            if last & (1 << 1) != 0 {
                self.supported_features.insert("layer_lock".to_string());
            }
        }
        if self.vial_protocol >= VIAL_PROTOCOL_KEY_OVERRIDE {
            self.supported_features.insert("persistent_default_layer".to_string());
        }
        if self.alt_repeat_key_count > 0 {
            self.supported_features.insert("repeat_key".to_string());
        }
        Ok(())
    }

    fn reload_persistent_rgb(&mut self) -> Result<(), String> {
        if let Some(lighting) = self.definition.get("lighting").and_then(Value::as_str) {
            self.lighting_qmk_rgblight =
                lighting == "qmk_rgblight" || lighting == "qmk_backlight_rgblight";
            self.lighting_qmk_backlight =
                lighting == "qmk_backlight" || lighting == "qmk_backlight_rgblight";
            self.lighting_vialrgb = lighting == "vialrgb";
        }

        if self.lighting_vialrgb {
            let data = self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_INFO], 20)?;
            self.rgb_version = (data[2] as i32) | ((data[3] as i32) << 8);
            if self.rgb_version < 1 {
                return Err(format!(
                    "Unsupported VialRGB protocol ({}), update your Vial version to latest",
                    self.rgb_version
                ));
            }
            self.rgb_maximum_brightness = data[4] as i32;
            self.rgb_layer_support = self.rgb_version >= 2;
            self.rgb_key_disable_support = self.rgb_version >= 3;

            if self.rgb_key_disable_support {
                // Number of LEDs and their position in the keyboard matrix.
                let data =
                    self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_NUMBER_LEDS], 20)?;
                self.rgb_number_leds = u16::from_le_bytes([data[2], data[3]]) as usize;

                self.rgb_led_matrix.clear();
                for led in 0..self.rgb_number_leds {
                    let mut msg = vec![CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_LED_INFO];
                    msg.extend_from_slice(&[led as u8, (led >> 8) as u8]);
                    let data = self.usb_send(&msg, 20)?;
                    let row = data[5];
                    let col = data[6];
                    self.rgb_led_matrix.push(if row != 0xFF && col != 0xFF {
                        Some((row as usize, col as usize))
                    } else {
                        None
                    });
                }
            }

            self.rgb_supported_effects.clear();
            self.rgb_supported_effects.insert(0);
            let mut max_effect: u16 = 0;
            while max_effect < 0xFFFF {
                let mut msg = vec![CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_SUPPORTED];
                msg.extend_from_slice(&le_u16(max_effect));
                let data = self.usb_send(&msg, 20)?;
                for x in (2..data.len()).step_by(2) {
                    if x + 2 > data.len() {
                        break;
                    }
                    let value = u16::from_le_bytes([data[x], data[x + 1]]);
                    if value != 0xFFFF {
                        self.rgb_supported_effects.insert(value);
                    }
                    max_effect = max_effect.max(value);
                }
            }
        }
        Ok(())
    }

    fn reload_rgb(&mut self) -> Result<(), String> {
        if self.lighting_qmk_rgblight {
            self.underglow_brightness =
                self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, QMK_RGBLIGHT_BRIGHTNESS], 20)?[2] as i32;
            self.underglow_effect =
                self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, QMK_RGBLIGHT_EFFECT], 20)?[2] as i32;
            self.underglow_effect_speed =
                self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, QMK_RGBLIGHT_EFFECT_SPEED], 20)?[2]
                    as i32;
            let color = self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, QMK_RGBLIGHT_COLOR], 20)?;
            self.underglow_color = (color[2], color[3]);
        }

        if self.lighting_qmk_backlight {
            self.backlight_brightness =
                self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, QMK_BACKLIGHT_BRIGHTNESS], 20)?[2] as i32;
            self.backlight_effect =
                self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, QMK_BACKLIGHT_EFFECT], 20)?[2] as i32;
        }

        if self.lighting_vialrgb {
            if self.rgb_layer_support {
                self.rgb_layer_configs.clear();
                for layer in 0..self.layers {
                    let data = self.usb_send(
                        &[CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_LAYER, layer as u8],
                        20,
                    )?;
                    self.rgb_layer_configs.push(RgbLayerConfig {
                        mode: u16::from_le_bytes([data[2], data[3]]),
                        speed: data[4],
                        hue: data[5],
                        sat: data[6],
                        val: data[7],
                    });
                }
                let cfg = self.rgb_layer_configs.first().copied().unwrap_or_default();
                self.rgb_mode = cfg.mode as i32;
                self.rgb_speed = cfg.speed as i32;
                self.rgb_hsv = (cfg.hue, cfg.sat, cfg.val);
            } else {
                let data = self.usb_send(&[CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_MODE], 20)?;
                self.rgb_mode = u16::from_le_bytes([data[2], data[3]]) as i32;
                self.rgb_speed = data[4] as i32;
                self.rgb_hsv = (data[5], data[6], data[7]);
                self.rgb_layer_configs.clear();
            }

            if self.rgb_key_disable_support {
                let nbytes = (self.rgb_number_leds + 7) / 8;
                self.rgb_key_disable.clear();
                for layer in 0..self.layers {
                    let data = self.usb_send(
                        &[CMD_VIA_LIGHTING_GET_VALUE, VIALRGB_GET_KEY_DISABLE, layer as u8],
                        20,
                    )?;
                    self.rgb_key_disable.push(data[2..2 + nbytes].to_vec());
                }
            }
        }
        Ok(())
    }

    /// Return the VialRGB config for a layer (or the single global config).
    pub fn rgb_config(&self, layer: usize) -> RgbLayerConfig {
        if self.rgb_layer_support {
            self.rgb_layer_configs.get(layer).copied().unwrap_or_default()
        } else {
            RgbLayerConfig {
                mode: self.rgb_mode as u16,
                speed: self.rgb_speed as u8,
                hue: self.rgb_hsv.0,
                sat: self.rgb_hsv.1,
                val: self.rgb_hsv.2,
            }
        }
    }

    // --- QMK rgblight (underglow) ---

    pub fn set_qmk_rgblight_brightness(&mut self, value: u8) -> Result<(), String> {
        self.underglow_brightness = value as i32;
        self.usb_send(&[CMD_VIA_LIGHTING_SET_VALUE, QMK_RGBLIGHT_BRIGHTNESS, value], 20)?;
        Ok(())
    }

    pub fn set_qmk_rgblight_effect(&mut self, index: u8) -> Result<(), String> {
        self.underglow_effect = index as i32;
        self.usb_send(&[CMD_VIA_LIGHTING_SET_VALUE, QMK_RGBLIGHT_EFFECT, index], 20)?;
        Ok(())
    }

    pub fn set_qmk_rgblight_effect_speed(&mut self, value: u8) -> Result<(), String> {
        self.underglow_effect_speed = value as i32;
        self.usb_send(&[CMD_VIA_LIGHTING_SET_VALUE, QMK_RGBLIGHT_EFFECT_SPEED, value], 20)?;
        Ok(())
    }

    pub fn set_qmk_rgblight_color(&mut self, h: u8, s: u8, v: u8) -> Result<(), String> {
        self.set_qmk_rgblight_brightness(v)?;
        self.usb_send(&[CMD_VIA_LIGHTING_SET_VALUE, QMK_RGBLIGHT_COLOR, h, s], 20)?;
        Ok(())
    }

    // --- QMK backlight ---

    pub fn set_qmk_backlight_brightness(&mut self, value: u8) -> Result<(), String> {
        self.backlight_brightness = value as i32;
        self.usb_send(&[CMD_VIA_LIGHTING_SET_VALUE, QMK_BACKLIGHT_BRIGHTNESS, value], 20)?;
        Ok(())
    }

    pub fn set_qmk_backlight_effect(&mut self, value: u8) -> Result<(), String> {
        self.backlight_effect = value as i32;
        self.usb_send(&[CMD_VIA_LIGHTING_SET_VALUE, QMK_BACKLIGHT_EFFECT, value], 20)?;
        Ok(())
    }

    pub fn save_rgb(&self) -> Result<(), String> {
        self.usb_send(&[CMD_VIA_LIGHTING_SAVE], 20)?;
        Ok(())
    }

    // --- VialRGB (layer-aware) ---

    fn vialrgb_set_layer(&mut self, layer: usize) -> Result<(), String> {
        let cfg = self.rgb_layer_configs[layer];
        let mut msg = vec![CMD_VIA_LIGHTING_SET_VALUE, VIALRGB_SET_LAYER, layer as u8];
        msg.extend_from_slice(&le_u16(cfg.mode));
        msg.extend_from_slice(&[cfg.speed, cfg.hue, cfg.sat, cfg.val]);
        self.usb_send(&msg, 20)?;
        Ok(())
    }

    pub fn set_vialrgb_layer_brightness(&mut self, layer: usize, value: u8) -> Result<(), String> {
        if let Some(cfg) = self.rgb_layer_configs.get_mut(layer) {
            cfg.val = value;
        }
        self.vialrgb_set_layer(layer)
    }

    pub fn set_vialrgb_layer_speed(&mut self, layer: usize, value: u8) -> Result<(), String> {
        if let Some(cfg) = self.rgb_layer_configs.get_mut(layer) {
            cfg.speed = value;
        }
        self.vialrgb_set_layer(layer)
    }

    pub fn set_vialrgb_layer_mode(&mut self, layer: usize, value: u16) -> Result<(), String> {
        if let Some(cfg) = self.rgb_layer_configs.get_mut(layer) {
            cfg.mode = value;
        }
        self.vialrgb_set_layer(layer)
    }

    pub fn set_vialrgb_layer_color(
        &mut self,
        layer: usize,
        h: u8,
        s: u8,
        v: u8,
    ) -> Result<(), String> {
        if let Some(cfg) = self.rgb_layer_configs.get_mut(layer) {
            cfg.hue = h;
            cfg.sat = s;
            cfg.val = v;
        }
        self.vialrgb_set_layer(layer)
    }

    // --- VialRGB (legacy single config) ---

    fn vialrgb_set_mode(&mut self) -> Result<(), String> {
        let mut msg = vec![CMD_VIA_LIGHTING_SET_VALUE, VIALRGB_SET_MODE];
        msg.extend_from_slice(&le_u16(self.rgb_mode as u16));
        msg.extend_from_slice(&[
            self.rgb_speed as u8,
            self.rgb_hsv.0,
            self.rgb_hsv.1,
            self.rgb_hsv.2,
        ]);
        self.usb_send(&msg, 20)?;
        Ok(())
    }

    pub fn set_vialrgb_brightness(&mut self, value: u8) -> Result<(), String> {
        self.rgb_hsv.2 = value;
        self.vialrgb_set_mode()
    }

    pub fn set_vialrgb_speed(&mut self, value: u8) -> Result<(), String> {
        self.rgb_speed = value as i32;
        self.vialrgb_set_mode()
    }

    pub fn set_vialrgb_mode(&mut self, value: u16) -> Result<(), String> {
        self.rgb_mode = value as i32;
        self.vialrgb_set_mode()
    }

    pub fn set_vialrgb_color(&mut self, h: u8, s: u8, v: u8) -> Result<(), String> {
        self.rgb_hsv = (h, s, v);
        self.vialrgb_set_mode()
    }

    /// Set a full per-layer config in a single packet (for restore).
    pub fn set_vialrgb_layer_config(
        &mut self,
        layer: usize,
        cfg: RgbLayerConfig,
    ) -> Result<(), String> {
        if let Some(slot) = self.rgb_layer_configs.get_mut(layer) {
            *slot = cfg;
        }
        self.vialrgb_set_layer(layer)
    }

    /// Set the legacy single config in a single packet (for restore).
    pub fn set_vialrgb_config(
        &mut self,
        mode: u16,
        speed: u8,
        hue: u8,
        sat: u8,
        val: u8,
    ) -> Result<(), String> {
        self.rgb_mode = mode as i32;
        self.rgb_speed = speed as i32;
        self.rgb_hsv = (hue, sat, val);
        self.vialrgb_set_mode()
    }

    // --- VialRGB per-key "no light" mask ---

    /// LED indices that illuminate the given key (empty if the key has no LED).
    pub fn rgb_leds_for_key(&self, row: usize, col: usize) -> Vec<usize> {
        self.rgb_led_matrix
            .iter()
            .enumerate()
            .filter_map(|(led, m)| m.filter(|(r, c)| *r == row && *c == col).map(|_| led))
            .collect()
    }

    /// Whether the given key has its light turned off on `layer`.
    pub fn rgb_key_disabled(&self, layer: usize, row: usize, col: usize) -> bool {
        let Some(mask) = self.rgb_key_disable.get(layer) else {
            return false;
        };
        self.rgb_leds_for_key(row, col)
            .iter()
            .any(|&led| mask[led / 8] & (1 << (led % 8)) != 0)
    }

    /// Set the "no light" state for every LED of a key on the given layer.
    pub fn set_vialrgb_key_disable(
        &mut self,
        layer: usize,
        row: usize,
        col: usize,
        disabled: bool,
    ) -> Result<(), String> {
        let leds = self.rgb_leds_for_key(row, col);
        if leds.is_empty() {
            return Ok(());
        }

        let mask: Vec<u8> = {
            let Some(slot) = self.rgb_key_disable.get_mut(layer) else {
                return Err("invalid layer".to_string());
            };
            for led in &leds {
                let byte = led / 8;
                let bit = 1 << (led % 8);
                if disabled {
                    slot[byte] |= bit;
                } else {
                    slot[byte] &= !bit;
                }
            }
            slot.clone()
        };

        let mut msg = vec![CMD_VIA_LIGHTING_SET_VALUE, VIALRGB_SET_KEY_DISABLE, layer as u8];
        msg.extend_from_slice(&mask);
        self.usb_send(&msg, 20)?;
        Ok(())
    }

    /// Set a full per-layer mask in a single packet (for restore).
    pub fn set_vialrgb_key_disable_mask(&mut self, layer: usize, mask: &[u8]) -> Result<(), String> {
        let nbytes = (self.rgb_number_leds + 7) / 8;
        if mask.len() < nbytes {
            return Err("mask too short".to_string());
        }
        if let Some(slot) = self.rgb_key_disable.get_mut(layer) {
            slot.copy_from_slice(&mask[..nbytes]);
        }
        let mut msg = vec![CMD_VIA_LIGHTING_SET_VALUE, VIALRGB_SET_KEY_DISABLE, layer as u8];
        msg.extend_from_slice(&mask[..nbytes]);
        self.usb_send(&msg, 20)?;
        Ok(())
    }

    fn reload_keymap(&mut self) -> Result<(), String> {
        let mut keymap = Vec::new();
        let size = self.layers * self.rows * self.cols * 2;
        for x in (0..size).step_by(BUFFER_FETCH_CHUNK) {
            let offset = x;
            let sz = BUFFER_FETCH_CHUNK.min(size - offset);
            let mut msg = vec![CMD_VIA_KEYMAP_GET_BUFFER];
            msg.extend_from_slice(&be_u16(offset as u16));
            msg.push(sz as u8);
            let data = self.usb_send(&msg, 20)?;
            keymap.extend_from_slice(&data[4..4 + sz]);
        }

        self.layout.clear();
        for layer in 0..self.layers {
            for (row, col) in self.rowcol.iter().copied() {
                if row >= self.rows || col >= self.cols {
                    return Err(format!(
                        "malformed vial.json, key references {},{} but matrix declares rows={} cols={}",
                        row, col, self.rows, self.cols
                    ));
                }
                let offset = layer * self.rows * self.cols * 2 + row * self.cols * 2 + col * 2;
                let code = unpack_be_u16(&keymap[offset..offset + 2]);
                self.layout
                    .insert((layer, row, col), self.keycode_db.serialize(code as u32));
            }
        }

        self.encoder_layout.clear();
        for layer in 0..self.layers {
            for idx in self.encoderpos.iter().copied() {
                let data = self.usb_send(
                    &[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_ENCODER, layer as u8, idx as u8],
                    20,
                )?;
                let cw = unpack_be_u16(&data[0..2]);
                let ccw = unpack_be_u16(&data[2..4]);
                self.encoder_layout
                    .insert((layer, idx, 0), self.keycode_db.serialize(cw as u32));
                self.encoder_layout
                    .insert((layer, idx, 1), self.keycode_db.serialize(ccw as u32));
            }
        }

        if self.layout_labels.is_some() {
            let data = self.usb_send(&[CMD_VIA_GET_KEYBOARD_VALUE, VIA_LAYOUT_OPTIONS], 20)?;
            self.layout_options = unpack_be_u32(&data[2..6]) as i32;
        }
        Ok(())
    }

    pub fn set_key(&mut self, layer: usize, row: usize, col: usize, code: &str) -> Result<(), String> {
        let key = (layer, row, col);
        if self.layout.get(&key).map(String::as_str) != Some(code) {
            let value = self.keycode_db.deserialize(code);
            let mut msg = vec![CMD_VIA_SET_KEYCODE, layer as u8, row as u8, col as u8];
            msg.extend_from_slice(&be_u16(value as u16));
            self.usb_send(&msg, 20)?;
            self.layout.insert(key, code.to_string());
        }
        Ok(())
    }

    pub fn set_encoder(
        &mut self,
        layer: usize,
        index: usize,
        direction: usize,
        code: &str,
    ) -> Result<(), String> {
        let key = (layer, index, direction);
        if self.encoder_layout.get(&key).map(String::as_str) != Some(code) {
            let value = self.keycode_db.deserialize(code);
            let mut msg = vec![
                CMD_VIA_VIAL_PREFIX,
                CMD_VIAL_SET_ENCODER,
                layer as u8,
                index as u8,
                direction as u8,
            ];
            msg.extend_from_slice(&be_u16(value as u16));
            self.usb_send(&msg, 20)?;
            self.encoder_layout.insert(key, code.to_string());
        }
        Ok(())
    }

    pub fn set_layout_options(&mut self, options: u32) -> Result<(), String> {
        if self.layout_options != -1 && self.layout_options != options as i32 {
            self.layout_options = options as i32;
            let mut msg = vec![CMD_VIA_SET_KEYBOARD_VALUE, VIA_LAYOUT_OPTIONS];
            msg.extend_from_slice(&be_u32(options));
            self.usb_send(&msg, 20)?;
        }
        Ok(())
    }

    pub fn reset(&self) -> Result<(), String> {
        self.usb_send(&[0x0B], 20)?;
        Ok(())
    }

    pub fn lock(&self) -> Result<(), String> {
        if self.vial_protocol >= 0 {
            self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_LOCK], 20)?;
        }
        Ok(())
    }

    pub fn get_unlock_status(&self) -> Result<(u8, u8), String> {
        if self.vial_protocol < 0 {
            return Ok((1, 0));
        }
        let data = self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_UNLOCK_STATUS], 20)?;
        Ok((data[0], data[1]))
    }

    pub fn get_unlock_keys(&self) -> Result<Vec<(usize, usize)>, String> {
        if self.vial_protocol < 0 {
            return Ok(Vec::new());
        }
        let data = self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_UNLOCK_STATUS], 20)?;
        let mut rowcol = Vec::new();
        for x in 0..15 {
            let row = data[2 + x * 2] as usize;
            let col = data[3 + x * 2] as usize;
            if row != 255 && col != 255 {
                rowcol.push((row, col));
            }
        }
        Ok(rowcol)
    }

    pub fn matrix_poll(&self) -> Result<Vec<u8>, String> {
        if self.via_protocol < 0 {
            return Ok(Vec::new());
        }
        self.usb_send(&[CMD_VIA_GET_KEYBOARD_VALUE, VIA_SWITCH_MATRIX_STATE], 3)
    }

    /// Serialize all configuration to the `.vil2` JSON format.
    pub fn save_layout(&self) -> Result<Vec<u8>, String> {
        let mut layout: Vec<Vec<Vec<u32>>> = Vec::new();
        for l in 0..self.layers {
            let mut layer = Vec::new();
            for r in 0..self.rows {
                let mut row = Vec::new();
                for c in 0..self.cols {
                    let val = match self.layout.get(&(l, r, c)) {
                        Some(code) => self.keycode_db.deserialize(code),
                        None => 0xFFFF,
                    };
                    row.push(val);
                }
                layer.push(row);
            }
            layout.push(layer);
        }

        let mut encoder_layout: Vec<Vec<Vec<u32>>> = Vec::new();
        for l in 0..self.layers {
            let mut layer = Vec::new();
            for e in 0..self.encoder_count {
                let cw = self
                    .encoder_layout
                    .get(&(l, e, 0))
                    .map(|c| self.keycode_db.deserialize(c))
                    .unwrap_or(0xFFFF);
                let ccw = self
                    .encoder_layout
                    .get(&(l, e, 1))
                    .map(|c| self.keycode_db.deserialize(c))
                    .unwrap_or(0xFFFF);
                layer.push(vec![cw, ccw]);
            }
            encoder_layout.push(layer);
        }

        let data = serde_json::json!({
            "version": 2,
            "uid": self.keyboard_id,
            "layout": layout,
            "encoder_layout": encoder_layout,
            "layout_options": self.layout_options,
            "macro": [],
            "vial_protocol": self.vial_protocol,
            "via_protocol": self.via_protocol,
            "tap_dance": [],
            "combo": [],
            "key_override": [],
            "alt_repeat_key": [],
            "settings": {},
            "rgb": self.save_rgb_json(),
        });
        serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())
    }

    fn save_rgb_json(&self) -> Value {
        let mut rgb = serde_json::Map::new();
        if self.lighting_vialrgb {
            let mut vialrgb = serde_json::Map::new();
            if self.rgb_layer_support {
                let configs: Vec<Value> = self
                    .rgb_layer_configs
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "mode": c.mode,
                            "speed": c.speed,
                            "hue": c.hue,
                            "sat": c.sat,
                            "val": c.val,
                        })
                    })
                    .collect();
                vialrgb.insert("layer_configs".to_string(), Value::Array(configs));
            } else {
                vialrgb.insert("mode".to_string(), serde_json::json!(self.rgb_mode));
                vialrgb.insert("speed".to_string(), serde_json::json!(self.rgb_speed));
                vialrgb.insert("hue".to_string(), serde_json::json!(self.rgb_hsv.0));
                vialrgb.insert("sat".to_string(), serde_json::json!(self.rgb_hsv.1));
                vialrgb.insert("val".to_string(), serde_json::json!(self.rgb_hsv.2));
            }
            if self.rgb_key_disable_support {
                let masks: Vec<Value> = self
                    .rgb_key_disable
                    .iter()
                    .map(|m| Value::Array(m.iter().map(|&b| serde_json::json!(b)).collect()))
                    .collect();
                vialrgb.insert("key_disable".to_string(), Value::Array(masks));
            }
            rgb.insert("vialrgb".to_string(), Value::Object(vialrgb));
        }
        if self.lighting_qmk_rgblight {
            rgb.insert(
                "underglow".to_string(),
                serde_json::json!({
                    "brightness": self.underglow_brightness,
                    "effect": self.underglow_effect,
                    "speed": self.underglow_effect_speed,
                    "hue": self.underglow_color.0,
                    "sat": self.underglow_color.1,
                }),
            );
        }
        if self.lighting_qmk_backlight {
            rgb.insert(
                "backlight".to_string(),
                serde_json::json!({
                    "brightness": self.backlight_brightness,
                    "effect": self.backlight_effect,
                }),
            );
        }
        Value::Object(rgb)
    }

    /// Restore a `.vil2` layout, applying all recognized configuration.
    pub fn restore_layout(&mut self, data: &[u8]) -> Result<(), String> {
        let data: Value = serde_json::from_slice(data).map_err(|e| e.to_string())?;

        if let Some(layout) = data.get("layout").and_then(Value::as_array) {
            for (l, layer) in layout.iter().enumerate() {
                if let Some(rows) = layer.as_array() {
                    for (r, row) in rows.iter().enumerate() {
                        if let Some(cols) = row.as_array() {
                            for (c, code) in cols.iter().enumerate() {
                                if let Some(code) = code.as_u64() {
                                    if self.layout.contains_key(&(l, r, c)) {
                                        let s = self.keycode_db.serialize(code as u32);
                                        self.set_key(l, r, c, &s)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(encoders) = data.get("encoder_layout").and_then(Value::as_array) {
            for (l, layer) in encoders.iter().enumerate() {
                if let Some(encs) = layer.as_array() {
                    for (e, enc) in encs.iter().enumerate() {
                        if let Some(pair) = enc.as_array() {
                            if let Some(cw) = pair.first().and_then(Value::as_u64) {
                                let s = self.keycode_db.serialize(cw as u32);
                                self.set_encoder(l, e, 0, &s)?;
                            }
                            if let Some(ccw) = pair.get(1).and_then(Value::as_u64) {
                                let s = self.keycode_db.serialize(ccw as u32);
                                self.set_encoder(l, e, 1, &s)?;
                            }
                        }
                    }
                }
            }
        }

        if let Some(options) = data.get("layout_options").and_then(Value::as_u64) {
            self.set_layout_options(options as u32)?;
        }

        if let Some(rgb) = data.get("rgb") {
            self.restore_rgb(rgb)?;
        }

        Ok(())
    }

    fn restore_rgb(&mut self, rgb: &Value) -> Result<(), String> {
        if let Some(vialrgb) = rgb.get("vialrgb") {
            if self.lighting_vialrgb {
                if self.rgb_layer_support {
                    if let Some(configs) = vialrgb.get("layer_configs").and_then(Value::as_array) {
                        for (layer, cfg) in configs.iter().enumerate() {
                            if layer >= self.rgb_layer_configs.len() {
                                break;
                            }
                            let mode = cfg.get("mode").and_then(Value::as_u64).unwrap_or(0) as u16;
                            let speed = cfg.get("speed").and_then(Value::as_u64).unwrap_or(0) as u8;
                            let hue = cfg.get("hue").and_then(Value::as_u64).unwrap_or(0) as u8;
                            let sat = cfg.get("sat").and_then(Value::as_u64).unwrap_or(0) as u8;
                            let val = cfg.get("val").and_then(Value::as_u64).unwrap_or(0) as u8;
                            self.set_vialrgb_layer_config(layer, RgbLayerConfig {
                                mode,
                                speed,
                                hue,
                                sat,
                                val,
                            })?;
                        }
                    }
                } else {
                    let mode = vialrgb.get("mode").and_then(Value::as_u64).unwrap_or(0) as u16;
                    let speed = vialrgb.get("speed").and_then(Value::as_u64).unwrap_or(0) as u8;
                    let hue = vialrgb.get("hue").and_then(Value::as_u64).unwrap_or(0) as u8;
                    let sat = vialrgb.get("sat").and_then(Value::as_u64).unwrap_or(0) as u8;
                    let val = vialrgb.get("val").and_then(Value::as_u64).unwrap_or(0) as u8;
                    self.set_vialrgb_config(mode, speed, hue, sat, val)?;
                }

                if self.rgb_key_disable_support {
                    if let Some(masks) = vialrgb.get("key_disable").and_then(Value::as_array) {
                        for (layer, mask) in masks.iter().enumerate() {
                            if layer >= self.rgb_key_disable.len() {
                                break;
                            }
                            let bytes: Vec<u8> = mask
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                                        .collect()
                                })
                                .unwrap_or_default();
                            self.set_vialrgb_key_disable_mask(layer, &bytes)?;
                        }
                    }
                }
            }
        }

        if let Some(underglow) = rgb.get("underglow") {
            if self.lighting_qmk_rgblight {
                if let Some(v) = underglow.get("brightness").and_then(Value::as_u64) {
                    self.set_qmk_rgblight_brightness(v as u8)?;
                }
                if let Some(v) = underglow.get("effect").and_then(Value::as_u64) {
                    self.set_qmk_rgblight_effect(v as u8)?;
                }
                if let Some(v) = underglow.get("speed").and_then(Value::as_u64) {
                    self.set_qmk_rgblight_effect_speed(v as u8)?;
                }
                if let (Some(h), Some(s)) = (
                    underglow.get("hue").and_then(Value::as_u64),
                    underglow.get("sat").and_then(Value::as_u64),
                ) {
                    let v = self.underglow_brightness.clamp(0, 255) as u8;
                    self.set_qmk_rgblight_color(h as u8, s as u8, v)?;
                }
            }
        }

        if let Some(backlight) = rgb.get("backlight") {
            if self.lighting_qmk_backlight {
                if let Some(v) = backlight.get("brightness").and_then(Value::as_u64) {
                    self.set_qmk_backlight_brightness(v as u8)?;
                }
                if let Some(v) = backlight.get("effect").and_then(Value::as_u64) {
                    self.set_qmk_backlight_effect(v as u8)?;
                }
            }
        }

        // Persist RGB configs to EEPROM after applying.
        if self.lighting_vialrgb || self.lighting_qmk_rgblight || self.lighting_qmk_backlight {
            self.save_rgb()?;
        }
        Ok(())
    }
}

/// Parse the `uid` field from a `.vil2` file (without applying anything).
pub fn parse_layout_uid(data: &[u8]) -> Option<u64> {
    let v: Value = serde_json::from_slice(data).ok()?;
    v.get("uid").and_then(Value::as_u64)
}

/// Decompress an XZ stream (Vial definitions are compressed with Python's
/// `lzma.compress`, which uses the XZ container format).
fn decompress_xz(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut decoder = xz2::read::XzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
