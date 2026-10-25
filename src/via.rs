// SPDX-License-Identifier: GPL-2.0-or-later
// VIA definition stack (via_keyboard_stack.json) handling.

use std::collections::HashMap;

use serde_json::Value;

#[derive(Default, Clone)]
pub struct ViaStack {
    pub definitions: HashMap<String, Value>,
}

impl ViaStack {
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        let v: Value = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        let mut definitions = HashMap::new();
        if let Some(obj) = v.get("definitions").and_then(Value::as_object) {
            for (k, val) in obj {
                definitions.insert(k.clone(), val.clone());
            }
        }
        Ok(ViaStack { definitions })
    }

    pub fn contains(&self, vid: u16, pid: u16) -> bool {
        self.definitions.contains_key(&(vid as u32 * 65536 + pid as u32).to_string())
    }

    pub fn definition(&self, vid: u16, pid: u16) -> Option<Value> {
        self.definitions
            .get(&(vid as u32 * 65536 + pid as u32).to_string())
            .cloned()
    }
}

/// Download the VIA definition stack from the precompiled repository.
pub fn download_via_stack() -> Result<Vec<u8>, String> {
    const URL: &str =
        "https://github.com/vial-kb/via-keymap-precompiled/raw/main/via_keyboard_stack.json";
    let resp = ureq::get(URL)
        .call()
        .map_err(|e| format!("download failed: {}", e))?;
    let body = resp
        .into_body()
        .read_to_vec()
        .map_err(|e| format!("download read failed: {}", e))?;
    Ok(body)
}
