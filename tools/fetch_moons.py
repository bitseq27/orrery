#!/usr/bin/env python3
"""Bake major-moon mean circular elements into crates/astro/src/moon_data.rs.

Everything is fetched from Horizons at epoch J2000 (TLIST=2451545.0):

  EPHEM_TYPE=ELEMENTS, CENTER=<parent>, REF_PLANE=ECLIPTIC gives mean/osculating
  elements referred to the J2000 ECLIPTIC — a, e, i, Ω, ω, M, period:
    A_km    = semi-major axis (fetched, km)
    P_d     = sidereal period (fetched, days; ALWAYS POSITIVE — retrograde motion
              is encoded by i > 90°, matching the mean-circle model in moons.rs,
              where the ecliptic longitude runs ≈ −λ for an i>90 plane)
    i_ecl   = inclination to the ecliptic (0..180°)
    L0      = mean longitude at J2000:  Ω+ω+M (direct)  or  ω+M−Ω (retrograde)

  OBJ_DATA=YES gives GM / Mass / Mean Radius → mass_kg, radius_km (0 = unknown).

Accuracy class: arc-minute (visual + timing correct), stated in UI per §5.1 note.
NOTE: satellite IDs are per-planet (5xx Jupiter, 6xx Saturn, 7xx Uranus, 8xx
Neptune, 9xx Pluto) — the previous revision of this script reused Saturn IDs for
the Galileans, baking garbage longitudes for Io..Callisto.

Usage: python3 tools/fetch_moons.py > crates/astro/src/moon_data.rs
"""
import math
import re
import sys
import urllib.parse
import urllib.request

J2000 = 2451545.0

# name, horizons satellite id, center id, parent NAME
MOONS = [
    # ---- Earth ----
    ("LUNA", "301", "399", "EARTH"),
    # ---- Mars ----
    ("PHOBOS", "401", "499", "MARS"),
    ("DEIMOS", "402", "499", "MARS"),
    # ---- Jupiter: inner + Galilean + irregulars ----
    ("METIS", "516", "599", "JUPITER"),
    ("ADRASTEA", "515", "599", "JUPITER"),
    ("AMALTHEA", "505", "599", "JUPITER"),
    ("THEBE", "514", "599", "JUPITER"),
    ("IO", "501", "599", "JUPITER"),
    ("EUROPA", "502", "599", "JUPITER"),
    ("GANYMEDE", "503", "599", "JUPITER"),
    ("CALLISTO", "504", "599", "JUPITER"),
    ("LEDA", "513", "599", "JUPITER"),
    ("HIMALIA", "506", "599", "JUPITER"),
    ("LYSITHEA", "510", "599", "JUPITER"),
    ("ELARA", "507", "599", "JUPITER"),
    ("ANANKE", "512", "599", "JUPITER"),
    ("CARME", "511", "599", "JUPITER"),
    ("PASIPHAE", "508", "599", "JUPITER"),
    ("SINOPE", "509", "599", "JUPITER"),
    # ---- Saturn ----
    ("JANUS", "610", "699", "SATURN"),
    ("EPIMETHEUS", "611", "699", "SATURN"),
    ("MIMAS", "601", "699", "SATURN"),
    ("ENCELADUS", "602", "699", "SATURN"),
    ("TETHYS", "603", "699", "SATURN"),
    ("DIONE", "604", "699", "SATURN"),
    ("RHEA", "605", "699", "SATURN"),
    ("TITAN", "606", "699", "SATURN"),
    ("HYPERION", "607", "699", "SATURN"),
    ("IAPETUS", "608", "699", "SATURN"),
    ("PHOEBE", "609", "699", "SATURN"),
    ("PROMETHEUS", "616", "699", "SATURN"),
    ("PANDORA", "617", "699", "SATURN"),
    # ---- Uranus ----
    ("MIRANDA", "705", "799", "URANUS"),
    ("ARIEL", "701", "799", "URANUS"),
    ("UMBRIEL", "702", "799", "URANUS"),
    ("TITANIA", "703", "799", "URANUS"),
    ("OBERON", "704", "799", "URANUS"),
    # ---- Neptune ----
    ("LARISSA", "807", "899", "NEPTUNE"),
    ("PROTEUS", "808", "899", "NEPTUNE"),
    ("TRITON", "801", "899", "NEPTUNE"),
    ("NEREID", "802", "899", "NEPTUNE"),
    # ---- Pluto system (barycentre-centred) ----
    ("CHARON", "901", "9", "PLUTO"),
    ("STYX", "905", "9", "PLUTO"),
    ("NIX", "902", "9", "PLUTO"),
    ("KERBEROS", "904", "9", "PLUTO"),
    ("HYDRA", "903", "9", "PLUTO"),
]

G_KM = 6.67430e-20  # km^3 kg^-1 s^-2


def fetch(mid, cen):
    q = {
        "format": "text", "COMMAND": f"'{mid}'", "OBJ_DATA": "YES", "MAKE_EPHEM": "YES",
        "EPHEM_TYPE": "ELEMENTS", "CENTER": f"'{cen}'", "TLIST": f"{J2000}",
        "TLIST_TYPE": "JD", "TIME_TYPE": "TDB", "REF_PLANE": "ECLIPTIC",
        "OUT_UNITS": "KM-D",
    }
    t = urllib.request.urlopen(
        "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(q), timeout=90
    ).read().decode()
    if "$$SOE" not in t:
        raise RuntimeError("no ephemeris in response")
    # sanity: header must name the requested satellite
    hdr = re.search(r"Target body name:\s*(\w+)", t)
    return t, (hdr.group(1).upper() if hdr else "")


def elt(t, tag):
    m = re.search(r"(?<![A-Z])" + tag + r"\s*=\s*([-+0-9.Ee]+)", t)
    return float(m.group(1)) if m else None


def main():
    rows = []
    for name, mid, cen, parent in MOONS:
        t, got = fetch(mid, cen)
        if got and not name.startswith(got[:4]) and got[:4] != name[:4] and not (name == "LUNA" and got == "MOON"):
            print(f"NOTE {name}: header says {got}", file=sys.stderr)
        a_km = elt(t, "A")
        pr_d = elt(t, "PR")
        inc = elt(t, "IN")
        om = elt(t, "OM") or 0.0
        w = elt(t, "W") or 0.0
        ma = elt(t, "MA") or 0.0
        if None in (a_km, pr_d, inc):
            raise RuntimeError(f"{name}: missing elements")
        # mean longitude at J2000; retrograde planes (i>90) run λ backwards
        l0 = ((om + w + ma) % 360.0) if inc < 90.0 else ((w + ma - om) % 360.0)
        # physical data
        mass = 0.0
        mg = re.search(r"GM[^=\n]*=\s*([-+]?[0-9.]+(?:[eE][-+]?[0-9]+)?)", t)
        mm = re.search(r"Mass \(10\^(-?\d+) kg\)\s*=\s*([-+]?[0-9.]+(?:[eE][-+]?[0-9]+)?)", t)
        if mg and float(mg.group(1)) > 0:
            mass = float(mg.group(1)) / G_KM
        elif mm:
            mass = float(mm.group(2)) * 10.0 ** int(mm.group(1))
        rad = 0.0
        mr = re.search(r"Mean Radius \(km\)\s*=\s*([-+]?[0-9.]+(?:[eE][-+]?[0-9]+)?)", t)
        if not mr:
            mr = re.search(r"[Rr]adius[^=\n]*=\s*([-+]?[0-9.]+(?:[eE][-+]?[0-9]+)?)", t)
        if mr:
            rad = float(mr.group(1))
        rows.append((name, parent, a_km, pr_d, inc, l0, mass, rad))
        print(f"fetched {name:12} a={a_km:12.1f} km P={pr_d:9.4f} d i={inc:7.3f}° "
              f"L0(J2000)={l0:8.3f}° mass={mass:.4g} kg r={rad:.1f} km", file=sys.stderr)

    print("// @generated by tools/fetch_moons.py — do not edit.")
    print("//! Major moons: mean-circle elements at J2000, referred to the J2000 ECLIPTIC")
    print("//! (Horizons ELEMENTS, parent-centred): [name, parent, a km, period d (POSITIVE —")
    print("//! retrograde encoded as i > 90°), i_ecl deg, mean longitude L0 deg at J2000,")
    print("//! mass kg (0 = unpublished), mean radius km (0 = unpublished)]. ADR-004.")
    print("pub const MOONS: [(&str, &str, f64, f64, f64, f64, f64, f64); %d] = [" % len(rows))
    for (name, parent, a, pr, inc, l0, mass, rad) in rows:
        print('    ("%s", "%s", %.3f, %.6f, %.4f, %.4f, %.6e, %.1f),'
              % (name, parent, a, pr, inc, l0, mass, rad))
    print("];")


if __name__ == "__main__":
    main()
