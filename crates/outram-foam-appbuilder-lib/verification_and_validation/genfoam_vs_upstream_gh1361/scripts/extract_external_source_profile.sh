#!/bin/bash
# Round 32b (gh:#1863): refs/external_source_profile.ref.txt.gz from the runs of run_external_source_profile.sh
# (/home/user/upstream-build/gh1863b/<variant>). Per variant (same format as extract_external_source.sh):
#   CASE <v> ; TIME / DT (the `deltaT =` printed after each step) / POWER / NITER / RESID (neutroResidual of
#   the last iteration of each step) : one value per step ; FLUX0_100 / FLUX1_100 : per cell at t = 100 ;
#   PTARGET : `pTarget` of 100/uniform/reactorState
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT=/home/user/upstream-build/gh1863b/external_source_profile.ref.txt
: > "$OUT"
cell() { awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' "$1" | tr '\n' ' '; }
for v in F G; do
  R=/home/user/upstream-build/gh1863b/$v
  L=$R/log.GeN-Foam
  echo "CASE $v" >> "$OUT"
  echo "TIME $(grep '^Time = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
  echo "DT $(grep '^deltaT = ' $L | awk '{print $3}' | tr '\n' ' ')" >> "$OUT"
  echo "POWER $(grep '^power: ' $L | awk '{print $2}' | tr '\n' ' ')" >> "$OUT"
  echo "NITER $(awk '/^Time = /{if(n)print n; n=0} /^Neutron iteration no:/{n++} END{print n}' $L | tr '\n' ' ')" >> "$OUT"
  echo "RESID $(awk '/^Time = /{if(r!="")print r; r=""} /^neutroResidual \(neutroIter/{r=$4} END{print r}' $L | tr '\n' ' ')" >> "$OUT"
  echo "FLUX0_100 $(cell $R/100/neutroRegion/flux0)" >> "$OUT"
  echo "FLUX1_100 $(cell $R/100/neutroRegion/flux1)" >> "$OUT"
  echo "PTARGET $(awk '/^pTarget/{gsub(";","");print $2}' $R/100/uniform/reactorState)" >> "$OUT"
done
mkdir -p "$HERE/refs"
gzip -9 -n -c "$OUT" > "$HERE/refs/external_source_profile.ref.txt.gz"
ls -l "$HERE/refs/external_source_profile.ref.txt.gz"
