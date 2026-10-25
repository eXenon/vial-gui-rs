// SPDX-License-Identifier: GPL-2.0-or-later
// RGB effect tables (from editor/rgb_configurator.py).

/// QMK rgblight effects: (index, name, has_color_picker).
pub const QMK_RGBLIGHT_EFFECTS: &[(u8, &str, bool)] = &[
    (0, "All Off", false),
    (1, "Solid Color", true),
    (2, "Breathing 1", true),
    (3, "Breathing 2", true),
    (4, "Breathing 3", true),
    (5, "Breathing 4", true),
    (6, "Rainbow Mood 1", false),
    (7, "Rainbow Mood 2", false),
    (8, "Rainbow Mood 3", false),
    (9, "Rainbow Swirl 1", false),
    (10, "Rainbow Swirl 2", false),
    (11, "Rainbow Swirl 3", false),
    (12, "Rainbow Swirl 4", false),
    (13, "Rainbow Swirl 5", false),
    (14, "Rainbow Swirl 6", false),
    (15, "Snake 1", true),
    (16, "Snake 2", true),
    (17, "Snake 3", true),
    (18, "Snake 4", true),
    (19, "Snake 5", true),
    (20, "Snake 6", true),
    (21, "Knight 1", true),
    (22, "Knight 2", true),
    (23, "Knight 3", true),
    (24, "Christmas", true),
    (25, "Gradient 1", true),
    (26, "Gradient 2", true),
    (27, "Gradient 3", true),
    (28, "Gradient 4", true),
    (29, "Gradient 5", true),
    (30, "Gradient 6", true),
    (31, "Gradient 7", true),
    (32, "Gradient 8", true),
    (33, "Gradient 9", true),
    (34, "Gradient 10", true),
    (35, "RGB Test", true),
    (36, "Alternating", true),
];

/// VialRGB effects: (index, name).
pub const VIALRGB_EFFECTS: &[(u16, &str)] = &[
    (0, "Disable"),
    (1, "Direct Control"),
    (2, "Solid Color"),
    (3, "Alphas Mods"),
    (4, "Gradient Up Down"),
    (5, "Gradient Left Right"),
    (6, "Breathing"),
    (7, "Band Sat"),
    (8, "Band Val"),
    (9, "Band Pinwheel Sat"),
    (10, "Band Pinwheel Val"),
    (11, "Band Spiral Sat"),
    (12, "Band Spiral Val"),
    (13, "Cycle All"),
    (14, "Cycle Left Right"),
    (15, "Cycle Up Down"),
    (16, "Rainbow Moving Chevron"),
    (17, "Cycle Out In"),
    (18, "Cycle Out In Dual"),
    (19, "Cycle Pinwheel"),
    (20, "Cycle Spiral"),
    (21, "Dual Beacon"),
    (22, "Rainbow Beacon"),
    (23, "Rainbow Pinwheels"),
    (24, "Raindrops"),
    (25, "Jellybean Raindrops"),
    (26, "Hue Breathing"),
    (27, "Hue Pendulum"),
    (28, "Hue Wave"),
    (29, "Typing Heatmap"),
    (30, "Digital Rain"),
    (31, "Solid Reactive Simple"),
    (32, "Solid Reactive"),
    (33, "Solid Reactive Wide"),
    (34, "Solid Reactive Multiwide"),
    (35, "Solid Reactive Cross"),
    (36, "Solid Reactive Multicross"),
    (37, "Solid Reactive Nexus"),
    (38, "Solid Reactive Multinexus"),
    (39, "Splash"),
    (40, "Multisplash"),
    (41, "Solid Splash"),
    (42, "Solid Multisplash"),
    (43, "Pixel Rain"),
    (44, "Pixel Fractal"),
];

pub fn qmk_rgblight_name(idx: u8) -> String {
    QMK_RGBLIGHT_EFFECTS
        .iter()
        .find(|(i, _, _)| *i == idx)
        .map(|(_, n, _)| n.to_string())
        .unwrap_or_else(|| format!("{}", idx))
}

pub fn vialrgb_name(id: u16) -> String {
    VIALRGB_EFFECTS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("{}", id))
}
