// SPDX-License-Identifier: GPL-2.0-or-later
// Port of vial-gui's kle_serial.py (based on https://github.com/ijprest/kle-serial)

use serde_json::Value;

#[derive(Clone, Debug, Default)]
pub struct Key {
    pub color: String,
    pub labels: [Option<String>; 12],
    pub text_color: [Option<String>; 12],
    pub text_size: [Option<f32>; 12],
    pub default_text_size: f32,
    pub default_text_color: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub x2: f32,
    pub y2: f32,
    pub width2: f32,
    pub height2: f32,
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_angle: f32,
    pub decal: bool,
    pub ghost: bool,
    pub stepped: bool,
    pub nub: bool,
    pub profile: String,
    pub sm: String,
    pub sb: String,
    pub st: String,
    // Vial-specific annotations computed during layout loading
    pub row: Option<usize>,
    pub col: Option<usize>,
    pub encoder_idx: Option<usize>,
    pub encoder_dir: Option<usize>,
    pub layout_index: i32,
    pub layout_option: i32,
}

impl Key {
    pub fn new() -> Self {
        Key {
            color: "#cccccc".to_string(),
            default_text_size: 3.0,
            default_text_color: "#000000".to_string(),
            width: 1.0,
            height: 1.0,
            width2: 1.0,
            height2: 1.0,
            layout_index: -1,
            layout_option: -1,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Keyboard {
    pub name: String,
    pub backcolor: String,
    pub keys: Vec<Key>,
}

#[derive(Clone, Debug)]
struct Cluster {
    x: f32,
    y: f32,
}

impl Default for Cluster {
    fn default() -> Self {
        Cluster { x: 0.0, y: 0.0 }
    }
}

// Alignment flag reordering table (labelMap in kle_serial.py)
const LABEL_MAP: [[i16; 12]; 8] = [
    [0, 6, 2, 8, 9, 11, 3, 5, 1, 4, 7, 10],
    [1, 7, -1, -1, 9, 11, 4, -1, -1, -1, -1, 10],
    [3, -1, 5, -1, 9, 11, -1, -1, 4, -1, -1, 10],
    [4, -1, -1, -1, 9, 11, -1, -1, -1, -1, -1, 10],
    [0, 6, 2, 8, 10, -1, 3, 5, 1, 4, 7, -1],
    [1, 7, -1, -1, 10, -1, 4, -1, -1, -1, -1, -1],
    [3, -1, 5, -1, 10, -1, -1, -1, 4, -1, -1, -1],
    [4, -1, -1, -1, 10, -1, -1, -1, -1, -1, -1, -1],
];

pub struct Serial;

fn reorder_labels(labels: &[String], align: usize) -> [Option<String>; 12] {
    let mut ret: [Option<String>; 12] = Default::default();
    for (i, label) in labels.iter().enumerate() {
        if !label.is_empty() && i < 12 {
            let map_idx = LABEL_MAP[align % LABEL_MAP.len()][i];
            if map_idx >= 0 {
                ret[map_idx as usize] = Some(label.clone());
            }
        }
    }
    ret
}

fn reorder_sizes<T: Clone + Default>(sizes: &[T], align: usize) -> [Option<T>; 12] {
    let mut ret: [Option<T>; 12] = Default::default();
    for (i, size) in sizes.iter().enumerate() {
        if i < 12 {
            let map_idx = LABEL_MAP[align % LABEL_MAP.len()][i];
            if map_idx >= 0 {
                ret[map_idx as usize] = Some(size.clone());
            }
        }
    }
    ret
}

impl Serial {
    /// Deserialize a KLE layout (the `keymap` array from a Vial/VIA definition).
    pub fn deserialize(rows: &Value) -> Result<Keyboard, String> {
        let rows = rows
            .as_array()
            .ok_or_else(|| "KLE keymap must be an array".to_string())?;

        let mut current = Key::new();
        let mut cluster = Cluster::default();
        let mut kbd = Keyboard::default();
        let mut align: usize = 4;

        for (_r, row) in rows.iter().enumerate() {
            match row {
                Value::Array(items) => {
                    for (k, item) in items.iter().enumerate() {
                        match item {
                            Value::String(s) => {
                                let mut new_key = current.clone();
                                new_key.width2 =
                                    if new_key.width2 == 0.0 { current.width } else { current.width2 };
                                new_key.height2 = if new_key.height2 == 0.0 {
                                    current.height
                                } else {
                                    current.height2
                                };
                                new_key.labels = reorder_labels(&s.split('\n').map(str::to_string).collect::<Vec<_>>(), align);
                                new_key.text_size = reorder_sizes(&current.text_size.iter().map(|x| x.unwrap_or(0.0)).collect::<Vec<_>>(), align);

                                // Clean up defaults
                                for i in 0..12 {
                                    if new_key.labels[i].is_none() {
                                        new_key.text_size[i] = None;
                                        new_key.text_color[i] = None;
                                    }
                                    if new_key.text_size[i] == Some(new_key.default_text_size) {
                                        new_key.text_size[i] = None;
                                    }
                                    if new_key.text_color[i].as_deref()
                                        == Some(new_key.default_text_color.as_str())
                                    {
                                        new_key.text_color[i] = None;
                                    }
                                }

                                kbd.keys.push(new_key);

                                current.x += current.width;
                                current.width = 1.0;
                                current.height = 1.0;
                                current.x2 = 0.0;
                                current.y2 = 0.0;
                                current.width2 = 0.0;
                                current.height2 = 0.0;
                                current.nub = false;
                                current.stepped = false;
                                current.decal = false;
                            }
                            Value::Object(prop) => {
                                if k != 0
                                    && (prop.contains_key("r")
                                        || prop.contains_key("rx")
                                        || prop.contains_key("ry"))
                                {
                                    return Err(format!(
                                        "rotation can only be specified on the first key in a row: {:?}",
                                        item
                                    ));
                                }
                                if let Some(v) = prop.get("r").and_then(Value::as_f64) {
                                    current.rotation_angle = v as f32;
                                }
                                if let Some(v) = prop.get("rx").and_then(Value::as_f64) {
                                    current.rotation_x = v as f32;
                                    cluster.x = current.rotation_x;
                                    current.x = cluster.x;
                                    current.y = cluster.y;
                                }
                                if let Some(v) = prop.get("ry").and_then(Value::as_f64) {
                                    current.rotation_y = v as f32;
                                    cluster.y = current.rotation_y;
                                    current.x = cluster.x;
                                    current.y = cluster.y;
                                }
                                if let Some(v) = prop.get("a").and_then(Value::as_u64) {
                                    align = v as usize;
                                }
                                if let Some(v) = prop.get("f").and_then(Value::as_f64) {
                                    current.default_text_size = v as f32;
                                    current.text_size = Default::default();
                                }
                                if let Some(v) = prop.get("f2").and_then(Value::as_f64) {
                                    for i in 1..12 {
                                        current.text_size[i] = Some(v as f32);
                                    }
                                }
                                if let Some(v) = prop.get("fa").and_then(Value::as_array) {
                                    let mut sizes: [Option<f32>; 12] = Default::default();
                                    for (i, x) in v.iter().take(12).enumerate() {
                                        sizes[i] = x.as_f64().map(|f| f as f32);
                                    }
                                    current.text_size = sizes;
                                }
                                if let Some(v) = prop.get("p").and_then(Value::as_str) {
                                    current.profile = v.to_string();
                                }
                                if let Some(v) = prop.get("c").and_then(Value::as_str) {
                                    current.color = v.to_string();
                                }
                                if let Some(v) = prop.get("t").and_then(Value::as_str) {
                                    let split: Vec<String> =
                                        v.split('\n').map(str::to_string).collect();
                                    if !split.is_empty() && !split[0].is_empty() {
                                        current.default_text_color = split[0].clone();
                                    }
                                    current.text_color = reorder_sizes(&split, align);
                                }
                                if let Some(v) = prop.get("x").and_then(Value::as_f64) {
                                    current.x += v as f32;
                                }
                                if let Some(v) = prop.get("y").and_then(Value::as_f64) {
                                    current.y += v as f32;
                                }
                                if let Some(v) = prop.get("w").and_then(Value::as_f64) {
                                    current.width = v as f32;
                                    current.width2 = v as f32;
                                }
                                if let Some(v) = prop.get("h").and_then(Value::as_f64) {
                                    current.height = v as f32;
                                    current.height2 = v as f32;
                                }
                                if let Some(v) = prop.get("x2").and_then(Value::as_f64) {
                                    current.x2 = v as f32;
                                }
                                if let Some(v) = prop.get("y2").and_then(Value::as_f64) {
                                    current.y2 = v as f32;
                                }
                                if let Some(v) = prop.get("w2").and_then(Value::as_f64) {
                                    current.width2 = v as f32;
                                }
                                if let Some(v) = prop.get("h2").and_then(Value::as_f64) {
                                    current.height2 = v as f32;
                                }
                                if let Some(v) = prop.get("n").and_then(Value::as_bool) {
                                    current.nub = v;
                                }
                                if let Some(v) = prop.get("l").and_then(Value::as_bool) {
                                    current.stepped = v;
                                }
                                if let Some(v) = prop.get("d").and_then(Value::as_bool) {
                                    current.decal = v;
                                }
                                if let Some(v) = prop.get("g").and_then(Value::as_bool) {
                                    current.ghost = v;
                                }
                                if let Some(v) = prop.get("sm").and_then(Value::as_str) {
                                    current.sm = v.to_string();
                                }
                                if let Some(v) = prop.get("sb").and_then(Value::as_str) {
                                    current.sb = v.to_string();
                                }
                                if let Some(v) = prop.get("st").and_then(Value::as_str) {
                                    current.st = v.to_string();
                                }
                            }
                            _ => {}
                        }
                    }
                    current.y += 1.0;
                    current.x = current.rotation_x;
                }
                Value::Object(_) => {
                    // keyboard metadata must be the first element; ignore for now
                }
                _ => {}
            }
        }

        Ok(kbd)
    }
}
