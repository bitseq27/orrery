//! ORRERY — native desktop orrery HMI (PLAN.md).
//! M1: live sim clock + VSOP87 scene + speed presets + live telemetry.

mod app_state;
mod body_data;
mod catalog;
mod key_store;
mod online;
mod rate_limiter;
mod layout;
mod scene;
mod sim;
mod spectra;
mod telemetry;
mod theme;
mod widgets;

use app_state::{AppState, CamMode, ScaleMode};
use eframe::egui;
use catalog::Catalog;
use scene::SceneCache;
use sim::SimClock;
use telemetry::{resolve, telemetry_exo};
use theme::*;

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1560.0, 900.0])
            .with_min_inner_size([1100.0, 680.0])
            .with_title("ORRERY — TIME-TRUE SOLAR SYSTEM SCOPE"),
        ..Default::default()
    };
    eframe::run_native(
        "ORRERY",
        opts,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(OrreryApp::new_with_env()))
        }),
    )
}

struct OrreryApp {
    st: AppState,
    clock: SimClock,
    cache: SceneCache,
    cat: Catalog,
    bucket: std::sync::Arc<std::sync::Mutex<rate_limiter::TokenBucket>>,
    link: std::sync::Arc<std::sync::Mutex<online::LinkState>>,
    api_key: String,
    store_key: bool,
    show_key: bool,
    fps: f32,
    last_system: usize,
    exo_frame: [[f64; 3]; 3],
    /// previous frame's camera mode — entering FREE SNAPS the orbit centre to
    /// the selected body (session-5 fix: lerping from the pinned TopDown origin
    /// made the camera fly through/to Sol when you tried to free-orbit a planet)
    prev_cam: CamMode,
}

impl OrreryApp {
    /// Dev/verification hooks (also used by the M6 visual-regression harness):
    /// ORRERY_ZOOM=px/AU, ORRERY_SELECT=NAME, ORRERY_JD=<jd tt>, ORRERY_RATE_TAB=n
    fn new_with_env() -> Self {
        let mut app = Self { st: AppState::default(), clock: SimClock::default(), cache: SceneCache::default(), cat: Catalog::load(),
            bucket: std::sync::Arc::new(std::sync::Mutex::new(rate_limiter::TokenBucket::new(20, 0.0))),
            link: std::sync::Arc::new(std::sync::Mutex::new(online::LinkState::default())),
            api_key: key_store::load().unwrap_or_default(),
            store_key: true,
            show_key: false,
            fps: 0.0,
            last_system: 999,
            exo_frame: scene::IDENTITY,
            prev_cam: CamMode::TopDown,
        };
        app.exo_frame = Self::system_frame(&app.cat, app.st.system);
        app.st.exo_frame = app.exo_frame;
        app.st.key_set = !app.api_key.is_empty();
        app.st.cpm = 20;
        app.bucket.lock().unwrap().set_cpm(app.st.cpm, 0.0);
        if let Ok(z) = std::env::var("ORRERY_ZOOM") {
            if let Ok(z) = z.parse::<f32>() { app.st.zoom = z; }
        }
        if let Ok(sel) = std::env::var("ORRERY_SELECT") {
            app.st.selected_body = Some(sel);
        }
        if let Ok(q) = std::env::var("ORRERY_SEARCH") {
            app.st.search = q;
            app.st.search_open = !app.st.search.is_empty();
        }
        if let Ok(jd) = std::env::var("ORRERY_JD") {
            if let Ok(jd) = jd.parse::<f64>() { app.clock.jd_tt = jd; app.st.rate_tab = 3; app.st.paused = true; }
        }
        if let Ok(c) = std::env::var("ORRERY_CAM") {
            app.st.cam = match c.to_lowercase().as_str() {
                "free" => CamMode::Free,
                "track" => CamMode::Track,
                _ => CamMode::TopDown,
            };
        }
        if let Ok(y) = std::env::var("ORRERY_YAW") { if let Ok(v) = y.parse() { app.st.cam_yaw = v; } }
        if let Ok(y) = std::env::var("ORRERY_PITCH") { if let Ok(v) = y.parse() { app.st.cam_pitch = v; } }
        if let Ok(a) = std::env::var("ORRERY_AUTO") { app.st.auto_tour = a == "1"; }
        if let Ok(k) = std::env::var("ORRERY_KEY") {
            app.api_key = k;
            app.st.key_set = true;
        }
        if let Ok(v) = std::env::var("ORRERY_LINK") {
            app.st.link_on = v == "1";
        }
        if let Ok(v) = std::env::var("ORRERY_LSCROLL") {
            if let Ok(v) = v.parse::<f32>() { app.st.left_scroll = v; }
        }
        if let Ok(v) = std::env::var("ORRERY_RSCROLL") {
            if let Ok(v) = v.parse::<f32>() { app.st.right_scroll = v; }
        }
        if std::env::var("ORRERY_LINKTEST").is_ok() {
            let key = if app.st.key_set { Some(app.api_key.clone()) } else { None };
            online::run_test_suite(key, app.bucket.clone(), app.link.clone());
        }
        if let Ok(v) = std::env::var("ORRERY_SYS") {
            if let Ok(v) = v.parse::<usize>() { app.st.system = v.min(10); }
        }
        if let Ok(t) = std::env::var("ORRERY_TOUR_STOP") {
            if let Ok(t) = t.parse::<usize>() { app.st.tour_stop = t % 10; app.st.auto_tour = true; }
        }
        if let Ok(v) = std::env::var("ORRERY_VIS") {
            app.st.scale = if v == "1" { ScaleMode::Visual } else { ScaleMode::True };
        }
        if let Ok(r) = std::env::var("ORRERY_RATE_TAB") {
            if let Ok(r) = r.parse::<usize>() { app.st.rate_tab = r.min(5); }
        }
        if let Ok(v) = std::env::var("ORRERY_DENSITY") {
            if let Ok(v) = v.parse::<usize>() { app.st.density = v.clamp(20, 1000); }
        }
        if std::env::var("ORRERY_COLLAPSE").map(|v| v == "1").unwrap_or(false) {
            app.st.show_left = false;
            app.st.show_right = false;
            app.st.show_console = false;
            app.st.show_spectra = false;
            app.st.show_legend = false;
        }
        if std::env::var("ORRERY_SKY").map(|v| v == "0").unwrap_or(false) {
            app.st.show_deep_sky = false;
            app.st.show_galaxies = false;
            app.st.show_systems = false;
        }
        // AFTER env parsing: the per-frame system-switch reset (top-down arrival)
        // must not clobber an env-requested camera/zoom on the first frame
        app.last_system = app.st.system;
        app.exo_frame = Self::system_frame(&app.cat, app.st.system);
        app.st.exo_frame = app.exo_frame;
        app
    }
}

impl eframe::App for OrreryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);
        // clamp hitch spikes; 0.25 keeps sim time honest down to 4 fps (the old
        // 0.1 clamp made the animation itself slow whenever a frame hitched —
        // session-5 "animation slows down when a planet is selected" fix, part 3)
        let dt = ctx.input(|i| i.predicted_dt).min(0.25);
        if dt > 1e-4 {
            self.fps = if self.fps < 1.0 { 1.0 / dt } else { self.fps * 0.95 + (1.0 / dt) * 0.05 };
        }
        self.clock.advance(&self.st, dt);
        let jd = self.clock.jd_tt;
        self.auto_tour_step(dt);

        // system switch: always arrive in TOP-DOWN of the system's own plane
        if self.last_system != self.st.system {
            self.last_system = self.st.system;
            self.st.cam = CamMode::TopDown;
            self.prev_cam = CamMode::TopDown;
            self.st.pan = [0.0, 0.0];
            self.st.zoom = 0.0;
            self.st.cam_yaw = 0.0;
            self.st.cam_pitch = 0.0;
            self.st.cam_focus = [0.0; 3];
            self.st.auto_tour = false;
            self.st.tour_stop = 0;
            self.st.tour_t = 0.0;
            self.st.scope_cache = None;
            self.exo_frame = Self::system_frame(&self.cat, self.st.system);
            self.st.exo_frame = self.exo_frame;
        }

        // live telemetry for the selected Sol body (R11)
        let tel = match self.st.selected_body.as_deref() {
            Some(n) if self.st.system == 0 => resolve(n, jd),
            Some(n) => telemetry_exo(&self.cat, self.st.system - 1, n, jd),
            None => None,
        };
        let cam_target = tel
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "---".into());

        // FREE-camera focus: smoothly fly the orbit centre to the selected body
        // so FREE mode can orbit ANY target (session-4 fix: rotation was pinned
        // to the system origin and exo systems ignored yaw/pitch entirely).
        match self.st.cam {
            CamMode::Track => {
                if let Some(t) = tel.as_ref() {
                    self.st.cam_focus = t.pos_icrs; // snap — track is already locked
                }
            }
            CamMode::Free => {
                let target = tel.as_ref().map(|t| t.pos_icrs).unwrap_or([0.0; 3]);
                if self.prev_cam != CamMode::Free {
                    // ENTERING FREE: snap straight to the target. The old lerp
                    // started from wherever TopDown pinned the focus (the system
                    // origin), so zooming into a planet and switching to FREE
                    // threw the camera across the system — "jumps to Sol".
                    self.st.cam_focus = target;
                } else {
                    // within FREE: keep the smooth fly-to when the target moves
                    // or the selection changes
                    let k = (1.0 - (-6.0 * dt as f64).exp()) as f64;
                    for ax in 0..3 {
                        self.st.cam_focus[ax] += (target[ax] - self.st.cam_focus[ax]) * k;
                    }
                }
            }
            CamMode::TopDown => self.st.cam_focus = [0.0; 3],
        }
        self.prev_cam = self.st.cam;

        // ---- top header ----------------------------------------------------
        let epoch = self.clock.epoch_str();
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::default().fill(BG).inner_margin(0))
            .resizable(false)
            .show(ctx, |ui| {
                let names = layout::system_names(&self.cat);
                layout::draw_header(ui, &mut self.st, &epoch, &cam_target, &names, self.fps, &self.cat);
            });

        // ---- bottom: number strip + tabs + key row (collapsible console) -----
        egui::TopBottomPanel::bottom("bottom")
            .frame(egui::Frame::default().fill(BG).inner_margin(0))
            .resizable(false)
            .show(ctx, |ui| {
                if self.st.show_console {
                    layout::draw_bottom(ui, &mut self.st, self.cat.systems.len() + 1);
                } else {
                    let (r, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 18.0),
                        egui::Sense::hover(),
                    );
                    if layout::draw_strip(ui, &mut self.st, r, "CONSOLE (SYS · SPEED · CLASSES)", "xp_console") {
                        self.st.show_console = true;
                    }
                }
            });

        // ---- left telemetry column (selected object · raw · scope) ----------
        if self.st.show_left {
            egui::SidePanel::left("telemetry")
                .frame(egui::Frame::default().fill(BG).inner_margin(6))
                .resizable(true)
                .default_width(360.0)
                .width_range(300.0..=520.0)
                .show(ctx, |ui| {
                    let mut scope_cache = self.st.scope_cache.take();
                    layout::draw_left_column(ui, &mut self.st, tel.as_ref(), jd, &mut scope_cache);
                    self.st.scope_cache = scope_cache;
                    if layout::collapse_chip(ui, ui.max_rect().right_top() + egui::vec2(-18.0, 2.0), "xp_left", self.st.tips) {
                        self.st.show_left = false;
                    }
                });
        } else {
            egui::SidePanel::left("telemetry_min")
                .frame(egui::Frame::default().fill(BG).inner_margin(0))
                .exact_width(20.0)
                .show(ctx, |ui| {
                    if layout::expand_rail(ui, "TELEMETRY", "xp_left", self.st.tips) {
                        self.st.show_left = true;
                    }
                });
        }

        // ---- spectra column (session 6): docked immediately RIGHT of the
        // telemetry/data column — 4 model graphs + composition chart ----------
        if self.st.show_spectra {
            egui::SidePanel::left("spectra_col")
                .frame(egui::Frame::default().fill(BG).inner_margin(6))
                .resizable(true)
                .default_width(220.0)
                .width_range(200.0..=660.0)
                .show(ctx, |ui| {
                    layout::draw_spectra_column(ui, tel.as_ref(), self.st.tips);
                    if layout::collapse_chip(ui, ui.max_rect().right_top() + egui::vec2(-18.0, 2.0), "xp_spec_col", self.st.tips) {
                        self.st.show_spectra = false;
                    }
                });
        } else {
            egui::SidePanel::left("spectra_min")
                .frame(egui::Frame::default().fill(BG).inner_margin(0))
                .exact_width(20.0)
                .show(ctx, |ui| {
                    if layout::expand_rail(ui, "SPECTRA", "xp_spec_col", self.st.tips) {
                        self.st.show_spectra = true;
                    }
                });
        }

        // ---- right column (sky map · residual log · data link) ---------------
        if self.st.show_right {
            egui::SidePanel::right("skymap_link")
                .frame(egui::Frame::default().fill(BG).inner_margin(6))
                .resizable(true)
                .default_width(350.0)
                .width_range(300.0..=520.0)
                .show(ctx, |ui| {
                    let mut link = layout::LinkUi {
                        key: &mut self.api_key,
                        store: &mut self.store_key,
                        show: &mut self.show_key,
                        bucket: self.bucket.clone(),
                        link: self.link.clone(),
                    };
                    layout::draw_right_column(ui, &mut self.st, &self.cat, &mut link);
                    if layout::collapse_chip(ui, ui.max_rect().left_top() + egui::vec2(2.0, 2.0), "xp_right", self.st.tips) {
                        self.st.show_right = false;
                    }
                });
        } else {
            egui::SidePanel::right("skymap_min")
                .frame(egui::Frame::default().fill(BG).inner_margin(0))
                .exact_width(20.0)
                .show(ctx, |ui| {
                    if layout::expand_rail(ui, "SKY/LINK", "xp_right", self.st.tips) {
                        self.st.show_right = true;
                    }
                });
        }

        // camera centre: TRACK locks on the selected body (R9); FREE orbits the
        // smoothed focus point (selected body or origin)
        let center = match self.st.cam {
            CamMode::Track => tel.as_ref().map(|t| t.pos_icrs).unwrap_or([0.0, 0.0, 0.0]),
            CamMode::Free => self.st.cam_focus,
            CamMode::TopDown => [0.0, 0.0, 0.0],
        };

        // ---- centre: 3D viewport cell ---------------------------------------
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(6))
            .show(ctx, |ui| {
                let sysidx = self.st.system;
                let frame = self.exo_frame;
                scene::draw_scene(ui, &mut self.st, jd, &mut self.cache, center, &self.cat, sysidx, frame);
            });

        ctx.request_repaint_after(std::time::Duration::from_millis(16)); // 60 fps loop
    }
}

impl OrreryApp {
    /// Systems always LOAD top-down in their own orbital plane (user feedback:
    /// transiting systems were appearing edge-on). Frame axes: z' = orbit normal
    /// of the first catalog planet, x' = node-like perpendicular, y' = z'×x'.
    fn system_frame(cat: &Catalog, sys: usize) -> [[f64; 3]; 3] {
        if sys == 0 {
            return scene::IDENTITY;
        }
        let Some(s) = cat.systems.get(sys - 1) else { return scene::IDENTITY };
        let Some(p0) = s.planets.first() else { return scene::IDENTITY };
        let el = cat.elements(sys - 1, 0);
        let stt = el.state(el.epoch);
        let _ = p0;
        let h = [
            stt.pos[1] * stt.vel[2] - stt.pos[2] * stt.vel[1],
            stt.pos[2] * stt.vel[0] - stt.pos[0] * stt.vel[2],
            stt.pos[0] * stt.vel[1] - stt.pos[1] * stt.vel[0],
        ];
        let hn = (h[0].powi(2) + h[1].powi(2) + h[2].powi(2)).sqrt().max(1e-12);
        let z = [h[0] / hn, h[1] / hn, h[2] / hn];
        let ref_v = if z[2].abs() > 0.95 { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let x = [
            ref_v[1] * z[2] - ref_v[2] * z[1],
            ref_v[2] * z[0] - ref_v[0] * z[2],
            ref_v[0] * z[1] - ref_v[1] * z[0],
        ];
        let xn = (x[0].powi(2) + x[1].powi(2) + x[2].powi(2)).sqrt().max(1e-12);
        let x = [x[0] / xn, x[1] / xn, x[2] / xn];
        let y = [
            z[1] * x[2] - z[2] * x[1],
            z[2] * x[0] - z[0] * x[2],
            z[0] * x[1] - z[1] * x[0],
        ];
        [x, y, z]
    }

    /// §8.3 AUTO-TOUR: overview → each planet in orbital order (track + dwell) → loop.
    fn auto_tour_step(&mut self, dt: f32) {
        if !self.st.auto_tour {
            return;
        }
        self.st.tour_t += dt;
        let dur = if self.st.tour_stop == 0 { 6.0 } else { 5.0 };
        if self.st.tour_t >= dur {
            self.st.tour_t = 0.0;
            let nstops = if self.st.system == 0 {
                10
            } else {
                self.cat.systems[self.st.system - 1].planets.len() + 1
            };
            self.st.tour_stop = (self.st.tour_stop + 1) % nstops;
        }
        let nstops = if self.st.system == 0 {
            10
        } else {
            self.cat.systems[self.st.system - 1].planets.len() + 1
        };
        let stop = self.st.tour_stop;
        if stop == 0 {
            self.st.cam = CamMode::TopDown;
            self.st.zoom = 0.0;
            self.st.pan = [0.0, 0.0];
        } else if self.st.system == 0 {
            use orrery_astro::planets::{jpl_elements, PLANET_IDS, PLANET_NAMES};
            let id = PLANET_IDS[stop - 1];
            let a = jpl_elements(id).a;
            self.st.selected_body = Some(PLANET_NAMES[stop - 1].to_string());
            self.st.cam = CamMode::Track;
            self.st.zoom = (315.0 / a).clamp(4.0, 200_000.0) as f32;
            self.st.pan = [0.0, 0.0];
        } else {
            let sys = &self.cat.systems[self.st.system - 1];
            let pl = &sys.planets[(stop - 1) % sys.planets.len()];
            self.st.selected_body = Some(pl.name.clone());
            self.st.cam = CamMode::Track;
            self.st.zoom = (90.0 / pl.a_au).clamp(4.0, 400_000.0) as f32;
            self.st.pan = [0.0, 0.0];
        }
        let _ = nstops;
    }

    /// §11 keyboard map.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        let st = &mut self.st;
        ctx.input(|i| {
            for (k, idx) in [
                (egui::Key::Num1, 0usize), (egui::Key::Num2, 1), (egui::Key::Num3, 2),
                (egui::Key::Num4, 3), (egui::Key::Num5, 4), (egui::Key::Num6, 5),
                (egui::Key::Num7, 6), (egui::Key::Num8, 7), (egui::Key::Num9, 8),
                (egui::Key::Num0, 9), (egui::Key::Minus, 10),
            ] {
                if i.key_pressed(k) && !i.modifiers.ctrl && !i.modifiers.command {
                    st.system = idx;
                }
            }
            if i.key_pressed(egui::Key::Space) {
                st.paused = !st.paused;
            }
            if i.key_pressed(egui::Key::A) {
                st.auto_tour = !st.auto_tour;
            }
            if i.key_pressed(egui::Key::T) {
                st.cam = CamMode::Track;
                st.auto_tour = false;
            }
            if i.key_pressed(egui::Key::G) {
                st.scale = if st.scale == ScaleMode::True { ScaleMode::Visual } else { ScaleMode::True };
            }
            if i.key_pressed(egui::Key::OpenBracket) {
                st.show_orbits = !st.show_orbits;
            }
            if i.key_pressed(egui::Key::CloseBracket) {
                st.show_labels = !st.show_labels;
            }
            if i.key_pressed(egui::Key::P) {
                st.show_probes = !st.show_probes;
            }
            if i.key_pressed(egui::Key::D) {
                st.show_deep_sky = !st.show_deep_sky;
            }
            if i.key_pressed(egui::Key::X) {
                st.show_galaxies = !st.show_galaxies;
            }
            if i.key_pressed(egui::Key::Y) {
                st.show_systems = !st.show_systems;
            }
            if i.key_pressed(egui::Key::L) {
                st.show_legend = !st.show_legend;
            }
            if i.key_pressed(egui::Key::C) {
                st.show_console = !st.show_console;
            }
            if i.key_pressed(egui::Key::V) {
                st.show_spectra = !st.show_spectra;
            }
            if i.key_pressed(egui::Key::Home) {
                st.cam = CamMode::TopDown;
                st.rate_tab = 0;
                st.rewind = false;
                st.paused = false;
                st.zoom = 0.0;
            }
            if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                st.rate_log = (st.rate_log + 1.0).min(12.0);
            }
        });
    }
}
