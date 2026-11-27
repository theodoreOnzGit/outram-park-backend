#!/bin/bash
# Round 32b (gh:#1863): the diffusion external source with a time profile, real GeN-Foam binary
# (652b3da on ESI v2506), serial, tight protocol (writePrecision 17, linear solvers 1e-14 / relTol 0).
#   F  closed medium (walls zeroGradient, round 32's B), ddtSchemes Euler, constant/neutroRegion/externalSource
#      = cases/external_source_profile/externalSource_F (inline `table table (...)`, startTime 5)
#   G  shipped walls (fixedValue 0, round 32's A), ddtSchemes Euler, externalSource_G (`values (...)` form)
# Usage: scripts/with-of.sh run_external_source_profile.sh <F|G>  (GENFOAM_UPSTREAM selects the source tree).
# Work dir (scratch, deleted after extraction): /home/user/upstream-build/gh1863b/<variant>.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
V=$1
HERE="$(cd "$(dirname "$0")/.." && pwd)"
T=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/featureCases/2D_externalSourceDiffusion/rootCase
W=/home/user/upstream-build/gh1863b/$V
rm -rf "$W"; mkdir -p "$W"; cp -r "$T/." "$W/"; cd "$W" || exit 1
if [ "$V" = F ]; then
  for f in defaultFlux defaultFlux2 defaultExternalSourceFlux; do
    perl -0pi -e 's/(walls\s*\{\s*)type\s+fixedValue;\s*value\s+uniform 0;/$1type zeroGradient;/' 0/neutroRegion/$f
  done
fi
sed -i 's/^\(\s*default\s*\)steadyState;.*/\1Euler;/' system/neutroRegion/fvSchemes
cp "$HERE/cases/external_source_profile/externalSource_$V" constant/neutroRegion/externalSource
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
for f in system/*/fvSolution; do
  sed -i 's/^\( *\)tolerance .*/\1tolerance        1e-14;/; s/^\( *\)relTol .*/\1relTol           0;/' "$f"
done
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "variant $V: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 log.GeN-Foam
