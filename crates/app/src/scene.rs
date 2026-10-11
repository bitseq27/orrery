//! 3D scene renderer (ADR-002): painter-primitive orrery view.
//! Cameras (R9): TOP-DOWN (ecliptic north), TRACK (locked on selected body),
//! FREE (arcball: L-drag rotate about the FOCUS body, R/middle-drag pan,
//! wheel dolly). Session-5 additions:
//!   * SKY SPHERE backdrop: 99 blurred deep-sky objects (pulsars, quasars,
//!     nebulae, clusters, dwarfs, stars, remnants), ≤100 galaxies as blue
//!     spirals, and the other featured systems as pink circles. Direction-only
//!     projection (parallax negligible): they rotate with FREE yaw/pitch but
//!     never pan or zoom.
//!   * 40 exploration probes from baked Horizons states — green 45° squares
//!     (diamonds); off-screen craft get edge chips so the fleet stays visible.
//!   * Extended REAL small-body catalogue (1000+ SBDB objects, PHA/NEO flags
//!     intact) with the DENSITY slider thinning the drawn prefix 20..1000.
//!   * Two-part comet tails (straight anti-solar ion tail + curved dust tail),
//!     star glow, canvas legend (top-left), per-class toggles.
//! Session-5 fixes: FREE mode SNAPS its focus when entered (no fly-through
//! from Sol at planetary zoom); drag-releases no longer deselect (the click
//! that cleared the selection made the camera "jump to Sol" mid-orbit).
//! Scale modes (§7): TRUE (honest) / VISUAL (log-compressed radii, timing untouched).

use crate::app_state::{AppState, CamMode, ScaleMode};
use crate::catalog::Catalog;
use crate::telemetry::PLANET_RGB;
use crate::theme::*;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use orrery_astro::frames::icrs_to_ecl;
use orrery_astro::moons::moon_defs;
use orrery_astro::planets::{period_days, planet_state_ecl, PlanetId, PLANET_IDS, PLANET_NAMES};
use orrery_astro::population::population;
use orrery_astro::probes::{probe_state, probes};
use orrery_astro::small_bodies::{small_bodies, Kind, SmallBody};
use orrery_astro::time::JD_J2000;
use orrery_astro::{deep_sky, AU_KM};

/// Class colours (session 5).
pub const PROBE_GREEN: Color32 = Color32::from_rgb(70, 225, 120);
pub const PROBE_DIM: Color32 = Color32::from_rgb(40, 130, 80);
pub const SYS_PINK: Color32 = Color32::from_rgb(255, 105, 180);
pub const GAL_BLUE: Color32 = Color32::from_rgb(110, 165, 255);

/// Cached orbit line-loops (re-baked when the epoch moves > 5% of a period, §7)
/// + the baked belt clouds + the selected small body's orbit loop.
pub struct SceneCache {
    pub orbits: Vec<Vec<[f64; 3]>>,
    pub baked_jd: f64,
    pending: Option<std::thread::JoinHandle<(f64, Vec<Vec<[f64; 3]>>)>>,
    /// [a AU, λ0 rad, node rad, inclination rad] particles, main belt & Kuiper
    belts: Option<(Vec<[f64; 4]>, Vec<[f64; 4]>)>,
    /// selected asteroid/comet orbit (osculating → static curve): (name, ecl pts)
    pub sel_orbit: Option<(String, Vec<[f64; 3]>)>,
}

impl Default for SceneCache {
    fn default() -> Self {
        Self {
            orbits: Vec::new(),
            baked_jd: 0.0,
            pending: None,
            belts: None,
            sel_orbit: None,
        }
    }
}

/// Re-bake cadence: orbits are near-static curves; 20 y of secular drift is far
/// below pixel scale, so we rebake rarely — and on a worker thread so high sim
/// rates (1 yr/s, MAX) never hitch the frame loop (user feedback, session 3).
const REBAKE_SPAN_D: f64 = 7305.0;

fn bake_orbits(jd: f64) -> Vec<Vec<[f64; 3]>> {
    PLANET_IDS
        .iter()
        .map(|id| {
            let p = period_days(*id);
            // 0..=512 closes the loop; uniform-in-time stays smooth for e ≤ 0.25.
            (0..=512)
                .map(|k| {
                    let t = jd - p / 2.0 + p * (k as f64 / 512.0);
                    planet_state_ecl(*id, t).pos
                })
                .collect()
        })
        .collect()
}

/// Belt particle clouds, baked ONCE (session 4). Inclined circular orbits with
/// random nodes; the main belt carries Kirkwood-gap rejection sampling.
fn bake_belts() -> (Vec<[f64; 4]>, Vec<[f64; 4]>) {
    let mut seed: u32 = 0xbe17;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((seed >> 8) & 0xFF_FFFF) as f32 / 16_777_216.0
    };
    let kirkwood = [2.502, 2.825, 2.958, 3.279]; // 3:1, 5:2, 7:3, 2:1 resonances
    let mut main = Vec::with_capacity(6000);
    while main.len() < 6000 {
        let a = 2.06 + (3.33 - 2.06) * rnd() as f64;
        if kirkwood.iter().any(|g| (a - g).abs() < 0.018) {
            continue; // gap
        }
        let l0 = rnd() as f64 * std::f64::consts::TAU;
        let node = rnd() as f64 * std::f64::consts::TAU;
        let inc = ((rnd() + rnd()) * 0.5) as f64 * 0.16; // ~0..9° spread
        main.push([a, l0, node, inc]);
    }
    let mut kuiper = Vec::with_capacity(4200);
    while kuiper.len() < 4200 {
        // classical belt + a Plutino clump at the 3:2 resonance (39.4 AU)
        let a = if rnd() < 0.18 {
            39.4 + (rnd() as f64 - 0.5) * 0.8
        } else {
            41.0 + (48.6 - 41.0) * rnd() as f64
        };
        let l0 = rnd() as f64 * std::f64::consts::TAU;
        let node = rnd() as f64 * std::f64::consts::TAU;
        let inc = ((rnd() + rnd()) * 0.5) as f64 * 0.28; // ~0..16° spread
        kuiper.push([a, l0, node, inc]);
    }
    (main, kuiper)
}

impl SceneCache {
    pub fn ensure(&mut self, jd: f64) {
        // collect a finished worker result
        if let Some(h) = self.pending.take() {
            if h.is_finished() {
                let (bj, orbs) = h.join().unwrap();
                self.orbits = orbs;
                self.baked_jd = bj;
            } else {
                self.pending = Some(h);
            }
        }
        if self.orbits.is_empty() {
            self.orbits = bake_orbits(jd);
            self.baked_jd = jd;
        }
        if self.belts.is_none() {
            self.belts = Some(bake_belts());
        }
        if (jd - self.baked_jd).abs() > REBAKE_SPAN_D && self.pending.is_none() {
            let jd2 = jd;
            self.pending = Some(std::thread::spawn(move || (jd2, bake_orbits(jd2))));
        }
    }
}

/// Rotate a point into the view frame (rows = frame axes).
fn frame_mul(f: &[[f64; 3]; 3], p: &[f64; 3]) -> [f64; 3] {
    [
        f[0][0] * p[0] + f[0][1] * p[1] + f[0][2] * p[2],
        f[1][0] * p[0] + f[1][1] * p[1] + f[1][2] * p[2],
        f[2][0] * p[0] + f[2][1] * p[1] + f[2][2] * p[2],
    ]
}

pub const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

struct Proj {
    c: Pos2,
    scale: f32,
    yaw: f32,
    pitch: f32,
    visual: bool,
    /// camera centre in (visual-scaled) ecliptic AU
    center: [f64; 3],
}

impl Proj {
    /// VISUAL scale mode (§7): compress radial distance, keep direction & timing.
    fn shrink(&self, p: &[f64; 3]) -> [f64; 3] {
        if !self.visual {
            return *p;
        }
        let r = (p[0].powi(2) + p[1].powi(2) + p[2].powi(2)).sqrt();
        if r < 1e-12 {
            return *p;
        }
        let f = 4.0 * r.powf(0.42); // cosmetic compression, flagged in UI
        let k = f / r;
        [p[0] * k, p[1] * k, p[2] * k]
    }

    fn to_screen(&self, p_icrs: &[f64; 3]) -> Pos2 {
        let p = self.shrink(&icrs_to_ecl(*p_icrs));
        self.to_screen_ecl(&p)
    }

    fn to_screen_ecl(&self, p: &[f64; 3]) -> Pos2 {
        let q = self.shrink(p);
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let vx = q[0] - self.center[0];
        let vy = q[1] - self.center[1];
        let vz = q[2] - self.center[2];
        let x1 = vx as f32 * cy - vy as f32 * sy;
        let y1 = vx as f32 * sy + vy as f32 * cy;
        let y2 = y1 * cp - vz as f32 * sp;
        Pos2::new(self.c.x + x1 * self.scale, self.c.y - y2 * self.scale)
    }
}

/// Sky-sphere projector for the deep-sky backdrop (session 5): unit direction
/// only — no centre, no pan, no zoom (parallax is negligible beyond ~100 ly).
/// Session-6 fix: GNOMONIC (perspective) projection, like a planetarium dome —
/// the session-5 orthographic disk compressed the whole hemisphere into a rim
/// at a radius comparable to the planetary system on screen, which made the
/// deep sky read as a shell *around* the system instead of infinity. Gnomonic
/// keeps angular density uniform and never piles objects onto a ring.
struct Sky {
    c: Pos2,
    r: f32,
    yaw: f32,
    pitch: f32,
    frame: [[f64; 3]; 3],
}

impl Sky {
    /// Front-hemisphere RA/Dec → screen position (None when behind the camera).
    fn project(&self, ra_deg: f64, dec_deg: f64) -> Option<Pos2> {
        let d = deep_sky::dir_ecl(ra_deg, dec_deg);
        let q = frame_mul(&self.frame, &d);
        let yaw = self.yaw as f64;
        let pitch = self.pitch as f64;
        let (cy, sy) = (yaw.cos(), yaw.sin());
        let (cp, sp) = (pitch.cos(), pitch.sin());
        let x1 = q[0] * cy - q[1] * sy;
        let y1 = q[0] * sy + q[1] * cy;
        let depth = y1 * sp + q[2] * cp;
        if depth < 0.15 {
            return None; // behind the camera / beyond the dome edge (~81°)
        }
        let y2 = y1 * cp - q[2] * sp;
        // perspective divide: tangent-plane (gnomonic) coordinates × focal px
        let sx = (x1 / depth) as f32 * self.r;
        let sy = (y2 / depth) as f32 * self.r;
        Some(Pos2::new(self.c.x + sx, self.c.y - sy))
    }
}

/// Black-hole accretion-disk icon (session 6): dark core + photon ring + a
/// doppler-brightened elliptical disk (approaching side hotter/brighter).
fn accretion_disk(p: &egui::Painter, c: Pos2, r: f32, seed: f32) {
    let rot = seed * 3.0;
    let (ca, sa) = (rot.cos(), rot.sin());
    p.circle_filled(c, r * 2.3, Color32::from_rgb(255, 150, 60).linear_multiply(0.06));
    for k in 0..3 {
        let rr = r * (0.95 + 0.42 * k as f32);
        let sq = 0.30 + 0.06 * k as f32;
        let mut prev: Option<Pos2> = None;
        for seg in 0..=44 {
            let a = seg as f32 / 44.0 * std::f64::consts::TAU as f32;
            let (x, y) = (a.cos() * rr, a.sin() * rr * sq);
            let pt = Pos2::new(c.x + x * ca - y * sa, c.y + x * sa + y * ca);
            if let Some(pv) = prev {
                let mid = a - 0.07 + rot;
                let dop = 0.5 - 0.5 * mid.cos(); // 1 on the approaching side
                let col = Color32::from_rgb(255, 175, 85)
                    .linear_multiply((0.10 + 0.55 * dop) / (1.0 + k as f32 * 0.8));
                p.line_segment([pv, pt], Stroke::new(1.7 - k as f32 * 0.45, col));
            }
            prev = Some(pt);
        }
    }
    p.circle_filled(c, r * 0.52, Color32::BLACK.linear_multiply(0.95));
    p.circle_stroke(c, r * 0.60, Stroke::new(1.0_f32, Color32::from_rgb(255, 210, 130).linear_multiply(0.75)));
}

/// Rotate an ecliptic direction into view space (x right, y up, z toward eye).
fn view_dir(v: [f64; 3], yaw: f32, pitch: f32) -> [f32; 3] {
    let (cy, sy) = ((yaw as f64).cos(), (yaw as f64).sin());
    let (cp, sp) = ((pitch as f64).cos(), (pitch as f64).sin());
    let x1 = v[0] * cy - v[1] * sy;
    let y1 = v[0] * sy + v[1] * cy;
    let y2 = y1 * cp - v[2] * sp;
    let depth = y1 * sp + v[2] * cp;
    [x1 as f32, y2 as f32, depth as f32]
}

const OBLIQ_DEG: [f32; 9] = [0.03, 177.4, 23.44, 25.19, 3.13, 26.73, 97.77, 28.32, 122.53];

fn h3(x: f32, y: f32, z: f32) -> f32 {
    let n = (x * 12.9898 + y * 78.233 + z * 37.719).sin() * 43758.5453;
    n - n.floor()
}
fn n3(x: f32, y: f32, z: f32) -> f32 {
    0.55 * h3(x * 3.1, y * 3.1, z * 3.1) + 0.30 * h3(x * 7.3, y * 7.3, z * 7.3) + 0.15 * h3(x * 15.7, y * 15.7, z * 15.7)
}
fn mixc(a: [u8; 3], b: [u8; 3], t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    )
}

/// Surface colour of a lidar point in BODY-FIXED coordinates (x,y,z unit).
fn lidar_color(pal: u8, x: f32, y: f32, z: f32) -> Color32 {
    let n = n3(x, y, z);
    match pal {
        0 => mixc([115, 115, 118], [185, 185, 188], n),
        1 => mixc([212, 192, 148], [242, 226, 182], n),
        2 => {
            if z.abs() > 0.88 {
                Color32::from_rgb(240, 245, 250)
            } else if n > 0.56 {
                mixc([58, 108, 44], [148, 128, 78], (n - 0.56) * 4.0)
            } else if h3(x * 9.1, y * 9.1, z * 9.1) > 0.86 {
                Color32::from_rgb(250, 250, 252)
            } else {
                mixc([18, 58, 138], [42, 98, 178], n * 1.6)
            }
        }
        3 => {
            if z.abs() > 0.90 {
                Color32::from_rgb(245, 245, 238)
            } else if n > 0.62 {
                mixc([108, 64, 44], [140, 84, 58], n)
            } else {
                mixc([176, 94, 58], [212, 132, 84], n * 1.4)
            }
        }
        4 => {
            let t = (z * 9.0).sin() * 0.5 + 0.5;
            let mut c = mixc([162, 122, 88], [236, 224, 202], t * 0.7 + n * 0.3);
            let lon = y.atan2(x);
            if (z + 0.32).abs() < 0.16 && lon.abs() < 0.55 {
                c = mixc([188, 88, 58], [212, 116, 78], n);
            }
            c
        }
        5 => {
            let t = (z * 7.0).sin() * 0.5 + 0.5;
            mixc([196, 168, 118], [240, 224, 178], t * 0.6 + n * 0.4)
        }
        6 => mixc([138, 208, 214], [176, 228, 228], n * 0.5 + 0.25 * ((z * 4.0).sin() * 0.5 + 0.5)),
        7 => {
            if n > 0.80 {
                Color32::from_rgb(232, 238, 250)
            } else {
                mixc([58, 98, 202], [96, 138, 232], n)
            }
        }
        8 => {
            if n > 0.68 {
                mixc([84, 64, 54], [112, 86, 70], n)
            } else if z > 0.55 {
                Color32::from_rgb(234, 228, 218)
            } else {
                mixc([196, 164, 124], [226, 204, 172], n * 1.3)
            }
        }
        9 => mixc([212, 222, 238], [246, 250, 255], n),
        10 => {
            let t = (z * 8.0).sin() * 0.5 + 0.5;
            mixc([168, 128, 92], [228, 212, 186], t * 0.6 + n * 0.4)
        }
        _ => mixc([138, 122, 108], [186, 168, 148], n),
    }
}

/// "LIDAR scan" planet: fibonacci point sphere, spun by the sidereal period,
/// tilted by the obliquity, lit from the Sun, culled to the front hemisphere.
#[allow(clippy::too_many_arguments)]
fn lidar_sphere(
    p: &egui::Painter,
    c: Pos2,
    radius_px: f32,
    pal: u8,
    spin: f64,
    obliq: f32,
    yaw: f32,
    pitch: f32,
    light: [f32; 3],
) {
    let n = (radius_px * 12.0).clamp(300.0, 2600.0) as usize;
    let ga = 2.399_963_23f32;
    let (cs, ss) = (spin.cos() as f32, spin.sin() as f32);
    let (co, so) = (obliq.cos(), obliq.sin());
    let (cy, sy) = (yaw.cos(), yaw.sin());
    let (cp, sp) = (pitch.cos(), pitch.sin());
    for i in 0..n {
        let zf = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let rr = (1.0 - zf * zf).max(0.0).sqrt();
        let th = i as f32 * ga;
        let (bx, by, bz) = (rr * th.cos(), rr * th.sin(), zf);
        let col = lidar_color(pal, bx, by, bz); // body-fixed → surface features
        // spin about the body axis, then obliquity tilt about x
        let sx = bx * cs - by * ss;
        let sy2 = bx * ss + by * cs;
        let ty = sy2 * co - bz * so;
        let tz = sy2 * so + bz * co;
        // into camera view (same convention as Proj)
        let x1 = sx * cy - ty * sy;
        let y1 = sx * sy + ty * cy;
        let depth = y1 * sp + tz * cp;
        if depth <= 0.03 {
            continue; // back hemisphere
        }
        let y2 = y1 * cp - tz * sp;
        let lam = (x1 * light[0] + y2 * light[1] + depth * light[2]).clamp(0.0, 1.0);
        let shade = 0.22 + 0.78 * lam;
        let sz = 1.3 + 0.7 * depth;
        p.rect_filled(
            Rect::from_center_size(
                Pos2::new(c.x + x1 * radius_px, c.y - y2 * radius_px),
                Vec2::splat(sz),
            ),
            0.0,
            col.linear_multiply(shade * (0.5 + 0.5 * depth)),
        );
    }
}

/// Nearest hover candidate: (screen distance, name, screen position).
type Pick = Option<(f32, String, Pos2)>;

fn offer(pick: &mut Pick, d: f32, name: &str, sp: Pos2, radius: f32) {
    if d < radius && pick.as_ref().map(|(pd, _, _)| d < *pd).unwrap_or(true) {
        *pick = Some((d, name.to_string(), sp));
    }
}

/// Layered-alpha star glow ("make stars glow a little"): the glow backend has
/// no shader blur, so concentric translucent discs fake the falloff.
fn star_glow(p: &egui::Painter, c: Pos2, col: Color32, r_core: f32, boost: f32) {
    p.circle_filled(c, r_core * 5.5, col.linear_multiply(0.045 * boost));
    p.circle_filled(c, r_core * 3.4, col.linear_multiply(0.08 * boost));
    p.circle_filled(c, r_core * 2.1, col.linear_multiply(0.15 * boost));
    p.circle_filled(c, r_core * 1.35, col.linear_multiply(0.30 * boost));
    p.circle_filled(c, r_core, col.linear_multiply((0.30 + 0.70 * boost).clamp(0.0, 1.0)));
    p.circle_stroke(c, r_core, Stroke::new(1.0_f32, GRID_HI.linear_multiply(boost.clamp(0.15, 1.0))));
}

/// Blurred deep-sky blob: layered translucent discs simulate distance haze.
fn blur_blob(p: &egui::Painter, c: Pos2, r: f32, col: Color32, bright: f32) {
    p.circle_filled(c, r * 2.6, col.linear_multiply(0.06 * bright));
    p.circle_filled(c, r * 1.9, col.linear_multiply(0.11 * bright));
    p.circle_filled(c, r * 1.3, col.linear_multiply(0.20 * bright));
    p.circle_filled(c, r * 0.8, col.linear_multiply(0.40 * bright));
    p.circle_filled(c, r * 0.4, Color32::WHITE.linear_multiply(0.22 * bright));
}

fn deepsky_style(class: &str) -> (Color32, f32) {
    match class {
        "PULSAR" => (Color32::from_rgb(178, 160, 255), 2.2),
        "NEUTRONSTAR" => (Color32::from_rgb(200, 205, 255), 1.9),
        "MAGNETAR" => (Color32::from_rgb(255, 140, 220), 2.5),
        "QUASAR" => (Color32::from_rgb(235, 120, 255), 2.7),
        "SNR" => (Color32::from_rgb(120, 230, 200), 5.5),
        "PNEB" => (Color32::from_rgb(140, 230, 255), 4.2),
        "HII" => (Color32::from_rgb(255, 140, 160), 5.0),
        "DARK" => (Color32::from_rgb(90, 88, 100), 4.5),
        "CLUSTER" => (Color32::from_rgb(255, 240, 210), 3.6),
        "DWARF" => (Color32::from_rgb(150, 170, 220), 5.5),
        "STAR" => (Color32::from_rgb(255, 250, 235), 2.0),
        "WHITEDWARF" => (Color32::from_rgb(215, 230, 255), 1.5),
        "BROWNDWARF" => (Color32::from_rgb(200, 140, 110), 1.4),
        "BLACKHOLE" => (Color32::from_rgb(255, 190, 90), 3.0),
        _ => (Color32::from_rgb(255, 170, 120), 2.4), // MICROQUASAR etc.
    }
}

/// Blue spiral glyph: two logarithmic arms + bulge, flattened by a per-object
/// inclination and rotated by a per-object position angle (deterministic).
fn galaxy_glyph(p: &egui::Painter, c: Pos2, size: f32, seed: f32, morph: &str, dim: f32) {
    let rot = seed * 6.283;
    let squash = 0.45 + 0.45 * ((seed * 37.0).sin().abs());
    let (ca, sa) = (rot.cos(), rot.sin());
    let map = |x: f32, y: f32| {
        let yy = y * squash;
        Pos2::new(c.x + x * ca - yy * sa, c.y + x * sa + yy * ca)
    };
    // halo blur first
    p.circle_filled(c, size * 0.85, GAL_BLUE.linear_multiply(0.05 * dim));
    if morph == "S" || morph == "I" {
        let arms = if morph == "S" { 2 } else { 3 };
        for arm in 0..arms {
            let off = arm as f32 * std::f32::consts::TAU / arms as f32;
            let mut prev: Option<Pos2> = None;
            for k in 0..26 {
                let th = k as f32 / 25.0 * 3.4 * std::f32::consts::PI * 0.5 + off;
                let rr = size * (0.12 + 0.88 * k as f32 / 25.0);
                let pt = map(th.cos() * rr, th.sin() * rr);
                if let Some(pv) = prev {
                    // wide faint pass = blur, then the crisp arm
                    p.line_segment([pv, pt], Stroke::new(2.4_f32, GAL_BLUE.linear_multiply(0.07 * dim)));
                    p.line_segment([pv, pt], Stroke::new(1.0_f32, GAL_BLUE.linear_multiply(0.30 * dim)));
                }
                prev = Some(pt);
            }
        }
    }
    // bulge / elliptical core (E galaxies are core-only)
    let core_col = if morph == "E" {
        Color32::from_rgb(190, 200, 255)
    } else {
        Color32::from_rgb(225, 235, 255)
    };
    let cr = if morph == "E" { size * 0.55 } else { size * 0.30 };
    p.circle_filled(c, cr * 1.8, core_col.linear_multiply(0.10 * dim));
    p.circle_filled(c, cr, core_col.linear_multiply(0.55 * dim));
}

/// Two-part comet tail (session 5): straight anti-solar ION tail (cyan) plus a
/// curved DUST tail lagging along the orbit (amber), both alpha-tapered.
fn comet_tails(
    p: &egui::Painter,
    sp: Pos2,
    sun_sp: Pos2,
    vdir: Option<Vec2>,
    act: f32,
    len_scale: f32,
    bright: f32,
) {
    if act <= 0.02 {
        return;
    }
    let anti = sp - sun_sp;
    let la = anti.length().max(1e-3);
    let ion = anti / la;
    // tails shrink with zoom-out (they are ~0.1-2 AU structures); a small floor
    // keeps them findable, but never a 60 px wedge at full-system zoom
    let l_ion = (act * 150.0 * len_scale).clamp(3.0, 320.0);
    // ion tail: three slightly fanned strokes, anti-solar
    for k in 0..3 {
        let spread = (k as f32 - 1.0) * 0.055;
        let dir = Vec2::new(
            ion.x * spread.cos() - ion.y * spread.sin(),
            ion.x * spread.sin() + ion.y * spread.cos(),
        );
        let w = 2.2 - k as f32 * 0.6;
        p.line_segment(
            [sp, sp + dir * l_ion * (0.85 + 0.15 * k as f32)],
            Stroke::new(w, CYAN.linear_multiply((0.10 + 0.30 * act) * bright)),
        );
    }
    // dust tail: quadratic bezier bending from anti-solar toward -velocity
    let vd = vdir.unwrap_or(Vec2::new(-ion.y, ion.x));
    let l_dust = (act * 110.0 * len_scale).clamp(2.5, 240.0);
    let ctrl = sp + ion * l_dust * 0.55 - vd * l_dust * 0.35;
    let end = sp + ion * l_dust * 0.92 - vd * l_dust * 0.85;
    let mut prev = sp;
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        let mt = 1.0 - t;
        let w0 = mt * mt;
        let w1 = 2.0 * mt * t;
        let w2 = t * t;
        let pt = Pos2::new(
            sp.x * w0 + ctrl.x * w1 + end.x * w2,
            sp.y * w0 + ctrl.y * w1 + end.y * w2,
        );
        let w = 2.6 * (1.0 - t * 0.7);
        p.line_segment(
            [prev, pt],
            Stroke::new(w, AMBER.linear_multiply((0.30 + 0.35 * act) * bright * (1.0 - t * 0.65))),
        );
        prev = pt;
    }
}

/// The deep-sky backdrop: galaxies (blue spirals) → deep-sky objects (blurred)
/// → other featured systems (pink circles). Returns (hover tag, system marker
/// positions) for the caller. `sys` = current system index (0 = Sol).
fn draw_sky_layer(
    p: &egui::Painter,
    rect: Rect,
    st: &AppState,
    cat: &Catalog,
    sys: usize,
    sky: &Sky,
    hover: Option<Pos2>,
    dim: f32,
) -> (Option<(String, Pos2)>, Vec<(usize, Pos2)>) {
    let mut tag: Option<(f32, String, Pos2)> = None;
    let mut near = |sp: Pos2, label: String| {
        if let Some(h) = hover {
            let d = (h - sp).length();
            if d < 14.0 && tag.as_ref().map(|(td, _, _)| d < *td).unwrap_or(true) {
                tag = Some((d, label, sp));
            }
        }
    };

    // ---- galaxies: ≤100 blue spirals ---------------------------------------
    if st.show_galaxies {
        for (gi, g) in deep_sky::galaxies().iter().enumerate() {
            let (name, morph, ra, dec, dist_mly, size_arcmin) = *g;
            let Some(sp) = sky.project(ra, dec) else { continue };
            if !rect.expand(20.0).contains(sp) {
                continue;
            }
            // apparent-size driven glyph, damped for the far field
            let far = (dist_mly as f32 / 100.0).min(1.6);
            let size = ((1.6 + (size_arcmin as f32).sqrt() * 0.85) / (1.0 + far * 0.55)).clamp(2.0, 13.0);
            // session-6 depth cue: the sky sphere must read as INFINITY —
            // fade + shrink it as the camera dollies into the planetary scale
            let gdim = ((1.25 - far * 0.35).clamp(0.35, 1.0)) * dim;
            galaxy_glyph(p, sp, size * (0.55 + 0.45 * dim), (gi as f32 * 0.6180339887).fract(), morph, gdim);
            near(sp, format!("{} · GALAXY {} · {}", name, morph, deep_sky::dist_str(dist_mly)));
        }
    }

    // ---- deep-sky objects: 100 blurred --------------------------------------
    if st.show_deep_sky {
        for d in deep_sky::deep_sky().iter() {
            let (name, class, ra, dec, dist_mly) = *d;
            let Some(sp) = sky.project(ra, dec) else { continue };
            if !rect.expand(12.0).contains(sp) {
                continue;
            }
            let (col, r) = deepsky_style(class);
            let bright = dim;
            let rr = r * (0.75 + 0.25 * dim);
            if class == "BLACKHOLE" {
                // session 6: accretion-disk icon (dark core + lensed disk)
                accretion_disk(p, sp, rr * 1.35, (ra as f32).fract());
                let _ = col;
            } else if class == "STAR" || class == "WHITEDWARF" || class == "BROWNDWARF" {
                // stars glow a little too
                star_glow(p, sp, col, rr * 0.75, 0.55 * dim);
            } else {
                blur_blob(p, sp, rr, col, bright);
            }
            near(sp, format!("{} · {} · {}", name, class, deep_sky::dist_str(dist_mly)));
        }
    }

    // ---- other featured systems: pink circles --------------------------------
    let mut markers: Vec<(usize, Pos2)> = Vec::new();
    if st.show_systems {
        // In Sol view: the 10 exo hosts. In an exo view: SOL (opposite the host
        // direction) + every other system.
        let mut entries: Vec<(usize, f64, f64)> = Vec::new();
        for i in 0..cat.systems.len() + 1 {
            if i == sys {
                continue;
            }
            if i == 0 {
                if sys > 0 {
                    let h = &cat.systems[sys - 1].host;
                    if let (Some(ra), Some(dec)) = (h.ra_deg, h.dec_deg) {
                        entries.push((0, (ra + 180.0) % 360.0, -dec));
                    }
                }
                continue;
            }
            let h = &cat.systems[i - 1].host;
            if let (Some(ra), Some(dec)) = (h.ra_deg, h.dec_deg) {
                entries.push((i, ra, dec));
            }
        }
        for (i, ra, dec) in entries {
            let Some(sp) = sky.project(ra, dec) else { continue };
            if !rect.expand(16.0).contains(sp) {
                continue;
            }
            let label = if i == 0 { "SOL (OBSERVER)" } else { &cat.systems[i - 1].host.name };
            let dist = if i == 0 {
                String::new()
            } else {
                cat.systems[i - 1]
                    .host
                    .dist_ly
                    .map(|d| format!(" · {} LY", d as u64))
                    .unwrap_or_default()
            };
            let mdim = 0.30 + 0.70 * dim; // same infinity cue as the rest of the sky
            p.circle_filled(sp, 1.6, SYS_PINK.linear_multiply(0.55 * mdim));
            p.circle_stroke(sp, 4.0, Stroke::new(1.2_f32, SYS_PINK.linear_multiply(0.9 * mdim)));
            if dim > 0.55 {
                p.text(
                    sp + Vec2::new(7.0, -6.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:02}", i),
                    font(10.0),
                    SYS_PINK.linear_multiply(0.75 * mdim),
                );
            }
            markers.push((i, sp));
            near(sp, format!("{} · SYSTEM{:02}{} · CLICK=JUMP", label, i, dist));
        }
    }

    (tag.map(|(_, t, s)| (t, s)), markers)
}

/// Canvas legend, top-left (session-5 request): glyph → class, with live counts.
fn draw_legend(ui: &mut Ui, p: &egui::Painter, rect: Rect, st: &AppState, y_off: f32) -> bool {
    let x0 = rect.left() + 8.0;
    let y0 = rect.top() + y_off;
    let row_h = 13.5;
    let col_w = 198.0;
    let rows_l: [(&str, fn(&egui::Painter, Pos2)); 6] = [
        ("PLANET / STAR (GLOW)", |p, c| star_glow(p, c, AMBER, 2.4, 1.0)),
        ("MOON", |p, c| { p.circle_filled(c, 1.6, Color32::from_gray(210)); }),
        ("ASTEROID (SBDB REAL)", |p, c| {
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(2.6)), 0.0, Color32::from_rgb(200, 160, 110));
        }),
        ("COMET + TAILS", |p, c| {
            p.circle_filled(c, 1.8, Color32::from_rgb(180, 230, 230));
            p.line_segment([c, c + Vec2::new(9.0, -3.0)], Stroke::new(1.0_f32, AMBER.linear_multiply(0.7)));
            p.line_segment([c, c + Vec2::new(8.0, 3.0)], Stroke::new(1.0_f32, CYAN.linear_multiply(0.7)));
        }),
        ("PHA IMPACT RISK", |p, c| {
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(3.0)), 0.0, ALARM);
            p.circle_stroke(c, 6.0, Stroke::new(1.0_f32, ALARM.linear_multiply(0.8)));
        }),
        ("NEAR-EARTH (NEO)", |p, c| {
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(2.4)), 0.0, Color32::from_rgb(200, 160, 110));
            p.circle_stroke(c, 5.0, Stroke::new(1.0_f32, AMBER.linear_multiply(0.7)));
        }),
    ];
    let rows_r: [(&str, fn(&egui::Painter, Pos2)); 6] = [
        ("PROBE (SPACECRAFT)", |p, c| diamond(p, c, 3.0, PROBE_GREEN)),
        ("OTHER SYSTEM", |p, c| {
            p.circle_filled(c, 1.6, SYS_PINK.linear_multiply(0.55));
            p.circle_stroke(c, 4.0, Stroke::new(1.2_f32, SYS_PINK.linear_multiply(0.9)));
        }),
        ("GALAXY (SPIRAL)", |p, c| galaxy_glyph(p, c, 5.0, 0.13, "S", 1.0)),
        ("DEEP SKY (BLURRED)", |p, c| blur_blob(p, c, 3.0, Color32::from_rgb(140, 230, 255), 1.0)),
        ("BELT DUST", |p, c| {
            for k in 0..5 {
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::new(k as f32 * 2.4 - 4.8, (k % 2) as f32 * 2.0 - 1.0), Vec2::splat(1.2)),
                    0.0,
                    Color32::from_gray(64),
                );
            }
        }),
        ("EXT CATALOGUE BODY", |p, c| {
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(2.4)), 0.0, Color32::from_rgb(200, 160, 110));
        }),
    ];
    let h = 20.0 + rows_l.len() as f32 * row_h + 16.0;
    let w = col_w * 2.0 + 10.0;
    let boxr = Rect::from_min_size(Pos2::new(x0, y0), Vec2::new(w, h));
    p.rect_filled(boxr, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 210));
    p.rect_stroke(boxr, 0.0, Stroke::new(1.0_f32, GRID), egui::StrokeKind::Outside);
    p.text(boxr.left_top() + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, "LEGEND — OBJECT CLASSES", font(PX_SM), GRID_HI);
    for (col_i, rows) in [rows_l.as_slice(), rows_r.as_slice()].iter().enumerate() {
        for (ri, (label, glyph)) in rows.iter().enumerate() {
            let c = Pos2::new(x0 + 16.0 + col_i as f32 * col_w, y0 + 28.0 + ri as f32 * row_h);
            glyph(p, c);
            p.text(c + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER, *label, font(10.5), GRID);
        }
    }
    let counts = format!(
        "SBDB CORE {} · EXT {}/{} · PROBES {} · SKY {} · GAL {} · SYS 10 — ALL REAL CATALOGUE DATA",
        small_bodies().len(),
        st.density,
        population().len(),
        probes().len(),
        deep_sky::deep_sky().len(),
        deep_sky::galaxies().len(),
    );
    p.text(
        Pos2::new(x0 + 6.0, boxr.bottom() - 13.0),
        egui::Align2::LEFT_TOP,
        counts,
        font(10.0),
        GRAY,
    );
    // collapse handle (session 6)
    let cr = Rect::from_min_size(boxr.right_top() + Vec2::new(-18.0, 2.0), Vec2::new(16.0, 16.0));
    p.rect_filled(cr, 0.0, CELL);
    p.rect_stroke(cr, 0.0, Stroke::new(1.0_f32, GRID), egui::StrokeKind::Outside);
    p.text(cr.center(), egui::Align2::CENTER_CENTER, "–", font(PX_SM), GRID_HI);
    crate::widgets::tip(ui.interact(cr, ui.id().with("legend_close"), Sense::click()), st.tips,
        "Hide this legend. Press L or the LEGEND chip to bring it back.")
        .clicked()
}

/// Small `LEGEND +` chip shown when the legend overlay is collapsed.
fn legend_expand_chip(ui: &mut Ui, p: &egui::Painter, rect: Rect, tips: bool) -> bool {
    let r = Rect::from_min_size(rect.left_top() + Vec2::new(8.0, 34.0), Vec2::new(66.0, 16.0));
    p.rect_filled(r, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 210));
    p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, GRID), egui::StrokeKind::Outside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, "LEGEND +", font(10.0), GRID_HI);
    crate::widgets::tip(ui.interact(r, ui.id().with("legend_open"), Sense::click()), tips,
        "Show the legend: what every symbol and colour on this canvas means.")
        .clicked()
}

/// Green 45°-square (diamond) probe glyph.
fn diamond(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(c.x, c.y - s),
            Pos2::new(c.x + s, c.y),
            Pos2::new(c.x, c.y + s),
            Pos2::new(c.x - s, c.y),
        ],
        col,
        Stroke::new(0.7_f32, col.linear_multiply(0.55)),
    ));
}

pub fn draw_scene(
    ui: &mut Ui,
    st: &mut AppState,
    jd: f64,
    cache: &mut SceneCache,
    center_icrs: [f64; 3],
    cat: &Catalog,
    sys: usize,
    frame: [[f64; 3]; 3],
) {
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    paint_cell(&p, rect, BG, STROKE_GRID);
    p.text(
        rect.left_top() + Vec2::new(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        "3D ORRERY VIEW",
        font(PX_SM),
        GRAY,
    );

    cache.ensure(jd);

    let resp = crate::widgets::tip(
        ui.interact(rect, ui.id().with("scene"), Sense::click_and_drag()),
        st.tips,
        "Space view. Click a body to select it, double-click to zoom onto it,\nleft-drag to orbit the camera, right-drag to slide, wheel to zoom.",
    );
    let hover = ui.ctx().pointer_hover_pos().filter(|h| rect.contains(*h));
    // remember where a press started — used to tell true clicks from the tail
    // end of an orbit drag (session-5 "camera jumps to Sol" fix, part 2)
    if hover.is_some() && ui.input(|i| i.pointer.primary_pressed()) {
        st.click_origin = hover.map(|h| [h.x, h.y]);
    }

    // ---- camera input (R9) ---------------------------------------------------
    // Auto-fit baseline for THIS view (session-4 fix kept).
    let fit_scale = if sys > 0 {
        let amax = cat.systems[sys - 1]
            .planets
            .iter()
            .map(|pl| pl.a_au)
            .fold(0.0f64, f64::max)
            .max(0.05);
        (rect.width().min(rect.height()) * 0.42) / amax as f32
    } else {
        (rect.width().min(rect.height()) * 0.46) / 30.2
    };
    if hover.is_some() {
        let dz = ui.input(|i| i.raw_scroll_delta.y);
        if dz != 0.0 {
            let base = if st.zoom <= 0.0 { fit_scale } else { st.zoom };
            st.zoom = (base * (1.0 + dz * 0.001)).clamp(2.0, 400_000.0);
        }
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        match st.cam {
            CamMode::Free => {
                if ui.input(|i| i.pointer.secondary_down()) || ui.input(|i| i.pointer.middle_down()) {
                    st.pan[0] += d.x / st.zoom.max(1.0); // grab-the-world
                    st.pan[1] += d.y / st.zoom.max(1.0);
                } else {
                    // rotate about the FOCUS body
                    st.cam_yaw -= d.x * 0.005;
                    st.cam_pitch = (st.cam_pitch + d.y * 0.005).clamp(-1.55, 1.55);
                }
            }
            _ => {
                st.pan[0] += d.x / st.zoom.max(1.0);
                st.pan[1] += d.y / st.zoom.max(1.0);
            }
        }
        st.auto_tour = false; // any manual camera move drops AUTO (§8.3)
    }

    let (yaw, pitch) = match st.cam {
        CamMode::Free => (st.cam_yaw, st.cam_pitch),
        _ => (0.0, 0.0),
    };
    let fit = fit_scale;
    let scale = if st.zoom <= 0.0 { fit } else { st.zoom.clamp(fit * 0.02, 400_000.0) };
    // infinity cue (session 6): sky-sphere layers fade as we dolly in
    let sky_dim = (fit / scale).clamp(0.05, 1.0).powf(0.5);

    // ---- sky-sphere backdrop (session 5): galaxies · deep sky · systems -------
    let sky = Sky {
        c: rect.center(),
        r: rect.width().min(rect.height()) * 0.45, // focal length [px] (~84° FOV)
        yaw,
        pitch,
        frame,
    };
    let (sky_tag, sys_markers) = draw_sky_layer(&p, rect, st, cat, sys, &sky, hover, sky_dim);
    if let Some((txt, sp)) = sky_tag {
        let w = (14.0 + txt.len() as f32 * 6.6).min(rect.width() - 20.0);
        let mut tr = Rect::from_min_size(sp + Vec2::new(10.0, -22.0), Vec2::new(w, 16.0));
        if tr.right() > rect.right() - 4.0 {
            tr = Rect::from_min_size(Pos2::new(sp.x - 10.0 - w, tr.top()), Vec2::new(w, 16.0));
        }
        p.rect_filled(tr, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 220));
        p.rect_stroke(tr, 0.0, Stroke::new(1.0_f32, GRID), egui::StrokeKind::Outside);
        p.text(tr.center(), egui::Align2::CENTER_CENTER, txt, font(10.5), GRID_HI);
    }

    // ---- exoplanet systems (R5): host-centred two-body scene ------------------
    if sys > 0 {
        draw_exo(ui, st, jd, cat, sys - 1, &resp, hover, rect, &p, frame, center_icrs, fit_scale, &sys_markers, sky_dim);
        return;
    }

    // ---- projection ----------------------------------------------------------
    let visual = st.scale == ScaleMode::Visual;
    let center_probe = Proj { c: rect.center(), scale: 1.0, yaw, pitch, visual, center: [0.0; 3] };
    let cshift = center_probe.shrink(&icrs_to_ecl(center_icrs));
    let proj = Proj {
        c: rect.center() + Vec2::new(st.pan[0] * scale, st.pan[1] * scale),
        scale,
        yaw,
        pitch,
        visual,
        center: cshift,
    };
    let focused = center_icrs != [0.0, 0.0, 0.0];

    // ecliptic grid rings
    if st.show_grid {
        for au in [1.0, 5.0, 10.0, 20.0, 30.0] {
            dotted_circle(&p, proj.c, au as f32 * scale, PLOT_FAINT);
        }
        p.line_segment(
            [Pos2::new(rect.left() + 4.0, proj.c.y), Pos2::new(rect.right() - 4.0, proj.c.y)],
            Stroke::new(1.0_f32, ROW_LINE),
        );
        p.line_segment(
            [Pos2::new(proj.c.x, rect.top() + 4.0), Pos2::new(proj.c.x, rect.bottom() - 4.0)],
            Stroke::new(1.0_f32, ROW_LINE),
        );
    }

    // orbit line-loops (§7)
    let sel_idx = st
        .selected_body
        .as_deref()
        .and_then(|n| PLANET_NAMES.iter().position(|x| *x == n));
    if st.show_orbits {
        for (i, loop_) in cache.orbits.iter().enumerate() {
            let col = if Some(i) == sel_idx { CYAN } else { GRID.linear_multiply(0.55) };
            let pts: Vec<Pos2> = loop_.iter().map(|q| proj.to_screen_ecl(&frame_mul(&frame, q))).collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.0_f32, col)));
        }
    }

    let mut pick: Pick = None;

    // ---- belts FIRST so bodies sit on top ------------------------------------
    if st.show_belts {
        if let Some((main, kuiper)) = cache.belts.as_ref() {
            // LOD: thin the clouds when zoomed out; screen-space occupancy grid
            // kills sub-pixel clustering that aliased into flicker.
            let lod = ((scale / 30.0).clamp(0.08, 1.0)).powf(0.6);
            let dot = Color32::from_gray(64);
            let mut drawn: std::collections::HashSet<u32> = std::collections::HashSet::new();
            let cell = 3.0f32; // px occupancy cell
            let t_d = jd - JD_J2000;
            let draw_cloud = |cloud: &[[f64; 4]], proj: &Proj, p: &egui::Painter, drawn: &mut std::collections::HashSet<u32>| {
                let n_lod = ((cloud.len() as f32) * lod) as usize;
                let stride = (cloud.len() / n_lod.max(1)).max(1);
                let margin = rect.expand(20.0);
                for pp in cloud.iter().step_by(stride) {
                    let [a, l0, node, inc] = *pp;
                    let nrad = 0.017_202_098_95 / a.powf(1.5);
                    let lam = l0 + nrad * t_d;
                    let probe = proj.to_screen_ecl(&frame_mul(&frame, &[a * lam.cos(), a * lam.sin(), 0.0]));
                    if !margin.contains(probe) {
                        continue;
                    }
                    let u = lam - node;
                    let (su, cu) = (u.sin(), u.cos());
                    let (sn, cn) = (node.sin(), node.cos());
                    let (si, ci) = (inc.sin(), inc.cos());
                    let pos = [
                        a * (cn * cu - sn * su * ci),
                        a * (sn * cu + cn * su * ci),
                        a * (su * si),
                    ];
                    let sp = proj.to_screen_ecl(&frame_mul(&frame, &pos));
                    if !rect.contains(sp) {
                        continue;
                    }
                    let key = ((sp.x / cell) as u32) | (((sp.y / cell) as u32) << 16);
                    if !drawn.insert(key) {
                        continue;
                    }
                    p.rect_filled(
                        Rect::from_center_size(Pos2::new(sp.x.round() + 0.5, sp.y.round() + 0.5), Vec2::splat(1.2)),
                        0.0,
                        dot,
                    );
                }
            };
            draw_cloud(main, &proj, &p, &mut drawn);
            draw_cloud(kuiper, &proj, &p, &mut drawn);
        }
    }

    // Sun — glow (session 5: "make stars glow a little"), still the focus anchor
    let sun_sp = proj.to_screen_ecl(&frame_mul(&frame, &[0.0, 0.0, 0.0]));
    star_glow(&p, sun_sp, AMBER, 4.0, 1.0);
    let sun_sel = st.selected_body.as_deref() == Some(crate::body_data::SUN);
    if sun_sel {
        p.circle_stroke(sun_sp, 9.0, Stroke::new(1.0_f32, CYAN));
    }
    if st.show_labels || sun_sel {
        p.text(sun_sp + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, "SOL", font(PX_SM), AMBER);
    }
    if let Some(h) = hover {
        offer(&mut pick, (h - sun_sp).length(), crate::body_data::SUN, sun_sp, 14.0);
    }

    // planets (toggle: PLANETS)
    if st.show_planets {
        for (i, id) in PLANET_IDS.iter().enumerate() {
            let s = planet_state_ecl(*id, jd);
            let sp = proj.to_screen_ecl(&frame_mul(&frame, &s.pos));
            let rgb = PLANET_RGB[i];
            let col = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            // session 8: markers enlarged (~2x area) so planets read at a glance
            let rad = (4.0 + 2.6 * ((crate::telemetry::PLANET_RADIUS_KM[i] / 1000.0).log10() as f32)).clamp(4.0, 12.0);
            // session 6: zoomed in far enough that the TRUE radius resolves →
            // render the body as a lidar-style 3-D point sphere (spun by its
            // sidereal period, tilted by its obliquity, lit from the Sun)
            let apparent = (crate::telemetry::PLANET_RADIUS_KM[i] / AU_KM * scale as f64) as f32;
            if st.scale == ScaleMode::True && apparent >= 7.0 {
                let rot_d = crate::body_data::PLANET_ROT_D[i];
                let spin = if rot_d.abs() > 1e-6 {
                    let dir = if rot_d < 0.0 { -1.0 } else { 1.0 };
                    (jd.rem_euclid(rot_d.abs()) / rot_d.abs()) * std::f64::consts::TAU * dir
                } else {
                    0.0
                };
                let pv = frame_mul(&frame, &s.pos);
                let pn = (pv[0].powi(2) + pv[1].powi(2) + pv[2].powi(2)).sqrt().max(1e-9);
                let light = view_dir([-pv[0] / pn, -pv[1] / pn, -pv[2] / pn], yaw, pitch);
                lidar_sphere(&p, sp, apparent, [0, 1, 2, 3, 4, 5, 6, 7, 8][i], spin, OBLIQ_DEG[i].to_radians(), yaw, pitch, light);
            } else {
                p.circle_filled(sp, rad, col);
            }
            let is_sel = sel_idx == Some(i);
            if is_sel {
                let tag = PLANET_NAMES[i];
                let w = 10.0 + tag.len() as f32 * 9.0;
                let tr = Rect::from_min_size(sp + Vec2::new(8.0, -20.0), Vec2::new(w, 16.0));
                p.rect_filled(tr, 0.0, CYAN);
                p.rect_stroke(tr, 0.0, STROKE_HI, egui::StrokeKind::Outside);
                p.text(tr.center(), egui::Align2::CENTER_CENTER, tag, font(PX_SM), INK);
                p.line_segment([sp, tr.left_bottom()], Stroke::new(1.0_f32, CYAN));
            } else if st.show_labels && rect.contains(sp) {
                p.text(sp + Vec2::new(7.0, -7.0), egui::Align2::LEFT_BOTTOM, PLANET_NAMES[i], font(PX_SM), GRID);
            }
            if let Some(h) = hover {
                offer(&mut pick, (h - sp).length(), PLANET_NAMES[i], sp, 14.0);
            }
        }
    }

    // moons (toggle: MOONS) — cull below 3 px separation
    if st.show_moons {
        for m in moon_defs().iter() {
            let sep_px = (m.a_km / AU_KM) * scale as f64;
            if sep_px < 3.0 {
                continue;
            }
            let sstate = orrery_astro::moons::moon_state(m, jd);
            let sp = proj.to_screen(&frame_mul(&frame, &sstate.pos));
            if !rect.expand(10.0).contains(sp) {
                continue;
            }
            let pe = planet_state_ecl(m.parent, jd).pos;
            let psp = proj.to_screen_ecl(&frame_mul(&frame, &pe));
            let rpx = (sp - psp).length();
            let moon_sel = st.selected_body.as_deref() == Some(m.name);
            if st.show_orbits && rpx >= 5.0 && rpx < 3_500.0 {
                // session-6 fix: the mean circle is a real inclined 3-D loop,
                // projected through the camera — the old screen-space circle
                // stayed flat when FREE-pitching onto a planet
                let rr = m.a_km / AU_KM;
                let (si, ci) = (m.i_deg.to_radians().sin(), m.i_deg.to_radians().cos());
                let pts: Vec<Pos2> = (0..=48)
                    .map(|k| {
                        let lam = std::f64::consts::TAU * k as f64 / 48.0;
                        let off = [rr * lam.cos(), rr * lam.sin() * ci, rr * lam.sin() * si];
                        proj.to_screen_ecl(&frame_mul(&frame, &[pe[0] + off[0], pe[1] + off[1], pe[2] + off[2]]))
                    })
                    .collect();
                p.add(egui::Shape::line(pts, Stroke::new(1.0_f32, if moon_sel { CYAN } else { GRID.linear_multiply(0.4) })));
            }
            let rad_km = orrery_astro::moons::moon_props(m.name).1;
            // session 8: moon dots enlarged with the planets
            let dot_r = if rad_km >= 1000.0 { 3.6 } else if rad_km >= 100.0 { 2.8 } else { 2.0 };
            p.circle_filled(sp, dot_r, Color32::from_gray(210));
            if st.show_labels && scale > 900.0 {
                p.text(sp + Vec2::new(4.0, -4.0), egui::Align2::LEFT_BOTTOM, m.name, font(PX_SM), GRAY);
            }
            if moon_sel {
                p.circle_stroke(sp, 6.0, Stroke::new(1.0_f32, CYAN));
            }
            if let Some(h) = hover {
                offer(&mut pick, (h - sp).length(), m.name, sp, 10.0);
            }
        }
    }

    // ---- exploration probes (session 5): green 45° squares ---------------------
    if st.show_probes {
        for pr in probes().iter() {
            let s = probe_state(pr, jd);
            let ecl = frame_mul(&frame, &icrs_to_ecl(s.pos));
            let sp = proj.to_screen_ecl(&ecl);
            let col = if pr.active { PROBE_GREEN } else { PROBE_DIM };
            if rect.contains(sp) {
                diamond(&p, sp, 3.0, col);
                if st.selected_body.as_deref() == Some(pr.name) {
                    p.circle_stroke(sp, 7.5, Stroke::new(1.0_f32, CYAN));
                    p.text(sp + Vec2::new(9.0, -7.0), egui::Align2::LEFT_BOTTOM, pr.name, font(PX_SM), PROBE_GREEN);
                } else if st.show_labels && scale < 60.0 {
                    p.text(sp + Vec2::new(5.0, -5.0), egui::Align2::LEFT_BOTTOM, pr.name, font(9.5), PROBE_DIM);
                }
                if let Some(h) = hover {
                    offer(&mut pick, (h - sp).length(), pr.name, sp, 9.0);
                }
            } else {
                // edge chip: clamp to the border so deep-space craft stay visible
                if scale > 60.0 {
                    continue; // zoomed in — chips would swarm
                }
                let inner = rect.shrink(16.0);
                let c = Pos2::new(sp.x.clamp(inner.left(), inner.right()), sp.y.clamp(inner.top(), inner.bottom()));
                diamond(&p, c, 2.2, col.linear_multiply(0.35));
                p.circle_stroke(c, 4.0, Stroke::new(1.0_f32, col.linear_multiply(0.55)));
                let to_left = c.x - rect.left() < rect.right() - c.x;
                p.text(
                    c + Vec2::new(if to_left { -6.0 } else { 6.0 }, 0.0),
                    if to_left { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER },
                    pr.name,
                    font(9.0),
                    col.linear_multiply(0.6),
                );
            }
        }
    }

    // ---- selected asteroid/comet orbit: CACHED (session-5 perf) ---------------
    // The osculating ellipse is static — bake once per selection, not per frame.
    let sel_body = st
        .selected_body
        .as_deref()
        .and_then(|n| small_bodies().iter().chain(population().iter()).find(|b| b.name == n))
        .map(|b| (b.name, b.el, b.pha));
    if let Some((name, el, pha)) = sel_body {
        if cache.sel_orbit.as_ref().map(|(n, _)| n != name).unwrap_or(true) {
            cache.sel_orbit = Some((name.to_string(), el.orbit_points_ecl(jd, 512)));
        }
        if let Some((_, pts)) = cache.sel_orbit.as_ref() {
            let pts: Vec<Pos2> = pts.iter().map(|q| proj.to_screen_ecl(&frame_mul(&frame, q))).collect();
            if pts.len() > 1 {
                let col = if pha { ALARM } else { CYAN };
                p.add(egui::Shape::line(pts, Stroke::new(1.0_f32, col)));
            }
        }
    } else if st.selected_body.is_none() {
        cache.sel_orbit = None;
    }

    // ---- asteroids + comets: real SBDB set + synthetic population -------------
    // DENSITY slider (20..1000) takes a PREFIX of the deterministically shuffled
    // synthetic population, so any cut shows a balanced mix of all families.
    let dens = st.density.clamp(20, population().len());
    if st.show_asteroids || st.show_comets {
        for b in small_bodies().iter() {
            draw_small_body(&p, b, jd, &proj, &frame, rect, scale, sun_sp, st, &mut pick, hover);
        }
        // session 8: the SELECTED minor body is always drawn, even when it sits
        // outside the DENSITY prefix — otherwise picking it from the FIND BODY
        // list would select something the map doesn't show
        let sel_name = st.selected_body.as_deref();
        for (k, b) in population().iter().enumerate() {
            if k >= dens && sel_name != Some(b.name) {
                continue;
            }
            draw_small_body(&p, b, jd, &proj, &frame, rect, scale, sun_sp, st, &mut pick, hover);
        }
    }

    // hover highlight: ring + name tag
    if let Some((_, name, sp)) = pick.as_ref() {
        p.circle_stroke(*sp, 10.0, Stroke::new(1.0_f32, CYAN.linear_multiply(0.85)));
        let w = 10.0 + name.len() as f32 * 8.0;
        let tr = Rect::from_min_size(*sp + Vec2::new(12.0, -24.0), Vec2::new(w, 16.0));
        p.rect_filled(tr, 0.0, CELL);
        p.rect_stroke(tr, 0.0, Stroke::new(1.0_f32, GRID), egui::StrokeKind::Outside);
        p.text(tr.center(), egui::Align2::CENTER_CENTER, name, font(PX_SM), GRID_HI);
    }

    // ---- selection (click selects, double-click tracks) ------------------------
    // Session-5 fix: an orbit drag that RELEASES inside the scene used to count
    // as a click on empty space → deselect → FREE focus target became the origin
    // → "camera jumps to Sol". Now: (a) system markers take priority, (b) a
    // clear-selection requires the pointer to have stayed put (≤ 4 px).
    if resp.clicked() && !resp.dragged() {
        st.auto_tour = false;
        let sys_hit = hover.and_then(|h| {
            sys_markers
                .iter()
                .find(|(_, sp)| (*sp - h).length() < 10.0)
                .map(|(i, _)| *i)
        });
        if let Some(i) = sys_hit {
            st.system = i;
            st.selected_body = None;
        } else if let Some((_, name, _)) = pick {
            let dbl = resp.double_clicked();
            st.selected_body = Some(name.clone());
            if dbl {
                track_and_zoom(st, &name, rect, cat, sys);
            }
        } else {
            let moved = match (st.click_origin, hover) {
                (Some(o), Some(h)) => ((h.x - o[0]).powi(2) + (h.y - o[1]).powi(2)).sqrt(),
                _ => 0.0,
            };
            if moved <= 4.0 {
                st.selected_body = None;
            }
        }
    }

    // ---- legend (canvas top-left, session 5; collapsible session 6) -----------
    if st.show_legend {
        if draw_legend(ui, &p, rect, st, 34.0) {
            st.show_legend = false;
        }
    } else if legend_expand_chip(ui, &p, rect, st.tips) {
        st.show_legend = true;
    }

    // mode banner (+ focus readout when orbiting a body)
    let mode = match st.cam {
        CamMode::Free => "FREE",
        CamMode::Track => "TRACK",
        CamMode::TopDown => "TOP-DOWN",
    };
    let focus = if focused {
        format!("  FOCUS {}", st.selected_body.as_deref().unwrap_or("---"))
    } else {
        String::new()
    };
    let scale_flag = if st.scale == ScaleMode::Visual { "  SCALE VIS (cosmetic)" } else { "" };
    p.text(
        rect.right_bottom() + Vec2::new(-10.0, -10.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("CAM {mode}{focus}  ZOOM {scale:.1} px/AU{scale_flag}"),
        font(PX_SM),
        GRAY,
    );
}

/// One asteroid/comet (real or synthetic): marker + comet tails + pick.
#[allow(clippy::too_many_arguments)]
fn draw_small_body(
    p: &egui::Painter,
    b: &SmallBody,
    jd: f64,
    proj: &Proj,
    frame: &[[f64; 3]; 3],
    rect: Rect,
    scale: f32,
    sun_sp: Pos2,
    st: &AppState,
    pick: &mut Pick,
    hover: Option<Pos2>,
) {
    // per-class toggles gate both the real and the synthetic sets
    match b.kind {
        Kind::Comet if !st.show_comets => return,
        Kind::Asteroid if !st.show_asteroids => return,
        _ => {}
    }
    let sstate = b.state(jd);
    let ecl = frame_mul(frame, &icrs_to_ecl(sstate.pos));
    let sp = proj.to_screen_ecl(&ecl);
    if !rect.contains(sp) {
        return;
    }
    let r_au = sstate.r_au();
    if b.kind == Kind::Comet {
        let act = b.activity(r_au);
        // tails need the screen-space velocity direction (dust lags the orbit)
        let vel_ecl = frame_mul(frame, &icrs_to_ecl(sstate.vel));
        let vsp = proj.to_screen_ecl(&[ecl[0] + vel_ecl[0] * 3.0, ecl[1] + vel_ecl[1] * 3.0, ecl[2] + vel_ecl[2] * 3.0]);
        let dv = vsp - sp;
        let vdir = (dv.length() > 1e-3).then(|| dv / dv.length().max(1e-6));
        let bright = 1.0;
        comet_tails(p, sp, sun_sp, vdir, act, (scale / 300.0).clamp(0.08, 2.2), bright);
        let core = if b.pha { ALARM } else { Color32::from_rgb(180, 230, 230) };
        let cr = 1.8 + 2.5 * act;
        p.circle_filled(
            sp,
            cr,
            Color32::from_rgba_premultiplied(core[0], core[1], core[2], 90 + (120.0 * act) as u8),
        );
    } else if b.pha {
        // potentially hazardous — red marker + ring (real bodies only)
        p.rect_filled(Rect::from_center_size(sp, Vec2::splat(3.0)), 0.0, ALARM);
        p.circle_stroke(sp, 6.5, Stroke::new(1.0_f32, ALARM.linear_multiply(0.8)));
    } else {
        p.rect_filled(Rect::from_center_size(sp, Vec2::splat(2.4)), 0.0, Color32::from_rgb(200, 160, 110));
        if b.neo {
            p.circle_stroke(sp, 5.0, Stroke::new(1.0_f32, AMBER.linear_multiply(0.6)));
        }
    }
    if st.selected_body.as_deref() == Some(b.name) {
        p.circle_stroke(sp, 7.5, Stroke::new(1.0_f32, if b.pha { ALARM } else { CYAN }));
        let tag = if b.pha { format!("{} · PHA", b.name) } else { b.name.to_string() };
        p.text(sp + Vec2::new(9.0, -7.0), egui::Align2::LEFT_BOTTOM, tag, font(PX_SM), if b.pha { ALARM } else { CYAN });
    }
    if let Some(h) = hover {
        offer(pick, (h - sp).length(), b.name, sp, 12.0);
    }
}

/// Double-click behaviour: TRACK + fit the zoom to the object's system scale.
fn track_and_zoom(st: &mut AppState, name: &str, rect: Rect, cat: &Catalog, sys: usize) {
    st.cam = CamMode::Track;
    st.pan = [0.0, 0.0];
    let base = rect.width().min(rect.height());
    let moon_a = moon_defs()
        .iter()
        .filter(|m| m.parent == orrery_astro::planets::id_from_name(name).unwrap_or(PlanetId::Mercury))
        .map(|m| m.a_km / AU_KM)
        .fold(0.0f64, f64::max);
    let target = if name == crate::body_data::SUN {
        0.05 // solar fly-by
    } else if moon_a > 0.0 {
        moon_a
    } else if let Some(m) = moon_defs().iter().find(|m| m.name == name) {
        (m.a_km / AU_KM).max(1e-5) // double-click a moon → its orbit fills the view
    } else if let Some(b) = small_bodies().iter().chain(population().iter()).find(|b| b.name == name) {
        b.el.a.max(0.05)
    } else if let Some(pr) = probes().iter().find(|p| p.name == name) {
        // probe: fit its heliocentric distance
        pr.el.a.max(0.05)
    } else if let Some(pl) = cat.systems.get(sys.saturating_sub(1)).and_then(|ss| ss.planets.iter().find(|pl| pl.name == name)) {
        pl.a_au.max(0.01)
    } else {
        0.004 // moonless planet: close fly-by
    };
    st.zoom = (base as f64 * 0.35 / target).clamp(4.0, 400_000.0) as f32;
}

/// Exoplanet system view: host star + catalog planets on Kepler two-body orbits
/// (§5.2), with the same sky-sphere backdrop + click semantics as the Sol scene.
#[allow(clippy::too_many_arguments)]
fn draw_exo(
    ui: &mut Ui,
    st: &mut AppState,
    jd: f64,
    cat: &Catalog,
    sys: usize,
    resp: &egui::Response,
    hover: Option<Pos2>,
    rect: Rect,
    p: &egui::Painter,
    frame: [[f64; 3]; 3],
    center_icrs: [f64; 3],
    fit_scale: f32,
    sys_markers: &[(usize, Pos2)],
    sky_dim: f32,
) {
    let s = &cat.systems[sys];
    let visual = st.scale == ScaleMode::Visual;
    let (yaw, pitch) = match st.cam {
        CamMode::Free => (st.cam_yaw, st.cam_pitch),
        _ => (0.0, 0.0),
    };
    let probe = Proj { c: rect.center(), scale: 1.0, yaw, pitch, visual, center: [0.0; 3] };
    let cshift = probe.shrink(&icrs_to_ecl(center_icrs));
    let fit = fit_scale;
    let scale = if st.zoom <= 0.0 { fit } else { st.zoom.clamp(fit * 0.02, 400_000.0) };
    let proj = Proj {
        c: rect.center() + Vec2::new(st.pan[0] * scale, st.pan[1] * scale),
        scale,
        yaw,
        pitch,
        visual,
        center: cshift,
    };
    let focused = center_icrs != [0.0, 0.0, 0.0];

    // host star colour from Teff — now with a glow (session 5)
    let teff = s.host.teff_k.unwrap_or(5500.0);
    let hc = if teff > 7000.0 {
        Color32::from_rgb(170, 190, 255)
    } else if teff > 6000.0 {
        Color32::from_rgb(255, 244, 214)
    } else if teff > 4900.0 {
        AMBER
    } else {
        Color32::from_rgb(224, 122, 95)
    };
    let hsp = proj.to_screen_ecl(&[0.0, 0.0, 0.0]);
    star_glow(p, hsp, hc, 4.5, 1.0);
    if st.show_labels {
        p.text(hsp + Vec2::new(11.0, -11.0), egui::Align2::LEFT_BOTTOM, s.host.name.as_str(), font(PX_SM), hc);
    }

    let mut pick: Pick = None;
    let cols: [Color32; 10] = [
        Color32::from_rgb(95, 165, 224), Color32::from_rgb(210, 96, 74), Color32::from_rgb(230, 200, 122),
        Color32::from_rgb(127, 212, 212), Color32::from_rgb(200, 160, 110), Color32::from_rgb(168, 168, 168),
        Color32::from_rgb(79, 110, 216), Color32::from_rgb(216, 192, 138), Color32::from_rgb(176, 152, 128),
        Color32::from_rgb(210, 212, 212),
    ];
    if st.show_planets {
        for (pi, pl) in s.planets.iter().enumerate() {
            let el = cat.elements(sys, pi);
            if st.show_orbits {
                let pts: Vec<Pos2> = el
                    .orbit_points_ecl(jd, 360)
                    .iter()
                    .map(|q| proj.to_screen_ecl(&frame_mul(&frame, q)))
                    .collect();
                if pts.len() > 1 {
                    let col = if st.selected_body.as_deref() == Some(pl.name.as_str()) { CYAN } else { GRID.linear_multiply(0.55) };
                    p.add(egui::Shape::line(pts, Stroke::new(1.0_f32, col)));
                }
            }
            let stt = el.state(jd);
            let sp = proj.to_screen_ecl(&frame_mul(&frame, &orrery_astro::frames::icrs_to_ecl(stt.pos)));
            let col = cols[pi % 10];
            // session 8: exoplanet markers enlarged with the Sol planets
            let rad = pl.radius_rj.map(|r| 3.0 + 3.4 * (r.max(0.1) as f32).sqrt()).unwrap_or(3.5);
            let apparent = (pl.radius_rj.unwrap_or(0.0).max(0.05) * 69911.0 / AU_KM * scale as f64) as f32;
            if st.scale == ScaleMode::True && apparent >= 7.0 {
                let pv = frame_mul(&frame, &orrery_astro::frames::icrs_to_ecl(stt.pos));
                let pn = (pv[0].powi(2) + pv[1].powi(2) + pv[2].powi(2)).sqrt().max(1e-9);
                let light = view_dir([-pv[0] / pn, -pv[1] / pn, -pv[2] / pn], yaw, pitch);
                let pal = if pl.radius_rj.unwrap_or(0.0) > 0.5 { 10 } else { 11 };
                let spin = (jd.rem_euclid(0.8) / 0.8) * std::f64::consts::TAU;
                lidar_sphere(p, sp, apparent, pal, spin, 0.0, yaw, pitch, light);
            } else {
                p.circle_filled(sp, rad.clamp(3.0, 9.0), col);
            }
            if st.selected_body.as_deref() == Some(pl.name.as_str()) {
                let tr = Rect::from_min_size(sp + Vec2::new(8.0, -20.0), Vec2::new(10.0 + pl.name.len() as f32 * 8.0, 16.0));
                p.rect_filled(tr, 0.0, CYAN);
                p.rect_stroke(tr, 0.0, STROKE_HI, egui::StrokeKind::Outside);
                p.text(tr.center(), egui::Align2::CENTER_CENTER, pl.name.as_str(), font(PX_SM), INK);
            } else if st.show_labels {
                p.text(sp + Vec2::new(6.0, -6.0), egui::Align2::LEFT_BOTTOM, pl.name.as_str(), font(PX_SM), GRID);
            }
            if let Some(h) = hover {
                offer(&mut pick, (h - sp).length(), pl.name.as_str(), sp, 14.0);
            }
        }
    }
    // hover highlight, same treatment as the Sol scene
    if let Some((_, name, sp)) = pick.as_ref() {
        p.circle_stroke(*sp, 10.0, Stroke::new(1.0_f32, CYAN.linear_multiply(0.85)));
        let w = 10.0 + name.len() as f32 * 8.0;
        let tr = Rect::from_min_size(*sp + Vec2::new(12.0, -24.0), Vec2::new(w, 16.0));
        p.rect_filled(tr, 0.0, CELL);
        p.rect_stroke(tr, 0.0, Stroke::new(1.0_f32, GRID), egui::StrokeKind::Outside);
        p.text(tr.center(), egui::Align2::CENTER_CENTER, name, font(PX_SM), GRID_HI);
    }
    if resp.clicked() && !resp.dragged() {
        st.auto_tour = false;
        let sys_hit = hover.and_then(|h| {
            sys_markers
                .iter()
                .find(|(_, sp)| (*sp - h).length() < 10.0)
                .map(|(i, _)| *i)
        });
        let dbl = resp.double_clicked();
        let new_sel = pick.map(|(_, n, _)| n);
        let keep_sel = new_sel.is_none() && {
            let moved = match (st.click_origin, hover) {
                (Some(o), Some(h)) => ((h.x - o[0]).powi(2) + (h.y - o[1]).powi(2)).sqrt(),
                _ => 0.0,
            };
            moved > 4.0 // drag-release: keep the selection
        };
        if let Some(i) = sys_hit {
            st.system = i;
            st.selected_body = None;
        } else if !keep_sel {
            st.selected_body = new_sel;
        }
        if dbl && st.selected_body.is_some() {
            st.cam = CamMode::Track;
            st.pan = [0.0, 0.0];
            if let Some(pl) = s.planets.iter().find(|pl| Some(pl.name.as_str()) == st.selected_body.as_deref()) {
                let base = rect.width().min(rect.height());
                st.zoom = (base as f64 * 0.35 / pl.a_au.max(0.005)).clamp(4.0, 400_000.0) as f32;
            }
        }
    }
    p.text(
        rect.left_top() + Vec2::new(8.0, 22.0),
        egui::Align2::LEFT_TOP,
        format!(
            "HOST {}  {}  d={} ly  TWO-BODY KEPLER — ELEMENT CEILING §5.2",
            s.host.name,
            s.host.spectral,
            s.host.dist_ly.map(|d| format!("{d:.1}")).unwrap_or_else(|| "--".into())
        ),
        font(PX_SM),
        GRAY,
    );
    if st.show_legend {
        if draw_legend(ui, p, rect, st, 48.0) {
            st.show_legend = false;
        } // below the HOST info row
    } else if legend_expand_chip(ui, p, rect, st.tips) {
        st.show_legend = true;
    }
    let mode = match st.cam {
        CamMode::Free => "FREE",
        CamMode::Track => "TRACK",
        CamMode::TopDown => "TOP-DOWN",
    };
    let focus = if focused {
        format!("  FOCUS {}", st.selected_body.as_deref().unwrap_or("---"))
    } else {
        String::new()
    };
    p.text(
        rect.right_bottom() + Vec2::new(-10.0, -10.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("CAM {mode}{focus}  ZOOM {scale:.1} px/AU"),
        font(PX_SM),
        GRAY,
    );
    let _ = ui;
}

fn dotted_circle(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let n = ((r * 0.9) as usize).clamp(24, 400);
    let s = Stroke::new(1.0_f32, col);
    for i in (0..n).step_by(2) {
        let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        p.line_segment(
            [c + r * Vec2::new(a0.cos(), a0.sin()), c + r * Vec2::new(a1.cos(), a1.sin())],
            s,
        );
    }
}
