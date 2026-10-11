#!/usr/bin/env python3
"""Bake the EXTENDED real small-body set (1000 catalogued asteroids/comets)
into crates/astro/src/pop_data.rs from the JPL SBDB Query API.

REAL DATA ONLY: every row is a catalogued object with its current SBDB
osculating elements, PHA/NEO flags, H magnitude, diameter and rotation period.
Families (brightest-first by H within each): MBA 420 · APO 130 · ATE 40 ·
AMO 80 · TJN 90 · TNO 130 · IEO 10 · JFC 60 · HTC 40 = 1000.
Rows overlapping the 52-body core SBDB table are skipped.

Usage: python3 tools/fetch_population.py   (needs network)
"""
import urllib.request, urllib.parse, socket, json, re, sys, os
socket.setdefaulttimeout(120)
API = "https://ssd-api.jpl.nasa.gov/sbdb_query.api"
FIELDS = "full_name,epoch,a,e,i,om,w,ma,per,q,pha,neo,H,diameter,class,rot_per"

QUOTA = [  # (label, query params, count)
    ("MBA", {"sb-class": "MBA"}, 420),
    ("APO", {"sb-class": "APO"}, 130),
    ("ATE", {"sb-class": "ATE"}, 40),
    ("AMO", {"sb-class": "AMO"}, 80),
    ("TJN", {"sb-class": "TJN"}, 90),
    ("TNO", {"sb-class": "TNO"}, 130),
    ("IEO", {"sb-class": "IEO"}, 10),
    ("JFC", {"sb-kind": "c", "sb-class": "JFC"}, 60),
    ("HTC", {"sb-kind": "c", "sb-class": "HTC"}, 40),
    ("COM", {"sb-kind": "c"}, 120),  # top-up: remaining catalogued comets (LPC/etc.)
]

CORE = {"CERES","PALLAS","VESTA","HYGIEA","INTERAMNIA","DAVIDA","PSYCHE","LUTETIA","MATHILDE",
 "KLEOPATRA","IDA","GASPAR","JUNO","IRIS","FLORA","EROS","APOPHIS","BENNU","RYUGU","DIDYMOS",
 "ITOKAWA","TOUTATIS","ICARUS","PHAETHON","ATEN","APOLLO","ADONIS","CRUITHNE","1950-DA",
 "GEOGRAPHOS","TORO","2024-YR4","HECTOR","PATROCLUS","ACHILLES","QUAOAR","SEDNA","ORCUS",
 "MAKEMAKE","HAUMEA","ERIS","VARUNA","HALLEY","ENCKE","TEMPEL-1","HALE-BOPP","CHURYUMOV",
 "BORRELLY","WILD-2","SCHWASSMANN-1","SWIFT-TUTTLE","GIACOBINI-ZINNER"}

def fetch(params, limit):
    # NB: server-side sort=H ranks unpublished H (=0) first, so we pull a wide
    # window and order client-side: published-H brightest first, then the rest.
    q = dict(params)
    q.update(fields=FIELDS, **{"full-prec": "1", "limit": str(limit * 6)})
    u = API + "?" + urllib.parse.urlencode(q)
    d = json.loads(urllib.request.urlopen(u).read().decode())
    fields, data = d["fields"], d["data"]
    ix = {f: i for i, f in enumerate(fields)}
    def hval(rec):
        try:
            v = float(rec[ix["H"]])
            return v if v > 0 else 1e9
        except (TypeError, ValueError):
            return 1e9
    data.sort(key=hval)
    return fields, data

def alt_name(n):
    return re.sub(r"^\d+[A-Z]/", "", n)   # 1P/HALLEY -> HALLEY

def clean_name(raw):
    n = raw.strip()
    n = re.sub(r"^\d+\s+", "", n)              # drop number prefix
    core = n.split("(")[0].strip()
    if not core:
        m = re.search(r"\(([^)]+)\)", n)
        core = m.group(1) if m else n
    core = core.upper().replace(" ", "-")
    return core

def main():
    rows, seen = [], set(CORE) | {alt_name(c) for c in CORE}
    per_class = {}
    for label, params, want in QUOTA:
        fields, data = fetch(params, want)
        ix = {f: i for i, f in enumerate(fields)}
        got = 0
        for rec in data:
            kind_c = params.get("sb-kind") == "c"
            name = clean_name(rec[ix["full_name"]])
            stem = alt_name(name).split("-")[0] if kind_c else alt_name(name)
            if not name or name in seen or alt_name(name) in seen or stem in seen:
                continue
            seen.add(name)
            kind = 1 if params.get("sb-kind") == "c" else 0
            def num(i, d=0.0):
                v = rec[ix[i]]
                try: return float(v)
                except (TypeError, ValueError): return d
            rows.append((name, kind, num("epoch"), num("a"), num("e"), num("i"),
                         num("om"), num("w"), num("ma"), num("per"), num("q"),
                         1 if rec[ix["pha"]] == "Y" else 0,
                         1 if rec[ix["neo"]] == "Y" else 0,
                         num("H"), num("diameter"), num("rot_per"),
                         rec[ix["class"]] or label, label))
            per_class[label] = per_class.get(label, 0) + 1
            got += 1
            if got >= want:
                break
        print(f"{label}: {got}", file=sys.stderr)
    # interleave families so any DENSITY prefix is a balanced mix
    buckets = {}
    for r in rows:
        buckets.setdefault(r[17], []).append(r)
    order = [lbl for lbl, _, _ in QUOTA]
    weights = {lbl: cnt / sum(c for _, _, c in QUOTA) for lbl, _, cnt in
               [(lbl, p, len(buckets.get(lbl, []))) for lbl, p, _ in QUOTA]}
    out, idx, emit = [], {lbl: 0 for lbl in order}, 0
    target = {lbl: weights[lbl] for lbl in order}
    acc = {lbl: 0.0 for lbl in order}
    while len(out) < len(rows):
        best = None
        for lbl in order:
            if idx[lbl] >= len(buckets.get(lbl, [])):
                continue
            acc[lbl] += target[lbl]
            if best is None or acc[lbl] > acc[best]:
                best = lbl
            acc[lbl] -= target[lbl]
        if best is None:
            break
        acc[best] -= target[best]
        out.append(buckets[best][idx[best]])
        idx[best] += 1
    here = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "astro", "src", "pop_data.rs")
    L = ["// @generated by tools/fetch_population.py — do not edit.",
         "//! EXTENDED real small-body set: 1000 catalogued asteroids/comets from the",
         "//! JPL SBDB Query API (osculating elements, PHA/NEO flags, H, diameter, rot).",
         "//! Interleaved by family so any DENSITY prefix stays a balanced mix.",
         "//! [name, kind(0 ast/1 comet), epoch JD, a, e, i, om, w, ma, per, q, pha, neo, H, diam, rot h, class]",
         "pub const POP: [(&str, u8, f64, f64, f64, f64, f64, f64, f64, f64, f64, u8, u8, f64, f64, f64, &str); %d] = [" % len(out)]
    for r in out:
        L.append('    ("%s", %d, %.6f, %.9f, %.9f, %.6f, %.6f, %.6f, %.6f, %.6f, %.9f, %d, %d, %.2f, %.1f, %.2f, "%s"),' % r[:17])
    L.append("];")
    open(here, "w").write("\n".join(L) + "\n")
    print(f"wrote {here}: {len(out)} bodies {per_class}", file=sys.stderr)

if __name__ == "__main__":
    main()
