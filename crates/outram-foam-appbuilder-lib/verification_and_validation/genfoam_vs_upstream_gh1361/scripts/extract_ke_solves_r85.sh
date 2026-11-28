#!/bin/bash
# gh:#1984 round 85: the k/epsilon solve lines of a onePhase GeN-Foam log, one line per outer iteration:
#   KE <step> <outer> <epsilon initial residual> <epsilon iterations> <k initial residual> <k iterations>
# usage: extract_ke_solves_r85.sh <log.GeN-Foam> >> <prefix>_log.ref.txt
awk '
/^Time = / { step++ }
/^PIMPLE: iteration / { outer = $3 }
/Solving for epsilon,/ { split($0, a, "Initial residual = "); split(a[2], b, ","); ei = b[1]; en = $NF }
/Solving for k,/ { split($0, a, "Initial residual = "); split(a[2], b, ","); print "KE", step, outer, ei, en, b[1], $NF }
' "$1"
