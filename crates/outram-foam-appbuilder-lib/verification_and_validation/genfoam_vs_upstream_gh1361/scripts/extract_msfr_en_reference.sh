#!/bin/bash
# Round 11 (gh:#1814): refs/msfr_en.ref.txt.gz from a `2D_MSFR` steadyStateEN run of the real GeN-Foam binary
# (run_msfr_chain.sh steadyStateEN, PROTOCOL=tight, writePrecision 17).
# usage: extract_msfr_en_reference.sh <run root> [endTimeEN = 165] [output name = msfr_en]
# Output (one item per number, space separated):
#   N <steps>
#   TIME <the `Time = ` of each step>        KEFF <`keff_ = ` after each step>      KEFFPREV <`keff_ previous`>
#   POWER <`power:` of each step>             NRES <`neutroResidual (neutroIter: 1)` of each step>
#   HRES_n   <5 initial residuals of the `h` solves of step n>   TSTAT_n <avg min max after each of the 5 outer iterations>
#   END_flux<g>, END_precStar<k>, END_powerDensity, END_T   the fields at endTimeEN (internalField)
#   END_KEFF, END_PTARGET   from <end>/uniform/reactorState
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUTNAME=${3:-msfr_en}   # gh:#1814 round 14: third argument names the output (the light chain writes msfr_en20)
R=$1/steadyStateEN
E=${2:-165}
L=$R/log.GeN-Foam
OUT=$(mktemp)
fld() { awk '/^\($/{f=1;next} /^\)$/{exit} f{gsub(/[()]/,""); print $1}' "$1" | tr '\n' ' '; }
echo "N $(grep -c '^Time = ' $L)" > "$OUT"
echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "KEFF $(grep '^keff_ = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
echo "KEFFPREV $(grep '^keff_ previous = ' $L | awk '{print $4}' | tr '\n' ' ')" >> "$OUT"
echo "POWER $(grep '^power: ' $L | awk '{print $2}' | tr '\n' ' ')" >> "$OUT"
echo "NRES $(grep '^neutroResidual (neutroIter: 1)' $L | awk '{print $4}' | tr '\n' ' ')" >> "$OUT"
awk '
  /^Time = /{s++; kh=0; k=0}
  /Solving for h,/{kh++; split($0,a,"Initial residual = "); split(a[2],b,","); hres[s,kh]=b[1]}
  /^T \(avg min max\)/{k++; ts[s,k]=$6" "$7" "$8}
  END{for(i=1;i<=s;i++){
        printf "HRES_%d", i; for(j=1;j<=5;j++) printf " %s", hres[i,j]; printf "\n";
        printf "TSTAT_%d", i; for(j=1;j<=5;j++) printf " %s", ts[i,j]; printf "\n"; } }' "$L" >> "$OUT"
for g in 0 1 2 3 4 5; do echo "END_flux$g $(fld $R/$E/neutroRegion/flux$g)" >> "$OUT"; done
for k in 0 1 2 3 4 5 6 7; do echo "END_precStar$k $(fld $R/$E/neutroRegion/precStar$k)" >> "$OUT"; done
echo "END_powerDensity $(fld $R/$E/neutroRegion/powerDensity)" >> "$OUT"
echo "END_T $(fld $R/$E/fluidRegion/T)" >> "$OUT"
echo "END_KEFF $(grep '^keff' $R/$E/uniform/reactorState | awk '{gsub(/;/,"");print $2}')" >> "$OUT"
echo "END_PTARGET $(grep '^pTarget' $R/$E/uniform/reactorState | awk '{gsub(/;/,"");print $2}')" >> "$OUT"
gzip -9 -n -c "$OUT" > "$HERE/refs/${OUTNAME}.ref.txt.gz"
rm -f "$OUT"
ls -l "$HERE/refs/${OUTNAME}.ref.txt.gz"
