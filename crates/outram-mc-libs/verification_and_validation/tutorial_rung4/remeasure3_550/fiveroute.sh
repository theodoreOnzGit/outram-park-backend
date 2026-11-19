#!/bin/sh
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf RAYON_NUM_THREADS=1
ulimit -v 12582912
D=/tmp/claude-0/-home-user-outram-park-backend/208a80c6-8981-41e3-ac0f-1769a3a0e997/scratchpad/target-njoy/release/examples/icsbep_five_route_keff
O=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/fiveroute
for r in 3 5; do
  [ $r = 3 ] && lib=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/fr/njoy || lib=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/remeasure3_rest/fr/rust
  for arm in off on; do
    if [ $arm = on ]; then export OUTRAM_SAB_LOG_SPACED_GRID=1; else unset OUTRAM_SAB_LOG_SPACED_GRID; fi
    nice -n 19 $D --case hst009 --route ace --ace-dir $lib --label route${r}_$arm --seeds 1,2,3 --csv $O/r${r}_$arm.csv --threads 1 --particles 5000 --inactive 40 --active 120 --commit c025f20079 > $O/r${r}_$arm.out 2> $O/r${r}_$arm.err
  done
done
for arm in off on; do
  if [ $arm = on ]; then export OUTRAM_SAB_LOG_SPACED_GRID=1; else unset OUTRAM_SAB_LOG_SPACED_GRID; fi
  nice -n 19 $D --case hst009 --route endf --label route4_$arm --seeds 1 --csv $O/r4_$arm.csv --threads 1 --particles 5000 --inactive 40 --active 120 > $O/r4_$arm.out 2> $O/r4_$arm.err
done
echo FIVE_DONE > $O/done
