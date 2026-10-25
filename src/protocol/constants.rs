// SPDX-License-Identifier: GPL-2.0-or-later
// Port of vial-gui's protocol/constants.py

// VIA protocol commands
pub const CMD_VIA_GET_PROTOCOL_VERSION: u8 = 0x01;
pub const CMD_VIA_GET_KEYBOARD_VALUE: u8 = 0x02;
pub const CMD_VIA_SET_KEYBOARD_VALUE: u8 = 0x03;
pub const CMD_VIA_GET_KEYCODE: u8 = 0x04;
pub const CMD_VIA_SET_KEYCODE: u8 = 0x05;
pub const CMD_VIA_LIGHTING_SET_VALUE: u8 = 0x07;
pub const CMD_VIA_LIGHTING_GET_VALUE: u8 = 0x08;
pub const CMD_VIA_LIGHTING_SAVE: u8 = 0x09;
pub const CMD_VIA_MACRO_GET_COUNT: u8 = 0x0C;
pub const CMD_VIA_MACRO_GET_BUFFER_SIZE: u8 = 0x0D;
pub const CMD_VIA_MACRO_GET_BUFFER: u8 = 0x0E;
pub const CMD_VIA_MACRO_SET_BUFFER: u8 = 0x0F;
pub const CMD_VIA_GET_LAYER_COUNT: u8 = 0x11;
pub const CMD_VIA_KEYMAP_GET_BUFFER: u8 = 0x12;
pub const CMD_VIA_VIAL_PREFIX: u8 = 0xFE;

// VIA "get/set keyboard value" ids
pub const VIA_LAYOUT_OPTIONS: u8 = 0x02;
pub const VIA_SWITCH_MATRIX_STATE: u8 = 0x03;

// QMK lighting ids
pub const QMK_BACKLIGHT_BRIGHTNESS: u8 = 0x09;
pub const QMK_BACKLIGHT_EFFECT: u8 = 0x0A;
pub const QMK_RGBLIGHT_BRIGHTNESS: u8 = 0x80;
pub const QMK_RGBLIGHT_EFFECT: u8 = 0x81;
pub const QMK_RGBLIGHT_EFFECT_SPEED: u8 = 0x82;
pub const QMK_RGBLIGHT_COLOR: u8 = 0x83;

// VialRGB ids
pub const VIALRGB_GET_INFO: u8 = 0x40;
pub const VIALRGB_GET_MODE: u8 = 0x41;
pub const VIALRGB_GET_SUPPORTED: u8 = 0x42;
pub const VIALRGB_GET_LAYER: u8 = 0x45;
pub const VIALRGB_SET_MODE: u8 = 0x41;
pub const VIALRGB_SET_LAYER: u8 = 0x43;

// Vial-specific commands (sent with CMD_VIA_VIAL_PREFIX)
pub const CMD_VIAL_GET_KEYBOARD_ID: u8 = 0x00;
pub const CMD_VIAL_GET_SIZE: u8 = 0x01;
pub const CMD_VIAL_GET_DEFINITION: u8 = 0x02;
pub const CMD_VIAL_GET_ENCODER: u8 = 0x03;
pub const CMD_VIAL_SET_ENCODER: u8 = 0x04;
pub const CMD_VIAL_GET_UNLOCK_STATUS: u8 = 0x05;
pub const CMD_VIAL_UNLOCK_START: u8 = 0x06;
pub const CMD_VIAL_UNLOCK_POLL: u8 = 0x07;
pub const CMD_VIAL_LOCK: u8 = 0x08;
pub const CMD_VIAL_QMK_SETTINGS_QUERY: u8 = 0x09;
pub const CMD_VIAL_QMK_SETTINGS_GET: u8 = 0x0A;
pub const CMD_VIAL_QMK_SETTINGS_SET: u8 = 0x0B;
pub const CMD_VIAL_QMK_SETTINGS_RESET: u8 = 0x0C;
pub const CMD_VIAL_DYNAMIC_ENTRY_OP: u8 = 0x0D;

// Dynamic entry subcommands
pub const DYNAMIC_VIAL_GET_NUMBER_OF_ENTRIES: u8 = 0x00;
pub const DYNAMIC_VIAL_TAP_DANCE_GET: u8 = 0x01;
pub const DYNAMIC_VIAL_TAP_DANCE_SET: u8 = 0x02;
pub const DYNAMIC_VIAL_COMBO_GET: u8 = 0x03;
pub const DYNAMIC_VIAL_COMBO_SET: u8 = 0x04;
pub const DYNAMIC_VIAL_KEY_OVERRIDE_GET: u8 = 0x05;
pub const DYNAMIC_VIAL_KEY_OVERRIDE_SET: u8 = 0x06;
pub const DYNAMIC_VIAL_ALT_REPEAT_KEY_GET: u8 = 0x07;
pub const DYNAMIC_VIAL_ALT_REPEAT_KEY_SET: u8 = 0x08;

// How much of a macro/keymap buffer we can read/write per packet
pub const BUFFER_FETCH_CHUNK: usize = 28;

// Protocol version feature gates
pub const VIAL_PROTOCOL_ADVANCED_MACROS: i32 = 2;
pub const VIAL_PROTOCOL_MATRIX_TESTER: i32 = 3;
pub const VIAL_PROTOCOL_DYNAMIC: i32 = 4;
pub const VIAL_PROTOCOL_QMK_SETTINGS: i32 = 4;
pub const VIAL_PROTOCOL_EXT_MACROS: i32 = 5;
pub const VIAL_PROTOCOL_KEY_OVERRIDE: i32 = 5;

// Supported protocol versions
pub const SUPPORTED_VIA_PROTOCOL: [i32; 2] = [-1, 9];
pub const SUPPORTED_VIAL_PROTOCOL: [i32; 8] = [-1, 0, 1, 2, 3, 4, 5, 6];

// HID report length
pub const MSG_LEN: usize = 32;

// Serial number magic strings used to identify Vial devices
pub const VIAL_SERIAL_NUMBER_MAGIC: &str = "vial:f64c2b3c";
pub const VIBL_SERIAL_NUMBER_MAGIC: &str = "vibl:d4f8159c";

// Example keyboard UIDs that should warn the user
pub const EXAMPLE_KEYBOARDS: [u64; 4] = [
    0xD4A36200603E3007,
    0x32F62BC2EEF2237B,
    0x38CEA320F23046A5,
    0xBED2D31EC59A0BD8,
];
pub const EXAMPLE_KEYBOARD_PREFIX: u64 = 0xA6867BDFD3B00F;
