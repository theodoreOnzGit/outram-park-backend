#!/bin/bash
# run_stages.sh <src run dir> <app> <short name> <step...>
# Runs <app> stopped after each step in /home/user/upstream-build/runs/stage_<short>_<step>.
src=$1; app=$2; short=$3; shift 3
for step in "$@"; do
  R=/home/user/upstream-build/runs/stage_${short}_$step
  rm -rf "$R"; mkdir -p "$R"
  cp -r "$src"/. "$R"/
  rm -rf "$R"/constant/polyMesh "$R"/log.* "$R"/dist.txt
  printf '\nworkflowControls\n{\n    stopAfter %s;\n}\n' "$step" >> "$R"/system/meshDict
  ( cd "$R" && OMP_NUM_THREADS=1 /home/user/upstream-build/scripts/with-of.sh $app > log.$app 2>&1; /home/user/upstream-build/scripts/with-of.sh checkMesh > log.checkMesh 2>&1 )
  echo "$step: $(tail -2 $R/log.$app | tr '\n' ' ') polyMesh: $(ls $R/constant/polyMesh 2>/dev/null | tr '\n' ' ')"
done
