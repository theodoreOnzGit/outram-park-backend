#!/bin/bash
# gh:#1855 / gh:#1361 round 31: for each variant of cases/msfr_th_variants_r31, run the REAL unmodified GeN-Foam for 3 steps with
# run_msfr_th_tight.sh (VARIANT overlay), extract U k epsilon p_rgh T of the three written steps (every 11th cell) plus the solve log
# (and the PIMPLE lines) into refs/msfr_th_r31_<variant>.ref.txt.gz, and DELETE the run directory (disk is tight).
# usage: run_msfr_th_pimple_variants.sh <variant>...      (default: all)
HERE=$(cd "$(dirname "$0")/.." && pwd)
export VARIANT_DIR=$HERE/cases/msfr_th_variants_r31
RUNS=${RUNS:-/home/user/upstream-build/gh1812/runs}
VARS=${*:-$(ls "$VARIANT_DIR")}
for v in $VARS; do
  END_TIME=0.0037 TURB=on VARIANT=$v bash "$HERE/scripts/run_msfr_th_tight.sh" "r31_$v" "$RUNS" || { echo "FAILED $v"; continue; }
  D=$RUNS/r31_$v
  T=$(ls "$D" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | tr '\n' ' ')
  echo "$v times: $T"
  set -- $T
  bash "$HERE/scripts/extract_msfr_th_fields_sub.sh" "$D" /tmp/r31_fields.txt "$1 $2 $3" "U k epsilon p_rgh T" 11
  bash "$HERE/scripts/extract_msfr_th_log.sh" "$D/log.GeN-Foam" /tmp/r31_log.txt
  grep -E "^PIMPLE: (converged|not converged)|^Time = " "$D/log.GeN-Foam" | sed 's/^/LOG /' > /tmp/r31_pimple.txt
  cat /tmp/r31_fields.txt /tmp/r31_log.txt /tmp/r31_pimple.txt | gzip -9 > "$HERE/refs/msfr_th_r31_$v.ref.txt.gz"
  ls -la "$HERE/refs/msfr_th_r31_$v.ref.txt.gz"
  rm -rf "$D"
done
