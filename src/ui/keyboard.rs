// SPDX-License-Identifier: GPL-2.0-or-later
// egui keyboard rendering widget (port of widgets/keyboard_widget.py).

use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::constants::*;
use crate::kle::Key;

/// Which physical key a widget represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyTarget {
    Matrix(usize, usize),
    Encoder(usize, usize), // (index, direction)
}

/// A key ready to draw (label already resolved for the current layer).
pub struct DrawKey<'a> {
    pub desc: &'a Key,
    pub target: KeyTarget,
    pub text: String,
    pub mask_text: String,
    pub masked: bool,
    pub tooltip: String,
}

/// The result of a click on the keyboard.
#[derive(Clone, Copy, Debug)]
pub struct KeyboardHit {
    pub target: KeyTarget,
    pub mask: bool,
}

struct Geom {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    has2: bool,
    x2: f32,
    y2: f32,
    w2: f32,
    h2: f32,
    rot_x: f32,
    rot_y: f32,
    angle: f32,
    // mask rect (for masked keys)
    mask_rect: Option<(f32, f32, f32, f32)>,
}

fn key_geom(desc: &Key, unit: f32) -> Geom {
    let size = unit * (KEY_SIZE_RATIO + KEY_SPACING_RATIO);
    let spacing = unit * KEY_SPACING_RATIO;
    let x = size * desc.x;
    let y = size * desc.y;
    let w = size * desc.width - spacing;
    let h = size * desc.height - spacing;
    let x2 = x + size * desc.x2;
    let y2 = y + size * desc.y2;
    let w2 = size * desc.width2 - spacing;
    let h2 = size * desc.height2 - spacing;
    let rot_x = size * desc.rotation_x;
    let rot_y = size * desc.rotation_y;
    let has2 = desc.width2 != desc.width || desc.height2 != desc.height || desc.x2 != 0.0 || desc.y2 != 0.0;

    let mask_rect = if desc.encoder_idx.is_none() {
        // same calculation as keyboard_widget.py (only meaningful for masked keys)
        Some((
            x + size * SHADOW_SIDE_PADDING,
            y + h * (1.0 - KEYBOARD_WIDGET_MASK_HEIGHT),
            w - 2.0 * size * SHADOW_SIDE_PADDING,
            h * KEYBOARD_WIDGET_MASK_HEIGHT - size * SHADOW_BOTTOM_PADDING,
        ))
    } else {
        None
    };

    Geom {
        x,
        y,
        w,
        h,
        has2,
        x2,
        y2,
        w2,
        h2,
        rot_x,
        rot_y,
        angle: desc.rotation_angle,
        mask_rect,
    }
}

fn rotate(p: Pos2, cx: f32, cy: f32, angle_deg: f32) -> Pos2 {
    if angle_deg == 0.0 {
        return p;
    }
    let a = angle_deg.to_radians();
    let (s, c) = a.sin_cos();
    let dx = p.x - cx;
    let dy = p.y - cy;
    pos2(cx + dx * c - dy * s, cy + dx * s + dy * c)
}

fn rect_poly(rect: Rect, cx: f32, cy: f32, angle: f32, shift: Vec2) -> Vec<Pos2> {
    let corners = [
        rect.min,
        pos2(rect.max.x, rect.min.y),
        rect.max,
        pos2(rect.min.x, rect.max.y),
    ];
    corners
        .iter()
        .map(|c| rotate(*c, cx, cy, angle) + shift)
        .collect()
}

fn point_in_poly(p: Pos2, poly: &[Pos2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = poly[i];
        let pj = poly[j];
        if (pi.y > p.y) != (pj.y > p.y)
            && p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Draw the keyboard and return a hit if a key was clicked this frame.
pub fn show(
    ui: &mut egui::Ui,
    keys: &[DrawKey],
    unit: f32,
    active: Option<&(KeyTarget, bool)>,
) -> Option<KeyboardHit> {
    // Layout pass: compute all geometries and overall bounds.
    struct Placed<'a> {
        dk: &'a DrawKey<'a>,
        geom: Geom,
        primary: Vec<Pos2>,
        secondary: Option<Vec<Pos2>>,
        mask: Option<Vec<Pos2>>,
        decal: bool,
    }

    let mut placed: Vec<Placed> = Vec::new();
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for dk in keys {
        let geom = key_geom(dk.desc, unit);
        let primary = rect_poly(
            Rect::from_min_size(pos2(geom.x, geom.y), vec2(geom.w, geom.h)),
            geom.rot_x,
            geom.rot_y,
            geom.angle,
            Vec2::ZERO,
        );
        let secondary = if geom.has2 {
            Some(rect_poly(
                Rect::from_min_size(pos2(geom.x2, geom.y2), vec2(geom.w2, geom.h2)),
                geom.rot_x,
                geom.rot_y,
                geom.angle,
                Vec2::ZERO,
            ))
        } else {
            None
        };
        let mask = geom.mask_rect.map(|(mx, my, mw, mh)| {
            rect_poly(
                Rect::from_min_size(pos2(mx, my), vec2(mw, mh)),
                geom.rot_x,
                geom.rot_y,
                geom.angle,
                Vec2::ZERO,
            )
        });

        if !dk.desc.decal {
            for p in &primary {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            if let Some(s) = &secondary {
                for p in s {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
            }
        }

        placed.push(Placed {
            dk,
            geom,
            primary,
            secondary,
            mask,
            decal: dk.desc.decal,
        });
    }

    if min_x == f32::MAX {
        return None;
    }

    let shift = vec2(KEYBOARD_WIDGET_PADDING - min_x, KEYBOARD_WIDGET_PADDING - min_y);
    let canvas_size = vec2(
        max_x - min_x + 2.0 * KEYBOARD_WIDGET_PADDING,
        max_y - min_y + 2.0 * KEYBOARD_WIDGET_PADDING,
    );

    let (rect, response) = ui.allocate_exact_size(canvas_size, Sense::click());
    let painter = ui.painter_at(rect);
    let origin = rect.min.to_vec2();

    // Apply the shift to all polygons now that we know the canvas origin.
    for p in &mut placed {
        for v in p.primary.iter_mut() {
            *v += shift + origin;
        }
        if let Some(s) = p.secondary.as_mut() {
            for v in s.iter_mut() {
                *v += shift + origin;
            }
        }
        if let Some(m) = p.mask.as_mut() {
            for v in m.iter_mut() {
                *v += shift + origin;
            }
        }
    }

    // Palette (dark theme keycaps).
    let base = Color32::from_gray(0x3a);
    let base_top = Color32::from_gray(0x4a);
    let mask_fill = Color32::from_gray(0x50);
    let text_color = Color32::from_gray(0xe0);
    let accent = Color32::from_rgb(0x60, 0x90, 0xd0);

    for p in &placed {
        let is_active = active.map(|(t, _)| *t == p.dk.target).unwrap_or(false);
        let is_mask_active = active.map(|(t, m)| *t == p.dk.target && *m).unwrap_or(false);

        let stroke = if is_active && !is_mask_active {
            Stroke::new(2.0_f32, accent)
        } else {
            Stroke::NONE
        };

        // Background (drop shadow / base) and foreground.
        let corner = unit * KEY_ROUNDNESS;
        if p.dk.desc.encoder_idx.is_some() {
            // Encoder: draw a circle.
            let center = rect_center(&p.primary);
            let radius = p.geom.w / 2.0;
            painter.circle_filled(center, radius, base);
            painter.circle_filled(center, radius * 0.85, base_top);
            painter.circle_stroke(center, radius, stroke);
            if !p.dk.text.is_empty() {
                painter.text(
                    center,
                    Align2::CENTER_CENTER,
                    &p.dk.text,
                    FontId::proportional(unit * 0.55),
                    text_color,
                );
            }
        } else {
            draw_rounded_rect(&painter, &p.primary, corner, base);
            if let Some(s) = &p.secondary {
                draw_rounded_rect(&painter, s, corner, base);
            }
            // Foreground (keycap top).
            let fg = p.primary.iter().map(|v| *v).collect::<Vec<_>>();
            let fg_rect = polygon_rect(&fg);
            let inset = unit * SHADOW_SIDE_PADDING;
            let top = Rect::from_min_size(
                fg_rect.min + vec2(inset, unit * SHADOW_TOP_PADDING),
                fg_rect.size() - vec2(2.0 * inset, unit * (SHADOW_BOTTOM_PADDING + SHADOW_TOP_PADDING)),
            );
            painter.rect_filled(top, corner, base_top);

            if is_active {
                painter.rect_stroke(top, corner, stroke, egui::StrokeKind::Inside);
            }

            if p.dk.masked {
                // Outer legend.
                painter.text(
                    top.center(),
                    Align2::CENTER_CENTER,
                    &p.dk.text,
                    FontId::proportional(unit * 0.5),
                    text_color,
                );
                // Mask area.
                if let Some(m) = &p.mask {
                    let mr = polygon_rect(m);
                    let mr = mr.shrink(0.0);
                    painter.rect_filled(mr, corner, mask_fill);
                    if is_mask_active {
                        painter.rect_stroke(mr, corner, Stroke::new(2.0_f32, accent), egui::StrokeKind::Inside);
                    }
                    painter.text(
                        mr.center(),
                        Align2::CENTER_CENTER,
                        &p.dk.mask_text,
                        FontId::proportional(unit * 0.4),
                        text_color,
                    );
                }
            } else {
                painter.text(
                    top.center(),
                    Align2::CENTER_CENTER,
                    &p.dk.text,
                    FontId::proportional(unit * 0.6),
                    text_color,
                );
            }
        }

        let _ = p.dk.tooltip;
    }

    // Hit test.
    let mut hit: Option<KeyboardHit> = None;
    if response.clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            // Iterate in reverse draw order (topmost first).
            for p in placed.iter().rev() {
                if p.decal {
                    continue;
                }
                if p.dk.masked {
                    if let Some(m) = &p.mask {
                        if point_in_poly(pointer, m) {
                            hit = Some(KeyboardHit { target: p.dk.target, mask: true });
                            break;
                        }
                    }
                }
                let in_primary = point_in_poly(pointer, &p.primary);
                let in_secondary = p
                    .secondary
                    .as_ref()
                    .map(|s| point_in_poly(pointer, s))
                    .unwrap_or(false);
                if in_primary || in_secondary {
                    hit = Some(KeyboardHit { target: p.dk.target, mask: false });
                    break;
                }
            }
        }
    }

    hit
}

fn polygon_rect(poly: &[Pos2]) -> Rect {
    let mut min = pos2(f32::MAX, f32::MAX);
    let mut max = pos2(f32::MIN, f32::MIN);
    for p in poly {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    Rect::from_min_max(min, max)
}

fn rect_center(poly: &[Pos2]) -> Pos2 {
    let r = polygon_rect(poly);
    r.center()
}

fn draw_rounded_rect(painter: &egui::Painter, poly: &[Pos2], corner: f32, fill: Color32) {
    let r = polygon_rect(poly);
    painter.rect_filled(r, corner, fill);
}
