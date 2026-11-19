#!/bin/sh
# usage: run.sh FIRST_SEED TAG   (gh:#550 arm E: OpenMC .h5 neutron data, ENDF-built S(a,b) on THERMR's grid)
S=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf RAYON_NUM_THREADS=2 OUTRAM_PITCH_SEEDS=4
ulimit -v 12582912
exec /tmp/claude-0/-home-user-outram-park-backend/208a80c6-8981-41e3-ac0f-1769a3a0e997/scratchpad/target-njoy/release/examples/lct008_pitch_sweep --hdf5-dir $S/openmc-libs/lct008_pin11/h5_njoy --sab-node-grid --cheap-nuclides --pitches 2.40,1.63576,1.75 --particles 5000 --inactive 50 --active 200 --seed $1 --csv $S/e550/E_$2.csv > $S/e550/E_$2.log 2>&1
