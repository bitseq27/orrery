# ORRERY — HMI Aesthetic Lock (PLAN §2, §13 risk row)

The look is a **faithful recreation of the reference control panel**: black field,
1 px grid, monospace pixel type, cyan/green/amber status logic, circular orbit scope,
numbered bottom selector, yellow AUTO key. No gradients, no rounded corners, no shadows.

## Palette tokens (single source of truth: `crates/app/src/theme.rs`)

| Token | Hex | Meaning |
|-------|-----|---------|
| BG | #000000 | global field |
| CELL | #0A0A0A | panel fill |
| GRID | #6E6E6E | minor 1px borders |
| GRID_HI | #FFFFFF | major separators, axes, primary text |
| CYAN | #21D0C5 | active tab / selected row / info value / primary trace |
| GREEN | #43A047 | OK / nominal value, filled gauge segments |
| AMBER | #E0A800 | caution / AUTO key / dust tail / 1″ cap line |
| ALARM | #D23B2E | fault / out-of-range |
| GRAY | #57574A | inactive keys / disabled cells |
| INK | #000000 | text on cyan/green/amber fills |

## Rules enforced in code

1. Every panel is a 1 px `GRID`-bordered cell on `BG`; titles upper-case, 10 px pixel font.
2. Active/selected = solid `CYAN` fill with `INK` text; the AUTO key uses `AMBER`.
3. Numeric values are state-coloured: GREEN nominal, CYAN info, AMBER caution, ALARM fault.
4. Gauges are **discrete segments** (10 cells), never smooth bars.
5. Plots: black field, white axis + ticks, dotted #2E2E2E grid, single-colour trace.
6. Keys are flat rectangles; inactive = `GRAY` text on `CELL`; hover = `GRID_HI` border only.
7. Type: bundled VT323 terminal pixel face (OFL, 1 px stems) at 13/16/22 px; monospace
   only. Replaced Press Start 2P in session 3 — its heavy stems read as bold text
   (user feedback); VT323 keeps the CRT look at a lighter weight.

## Visual regression

`tools/shots.sh` renders the locked set of states headlessly (Xvfb + scrot) into
`docs/shots/`: M0 shell, M1 scene, M2 moons/belts, M3 cameras+tour, M4 exo,
M4.5 data link, M5 residuals. Compare against these baselines when touching
theme/layout code; any intentional change must refresh the baselines in the same commit.
