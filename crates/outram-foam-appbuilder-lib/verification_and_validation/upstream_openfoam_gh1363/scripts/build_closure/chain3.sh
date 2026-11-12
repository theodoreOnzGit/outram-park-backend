#!/bin/bash
# GeN-Foam additions (gh:#1361 step 4): everything else its thermal-hydraulics links
cd /home/user/upstream-build/gh1363/closure
for d in thermophysicalModels/solidSpecie thermophysicalModels/reactionThermo thermophysicalModels/solidThermo \
         thermophysicalModels/SLGThermo thermophysicalModels/radiation transportModels/twoPhaseMixture \
         transportModels/incompressible TurbulenceModels/incompressible dynamicFvMesh overset ODE \
         regionModels/regionModel fvOptions; do
  df -h / | tail -1 | awk '{print $4}' | grep -q "^[0-2]\.[0-9]G" && { echo "disk low before $d" >> timings.txt; exit 2; }
  ./build_lib.sh $d || { echo "FAILED $d" >> timings.txt; exit 1; }
done
echo CHAIN3_DONE >> timings.txt
