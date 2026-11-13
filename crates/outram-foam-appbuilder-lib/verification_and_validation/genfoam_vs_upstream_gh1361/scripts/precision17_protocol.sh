#!/bin/bash
# Protocol edit (run_tutorial.sh PRE_RUN hook) for gh:#1361 round 3, PSBT / gFHR / onePhase cases:
# only the write precision changes, never the physics, mesh, solvers or tolerances.
#   writePrecision 17        fields and the log carry full doubles
#   END_TIME=<t>             optional shorter run
#   EVERY_STEP=1             write every step (writeControl timeStep; writeInterval 1)
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
[ -n "$END_TIME" ] && sed -i "s/^endTime .*/endTime         $END_TIME;/" system/controlDict
if [ -n "$EVERY_STEP" ]; then
  sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/' system/controlDict
fi
exit 0
