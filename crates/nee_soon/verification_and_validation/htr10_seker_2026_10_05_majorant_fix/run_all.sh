#!/bin/bash
# Re-measurement of the HTR-10 k-vs-height record on the bounded majorant (gh:#589).
# Sequential, one run at a time, 4 threads pinned to cores 0-3.
# Usage (from this directory): ./run_all.sh
# Order: VIII.0 N = 14, 10, 20, 17, then VII.0 N = 14. A run whose log already
# holds a k_eff line is skipped, so the script can be relaunched after a restart.
set -u
BIN=${BIN:-/home/user/outram-park-backend/target/release/examples/htr10_rmc_keff}
mkdir -p logs
for spec in e8:14 e8:10 e8:20 e8:17 e7:14; do
  lib=${spec%%:*}; n=${spec##*:}
  log=logs/run_${lib}_N${n}.log
  if [ -f "$log" ] && grep -q "k_eff        =" "$log"; then continue; fi
  e7=""; [ "$lib" = e7 ] && e7=1
  env OUTRAM_HTR10_HISTORIES=10000 OUTRAM_HTR10_INACTIVE=5 OUTRAM_HTR10_ACTIVE=20 \
      OUTRAM_HTR10_RINGS=14 OUTRAM_HTR10_LAYERS=$n OUTRAM_HTR10_THREADS=4 \
      ${e7:+OUTRAM_HTR10_ENDF7=1} \
      taskset -c 0-3 "$BIN" > "$log" 2>&1 &
  pid=$!
  peak=0
  while kill -0 $pid 2>/dev/null; do
    r=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null)
    [ -n "$r" ] && [ "$r" -gt "$peak" ] && peak=$r
    sleep 20
  done
  wait $pid
  echo "  peak RSS     = $peak kB (VmHWM, sampled every 20 s)" >> "$log"
  echo "DONE $lib N=$n $(date -u +%FT%TZ)"
done
