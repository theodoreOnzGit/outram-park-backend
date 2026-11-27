#!/bin/bash
# gh:#1887 (README round 42): run the real GeN-Foam binary on the round-39 df_slab case (guided slab 2 re-zoned, synthetic
# two-group data, scripts/make_df_driveline_cases.sh) with the round-42 variations, tight protocol (writePrecision 17,
# solvers 1e-14 / relTol 0).
#   make_r42_cases.sh <name> <nuclearData variant v0|v1|v2|v3> [key=value ...]
#     solver=SP3Neutronics     region solver (default diffusionNeutronics)
#     eigen=false              eigenvalueNeutronics false (the stationary-fuel transient step)
#     end=<t>                  endTime (default 60 for eigenvalue runs)
#     fvedit='<sed expr>'      applied to system/neutroRegion/fvSolution AFTER the tight protocol (relTol / minIter variants)
#     crmove=d1|d3             CRmove variant
#     dt=<t>                   deltaT
# Work dir (scratch, deleted after extraction): /home/user/upstream-build/gh1887/runs/<name>.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$HERE/cases/df_slab"
NAME=$1; ND=$2; shift 2
SOLVER=; EIGEN=true; END=; FVEDIT=; CR=none; DT=
for kv in "$@"; do
  case "$kv" in
    solver=*) SOLVER=${kv#solver=} ;;
    eigen=*) EIGEN=${kv#eigen=} ;;
    end=*) END=${kv#end=} ;;
    fvedit=*) FVEDIT=${kv#fvedit=} ;;
    crmove=*) CR=${kv#crmove=} ;;
    dt=*) DT=${kv#dt=} ;;
  esac
done
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
W=/home/user/upstream-build/gh1887/runs/$NAME
rm -rf "$W"; mkdir -p "$W"
cp -r /home/user/upstream-build/GeN-Foam-src/Tutorials/guidedCases/2_reactorSlabReflected_1D_1Gr_neutronicDiffusion/. "$W/"
cd "$W" || exit 1
cp -r 0.orig 0
cp "$OUT/nuclearData.${ND}" constant/neutroRegion/nuclearData
zcat "$OUT/cellZones.gz" > constant/neutroRegion/polyMesh/cellZones
for f in points faces owner neighbour boundary; do
  zcat "$HERE/cases/guided2/polyMesh/$f.gz" > constant/neutroRegion/polyMesh/$f
done
[ "$CR" != none ] && cp "$OUT/CRmove.$CR" constant/neutroRegion/CRmove
[ -n "$END" ] && export END_TIME=$END
[ -n "$SOLVER" ] && export SOLVER
bash "$HERE/scripts/tight_protocol.sh"
[ -n "$FVEDIT" ] && sed -i "$FVEDIT" system/neutroRegion/fvSolution
[ -n "$DT" ] && sed -i "s/^deltaT .*/deltaT          $DT;/" system/controlDict
[ "$EIGEN" = false ] && sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics        false;/' constant/neutroRegion/neutronicsProperties
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "run $NAME: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 log.GeN-Foam
