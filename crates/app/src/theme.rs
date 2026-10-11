//! ORRERY HMI theme — PLAN.md §2 design language.
//! Flat, hard-edged, monochrome + three accents. No gradients, no rounding, no shadows.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

// ---- §2 palette tokens -----------------------------------------------------
pub const BG: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
pub const CELL: Color32 = Color32::from_rgb(0x0A, 0x0A, 0x0A);
pub const GRID: Color32 = Color32::from_rgb(0x6E, 0x6E, 0x6E);
pub const GRID_HI: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const CYAN: Color32 = Color32::from_rgb(0x21, 0xD0, 0xC5);
pub const GREEN: Color32 = Color32::from_rgb(0x43, 0xA0, 0x47);
pub const AMBER: Color32 = Color32::from_rgb(0xE0, 0xA8, 0x00);
pub const ALARM: Color32 = Color32::from_rgb(0xD2, 0x3B, 0x2E);
pub const GRAY: Color32 = Color32::from_rgb(0x57, 0x57, 0x4A);
pub const INK: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
pub const CELL_DIM: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
pub const CELL_HOV: Color32 = Color32::from_rgb(0x16, 0x16, 0x16);
pub const ROW_LINE: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1C);
pub const PLOT_DOT: Color32 = Color32::from_rgb(0x2E, 0x2E, 0x2E);
pub const PLOT_FAINT: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);

pub const STROKE_GRID: Stroke = Stroke { width: 1.0, color: GRID };
pub const STROKE_HI: Stroke = Stroke { width: 1.0, color: GRID_HI };

/// Base pixel-font sizes. VT323 (OFL) is a light 1px-stem terminal face — the
/// previous Press Start 2P read too bold (user feedback, session 3).
pub const PX: f32 = 16.0;
pub const PX_SM: f32 = 13.0;
pub const PX_LG: f32 = 22.0;
/// Approx glyph advance of VT323 (≈0.43 em) — for fixed-width box math.
pub const CH: f32 = 0.43;

pub fn font(sz: f32) -> FontId {
    FontId::new(sz, FontFamily::Monospace)
}

pub fn txt(s: impl Into<String>, sz: f32, c: Color32) -> egui::RichText {
    egui::RichText::new(s.into()).font(font(sz)).color(c)
}

pub fn apply_theme(ctx: &egui::Context) {
    // ---- fonts: bundled pixel font first, Consolas-ish fallback ----------
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        "pixel".to_owned(),
        std::sync::Arc::new(FontData::from_owned(
            include_bytes!("../../../assets/fonts/VT323-Regular.ttf").to_vec(),
        )),
    );
    defs.families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "pixel".to_owned());
    defs.families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "pixel".to_owned());
    ctx.set_fonts(defs);

    // ---- visuals: hard flat cells -----------------------------------------
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CELL;
    v.window_stroke = STROKE_GRID;
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.window_shadow = Default::default();
    v.selection.bg_fill = CYAN.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, CYAN);
    v.faint_bg_color = CELL;
    v.extreme_bg_color = BG;

    v.widgets.noninteractive.bg_fill = CELL;
    v.widgets.noninteractive.bg_stroke = STROKE_GRID;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, GRID_HI);
    v.widgets.noninteractive.corner_radius = egui::CornerRadius::ZERO;

    v.widgets.inactive.bg_fill = CELL;
    v.widgets.inactive.bg_stroke = STROKE_GRID;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, GRAY);
    v.widgets.inactive.corner_radius = egui::CornerRadius::ZERO;
    v.widgets.inactive.weak_bg_fill = CELL;

    v.widgets.hovered.bg_fill = CELL_HOV;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GRID_HI);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, GRID_HI);
    v.widgets.hovered.corner_radius = egui::CornerRadius::ZERO;
    v.widgets.hovered.weak_bg_fill = v.widgets.hovered.bg_fill;

    v.widgets.active.bg_fill = CYAN;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, GRID_HI);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, INK);
    v.widgets.active.corner_radius = egui::CornerRadius::ZERO;
    v.widgets.active.weak_bg_fill = CYAN;

    v.widgets.open = v.widgets.inactive;

    ctx.set_visuals(v);

    // ---- spacing: tight mechanical grid ------------------------------------
    let mut s = (*ctx.style()).clone();
    s.spacing.item_spacing = egui::vec2(6.0, 6.0);
    s.spacing.button_padding = egui::vec2(6.0, 4.0);
    s.spacing.window_margin = egui::Margin::ZERO;
    s.text_styles.insert(egui::TextStyle::Body, font(PX));
    s.text_styles.insert(egui::TextStyle::Monospace, font(PX));
    s.text_styles.insert(egui::TextStyle::Button, font(PX_SM));
    s.text_styles.insert(egui::TextStyle::Small, font(PX_SM));
    s.text_styles.insert(egui::TextStyle::Heading, font(PX_LG));
    ctx.set_style(s);
}

// ---- cell primitives (§2: every panel = 1px-bordered cell on black) --------

/// Fill + 1px border a rect like a reference-image cell.
pub fn paint_cell(p: &egui::Painter, r: egui::Rect, fill: Color32, stroke: Stroke) {
    p.rect_filled(r, 0.0, fill);
    p.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Outside);
}

/// Cell title strip: uppercase label, top-left, with a separator line under it.
pub fn cell_title(p: &egui::Painter, r: egui::Rect, title: &str) {
    p.text(
        r.left_top() + egui::vec2(6.0, 5.0),
        egui::Align2::LEFT_TOP,
        title,
        font(PX_SM),
        GRID_HI,
    );
    let y = r.top() + 24.0;
    p.line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], STROKE_GRID);
}

/// Discrete segmented bar (like the МИН/МАКС gauges): `frac` filled segments.
pub fn segment_bar(p: &egui::Painter, r: egui::Rect, frac: f32, segs: usize, on: Color32) {
    let segs = segs.max(1);
    let gap = 2.0;
    let w = (r.width() - gap * (segs as f32 - 1.0)) / segs as f32;
    let filled = (frac.clamp(0.0, 1.0) * segs as f32).round() as usize;
    for i in 0..segs {
        let sr = egui::Rect::from_min_size(
            egui::pos2(r.left() + i as f32 * (w + gap), r.top()),
            egui::vec2(w, r.height()),
        );
        let c = if i < filled { on } else { CELL_DIM };
        p.rect_filled(sr, 0.0, c);
        p.rect_stroke(sr, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    }
}
