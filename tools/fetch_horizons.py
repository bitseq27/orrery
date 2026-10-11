#!/usr/bin/env python3
"""Fetch reference data for ORRERY M1 verification (offline-safe: results are committed).

1. tests/data/horizons_vectors.json — JPL Horizons heliocentric ICRF vectors,
   9 bodies x 3 epochs (2000-01-01, 2026-10-09, 2100-01-01), AU, for the <1" test.
2. prints the JPL approximate-position Keplerian table (Pluto etc.) for embedding.
"""
import json, re, urllib.request, urllib.parse, sys

OUT = "/home/user/Orrery/tests/data/horizons_vectors.json"

BODIES = {  # name: horizons id
    "mercury": "199", "venus": "299", "emb": "3", "mars": "499",
    "jupiter": "599", "saturn": "699", "uranus": "799", "neptune": "899",
    "pluto": "999", "earth": "399",
}
EPOCHS_JD = [2451545.0, 2461322.5, 2488070.0]  # 2000-01-01, 2026-10-09, 2100-01-01 TT


def horizons_vectors(body_id: str, jd: float):
    q = {
        "format": "text", "COMMAND": f"'{body_id}'", "OBJ_DATA": "NO",
        "MAKE_EPHEM": "YES", "EPHEM_TYPE": "VECTORS", "CENTER": "'500@10'",
        "TLIST": f"{jd}", "REF_SYSTEM": "ICRF", "OUT_UNITS": "AU-D", "TIME_TYPE": "TDB",
        "VEC_TABLE": "2", "CSV_FORMAT": "NO",
    }
    url = "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(q)
    txt = urllib.request.urlopen(url, timeout=60).read().decode()
    m = re.search(r"\$\$SOE\n(.*?)\$\$EOE", txt, re.S)
    if not m:
        raise RuntimeError(f"no SOE for {body_id}: {txt[:400]}")
    block = m.group(1)
    def grab(tag):
        mm = re.search(tag + r"\s*=\s*([-+0-9.Ee]+)", block)
        return float(mm.group(1))
    pos = [grab("X"), grab("Y"), grab("Z")]
    vel = [grab("VX"), grab("VY"), grab("VZ")]
    return {"jd": jd, "pos": pos, "vel": vel}


def main():
    data = {}
    for name, bid in BODIES.items():
        data[name] = [horizons_vectors(bid, jd) for jd in EPOCHS_JD]
        import sys; print(f"fetched {name}", data[name][0]["pos"], file=sys.stderr)
    # NOTE: sandbox VFS drops files written by long-lived python processes;
    # emit on stdout and let the shell redirect (persisted reliably).
    print(json.dumps(data, indent=1))


if __name__ == "__main__":
    main()
