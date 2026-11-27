#!/bin/bash
# Round 32 (gh:#1863): the shipped `featureCases/2D_externalSourceDiffusion` in variants, real GeN-Foam
# binary (652b3da on ESI v2506), serial, tight protocol (writePrecision 17, linear solvers 1e-14 / relTol 0).
#   A   rootCase as shipped (walls fixedValue 0)
#   B   Alltest's form: walls zeroGradient on defaultFlux, defaultFlux2, defaultExternalSourceFlux
#   C   B with `ddtSchemes default Euler`
#   BO  B with `externalSourceNeutronics false` (source off, run in upstream too)
# Usage: run_external_source.sh <A|B|C|BO>. Work dir (scratch, deleted after extraction):
# /home/user/upstream-build/gh1863/<variant>.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
V=$1
T=/home/user/upstream-build/GeN-Foam-src/Tutorials/featureCases/2D_externalSourceDiffusion/rootCase
W=/home/user/upstream-build/gh1863/$V
rm -rf "$W"; mkdir -p "$W"; cp -r "$T/." "$W/"; cd "$W" || exit 1
if [ "$V" != A ]; then
  for f in defaultFlux defaultFlux2 defaultExternalSourceFlux; do
    perl -0pi -e 's/(walls\s*\{\s*)type\s+fixedValue;\s*value\s+uniform 0;/$1type zeroGradient;/' 0/neutroRegion/$f
  done
fi
[ "$V" = C ] && sed -i 's/^\(\s*default\s*\)steadyState;.*/\1Euler;/' system/neutroRegion/fvSchemes
[ "$V" = BO ] && sed -i 's/^externalSourceNeutronics .*/externalSourceNeutronics false;/' constant/neutroRegion/neutronicsProperties
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
for f in system/*/fvSolution; do
  sed -i 's/^\( *\)tolerance .*/\1tolerance        1e-14;/; s/^\( *\)relTol .*/\1relTol           0;/' "$f"
done
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "variant $V: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 log.GeN-Foam
