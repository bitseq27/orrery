//! Persistent app state (UI-level). M0: placeholders; filled in from M1 onward.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamMode {
    Free,
    Track,
    TopDown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScaleMode {
    True,
    Visual,
}

/// Speed preset tabs (§8.2). Index into RATE_TABS.
pub const RATE_TABS: [&str; 6] = ["LIVE 1X", "60X", "1H/S", "1D/S", "1YR/S", "MAX"];

/// Cached r/|v|/phase(t) scope samples (recomputed only when the epoch moves
/// by 1/240 of the plotted period — removes per-frame resampling cost).
#[derive(Clone)]
pub struct ScopeCache {
    pub key: String,
    pub mode: u8,
    pub jd_q: f64,
    pub vals: Vec<f64>,
    pub rmin: f64,
    pub rmax: f64,
    /// wall-clock stamp of the bake — the SCOPE re-bake is throttled with this
    /// (session-5 perf: at high sim rates the old period/240 quantisation made
    /// the plot resample EVERY frame, and the frame-time clamp then literally
    /// slowed the animation whenever a body was selected)
    pub baked: std::time::Instant,
}

pub struct AppState {
    // system selector (R7): 0 = Sol, 1..10 = exo systems
    pub system: usize,
    // camera (R9/R10)
    pub cam: CamMode,
    pub auto_tour: bool,
    // display toggles
    pub show_orbits: bool,
    pub show_labels: bool,
    pub show_grid: bool,
    pub show_belts: bool,
    // session-5 per-class object toggles
    pub show_planets: bool,
    pub show_moons: bool,
    pub show_asteroids: bool,
    pub show_comets: bool,
    pub show_probes: bool,
    pub show_deep_sky: bool,
    pub show_galaxies: bool,
    pub show_systems: bool,
    /// how many of the 1000+ REAL catalogued minor bodies (SBDB extended set)
    /// are drawn & tracked — DENSITY slider, 20..=1000, brightest-first prefix
    pub density: usize,
    /// scene pointer-press origin — distinguishes real clicks from orbit drags
    /// that end where they started-ish (session-5: free-orbit no longer
    /// deselects → camera no longer "jumps to Sol")
    pub click_origin: Option<[f32; 2]>,
    pub scale: ScaleMode,
    pub paused: bool,
    // speed (R8)
    pub rate_tab: usize,
    pub rate_log: f32,  // log2(days per second), from slider
    pub rewind: bool,
    // selection (R11)
    pub selected_body: Option<String>,
    // data link (R14/R15) — M4.5
    pub link_on: bool,
    pub cpm: u32,
    pub key_set: bool,
    pub sys_menu_open: bool,
    /// session 8: header FIND BODY field — live query + dropdown open flag
    pub search: String,
    pub search_open: bool,
    /// memoised pool of selectable names; rebuilt when `system` changes
    pub search_pool: Vec<String>,
    pub search_pool_sys: usize,
    /// px per AU; 0.0 = auto-fit
    pub zoom: f32,
    // FREE arcball (R9)
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    /// view-plane pan offset [AU]
    pub pan: [f32; 2],
    // AUTO tour (R10)
    pub left_scroll: f32,
    /// right column (sky map / residual log / data link) manual scroll
    pub right_scroll: f32,
    /// smoothed FREE-camera focus point (ICRS AU) — lerps toward the selected
    /// body so FREE mode orbits the target instead of the system origin
    pub cam_focus: [f64; 3],
    /// scope plot mode: 0 r(t), 1 |v|(t), 2 phase(t)
    pub plot_mode: u8,
    pub tour_stop: usize,
    pub scope_cache: Option<ScopeCache>,
    pub last_system: usize,
    /// view frame of the current system (rows = axes); Sol = identity
    pub exo_frame: [[f64; 3]; 3],
    pub tour_t: f32,
    /// session-5: canvas legend overlay (top-left)
    pub show_legend: bool,
    /// session-6: plain-language tooltips on every control (header toggle)
    pub tips: bool,
    // session-6: collapsible panels
    pub show_left: bool,
    pub show_right: bool,
    pub show_console: bool,
    pub show_spectra: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            system: 0,
            cam: CamMode::TopDown,
            auto_tour: false,
            show_orbits: true,
            show_labels: true,
            show_grid: true,
            show_belts: true,
            show_planets: true,
            show_moons: true,
            show_asteroids: true,
            show_comets: true,
            show_probes: true,
            show_deep_sky: true,
            show_galaxies: true,
            show_systems: true,
            density: 300,
            click_origin: None,
            scale: ScaleMode::True,
            paused: false,
            rate_tab: 0,
            rate_log: 0.0,
            rewind: false,
            selected_body: Some("MARS".into()),
            link_on: false,
            cpm: 20,
            key_set: false,
            sys_menu_open: false,
            search: String::new(),
            search_open: false,
            search_pool: Vec::new(),
            search_pool_sys: usize::MAX,
            zoom: 0.0,
            cam_yaw: 0.0,
            cam_pitch: 0.0,
            pan: [0.0, 0.0],
            left_scroll: 0.0,
            right_scroll: 0.0,
            cam_focus: [0.0; 3],
            plot_mode: 0,
            tour_stop: 0,
            scope_cache: None,
            last_system: 0,
            exo_frame: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            tour_t: 0.0,
            show_legend: true,
            tips: true,
            show_left: true,
            show_right: true,
            show_console: true,
            show_spectra: true,
        }
    }
}
