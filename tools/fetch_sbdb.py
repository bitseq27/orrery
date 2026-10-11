#!/usr/bin/env python3
"""Fetch osculating elements from JPL SBDB and bake crates/astro/src/sbdb_data.rs.

Bodies: large/named asteroids (incl. Trojans & TNOs), near-Earth asteroids and
comets. Elements are JPL SBDB osculating Keplerians at the solution epoch
(PLAN.md §5.1: asteroids/comets = SBDB elements + 2-body propagation).

Also bakes SBDB_PROPS from SBDB physical parameters / object metadata:
PHA + NEO flags (→ red near-Earth-collision markers in the scene), H, diameter,
rotation period, spectral class and orbit class (→ composition readouts).
Hyperbolic orbits (e >= 1) are skipped: the two-body Kepler propagator is elliptical.

Usage:  python3 tools/fetch_sbdb.py > crates/astro/src/sbdb_data.rs   (shell redirect:
the sandbox VFS can drop files written by long-lived python processes)
"""
import json
import sys
import urllib.parse
import urllib.request

# sstr (designation number = most robust), NAME, kind (asteroid|comet)
BODIES = [
    # --- main belt (large / famous) ---
    ("1", "CERES", "asteroid"),
    ("2", "PALLAS", "asteroid"),
    ("4", "VESTA", "asteroid"),
    ("10", "HYGIEA", "asteroid"),
    ("704", "INTERAMNIA", "asteroid"),
    ("511", "DAVIDA", "asteroid"),
    ("16", "PSYCHE", "asteroid"),
    ("21", "LUTETIA", "asteroid"),
    ("253", "MATHILDE", "asteroid"),
    ("216", "KLEOPATRA", "asteroid"),
    ("243", "IDA", "asteroid"),
    ("951", "GASPAR", "asteroid"),
    ("3", "JUNO", "asteroid"),
    ("7", "IRIS", "asteroid"),
    ("8", "FLORA", "asteroid"),
    # --- near-Earth asteroids ---
    ("433", "EROS", "asteroid"),
    ("99942", "APOPHIS", "asteroid"),
    ("101955", "BENNU", "asteroid"),
    ("162173", "RYUGU", "asteroid"),
    ("65803", "DIDYMOS", "asteroid"),
    ("25143", "ITOKAWA", "asteroid"),
    ("4179", "TOUTATIS", "asteroid"),
    ("1566", "ICARUS", "asteroid"),
    ("3200", "PHAETHON", "asteroid"),
    ("2062", "ATEN", "asteroid"),
    ("1862", "APOLLO", "asteroid"),
    ("2101", "ADONIS", "asteroid"),
    ("3753", "CRUITHNE", "asteroid"),
    ("3712", "1950-DA", "asteroid"),
    ("1620", "GEOGRAPHOS", "asteroid"),
    ("1685", "TORO", "asteroid"),
    ("2024 YR4", "2024-YR4", "asteroid"),
    # --- Jupiter Trojans ---
    ("624", "HECTOR", "asteroid"),
    ("617", "PATROCLUS", "asteroid"),
    ("588", "ACHILLES", "asteroid"),
    # --- trans-Neptunian ---
    ("50000", "QUAOAR", "asteroid"),
    ("90377", "SEDNA", "asteroid"),
    ("90482", "ORCUS", "asteroid"),
    ("136472", "MAKEMAKE", "asteroid"),
    ("136108", "HAUMEA", "asteroid"),
    ("136199", "ERIS", "asteroid"),
    ("20000", "VARUNA", "asteroid"),
    # --- comets ---
    ("1P", "HALLEY", "comet"),
    ("2P", "ENCKE", "comet"),
    ("9P", "TEMPEL-1", "comet"),
    ("C/1995 O1", "HALE-BOPP", "comet"),
    ("67P", "CHURYUMOV", "comet"),
    ("19P", "BORRELLY", "comet"),
    ("81P", "WILD-2", "comet"),
    ("73P", "SCHWASSMANN-3", "comet"),
    ("29P", "SCHWASSMANN-1", "comet"),
    ("109P", "SWIFT-TUTTLE", "comet"),
    ("21P", "GIACOBINI-ZINNER", "comet"),
]

# Comets SBDB does not carry a `pha` flag for but that are widely classed as
# potentially hazardous (Earth-crossing, impact-capable; 109P MOID ~0.001 AU).
FORCE_PHA = {"SWIFT-TUTTLE"}


def fetch(sstr):
    q = {"sstr": sstr, "full-prec": "true", "phys-par": "true"}
    u = "https://ssd-api.jpl.nasa.gov/sbdb.api?" + urllib.parse.urlencode(q)
    return json.loads(urllib.request.urlopen(u, timeout=60).read())


def phys(d, key):
    for x in d.get("phys_par", []):
        if x.get("name") == key and x.get("value") not in (None, ""):
            try:
                return float(x["value"])
            except ValueError:
                return None
    return None


def main():
    rows = []
    props = []
    for sstr, name, kind in BODIES:
        try:
            d = fetch(sstr)
            obj = d["object"]
            el = {e["name"]: float(e["value"]) for e in d["orbit"]["elements"]}
            epoch = float(d["orbit"]["epoch"])
        except Exception as e:
            print(f"SKIP {name} ({sstr}): {e}", file=sys.stderr)
            continue
        e = el["e"]
        if e >= 0.9999:  # hyperbolic / parabolic — outside elliptical 2-body model
            print(f"SKIP {name}: e={e} hyperbolic", file=sys.stderr)
            continue
        q_ = el.get("q", el.get("a") * (1.0 - e))
        a = el.get("a", q_ / (1.0 - e))
        # SBDB returns JSON booleans for pha/neo (some payloads use "Y"/"N")
        def flag(v):
            return v is True or str(v).strip().upper().startswith("Y") or str(v).strip() == "1"
        pha = flag(obj.get("pha")) or name in FORCE_PHA
        neo = flag(obj.get("neo"))
        h = phys(d, "H") or 0.0
        diam = phys(d, "diameter") or 0.0
        rot = phys(d, "rot_per") or 0.0
        spec = ""
        for x in d.get("phys_par", []):
            if x.get("name") in ("spec_T", "spec_B") and x.get("value"):
                spec = str(x["value"])[:2]
                break
        oc = obj.get("orbit_class") or {}
        ocls = str(oc.get("code", ""))[:8] if isinstance(oc, dict) else str(oc)[:8]
        rows.append(
            dict(name=name, kind=kind, epoch=epoch, a=a, e=e, i=el["i"],
                 om=el["om"], w=el["w"], ma=el.get("ma", 0.0),
                 per=el.get("per", 0.0), q=q_)
        )
        props.append(dict(name=name, pha=pha, neo=neo, h=h, diam=diam, rot=rot,
                          spec=spec, ocls=ocls))
        print(f"fetched {name:16} a={a:.5f} e={e:.5f} q={q_:.4f} pha={pha} neo={neo} "
              f"H={h:.1f} d={diam:.2f}km rot={rot:.1f}h spec={spec!r} cls={ocls!r}",
              file=sys.stderr)

    print("// @generated by tools/fetch_sbdb.py — do not edit.")
    print("//! JPL SBDB osculating Keplerian elements + physical properties.")
    print("//! SBDB: [name, kind(0 asteroid/1 comet), epoch JD, a AU, e, i deg, Ω deg, ω deg, M0 deg, period d, q AU]")
    print("//! SBDB_PROPS: [name, pha(1 = potentially hazardous → red), neo, H mag, diameter km, rot h, spectral class, orbit class]")
    print("pub const SBDB: [(&str, u8, f64, f64, f64, f64, f64, f64, f64, f64, f64); %d] = [" % len(rows))
    for r in rows:
        print(
            '    ("%s", %d, %.9f, %.9f, %.9f, %.9f, %.9f, %.9f, %.9f, %.9f, %.9f),'
            % (r["name"], 0 if r["kind"] == "asteroid" else 1,
               r["epoch"], r["a"], r["e"], r["i"], r["om"], r["w"], r["ma"], r["per"], r["q"])
        )
    print("];")
    print()
    print("pub const SBDB_PROPS: [(&str, u8, u8, f64, f64, f64, &str, &str); %d] = [" % len(props))
    for r in props:
        print('    ("%s", %d, %d, %.3f, %.4f, %.4f, "%s", "%s"),'
              % (r["name"], 1 if r["pha"] else 0, 1 if r["neo"] else 0,
                 r["h"], r["diam"], r["rot"], r["spec"], r["ocls"]))
    print("];")


if __name__ == "__main__":
    main()
