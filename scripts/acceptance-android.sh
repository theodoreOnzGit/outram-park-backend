#!/usr/bin/env bash
# The Android half of the acceptance check (GitHub #859): runs the staged
# `icsbep_five_route_keff` binary on a connected Android device or emulator
# through adb. Called by .github/workflows/acceptance-tests.yml after it has
# pushed `stage/` to /data/local/tmp/acceptance. Seeds per case and the run
# size (5000 x [40 + 120] for every case) are those of scripts/acceptance-keff.sh.
#
# All four cases run: the lattice's OpenMC cards are compiled into the binary
# (`include_str!` in examples/common/lct008_model.rs), so only the ENDF tapes
# have to be on the device.
set -uo pipefail

dir=/data/local/tmp/acceptance
commit="$(cat stage/commit.txt 2>/dev/null || echo unknown)"
status=0
run() {
  local c="$1" seeds="$2"
  adb shell "cd $dir && OUTRAM_PARK_ENDF_DIR=$dir/endf ./icsbep_five_route_keff \
    --case $c --route endf --label route4 --seeds $seeds --threads 4 \
    --particles 5000 --inactive 40 --active 120 \
    --csv per_seed_$c.csv --commit $commit \
    --accept summary_keff.csv --allowance-pcm 200 --accept-record keff_acceptance.csv" || status=1
}
run godiva 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16
run jemima 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24
run hst009 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16
run lct008 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16
adb shell "cat $dir/keff_acceptance.csv"
exit $status
