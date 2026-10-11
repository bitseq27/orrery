//! §9 UI layout — direct mapping of the reference control panel.
//!
//! ┌──────────────────────── header (SYS ▾ · EPOCH · CAM TARGET) ─────────────────────────┐
//! ├───────────────┬──────────────────────────────────────────────────────────────────────┤
//! │ left column   │                     3D viewport cell                                 │
//! │ telemetry     │                     + top-right minimap cell                         │
//! │ raw / scope   │                                                                      │
//! │ data link     │                                                                      │
//! ├───────────────┴──────────────────────────────────────────────────────────────────────┤
//! │ 01..11 selector strip · speed tabs + slider · bottom key row (AUTO = amber)          │
//! └──────────────────────────────────────────────────────────────────────────────────────┘

use crate::app_state::{AppState, CamMode, ScaleMode, RATE_TABS};
use crate::telemetry::Telemetry;
use crate::theme::*;
use crate::widgets::*;
use crate::widgets::tip;
use eframe::egui::{self, Color32, Pos2, Rect, Ui, Vec2};
use orrery_astro::planets::period_days;

/// The 11 systems of the selector strip (§6.2) — SOL + catalog hosts.
pub fn system_names(cat: &crate::catalog::Catalog) -> Vec<String> {
    let mut v = vec!["SOL".to_string()];
    v.extend(cat.systems.iter().map(|s| s.host.name.to_uppercase().replace(' ', "-")));
    v
}

// ---------------------------------------------------------------- header ----
/// Every name the FIND BODY field can offer, for the active system: exactly
/// the set `telemetry::resolve` / `telemetry_exo` can turn into live telemetry
/// (stars excluded in exo systems — their host carries no target telemetry).
fn searchable_pool(cat: &crate::catalog::Catalog, system: usize) -> Vec<String> {
    let mut v: Vec<String> = Vec::with_capacity(1300);
    if system == 0 {
        v.push(crate::body_data::SUN.to_string());
        v.extend(orrery_astro::planets::PLANET_NAMES.iter().map(|n| n.to_string()));
        v.extend(orrery_astro::moons::moon_defs().iter().map(|m| m.name.to_string()));
        v.extend(orrery_astro::probes::probes().iter().map(|p| p.name.to_string()));
        v.extend(orrery_astro::small_bodies::small_bodies().iter().map(|b| b.name.to_string()));
        v.extend(orrery_astro::population::population().iter().map(|b| b.name.to_string()));
    } else if let Some(s) = cat.systems.get(system - 1) {
        v.extend(s.planets.iter().map(|p| p.name.clone()));
    }
    v.sort();
    v.dedup();
    v
}

pub fn draw_header(ui: &mut Ui, st: &mut AppState, epoch: &str, cam_target: &str, names: &[String], fps: f32, cat: &crate::catalog::Catalog) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), egui::Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, BG);
    p.line_segment([rect.left_bottom(), rect.right_bottom()], STROKE_GRID);

    // SYS combo (R7)
    let sys_rect = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 6.0), Vec2::new(150.0, 22.0));
    p.rect_filled(sys_rect, 0.0, CELL);
    p.rect_stroke(sys_rect, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(sys_rect.left_center() + Vec2::new(6.0, 0.0), egui::Align2::LEFT_CENTER,
        format!("SYS: {}", names[st.system]), font(PX_SM), CYAN);
    p.text(sys_rect.right_center() - Vec2::new(6.0, 0.0), egui::Align2::RIGHT_CENTER, "▼", font(PX_SM), GRAY);
    if tip(ui.interact(sys_rect, ui.id().with("sys_combo_btn"), egui::Sense::click()), st.tips,
        "Pick which star system to look at. 01 = SOL, our home system.")
        .clicked()
    {
        st.sys_menu_open = !st.sys_menu_open;
    }
    if st.sys_menu_open {
        let menu_pos = sys_rect.left_bottom() + Vec2::new(0.0, 2.0);
        let mut pick: Option<usize> = None;
        let resp = egui::Area::new(egui::Id::new("sys_menu"))
            .fixed_pos(menu_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let frame = egui::Frame::popup(&ui.style());
                frame.show(ui, |ui| {
                    ui.set_width(220.0);
                    for (i, n) in names.iter().enumerate() {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(216.0, 24.0), egui::Sense::click());
                        let hov = ui.rect_contains_pointer(r);
                        let p = ui.painter().clone();
                        p.rect_filled(r, 0.0, if st.system == i { CYAN } else if hov { CELL_HOV } else { CELL });
                        p.text(r.left_center() + Vec2::new(6.0, 0.0), egui::Align2::LEFT_CENTER,
                            format!("{:02} {}", i + 1, n), font(PX_SM), if st.system == i { INK } else if hov { GRID_HI } else { GRAY });
                        if ui.interact(r, ui.id().with(("sysmenu_item", i)), egui::Sense::click()).clicked() {
                            pick = Some(i);
                        }
                    }
                })
            })
            .response;
        if let Some(i) = pick {
            st.system = i;
            st.sys_menu_open = false;
        }
        // close on click outside or Escape
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let outside_click = ui.input(|i| i.pointer.any_click()) && !resp.hovered() && !ui.rect_contains_pointer(sys_rect);
        if esc || outside_click {
            st.sys_menu_open = false;
        }
    }

    // EPOCH readout + FPS (M6 perf visibility, §12)
    let epos = Pos2::new(sys_rect.right() + 24.0, rect.center().y);
    p.text(epos, egui::Align2::LEFT_CENTER, format!("EPOCH {}", epoch), font(PX_SM), GRID_HI);
    let fcol = if fps > 50.0 { GREEN } else if fps > 25.0 { AMBER } else { ALARM };
    let camx = rect.right() - 470.0;
    p.text(Pos2::new(camx - 96.0, rect.center().y), egui::Align2::LEFT_CENTER,
        format!("FPS {:>4.1}", fps), font(PX_SM), fcol);

    // ---- FIND BODY search field (session 8): populate-as-you-type list, ----
    // ---- clicking a candidate selects that body on the map               ----
    let sx = sys_rect.right() + 210.0;
    let sw = (camx - 104.0 - sx).clamp(120.0, 320.0);
    let s_rect = Rect::from_min_size(Pos2::new(sx, rect.top() + 6.0), Vec2::new(sw, 22.0));
    let ed = egui::TextEdit::singleline(&mut st.search)
        .id_source("find_body")
        .desired_width(sw - 8.0)
        .hint_text("FIND BODY...")
        .font(font(PX_SM))
        .text_color(GRID_HI);
    let ed_resp = ui.put(s_rect, ed);
    let focused = ui.ctx().memory(|m| m.focused()) == Some(ed_resp.id);
    if ed_resp.changed() || ed_resp.clicked() {
        st.search_open = true;
    }
    tip(ed_resp.clone(), st.tips,
        "Type any part of a body name (planet, moon, asteroid, comet, probe...).\nThe matching list fills in as you type; click a name to select it on the map.\nENTER picks the first match, ESC closes the list.");
    if st.search_pool_sys != st.system {
        st.search_pool = searchable_pool(cat, st.system);
        st.search_pool_sys = st.system;
    }
    let q = st.search.trim().to_uppercase();
    let cands: Vec<String> = if q.is_empty() {
        vec![]
    } else {
        let mut v: Vec<&String> = st.search_pool.iter().filter(|n| n.contains(&q)).collect();
        v.sort_by(|a, b| {
            b.starts_with(&q)
                .cmp(&a.starts_with(&q))
                .then(a.len().cmp(&b.len()))
                .then(a.cmp(b))
        });
        v.into_iter().take(12).cloned().collect()
    };
    // ENTER while the field has focus picks the first (best) match
    let enter_pick =
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && focused { cands.first().cloned() } else { None };
    if st.search_open && !q.is_empty() && !cands.is_empty() {
        let menu_pos = s_rect.left_bottom() + Vec2::new(0.0, 2.0);
        let mut pick: Option<String> = enter_pick;
        let dw = sw.max(240.0);
        let resp = egui::Area::new(egui::Id::new("find_menu"))
            .fixed_pos(menu_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let frame = egui::Frame::popup(&ui.style());
                frame.show(ui, |ui| {
                    ui.set_width(dw);
                    for n in cands.iter() {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(dw - 4.0, 22.0), egui::Sense::click());
                        let hov = ui.rect_contains_pointer(r);
                        let pp = ui.painter().clone();
                        pp.rect_filled(r, 0.0, if hov { CELL_HOV } else { CELL });
                        pp.text(r.left_center() + Vec2::new(6.0, 0.0), egui::Align2::LEFT_CENTER,
                            n, font(PX_SM), if hov { GRID_HI } else { GRAY });
                        if ui.interact(r, ui.id().with(("find_item", n.clone())), egui::Sense::click()).clicked() {
                            pick = Some(n.clone());
                        }
                    }
                })
            })
            .response;
        if let Some(n) = pick {
            st.selected_body = Some(n);
            st.search.clear();
            st.search_open = false;
        }
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let outside = ui.input(|i| i.pointer.any_click()) && !resp.hovered() && !ui.rect_contains_pointer(s_rect);
        if esc || outside {
            st.search_open = false;
        }
    }

    // camera mode tabs (R9)
    let modes = [("FREE", CamMode::Free), ("TRACK", CamMode::Track), ("TOP", CamMode::TopDown)];
    for (i, (label, m)) in modes.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(camx + i as f32 * 52.0, rect.top() + 6.0), Vec2::new(48.0, 22.0));
        let active = st.cam == *m;
        p.rect_filled(r, 0.0, if active { CYAN } else { CELL });
        p.rect_stroke(r, 0.0, if active { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, *label, font(PX_SM), if active { INK } else { GRAY });
        let cam_tip = match *m {
            CamMode::Free => "Free camera: left-drag to orbit around the target, right-drag to slide, mouse wheel to zoom.",
            CamMode::Track => "Tracking camera: glued to the selected body as it moves.",
            CamMode::TopDown => "Top-down camera: look straight onto the orbital plane.",
        };
        if tip(ui.interact(r, ui.id().with(format!("cam{i}")), egui::Sense::click()), st.tips, cam_tip).clicked() {
            st.cam = *m;
            st.auto_tour = false;
        }
    }
    // CAM TARGET
    p.text(Pos2::new(camx + 170.0, rect.center().y), egui::Align2::LEFT_CENTER,
        format!("CAM TARGET: {}", cam_target), font(PX_SM), AMBER);
    // TIPS toggle (session 6), far top-right
    let tr = Rect::from_min_size(rect.right_top() + Vec2::new(-92.0, 6.0), Vec2::new(84.0, 22.0));
    p.rect_filled(tr, 0.0, if st.tips { CYAN } else { CELL });
    p.rect_stroke(tr, 0.0, if st.tips { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
    p.text(tr.center(), egui::Align2::CENTER_CENTER, if st.tips { "TIPS ON" } else { "TIPS OFF" }, font(PX_SM), if st.tips { INK } else { GRAY });
    tip(ui.interact(tr, ui.id().with("tips_btn"), egui::Sense::click()), st.tips,
        "Turn these plain-language help tips on or off.")
        .clicked()
        .then(|| st.tips = !st.tips);
}

// --------------------------------------------- collapsible-panel widgets ----
/// 16×16 `[–]` chip: returns true when clicked (caller collapses the panel).
pub fn collapse_chip(ui: &mut Ui, at: Pos2, id: &str, tips: bool) -> bool {
    let r = Rect::from_min_size(at, Vec2::new(16.0, 16.0));
    let p = ui.painter().clone();
    p.rect_filled(r, 0.0, CELL);
    p.rect_stroke(r, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, "–", font(PX_SM), GRID_HI);
    tip(ui.interact(r, ui.id().with(id), egui::Sense::click()), tips, "Hide this panel.").clicked()
}

/// Thin collapsed strip with a centred `[+] LABEL` chip: true → expand.
pub fn draw_strip(ui: &mut Ui, _st: &mut AppState, rect: Rect, label: &str, id: &str) -> bool {
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, CELL_DIM);
    p.line_segment([rect.left_top(), rect.right_top()], STROKE_GRID);
    let w = 40.0 + label.len() as f32 * 7.0;
    let r = Rect::from_min_size(Pos2::new(rect.center().x - w / 2.0, rect.top() + 1.0), Vec2::new(w, rect.height() - 2.0));
    p.rect_filled(r, 0.0, CELL);
    p.rect_stroke(r, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, format!("+ {label}"), font(PX_SM), GRID_HI);
    let hit = ui.interact(r, ui.id().with(id), egui::Sense::click());
    hit.clicked() || (hit.hovered() && ui.input(|i| i.pointer.any_click()))
}

/// Vertical rail for collapsed side panels: `[+]` chip + stacked label letters.
pub fn expand_rail(ui: &mut Ui, label: &str, id: &str, tips: bool) -> bool {
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, CELL_DIM);
    let chip = Rect::from_min_size(rect.left_top() + Vec2::new(2.0, 2.0), Vec2::new(16.0, 16.0));
    p.rect_filled(chip, 0.0, CELL);
    p.rect_stroke(chip, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(chip.center(), egui::Align2::CENTER_CENTER, "+", font(PX_SM), GRID_HI);
    let mut y = chip.bottom() + 10.0;
    for ch in label.chars() {
        p.text(Pos2::new(rect.center().x, y), egui::Align2::CENTER_CENTER, ch.to_string(), font(10.0), GRAY);
        y += 10.0;
    }
    tip(ui.interact(rect, ui.id().with(id), egui::Sense::click()), tips,
        format!("Open the {label} panel again.").as_str())
        .clicked()
}

// --------------------------------------------------- SPECTRAL ANALYSIS col --
/// Generic 2-D curve graph for the spectra column.
#[allow(clippy::too_many_arguments)]
fn spec_graph(
    p: &egui::Painter,
    rect: Rect,
    title: &str,
    pts: &[[f32; 2]],
    xmin: f32,
    xmax: f32,
    logx: bool,
    logy: bool,
    col: Color32,
    bands: &[(f32, f32, f32, &str)],
    lines: &[(f32, f32, &str)],
    ticks: &[f32],
    xlab: &str,
) {
    p.rect_filled(rect, 0.0, BG);
    p.rect_stroke(rect, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(4.0, 12.0), rect.right_bottom() - Vec2::new(3.0, 10.0));
    let ttl: String = title.chars().take((rect.width() / 5.0) as usize).collect();
    p.text(rect.left_top() + Vec2::new(4.0, 1.0), egui::Align2::LEFT_TOP, ttl, font(9.0), GRID_HI);
    p.text(Pos2::new(rect.right() - 3.0, rect.bottom() - 9.0), egui::Align2::RIGHT_TOP, xlab, font(8.0), GRAY);
    let lx = |lam: f32| -> f32 {
        if logx {
            let a = xmin.log10();
            let b = xmax.log10();
            ((lam.max(xmin).log10() - a) / (b - a)).clamp(0.0, 1.0)
        } else {
            ((lam - xmin) / (xmax - xmin)).clamp(0.0, 1.0)
        }
    };
    let ly = |v: f32| -> f32 {
        if logy {
            ((v.max(1e-3).log10() + 3.0) / 3.0).clamp(0.0, 1.0)
        } else {
            (v / 1.05).clamp(0.0, 1.0)
        }
    };
    for t in ticks {
        let x = egui::lerp(plot.left()..=plot.right(), lx(*t));
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())], Stroke1(PLOT_DOT));
        p.text(Pos2::new(x, rect.bottom() - 9.0), egui::Align2::CENTER_TOP, format!("{t}"), font(8.0), GRAY);
    }
    let poly: Vec<Pos2> = pts
        .iter()
        .map(|q| Pos2::new(egui::lerp(plot.left()..=plot.right(), lx(q[0])), egui::lerp(plot.bottom()..=plot.top(), ly(q[1]))))
        .collect();
    p.add(egui::Shape::line(poly, egui::Stroke::new(1.2_f32, col)));
    for (bi, (c, _, _, lab)) in bands.iter().take(4).enumerate() {
        let x = egui::lerp(plot.left()..=plot.right(), lx(*c));
        p.line_segment([Pos2::new(x, plot.bottom()), Pos2::new(x, plot.bottom() - 4.0)], Stroke1(CYAN.linear_multiply(0.7)));
        p.text(Pos2::new(x.clamp(plot.left() + 14.0, plot.right() - 14.0), plot.top() + 1.0 + bi as f32 * 8.0), egui::Align2::CENTER_TOP, format!("{:.2} {}", c, lab), font(7.5), CYAN.linear_multiply(0.7));
    }
    for (c, _, lab) in lines.iter().take(3) {
        let x = egui::lerp(plot.left()..=plot.right(), lx(*c));
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.top() + 4.0)], Stroke1(Color32::from_rgb(255, 120, 220)));
        p.text(Pos2::new(x.clamp(plot.left() + 14.0, plot.right() - 14.0), plot.bottom() - 2.0 - 0.0), egui::Align2::CENTER_BOTTOM, format!("{}", lab), font(7.5), Color32::from_rgb(255, 120, 220).linear_multiply(0.8));
    }
}

/// Fifth model graph (session 7): the MIR FINGERPRINT REGION, FTIR convention —
/// reversed wavenumber axis (4000→400 cm⁻¹ left→right) with a µm sub-scale,
/// deepest-feature tick labels, and the nearest-archetype MATCH readout from
/// curve cross-correlation (see spectra::fingerprint).
fn fp_graph(p: &egui::Painter, rect: Rect, fp: &crate::spectra::FpCurve) {
    use crate::spectra::{FP_MAX, FP_MIN};
    p.rect_filled(rect, 0.0, BG);
    p.rect_stroke(rect, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(4.0, 12.0), rect.right_bottom() - Vec2::new(3.0, 18.0));
    // MATCH readout string first — the title must truncate to leave room for it.
    // In a narrow column (<340 px) the match goes on its own second line instead.
    let match_str = match (fp.match_label, fp.match_pct) {
        (Some(l), Some(pct)) => format!("MATCH {} {}%", l, pct),
        _ => "NO SOLID-STATE FINGERPRINT".to_string(),
    };
    let narrow = rect.width() < 340.0;
    let plot = if narrow {
        Rect::from_min_max(rect.left_top() + Vec2::new(4.0, 20.0), rect.right_bottom() - Vec2::new(3.0, 18.0))
    } else {
        plot
    };
    let title_room = if narrow {
        (rect.width() / 5.0) as usize
    } else {
        ((rect.width() / 5.0) as usize).saturating_sub(match_str.chars().count() + 2)
    };
    let ttl: String = format!("MIR FINGERPRINT cm-1 — {}", fp.material).chars().take(title_room).collect();
    p.text(rect.left_top() + Vec2::new(4.0, 1.0), egui::Align2::LEFT_TOP, ttl, font(9.0), GRID_HI);
    // MATCH readout — the fingerprint score against the class library
    let col = match fp.match_pct {
        Some(pct) if pct >= 75 => GREEN,
        Some(_) => AMBER,
        None => GRAY,
    };
    let my = if narrow { rect.top() + 10.0 } else { rect.top() + 1.0 };
    p.text(Pos2::new(rect.right() - 4.0, my), egui::Align2::RIGHT_TOP, match_str, font(9.0), col);
    // reversed axis: high ν̃ on the left, like a real FTIR plot
    let lx = |nu: f32| ((FP_MAX - nu) / (FP_MAX - FP_MIN)).clamp(0.0, 1.0);
    let ly = |v: f32| (v / 1.05).clamp(0.0, 1.0);
    for t in [4000.0f32, 3000.0, 2000.0, 1500.0, 1000.0, 500.0] {
        let x = egui::lerp(plot.left()..=plot.right(), lx(t));
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())], Stroke1(PLOT_DOT));
        p.text(Pos2::new(x, rect.bottom() - 17.0), egui::Align2::CENTER_TOP, format!("{t:.0}"), font(8.0), GRAY);
        p.text(Pos2::new(x, rect.bottom() - 9.0), egui::Align2::CENTER_TOP, format!("{:.1}um", 1e4 / t), font(7.0), GRAY.linear_multiply(0.8));
    }
    let poly: Vec<Pos2> = fp
        .pts
        .iter()
        .map(|q| Pos2::new(egui::lerp(plot.left()..=plot.right(), lx(q[0])), egui::lerp(plot.bottom()..=plot.top(), ly(q[1]))))
        .collect();
    let pink = Color32::from_rgb(255, 120, 220);
    p.add(egui::Shape::line(poly, egui::Stroke::new(1.2, pink)));
    // deepest features: top ticks + staggered labels (up to 6 to limit clutter)
    for (bi, (c, _, _, lab)) in fp.feats.iter().take(6).enumerate() {
        let x = egui::lerp(plot.left()..=plot.right(), lx(*c));
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.top() + 4.0)], Stroke1(pink.linear_multiply(0.7)));
        p.text(Pos2::new(x.clamp(plot.left() + 20.0, plot.right() - 20.0), plot.top() + 1.0 + bi as f32 * 8.0), egui::Align2::CENTER_TOP,
            format!("{:.0} {}", c, lab), font(7.5), pink.linear_multiply(0.7));
    }
    // model-provenance note, lower-left inside the plot
    p.text(plot.left_bottom() + Vec2::new(2.0, -2.0), egui::Align2::LEFT_BOTTOM, fp.note, font(7.0), GRAY);
}

/// Sixth model graph (session 8): rotation light curve, Fourier-synthesised
/// from period + harmonic amplitudes (spectra::light_curve). Phase axis 0..1,
/// Δmag flipped so brighter is up, harmonic amplitudes read out in the corner.
fn lc_graph(p: &egui::Painter, rect: Rect, lc: &crate::spectra::LcCurve) {
    p.rect_filled(rect, 0.0, BG);
    p.rect_stroke(rect, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    let read = match lc.period_h {
        Some(_) => format!("PK-PK {:.2} MAG (FOURIER MODEL)", lc.pk_pk),
        None => "NO ROTATION PERIOD".to_string(),
    };
    // narrow column: readout gets its own second line, like the fingerprint graph
    let narrow = rect.width() < 340.0;
    let plot = if narrow {
        Rect::from_min_max(rect.left_top() + Vec2::new(4.0, 20.0), rect.right_bottom() - Vec2::new(3.0, 18.0))
    } else {
        Rect::from_min_max(rect.left_top() + Vec2::new(4.0, 12.0), rect.right_bottom() - Vec2::new(3.0, 18.0))
    };
    let head = match lc.period_h {
        Some(h) => format!("ROT LIGHT CURVE P={h:.2} H{}", if lc.retro { " RETRO" } else { "" }),
        None => "ROT LIGHT CURVE".to_string(),
    };
    let room = if narrow {
        (rect.width() / 5.0) as usize
    } else {
        ((rect.width() / 5.0) as usize).saturating_sub(read.chars().count() + 2)
    };
    let ttl: String = format!("{head} — {}", lc.note).chars().take(room.max(10)).collect();
    p.text(rect.left_top() + Vec2::new(4.0, 1.0), egui::Align2::LEFT_TOP, ttl, font(9.0), GRID_HI);
    let ry = if narrow { rect.top() + 10.0 } else { rect.top() + 1.0 };
    p.text(Pos2::new(rect.right() - 4.0, ry), egui::Align2::RIGHT_TOP, read, font(9.0),
        if lc.period_h.is_some() { AMBER } else { GRAY });
    let lx = |ph: f32| ph.clamp(0.0, 1.0);
    let (mn, mx) = lc.pts.iter().map(|q| q[1]).fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let span = (mx - mn).max(1e-6);
    // brighter (lower Δmag) towards the top, 7% head/bottom room
    let ly = |v: f32| 0.93 - ((v - mn) / span).clamp(0.0, 1.0) * 0.86;
    for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let x = egui::lerp(plot.left()..=plot.right(), lx(t));
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())], Stroke1(PLOT_DOT));
        p.text(Pos2::new(x, rect.bottom() - 17.0), egui::Align2::CENTER_TOP, format!("{t:.2}"), font(8.0), GRAY);
    }
    p.text(Pos2::new(rect.right() - 3.0, rect.bottom() - 9.0), egui::Align2::RIGHT_TOP, "ROT PHASE", font(8.0), GRAY);
    let poly: Vec<Pos2> = lc
        .pts
        .iter()
        .map(|q| Pos2::new(egui::lerp(plot.left()..=plot.right(), lx(q[0])), egui::lerp(plot.bottom()..=plot.top(), ly(q[1]))))
        .collect();
    p.add(egui::Shape::line(poly, egui::Stroke::new(1.2, GRID_HI)));
    // harmonic amplitude read-out + axis sense, in the plot corners
    let hs: String = lc.amps.iter().map(|(k, a)| format!("A{k} {a:.3} ")).collect();
    let corner = if hs.is_empty() { lc.note.to_string() } else { format!("{hs}MAG") };
    p.text(plot.left_bottom() + Vec2::new(2.0, -2.0), egui::Align2::LEFT_BOTTOM, corner, font(7.0), GRAY);
    p.text(plot.left_top() + Vec2::new(2.0, 1.0), egui::Align2::LEFT_TOP, "BRIGHTER UP", font(7.0), GRAY);
}

/// SPECTRA column (sessions 6–8): 4 model graphs + MIR fingerprint graph +
/// rotation light-curve graph + composition chart + band IDs, docked
/// immediately right of the telemetry/data column.
pub fn draw_spectra_column(ui: &mut Ui, tel: Option<&Telemetry>, tips: bool) {
    let head = ui.max_rect();
    let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), egui::Sense::hover());
    let p = ui.painter().clone();
    p.text(hr.left_center() + Vec2::new(2.0, 0.0), egui::Align2::LEFT_CENTER,
        format!("SPECTRAL ANALYSIS — {}", tel.map(|t| t.name.clone()).unwrap_or_else(|| "---".into())),
        font(PX_SM), GRID_HI);
    let _ = head;
    egui::ScrollArea::vertical()
        .id_salt("spectra_scroll")
        .auto_shrink(false)
        .show(ui, |ui| {
            let Some(t) = tel else {
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 200.0), egui::Sense::hover());
                let r = ui.max_rect();
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "NO TARGET SELECTED", font(PX_SM), GRAY);
                return;
            };
            let f = crate::spectra::full_spectra(t);
            let w = ui.available_width();
            if t.kind == "PROBE" {
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 120.0), egui::Sense::hover());
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                    "ARTIFICIAL OBJECT —\nNO NATURAL SPECTRUM OR BULK COMPOSITION", font(PX_SM), GRAY);
                return;
            }
            // 4 model graphs: 2×2 when the column is wide, single column when
            // the user keeps it at half width (session-6 request)
            let cols = if w >= 340.0 { 2 } else { 1 };
            let gh = if cols == 2 { 138.0 } else { 112.0 };
            let gw = if cols == 2 { (w - 6.0) / 2.0 } else { w };
            let rows = 4 / cols;
            let (gr, gr_resp) = ui.allocate_exact_size(Vec2::new(w, gh * rows as f32 + 4.0 * (rows - 1) as f32), egui::Sense::hover());
            tip(gr_resp, tips,
                "Four model graphs: sunlight reflected off the body (visible & near-infrared),\nUV features, the body's own heat glow (mid-infrared),\nand the whole energy budget (SED). Dips = substances absorbing light.\nThe fifth graph below is the composition fingerprint (FTIR region),\nthe sixth the rotation light curve.");
            let p = ui.painter().clone();
            let rect_at = |k: usize| -> Rect {
                let cx = k % cols;
                let cy = k / cols;
                Rect::from_min_size(
                    Pos2::new(gr.left() + cx as f32 * (gw + 6.0), gr.top() + cy as f32 * (gh + 4.0)),
                    Vec2::new(gw, gh),
                )
            };
            spec_graph(&p, rect_at(0),
                &format!("VIS-NIR REFLECTANCE  {}", f.vis.note), &f.vis.pts,
                crate::spectra::LAM_MIN, crate::spectra::LAM_MAX, false, false, CYAN,
                &f.vis.bands, &f.vis.lines, &[0.5, 1.0, 1.5, 2.0, 2.5], "µm");
            spec_graph(&p, rect_at(1),
                &format!("UV-VIS  {}", f.uv.note), &f.uv.pts, 0.20, 0.90, false, false,
                Color32::from_rgb(160, 190, 255), &f.uv.bands, &f.uv.lines, &[0.3, 0.5, 0.7, 0.9], "µm");
            spec_graph(&p, rect_at(2),
                &format!("MIR THERMAL EMISSION T={:.0} K", f.mir_t), &f.mir_pts,
                2.0, 50.0, false, false, AMBER, &f.mir_bands, &[], &[5.0, 10.0, 20.0, 50.0], "µm");
            spec_graph(&p, rect_at(3),
                &format!("SED log-log  {}", f.sed_note), &f.sed_pts,
                0.316, 316.0, true, true, GREEN, &[], &[], &[1.0, 10.0, 100.0], "µm");
            // fifth graph (session 7): the MIR fingerprint region, full column
            // width — it carries far more features than the other four and the
            // reversed wavenumber axis needs the room to stay readable.
            let fph = if cols == 2 { 150.0 } else { 126.0 };
            let (fr, fr_resp) = ui.allocate_exact_size(Vec2::new(w, fph), egui::Sense::hover());
            tip(fr_resp, tips,
                "Fifth model graph — the MIR FINGERPRINT REGION (4000-400 cm-1, i.e. 2.5-25 µm,\nwavenumber axis reversed exactly like a real FTIR plot).\nEvery solid-state vibration of every mineral, ice, salt and organic in the body's\npublished composition drops a narrow line here, so the pattern is as distinctive\nas a fingerprint: no two compositions produce the same curve.\nMATCH scores the curve against the class archetype library by correlation; 100% means the\nbody's fingerprint is indistinguishable from that class. Stars and probes have no\nsolid surface, so they carry no fingerprint at all.");
            let pf = ui.painter().clone();
            fp_graph(&pf, fr, &f.fp);
            // sixth graph (session 8): rotation light curve, Fourier-synthesised
            let lch = if cols == 2 { 124.0 } else { 104.0 };
            let (lr, lr_resp) = ui.allocate_exact_size(Vec2::new(w, lch), egui::Sense::hover());
            tip(lr_resp, tips,
                "Sixth model graph — ROTATION LIGHT CURVE: the brightness wobble of the\nspinning body over one full rotation, Fourier-synthesised from the published\nrotation period plus per-class harmonic amplitudes (A1..A4 in the corner).\nElongated rocky bodies are double-peaked (2nd harmonic dominant); synchronous\nmoons barely wobble; comets wobble through coma jets. Δ magnitude, brighter up.");
            let pl2 = ui.painter().clone();
            lc_graph(&pl2, lr, &f.lc);
            // composition chart
            let comp = crate::spectra::composition(t);
            let (r3, r3_resp) = ui.allocate_exact_size(Vec2::new(w, 24.0 + comp.len().min(9) as f32 * 13.0 + 8.0), egui::Sense::hover());
            tip(r3_resp, tips, "What the body is made of, by mass. The bar shows each ingredient's share.");
            let p = ui.painter().clone();
            p.text(r3.left_top(), egui::Align2::LEFT_TOP, "BULK COMPOSITION (MASS FRACTION)", font(9.5), GRAY);
            let bar = Rect::from_min_size(r3.left_top() + Vec2::new(0.0, 13.0), Vec2::new(r3.width(), 14.0));
            let mut x = bar.left();
            for (_lab, fr, rgb) in comp.iter() {
                let bw = bar.width() * fr;
                let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                p.rect_filled(Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(bw, bar.height())), 0.0, c);
                p.rect_stroke(Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(bw, bar.height())), 0.0, Stroke1(BG), egui::StrokeKind::Outside);
                if bw > 26.0 {
                    p.text(Pos2::new(x + bw / 2.0, bar.center().y), egui::Align2::CENTER_CENTER, format!("{:.0}%", fr * 100.0), font(8.5), INK);
                }
                x += bw;
            }
            let mut y = bar.bottom() + 5.0;
            for (lab, fr, rgb) in comp.iter().take(9) {
                p.rect_filled(Rect::from_min_size(Pos2::new(r3.left(), y), Vec2::new(8.0, 8.0)), 0.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                p.text(Pos2::new(r3.left() + 12.0, y + 4.0), egui::Align2::LEFT_CENTER, format!("{lab}  {:.1}%", fr * 100.0), font(9.5), GRID);
                y += 13.0;
            }
            // band / line identifications + albedo
            let (r4, _) = ui.allocate_exact_size(Vec2::new(w, 14.0 + (f.vis.bands.len() + f.vis.lines.len()).min(10) as f32 * 11.0 + 26.0), egui::Sense::hover());
            let p = ui.painter().clone();
            p.text(r4.left_top(), egui::Align2::LEFT_TOP, "BAND / LINE IDENTIFICATIONS (VIS-NIR + UV)", font(9.5), GRAY);
            let mut y = r4.top() + 12.0;
            for (c, d, _, lab) in f.vis.bands.iter().take(7) {
                p.text(Pos2::new(r4.left(), y), egui::Align2::LEFT_TOP, format!("ABS {c:.3}µm d={d:.2}  {lab}"), font(8.5), CYAN.linear_multiply(0.8));
                y += 11.0;
            }
            for (c, s_, lab) in f.vis.lines.iter().take(3) {
                p.text(Pos2::new(r4.left(), y), egui::Align2::LEFT_TOP, format!("EMI {c:.3}µm s={s_:.2}  {lab}"), font(8.5), Color32::from_rgb(255, 120, 220).linear_multiply(0.8));
                y += 11.0;
            }
            if let Some(a) = f.vis.albedo {
                p.text(Pos2::new(r4.left(), y + 3.0), egui::Align2::LEFT_TOP, format!("GEOMETRIC ALBEDO {a:.3}", ), font(9.5), AMBER);
            }
            p.text(Pos2::new(r4.left(), y + 15.0), egui::Align2::LEFT_TOP,
                "ALL CURVES ARE DOCUMENTED MODELS (see BUILD_LOG §session 6), NOT MEASURED DATA", font(8.5), GRAY);
        });
}

// ----------------------------------------------------------- left column ----
/// Mutable handles for the DATA LINK block (R14/R15).
pub struct LinkUi<'a> {
    pub key: &'a mut String,
    pub store: &'a mut bool,
    pub show: &'a mut bool,
    pub bucket: std::sync::Arc<std::sync::Mutex<crate::rate_limiter::TokenBucket>>,
    pub link: std::sync::Arc<std::sync::Mutex<crate::online::LinkState>>,
}

/// Left column heights (content space) for the manual scroll math.
/// Session 4: RESIDUAL LOG + DATA LINK moved to the RIGHT column (with the
/// enlarged sky map on top); SELECTED OBJECT grew its physical-property rows.
const LC_H: [f32; 3] = [560.0, 180.0, 210.0];
const LC_GAP: f32 = 8.0;

pub fn draw_left_column(ui: &mut Ui, st: &mut AppState, tel: Option<&Telemetry>, jd: f64, scope_cache: &mut Option<crate::app_state::ScopeCache>) {
    // Deterministic absolute layout with own wheel scroll: egui's ScrollArea
    // clamps tall cells to the viewport (clipped DATA LINK), so the column is
    // hand-stacked like a real HMI rack (M4.5 fix).
    let col = ui.max_rect();
    let content_h: f32 = LC_H.iter().sum::<f32>() + LC_GAP * (LC_H.len() as f32 - 1.0);
    let hover_col = ui.rect_contains_pointer(col);
    if hover_col {
        let dy = ui.input(|i| i.raw_scroll_delta.y);
        st.left_scroll = (st.left_scroll - dy * 0.6).clamp(0.0, (content_h - col.height()).max(0.0));
    }
    let mut y = col.top() - st.left_scroll;
    let mut rect_at = |h: f32| -> Rect {
        let r = Rect::from_min_max(Pos2::new(col.left(), y), Pos2::new(col.right(), y + h));
        y += h + LC_GAP;
        r
    };
    let mut p = ui.painter().clone();
    p.set_clip_rect(col);

    let r = rect_at(LC_H[0]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    cell_title(&p, r, "SELECTED OBJECT");
    selected_object_cell(ui, st, tel, r);

    let r = rect_at(LC_H[1]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    cell_title(&p, r, "RAW DATA");
    raw_data_cell(ui, tel, r);

    let r = rect_at(LC_H[2]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    cell_title(&p, r, "SCOPE: r(t)");
    scope_cell(ui, st, tel, jd, r, scope_cache);

    // scroll position indicator (right edge ticks)
    if content_h > col.height() {
        let f = st.left_scroll / (content_h - col.height());
        let th = (col.height() / content_h) * col.height();
        let ty = col.top() + f * (col.height() - th);
        p.rect_filled(Rect::from_min_size(Pos2::new(col.right() - 3.0, ty), Vec2::new(3.0, th)), 0.0, GRID);
    }
}


// ---------------------------------------------------------- right column ----
/// Right column heights — SKY MAP is the big cell on top (session 4 layout
/// request), with RESIDUAL LOG and DATA LINK moved over from the left rack.
const RC_H: [f32; 4] = [384.0, 190.0, 252.0, 92.0];

pub fn draw_right_column(ui: &mut Ui, st: &mut AppState, cat: &crate::catalog::Catalog, link: &mut LinkUi) {
    // same hand-stacked rack as the left column, own wheel scroll
    let col = ui.max_rect();
    let content_h: f32 = RC_H.iter().sum::<f32>() + LC_GAP * (RC_H.len() as f32 - 1.0);
    let hover_col = ui.rect_contains_pointer(col);
    if hover_col {
        let dy = ui.input(|i| i.raw_scroll_delta.y);
        st.right_scroll = (st.right_scroll - dy * 0.6).clamp(0.0, (content_h - col.height()).max(0.0));
    }
    let mut y = col.top() - st.right_scroll;
    let mut rect_at = |h: f32| -> Rect {
        let r = Rect::from_min_max(Pos2::new(col.left(), y), Pos2::new(col.right(), y + h));
        y += h + LC_GAP;
        r
    };
    let mut p = ui.painter().clone();
    p.set_clip_rect(col);

    let r = rect_at(RC_H[0]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    cell_title(&p, r, "SKY MAP — RA/DEC J2000");
    sky_map_cell(ui, st, cat, r);

    let r = rect_at(RC_H[1]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    cell_title(&p, r, "RESIDUAL LOG vs HORIZONS (FROZEN 2026-10-09)");
    resid_cell(&p, r);

    let r = rect_at(RC_H[2]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    cell_title(&p, r, "DATA LINK");
    data_link_cell(ui, st, link, r);

    let r = rect_at(RC_H[3]);
    paint_cell(&p, r, CELL, STROKE_GRID);
    accuracy_footer(r, &p, st);

    // scroll position indicator (right edge ticks)
    if content_h > col.height() {
        let f = st.right_scroll / (content_h - col.height());
        let th = (col.height() / content_h) * col.height();
        let ty = col.top() + f * (col.height() - th);
        p.rect_filled(Rect::from_min_size(Pos2::new(col.right() - 3.0, ty), Vec2::new(3.0, th)), 0.0, GRID);
    }
}

/// Big interactive sky map (session 4 — replaces the 160×130 viewport minimap):
/// catalog host stars by RA/Dec on a graticule with the ecliptic drawn for
/// orientation. Hover a star for its tag; click to jump to that system; the
/// ⊕ SOL chip returns to the Solar System.
fn sky_map_cell(ui: &mut Ui, st: &mut AppState, cat: &crate::catalog::Catalog, rect: Rect) {
    let p = ui.painter().clone();
    let field = Rect::from_min_max(rect.left_top() + Vec2::new(8.0, 30.0), rect.right_bottom() - Vec2::new(8.0, 18.0));
    p.rect_filled(field, 0.0, BG);
    p.rect_stroke(field, 0.0, egui::Stroke::new(1.0_f32, ROW_LINE), egui::StrokeKind::Outside);

    let pos_of = |ra: f64, dec: f64| -> Pos2 {
        Pos2::new(
            egui::lerp(field.left()..=field.right(), (ra / 360.0) as f32),
            egui::lerp(field.top()..=field.bottom(), ((90.0 - dec) / 180.0) as f32),
        )
    };

    // graticule: RA every 4 h, Dec every 45°
    for k in 1..6 {
        let x = egui::lerp(field.left()..=field.right(), k as f32 / 6.0);
        p.line_segment([Pos2::new(x, field.top()), Pos2::new(x, field.bottom())], egui::Stroke::new(1.0_f32, PLOT_DOT));
    }
    for k in 1..4 {
        let yy = egui::lerp(field.top()..=field.bottom(), k as f32 / 4.0);
        p.line_segment([Pos2::new(field.left(), yy), Pos2::new(field.right(), yy)], egui::Stroke::new(1.0_f32, PLOT_DOT));
    }
    // axis furniture
    for k in 0..=6 {
        let x = egui::lerp(field.left()..=field.right(), k as f32 / 6.0);
        p.text(Pos2::new(x, field.bottom() + 2.0), egui::Align2::CENTER_TOP, format!("{}H", k * 4), font(11.0), GRAY);
    }
    p.text(field.left_top() + Vec2::new(2.0, 0.0), egui::Align2::LEFT_TOP, "+90", font(11.0), GRAY);
    p.text(Pos2::new(field.left() + 2.0, field.center().y), egui::Align2::LEFT_CENTER, " 0", font(11.0), GRAY);
    p.text(field.left_bottom() + Vec2::new(2.0, -13.0), egui::Align2::LEFT_TOP, "−90", font(11.0), GRAY);

    // the ecliptic, as an RA/Dec sine curve (orientation for planet-hosting stars)
    let eps = 23.439_291_1f64.to_radians();
    let mut seg: Vec<Pos2> = Vec::new();
    let mut last_x = f32::NAN;
    for k in 0..=360u32 {
        let lam = k as f64 * std::f64::consts::TAU / 360.0;
        let dec = (eps.sin() * lam.sin()).asin().to_degrees();
        let ra = ((eps.cos() * lam.sin()).atan2(lam.cos())).to_degrees().rem_euclid(360.0);
        let pt = pos_of(ra, dec);
        if !last_x.is_nan() && (pt.x - last_x).abs() > field.width() * 0.5 {
            if seg.len() > 1 {
                p.add(egui::Shape::line(std::mem::take(&mut seg), egui::Stroke::new(1.0_f32, AMBER.linear_multiply(0.35))));
            }
            seg.clear();
        }
        seg.push(pt);
        last_x = pt.x;
    }
    if seg.len() > 1 {
        p.add(egui::Shape::line(seg, egui::Stroke::new(1.0_f32, AMBER.linear_multiply(0.35))));
    }

    // host stars
    let ptr = ui.ctx().pointer_hover_pos().filter(|h| field.contains(*h));
    let mut hovered: Option<usize> = None;
    let mut hdist = f32::MAX;
    for (i, s) in cat.systems.iter().enumerate() {
        let (Some(ra), Some(dec)) = (s.host.ra_deg, s.host.dec_deg) else { continue };
        let sp = pos_of(ra, dec);
        let active = st.system == i + 1;
        let mag = s.host.vmag.unwrap_or(12.0);
        let r = ((4.6 - 0.3 * mag).clamp(1.2, 3.4) + if active { 0.8 } else { 0.0 }) as f32;
        p.circle_filled(sp, r, if active { CYAN } else { Color32::from_gray(150) });
        if active {
            p.circle_stroke(sp, r + 4.0, egui::Stroke::new(1.0_f32, CYAN));
        }
        // strip index (matches the 01..10 selector keys at the bottom)
        p.text(sp + Vec2::new(5.0, 4.0), egui::Align2::LEFT_TOP, format!("{:02}", i + 1), font(10.0),
            if active { CYAN } else { Color32::from_gray(90) });
        if let Some(h) = ptr {
            let d = (h - sp).length();
            if d < 12.0 && d < hdist {
                hdist = d;
                hovered = Some(i);
            }
        }
    }

    // hover tag
    if let Some(i) = hovered {
        let s = &cat.systems[i];
        if let (Some(ra), Some(dec)) = (s.host.ra_deg, s.host.dec_deg) {
            let sp = pos_of(ra, dec);
            p.circle_stroke(sp, 8.0, egui::Stroke::new(1.0_f32, GRID_HI));
            let label = format!("{} · {} LY · KEY {:02}", s.host.name,
                s.host.dist_ly.map(|d| format!("{d:.0}")).unwrap_or_else(|| "--".into()), i + 1);
            let w = (12.0 + label.len() as f32 * 6.4).min(field.width() - 4.0);
            let mut tr = Rect::from_min_size(sp + Vec2::new(10.0, -22.0), Vec2::new(w, 16.0));
            if tr.right() > field.right() {
                tr = Rect::from_min_size(Pos2::new(sp.x - 10.0 - w, tr.top()), Vec2::new(w, 16.0));
            }
            p.rect_filled(tr, 0.0, CELL);
            p.rect_stroke(tr, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
            p.text(tr.center(), egui::Align2::CENTER_CENTER, label, font(PX_SM), GRID_HI);
        }
    }

    // ⊕ SOL observer chip — click returns to the Solar System
    let chip = Rect::from_min_size(field.left_top() + Vec2::new(4.0, 4.0), Vec2::new(100.0, 18.0));
    let sol_active = st.system == 0;
    p.rect_filled(chip, 0.0, if sol_active { CYAN } else { CELL });
    p.rect_stroke(chip, 0.0, if sol_active { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
    p.text(chip.center(), egui::Align2::CENTER_CENTER, "SOL (OBSERVER)", font(PX_SM), if sol_active { INK } else { GRAY });
    let sol_hit = ptr.map(|h| chip.contains(h)).unwrap_or(false);

    // click = jump to system
    let resp = tip(ui.interact(field, ui.id().with("skymap_field"), egui::Sense::click()), st.tips,
        "Map of the whole sky. Each dot is a star with planets — click it to travel there.");
    if resp.clicked() {
        if let Some(i) = hovered {
            st.system = i + 1;
        } else if sol_hit {
            st.system = 0;
        }
    }
    p.text(rect.right_top() + Vec2::new(-8.0, 6.0), egui::Align2::RIGHT_TOP,
        "CLICK=JUMP · AMBER=ECLIPTIC", font(11.0), Color32::from_gray(70));
}

fn cell_title(p: &egui::Painter, r: Rect, title: &str) {
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

/// Format helpers for the physical-property rows (session 4).
fn fmt_period(d: f64) -> String {
    if !(d > 0.0) {
        "---".into()
    } else if d < 1.0 {
        format!("{:.1} H", d * 24.0)
    } else if d < 1000.0 {
        format!("{:.3} D", d)
    } else {
        format!("{:.2} YR", d / 365.25)
    }
}

fn fmt_rot(d: f64, note: &str) -> String {
    let mag = d.abs();
    let s = if mag < 1.0 {
        format!("{:.1} H", mag * 24.0)
    } else {
        format!("{:.2} D", mag)
    };
    let retro = if d < 0.0 { " RETRO" } else { "" };
    let note = if note.is_empty() { "" } else {
        if note == "SYNC" { " SYNC" } else { " " }
    };
    format!("{s}{retro}{note}")
}

fn fmt_radius(km: f64) -> String {
    fn grouped(v: f64) -> String {
        let s = format!("{:.0}", v);
        let bytes = s.as_bytes();
        let mut out = String::new();
        for (i, b) in bytes.iter().enumerate() {
            if i > 0 && (bytes.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(*b as char);
        }
        out
    }
    if km <= 0.0 {
        "---".into()
    } else if km >= 100.0 {
        format!("{} KM", grouped(km))
    } else {
        format!("{:.1} KM", km)
    }
}

fn selected_object_cell(ui: &mut Ui, st: &AppState, tel: Option<&Telemetry>, rect: Rect) {
    let p = ui.painter().clone();
    let inner = Rect::from_min_max(rect.left_top() + Vec2::new(8.0, 30.0), rect.right_bottom() - Vec2::new(8.0, 8.0));
    let name = tel.map(|t| t.name.clone()).unwrap_or_else(|| "---".into());
    let banner = Rect::from_min_size(inner.left_top(), Vec2::new(inner.width(), 26.0));
    p.rect_filled(banner, 0.0, if tel.is_some() { CYAN } else { CELL });
    p.rect_stroke(banner, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(banner.center(), egui::Align2::CENTER_CENTER, name, font(PX), if tel.is_some() { INK } else { GRAY });
    let mut ry = banner.bottom() + 6.0;
    let row = |l: &str, v: &str, c: Color32, p: &egui::Painter, ry: f32| {
        let rr = Rect::from_min_max(Pos2::new(inner.left(), ry), Pos2::new(inner.right(), ry + 22.0));
        p.rect_stroke(rr, 0.0, egui::Stroke::new(1.0_f32, ROW_LINE), egui::StrokeKind::Outside);
        p.text(rr.left_center() + Vec2::new(6.0, 0.0), egui::Align2::LEFT_CENTER, l, font(PX_SM), GRAY);
        p.text(rr.right_center() - Vec2::new(6.0, 0.0), egui::Align2::RIGHT_CENTER, v, font(PX_SM), c);
    };
    if let Some(t) = tel {
        if t.kind == "PROBE" {
            // spacecraft readout (session 5): mission facts + live state
            let status_col = if t.class.starts_with("ACTIVE") { GREEN } else { AMBER };
            row("TYPE", "PROBE (SPACECRAFT)", GRID_HI, &p, ry); ry += 24.0;
            row("STATUS", &t.class, status_col, &p, ry); ry += 24.0;
            row("AGENCY", t.agency, GRID_HI, &p, ry); ry += 24.0;
            row("LAUNCH", &t.launched.to_string(), GRID_HI, &p, ry); ry += 24.0;
            row("MISSION", t.mission, CYAN, &p, ry); ry += 24.0;
            row("PROPAGATION", t.status, GRAY, &p, ry); ry += 24.0;
            row("DIST-STAR", &format!("{:.4} AU", t.dist_star_au), CYAN, &p, ry); ry += 24.0;
            row("VEL", &format!("{:.2} km/s", t.vel_kms), CYAN, &p, ry); ry += 24.0;
            row("DIST-GEO", &format!("{:.4} AU", t.dist_geo_au), GREEN, &p, ry); ry += 24.0;
            row("ORBIT PERIOD", &fmt_period(t.period_d), CYAN, &p, ry); ry += 24.0;
            if t.el.a.is_finite() && t.el.a > 0.0 {
                row("A", &format!("{:.5} AU", t.el.a), GRID_HI, &p, ry); ry += 24.0;
                row("E", &format!("{:.5}", t.el.e), GRID_HI, &p, ry); ry += 24.0;
                row("I", &format!("{:.3} deg", t.el.i), GRID_HI, &p, ry); ry += 24.0;
            } else {
                row("ORBIT", "HYPERBOLIC — SOLAR ESCAPE", AMBER, &p, ry); ry += 24.0;
            }
        } else {
        let kind = if t.class.is_empty() { t.kind.to_string() } else { format!("{} · {}", t.kind, t.class) };
        row("TYPE", &kind, GRID_HI, &p, ry); ry += 24.0;
        if t.pha {
            row("IMPACT RISK", "PHA — EARTH-CROSSING", ALARM, &p, ry); ry += 24.0;
        } else if t.neo {
            row("NEO STATUS", "NEAR-EARTH OBJECT", AMBER, &p, ry); ry += 24.0;
        }
        let mass_s = if t.mass_kg > 0.0 { format!("{:.3e} kg", t.mass_kg) } else { "---".to_string() };
        row("MASS", &mass_s, GREEN, &p, ry); ry += 24.0;
        row("RADIUS", &fmt_radius(t.radius_km), GRID_HI, &p, ry); ry += 24.0;
        let temp_s = match t.temp_k {
            Some(k) => format!("{:.0} K · {}", k, t.temp_note),
            None => "---".to_string(),
        };
        row("TEMP", &temp_s, AMBER, &p, ry); ry += 24.0;
        row("COMPOSITION", t.composition, GRID_HI, &p, ry); ry += 24.0;
        let rot_s = match t.rot_d {
            Some(d) => fmt_rot(d, t.rot_note),
            None => "--- (UNK)".to_string(),
        };
        row("ROTATION", &rot_s, CYAN, &p, ry); ry += 24.0;
        row("ORBIT PERIOD", &fmt_period(t.period_d), CYAN, &p, ry); ry += 24.0;
        let age_s = match t.age_gyr {
            Some(a) => format!("{:.2} GYR", a),
            None => "---".to_string(),
        };
        row("AGE", &age_s, GRID_HI, &p, ry); ry += 24.0;
        row("DIST-STAR", &format!("{:.4} AU", t.dist_star_au), CYAN, &p, ry); ry += 24.0;
        row("VEL", &format!("{:.2} km/s", t.vel_kms), CYAN, &p, ry); ry += 24.0;
        if t.kind == "EXOPLANET" {
            row("DIST-GEO", "---", GRAY, &p, ry);
        } else {
            row("DIST-GEO", &format!("{:.4} AU", t.dist_geo_au), GREEN, &p, ry);
        }
        ry += 24.0;
        row("PHASE", &format!("{:.1} deg", t.phase_deg), AMBER, &p, ry); ry += 24.0;
        if t.kind == "PLANET" || t.kind == "DWARF PLANET" {
            let rendered = orrery_astro::moons::moon_defs().iter().filter(|m| m.parent == crate::telemetry::id_from_name(&t.name).unwrap_or(orrery_astro::planets::PlanetId::Mercury)).count();
            let known = crate::telemetry::KNOWN_MOONS[crate::telemetry::planet_index(crate::telemetry::id_from_name(&t.name).unwrap_or(orrery_astro::planets::PlanetId::Mercury))];
            row("MOONS", &format!("{rendered} RENDERED / {known} KNOWN"), CYAN, &p, ry);
            ry += 24.0;
        }
        row("A", &format!("{:.5} AU", t.el.a), GRID_HI, &p, ry); ry += 24.0;
        row("E", &format!("{:.5}", t.el.e), GRID_HI, &p, ry); ry += 24.0;
        row("I", &format!("{:.3} deg", t.el.i), GRID_HI, &p, ry); ry += 24.0;
        if let (Some(da), Some(de)) = (t.a_err, t.e_err) {
            row("±A", &format!("{:.1e} AU", da), AMBER, &p, ry); ry += 24.0;
            row("±E", &format!("{:.1e}", de), AMBER, &p, ry);
        }
        }
    } else {
        for (l, v) in [("TYPE", "---"), ("MASS", "---"), ("RADIUS", "---"), ("TEMP", "---"),
                       ("COMPOSITION", "---"), ("ROTATION", "---"), ("ORBIT PERIOD", "---"),
                       ("AGE", "---"), ("DIST-STAR", "---"), ("VEL", "---"),
                       ("DIST-GEO", "---"), ("PHASE", "---"), ("A", "---"), ("E", "---"), ("I", "---")] {
            row(l, v, GRAY, &p, ry);
            ry += 24.0;
        }
        if st.system != 0 {
            row("NOTE", "EXO CATALOG: M4", AMBER, &p, ry);
        }
    }
}

fn raw_data_cell(_ui: &mut Ui, tel: Option<&Telemetry>, rect: Rect) {
    let p = _ui.painter().clone();
    let mut y = rect.top() + 32.0;
    let f = |v: f64| format!("{v:+.9e}");
    let lines = match tel {
        Some(t) => vec![
            format!("X  {} AU", f(t.pos_icrs[0])),
            format!("Y  {} AU", f(t.pos_icrs[1])),
            format!("Z  {} AU", f(t.pos_icrs[2])),
            format!("VX {} AU/D", f(t.vel_icrs[0])),
            format!("VY {} AU/D", f(t.vel_icrs[1])),
            format!("VZ {} AU/D", f(t.vel_icrs[2])),
        ],
        None => vec!["X  ---".into(), "Y  ---".into(), "Z  ---".into(),
                     "VX ---".into(), "VY ---".into(), "VZ ---".into()],
    };
    for line in lines {
        p.text(Pos2::new(rect.left() + 10.0, y), egui::Align2::LEFT_TOP, line, font(PX_SM), GRID_HI);
        y += 17.0;
    }
    p.text(Pos2::new(rect.left() + 10.0, y), egui::Align2::LEFT_TOP, "PROV: VSOP87 v3.0 FULL", font(PX_SM), GRAY);
    y += 16.0;
    let theory = match tel.map(|t| t.kind) {
        Some("EXOPLANET") => "KEPLER-2B FROM PUBLISHED ELTS",
        Some("MOON") => "MEAN-CIRCLE (HORIZONS-DERIVED)",
        Some("COMET") | Some("ASTEROID") => "SBDB OSC + KEPLER-2B",
        _ => "VSOP87 (ICRS J2000)",
    };
    p.text(Pos2::new(rect.right() - 10.0, rect.bottom() - 22.0), egui::Align2::RIGHT_TOP,
        format!("THEORY: {theory}"), font(PX_SM), GRAY);
}

fn scope_cell(_ui: &mut Ui, st: &mut AppState, tel: Option<&Telemetry>, jd: f64, rect: Rect, scope_cache: &mut Option<crate::app_state::ScopeCache>) {
    let p = _ui.painter().clone();
    // plot selector tabs (§10.3): r(t) | |v|(t) | phase(t)
    for (i, lab) in ["R(T)", "|V|(T)", "PH(T)"].iter().enumerate() {
        let tr = Rect::from_min_size(Pos2::new(rect.right() - 168.0 + i as f32 * 54.0, rect.top() + 4.0), Vec2::new(50.0, 16.0));
        let act = st.plot_mode == i as u8;
        p.rect_filled(tr, 0.0, if act { CYAN } else { CELL });
        p.rect_stroke(tr, 0.0, if act { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
        p.text(tr.center(), egui::Align2::CENTER_CENTER, *lab, font(PX_SM), if act { INK } else { GRAY });
        let ptip = ["Distance of the body from the Sun over one full orbit.",
            "How fast the body travels along its orbit over one orbit.",
            "Sun–body–Earth angle over one orbit (0° = behind the Sun)."][i];
        if tip(_ui.interact(tr, _ui.id().with(("plot", i)), egui::Sense::click()), st.tips, ptip).clicked() {
            st.plot_mode = i as u8;
        }
    }
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(10.0, 30.0), rect.right_bottom() - Vec2::new(96.0, 10.0));
    let inner = scope_frame(&p, plot, 5);
    // periodic scope needs a finite, positive period — the Sun (selectable
    // session 4) has none, so it gets an honest message instead of NaN traces
    let period_of = |t: &Telemetry| -> f64 {
        crate::telemetry::id_from_name(&t.name)
            .map(period_days)
            .unwrap_or_else(|| t.el.period_days())
    };
    if let Some(t) = tel.filter(|t| period_of(t).is_finite() && period_of(t) > 1e-6) {
        {
            let per = period_of(t);
            let n = 121usize;
            // Session-5 perf fix ("animation slows down when a planet is
            // selected"): (a) samples come from the CHEAP evaluator — one
            // position-only VSOP call per sample instead of a full telemetry
            // build (~15 full-series evaluations); (b) the sliding window is
            // quantised to period/64; (c) re-bakes are wall-clock throttled to
            // ≤ ~7/s, so MAX-rate time flow can no longer storm the ephemeris.
            let step = per / 64.0;
            let jd_q = (jd / step).floor() * step;
            let mine = scope_cache.as_ref().filter(|c| c.key == t.name && c.mode == st.plot_mode);
            let usable = mine.filter(|c| {
                (c.jd_q - jd_q).abs() < 1e-9
                    || c.baked.elapsed() < std::time::Duration::from_millis(150)
            });
            let (vals, rmin, rmax) = match usable {
                Some(c) => (c.vals.clone(), c.rmin, c.rmax),
                None => {
                    let fallback = if t.kind == "MOON" { t.dist_parent_au } else { t.dist_star_au };
                    let (mut rmin, mut rmax) = (f64::INFINITY, f64::NEG_INFINITY);
                    let vals: Vec<f64> = (0..n)
                        .map(|k| {
                            let tt = jd_q - per / 2.0 + per * (k as f64 / (n as f64 - 1.0));
                            let r = crate::telemetry::sample_plot(&t.name, tt, st.plot_mode, fallback);
                            rmin = rmin.min(r);
                            rmax = rmax.max(r);
                            r
                        })
                        .collect();
                    *scope_cache = Some(crate::app_state::ScopeCache {
                        key: t.name.clone(), mode: st.plot_mode, jd_q, vals: vals.clone(), rmin, rmax,
                        baked: std::time::Instant::now(),
                    });
                    (vals, rmin, rmax)
                }
            };
            let span = (rmax - rmin).max(1e-9);
            let pts: Vec<Pos2> = vals
                .iter()
                .enumerate()
                .map(|(k, r)| {
                    let fx = k as f32 / (n as f32 - 1.0);
                    let fy = ((r - rmin) / span * 0.84 + 0.08) as f32;
                    Pos2::new(egui::lerp(inner.left()..=inner.right(), fx), egui::lerp(inner.bottom()..=inner.top(), fy))
                })
                .collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.5_f32, CYAN)));
            let vx = rect.right() - 92.0;
            let mut y = plot.top() + 4.0;
            let rnow = if t.kind == "MOON" { t.dist_parent_au } else { t.dist_star_au };
            let rows: [(&str, String, Color32); 4] = [
                ("R", format!("{:.4}", rnow), CYAN),
                ("V", format!("{:.2}", t.vel_kms), AMBER),
                ("RN", format!("{:.3}", rmin), GRID_HI),
                ("RX", format!("{:.3}", rmax), GREEN),
            ];
            for (l, v, c) in rows {
                p.text(Pos2::new(vx, y), egui::Align2::LEFT_TOP, l, font(PX_SM), GRAY);
                p.text(Pos2::new(rect.right() - 6.0, y), egui::Align2::RIGHT_TOP, v, font(PX_SM), c);
                y += 20.0;
            }
        }
    } else {
        let msg = if tel.is_some() { "NO PERIODIC SCOPE (STAR)" } else { "NO TARGET" };
        p.text(inner.center(), egui::Align2::CENTER_CENTER, msg, font(PX_SM), GRAY);
    }
}

/// Residual log (§10.3): worst-case |our theory − Horizons| per body, arcsec,
/// log-scaled bars with the 1″ acceptance line (M1 cap) marked.
fn resid_cell(p: &egui::Painter, rect: Rect) {
    static RESID: std::sync::OnceLock<Vec<(String, f64)>> = std::sync::OnceLock::new();
    let res = RESID.get_or_init(orrery_astro::validation_residuals);
    let inner = Rect::from_min_max(rect.left_top() + Vec2::new(10.0, 30.0), rect.right_bottom() - Vec2::new(10.0, 40.0));
    let bw = inner.width() / res.len() as f32;
    // log scale: 0.01″..100″
    let y_of = |v: f64| {
        let l = (v.max(0.005).log10() + 2.3) / 4.3; // -2.3..2.0 → 0..1
        egui::lerp(inner.bottom()..=inner.top(), (l as f32).clamp(0.0, 1.0))
    };
    // 1″ acceptance line
    let y1 = y_of(1.0);
    p.line_segment([Pos2::new(inner.left(), y1), Pos2::new(inner.right(), y1)], egui::Stroke::new(1.0_f32, AMBER));
    p.text(Pos2::new(inner.right(), y1 - 3.0), egui::Align2::RIGHT_BOTTOM, "1\" CAP", font(PX_SM), AMBER);
    for (i, (name, v)) in res.iter().enumerate() {
        let x = inner.left() + i as f32 * bw;
        let col = if *v < 1.0 { GREEN } else if *v < 2.5 { AMBER } else { GRAY };
        let bar = Rect::from_min_max(Pos2::new(x + bw * 0.25, y_of(*v)), Pos2::new(x + bw * 0.75, inner.bottom()));
        p.rect_filled(bar, 0.0, col);
        let lab = if *v >= 10.0 { format!("{v:.0}") } else { format!("{v:.1}") };
        p.text(Pos2::new(x + bw / 2.0, inner.bottom() + 2.0), egui::Align2::CENTER_TOP,
            name.chars().take(3).collect::<String>(), font(11.0), GRAY);
        p.text(Pos2::new(x + bw / 2.0, y_of(*v) - 2.0), egui::Align2::CENTER_BOTTOM, lab, font(11.0), col);
    }
    p.line_segment([inner.left_bottom(), inner.right_bottom()], STROKE_HI);
    p.text(Pos2::new(inner.left(), inner.bottom() + 16.0), egui::Align2::LEFT_TOP,
        "ARCSEC, LOG SCALE", font(11.0), GRAY);
    p.text(Pos2::new(inner.left(), inner.bottom() + 28.0), egui::Align2::LEFT_TOP,
        "U/N BAR = DE200 FIT; PLU SPAN-LIMITED", font(11.0), GRAY);
}

fn data_link_cell(ui: &mut Ui, st: &mut AppState, link: &mut LinkUi, rect: Rect) {
    use crate::online::SvcStatus;
    let p = ui.painter().clone();
    let x0 = rect.left() + 10.0;
    let mut y = rect.top() + 32.0;
    let row_h = 22.0;

    p.text(Pos2::new(x0, y + 4.0), egui::Align2::LEFT_TOP, "LINK", font(PX_SM), GRAY);
    let lk = Rect::from_min_size(Pos2::new(x0 + 70.0, y), Vec2::new(46.0, 18.0));
    p.rect_filled(lk, 0.0, if st.link_on { CYAN } else { CELL });
    p.rect_stroke(lk, 0.0, if st.link_on { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
    p.text(lk.center(), egui::Align2::CENTER_CENTER, if st.link_on { "ON" } else { "OFF" }, font(PX_SM), if st.link_on { INK } else { GRAY });
    if ui.interact(lk, ui.id().with("link_on"), egui::Sense::click()).clicked() {
        st.link_on = !st.link_on;
    }
    y += row_h;

    p.text(Pos2::new(x0, y + 4.0), egui::Align2::LEFT_TOP, "API KEY", font(PX_SM), GRAY);
    let kr = Rect::from_min_size(Pos2::new(x0 + 70.0, y), Vec2::new(150.0, 18.0));
    p.rect_filled(kr, 0.0, BG);
    p.rect_stroke(kr, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    let key_empty = link.key.is_empty();
    ui.put(
        kr,
        egui::TextEdit::singleline(link.key)
            .password(!*link.show)
            .font(font(PX_SM))
            .text_color(if key_empty { AMBER } else { GREEN })
            .desired_width(140.0),
    );
    let sh = Rect::from_min_size(Pos2::new(kr.right() + 6.0, y), Vec2::new(44.0, 18.0));
    p.rect_filled(sh, 0.0, if *link.show { CYAN } else { CELL });
    p.rect_stroke(sh, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(sh.center(), egui::Align2::CENTER_CENTER, "SHOW", font(PX_SM), if *link.show { INK } else { GRAY });
    if ui.interact(sh, ui.id().with("key_show"), egui::Sense::click()).clicked() {
        *link.show = !*link.show;
    }
    st.key_set = !link.key.is_empty();
    if *link.store && st.key_set {
        let _ = crate::key_store::store(link.key);
    }
    y += row_h;

    p.text(Pos2::new(x0, y + 4.0), egui::Align2::LEFT_TOP, "CPM", font(PX_SM), GRAY);
    let minus = Rect::from_min_size(Pos2::new(x0 + 70.0, y), Vec2::new(24.0, 18.0));
    let plus = Rect::from_min_size(Pos2::new(x0 + 100.0, y), Vec2::new(24.0, 18.0));
    for (r, id, d) in [(minus, "cpm-", -5i64), (plus, "cpm+", 5)] {
        p.rect_filled(r, 0.0, CELL);
        p.rect_stroke(r, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, if d < 0 { "-" } else { "+" }, font(PX_SM), GRID_HI);
        if tip(ui.interact(r, ui.id().with(id), egui::Sense::click()), st.tips,
            "How many online requests per minute the app is allowed to send.")
            .clicked()
        {
            st.cpm = (st.cpm as i64 + d).clamp(1, 120) as u32;
            link.bucket.lock().unwrap().set_cpm(st.cpm, 0.0);
        }
    }
    p.text(Pos2::new(x0 + 132.0, y + 4.0), egui::Align2::LEFT_TOP, st.cpm.to_string(), font(PX_SM), GRID_HI);
    y += row_h;

    p.text(Pos2::new(x0, y + 4.0), egui::Align2::LEFT_TOP, "STORE", font(PX_SM), GRAY);
    let stt = Rect::from_min_size(Pos2::new(x0 + 70.0, y), Vec2::new(46.0, 18.0));
    p.rect_filled(stt, 0.0, if *link.store { CYAN } else { CELL });
    p.rect_stroke(stt, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    p.text(stt.center(), egui::Align2::CENTER_CENTER, if *link.store { "YES" } else { "NO" }, font(PX_SM), if *link.store { INK } else { GRAY });
    if tip(ui.interact(stt, ui.id().with("store"), egui::Sense::click()), st.tips,
        "Remember my JPL API key on this computer (or forget it).")
        .clicked()
    {
        *link.store = !*link.store;
        if !*link.store {
            crate::key_store::clear();
        }
    }
    y += row_h + 4.0;

    let tr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(52.0, 20.0));
    let enabled = st.link_on;
    p.rect_filled(tr, 0.0, if enabled { CELL } else { CELL_DIM });
    p.rect_stroke(tr, 0.0, if enabled { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
    p.text(tr.center(), egui::Align2::CENTER_CENTER, "TEST", font(PX_SM), if enabled { GRID_HI } else { GRAY });
    if enabled && tip(ui.interact(tr, ui.id().with("link_test"), egui::Sense::click()), st.tips,
        "Ping the JPL services now to check whether the online link works.")
        .clicked()
    {
        let key = if st.key_set { Some(link.key.clone()) } else { None };
        crate::online::run_test_suite(key, link.bucket.clone(), link.link.clone());
    }
    let last = link.link.lock().unwrap().last_test.clone();
    p.text(Pos2::new(x0 + 62.0, y + 4.0), egui::Align2::LEFT_TOP, format!("LAST: {last}"), font(PX_SM), GRAY);
    y += row_h + 2.0;

    let ls = link.link.lock().unwrap();
    let rows: [(&str, &SvcStatus); 3] = [("HOR", &ls.horizons), ("EXO", &ls.exo), ("NEO", &ls.neo)];
    let mut x = x0;
    for (name, stt) in rows {
        let col = match stt {
            SvcStatus::Ok(_) => GREEN,
            SvcStatus::Wait | SvcStatus::NoKey => AMBER,
            SvcStatus::Fail(_) => ALARM,
            SvcStatus::Idle => GRAY,
        };
        let lab = match stt {
            SvcStatus::Ok(_) => "OK".to_string(),
            other => other.label(),
        };
        p.text(Pos2::new(x, y + 2.0), egui::Align2::LEFT_TOP, format!("{name}:{lab}"), font(PX_SM), col);
        x += 118.0;
    }
    drop(ls);
    y += 18.0;
    let b = link.bucket.lock().unwrap();
    let frac = b.gauge_frac() as f32;
    let cpm = b.cpm as u32;
    let toks = b.remaining();
    drop(b);
    p.text(Pos2::new(x0, y + 2.0), egui::Align2::LEFT_TOP, "BUCKET", font(PX_SM), GRAY);
    let g = Rect::from_min_size(Pos2::new(x0 + 70.0, y + 2.0), Vec2::new(rect.width() - 150.0, 12.0));
    segment_bar(&p, g, frac, 10, GREEN);
    p.text(Pos2::new(rect.right() - 10.0, y + 2.0), egui::Align2::RIGHT_TOP, format!("{toks:.0}/{cpm}"), font(PX_SM), GRID_HI);
}

fn accuracy_footer(rect: Rect, p: &egui::Painter, st: &AppState) {
    let link = format!(
        "LINK {} · KEY {} · CPM {}",
        if st.link_on { "ON" } else { "OFF" },
        if st.key_set { "SET" } else { "NONE" },
        st.cpm
    );
    let lines = [
        "SOL: VSOP87 <1\" VS HORIZONS (FROZEN CHECK)",
        "EXO: TWO-BODY FROM PUBLISHED",
        "ELEMENTS — CEILING STATED §5.2",
    ];
    let mut y = rect.top() + 7.0;
    p.text(Pos2::new(rect.left() + 10.0, y), egui::Align2::LEFT_TOP, link, font(PX_SM), CYAN);
    y += 20.0;
    for l in lines {
        p.text(Pos2::new(rect.left() + 10.0, y), egui::Align2::LEFT_TOP, l, font(PX_SM), GRAY);
        y += 19.0;
    }
}

pub fn draw_bottom(ui: &mut Ui, st: &mut AppState, nsys: usize) {
    let full = ui.available_width();
    // row 1: number strip (R7)
    let (r1, _) = ui.allocate_exact_size(Vec2::new(full, 34.0), egui::Sense::hover());
    let p = ui.painter().clone();
    p.line_segment([r1.left_top(), r1.right_top()], STROKE_GRID);
    p.text(r1.left_top() + Vec2::new(8.0, 10.0), egui::Align2::LEFT_TOP, "SYS", font(PX_SM), GRAY);
    for i in 0..nsys {
        let r = Rect::from_min_size(Pos2::new(r1.left() + 50.0 + i as f32 * 42.0, r1.top() + 4.0), Vec2::new(38.0, 26.0));
        let active = st.system == i;
        p.rect_filled(r, 0.0, if active { CYAN } else { CELL });
        p.rect_stroke(r, 0.0, if active { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, format!("{:02}", i + 1), font(PX_SM), if active { INK } else { GRAY });
        if tip(ui.interact(r, ui.id().with(format!("sysnum{i}")), egui::Sense::click()), st.tips,
            "Travel to this star system.")
            .clicked()
        {
            st.system = i;
        }
    }
    // speed tabs on the right of row 1
    let tabs_x = r1.right() - (RATE_TABS.len() as f32 * 78.0 + 60.0);
    for (i, t) in RATE_TABS.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(tabs_x + i as f32 * 78.0, r1.top() + 5.0), Vec2::new(74.0, 24.0));
        let active = st.rate_tab == i;
        p.rect_filled(r, 0.0, if active { CYAN } else { CELL });
        p.rect_stroke(r, 0.0, if active { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, *t, font(PX_SM), if active { INK } else { GRAY });
        if tip(ui.interact(r, ui.id().with(format!("rate{i}")), egui::Sense::click()), st.tips, rate_tip(i)).clicked() {
            st.rate_tab = i;
        }
    }
    // rewind ◀
    let rw = Rect::from_min_size(Pos2::new(r1.right() - 52.0, r1.top() + 5.0), Vec2::new(44.0, 24.0));
    p.rect_filled(rw, 0.0, if st.rewind { AMBER } else { CELL });
    p.rect_stroke(rw, 0.0, if st.rewind { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
    p.text(rw.center(), egui::Align2::CENTER_CENTER, "◀◀", font(PX_SM), if st.rewind { INK } else { GRAY });
    if tip(ui.interact(rw, ui.id().with("rewind"), egui::Sense::click()), st.tips,
        "Run time backwards instead of forwards.")
        .clicked()
    {
        st.rewind = !st.rewind;
    }

    // row 2: bottom key row (§9) — AUTO is AMBER
    let (r2, _) = ui.allocate_exact_size(Vec2::new(full, 36.0), egui::Sense::hover());
    p.line_segment([r2.left_top(), r2.right_top()], STROKE_GRID);
    let keys: [(&str, bool, Option<Color32>, &str); 11] = [
        ("AUTO", st.auto_tour, Some(AMBER), "auto"),
        ("MANUAL", st.cam == CamMode::Free, None, "manual"),
        ("PAUSE", st.paused, Some(ALARM), "pause"),
        ("SPEED", false, None, "speed"),
        ("ORBITS", st.show_orbits, None, "orbits"),
        ("LABELS", st.show_labels, None, "labels"),
        ("GRID", st.show_grid, None, "grid"),
        ("BELTS", st.show_belts, None, "belts"),
        (if st.scale == ScaleMode::True { "TRUE" } else { "VIS" }, st.scale == ScaleMode::Visual, None, "scale"),
        ("TRACK", st.cam == CamMode::Track, None, "track"),
        ("RESET", false, None, "reset"),
    ];
    let kw = ((full - 360.0) / 11.0).clamp(52.0, 96.0);
    let mut x = r2.left() + 8.0;
    for (label, active, accent, id) in keys {
        let r = Rect::from_min_size(Pos2::new(x, r2.top() + 5.0), Vec2::new(kw, 26.0));
        let (fill, fg, border) = if active {
            (accent.unwrap_or(CYAN), INK, GRID_HI)
        } else {
            (CELL, GRAY, GRID)
        };
        p.rect_filled(r, 0.0, fill);
        p.rect_stroke(r, 0.0, Stroke1(border), egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, label, font(PX_SM), fg);
        if tip(ui.interact(r, ui.id().with(id), egui::Sense::click()), st.tips, key_tip(id)).clicked() {
            match id {
                "auto" => { st.auto_tour = !st.auto_tour; if st.auto_tour { st.paused = false; } }
                "manual" => { st.cam = CamMode::Free; st.auto_tour = false; }
                "pause" => st.paused = !st.paused,
                "speed" => st.rate_tab = (st.rate_tab + 1) % RATE_TABS.len(),
                "orbits" => st.show_orbits = !st.show_orbits,
                "labels" => st.show_labels = !st.show_labels,
                "grid" => st.show_grid = !st.show_grid,
                "belts" => st.show_belts = !st.show_belts,
                "scale" => st.scale = if st.scale == ScaleMode::True { ScaleMode::Visual } else { ScaleMode::True },
                "track" => { st.cam = CamMode::Track; st.auto_tour = false; }
                "reset" => { st.cam = CamMode::TopDown; st.rate_tab = 0; st.rewind = false; st.paused = false; }
                _ => {}
            }
        }
        x += kw + 4.0;
    }
    // speed slider (R8) — log slider, right end of row 2
    let sl = Rect::from_min_size(Pos2::new(r2.right() - 320.0, r2.top() + 12.0), Vec2::new(300.0, 12.0));
    p.rect_filled(sl, 0.0, CELL);
    p.rect_stroke(sl, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    let f = (st.rate_log + 8.0) / 20.0; // range 2^-8 .. 2^12 days/s
    let kx = egui::lerp(sl.left()..=sl.right(), f.clamp(0.0, 1.0));
    p.rect_filled(Rect::from_min_size(Pos2::new(sl.left(), sl.top()), Vec2::new(kx - sl.left(), sl.height())), 0.0, CYAN.linear_multiply(0.4));
    p.rect_filled(Rect::from_center_size(Pos2::new(kx, sl.center().y), Vec2::new(6.0, 18.0)), 0.0, AMBER);
    p.rect_stroke(Rect::from_center_size(Pos2::new(kx, sl.center().y), Vec2::new(6.0, 18.0)), 0.0, STROKE_HI, egui::StrokeKind::Outside);
    {
        let inter = tip(ui.interact(sl, ui.id().with("rateslider"), egui::Sense::click_and_drag()), st.tips,
            "Fine time-speed control: drag for any speed between the presets.");
        if inter.dragged() || inter.clicked() {
            if let Some(pos) = inter.interact_pointer_pos() {
                let f = ((pos.x - sl.left()) / sl.width()).clamp(0.0, 1.0);
                st.rate_log = f * 20.0 - 8.0;
                st.rate_tab = 5; // custom
            }
        }
    }

    // row 3 (session 5): per-class object toggles + small-body DENSITY slider
    let (r3, _) = ui.allocate_exact_size(Vec2::new(full, 34.0), egui::Sense::hover());
    p.line_segment([r3.left_top(), r3.right_top()], STROKE_GRID);
    p.text(r3.left_top() + Vec2::new(8.0, 9.0), egui::Align2::LEFT_TOP, "OBJ", font(PX_SM), GRAY);
    let chips: [(&str, bool, fn(&mut AppState)); 8] = [
        ("PLANETS", st.show_planets, |s| s.show_planets = !s.show_planets),
        ("MOONS", st.show_moons, |s| s.show_moons = !s.show_moons),
        ("ASTEROIDS", st.show_asteroids, |s| s.show_asteroids = !s.show_asteroids),
        ("COMETS", st.show_comets, |s| s.show_comets = !s.show_comets),
        ("PROBES", st.show_probes, |s| s.show_probes = !s.show_probes),
        ("DEEP-SKY", st.show_deep_sky, |s| s.show_deep_sky = !s.show_deep_sky),
        ("GALAXIES", st.show_galaxies, |s| s.show_galaxies = !s.show_galaxies),
        ("SYSTEMS", st.show_systems, |s| s.show_systems = !s.show_systems),
    ];
    let cw = 78.0;
    let mut x = r3.left() + 46.0;
    for (i, (label, active, flip)) in chips.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, r3.top() + 5.0), Vec2::new(cw, 24.0));
        p.rect_filled(r, 0.0, if *active { CYAN } else { CELL });
        p.rect_stroke(r, 0.0, if *active { STROKE_HI } else { STROKE_GRID }, egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, *label, font(PX_SM), if *active { INK } else { GRAY });
        if tip(ui.interact(r, ui.id().with(format!("cls{i}")), egui::Sense::click()), st.tips, cls_tip(i)).clicked() {
            flip(st);
        }
        x += cw + 4.0;
    }
    // DENSITY slider: how many synthetic asteroids/comets are tracked (20..1000)
    let dsl = Rect::from_min_size(Pos2::new(r3.right() - 292.0, r3.top() + 11.0), Vec2::new(216.0, 12.0));
    p.text(Pos2::new(dsl.left() - 8.0, r3.top() + 8.0), egui::Align2::RIGHT_TOP, "SB DENSITY", font(PX_SM), GRAY);
    p.rect_filled(dsl, 0.0, CELL);
    p.rect_stroke(dsl, 0.0, STROKE_GRID, egui::StrokeKind::Outside);
    let df = (st.density as f32 - 20.0) / 980.0;
    let dkx = egui::lerp(dsl.left()..=dsl.right(), df.clamp(0.0, 1.0));
    p.rect_filled(Rect::from_min_size(dsl.left_top(), Vec2::new(dkx - dsl.left(), dsl.height())), 0.0, CYAN.linear_multiply(0.4));
    p.rect_filled(Rect::from_center_size(Pos2::new(dkx, dsl.center().y), Vec2::new(6.0, 18.0)), 0.0, AMBER);
    p.rect_stroke(Rect::from_center_size(Pos2::new(dkx, dsl.center().y), Vec2::new(6.0, 18.0)), 0.0, STROKE_HI, egui::StrokeKind::Outside);
    p.text(Pos2::new(r3.right() - 62.0, r3.top() + 8.0), egui::Align2::LEFT_TOP, format!("{:>4}", st.density), font(PX_SM), GRID_HI);
    {
        let inter = tip(ui.interact(dsl, ui.id().with("denslider"), egui::Sense::click_and_drag()), st.tips,
            "How many of the 1000+ real catalogued minor bodies to show (brightest first). Drag for more or fewer.");
        if inter.dragged() || inter.clicked() {
            if let Some(pos) = inter.interact_pointer_pos() {
                let f = ((pos.x - dsl.left()) / dsl.width()).clamp(0.0, 1.0);
                st.density = ((20.0 + f * 980.0).round() as usize).clamp(20, 1000);
            }
        }
    }
    // console collapse handle (session 6)
    if collapse_chip(ui, Pos2::new(r3.right() - 18.0, r3.top() + 4.0), "xp_console_in", st.tips) {
        st.show_console = false;
    }
}

fn Stroke1(c: Color32) -> egui::Stroke {
    egui::Stroke::new(1.0_f32, c)
}

/// Plain-language tooltips for the bottom key row (session 6).
fn key_tip(id: &str) -> &'static str {
    match id {
        "auto" => "Guided tour: the camera flies to each planet in turn and waits there.",
        "manual" => "Free camera: left-drag orbits around, right-drag slides, wheel zooms.",
        "pause" => "Freeze simulated time exactly where it is.",
        "speed" => "Step through the time-speed presets.",
        "orbits" => "Show or hide the paths bodies follow around the Sun.",
        "labels" => "Show or hide the name tags next to bodies.",
        "grid" => "Show or hide the distance rings (1, 5, 10, 20, 30 AU).",
        "belts" => "Show or hide the dust dots of the asteroid and Kuiper belts.",
        "scale" => "TRUE = honest sizes and distances. VIS = bodies enlarged so moons are visible.",
        "track" => "Glue the camera to the selected body.",
        _ => "Back to the start: overview angle, real-time clock, running.",
    }
}

/// Tooltips for the object-class toggle chips (session 6).
fn cls_tip(i: usize) -> &'static str {
    match i {
        0 => "Show or hide the planets (and Pluto).",
        1 => "Show or hide the moons orbiting the planets.",
        2 => "Show or hide asteroids — real catalogued rocks, red ring = hazardous.",
        3 => "Show or hide comets, with their gas and dust tails when active.",
        4 => "Show or hide spacecraft (green diamonds): Voyagers, Juno, Parker…",
        5 => "Show or hide the blurred far-away stuff: pulsars, quasars, nebulae, stars…",
        6 => "Show or hide background galaxies (blue spirals).",
        _ => "Show or hide the pink rings of the other star systems. Click one to travel there.",
    }
}

/// Tooltips for the time-speed tabs (session 6).
fn rate_tip(i: usize) -> &'static str {
    match i {
        0 => "Real time: the simulation follows your wall clock exactly.",
        1 => "Time flows 60 times faster than real.",
        2 => "One simulated hour passes every second.",
        3 => "One simulated day passes every second.",
        4 => "One simulated year passes every second.",
        _ => "Fastest setting: 100 simulated years every second.",
    }
}
