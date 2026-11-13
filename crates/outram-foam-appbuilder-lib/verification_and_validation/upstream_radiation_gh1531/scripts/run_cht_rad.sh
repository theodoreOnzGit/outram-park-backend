#!/bin/bash
# gh:#1531. The gh:#1363 two-region case with P1 radiation ON in the fluid, run by
# driver/chtRadDump.C on ESI v2506 (needs libchtBCs from
# ../../upstream_openfoam_gh1363/scripts/build_cht_bcs.sh and libradiationModels).
# Writes the committed gzipped reference reference/<tag>_v2506.txt.gz:
# S/C lines of the gh:#1363 driver plus G, qr_*, Gb_* per step, plus
# "I <step> <PCG iterations of the G solve>" lines parsed from the log.
#   usage: run_cht_rad.sh [absorptivity] [nSteps] [solverFreq] [tag]
#   env: NOREF=1 leaves the committed reference untouched
set -e
A=${1:-50}
NSTEPS=${2:-50}
FREQ=${3:-1}
TAG=${4:-cht_rad}
HERE=$(cd "$(dirname "$0")/.." && pwd)
SRC13=$HERE/../upstream_openfoam_gh1363
W=/home/user/upstream-build/gh1531
WITH=/home/user/upstream-build/scripts/with-of.sh
C=$W/case_$TAG
mkdir -p $W/bin $W/src
rm -rf $W/src/chtRadDump; mkdir -p $W/src/chtRadDump/Make
cp $HERE/driver/chtRadDump.C $W/src/chtRadDump/
cp $HERE/driver/Make/options $W/src/chtRadDump/Make/options
printf 'chtRadDump.C\n\nEXE = %s/bin/chtRadDump\n' $W > $W/src/chtRadDump/Make/files
(cd $W/src/chtRadDump && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/src/chtRadDump/Make/linux*

rm -rf $C; mkdir -p $C
cp -r $SRC13/cht/constant $SRC13/cht/system $SRC13/cht/0 $C/
# radiation on in the fluid, off in the solid (as the gh:#1363 case)
sed "s/@A@/$A/;s/@E@/$A/;s/@EE@/0/;s/solverFreq 1/solverFreq $FREQ/" \
  $HERE/../../../outram-foam-basic-lib/verification_and_validation/upstream_radiation_gh1379/inputs/radiationProperties.tmpl \
  > $C/constant/fluid/radiationProperties
cp $HERE/inputs/boundaryRadiationProperties $C/constant/fluid/boundaryRadiationProperties
cp $HERE/inputs/G $C/0/fluid/G
# the G solver and its Laplacian scheme (the port's P1 is the uncorrected one)
sed -i '/^solvers/,/^}/ s|^}|    \"G.*\" { solver PCG; preconditioner DIC; tolerance 1e-15; relTol 0; maxIter 1000; }\n}|' $C/system/fluid/fvSolution
sed -i 's|laplacianSchemes { default Gauss linear corrected; }|laplacianSchemes { default Gauss linear corrected; laplacian(gammaRad,G) Gauss linear uncorrected; }|' $C/system/fluid/fvSchemes
sed -i "s|^endTime .*|endTime $(awk -v d=0.1 -v n=$NSTEPS 'BEGIN{printf "%.17g", d*(n-0.25)}');|" $C/system/controlDict
(cd $C && $WITH $W/bin/chtRadDump > $W/$TAG.log 2>&1) || { tail -30 $W/$TAG.log; exit 1; }
{ grep -E "^S |^C " $W/$TAG.log
  awk '/^Time = /{step++} /Solving for G/{ n=$0; sub(/.*No Iterations /,"",n); print "I " step " " n }' $W/$TAG.log
} > $W/ref_$TAG.txt
[ -n "$NOREF" ] || { mkdir -p $HERE/reference; gzip -9n -c $W/ref_$TAG.txt > $HERE/reference/${TAG}_v2506.txt.gz; }
tail -3 $W/$TAG.log
