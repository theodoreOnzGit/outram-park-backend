#!/bin/bash
# Run OpenFOAM.com v2506 polyDualMesh on each primal case, per option variant.
# Output: $R/runs/<case>__<variant>/0.005/polyMesh  (+ log.polyDualMesh)
. /home/user/upstream-build/scripts/of-env.sh
R=${POLYDUAL_WORK:?set POLYDUAL_WORK to a scratch directory}
rm -rf $R/runs; mkdir -p $R/runs
run() { # case variant args...
  c=$1; v=$2; shift 2
  d=$R/runs/${c}__$v
  cp -r $R/$c $d
  cd $d
  /usr/bin/time -f "%e s %M KB" polyDualMesh "$@" > log.polyDualMesh 2> log.time
  rc=$?; echo "$c $v: exit $rc $(tail -1 log.time) ; $(ls -d [0-9]* 2>/dev/null | tr '\n' ' ')"
}
for c in cavity annulus warped lshape; do
  run $c base 60
  run $c split 60 -splitAllFaces
  run $c concave 60 -concaveMultiCells
done
