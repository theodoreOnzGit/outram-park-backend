#!/bin/bash
# one process at a time; old then new per seed
L=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/l1635
cd /home/user/outram-park-backend/crates/nee_soon
ulimit -v 12582912
export OUTRAM_PARK_ENDF_DIR=/home/user/outram-park-backend/reference-data/endf OUTRAM_HTR10_HISTORIES=1000 OUTRAM_HTR10_INACTIVE=10 OUTRAM_HTR10_ACTIVE=30
for s in $(seq ${1:-20260917} ${2:-20260922}); do
  for b in old new; do
    f=$L/${b}_seed$s.out
    grep -q "k_eff\|k =" $f 2>/dev/null && continue
    OUTRAM_HTR10_SEED=$s $L/keff_$b > $f 2> $L/${b}_seed$s.err
  done
done
