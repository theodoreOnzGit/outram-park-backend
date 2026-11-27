#!/bin/bash
# gh:#1919 / gh:#1923 round 60: run the real GeN-Foam binary (652b3da on ESI v2506, unmodified; thermo-mechanics through the controlDict `libs` entry)
# on a case made by make_fullcoupling_transient_case.sh, as the tutorial's Allrun does (`-initializeMappedFields`, then the run).
# usage: run_fullcoupling_transient.sh <case dir>        (prints the wall time; log.init and log.GeN-Foam in the case)
C=$1
export LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH
cd "$C" || exit 1
/home/user/upstream-build/scripts/with-of.sh GeN-Foam -initializeMappedFields -case "$C" > log.init 2>&1
S=$(date +%s)
/home/user/upstream-build/scripts/with-of.sh GeN-Foam -case "$C" > log.GeN-Foam 2>&1
echo "exit $? in $(( $(date +%s)-S )) s"
