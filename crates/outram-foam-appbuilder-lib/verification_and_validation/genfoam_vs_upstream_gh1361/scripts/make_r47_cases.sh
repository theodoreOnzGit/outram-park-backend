#!/bin/bash
# gh:#1887 / gh:#1898 (README round 47): run the real GeN-Foam binary on the df_slab case (guided slab 2 re-zoned) with an
# arbitrary nuclearData file and the round-47 variations, tight protocol (writePrecision 17, solvers 1e-14 / relTol 0).
#   make_r47_cases.sh <name> <nuclearData file> [key=value ...]
#     solver=SP3Neutronics   region solver (default diffusionNeutronics)
#     flux2=<v>              internalField of defaultFlux2 (SP3 second moment; the shipped tutorial value is 1)
#     flux=<v>               internalField of defaultFlux (shipped 1)
#     eigen=false end=<t> dt=<t> crmove=<file>   as make_r42_cases.sh
# Work dir (scratch, deleted after extraction): /home/user/upstream-build/gh1887/runs/<name>.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
NAME=$1; ND=$2; shift 2
SOLVER=; EIGEN=true; END=; CR=none; DT=; FLUX2=; FLUX=
for kv in "$@"; do
  case "$kv" in
    solver=*) SOLVER=${kv#solver=} ;;
    eigen=*) EIGEN=${kv#eigen=} ;;
    end=*) END=${kv#end=} ;;
    crmove=*) CR=${kv#crmove=} ;;
    dt=*) DT=${kv#dt=} ;;
    flux2=*) FLUX2=${kv#flux2=} ;;
    flux=*) FLUX=${kv#flux=} ;;
  esac
done
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
W=/home/user/upstream-build/gh1887/runs/$NAME
rm -rf "$W"; mkdir -p "$W"
cp -r /home/user/upstream-build/GeN-Foam-src/Tutorials/guidedCases/2_reactorSlabReflected_1D_1Gr_neutronicDiffusion/. "$W/"
cd "$W" || exit 1
cp -r 0.orig 0
cp "$ND" constant/neutroRegion/nuclearData
zcat "$HERE/cases/df_slab/cellZones.gz" > constant/neutroRegion/polyMesh/cellZones
for f in points faces owner neighbour boundary; do
  zcat "$HERE/cases/guided2/polyMesh/$f.gz" > constant/neutroRegion/polyMesh/$f
done
[ "$CR" != none ] && cp "$CR" constant/neutroRegion/CRmove
[ -n "$END" ] && export END_TIME=$END
[ -n "$SOLVER" ] && export SOLVER
bash "$HERE/scripts/tight_protocol.sh"
[ -n "$DT" ] && sed -i "s/^deltaT .*/deltaT          $DT;/" system/controlDict
[ -n "$FLUX2" ] && sed -i "s/^internalField .*/internalField   uniform $FLUX2;/" 0/neutroRegion/defaultFlux2
[ -n "$FLUX" ] && sed -i "s/^internalField .*/internalField   uniform $FLUX;/" 0/neutroRegion/defaultFlux
[ "$EIGEN" = false ] && sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/; s/^purgeWrite .*/purgeWrite      2;/' system/controlDict
[ "$EIGEN" = false ] && sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics        false;/' constant/neutroRegion/neutronicsProperties
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "run $NAME: GeN-Foam exit $? in $(( $(date +%s)-S )) s, $(grep -c '^Time = ' log.GeN-Foam) steps"
