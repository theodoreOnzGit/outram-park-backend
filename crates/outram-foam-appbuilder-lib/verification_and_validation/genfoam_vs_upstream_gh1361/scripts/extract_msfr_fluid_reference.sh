#!/bin/bash
# Round 9 (issue 1361): refs/msfr_fluid_transient.ref.txt.gz from a COUPLED `2D_MSFR` transient run of the
# real GeN-Foam binary (run_msfr_chain.sh transient, PROTOCOL=tight, every step written, writePrecision 17).
# usage: extract_msfr_fluid_reference.sh <run root> [N steps = 50]
# Output lines (full doubles; one item per number, space separated):
#   N <steps>                         DT <the `deltaT = ` printed after each step (the step size of step n+1)>
#   START_T, START_powerDensity       the t = 165 s fluid T and the neutroRegion powerDensity (what step 1 maps)
#   HRES_n   <5 initial residuals of the `h` solve, one per PIMPLE outer iteration of step n>
#   TSTAT_n  <avg min max of `T (avg min max)` after each of the 5 outer iterations>
#   CO_n     <Courant number (avg max) printed after step n>
#   POWER_n  <`power:` of step n>
#   T_n      fluidRegion/T internal field after step n (8595 values)
#   RE_n     fluidRegion/Re  after step n, for n in 1 10 25 50
HERE="$(cd "$(dirname "$0")/.." && pwd)"
R=$1/transient
N=${2:-50}
L=$R/log.GeN-Foam
OUT=$(mktemp)
fld() { awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $1}' "$1" | tr '\n' ' '; }
echo "N $N" > "$OUT"
echo "DT $(grep '^deltaT = ' $L | head -n $N | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "START_T $(fld $R/165/fluidRegion/T)" >> "$OUT"
echo "START_powerDensity $(fld $R/165/neutroRegion/powerDensity)" >> "$OUT"
awk -v N="$N" '
  /^Time = /{s++; if (s>N) exit; kh=0; k=0}
  /Solving for h,/{kh++; split($0,a,"Initial residual = "); split(a[2],b,","); hres[s,kh]=b[1]}
  /^T \(avg min max\)/{k++; ts[s,k]=$6" "$7" "$8}
  /^Courant Number/{co[s]=$6" "$7}
  /^power: /{pw[s]=$2}
  END{for(i=1;i<=s&&i<=N;i++){
        printf "HRES_%d", i; for(j=1;j<=5;j++) printf " %s", hres[i,j]; printf "\n";
        printf "TSTAT_%d", i; for(j=1;j<=5;j++) printf " %s", ts[i,j]; printf "\n";
        printf "CO_%d %s\n", i, co[i]; printf "POWER_%d %s\n", i, pw[i]; } }' "$L" >> "$OUT"
ALLT=$(ls $R | grep -E '^[0-9.e+-]+$' | sort -g | grep -v '^0$' | grep -v '^165$')
i=0
for T in $ALLT; do
  i=$((i+1)); [ $i -gt $N ] && break
  echo "T_$i $(fld $R/$T/fluidRegion/T)" >> "$OUT"
  case $i in 1|10|25|50) echo "RE_$i $(fld $R/$T/fluidRegion/Re)" >> "$OUT";; esac
done
gzip -9 -n -c "$OUT" > "$HERE/refs/msfr_fluid_transient.ref.txt.gz"
rm -f "$OUT"
ls -l "$HERE/refs/msfr_fluid_transient.ref.txt.gz"
