// SPDX-License-Identifier: GPL-2.0-or-later
// Port of vial-gui's keycodes/keycodes.py — keycode model, resolution, and the
// per-keyboard keycode database.

pub mod definitions;
pub mod v5;
pub mod v6;

use std::collections::{HashMap, HashSet};

use super::any_keycode::AnyKeycode;

/// A custom (user) keycode as declared in a Vial/VIA definition.
#[derive(Clone, Debug, Default)]
pub struct CustomKeycode {
    pub short_name: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
}

/// A single keycode definition.
#[derive(Clone, Debug)]
pub struct Keycode {
    pub qmk_id: String,
    pub label: String,
    pub tooltip: Option<String>,
    pub masked: bool,
    pub printable: Option<String>,
    pub alias: Vec<String>,
    pub recorder_alias: Vec<String>,
    pub requires_feature: Option<String>,
    pub hidden: bool,
}

impl Keycode {
    pub fn new(qmk_id: &str, label: &str) -> Self {
        Keycode {
            qmk_id: qmk_id.to_string(),
            label: label.to_string(),
            tooltip: None,
            masked: false,
            printable: None,
            alias: vec![qmk_id.to_string()],
            recorder_alias: Vec::new(),
            requires_feature: None,
            hidden: false,
        }
    }

    pub fn tooltip(mut self, t: &str) -> Self {
        self.tooltip = Some(t.to_string());
        self
    }

    pub fn masked(mut self) -> Self {
        self.masked = true;
        self
    }

    pub fn printable(mut self, p: &str) -> Self {
        self.printable = Some(p.to_string());
        self
    }

    pub fn alias(mut self, aliases: &[&str]) -> Self {
        self.alias
            .extend(aliases.iter().map(|s| s.to_string()));
        self
    }

    pub fn recorder_alias(mut self, aliases: &[&str]) -> Self {
        self.recorder_alias
            .extend(aliases.iter().map(|s| s.to_string()));
        self
    }

    pub fn requires_feature(mut self, f: &str) -> Self {
        self.requires_feature = Some(f.to_string());
        self
    }
}

/// The per-keyboard keycode database. Rebuilt whenever a keyboard is loaded.
pub struct KeycodeDb {
    pub protocol: i32,

    // Category lists (display order, hidden flag applied).
    pub special: Vec<Keycode>,
    pub shifted: Vec<Keycode>,
    pub basic: Vec<Keycode>,
    pub basic_numpad: Vec<Keycode>,
    pub basic_nav: Vec<Keycode>,
    pub iso: Vec<Keycode>,
    pub iso_kr: Vec<Keycode>,
    pub boot: Vec<Keycode>,
    pub modifiers: Vec<Keycode>,
    pub quantum: Vec<Keycode>,
    pub backlight: Vec<Keycode>,
    pub media: Vec<Keycode>,
    pub macro_base: Vec<Keycode>,
    pub layers: Vec<Keycode>,
    pub tap_dance: Vec<Keycode>,
    pub macro_kcs: Vec<Keycode>,
    pub user: Vec<Keycode>,
    pub midi: Vec<Keycode>,

    // Combined list in Python KEYCODES order.
    pub keycodes: Vec<Keycode>,

    // Lookup maps.
    map: HashMap<String, Keycode>,
    qmk_id_to_keycode: HashMap<String, Keycode>,
    rawcodes: HashMap<u32, Keycode>,
    masked_keycodes: HashSet<String>,
    any_names: HashMap<String, u32>,

    // Raw QMK code table for the active protocol.
    kc: &'static HashMap<&'static str, u32>,
    masked: &'static HashSet<u32>,

    pub supported_features: HashSet<String>,
}

impl KeycodeDb {
    /// Build a keycode database for the given keyboard parameters.
    pub fn for_keyboard(
        protocol: i32,
        layers: usize,
        macro_count: usize,
        tap_dance_count: usize,
        custom_keycodes: Option<&[CustomKeycode]>,
        midi: Option<&str>,
        supported_features: &HashSet<String>,
    ) -> KeycodeDb {
        let kc = if protocol == 6 { v6::kc() } else { v5::kc() };
        let masked = if protocol == 6 { v6::masked() } else { v5::masked() };

        let resolve = |name: &str| -> Option<u32> { kc.get(name).copied() };

        // Generated layer keycodes
        let mut layer_kcs: Vec<Keycode> = Vec::new();
        layer_kcs.push(
            Keycode::new("QK_LAYER_LOCK", "Layer\nLock")
                .tooltip("Locks the current layer")
                .alias(&["QK_LLCK"])
                .requires_feature("layer_lock"),
        );
        if layers >= 4 {
            layer_kcs.push(Keycode::new("FN_MO13", "Fn1\n(Fn3)"));
            layer_kcs.push(Keycode::new("FN_MO23", "Fn2\n(Fn3)"));
        }
        let gen_mask = |label: &str, desc: &str, feat: Option<&str>| -> Vec<Keycode> {
            let mut v = Vec::new();
            for l in 0..layers {
                let qmk_id = format!("{}({})", label, l);
                let mut kc = Keycode::new(&qmk_id, &qmk_id).tooltip(desc);
                if let Some(f) = feat {
                    kc = kc.requires_feature(f);
                }
                v.push(kc);
            }
            v
        };
        layer_kcs.extend(gen_mask(
            "MO",
            "Momentarily turn on layer when pressed (requires KC_TRNS on destination layer)",
            None,
        ));
        layer_kcs.extend(gen_mask("DF", "Set the base (default) layer", None));
        layer_kcs.extend(gen_mask(
            "PDF",
            "Persistently set the base (default) layer",
            Some("persistent_default_layer"),
        ));
        layer_kcs.extend(gen_mask("TG", "Toggle layer on or off", None));
        layer_kcs.extend(gen_mask(
            "TT",
            "Normally acts like MO unless it's tapped multiple times, which toggles layer on",
            None,
        ));
        layer_kcs.extend(gen_mask(
            "OSL",
            "Momentarily activates layer until a key is pressed",
            None,
        ));
        layer_kcs.extend(gen_mask(
            "TO",
            "Turns on layer and turns off all other layers, except the default layer",
            None,
        ));
        for x in 0..layers.min(16) {
            layer_kcs.push(
                Keycode::new(&format!("LT{}(kc)", x), &format!("LT {}\n(kc)", x))
                    .tooltip(&format!("kc on tap, switch to layer {} while held", x))
                    .masked(),
            );
        }

        // Macro keycodes
        let mut macro_kcs: Vec<Keycode> = Vec::new();
        for x in 0..macro_count {
            macro_kcs.push(Keycode::new(&format!("M{}", x), &format!("M{}", x)));
        }
        macro_kcs.extend(definitions::macro_base());

        // Tap dance keycodes
        let mut tap_dance: Vec<Keycode> = Vec::new();
        for x in 0..tap_dance_count {
            tap_dance.push(
                Keycode::new(&format!("TD({})", x), &format!("TD({})", x))
                    .tooltip("Tap dance keycode"),
            );
        }

        // User keycodes
        let mut user: Vec<Keycode> = Vec::new();
        match custom_keycodes {
            Some(custom) if !custom.is_empty() => {
                for (x, c) in custom.iter().enumerate() {
                    let short = c
                        .short_name
                        .clone()
                        .unwrap_or_else(|| format!("USER{:02}", x));
                    let title = c
                        .title
                        .clone()
                        .unwrap_or_else(|| format!("USER{:02}", x));
                    let name = c
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("USER{:02}", x));
                    user.push(
                        Keycode::new(&format!("USER{:02}", x), &short)
                            .tooltip(&title)
                            .alias(&[&name]),
                    );
                }
            }
            _ => {
                for x in 0..16 {
                    user.push(Keycode::new(
                        &format!("USER{:02}", x),
                        &format!("User {}", x),
                    )
                    .tooltip(&format!("User keycode {}", x)));
                }
            }
        }

        // MIDI keycodes
        let mut midi_kcs: Vec<Keycode> = Vec::new();
        match midi {
            Some("basic") | Some("advanced") => midi_kcs.extend(definitions::midi_basic()),
            _ => {}
        }
        if midi == Some("advanced") {
            midi_kcs.extend(definitions::midi_advanced());
        }

        // Hidden TD() placeholders (always present so TD(n) resolves).
        let mut hidden: Vec<Keycode> = Vec::new();
        for x in 0..256usize {
            hidden.push(Keycode::new(&format!("TD({})", x), &format!("TD({})", x)));
        }

        // Build the combined list in Python KEYCODES order.
        let mut keycodes: Vec<Keycode> = Vec::new();
        keycodes.extend(definitions::special());
        keycodes.extend(definitions::basic());
        keycodes.extend(definitions::shifted());
        keycodes.extend(definitions::iso());
        keycodes.extend(layer_kcs.iter().cloned());
        keycodes.extend(definitions::boot());
        keycodes.extend(definitions::modifiers());
        keycodes.extend(definitions::quantum());
        keycodes.extend(definitions::backlight());
        keycodes.extend(definitions::media());
        keycodes.extend(tap_dance.iter().cloned());
        keycodes.extend(macro_kcs.iter().cloned());
        keycodes.extend(user.iter().cloned());
        keycodes.extend(hidden);
        keycodes.extend(midi_kcs.iter().cloned());

        // Apply hidden flags.
        let supported = supported_features;
        let mark_hidden = |kcs: &mut Vec<Keycode>| {
            for kc in kcs.iter_mut() {
                if let Some(feat) = &kc.requires_feature {
                    if !supported.contains(feat) {
                        kc.hidden = true;
                    }
                }
            }
        };
        for kc in keycodes.iter_mut() {
            if let Some(feat) = &kc.requires_feature {
                if !supported.contains(feat) {
                    kc.hidden = true;
                }
            }
        }
        mark_hidden(&mut layer_kcs);
        mark_hidden(&mut macro_kcs);
        mark_hidden(&mut tap_dance);
        mark_hidden(&mut user);
        mark_hidden(&mut midi_kcs);

        // Build lookup maps. Iterate in order so later duplicates win (mirrors Python).
        let mut map: HashMap<String, Keycode> = HashMap::new();
        let mut qmk_id_to_keycode: HashMap<String, Keycode> = HashMap::new();
        let mut rawcodes: HashMap<u32, Keycode> = HashMap::new();
        let mut masked_keycodes: HashSet<String> = HashSet::new();
        let mut recorder_alias: HashMap<String, Keycode> = HashMap::new();

        for kc in keycodes.iter() {
            map.insert(kc.qmk_id.replace("(kc)", ""), kc.clone());
            qmk_id_to_keycode.insert(kc.qmk_id.clone(), kc.clone());
            if let Some(code) = resolve(&kc.qmk_id) {
                rawcodes.insert(code, kc.clone());
            }
            if kc.masked {
                masked_keycodes.insert(kc.qmk_id.replace("(kc)", ""));
            }
            for alias in &kc.recorder_alias {
                recorder_alias.insert(alias.clone(), kc.clone());
            }
        }
        let _ = recorder_alias;

        // Build the AnyKeycode name table.
        let mut any_names: HashMap<String, u32> = HashMap::new();
        let mut add_aliases = |kcs: &[Keycode]| {
            for kc in kcs {
                if let Some(code) = resolve(&kc.qmk_id) {
                    for alias in &kc.alias {
                        any_names.insert(alias.clone(), code);
                    }
                }
            }
        };
        add_aliases(&definitions::special());
        add_aliases(&definitions::basic());
        add_aliases(&definitions::shifted());
        add_aliases(&definitions::iso());
        add_aliases(&definitions::backlight());
        add_aliases(&definitions::media());
        add_aliases(&user);
        for m in [
            "MOD_LCTL", "MOD_LSFT", "MOD_LALT", "MOD_LGUI", "MOD_RCTL", "MOD_RSFT", "MOD_RALT",
            "MOD_RGUI", "MOD_MEH", "MOD_HYPR",
        ] {
            if let Some(code) = resolve(m) {
                any_names.insert(m.to_string(), code);
            }
        }

        KeycodeDb {
            protocol,
            special: definitions::special(),
            shifted: definitions::shifted(),
            basic: definitions::basic(),
            basic_numpad: definitions::basic_numpad(),
            basic_nav: definitions::basic_nav(),
            iso: definitions::iso(),
            iso_kr: definitions::iso_kr(),
            boot: definitions::boot(),
            modifiers: definitions::modifiers(),
            quantum: definitions::quantum(),
            backlight: definitions::backlight(),
            media: definitions::media(),
            macro_base: definitions::macro_base(),
            layers: layer_kcs,
            tap_dance,
            macro_kcs,
            user,
            midi: midi_kcs,
            keycodes,
            map,
            qmk_id_to_keycode,
            rawcodes,
            masked_keycodes,
            any_names,
            kc,
            masked,
            supported_features: supported.clone(),
        }
    }

    /// Default database used before any keyboard is connected.
    pub fn default() -> KeycodeDb {
        KeycodeDb::for_keyboard(6, 0, 0, 0, None, None, &HashSet::new())
    }

    /// Resolve a QMK constant (qmk_id) to its raw integer value for the active protocol.
    pub fn resolve(&self, qmk_constant: &str) -> Option<u32> {
        self.kc.get(qmk_constant).copied()
    }

    pub fn find(&self, qmk_id: &str) -> Option<&Keycode> {
        self.map.get(qmk_id)
    }

    pub fn is_mask(&self, qmk_id: &str) -> bool {
        if let Some(idx) = qmk_id.find('(') {
            self.masked_keycodes.contains(&qmk_id[..idx])
        } else {
            false
        }
    }

    pub fn find_outer_keycode(&self, qmk_id: &str) -> Option<&Keycode> {
        let id = if self.is_mask(qmk_id) {
            &qmk_id[..qmk_id.find('(').unwrap()]
        } else {
            qmk_id
        };
        self.find(id)
    }

    pub fn find_inner_keycode(&self, qmk_id: &str) -> Option<&Keycode> {
        if self.is_mask(qmk_id) {
            let start = qmk_id.find('(').unwrap() + 1;
            let end = qmk_id.rfind(')').unwrap_or(qmk_id.len());
            self.find(&qmk_id[start..end])
        } else {
            self.find(qmk_id)
        }
    }

    pub fn is_basic(&self, qmk_id: &str) -> bool {
        self.deserialize(qmk_id) < 0x00FF
    }

    pub fn label(&self, qmk_id: &str) -> String {
        match self.find_outer_keycode(qmk_id) {
            Some(kc) => kc.label.clone(),
            None => qmk_id.to_string(),
        }
    }

    pub fn tooltip(&self, qmk_id: &str) -> Option<String> {
        let kc = self.find_outer_keycode(qmk_id)?;
        match &kc.tooltip {
            Some(t) => Some(format!("{}: {}", kc.qmk_id, t)),
            None => Some(kc.qmk_id.clone()),
        }
    }

    /// Convert an integer keycode to its string representation.
    pub fn serialize(&self, code: u32) -> String {
        if !self.masked.contains(&(code & 0xFF00)) {
            if let Some(kc) = self.rawcodes.get(&code) {
                return kc.qmk_id.clone();
            }
        } else {
            let outer = self.rawcodes.get(&(code & 0xFF00));
            let inner = self.rawcodes.get(&(code & 0x00FF));
            if let (Some(o), Some(i)) = (outer, inner) {
                return o.qmk_id.replace("kc", &i.qmk_id);
            }
        }
        format!("0x{:X}", code)
    }

    /// Convert a string keycode to its integer value.
    pub fn deserialize(&self, val: &str) -> u32 {
        if let Some(kc) = self.qmk_id_to_keycode.get(val) {
            return self.resolve(&kc.qmk_id).unwrap_or(0);
        }
        AnyKeycode::decode(self, val).unwrap_or(0)
    }

    pub fn normalize(&self, code: &str) -> String {
        self.serialize(self.deserialize(code))
    }

    /// Look up a bare name (alias) for use by the AnyKeycode evaluator.
    pub fn any_name(&self, name: &str) -> Option<u32> {
        self.any_names.get(name).copied()
    }
}
