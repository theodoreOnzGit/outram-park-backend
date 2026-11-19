#!/bin/sh
# usage: fhr.sh default|knob "seeds"
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf RAYON_NUM_THREADS=2
ulimit -v 12582912
[ $1 = knob ] && export OUTRAM_SAB_LOG_SPACED_GRID=1
for s in $2; do
  OUTRAM_FHR_ALL_SEED=$s /tmp/claude-0/-home-user-outram-park-backend/208a80c6-8981-41e3-ac0f-1769a3a0e997/scratchpad/target-njoy/release/examples/fhr_ring_rpt_endf > /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/fhr/$1_s$s.out 2> /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/fhr/$1_s$s.err
done
echo done > /tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/fhr/$1.done
