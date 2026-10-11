#!/bin/bash
# ORRERY visual-regression harness: renders the locked state set into docs/shots/.
# Requires: Xvfb, scrot, a built ./target/debug/orrery (see BUILD_LOG.md).
set -e
cd "$(dirname "$0")/.."
export DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1
Xvfb :99 -screen 0 1600x900x24 >/tmp/xvfb_orrery.log 2>&1 &
XPID=$!
sleep 2
shot() { # name, env...
  local name=$1; shift
  env "$@" ./target/debug/orrery >/tmp/orrery_shot.log 2>&1 &
  local APID=$!
  sleep "${SHOT_SLEEP:-14}"
  scrot -o "docs/shots/${name}.png"
  kill $APID 2>/dev/null || true
  sleep 1
}
# --- shell & layout (right panel: sky map / residual / data link) -------------
shot REG_shell          ORRERY_JD=2461322.5
shot REG_skymap         ORRERY_JD=2461322.5 ORRERY_SYS=5
shot REG_resid          ORRERY_RSCROLL=0
shot REG_datalink       ORRERY_LINK=1 ORRERY_RSCROLL=170
# --- sol scene ---------------------------------------------------------------
shot REG_scene          ORRERY_SELECT=MARS
shot REG_track_io       ORRERY_ZOOM=90000 ORRERY_SELECT=IO ORRERY_CAM=track
shot REG_jup_moons      ORRERY_SELECT=JUPITER ORRERY_CAM=track ORRERY_ZOOM=140000
shot REG_uranus_moons   ORRERY_SELECT=URANUS ORRERY_CAM=track ORRERY_ZOOM=900000
shot REG_free           ORRERY_CAM=free ORRERY_YAW=0.7 ORRERY_PITCH=0.5
shot REG_free_focus     ORRERY_CAM=free ORRERY_SELECT=SATURN ORRERY_YAW=0.9 ORRERY_PITCH=0.45 ORRERY_ZOOM=3000
shot REG_neo            ORRERY_SELECT=APOPHIS ORRERY_CAM=track ORRERY_ZOOM=900
shot REG_halley         ORRERY_SELECT=HALLEY ORRERY_ZOOM=26
shot REG_tour_mars      ORRERY_TOUR_STOP=4
# --- exo systems (FREE camera must rotate here too) ---------------------------
shot REG_exo_trappist   ORRERY_SYS=5 ORRERY_SELECT="TRAPPIST-1 e" ORRERY_CAM=track
shot REG_exo_free       ORRERY_SYS=5 ORRERY_CAM=free ORRERY_YAW=1.2 ORRERY_PITCH=0.6
# --- session-5: probes / sky sphere / density / fixes -------------------------
shot REG_probes         ORRERY_JD=2461322.5 ORRERY_ZOOM=3 ORRERY_SELECT="VOYAGER 1" ORRERY_SKY=0
shot REG_sky_sol        ORRERY_JD=2461322.5 ORRERY_DENSITY=1000
shot REG_sky_exo        ORRERY_JD=2461322.5 ORRERY_SYS=5
shot REG_tails          ORRERY_JD=2461322.5 ORRERY_SELECT=CHURYUMOV ORRERY_ZOOM=300 ORRERY_SKY=0
shot REG_free_snap      ORRERY_JD=2461322.5 ORRERY_CAM=free ORRERY_SELECT=SATURN ORRERY_ZOOM=3000 ORRERY_YAW=0.6 ORRERY_PITCH=0.35
shot REG_scope_1yr      ORRERY_JD=2461322.5 ORRERY_SELECT=MARS ORRERY_RATE_TAB=4 ORRERY_LSCROLL=430 ORRERY_SKY=0
# --- session-6: spectra panel / collapsible panels / lidar planets / BH disk --
shot REG_spectra_col    ORRERY_JD=2461322.5 ORRERY_SELECT=JUPITER
shot REG_gnomonic       ORRERY_JD=2461322.5 ORRERY_DENSITY=200
shot REG_collapsed      ORRERY_JD=2461322.5 ORRERY_COLLAPSE=1
shot REG_lidar_jup      ORRERY_JD=2461322.5 ORRERY_CAM=free ORRERY_SELECT=JUPITER ORRERY_ZOOM=140000 ORRERY_PITCH=0.6 ORRERY_YAW=0.3
shot REG_bh_disk        ORRERY_JD=2461322.5 ORRERY_CAM=free ORRERY_YAW=-3.0866 ORRERY_PITCH=1.55
# --- session-7: MIR fingerprint region (5th spectra graph) --------------------
shot S7_fingerprint_europa  ORRERY_JD=2461322.5 ORRERY_SELECT=EUROPA
shot S7_fingerprint_halley  ORRERY_JD=2461322.5 ORRERY_SELECT=HALLEY
shot S7_fingerprint_luna    ORRERY_JD=2461322.5 ORRERY_SELECT=LUNA
shot S7_fingerprint_mars    ORRERY_JD=2461322.5 ORRERY_SELECT=MARS
# --- session-8: FIND BODY search, bigger markers, rotation light curve ------
shot S8_search          ORRERY_JD=2461322.5 ORRERY_SEARCH=EU
shot S8_scene           ORRERY_JD=2461322.5
shot S8_vesta           ORRERY_JD=2461322.5 ORRERY_SELECT=VESTA
shot S8_halley          ORRERY_JD=2461322.5 ORRERY_SELECT=HALLEY
kill $XPID 2>/dev/null || true
echo "shots written to docs/shots/REG_*.png"
