// SPDX-License-Identifier: GPL-2.0-or-later
// The keycode tray (port of tabbed_keycodes.py, simplified to a tabbed button grid).

use egui::{ScrollArea, Ui};

use crate::keycodes::KeycodeDb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayTab {
    Basic,
    Iso,
    Layers,
    Quantum,
    Backlight,
    Media,
    Midi,
    TapDance,
    User,
    Macro,
}

impl TrayTab {
    const ALL: [TrayTab; 10] = [
        TrayTab::Basic,
        TrayTab::Iso,
        TrayTab::Layers,
        TrayTab::Quantum,
        TrayTab::Backlight,
        TrayTab::Media,
        TrayTab::Midi,
        TrayTab::TapDance,
        TrayTab::User,
        TrayTab::Macro,
    ];

    fn label(self) -> &'static str {
        match self {
            TrayTab::Basic => "Basic",
            TrayTab::Iso => "ISO/JIS",
            TrayTab::Layers => "Layers",
            TrayTab::Quantum => "Quantum",
            TrayTab::Backlight => "Backlight",
            TrayTab::Media => "App, Media and Mouse",
            TrayTab::Midi => "MIDI",
            TrayTab::TapDance => "Tap Dance",
            TrayTab::User => "User",
            TrayTab::Macro => "Macro",
        }
    }

    fn qmk_ids(self, db: &KeycodeDb) -> Vec<String> {
        let mut out = Vec::new();
        let mut push = |kcs: &[crate::keycodes::Keycode]| {
            for kc in kcs {
                if !kc.hidden {
                    out.push(kc.qmk_id.clone());
                }
            }
        };
        match self {
            TrayTab::Basic => {
                push(&db.special);
                push(&db.basic);
                push(&db.shifted);
            }
            TrayTab::Iso => {
                push(&db.special);
                push(&db.shifted);
                push(&db.iso);
            }
            TrayTab::Layers => push(&db.layers),
            TrayTab::Quantum => {
                push(&db.boot);
                push(&db.modifiers);
                push(&db.quantum);
            }
            TrayTab::Backlight => push(&db.backlight),
            TrayTab::Media => push(&db.media),
            TrayTab::Midi => push(&db.midi),
            TrayTab::TapDance => push(&db.tap_dance),
            TrayTab::User => push(&db.user),
            TrayTab::Macro => push(&db.macro_kcs),
        }
        out
    }
}

pub enum TrayAction {
    Keycode(String),
    Any,
}

/// Draw the keycode tray. Returns an action if a keycode was chosen.
/// When `masked_filter` is set, only basic (byte-sized) keycodes are shown —
/// used when editing the inner key of a composite keycode like `LCTL(kc)`.
pub fn show(
    ui: &mut Ui,
    db: &KeycodeDb,
    tab: &mut TrayTab,
    masked_filter: bool,
) -> Option<TrayAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        for t in TrayTab::ALL {
            if ui.selectable_label(*tab == t, t.label()).clicked() {
                *tab = t;
            }
        }
    });
    ui.separator();

    // "Any" button (opens the free-form keycode entry dialog).
    if ui.button("Any").clicked() {
        action = Some(TrayAction::Any);
        return action;
    }
    ui.separator();

    let qmk_ids: Vec<String> = tab
        .qmk_ids(db)
        .into_iter()
        .filter(|id| !masked_filter || db.is_basic(id))
        .collect();

    let label_for = |qmk_id: &str| -> String {
        let label = db.label(qmk_id);
        label.replace('\n', " ")
    };

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for qmk_id in &qmk_ids {
                let label = label_for(qmk_id);
                let tooltip = db.tooltip(qmk_id);
                let resp = ui.add(egui::Button::new(label));
                let mut resp = resp;
                if let Some(t) = tooltip {
                    resp = resp.on_hover_text(t);
                }
                if resp.clicked() {
                    action = Some(TrayAction::Keycode(qmk_id.clone()));
                }
            }
        });
    });

    action
}
