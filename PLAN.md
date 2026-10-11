```markdown
# ORRERY — Design & Build Plan

> A native Windows desktop application that renders a **physically accurate, time‑true 3D orrery** of the Solar System (planets, major moons, asteroid belts, comets) plus **10 nearby multi‑planet star systems**, dressed in the **retro‑industrial control‑panel aesthetic** of the reference image (black field, 1 px grid, monospace pixel font, cyan / green / amber status cells, circular orbit scope, numbered bottom selector, yellow **AUTO** key).

**Accuracy statement (read first).** The Solar System is computed to **arc‑second** precision from bundled analytical ephemerides (VSOP87 planets + LIESSE major moons + MPC/JPL osculating elements for asteroids & comets), optionally cross‑checked / refreshed against **JPL Horizons**. At **1× speed the view is a live, true‑to‑the‑moment picture of the actual sky.** Exoplanet systems are propagated from their **published Keplerian elements** (two‑body), which is the maximum precision physically available for them (no DE files; no confirmed moon/asteroid/comet companions in the catalog). This ceiling is stated in the UI.

**Out of scope (per stakeholder):** APOD / Donki / Mars‑Rover image & weather enrichment cells are **excluded**.

| Meta | |
|------|---|
| Document | `PLAN.md` |
| Status | v1.1 — approved |
| Platform | Windows (native) |

### Revisions
| Ver | Change |
|-----|--------|
| v1.0 | Initial plan (R1–R13) |
| v1.1 | Added optional NASA/JPL online layer + **API key** and **CPM** settings UI (R14, R15); excluded garnish cells |

---

## 1. Goals & hard requirements (traceability)

| # | Requirement | Where satisfied |
|---|-------------|-----------------|
| R1 | Live 3D Solar System (planets) | §6, §7 |
| R2 | … including **moons** | §6.1 (LIESSE + element sets) |
| R3 | … including **asteroids** | §6.1 (belt + MPC elements) |
| R4 | … including **comets** | §6.1 (SBDB elements + procedural tails) |
| R5 | **10 other nearby systems, ≥3 planets each** | §6.2 (curated catalog) |
| R6 | **Accurate trajectories & timings** | §5 (VSOP87/LIESSE/elements; Horizons verify) |
| R7 | **Drop‑down** system selector | §9 (header combo, image‑styled) |
| R8 | **Speed settings** | §8.2 (preset tabs + slider + rewind) |
| R9 | **Camera controls** | §8.1 (free / track / target) |
| R10 | **Automatic camera movement** | §8.3 (AUTO tour, yellow key) |
| R11 | **Live details** of selected object | §10 (telemetry panel) |
| R12 | **Raw data + graphs** | §10.3 (scope plot, time‑series) |
| R13 | **Interface like the attached image** | §2, §9 (palette, grid, font, layout map) |
| R14 | Settings UI: enter **NASA API key** + set **calls‑per‑minute** | §5.4, §9.4 |
| R15 | Optional, rate‑limited, keyed **online layer** (Horizons + Exoplanet Archive + NEO feed) | §4, §5.4, M4.5 |

> **Out of scope:** APOD / Donki / Mars‑Rover image & weather cells (excluded per stakeholder).

---

## 2. Design language (derived from the reference image)

Flat, hard‑edged, monochrome‑plus‑three‑accents. **No gradients, no rounded corners, no drop shadows.** Everything is a 1 px‑bordered cell on black.

**Palette (sampled from the image)**

| Token | Hex | Use |
|-------|-----|-----|
| `BG` | `#000000` | global background |
| `CELL` | `#0A0A0A` | panel fill |
| `GRID` | `#6E6E6E` | minor cell borders |
| `GRID_HI` | `#FFFFFF` | major separators, axes, text |
| `CYAN` | `#21D0C5` | *active* tab / selected row / info value / primary trace |
| `GREEN` | `#43A047` | *OK / nominal* value, filled status bar |
| `AMBER` | `#E0A800` | *warning / selected / AUTO* key |
| `ALARM` | `#D23B2E` | error / out‑of‑range value |
| `GRAY` | `#57574A` | inactive row / disabled cell |
| `INK` | `#000000` | text on cyan/green/amber fills |

**Typography** — bundled bitmap pixel font, 8×14 px, monospace, Latin + optional Cyrillic (to match the source glyph shapes); UI scale ×2. Fallback: `Consolas` / `IBM Plex Mono`. Labels UPPER‑CASE, no anti‑aliased curves.

**Rules**
- Every panel = a grid of cells separated by 1 px `GRID` lines.
- Active/selected = solid `CYAN` fill, black text.
- Numeric values colour‑coded by state: `GREEN` nominal, `CYAN` informational, `AMBER` caution, `ALARM` fault.
- Bars rendered as discrete filled segments (like the `МИН/МАКС` gauges in the image).
- Plots = black field, white tick marks, dotted grid, single‑colour trace (`CYAN` or `AMBER`) — exactly the scope on the right of the image.
- Bottom action bar = flat rectangular keys; the **AUTO** key is `AMBER` (mirrors `АВТОМ` in the image).

---

## 3. Technology stack

| Layer | Choice | Why |
|-------|--------|-----|
| Language | **C++20** | performance, single static binary, no runtime |
| Window / GPU | **Win32 + Direct3D 11** | native Windows, best DX11 backend for the UI lib |
| 2D UI / panels / graphs | **Dear ImGui (DX11 + Win32 backends)** | the flat, gridded, monospace aesthetic is its native mode; built‑in plots, tables, combo, slider |
| 3D scene | **Custom minimal D3D11 renderer** | spheres, line‑loops, instanced points, billboards, labels — fully stylable to the HMI look |
| Astronomy math | **erfa** (IERS/IMCCE, public‑domain C) + **VSOP87** + **LIESSE** + **SGP4** | arc‑second positions, tiny coefficient files, offline |
| HTTP (optional online) | **WinHTTP** (or `cpp-httplib`) | TLS to the 3 official endpoints; event‑driven, rate‑limited (§5.4) |
| Data | Bundled JSON/CSV element files + binary theory coefficients | self‑contained, works offline |
| Build | **MSVC + CMake** → single `.exe` + bundled assets, optional NSIS installer | "a Windows application" |

> *Alternative (if a faster iteration / cross‑build is wanted):* **Rust + `egui` + `wgpu`** — `egui` matches the same flat aesthetic; the architecture is identical. The plan is renderer‑agnostic.

---

## 4. Architecture & module map

```
┌──────────────────────────────────────────────────────────────────────┐
│                              ORRERY.exe                              │
│                                                                      │
│  ┌────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐  │
│  │  app/      │  │  core/      │  │  astro/     │  │  render/     │  │
│  │ main loop  │─▶│  state,     │─▶│  ephemeris  │─▶│  D3D11 scene │  │
│  │  (ImGui)   │  │  clock,     │  │  VSOP87/    │  │  + 3D pick   │  │
│  │            │  │  selection, │  │  LIESSE/    │  │  + 2D panels │  │
│  └────────────┘  │  camera,    │  │  elements/  │  └──────────────┘  │
│       ▲          │  rate_      │  │  SGP4       │                    │
│       │          │  limiter,   │  └─────────────┘                    │
│  ┌────┴─────┐    │  key_store  │  ┌──────────────┐                   │
│  │ ui/      │    └─────────────┘  │  data/       │                   │
│  │ panels,  │    ┌─────────────┐  │  catalog,    │  ┌──────────────┐  │
│  │ theme,   │    │  data/      │  │  elements,   │  │  data/       │  │
│  │ graphs,  │    │  online/    │  │  theory,     │  │  catalog.json│  │
│  │ settings │    │  (3 clients)│  │  + rate/keys │  │  vsop87.bin  │  │
│  └──────────┘    └─────────────┘  └──────────────┘  │  liesse.bin  │  │
│                                                      │  sbdb/*.csv  │  │
│                                                      └──────────────┘  │
│  worker thread: ephemeris evaluation for N bodies @ sim‑time (lock‑free ring buffer)│
└──────────────────────────────────────────────────────────────────────┘
```

- **Main thread:** ImGui layout, input, 3D render, camera, selection.
- **Ephemeris worker:** recomputes all visible body states for the current sim‑epoch into a double‑buffered struct; main thread reads the last completed buffer → no frame hitch, supports "scrub" and "MAX" speed without blocking UI.
- **Data layer:** loads `catalog.json`, `vsop87.bin`, `liesse.bin`, `sbdb/*.csv` at start; the **online layer** (`data/online/`, §5.4) is optional and never on the critical path.

---

## 5. Ephemeris engine — how "accurate" is delivered

### 5.1 Solar System (primary)
| Body class | Model | Precision | Source |
|------------|-------|-----------|--------|
| 8 planets + Pluto (dwarf) | **VSOP87** truncated theory → J2000 Ecliptic, then `erfa` to ICRS/ECI | **~0.5–1″** over ±century | bundled theory coefficients |
| Major moons (Luna, Galilean, Titan, etc.) | **LIESSE** + parent position | arc‑second‑class (Galileans); arc‑minute (small moons) | bundled `liesse` |
| Phobos/Deimos, irregulars | published osculating elements + 2‑body | arc‑minute | MPC / JPL SBDB |
| Asteroids (Ceres, Pallas, Vesta, Eros, ~2 000‑belt sample, NEOs) | **MPC / JPL SBDB** osculating elements + secular rates; NEOs optionally **TLE/SGP4** | arc‑minute (visual + timing correct) | bundled CSV |
| Comets (Halley, Encke, Hale‑Bopp, + active short‑period) | JPL SBDB elements + outgassing‑free Kepler propagation; tail modelled visually | position good; long‑range inherently limited by non‑grav. effects (documented) | JPL SBDB |

### 5.2 Exoplanets (the 10 systems)
- Each planet stored with **a, e, i, Ω, ω, M₀ (or T), epoch, mass, radius (est.), host‑star mass**.
- Propagation = **Kepler two‑body** about the host star via `erfa` (rotation to ICRS using the star's RA/Dec) — *exact* to the precision of the published elements; element uncertainty displayed in the data panel.
- No mutual perturbations, no moons/asteroids/comets for these systems (none confirmed with full element sets) — stated in UI footer.

### 5.3 Time base
- Internal sim‑time = **JD (Terrestrial Time)**; "LIVE / 1×" pins `sim_TT = now(UTC→TT)`.
- All displays show **TDB/TT epoch**, UT‑date, and Δt from live when scrubbed.

### 5.4 Online data layer (optional — R15)

Three official endpoints, all **event‑driven** (user `REFRESH` or a long‑period timer, e.g. 5–15 min) — **never per‑frame**, so R6/R12 and the 60 fps target hold with `LINK = OFF`.

| Service | Endpoint | Gives | Key required? | Role in Orrery |
|---------|----------|-------|---------------|----------------|
| **JPL Horizons** | `ssd-api.jpl.nasa.gov` | true positions/vectors/angles | No | **verify** (residual log) + element refresh |
| **Exoplanet Archive TAP** | `exoplanetarchive.ipac.caltech.edu/TAP` | elements, masses, radii, disc year, uncertainty | No | **catalog source / refresh** (§6.2, M4) |
| **NEO feed** | `api.nasa.gov/neo` | estimated size/mass + close approaches | **Yes** | **live Sol NEO list** (parameters are *estimates*) |

> The casual `api.nasa.gov` portal does **not** provide a per‑frame ephemeris; it supplies *parameters/observations*. The accurate trajectory always comes from local propagation. These services **complement** the bundled theory; they do not replace it.

**Rate limiting (the CPM field is enforced, R14).**
- Token bucket shared by **all** clients: `rate = CPM/60` tokens/sec, `burst = min(CPM, 10)`. Default `CPM = 20` (user‑editable).
- Every request `try_acquire()` first; if empty → **deferred** with linear backoff, UI shows `WAIT`. No request is ever issued above the cap.
- `API KEY` gates **NEO feed only**: blank key → NEO disabled (`NO KEY`, amber); Horizons + Exoplanet Archive still work keyless but still honour CPM.

**Key storage & security.**
- Stored in **Windows Credential Manager** (fallback: DPAPI‑encrypted file). *Not* in `catalog.json`, never logged.
- `STORE = NO` keeps it in‑session only. Masked in UI (`•••••••`) with show/hide toggle.
- Egress only to the three official endpoints.

---

## 6. Celestial content catalog

### 6.1 Sol (full‑fidelity)
- **Planets:** Mercury→Neptune (+ Pluto toggle).
- **Moons:** 1 200+ known, *rendered* for the major ~40 (visual scale), *counted* for the rest; Galilean + Titan + Luna at full accuracy.
- **Asteroids:** Ceres/Pallas/Vesta/Eros as bodies; **Main Belt** and **Kuiper** as instanced point‑clouds (tens of thousands) for the "field" look seen in the image's star‑field; ~2 000 named NEOs/KBOs as selectable elements.
- **Comets:** Halley, Encke, Tempel‑1, Hale‑Bopp, + active short‑period list; procedural coma + dust/ion tail billboards that grow with `1/r²` near perihelion.

### 6.2 The 10 "other nearby systems" (≥3 confirmed planets, curated)

| # | System | Dist. (ly) | Planets (≥3) | Element source |
|---|--------|-----------|--------------|----------------|
| 01 | **Sol** (baseline) | 0 | 8 | VSOP87/LIESSE (full) |
| 02 | **55 Cancri / Rho Cnc** | ~12.8 | b,c,d,e,f (5) | Exoplanet Archive / exoplanet.eu |
| 03 | **Gliese 876** | ~15.4 | b,c,d,e (4) | " |
| 04 | **HD 10180 (Kips)** | ~11.7 | b…g (6) | " |
| 05 | **HD 40307** | ~42.0 | b…f (5–6) | " |
| 06 | **TRAPPIST‑1** | ~40.7 | b…h (7) | " |
| 07 | **GJ 667 C** | ~23.6 | b…g (6) | " |
| 08 | **51 Ophiuchi** | ~29.0 | b,c,d,e (4) | " |
| 09 | **47 Ursae Majoris** | ~45.0 | b,c,d (3) | " |
| 10 | **Upsilon Andromedae** | ~44.0 | b,c,d (3–4) | " |
| 11 | **HD 71255** | ~19.4 | b,c,d (3) | " |

- "Nearby" interpreted as the **closest known multi‑planet hosts** (≤ ~45 ly); each star carries **spectral type, mass, radius, RA/Dec, distance, V‑mag, colour** so the host star is rendered with correct relative size/colour and the system is placed on the **star‑field minimap** (the image's top‑right map slot, repurposed as the sky/constellation map).
- **Data hook:** `catalog.json` schema + an importer that ingests **Exoplanet Archive** TAP / **exoplanet.eu** CSV exports → users can swap in their own 10‑system list without recompiling (M4.5).

---

## 7. 3D scene & rendering

- **Coordinate frames:** ICRS (J2000) master; ecliptic grid toggle.
- **Orbits:** full period line‑loops (1 024–4 096 pts, re‑baked when elements change), thin `GRID_HI`/`CYAN` lines — *in TOP‑DOWN these read exactly like the dotted concentric‑orbit scope in the reference image* (intentional continuity).
- **Bodies:** shaded spheres (single directional light = host star) + rim; planet‑specific flat colour token from catalog (keeps HMI look, no PBR).
- **Asteroid/Kuiper field & star background:** GPU **instanced / point sprites**, additive faint white — matches the image's scattered star dots.
- **Comet tails:** two billboard ribbons (dust `AMBER`, ion `CYAN`), length ∝ activity.
- **Labels:** screen‑space pixel‑font tags (name + live distance), toggleable; selected body tag = `CYAN` filled cell.
- **Selection:** ray‑pick against body + orbit‑line proximity; click selects, double‑click **tracks**.
- **Scale modes:** *TRUE* (honest) / *VISUAL* (planet sizes & distances log‑scaled). TRUE is the "accurate" default; VISUAL keeps **timing & relative phases accurate**, only radii/spacing are cosmetic (flagged in UI).

---

## 8. Time & camera controls

### 8.1 Camera (R9)
- **FREE** (arcball: L‑drag rotate, R‑drag pan, wheel dolly), **TRACK** (locked on selected body, world‑up), **TOP‑DOWN** (ecliptic north — produces the image's scope look).
- Smooth critically‑damped transitions between modes; crisp, mechanical (no inertia), matching the HMI feel.

### 8.2 Speed (R8) — mirrors the image's tab strip
Preset tabs: `LIVE(1×) | 60× | 1h/s | 1d/s | 1yr/s | MAX`, plus a **log slider** and **◀ rewind** (negative rate). `MAX` clamps so the ephemeris stays inside its validated span. Rate is independent of camera rate.

### 8.3 Automatic camera movement (R10) — the yellow **AUTO** key
- **AUTO‑TOUR:** *system overview → each planet in orbital order (auto‑track, dwell, ease‑out) → back to overview*, looping.
- **AUTO‑FOLLOW:** continuously re‑frames the selected body.
- Pressing any manual key drops `AUTO` (key lights off, as in the image's active/inactive button logic).
- Tour order & dwell = user‑configurable table.

---

## 9. UI layout — direct mapping of the reference image

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│  SYS: SOL ▾        EPOCH 2025‑… TT 12:34:56     CAM TARGET: MARS  x.. y.. z..     │  ← top header (img: location/coords/title)
├────────────────────────────────────────────────┬─────────────────────────────────┤
│  SELECTED OBJECT  [MARS]                       │                                  │
│  TYPE  PLANET      MASS  6.417e23 kg           │         3D ORRERY VIEW           │  ← left tables  →  live details (§10)
│  DIST‑STAR  1.524 AU   VEL  24.1 km/s         │        (top‑down == image        │
│  DIST‑CAM  4.1e8 km    PHASE 142°             │          circular scope)         │
│  ┌──RAW DATA──┐  ┌──SCOPE: r(t)──┐           │                                  │  ← left scope  →  graphs (§10.3)
│  │ x 2.27e8   │  │  ·······      │            │   [minimap: sky / host star]      │  ← top‑right map → star‑field map
│  │ y -4.0e7   │  │  /           │            │                                  │
│  │ z  1.1e7   │  │/            │            │                                  │
│  └────────────┘  └────────────┘            │                                  │
├──────────────────────────────────────────────┴─────────────────────────────────┤
│  01 02 03 04 05 06 07 08 09 10 11   (system selector strip — image's 01‑10)     │  ← R7 (also ▾ combo in header)
│  [AUTO] [MANUAL] [PAUSE] [SPEED] [ORBITS] [LABELS] [GRID] [TRUE/VIS] [RESET]    │  ← image's bottom key row (AUTO = AMBER)
└──────────────────────────────────────────────────────────────────────────────────┘
```

- **Top header** (white‑on‑black, like the image's title bar): system name + **drop‑down combo** (R7, styled as the image's cyan tab cell), live TT epoch, camera target vector.
- **Left column** = status/telemetry tables (green/cyan/amber value cells, exactly the image's `НАСОС/ВЫКЛ` tables) + the **scope plot**.
- **Centre/right large cell** = the **3D viewport**; in TOP‑DOWN it visually equals the reference's circular orbit plot (dotted grids + one amber trajectory arc = selected body's recent path).
- **Top‑right map cell** = star‑field / host‑star map (replaces the green world map).
- **Number strip `01…11`** = the **10 other systems + Sol** selector (R7), active = `CYAN`.
- **Bottom key row** = actions; **AUTO** is `AMBER` (the image's `АВТОМ`), others white‑bordered; inactive = `GRAY`.
- **Tabs** (`LIVE/60×/…`) sit where the image shows `Х1/МИН/МАКС/ПИТАНИЕ`.

Every cell, bar, tick and key reuses the §2 tokens so the result is *indistinguishable in style* from the source while being a space app.

### 9.4 `DATA LINK` settings block (R14, R15)
Placed in the left column under the scope plot, styled like the reference's label/value cells. The **API KEY** and **CPM** fields are plain entry cells; an `OK`/`TEST` key mirrors the image's bottom‑key logic.

```
┌─ DATA LINK ────────────────────────────────────────────────┐
│  LINK    [ ON  ▾ ]   (ON = CYAN / OFF = GRAY)             │
│  API KEY [ ••••••••  ]  [SHOW]        ◄── R14 field 1      │
│  CPM     [ 20    ]      [−][+]         ◄── R14 field 2     │
│  STORE   [ YES ▾ ]                                       │
│  ────── TEST ───  [ OK ]   LAST: 12:34:56  RESID 0.4″    │
│  STATUS  NEO: OK   EXO: OK   HORIZ: STALE  BUCKET: 18/20 │
└────────────────────────────────────────────────────────────┘
```
- `TEST` fires **one** throwaway call per service (respecting the bucket) → each row lights `OK` (green) / `WAIT` (amber) / `NO KEY` (amber) / `FAIL` (red).
- `BUCKET: n/CPM` is a live gauge (filled segments, like the image's `МИН/МАКС` bars) so the user *sees* the cap being respected.
- `REFRESH` and the residual/element‑refresh jobs all route through the same bucket.

---

## 10. Live data & graphs panel (R11, R12)

### 10.1 Telemetry (updated every frame from the ephemeris buffer)
- Identity: name, designation, type, system, spectral/colour token.
- **Live state:** position (ICRS, AU & km), velocity vector & |v|, distance to host star, distance to camera, distance to Sun (for exo: to host), phase angle, apparent V‑mag (Sol sys), true/eccentric anomaly.
- **Orbital elements (live, osculating):** a, e, i, Ω, ω, M, n, period, epoch; **element uncertainty** shown for exoplanets (honesty re R6).
- State colouring: nominal `GREEN`, info `CYAN`, near‑perihelion/conjunction `AMBER`, data‑stale `ALARM`.
- **Link row:** `LINK ON|OFF · LAST SYNC <t> · KEY SET|NONE · CPM <n>` (§5.4).

### 10.2 Raw data block
- J2000 ECI vectors, heliocentric/geo‑centric (Sol), topocentric‑optional; TDB/UT timestamps; the exact coefficient set & theory version used (provenance).

### 10.3 Graphs (image‑style scope: black field, white ticks, dotted grid, single trace)
- **r(t)** distance‑to‑star vs. time (the amber arc in the image).
- **|v|(t)** speed; **phase(t)**; **altitude/phase‑angle(t)** (Sol).
- **Residual log:** bundled‑theory vs. **Horizons** (when online) / `THEORY` (offline) — proves accuracy on screen.
- Each plot: 1‑px border, 5‑tick axis, `CYAN`/`AMBER` trace, value readouts on the right edge (like the image's `ЧХ‑0.000 / ЧМ‑… / У 3.640` column).

---

## 11. Input & interaction
- Mouse: pick/track/rotate/pan/dolly; wheel = dolly (FREE) or speed (over speed control).
- Keyboard: `1‑9,0,‑` select systems (maps to the number strip), `Space` pause, `A` auto‑tour, `T` track, `+/-` speed step, `[ ]` toggle orbits/labels/grid, `Home` reset, `G` scale TRUE/VIS.
- All keys also present as HMI buttons (discoverability matches the image).

---

## 12. Performance targets & optimisation
- **≥60 fps** at 1080p on integrated GPU with full Sol scene (planets + 40 moons + 20k‑point belts + 12 comets + labels).
- Ephemeris off the render thread (worker + double buffer); pre‑baked orbit line‑loops; instanced point clouds; single‑pass flat shading; label atlas; no per‑frame allocations.
- **Online calls are event‑driven and rate‑limited** (§5.4) → zero impact on the frame budget.
- Memory: < 200 MB (theory files are small; DE440 optional, lazy‑loaded).

---

## 13. Build & distribution
- MSVC + CMake → single `Orrery.exe` + `assets/` (catalog, theory, fonts). Runs **fully offline** (R6 satisfied without network). Optional NSIS installer + opt‑in update check.
- Reproducible: pinned `erfa`, VSOP87/LIESSE coefficient checksums in `catalog.json` (integrity + accuracy provenance).
- API key handled via Credential Manager (§5.4); never shipped, never in assets.

---

## 14. Roadmap / milestones

| MS | Scope | Acceptance |
|----|-------|------------|
| **M0** | Project, D3D11 + ImGui shell, §2 theme, layout grid from §9 | image‑style empty HMI renders at 60 fps |
| **M1** | Time base + VSOP87 + erfa; 8 planets + orbit lines; speed presets | planet positions match Horizons < 1″ (logged) |
| **M2** | LIESSE moons + asteroid/Kuiper point clouds + 3 comets + tails; selection & track | Sol scene complete; pick/track works |
| **M3** | Camera FREE/TRACK/TOP‑DOWN + AUTO‑tour + scale modes | R9, R10 met; top‑down == image scope look |
| **M4** | 10‑system catalog + element propagation + selector strip/combo + minimap | R5, R7 met; switch systems live |
| **M4.5** | Online layer (`data/online/*`, `core/{rate_limiter,key_store}`) + `DATA LINK` block + `TEST`/`REFRESH` + TAP import | R14, R15 met — see criteria below |
| **M5** | Telemetry + raw data + scope plots + residual log + Horizons refresh | R6, R11, R12 met; accuracy panel green |
| **M6** | Polish, perf pass, installer, docs | all of R1–R15; 60 fps; ships |

**M4.5 acceptance criteria**
- `API KEY` and `CPM` fields persist; key masked; key **never** in `catalog.json`/logs.
- Rate‑limit harness: with `CPM = 5`, a burst of 30 forced refreshes issues **≤ 5 in any 60 s window** (`test_rate_limiter.cpp`).
- With `LINK = OFF`, the app is **100% functional** (all of R1–R13) and makes **zero** network calls.
- Exoplanet elements import from TAP; Sol residual vs. Horizons logged **< 1″**.
- NEO feed disabled + `NO KEY` shown when key blank; enabled when set.

---

## 15. Risks & mitigations
| Risk | Mitigation |
|------|------------|
| Comet long‑range drift (non‑grav. effects) | limit validated span, show warning, offer Horizons refresh *(documented)* |
| Exoplanet "accuracy" ceiling (elements only, no companions) | explicit uncertainty display + footer; do **not** claim moon/asteroid fidelity for them |
| DE440 size vs. offline | ship VSOP87/LIESSE (small) by default; DE440 optional download |
| ImGui theme drift from reference | lock the style struct in code; add a visual‑regression screenshot test of the HMI against the reference |
| Frame hitch at MAX speed | worker‑thread ephemeris + double buffer (§4) |
| API key misuse / quota blowout / throttling bans | global token‑bucket capped at user `CPM`; event‑driven calls only; key in Credential Manager (not plaintext); per‑service `NO KEY`/`WAIT` states; offline fallback keeps R1–R13 intact |

---

## 16. Repository layout

```
Orrery/
├─ CMakeLists.txt
├─ PLAN.md
├─ src/
│  ├─ app/        main.cpp, apploop.cpp, config.cpp
│  ├─ ui/         theme.cpp, panels.cpp, graphs.cpp, layout.cpp, input.cpp, settings.cpp
│  ├─ render/     d3d.cpp, scene.cpp, orbits.cpp, points.cpp, comets.cpp, labels.cpp, camera.cpp
│  ├─ astro/      ephemeris.cpp, vsop87.cpp, liesse.cpp, elements.cpp, sgp4.cpp, erfa_wrap.cpp
│  ├─ core/       rate_limiter.cpp, key_store.cpp
│  └─ data/
│     ├─ catalog.cpp, loader.cpp
│     └─ online/   http_client.cpp, horizons.cpp, exoplanet_archive.cpp, neo_feed.cpp
├─ assets/
│  ├─ catalog.json        (11 systems + elements + star props + checksums)
│  ├─ vsop87.bin  liesse.bin
│  ├─ sbdb/     *.csv     (asteroids / comets elements)
│  └─ fonts/    pixel.woff
├─ tests/
│  ├─ test_ephemeris_vs_horizons.cpp
│  ├─ test_rate_limiter.cpp
│  ├─ test_online_offline_parity.cpp
│  └─ test_ui_visual_regression.cpp
└─ docs/  DATA_SOURCES.md  AESTHETIC.md
```

---

### Bottom line
**Orrery** is a native C++/D3D11/ImGui Windows app whose *look* is a faithful recreation of the reference control panel (black + cyan/green/amber, 1 px grid, pixel monospace, scope plot, numbered selector, yellow **AUTO** key), whose *behaviour* meets every stated requirement (R1–R15), and whose *positions* are arc‑second‑accurate for the Solar System (live at 1×) and accurate‑to‑published‑elements for the 10 nearby multi‑planet systems — with the precision limits stated plainly in‑app. The optional NASA/JPL online layer is **rate‑limited by a user‑set CPM cap**, **keyed via Credential Manager**, and **degrades cleanly to a fully offline, fully accurate orrery** the instant `LINK = OFF`. Build in seven milestones (M0→M6, incl. M4.5) above.
```

