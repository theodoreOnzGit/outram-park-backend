#!/bin/sh
# usage: lct.sh FIRST_SEED NSEEDS
S=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest
T=/tmp/claude-0/-home-user-outram-park-backend/208a80c6-8981-41e3-ac0f-1769a3a0e997/scratchpad/target-njoy
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf RAYON_NUM_THREADS=2 OUTRAM_PITCH_SEEDS=$2
ulimit -v 12582912
exec nice -n 10 $T/release/examples/lct008_pitch_sweep --cheap-nuclides --pitches 1.63576,1.75 --particles 5000 --inactive 50 --active 200 --seed $1 --threads 2 --csv $S/lct/repro_s$1.csv > $S/lct/repro_s$1.log 2>&1
