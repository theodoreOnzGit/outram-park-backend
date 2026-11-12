#!/bin/bash
# Protocol edit for the step-3 comparison, applied by run_tutorial.sh (PRE_RUN) inside the
# copied tutorial. It changes how precisely the run is written and solved, never the
# physics, mesh, cross sections or boundary conditions:
#   writePrecision 17        fields and the log carry full doubles
#   linear solvers 1e-14/0   (tolerance 1e-14, relTol 0) in every region's fvSolution
#   endTime $END_TIME        long enough for the pseudo-time eigenvalue iteration to settle
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
sed -i "s/^endTime .*/endTime         ${END_TIME:-60};/" system/controlDict
for f in system/*/fvSolution; do
  sed -i 's/^\( *\)tolerance .*/\1tolerance        1e-14;/; s/^\( *\)relTol .*/\1relTol           0;/' "$f"
done
