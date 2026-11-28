#!/bin/bash
# gh:#1938 item 3 (README round 84): the 2D_fullCoupling steady stage (make_fullcoupling_coupled_case.sh, tight) for ONE step
# (`endTime 1`, `writeControl timeStep`, `writeInterval 1`) on the shipped fluid mesh (`base`) and on the two edits of
# make_fc_mesh_variant_r84.py (`swap`, `unused`); the real binary writes `0/fluidRegion.baffleLess/polyMesh` (removeBaffles at the start, zones
# `0()`, sets/duplicateFaces) and `1/fluidRegion.baffleLess/polyMesh` (a later write, with the zones).
# usage: run_fc_mesh_variants_r84.sh <scratch dir>
D=$1
S=$(dirname "$(readlink -f "$0")")
mkdir -p "$D"
for v in base swap unused; do
  bash "$S/make_fullcoupling_coupled_case.sh" "$D/$v"
  sed -i 's/^endTime .*/endTime 1;/' "$D/$v/system/controlDict"
  [ "$v" != base ] && python3 -I "$S/make_fc_mesh_variant_r84.py" "$D/$v" "$v"
  echo "$v: $(bash "$S/run_fullcoupling_coupled.sh" "$D/$v")"
done
