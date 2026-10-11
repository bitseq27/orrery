#!/usr/bin/env python3
"""Fetch heliocentric ecliptic-J2000 state vectors for interplanetary probes
from JPL Horizons and bake crates/astro/src/probe_data.rs.

Horizons covers every tracked spacecraft; silent-but-intact craft (Ulysses,
Dawn, Kepler, Spitzer, Rosetta on 67P, NEAR on Eros, ...) have extrapolated
SPKs — we store the LAST available state and mark them SILENT. Destroyed
probes (Cassini, MESSENGER, Galileo, Venus Express) are deliberately NOT
baked: there is no object left to show.

Usage: python3 tools/fetch_probes.py            (needs network)
       writes crates/astro/src/probe_data.rs
"""
import urllib.request, urllib.parse, socket, re, sys, os
socket.setdefaulttimeout(60)
API = "https://ssd.jpl.nasa.gov/api/horizons.api"
EPOCH_JD = 2461324.5  # A.D. 2026-Oct-11 00:00 TDB bake epoch

# label, Horizons COMMAND, agency, launch year, mission target, status code, status note
# status: 0 = ACTIVE, 1 = SILENT (intact, SPK extrapolated from last tracking)
PROBES = [
    ("VOYAGER 1",        "-31",  "NASA",       1977, "JUPITER·SATURN·INTERSTELLAR", 0, ""),
    ("VOYAGER 2",        "-32",  "NASA",       1977, "J·S·URANUS·NEPTUNE·INTERSTELLAR", 0, ""),
    ("PIONEER 10",       "-23",  "NASA",       1972, "JUPITER·HELIOCENTRIC ESCAPE",  1, "LAST SIGNAL 2003"),
    ("PIONEER 11",       "-24",  "NASA",       1973, "JUPITER·SATURN·ESCAPE",        1, "LAST SIGNAL 1995"),
    ("NEW HORIZONS",     "-98",  "NASA",       2006, "PLUTO·KUIPER BELT",            0, ""),
    ("PARKER SOLAR PROBE","-96", "NASA",       2018, "SOLAR CORONA",                 0, ""),
    ("SOLAR ORBITER",    "-144", "ESA",        2020, "INNER HELIOSPHERE",            0, ""),
    ("BEPICOLOMBO",      "-121", "ESA·JAXA",   2018, "MERCURY",                      0, ""),
    ("JUNO",             "-61",  "NASA",       2011, "JUPITER",                      0, ""),
    ("OSIRIS-APEX",      "-64",  "NASA",       2016, "BENNU→APOPHIS",                0, ""),
    ("LUCY",             "-49",  "NASA",       2021, "JUPITER TROJANS",              0, ""),
    ("PSYCHE",           "-255", "NASA",       2023, "(16) PSYCHE",                  0, ""),
    ("HERA",             "HERA", "ESA",        2024, "DIDYMOS·DIMORPHOS",            0, ""),
    ("JUICE",            "-28",  "ESA",        2023, "JUPITER ICY MOONS",            0, ""),
    ("EUROPA CLIPPER",   "-159", "NASA",       2024, "EUROPA",                       0, ""),
    ("MARS EXPRESS",     "-41",  "ESA",        2003, "MARS",                         0, ""),
    ("TRACE GAS ORBITER","-143", "ESA",        2016, "MARS ATMOSPHERE",              0, ""),
    ("MRO",              "MARS RECONNAISSANCE ORBITER", "NASA", 2005, "MARS",        0, ""),
    ("MAVEN",            "MAVEN","NASA",       2013, "MARS UPPER ATMOSPHERE",        0, ""),
    ("DANURI",           "DANURI","KARI",      2022, "MOON",                         0, ""),
    ("HAYABUSA2",        "Hayabusa 2","JAXA",  2014, "RYUGU→1998 KY26",              0, ""),
    ("JWST",             "-170", "NASA·ESA·CSA",2021,"SUN-EARTH L2",                 0, ""),
    ("HUBBLE",           "HST",  "NASA·ESA",   1990, "LOW EARTH ORBIT",              0, ""),
    ("ULYSSES",          "-55",  "ESA·NASA",   1990, "SOLAR POLAR ORBIT",            1, "LAST SIGNAL 2009"),
    ("DAWN",             "DAWN", "NASA",       2007, "VESTA·CERES",                  1, "SILENT AT CERES 2018"),
    ("SOHO",             "-21",  "ESA·NASA",   1995, "SUN-EARTH L1",                 0, ""),
    ("ACE",              "-92",  "NASA",       1997, "SUN-EARTH L1",                 0, ""),
    ("DSCOVR",           "DSCOVR","NASA·NOAA", 2015, "SUN-EARTH L1",                 0, ""),
    ("IMAP",             "IMAP", "NASA",       2025, "SUN-EARTH L1",                 0, ""),
    ("ADITYA-L1",        "ADITYA-L1","ISRO",   2023, "SUN-EARTH L1",                 0, ""),
    ("TESS",             "TESS", "NASA",       2018, "HEO EXOPLANET SURVEY",         0, ""),
    ("GAIA",             "GAIA", "ESA",        2013, "L2→HELIOCENTRIC DISPOSAL",     1, "RETIRED 2025"),
    ("SPITZER",          "SPITZER","NASA",     2003, "HELIOCENTRIC TRAILING",        1, "RETIRED 2020"),
    ("KEPLER",           "-227", "NASA",       2009, "HELIOCENTRIC TRAILING",        1, "RETIRED 2018"),
    ("ROSETTA",          "ROSETTA","ESA",      2004, "67P/CHURYUMOV (LANDED)",       1, "ON 67P SINCE 2016"),
    ("AKATSUKI",         "AKATSUKI","JAXA",    2010, "VENUS",                        1, "SILENT SINCE 2025"),
    ("CAPSTONE",         "CAPSTONE","NASA",    2022, "LUNAR NRHO",                   0, ""),
    ("CHANDRAYAAN-2",    "-152", "ISRO",       2019, "MOON",                         0, ""),
    ("DEEP IMPACT",      "-140", "NASA",       2005, "TEMPEL 1→HARTLEY 2 (EPOXI)",   1, "LAST SIGNAL 2013"),
    ("NEAR",             "NEAR", "NASA",       1996, "EROS (LANDED)",                1, "ON EROS SINCE 2001"),
]

MON = {m: i + 1 for i, m in enumerate(
    ["JAN","FEB","MAR","APR","MAY","JUN","JUL","AUG","SEP","OCT","NOV","DEC"])}

def query(cmd, jd):
    p = {"format": "text", "COMMAND": f"'{cmd}'", "OBJ_DATA": "NO", "MAKE_EPHEM": "YES",
         "EPHEM_TYPE": "VECTORS", "CENTER": "'500@10'", "TLIST": f"{jd:.5f}",
         "REF_SYSTEM": "J2000", "REF_PLANE": "ECLIPTIC", "OUT_UNITS": "AU-D",
         "TIME_TYPE": "TDB", "VEC_TABLE": "2", "CSV_FORMAT": "NO"}
    url = API + "?" + urllib.parse.urlencode(p)
    return urllib.request.urlopen(url).read().decode()

def date_to_jd(s):
    m = re.search(r"A\.D\.\s*(\d+)-([A-Z]{3})-(\d+)", s)
    y, mo, d = int(m.group(1)), MON[m.group(2)], int(m.group(3))
    a = (14 - mo) // 12; yy = y + 4800 - a; mm = mo + 12 * a - 3
    return d + (153 * mm + 2) // 5 + 365 * yy + yy // 4 - yy // 100 + yy // 400 - 32045 - 0.5

def fetch(cmd, jd=EPOCH_JD, tries=4):
    txt = query(cmd, jd)
    m = re.search(r"\$\$SOE\n(.*?)\$\$EOE", txt, re.S)
    if m:
        b = m.group(1)
        g = lambda t: float(re.search(t + r"\s*=\s*([-+0-9.Ee]+)", b).group(1))
        return jd, [g("X"), g("Y"), g("Z"), g("VX"), g("VY"), g("VZ")]
    m = re.search(r"No ephemeris for target .* after (A\.D\.[^\n.]*\.?\d*)", txt)
    if m and tries:
        return fetch(cmd, date_to_jd(m.group(1)) - 0.5, tries - 1)
    raise RuntimeError(f"{cmd}: no ephemeris — {txt[txt.find('!'):][:200]}")

def main():
    out = os.path.join(os.path.dirname(__file__), "..", "crates", "astro", "src", "probe_data.rs")
    lines = ["// @generated by tools/fetch_probes.py — do not edit.",
             "//! JPL Horizons heliocentric ecliptic-J2000 states (AU, AU/day, TDB) for tracked",
             "//! interplanetary probes. SILENT craft keep their LAST tracked state; the app",
             "//! propagates bound states with Kepler two-body and escaping states linearly.",
             "//! [name, agency, launch_year, target, status(0 ACTIVE/1 SILENT), status_note,",
             "//!  epoch JD, x, y, z, vx, vy, vz]",
             "pub const PROBE_EPOCH_JD: f64 = %.1f;" % EPOCH_JD, ""]
    rows = []
    for (label, cmd, agency, yr, target, status, note) in PROBES:
        jd, v = fetch(cmd)
        print(f"{label:20s} jd={jd:11.1f} r={ (v[0]**2+v[1]**2+v[2]**2)**0.5:8.3f} AU", file=sys.stderr)
        rows.append('    ("%s", "%s", %d, "%s", %d, "%s", %.1f, %.9f, %.9f, %.9f, %.9f, %.9f, %.9f),'
                    % (label, agency, yr, target, status, note, jd, *v))
    lines.append("pub const PROBES: [(&str, &str, u32, &str, u8, &str, f64, f64, f64, f64, f64, f64, f64); %d] = [" % len(rows))
    lines += rows
    lines.append("];")
    open(out, "w").write("\n".join(lines) + "\n")
    print(f"wrote {out} ({len(rows)} probes)", file=sys.stderr)

if __name__ == "__main__":
    main()
