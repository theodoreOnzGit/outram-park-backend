#!/bin/bash
# Compare the port's upstream mode with the v2506 runs, every case x variant.
# usage: difftable.sh [dev|esi]
D=${POLYDUAL_WORK:?set POLYDUAL_WORK}
B=${CARGO_TARGET_DIR:-target}/release/examples
F=${1:-esi}
for c in cavity annulus warped lshape; do
  for v in base split concave; do
    case $v in base) fl="";; split) fl="splitAllFaces";; concave) fl="concaveMultiCells";; esac
    echo "=== $c $v (centres $F)"
    $B/compare_polymesh $D/$c/constant/polyMesh $D/runs/${c}__$v/0.005/polyMesh 60 $F $fl 2>&1 | grep -v "^  face\|^          upstream"
  done
done
