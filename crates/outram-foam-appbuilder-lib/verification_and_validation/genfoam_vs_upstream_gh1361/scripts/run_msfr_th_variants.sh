#!/bin/bash
# gh:#1842 / gh:#1361 round 29: for each variant of cases/msfr_th_variants, run the REAL unmodified GeN-Foam for 3 steps with
# run_msfr_th_tight.sh (VARIANT overlay), extract U k epsilon nut p_rgh at steps 1 and 3 plus the k/epsilon/T log lines into one small
# gzipped reference refs/msfr_th_opt_<variant>.ref.txt.gz, and DELETE the run directory (disk is tight).
# usage: run_msfr_th_variants.sh <variant>...      (default: all)
HERE=$(cd "$(dirname "$0")/.." && pwd)
export VARIANT_DIR=$HERE/cases/msfr_th_variants
RUNS=${RUNS:-/home/user/upstream-build/gh1812/runs}
VARS=${*:-$(ls "$VARIANT_DIR")}
for v in $VARS; do
  END_TIME=0.0037 TURB=on VARIANT=$v bash "$HERE/scripts/run_msfr_th_tight.sh" "opt_$v" "$RUNS" || { echo "FAILED $v"; continue; }
  D=$RUNS/opt_$v
  T=$(ls "$D" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | tr '\n' ' ')
  echo "$v times: $T"
  set -- $T
  T1=$1; T3=$3
  bash "$HERE/scripts/extract_msfr_th_fields.sh" "$D" /tmp/opt_fields.txt "$T1 $T3" "U k epsilon nut"
  bash "$HERE/scripts/extract_msfr_th_log.sh" "$D/log.GeN-Foam" /tmp/opt_log.txt
  cat /tmp/opt_fields.txt /tmp/opt_log.txt | gzip -9 > "$HERE/refs/msfr_th_opt_$v.ref.txt.gz"
  ls -la "$HERE/refs/msfr_th_opt_$v.ref.txt.gz"
  rm -rf "$D"
done
