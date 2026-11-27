#!/bin/bash
# gh:#1879 / gh:#1361 round 37: for each variant of cases/msfr_th_variants_r37, run the REAL unmodified GeN-Foam for 3 steps with
# run_msfr_th_tight.sh (TURB=off, VARIANT overlay), extract the fields (extract_radial_fields.sh) and the solve log into
# refs/msfr_th_r37_<variant>.ref.txt.gz, and DELETE the run directory (disk is tight).
# usage: run_msfr_th_radial_variants.sh <variant>...      (default: all)
HERE=$(cd "$(dirname "$0")/.." && pwd)
export VARIANT_DIR=$HERE/cases/msfr_th_variants_r37
RUNS=${RUNS:-/home/user/upstream-build/gh1812/runs}
VARS=${*:-$(ls "$VARIANT_DIR")}
COMMON="T.fuelAvForNeutronics T.cladAvForNeutronics heatFlux.structure"
zone_fields() {
  case $1 in
    hp) echo "T To.heatedPin Ti.heatedPin Tav.heatedPin $COMMON";;
    nfp*) echo "T Tfi.nuclearFuelPin Tfo.nuclearFuelPin Tci.nuclearFuelPin Tco.nuclearFuelPin Tfav.nuclearFuelPin Tcav.nuclearFuelPin $COMMON";;
    lns*) echo "T Tmax.lumpedNuclearStructure Tmatrix.lumpedNuclearStructure Tsurface.lumpedNuclearStructure $COMMON";;
  esac
}
prof_fields() {
  case $1 in
    hp) echo "Trad.heatedPin";;
    nfp*) echo "Trad.nuclearFuelPin";;
    lns*) echo "T.lumpedNuclearStructure";;
  esac
}
for v in $VARS; do
  END_TIME=0.0037 TURB=off VARIANT=$v bash "$HERE/scripts/run_msfr_th_tight.sh" "r37_$v" "$RUNS" || { echo "FAILED $v"; continue; }
  D=$RUNS/r37_$v
  T=$(ls "$D" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | tr '\n' ' ')
  echo "$v times: $T"
  bash "$HERE/scripts/extract_radial_fields.sh" "$D" /tmp/r37_fields.txt "$T" "U p_rgh T" "$(zone_fields $v)" "$(prof_fields $v)"
  bash "$HERE/scripts/extract_msfr_th_log.sh" "$D/log.GeN-Foam" /tmp/r37_log.txt
  cat /tmp/r37_fields.txt /tmp/r37_log.txt | gzip -9 > "$HERE/refs/msfr_th_r37_$v.ref.txt.gz"
  ls -la "$HERE/refs/msfr_th_r37_$v.ref.txt.gz"
  rm -rf "$D"
done
