#!/bin/bash
# gh:#1887 (README round 42): external-source case B (round 32, walls zeroGradient) with the flux solver entry `relTol 0.1`
# (tolerance 1e-14 kept), real GeN-Foam binary, tight protocol otherwise. Work dir /home/user/upstream-build/gh1887/<variant>.
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
T=/home/user/upstream-build/GeN-Foam-src/Tutorials/featureCases/2D_externalSourceDiffusion/rootCase
V=${1:-BR}   # BR: flux relTol 0.1 on the tight protocol; BS: the SHIPPED fvSolution untouched (tolerance 1e-6, relTol 1e-3)
W=/home/user/upstream-build/gh1887/$V
rm -rf "$W"; mkdir -p "$W"; cp -r "$T/." "$W/"; cd "$W" || exit 1
for f in defaultFlux defaultFlux2 defaultExternalSourceFlux; do
  perl -0pi -e 's/(walls\s*\{\s*)type\s+fixedValue;\s*value\s+uniform 0;/$1type zeroGradient;/' 0/neutroRegion/$f
done
sed -i 's/^writePrecision .*/writePrecision  17;/' system/controlDict
[ "$V" = BR ] && for f in system/*/fvSolution; do
  sed -i 's/^\( *\)tolerance .*/\1tolerance        1e-14;/; s/^\( *\)relTol .*/\1relTol           0;/' "$f"
done
[ "$V" = BR ] && sed -i '/"flux.\*|adjoint_flux.\*"/,/}/ s/relTol .*/relTol 0.1;/' system/neutroRegion/fvSolution
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "$V: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
