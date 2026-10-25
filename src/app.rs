// SPDX-License-Identifier: GPL-2.0-or-later
// Main egui application window.

use std::sync::{Arc, Mutex};

use eframe::egui;
use hidapi::HidApi;

use crate::constants::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::device::{find_vial_devices, Device, DeviceKind};
use crate::keycodes::KeycodeDb;
use crate::protocol::keyboard::Keyboard;
use crate::ui::keyboard::{self, DrawKey, KeyTarget};
use crate::ui::keycode_tray::{self, TrayAction, TrayTab};
use crate::via::{download_via_stack, ViaStack};

enum LoadState {
    Idle,
    Loading,
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Keymap,
    Lighting,
}

pub struct App {
    via_stack: ViaStack,
    devices: Vec<Device>,
    selected: Option<usize>,
    keyboard: Option<Keyboard>,
    load_state: LoadState,
    load_slot: Option<Arc<Mutex<Option<Result<Keyboard, String>>>>>,

    current_layer: usize,
    active: Option<(KeyTarget, bool)>,
    scale: f32,
    tray_tab: TrayTab,
    show_any_dialog: bool,
    any_input: String,
    status: Option<String>,

    active_tab: ActiveTab,
    rgb_layer: usize,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = App {
            via_stack: ViaStack::default(),
            devices: Vec::new(),
            selected: None,
            keyboard: None,
            load_state: LoadState::Idle,
            load_slot: None,
            current_layer: 0,
            active: None,
            scale: 1.0,
            tray_tab: TrayTab::Basic,
            show_any_dialog: false,
            any_input: String::new(),
            status: None,
            active_tab: ActiveTab::Keymap,
            rgb_layer: 0,
        };
        app.refresh_devices();
        app
    }

    fn refresh_devices(&mut self) {
        match HidApi::new() {
            Ok(api) => {
                self.devices = find_vial_devices(&api, &self.via_stack, 0, 0);
            }
            Err(e) => {
                self.status = Some(format!("HID init failed: {}", e));
            }
        }
        // Auto-select the first keyboard if nothing is currently selected.
        if self.selected.is_none() {
            if let Some(idx) = self
                .devices
                .iter()
                .position(|d| d.kind == DeviceKind::Keyboard)
            {
                self.select_device(idx);
            }
        }
    }

    fn select_device(&mut self, index: usize) {
        let device = self.devices[index].clone();
        self.selected = Some(index);
        self.load_state = LoadState::Loading;
        self.keyboard = None;
        self.active = None;
        self.current_layer = 0;
        self.rgb_layer = 0;

        let slot = Arc::new(Mutex::new(None));
        self.load_slot = Some(slot.clone());
        std::thread::spawn(move || {
            let result = match device.kind {
                DeviceKind::Keyboard => match device.open() {
                    Ok(Some(kb)) => Ok(kb),
                    Ok(None) => Err("device did not expose a keyboard".to_string()),
                    Err(e) => Err(e),
                },
                DeviceKind::Bootloader => Err("bootloader devices cannot be configured (flashing not yet implemented)".to_string()),
                DeviceKind::Dummy => Err("dummy device not supported".to_string()),
            };
            *slot.lock().unwrap() = Some(result);
        });
    }

    fn poll_load(&mut self, ctx: &egui::Context) {
        let Some(slot) = self.load_slot.clone() else {
            return;
        };
        let result = slot.lock().unwrap().take();
        match result {
            Some(result) => {
                self.load_slot = None;
                match result {
                    Ok(kb) => {
                        self.keyboard = Some(kb);
                        self.load_state = LoadState::Idle;
                    }
                    Err(e) => {
                        self.load_state = LoadState::Error(e);
                    }
                }
                ctx.request_repaint();
            }
            None => {
                // Not ready yet; keep polling shortly.
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Export layout...").clicked() {
                        self.export_layout();
                        ui.close_menu();
                    }
                    if ui.button("Import layout...").clicked() {
                        self.import_layout();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Download VIA definitions").clicked() {
                        self.download_via_stack();
                        ui.close_menu();
                    }
                    if ui.button("Refresh devices").clicked() {
                        self.refresh_devices();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Security", |ui| {
                    if ui.button("Unlock").clicked() {
                        self.status = Some("Unlock not yet implemented".to_string());
                        ui.close_menu();
                    }
                    if ui.button("Lock").clicked() {
                        if let Some(kb) = &self.keyboard {
                            let _ = kb.lock();
                        }
                        ui.close_menu();
                    }
                    if ui.button("Reboot to bootloader").clicked() {
                        if let Some(kb) = &self.keyboard {
                            let _ = kb.reset();
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button("About", |ui| {
                    if ui.button("About Vial (Rust port)").clicked() {
                        ui.close_menu();
                    }
                });
            });

            ui.separator();

            // Device selector.
            let selected_text = self
                .selected
                .and_then(|i| self.devices.get(i))
                .map(|d| d.title())
                .unwrap_or_else(|| "Select a device".to_string());
            egui::ComboBox::from_id_salt("device")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    let mut chosen = None;
                    for (i, d) in self.devices.iter().enumerate() {
                        if ui.selectable_label(self.selected == Some(i), d.title()).clicked() {
                            chosen = Some(i);
                        }
                    }
                    if let Some(i) = chosen {
                        self.select_device(i);
                    }
                });

            if ui.button("Refresh").clicked() {
                self.refresh_devices();
            }

            if let LoadState::Loading = self.load_state {
                ui.spinner();
                ui.label("Loading…");
            }
        });

        if let LoadState::Error(e) = &self.load_state {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
        }
    }

    fn download_via_stack(&mut self) {
        self.status = Some("Downloading VIA definitions…".to_string());
        match download_via_stack() {
            Ok(data) => match ViaStack::parse(&data) {
                Ok(stack) => {
                    let n = stack.definitions.len();
                    self.via_stack = stack;
                    self.status = Some(format!("Loaded {} VIA definitions", n));
                    self.refresh_devices();
                }
                Err(e) => self.status = Some(format!("Failed to parse VIA definitions: {}", e)),
            },
            Err(e) => self.status = Some(format!("Download failed: {}", e)),
        }
    }

    fn export_layout(&mut self) {
        let data = match &self.keyboard {
            Some(kb) => kb.save_layout(),
            None => {
                self.status = Some("No keyboard loaded".to_string());
                return;
            }
        };
        let data = match data {
            Ok(d) => d,
            Err(e) => {
                self.status = Some(format!("Failed to save layout: {}", e));
                return;
            }
        };
        let path = rfd::FileDialog::new()
            .add_filter("Vial layout", &["vil2"])
            .set_file_name("layout.vil2")
            .save_file();
        if let Some(path) = path {
            match std::fs::write(&path, &data) {
                Ok(_) => self.status = Some(format!("Saved layout to {}", path.display())),
                Err(e) => self.status = Some(format!("Failed to write {}: {}", path.display(), e)),
            }
        }
    }

    fn import_layout(&mut self) {
        let path = match rfd::FileDialog::new()
            .add_filter("Vial layout", &["vil2"])
            .pick_file()
        {
            Some(p) => p,
            None => return,
        };
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                self.status = Some(format!("Failed to read {}: {}", path.display(), e));
                return;
            }
        };

        // Warn if the layout belongs to a different keyboard.
        let saved_uid = crate::protocol::keyboard::parse_layout_uid(&data).unwrap_or(0);
        let current_uid = self.keyboard.as_ref().map(|kb| kb.keyboard_id).unwrap_or(0);
        if saved_uid != 0 && current_uid != 0 && saved_uid != current_uid {
            let confirm = rfd::MessageDialog::new()
                .set_title("Import layout")
                .set_description(
                    "The saved layout belongs to a different keyboard. Continue anyway?",
                )
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if confirm != rfd::MessageDialogResult::Yes {
                return;
            }
        }

        match &mut self.keyboard {
            Some(kb) => match kb.restore_layout(&data) {
                Ok(_) => self.status = Some("Imported layout".to_string()),
                Err(e) => self.status = Some(format!("Failed to import layout: {}", e)),
            },
            None => self.status = Some("No keyboard loaded".to_string()),
        }
    }

    fn key_targets(&self) -> Vec<KeyTarget> {
        let kb = match &self.keyboard {
            Some(kb) => kb,
            None => return Vec::new(),
        };
        let mut matrix: Vec<&crate::kle::Key> = kb.keys.iter().collect();
        matrix.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        let mut targets = Vec::new();
        for k in matrix {
            if let (Some(r), Some(c)) = (k.row, k.col) {
                targets.push(KeyTarget::Matrix(r, c));
            }
        }
        for e in &kb.encoders {
            if let (Some(idx), Some(dir)) = (e.encoder_idx, e.encoder_dir) {
                targets.push(KeyTarget::Encoder(idx, dir));
            }
        }
        targets
    }

    fn select_next(&mut self) {
        let targets = self.key_targets();
        if targets.is_empty() {
            return;
        }
        let cur = self.active.map(|(t, _)| t);
        let next = match cur {
            None => Some(targets[0]),
            Some(t) => {
                let pos = targets.iter().position(|x| *x == t).unwrap_or(0);
                Some(targets[(pos + 1) % targets.len()])
            }
        };
        self.active = next.map(|t| (t, false));
    }

    fn code_for(&self, target: KeyTarget) -> String {
        let kb = match &self.keyboard {
            Some(kb) => kb,
            None => return "KC_NO".to_string(),
        };
        match target {
            KeyTarget::Matrix(r, c) => kb
                .layout
                .get(&(self.current_layer, r, c))
                .cloned()
                .unwrap_or_else(|| "KC_NO".to_string()),
            KeyTarget::Encoder(idx, dir) => kb
                .encoder_layout
                .get(&(self.current_layer, idx, dir))
                .cloned()
                .unwrap_or_else(|| "KC_NO".to_string()),
        }
    }

    fn set_key(&mut self, code: &str) {
        let (target, mask) = match self.active {
            Some((t, m)) => (t, m),
            None => return,
        };
        let layer = self.current_layer;

        // If editing the inner part of a composite keycode (e.g. `LCTL(kc)`),
        // compose the outer keycode with the newly chosen inner keycode.
        let code_to_set: String;
        if mask {
            let Some(kb) = &self.keyboard else { return };
            let db = &kb.keycode_db;
            if !db.is_basic(code) {
                self.status = Some("Inner keycode must be a basic key".to_string());
                return;
            }
            let current = match target {
                KeyTarget::Matrix(r, c) => kb
                    .layout
                    .get(&(layer, r, c))
                    .cloned()
                    .unwrap_or_default(),
                KeyTarget::Encoder(idx, dir) => kb
                    .encoder_layout
                    .get(&(layer, idx, dir))
                    .cloned()
                    .unwrap_or_default(),
            };
            let Some(outer) = db.find_outer_keycode(&current) else {
                return;
            };
            code_to_set = outer.qmk_id.replace("(kc)", &format!("({})", code));
        } else {
            code_to_set = code.to_string();
        }

        let result = match target {
            KeyTarget::Matrix(r, c) => {
                if let Some(kb) = &mut self.keyboard {
                    kb.set_key(layer, r, c, &code_to_set)
                } else {
                    Ok(())
                }
            }
            KeyTarget::Encoder(idx, dir) => {
                if let Some(kb) = &mut self.keyboard {
                    kb.set_encoder(layer, idx, dir, &code_to_set)
                } else {
                    Ok(())
                }
            }
        };
        if let Err(e) = result {
            self.status = Some(format!("Failed to set key: {}", e));
        } else {
            self.select_next();
        }
    }

    fn keymap_panel(&mut self, ui: &mut egui::Ui) {
        // Snapshot mutable state into locals so we can borrow `self.keyboard`
        // immutably while the UI closures mutate only the locals.
        let mut layer = self.current_layer;
        let mut scale = self.scale;
        let mut tray_tab = self.tray_tab;

        let (hit, clicked, tray_code, any_requested) = match &self.keyboard {
            Some(kb) => {
                let mut tray_code: Option<String> = None;
                let mut any_requested = false;

                // Layer + scale controls.
                ui.horizontal(|ui| {
                    ui.label("Layer:");
                    for l in 0..kb.layers {
                        if ui.selectable_label(layer == l, format!("{}", l)).clicked() {
                            layer = l;
                        }
                    }
                    ui.separator();
                    if ui.button("−").clicked() {
                        scale = (scale - 0.1).max(0.4);
                    }
                    if ui.button("+").clicked() {
                        scale = (scale + 0.1).min(3.0);
                    }
                });
                ui.separator();

                // Build draw keys.
                let db = &kb.keycode_db;
                let mut draw_keys: Vec<DrawKey> = Vec::new();
                for k in &kb.keys {
                    let (r, c) = match (k.row, k.col) {
                        (Some(r), Some(c)) => (r, c),
                        _ => continue,
                    };
                    let code = kb
                        .layout
                        .get(&(layer, r, c))
                        .cloned()
                        .unwrap_or_else(|| "KC_NO".to_string());
                    let (text, mask_text, masked) = key_label(db, &code);
                    draw_keys.push(DrawKey {
                        desc: k,
                        target: KeyTarget::Matrix(r, c),
                        text,
                        mask_text,
                        masked,
                        tooltip: db.tooltip(&code).unwrap_or_default(),
                    });
                }
                for e in &kb.encoders {
                    let (idx, dir) = match (e.encoder_idx, e.encoder_dir) {
                        (Some(idx), Some(dir)) => (idx, dir),
                        _ => continue,
                    };
                    let code = kb
                        .encoder_layout
                        .get(&(layer, idx, dir))
                        .cloned()
                        .unwrap_or_else(|| "KC_NO".to_string());
                    let (text, _mask_text, _masked) = key_label(db, &code);
                    draw_keys.push(DrawKey {
                        desc: e,
                        target: KeyTarget::Encoder(idx, dir),
                        text,
                        mask_text: String::new(),
                        masked: false,
                        tooltip: db.tooltip(&code).unwrap_or_default(),
                    });
                }

                let unit = 14.0 * scale;
                let (hit, clicked) = keyboard::show(ui, &draw_keys, unit, self.active.as_ref());

                // Tray (filter to basic keycodes when editing a masked inner key).
                let masked_filter = self.active.map(|(_, m)| m).unwrap_or(false);
                if let Some(action) = keycode_tray::show(ui, db, &mut tray_tab, masked_filter) {
                    match action {
                        TrayAction::Keycode(code) => tray_code = Some(db.normalize(&code)),
                        TrayAction::Any => any_requested = true,
                    }
                }

                // Save RGB (same button as the Lighting tab).
                ui.separator();
                crate::ui::rgb::save_button(ui, kb);

                (hit, clicked, tray_code, any_requested)
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        "No device selected. Connect a Vial-compatible device and press \"Refresh\",\nor use File → Download VIA definitions to enable VIA keyboard support.",
                    );
                });
                (None, false, None, false)
            }
        };

        // Apply mutations.
        self.current_layer = layer;
        self.scale = scale;
        self.tray_tab = tray_tab;
        if let Some(h) = hit {
            self.active = Some((h.target, h.mask));
        } else if clicked {
            // Clicked empty space: deselect.
            self.active = None;
        }
        if let Some(code) = tray_code {
            self.set_key(&code);
        }
        if any_requested {
            let current = self.any_dialog_default();
            self.show_any_dialog = true;
            self.any_input = current;
        }
    }

    /// The initial text for the "Any keycode" dialog. When editing the inner
    /// part of a composite keycode, show the inner keycode instead of the full one.
    fn any_dialog_default(&self) -> String {
        let (target, mask) = match self.active {
            Some((t, m)) => (t, m),
            None => return String::new(),
        };
        let code = self.code_for(target);
        if mask {
            if let Some(kb) = &self.keyboard {
                if let Some(inner) = kb.keycode_db.find_inner_keycode(&code) {
                    return inner.qmk_id.clone();
                }
            }
        }
        code
    }

    fn rgb_panel(&mut self, ui: &mut egui::Ui) {
        match &mut self.keyboard {
            Some(kb) => {
                crate::ui::rgb::show(ui, kb, &mut self.rgb_layer);
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("No device selected.");
                });
            }
        }
    }

    fn any_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_any_dialog {
            return;
        }
        let mut close = false;
        let mut confirm = false;
        egui::Window::new("Any keycode")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Enter a QMK keycode expression (e.g. KC_A, LCTL(KC_A), 0x1234):");
                ui.text_edit_singleline(&mut self.any_input);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        confirm = true;
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.show_any_dialog = false;
        }
        if confirm {
            let input = self.any_input.clone();
            self.set_key(&input);
        }
    }
}

/// Compute the display label pieces for a keycode string.
fn key_label(db: &KeycodeDb, code: &str) -> (String, String, bool) {
    let masked = db.is_mask(code);
    let mut text = db.label(code);
    let mask_text = if masked {
        db.find_inner_keycode(code)
            .map(|k| k.label.clone())
            .unwrap_or_default()
    } else {
        String::new()
    };
    if masked {
        text = text.split('\n').next().unwrap_or("").to_string();
    }
    (text, mask_text, masked)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_load(ctx);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            self.top_bar(ui);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if let Some(status) = &self.status {
                ui.label(status.as_str());
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Tab bar.
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Keymap, "Keymap");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Lighting, "Lighting");
            });
            ui.separator();

            match self.active_tab {
                ActiveTab::Keymap => {
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        self.keymap_panel(ui);
                    });
                }
                ActiveTab::Lighting => {
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        self.rgb_panel(ui);
                    });
                }
            }
        });

        self.any_dialog(ctx);
    }
}

pub fn run() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Vial (Rust)"),
        ..Default::default()
    };
    eframe::run_native(
        "vial-gui-rs",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
