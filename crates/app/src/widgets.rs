//! Flat HMI widgets drawn exactly like the reference control panel:
//! rectangular keys, status value cells, label/value readout rows, tabs.

use crate::theme::*;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Ui, Vec2};

/// Plain-language tooltip, gated by the global TIPS toggle (session 6).
pub fn tip(resp: Response, on: bool, text: &str) -> Response {
    if on {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

/// A bottom-bar style flat key. `accent` overrides the fill when active
/// (AUTO uses AMBER per §9). Returns click response.
pub fn hmi_key(
    ui: &mut Ui,
    id: &str,
    label: &str,
    active: bool,
    accent: Option<Color32>,
    size: Vec2,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, active, label));
    let id = ui.id().with(id);
    let hovered = resp.hovered();
    let p = ui.painter().clone();

    let (fill, fg, border) = if active {
        (accent.unwrap_or(CYAN), INK, GRID_HI)
    } else if hovered {
        (CELL_HOV, GRID_HI, GRID_HI)
    } else {
        (CELL, GRAY, GRID)
    };
    p.rect_filled(rect, 0.0, fill);
    p.rect_stroke(rect, 0.0, Stroke1(border), egui::StrokeKind::Outside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font(PX_SM), fg);
    if resp.clicked() {
        ui.memory_mut(|m| m.surrender_focus(id)); // keep mechanical feel, no focus ring
    }
    resp
}

fn Stroke1(c: Color32) -> egui::Stroke {
    egui::Stroke::new(1.0_f32, c)
}

/// Label / value readout row: `TYPE  PLANET`, value colour-coded by state (§2).
pub fn readout(ui: &mut Ui, label: &str, value: &str, vc: Color32) {
    let row = ui.available_height().min(22.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row), Sense::hover());
    let p = ui.painter();
    p.rect_stroke(rect, 0.0, Stroke1(ROW_LINE), egui::StrokeKind::Outside);
    p.text(
        Pos2::new(rect.left() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font(PX_SM),
        GRAY,
    );
    p.text(
        Pos2::new(rect.right() - 6.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        value,
        font(PX_SM),
        vc,
    );
}

/// Status cell with a solid colour fill and black text (like the cyan tab cells).
pub fn status_cell(ui: &mut Ui, label: &str, fill: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, fill);
    p.rect_stroke(rect, 0.0, Stroke1(GRID), egui::StrokeKind::Outside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font(PX_SM), INK);
}

/// Tab strip (`LIVE(1×) | 60× | …`): active tab = solid CYAN with black text (§2).
/// Returns the newly selected index if clicked.
pub fn tab_strip(ui: &mut Ui, tabs: &[&str], selected: usize) -> Option<usize> {
    let mut clicked = None;
    let h = 24.0;
    let w = ((ui.available_width() - (tabs.len().saturating_sub(1)) as f32 * 2.0) / tabs.len() as f32).max(40.0);
    for (i, t) in tabs.iter().enumerate() {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        if resp.clicked() {
            clicked = Some(i);
        }
        let active = i == selected;
        let hov = resp.hovered();
        let p = ui.painter();
        let (fill, fg) = if active {
            (CYAN, INK)
        } else if hov {
            (CELL_HOV, GRID_HI)
        } else {
            (CELL, GRAY)
        };
        p.rect_filled(rect, 0.0, fill);
        p.rect_stroke(rect, 0.0, Stroke1(if active { GRID_HI } else { GRID }), egui::StrokeKind::Outside);
        p.text(rect.center(), egui::Align2::CENTER_CENTER, *t, font(PX_SM), fg);
    }
    clicked
}

/// Numbered selector cell (`01`…`11`), active = CYAN (§9).
pub fn number_cell(ui: &mut Ui, n: usize, active: bool) -> Response {
    let label = format!("{:02}", n + 1);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(38.0, 26.0), Sense::click());
    let p = ui.painter();
    let hov = resp.hovered();
    let (fill, fg, border) = if active {
        (CYAN, INK, GRID_HI)
    } else if hov {
        (CELL_HOV, GRID_HI, GRID_HI)
    } else {
        (CELL, GRAY, GRID)
    };
    p.rect_filled(rect, 0.0, fill);
    p.rect_stroke(rect, 0.0, Stroke1(border), egui::StrokeKind::Outside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font(PX_SM), fg);
    resp
}

/// Scope-style plot frame: black field, white tick marks, dotted grid (§2).
/// Returns the inner plot rect for trace drawing.
pub fn scope_frame(p: &egui::Painter, r: Rect, ticks: usize) -> Rect {
    p.rect_filled(r, 0.0, BG);
    p.rect_stroke(r, 0.0, Stroke1(GRID), egui::StrokeKind::Outside);
    let inner = r.shrink(10.0);
    // dotted grid
    let dot = Stroke1(PLOT_DOT);
    for i in 1..ticks {
        let f = i as f32 / ticks as f32;
        let x = egui::lerp(inner.left()..=inner.right(), f);
        let y = egui::lerp(inner.top()..=inner.bottom(), f);
        dotted_vline(p, x, inner.top(), inner.bottom(), dot);
        dotted_hline(p, y, inner.left(), inner.right(), dot);
    }
    // axes + ticks
    p.line_segment([inner.left_bottom(), inner.right_bottom()], Stroke1(GRID_HI));
    p.line_segment([inner.left_bottom(), inner.left_top()], Stroke1(GRID_HI));
    for i in 0..=ticks {
        let f = i as f32 / ticks as f32;
        let x = egui::lerp(inner.left()..=inner.right(), f);
        let y = egui::lerp(inner.bottom()..=inner.top(), f);
        p.line_segment([egui::pos2(x, inner.bottom()), egui::pos2(x, inner.bottom() + 4.0)], Stroke1(GRID_HI));
        p.line_segment([egui::pos2(inner.left() - 4.0, y), egui::pos2(inner.left(), y)], Stroke1(GRID_HI));
    }
    inner
}

fn dotted_vline(p: &egui::Painter, x: f32, y0: f32, y1: f32, s: egui::Stroke) {
    let mut y = y0;
    while y < y1 {
        p.line_segment([egui::pos2(x, y), egui::pos2(x, (y + 2.0).min(y1))], s);
        y += 5.0;
    }
}
fn dotted_hline(p: &egui::Painter, y: f32, x0: f32, x1: f32, s: egui::Stroke) {
    let mut x = x0;
    while x < x1 {
        p.line_segment([egui::pos2(x, y), egui::pos2((x + 2.0).min(x1), y)], s);
        x += 5.0;
    }
}
