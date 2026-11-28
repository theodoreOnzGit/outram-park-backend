#!/bin/bash
# gh:#1876 (README round 89): the real GeN-Foam binary on 3D_gFHR.
# usage: run_gfhr_r89_real.sh <scratch run dir> <variant>
#   nssp   the shipped Allrun for steadyStateNSSP, unedited: clone rootCase, `GeN-Foam -initializeMappedFields`, `GeN-Foam`
#          (stopAt nextWrite: to the first adjustableRunTime write, t = 50). The md5 of every file is taken before and after
#          the -initializeMappedFields stage (md5_before.txt, md5_after.txt in the run dir).
#   lps    the shipped Allrun for steadyStateLPS: clone rootCase, `cp phaseProperties_lumpedParam phaseProperties`, `GeN-Foam`
#   tN     the tight protocol on the NSSP case: every solver tolerance 1e-14, relTol 0, writePrecision 17,
#          stopAt endTime, endTime N x 0.1 (N steps of the shipped first deltaT if Co stays < 1), writeControl timeStep,
#          writeInterval 1, purgeWrite 1 (writeControl timeStep drops the adjustableRunTime alignment: README round 88)
#   sN     the SHIPPED settings with only stopAt endTime, endTime N, writeControl timeStep, writeInterval 1, purgeWrite 1,
#          writePrecision 17 (whole-case prefix with every shipped solver setting)
# The run directory is left in place for the reference extraction; delete it afterwards.
D=$1; V=$2
ROOT=/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_gFHR/rootCase
W=/home/user/upstream-build/scripts/with-of.sh
export LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH
rm -rf "$D"; cp -r "$ROOT" "$D"
cd "$D" || exit 1
C=system/controlDict
F=system/fluidRegion/fvSolution
case "$V" in
  nssp)
    find . -type f | sort | xargs md5sum > md5_before.txt
    /usr/bin/time -v $W GeN-Foam -initializeMappedFields -case "$D" > log.init 2> time.init
    echo "init exit $?"
    find . -type f | grep -v "log.init\|time.init\|md5_" | sort | xargs md5sum > md5_after.txt
    ;;
  lps)
    cp constant/fluidRegion/phaseProperties_lumpedParam constant/fluidRegion/phaseProperties
    ;;
  t*|s*)
    N=${V:1}
    if [ "${V:0:1}" = t ]; then
      END=$(python3 -c "print(round($N*0.1, 10))")
      sed -i "s/tolerance  *[0-9.e+-]*;/tolerance       1e-14;/; s/relTol  *[0-9.e+-]*;/relTol          0;/" "$F"
      sed -i "/^ *relTol .*/a\        maxIter         5000;" "$F"
    else
      END=$N
    fi
    sed -i "s|^stopAt .*|stopAt          endTime;\nendTime         $END;|" "$C"
    sed -i "s/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/; s/^purgeWrite .*/purgeWrite      1;/; s/^writePrecision .*/writePrecision  17;/" "$C"
    ;;
  *) echo "unknown variant $V"; exit 1 ;;
esac
/usr/bin/time -v $W GeN-Foam -case "$D" > log.GeN-Foam 2> time.txt
echo "run exit $?"
tail -3 log.GeN-Foam
