#!/bin/bash
# gh:#1927 round 63: stage one variant of the point-kinetics options (README round 63) from the ADJ=1 steady state.
# usage: make_pk_variant_r63.sh <steady-state case dir> <out case dir> <variant>
# variants: see make_pk_variant_r63.py; `base` = round 61's PK stage unchanged (endTime 200)
SS=$1
C=$2
V=$3
D=$(dirname "$0")
bash "$D/make_fullcoupling_transient_case.sh" "$SS" "$C" > /dev/null || exit 1
if [ "$V" != base ]; then
  python3 -I "$D/make_pk_variant_r63.py" "$V" "$C" || exit 1
fi
if [ "$V" = gem ]; then
  # a plane of faces of the FLUID mesh named `gemCut` (transient stage only; the steady state has no such zone)
  cat > "$C/system/fluidRegion/topoSetDict" <<'EOT'
FoamFile { version 2.0; format ascii; class dictionary; object topoSetDict; }
actions
(
    { name gemCut; type faceSet; action new; source boxToFace; box (-10 -10 GEMZ0) (10 10 GEMZ1); }
    { name gemCut; type faceZoneSet; action new; source setToFaceZone; faceSet gemCut; }
);
EOT
  # the flow is along z (25 distinct levels): the faces of the 12th level
  YS=$(grep -E '^\([-0-9.e+]+ [-0-9.e+]+ [-0-9.e+]+\)$' "$C/constant/fluidRegion/polyMesh/points" | tr -d '()' | awk '{print $3}' | sort -g | uniq | sed -n '12p')
  Y0=$(echo $YS | awk '{printf "%.12g", $1-1e-9}')
  Y1=$(echo $YS | awk '{printf "%.12g", $1+1e-9}')
  sed -i "s/GEMZ0/$Y0/; s/GEMZ1/$Y1/" "$C/system/fluidRegion/topoSetDict"
  /home/user/upstream-build/scripts/with-of.sh topoSet -case "$C" -region fluidRegion > "$C/log.topoSet" 2>&1 || exit 1
fi
echo staged "$V"
