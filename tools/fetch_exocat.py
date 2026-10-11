#!/usr/bin/env python3
"""Fetch the §6.2 ten-system catalog from the NASA Exoplanet Archive TAP service
and emit assets/catalog.json (shell-redirect; see BUILD_LOG VFS note).

Schema notes (2026): eccentricity = pl_orbeccen, periastron time = pl_orbtper (JD),
errors = *err1/*err2. The archive carries NO longitude-of-ascending-node column for
RV-discovered orbits (node degeneracy) → Ω stored as null and set to 0 in-app,
flagged in the UI provenance row (§5.2 honesty).

Systems with fewer than 3 confirmed planets in the archive are replaced by the
nearest ≥3-planet host (R5) and the swap is printed to stderr for the build log.
"""
import json
import sys
import urllib.parse
import urllib.request

TAP = "https://exoplanetarchive.ipac.caltech.edu/TAP/sync"

SYSTEMS = [
    "55 Cnc", "GJ 876", "HD 10180", "HD 40307", "TRAPPIST-1",
    "GJ 667 C", "51 Oph", "47 UMa", "ups And", "HD 71255",
]
# fallback pool if a plan system has <3 confirmed planets (nearby, ≥3 planets)
FALLBACK = ["GJ 581", "HD 69830", "GJ 3293", "HD 40307"]

COLS = ("pl_name,hostname,pl_orbsmax,pl_orbsmaxerr1,pl_orbeccen,pl_orbeccenerr1,"
        "pl_orbincl,pl_orblper,pl_orbtper,pl_orbper,pl_bmassj,pl_radj,"
        "st_mass,st_rad,st_spectype,st_teff,ra,dec,sy_dist,sy_plx,sy_vmag")


def tap(query):
    u = TAP + "?" + urllib.parse.urlencode({"query": query, "format": "csv"})
    txt = urllib.request.urlopen(u, timeout=120).read().decode()
    lines = [l for l in txt.strip().splitlines() if l]
    if not lines:
        return []
    hdr = [h.strip() for h in lines[0].split(",")]
    out = []
    for ln in lines[1:]:
        parts = next_csv_row(ln)
        out.append(dict(zip(hdr, parts)))
    return out


def next_csv_row(ln):
    # archive csv quotes strings containing spaces
    import csv
    import io
    return [c.strip().strip('"') for c in next(csv.reader(io.StringIO(ln)))]


def num(d, k):
    v = d.get(k, "")
    if v in ("", "NULL", None):
        return None
    try:
        return float(v)
    except ValueError:
        return None


def fetch_system(host):
    q = f"select {COLS} from pscomppars where hostname='{host}' order by pl_name"
    rows = tap(q)
    return rows


def build(rows, host):
    planets = []
    for r in rows:
        a, e = num(r, "pl_orbsmax"), num(r, "pl_orbeccen")
        per, tp = num(r, "pl_orbper"), num(r, "pl_orbtper")
        if a is None or per is None or a <= 0:
            continue
        if e is None:
            e = 0.0
        # mean longitude at J2000: M0 = 360*(J2000 - tp)/per ; l0 = M0 + ϖ
        m0 = None
        if tp is not None:
            m0 = (360.0 * (2451545.0 - tp) / per) % 360.0
        planets.append({
            "name": r["pl_name"],
            "a_au": a,
            "a_err": num(r, "pl_orbsmaxerr1"),
            "e": e,
            "e_err": num(r, "pl_orbeccenerr1"),
            "i_deg": num(r, "pl_orbincl"),
            "omega_deg": None,           # not published for RV orbits (node degeneracy)
            "w_deg": num(r, "pl_orblper"),
            "m0_deg": m0,
            "period_d": per,
            "mass_mj": num(r, "pl_bmassj"),
            "radius_rj": num(r, "pl_radj"),
        })
    if not rows:
        return None
    r0 = rows[0]
    return {
        "host": {
            "name": host,
            "spectral": r0.get("st_spectype") or "",
            "mass_msun": num(r0, "st_mass"),
            "radius_rsun": num(r0, "st_rad"),
            "teff_k": num(r0, "st_teff"),
            "ra_deg": num(r0, "ra"),
            "dec_deg": num(r0, "dec"),
            "dist_ly": (lambda pc: None if pc is None else round(pc * 3.26156, 3))(
                num(r0, "sy_dist") or (1000.0 / num(r0, "sy_plx") if num(r0, "sy_plx") else None)
            ),
            "vmag": num(r0, "sy_vmag"),
        },
        "planets": planets,
    }


def main():
    catalog = {"schema": "orrery-catalog-1", "systems": []}
    pool = list(FALLBACK)
    idx = 1
    for host in SYSTEMS:
        rows = fetch_system(host)
        sysobj = build(rows, host)
        if sysobj is None or len(sysobj["planets"]) < 3:
            got = 0 if sysobj is None else len(sysobj["planets"])
            repl = None
            while pool:
                cand = pool.pop(0)
                if cand == host or cand in SYSTEMS:
                    continue
                r2 = fetch_system(cand)
                s2 = build(r2, cand)
                if s2 and len(s2["planets"]) >= 3:
                    repl = (cand, s2)
                    break
            if repl:
                print(f"SWAP {host} ({got} confirmed) -> {repl[0]} ({len(repl[1]['planets'])} planets)", file=sys.stderr)
                host, sysobj = repl
            else:
                print(f"KEEP {host} with only {got} confirmed planets (plan table)", file=sys.stderr)
                if sysobj is None:
                    continue
        sysobj["idx"] = idx
        idx += 1
        catalog["systems"].append(sysobj)
        print(f"system {host}: {len(sysobj['planets'])} planets", file=sys.stderr)
    print(json.dumps(catalog, indent=1))


if __name__ == "__main__":
    main()
