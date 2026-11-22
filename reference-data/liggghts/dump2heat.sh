#!/bin/sh
# LIGGGHTS `dump custom id ... f_heat f_heatFlux` (the last two columns) -> CSV
# `step,id,T,heatFlux`, columns copied as printed (%.17g from the precision
# patch). Optional third argument: keep only these steps (space separated).
#   ./dump2heat.sh <dump> <out.csv> ["0 1 10 100"]
set -eu
src=$1; dst=$2; keep=${3:-}
awk -v keep="$keep" '
  BEGIN { n=split(keep, k, " "); for (i=1;i<=n;i++) want[k[i]]=1; print "step,id,T,heatFlux" }
  /^ITEM: TIMESTEP/ { mode="step"; next }
  /^ITEM: ATOMS/    { mode="atom"; next }
  /^ITEM:/          { mode=""; next }
  mode=="step" { step=$1; mode=""; next }
  mode=="atom" { if (n==0 || (step in want)) print step","$1","$(NF-1)","$NF }
' "$src" > "$dst"
