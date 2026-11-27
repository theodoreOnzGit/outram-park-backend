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
[ "$EIGEN" = false ] && sed -i 's/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/; s/^purgeWrite .*/purgeWrite      2;/' system/controlDict
[ "$EIGEN" = false ] && sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics        false;/' constant/neutroRegion/neutronicsProperties
S=$(date +%s)
GeN-Foam > log.GeN-Foam 2>&1
echo "run $NAME: GeN-Foam exit $? in $(( $(date +%s)-S )) s"
tail -3 log.GeN-Foam

# ---------------------------------------------------------------------------------------------------------------------
# DATA of the SP3 adjustment variants (round 42). Upstream SP3 with round 39's `v1` integralFlux numbers died with a floating
# point exception at step 56 (df of the first group driven to the keff clamp, as round 39's first choice did for diffusion):
# those numbers are zone integrals of the DIFFUSION flux. The SP3 variants `s1` / `s2` therefore use the same RECIPE on the
# SP3 `fluxStar_` (the field `adjustDiscFactors(fluxStar_)` integrates): integralFlux[zone, g] = 1e-6 * V * sum over the zone's cells of the converged `s0` fluxStar<g> (gh1887 run s0, t = 60) *
# factor, factor = 1.10/0.92 (reflector), 0.95/1.08 (coreLow), 1.07/0.94 (follower), 1.12/0.90 (rod), V = 7.92723e-05 m^3 (the
# cell volume: 0.32269 = 1e-6 * V * 0.95 * sum_coreLow flux0 of the v1 numbers), five decimals, the precision of v1:
#   reflector 0.11737 0.38996 | coreLow 0.31664 0.07819 | follower 0.25263 0.04437 | controlRod 0.01450 0.00058
# (a FIRST attempt with the converged physical `flux` (0.10371 0.39245 | 0.32288 0.07775 | 0.25529 0.04450 | 0.01077 0.00053) also drove upstream SP3 to the keff clamp, FPE at step 39: fluxStar differs from flux by the second moment, which is the quantity the adjustment sees)
# (the reflector value of v1 (0.04534 / 0.17206) is 2.29x smaller than this recipe gives on the same flux: it is not
# reproduced by the recipe, which is why the SP3 numbers are given explicitly here).
