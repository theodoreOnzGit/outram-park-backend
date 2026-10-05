#!/bin/bash
# Control (gh:#589): VIII.0 N = 14 on TODAY's code with the OLD (pre-#589,
# log-grid-only) majorant, at the same 10 000 x [5 + 20]. Separates the
# majorant's effect from every other code change since the 2026-10-01 record.
set -u
BIN=${BIN:-/home/user/outram-park-backend/target/release/examples/htr10_rmc_keff}
log=logs/control_e8_N14_old_majorant.log
if [ -f "$log" ] && grep -q "k_eff        =" "$log"; then exit 0; fi
env OUTRAM_HTR10_HISTORIES=10000 OUTRAM_HTR10_INACTIVE=5 OUTRAM_HTR10_ACTIVE=20 \
    OUTRAM_HTR10_RINGS=14 OUTRAM_HTR10_LAYERS=14 OUTRAM_HTR10_THREADS=4 \
    OUTRAM_HTR10_MAJORANT_WITHOUT_BREAKPOINTS=1 \
    taskset -c 0-3 "$BIN" > "$log" 2>&1 &
pid=$!; peak=0
while kill -0 $pid 2>/dev/null; do
  r=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null)
  [ -n "$r" ] && [ "$r" -gt "$peak" ] && peak=$r
  sleep 20
done
wait $pid
echo "  peak RSS     = $peak kB (VmHWM, sampled every 20 s)" >> "$log"
echo "DONE control e8 N=14 $(date -u +%FT%TZ)"
