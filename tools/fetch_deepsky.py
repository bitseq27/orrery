#!/usr/bin/env python3
"""Bake the DEEP-SKY (100 blurred background objects) and GALAXY (100 blue
spirals) catalogs into crates/astro/src/deep_sky_data.rs + galaxy_data.rs.

RA/Dec/type resolved from SIMBAD TAP (CDS Strasbourg) where possible; the rest
(plus all distances) are published literature values baked as fallback rows.
Distance in Mly (million light-years).

Usage: python3 tools/fetch_deepsky.py   (needs network; falls back gracefully)
"""
import urllib.request, urllib.parse, socket, csv, io, re, sys, os, time

socket.setdefaulttimeout(90)
TAP = "https://simbad.cds.unistra.fr/simbad/sim-tap/sync"
H0 = 70.0            # km/s/Mpc (cz/H0 fallback)
C_KMS = 299792.458
MPC_TO_MLY = 3.2615638

# ---- deep sky: name, simbad id, class, dist Mly, (ra,dec fallback or None)
P = "PULSAR"
DEEP = [
 # pulsars / neutron stars / magnetars
 ("PSR B0531+21","Crab pulsar",P,0.0065,None),
 ("VELA PULSAR","Vela pulsar",P,0.00096,None),
 ("PSR B1919+21","PSR B1919+21",P,0.0023,None),
 ("PSR B0329+54","PSR B0329+54",P,0.0033,None),
 ("PSR J0437-4715","PSR J0437-4715",P,0.00016,None),
 ("GEMINGA","Geminga",P,0.00082,None),
 ("PSR B1257+12","PSR B1257+12",P,0.0023,None),
 ("BLACK WIDOW","PSR B1957+20",P,0.0035,None),
 ("PSR J1748-2446AD","PSR J1748-2446ad",P,0.019,None),
 ("PSR B1509-58","PSR B1509-58",P,0.017,None),
 ("RX J1856.5-3754","RX J1856.5-3754","NEUTRONSTAR",0.0004,None),
 ("SGR 1806-20","SGR 1806-20","MAGNETAR",0.028,(272.135,-20.411)),
 ("SGR 1935+2154","SGR 1935+2154","MAGNETAR",0.030,(293.735,21.897)),
 # quasars / blazars
 ("3C 273","3C 273","QUASAR",749.0,None),
 ("3C 48","3C 48","QUASAR",1300.0,None),
 ("3C 345","3C 345","QUASAR",2000.0,None),
 ("PKS 2155-304","PKS 2155-304","QUASAR",490.0,None),
 ("MRK 421","Mrk 421","QUASAR",434.0,(166.114,38.209)),
 ("MRK 501","Mrk 501","QUASAR",456.0,(253.467,39.760)),
 ("BL LACERTAE","BL Lac","QUASAR",900.0,(330.680,42.278)),
 ("TON 618","TON 618","QUASAR",3500.0,None),
 ("PG 1302-102","PG 1302-102","QUASAR",1050.0,None),
 ("OJ 287","OJ 287","QUASAR",1100.0,(133.704,24.609)),
 ("J1342+0928","ULAS J1342+0928","QUASAR",4200.0,(189.231,9.471)),
 ("J1030+0524","SDSS J1030+0524","QUASAR",4000.0,(157.619,5.415)),
 # supernova remnants
 ("CASSIOPEIA A","Cassiopeia A","SNR",0.011,None),
 ("CYGNUS LOOP","NGC 6960","SNR",0.0024,None),
 ("TYCHO SNR","Tycho SNR","SNR",0.009,None),
 ("KEPLER SNR","Kepler SNR","SNR",0.020,None),
 ("SN 1987A","SN 1987A","SNR",0.163,None),
 # planetary nebulae
 ("M 57 RING","M 57","PNEB",0.0023,None),
 ("HELIX","NGC 7293","PNEB",0.00065,None),
 ("CAT'S EYE","NGC 6543","PNEB",0.0033,None),
 ("M 27 DUMBBELL","M 27","PNEB",0.0014,None),
 ("ESKIMO","NGC 2392","PNEB",0.0040,None),
 ("OWL","M 97","PNEB",0.0020,None),
 ("SOUTHERN RING","NGC 3132","PNEB",0.0020,None),
 ("NGC 7027","NGC 7027","PNEB",0.0030,None),
 ("ANT NEBULA","Menzel 3","PNEB",0.0080,(244.297,-51.038)),
 # emission / HII regions
 ("M 42 ORION","M 42","HII",0.00134,None),
 ("M 8 LAGOON","M 8","HII",0.0041,None),
 ("M 16 EAGLE","M 16","HII",0.0057,None),
 ("M 20 TRIFID","M 20","HII",0.0052,None),
 ("M 17 OMEGA","M 17","HII",0.0055,None),
 ("CARINA NEBULA","Carina nebula","HII",0.0085,None),
 ("ROSETTE","NGC 2237","HII",0.0052,None),
 ("CALIFORNIA","NGC 1499","HII",0.0010,None),
 ("NORTH AMERICA","NGC 7000","HII",0.0022,None),
 ("TARANTULA","30 Doradus","HII",0.163,(84.677,-69.101)),
 # dark nebulae
 ("HORSEHEAD","IC 434","DARK",0.00137,None),
 ("COALSACK","Coalsack","DARK",0.0006,None),
 # clusters
 ("M 45 PLEIADES","M 45","CLUSTER",0.000444,None),
 ("HYADES","Hyades","CLUSTER",0.000153,None),
 ("M 44 BEEHIVE","M 44","CLUSTER",0.000577,None),
 ("H PERSEI","h Per","CLUSTER",0.0070,(35.450,57.130)),
 ("OMEGA CEN","NGC 5139","CLUSTER",0.0158,None),
 ("47 TUCANAE","47 Tuc","CLUSTER",0.0150,(6.024,-72.081)),
 ("M 13","M 13","CLUSTER",0.022,None),
 ("M 22","M 22","CLUSTER",0.0106,None),
 ("NGC 6397","NGC 6397","CLUSTER",0.0078,None),
 # dwarf galaxies
 ("LMC","Large Magellanic Cloud","DWARF",0.163,None),
 ("SMC","Small Magellanic Cloud","DWARF",0.200,None),
 ("SAGITTARIUS DSPH","Sagittarius dSph","DWARF",0.070,(283.831,-30.545)),
 ("SCULPTOR DSPH","Sculptor dSph","DWARF",0.280,(0.004,-33.707)),
 ("FORNAX DSPH","Fornax dwarf spheroidal galaxy","DWARF",0.460,None),
 ("CARINA DSPH","Carina dSph","DWARF",0.330,(100.403,-50.966)),
 ("DRACO DSPH","Draco dwarf spheroidal galaxy","DWARF",0.250,None),
 ("URSA MINOR DSPH","Ursa Minor dSph","DWARF",0.200,(227.296,67.223)),
 ("LEO I","Leo I","DWARF",0.820,None),
 ("LEO II","Leo II","DWARF",0.690,None),
 ("SEXTANS DSPH","Sextans dSph","DWARF",0.280,(153.266,-1.615)),
 ("NGC 6822","NGC 6822","DWARF",1.60,None),
 ("IC 1613","IC 1613","DWARF",2.40,None),
 ("WLM","WLM","DWARF",3.00,(0.492,-15.464)),
 ("TUCANA DSPH","Tucana dSph","DWARF",2.80,(340.790,-64.419)),
 ("DDO 216","DDO 216","DWARF",2.70,None),
 ("IC 10","IC 10","DWARF",2.50,None),
 ("NGC 147","NGC 147","DWARF",2.50,None),
 # stars (giants / supergiants / variables)
 ("BETELGEUSE","Betelgeuse","STAR",0.000548,None),
 ("DENEB","Deneb","STAR",0.0026,None),
 ("RIGEL","Rigel","STAR",0.00086,None),
 ("ANTARES","Antares","STAR",0.00055,None),
 ("VY CANIS MAJORIS","VY CMa","STAR",0.0039,(110.743,-25.767)),
 ("PISTOL STAR","Pistol Star","STAR",0.025,None),
 ("RHO CASSIOPEIAE","Rho Cas","STAR",0.010,(340.823,50.511)),
 ("LBV 1806-20","LBV 1806-20","STAR",0.030,None),
 ("MIRA","Mira","STAR",0.0003,None),
 ("POLARIS","Polaris","STAR",0.000433,None),
 ("SIRIUS","Sirius","STAR",0.0000086,None),
 ("PROXIMA CENTAURI","Proxima Centauri","STAR",0.0000042,None),
 ("ALNITAK","Alnitak","STAR",0.00126,None),
 # degenerate / sub-stellar
 ("VAN MAANEN 2","van Maanen 2","WHITEDWARF",0.000014,None),
 ("40 ERI B","40 Eri B","WHITEDWARF",0.000016,None),
 ("LUHMAN 16","Luhman 16","BROWNDWARF",0.0000065,None),
 ("WISE 0855-0714","WISE 0855-0714","BROWNDWARF",0.0000074,(133.795,-7.245)),
 # black holes / microquasars
 ("SAGITTARIUS A*","Sagittarius A*","BLACKHOLE",0.0267,None),
 ("CYGNUS X-1","Cygnus X-1","BLACKHOLE",0.006,(299.590,35.202)),
 ("GRS 1915+105","GRS 1915+105","MICROQUASAR",0.028,(288.798,10.946)),
 ("SS 433","SS 433","MICROQUASAR",0.018,None),
]

# ---- galaxies: name, simbad id, morph, dist Mly, size arcmin, (ra,dec fb)
GALS = [
 ("M 31 ANDROMEDA","M 31","S",2.54,178,None),("M 32","M 32","E",2.54,8,None),
 ("M 110","M 110","E",2.90,21,None),("M 33 TRIANGULUM","M 33","S",2.73,71,None),
 ("M 51 WHIRLPOOL","M 51","S",23,11,None),("NGC 5195","NGC 5195","I",23,6,None),
 ("M 81 BODE'S","M 81","S",11.8,26,None),("M 82 CIGAR","M 82","I",11.8,11,None),
 ("NGC 3077","NGC 3077","I",11.8,5,None),("M 101 PINWHEEL","M 101","S",21,28,None),
 ("M 63 SUNFLOWER","M 63","S",29,16,None),("M 94","M 94","S",16,11,None),
 ("M 64 BLACK EYE","M 64","S",17,10,None),("M 104 SOMBRERO","M 104","S",29.3,9,None),
 ("M 83 SOUTH. PINWHEEL","M 83","S",15,12,None),("NGC 253 SCULPTOR","NGC 253","I",11.4,30,None),
 ("NGC 55","NGC 55","I",6.5,14,None),("NGC 300","NGC 300","S",6.6,10,None),
 ("NGC 247","NGC 247","S",11,8,None),("NGC 7793","NGC 7793","S",12,10,None),
 ("IC 342","IC 342","S",11,21,None),("NGC 6946 FIREWORKS","NGC 6946","S",22,11,None),
 ("M 74","M 74","S",32,10,None),("NGC 1232","NGC 1232","S",65,25,None),
 ("NGC 1365 GREAT BARRED","NGC 1365","S",56,11,None),("NGC 1300","NGC 1300","S",61,10,None),
 ("NGC 2903","NGC 2903","S",30,12,None),("NGC 4565 NEEDLE","NGC 4565","S",43,16,None),
 ("NGC 891","NGC 891","S",30,14,None),("NGC 4631 WHALE","NGC 4631","I",30,15,None),
 ("NGC 4438 EYES","NGC 4438","S",52,8,None),("M 87","M 87","E",53.5,7,None),
 ("M 84","M 84","E",60,6,None),("M 86","M 86","E",52,9,None),
 ("M 49","M 49","E",56,9,None),("M 59","M 59","E",60,5,None),
 ("M 60","M 60","E",60,7,None),("M 89","M 89","E",50,5,None),
 ("NGC 1399","NGC 1399","E",65,9,None),("NGC 1316 FORNAX A","NGC 1316","E",60,12,None),
 ("NGC 5128 CENTAURUS A","NGC 5128","E",12,50,None),("NGC 1275 PERSEUS A","NGC 1275","E",237,8,None),
 ("NGC 4889","NGC 4889","E",308,5,None),("M 77 CETUS A","M 77","S",47,7,None),
 ("M 58","M 58","S",60,6,None),("M 61","M 61","S",45,6,None),
 ("M 65","M 65","S",35,10,None),("M 66","M 66","S",35,9,None),
 ("M 95","M 95","S",32,7,None),("M 96","M 96","S",31,7,None),
 ("M 99","M 99","S",50,5,None),("M 100","M 100","S",55,7,None),
 ("M 98","M 98","S",44,9,None),("M 90","M 90","S",60,9,None),
 ("NGC 4038 ANTENNAE","NGC 4038","I",45,5,None),("NGC 4676 MICE","NGC 4676","I",100,4,None),
 ("NGC 4414","NGC 4414","S",62,8,None),("NGC 2841","NGC 2841","S",46,8,None),
 ("NGC 3184","NGC 3184","S",25,7,None),("NGC 5033","NGC 5033","S",40,10,None),
 ("NGC 7331","NGC 7331","S",40,10,None),("NGC 4214","NGC 4214","I",9.6,9,None),
 ("NGC 4449","NGC 4449","I",12,7,None),("NGC 1569","NGC 1569","I",11,5,None),
 ("NGC 5253","NGC 5253","I",11,4,None),("IC 1101","IC 1101","E",1040,100,None),
 ("M 106","NGC 4258","S",23.7,19,None),("NGC 4650A POLAR-RING","NGC 4650A","S",100,3,None),
 ("NGC 4921","NGC 4921","S",320,4,None),("NGC 3370","NGC 3370","S",111,5,None),
 ("NGC 3982","NGC 3982","S",68,4,None),("NGC 4088","NGC 4088","S",50,6,None),
 ("NGC 2146","NGC 2146","S",46,6,None),("NGC 6503","NGC 6503","S",18,6,None),
 ("NGC 1427A","NGC 1427A","I",62,4,None),("NGC 1404","NGC 1404","E",60,5,None),
 ("NGC 4697","NGC 4697","E",40,6,None),("NGC 4395","NGC 4395","S",14,8,None),
 ("NGC 404","NGC 404","I",10,4,None),("NGC 4151 EYE OF SAURON","NGC 4151","S",62,10,None),
 ("NGC 2403","NGC 2403","S",10.6,22,None),("NGC 7479","NGC 7479","S",106,6,None),
 ("NGC 5248","NGC 5248","S",50,5,None),("NGC 4639","NGC 4639","S",72,4,None),
 ("NGC 3949","NGC 3949","S",45,5,None),("NGC 2683","NGC 2683","S",25,8,None),
 ("NGC 1672","NGC 1672","S",60,7,None),("NGC 1512","NGC 1512","S",35,7,None),
 ("NGC 925","NGC 925","S",30,8,None),("NGC 2500","NGC 2500","S",30,4,None),
 ("NGC 4656 HOCKEY STICK","NGC 4656","I",30,15,None),
 ("NGC 3628 HAMBURGER","NGC 3628","S",35,9,None),
 ("NGC 5005","NGC 5005","S",69,5,None),
 ("NGC 7479","NGC 7479","S",106,6,None),
 ("SEXTANS A","Sextans A","I",4.3,5,None),
 # high-redshift record holders (literature coordinates; drawn as faint smudges)
 ("GN-Z11","GN-z11","S",10300,1,(189.231,62.237)),
 ("COSMOS REDSHIFT 7","Cosmos Redshift 7","S",10000,1,(150.100,2.207)),
 ("HIMIKO","HIMIKO","I",9600,1,(34.533,-4.806)),
 ("GLASS-Z13","GLASS-z13","I",10400,1,(3.569,-30.398)),
 ("A2744-YD4","A2744-YD4","I",10100,1,(3.685,-30.391)),
 ("Z8 GND 5296","z8_GND_5296","I",9700,1,(52.718,27.906)),
]

def tap(query, tries=4):
    p = urllib.parse.urlencode({"REQUEST":"doQuery","LANG":"ADQL","FORMAT":"csv","QUERY":query}).encode()
    for t in range(tries):
        try:
            return urllib.request.urlopen(urllib.request.Request(TAP, data=p)).read().decode()
        except Exception as e:
            print("TAP try %d: %s" % (t, e), file=sys.stderr)
            time.sleep(2 + 4*t)
    return ""

def resolve(names):
    out = {}
    uniq = sorted(set(names))
    for i in range(0, len(uniq), 8):
        chunk = uniq[i:i+8]
        inlist = ",".join("'" + n.replace("'", "''") + "'" for n in chunk)
        q = ("SELECT i.id, b.ra, b.dec, b.otype_txt, b.rvz_redshift FROM ident AS i "
             "JOIN basic AS b ON i.oidref = b.oid WHERE i.id IN (%s)" % inlist)
        txt = tap(q)
        if not txt: continue
        try:
            for row in csv.DictReader(io.StringIO(txt)):
                ra, dec = float(row["ra"]), float(row["dec"])
                z = row.get("rvz_redshift") or ""
                try: z = float(z)
                except ValueError: z = None
                out[row["id"].strip().lower()] = (ra, dec, row.get("otype_txt","").strip(), z)
        except Exception:
            pass
    return out

def norm(s): return re.sub(r"[^a-z0-9]", "", s.lower())

def lookup(res, sid):
    n = norm(sid)
    for k, v in res.items():
        if norm(k) == n: return v
    for k, v in res.items():
        if n in norm(k) or norm(k) in n: return v
    return None

def fl(v):
    """%.6g but always a valid Rust f64 literal."""
    t = "%.6g" % v
    if not re.search(r"[.eE]", t): t += ".0"
    return t

def dist_mly(d, z):
    if d > 0: return d
    if z and z > 0: return (z * C_KMS / H0) * MPC_TO_MLY  # cz/H0 in Mpc × 3.26 = Mly
    return 1000.0

def main():
    here = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "astro", "src")
    res = resolve([x[1] for x in DEEP] + [x[1] for x in GALS])
    print("SIMBAD resolved %d ids" % len(res), file=sys.stderr)

    ds_rows, miss = [], []
    for (name, sid, cls, dist, fb) in DEEP:
        hit = lookup(res, sid)
        if hit: ra, dec, _ot, z = hit; d = dist_mly(dist, z)
        elif fb: ra, dec = fb; d = dist if dist > 0 else 1000.0
        else: miss.append(sid); continue
        ds_rows.append('    ("%s", "%s", %.6f, %.6f, %s),' % (name, cls, ra, dec, fl(d)))
    ds_rows = ds_rows[:100]
    print("deep-sky %d rows; unresolved (dropped): %s" % (len(ds_rows), miss), file=sys.stderr)

    gx_rows, gmiss = [], []
    for (name, sid, morph, dist, size, fb) in GALS:
        hit = lookup(res, sid)
        if hit: ra, dec, _ot, z = hit; d = dist_mly(dist, z)
        elif fb: ra, dec = fb; d = dist if dist > 0 else 1000.0
        else: gmiss.append(sid); continue
        gx_rows.append('    ("%s", "%s", %.6f, %.6f, %s, %.1f),' % (name, morph, ra, dec, fl(d), size))
    gx_rows = gx_rows[:100]
    print("galaxies %d rows; unresolved (dropped): %s" % (len(gx_rows), gmiss), file=sys.stderr)

    with open(os.path.join(here, "deep_sky_data.rs"), "w") as f:
        f.write("// @generated by tools/fetch_deepsky.py — do not edit.\n")
        f.write("//! DEEP-SKY background catalog (max 100): [name, class, RA deg, Dec deg, distance Mly].\n")
        f.write("//! Coordinates: SIMBAD (CDS); published-value fallbacks where SIMBAD lacks the id.\n")
        f.write("//! Classes: PULSAR NEUTRONSTAR MAGNETAR QUASAR SNR PNEB HII DARK CLUSTER DWARF STAR\n")
        f.write("//!          WHITEDWARF BROWNDWARF BLACKHOLE MICROQUASAR.\n")
        f.write("pub const DEEP_SKY: [(&str, &str, f64, f64, f64); %d] = [\n%s\n];\n" % (len(ds_rows), "\n".join(ds_rows)))
    with open(os.path.join(here, "galaxy_data.rs"), "w") as f:
        f.write("// @generated by tools/fetch_deepsky.py — do not edit.\n")
        f.write("//! GALAXY catalog (max 100): [name, morphology S/E/I, RA deg, Dec deg, distance Mly,\n")
        f.write("//! apparent major size arcmin]. Coordinates: SIMBAD (CDS) / literature for high-z.\n")
        f.write("pub const GALAXIES: [(&str, &str, f64, f64, f64, f64); %d] = [\n%s\n];\n" % (len(gx_rows), "\n".join(gx_rows)))

if __name__ == "__main__":
    main()
