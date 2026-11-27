#!/bin/bash
# gh:#1785 / #1474 (README round 39): write the SYNTHETIC two-group slab cases of the discontinuity-factor and
# control-rod-driveline comparisons.
#   make_df_driveline_cases.sh data   -> (re)writes cases/df_slab/{nuclearData.<v>,CRmove.<d>} from the tables below
#   make_df_driveline_cases.sh zones  -> (re)writes cases/df_slab/cellZones.gz (reflector, coreLow, follower, controlRod)
#   make_df_driveline_cases.sh run <variant> <crmove|none>
#        -> copies guided tutorial 2 to /home/user/upstream-build/gh1785/runs/<variant>, installs the case, applies the tight
#           protocol (writePrecision 17, solvers 1e-14/0, endTime 60) and runs the real GeN-Foam binary.
# The cross sections are NOT physical data: they are inputs of both codes (two groups, one fast one thermal, a
# fuel, a reflector and an absorbing "rod" zone), chosen only so that every zone and group carries weight.
# integralFlux (round 39, second choice): the zone integrals of the converged v0 flux of the real binary (run
# 2026-11-27) times 1.10/0.92 (reflector), 0.95/1.08 (coreLow), 1.07/0.94 (follower), 1.12/0.90 (rod), group 0/1,
# scaled by 1e-6. The FIRST choice (0.5 0.33 / 0.25 0.35 / 0.2 0.3 / 0.05 0.02, arbitrary numbers) made upstream
# itself diverge (v1: df down to 4.7e-15, keff clamped at 5 from step 4 on), a state in which a step-by-step
# comparison means nothing; it was replaced before any port run.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$HERE/cases/df_slab"

zone() { # name sigmaR0 nuSf0 sigmaR1 nuSf1 D0 D1 s01 s11 dfAdjust df0 df1 if0 if1
cat <<EOF
            $1
            {
                fuelFraction    1;
                dfAdjust        ${10};
                D               nonuniform List<scalar> 2 ($6 $7);
                nuSigmaEff      nonuniform List<scalar> 2 ($3 $5);
                sigmaPow        nonuniform List<scalar> 2 ($(awk "BEGIN{printf \"%.10g\", $3/2.5}") $(awk "BEGIN{printf \"%.10g\", $5/2.5}"));
                sigmaRemoval    nonuniform List<scalar> 2 ($2 $4);
                scatteringMatrixP0 2 2 ( ( 0.5 $8 ) ( 0 $9 ) );
                chiPrompt       nonuniform List<scalar> 2 (1 0);
                chiDelayed      nonuniform List<scalar> 2 (1 0);
                IV              nonuniform List<scalar> 2 (7.1e-08 4.5e-04);
                discFactor      nonuniform List<scalar> 2 (${11} ${12});
                integralFlux    nonuniform List<scalar> 2 (${13} ${14});
                lambda          nonuniform List<scalar> 6 (0.0125371 0.0300828 0.109879 0.325484 1.3036 9.51817);
                Beta            nonuniform List<scalar> 6 (7.2315e-05 0.000609661 0.000471181 0.00118907 0.000445487 9.58515e-05);
            }
EOF
}

nuclear() { # variant adjust groupsWoDF reflDfAdjust coreDf0 coreDf1 rodDf0 rodDf1 reflDf0 reflDf1
cat <<EOF
FoamFile { version 2.0; format ascii; class dictionary; object nuclearData; }
// gh:#1785 synthetic two-group slab, variant $1
fastNeutrons    true;
adjustDiscFactors $2;
$3
useGivenDiscFactors false;
energyGroups    2;
precGroups      6;
xsVariables {}
states
(
    reference
    {
        zones
        (
$(zone reflector 2.04 0 0.2 0 0.012 0.0030 2.0 0.1 $4 ${9} ${10} 0.04534 0.17206)
$(zone coreLow 3.0 0.6 10.0 18.0 0.013 0.0040 2.0 0.4 true $5 $6 0.32269 0.07787)
$(zone follower 3.0 0.6 10.0 18.0 0.013 0.0040 2.0 0.4 true $5 $6 0.25485 0.04440)
$(zone controlRod 3.5 0.4 40.0 2.0 0.011 0.0035 2.0 0.4 true $7 $8 0.01113 0.00055)
        );
    }
);
EOF
}

crmove() { # startTime endTime speed d0
cat <<EOF
FoamFile { version 2.0; format ascii; class dictionary; object CRmove; }
zones
(
    controlRod
    {
        startTime   $1;
        endTime     $2;
        speed       $3;
        initialDistanceFromMeshCR $4;
        followerName follower;
    }
);
EOF
}

case "$1" in
data)
  mkdir -p "$OUT"
  #            variant adjust groupsWoDF           reflDfAdjust coreDf0 coreDf1 rodDf0 rodDf1 reflDf0 reflDf1
  nuclear v0 false ""                    true  1 1 1 1 1 1 > "$OUT/nuclearData.v0"
  nuclear v1 true  ""                    true  1 1 1 1 1 1 > "$OUT/nuclearData.v1"
  nuclear v2 true  "groupsWoDF (1);"     false 1 1 1 1 1 1 > "$OUT/nuclearData.v2"
  nuclear v3 false ""                    true  1.05 0.96 0.9 1.2 0.98 1.04 > "$OUT/nuclearData.v3"
  crmove 10 30 0.005 0     > "$OUT/CRmove.d1"
  crmove 0 20 -0.004 0.05  > "$OUT/CRmove.d3"
  # gh:#1893 (round 43): the rod moves 0.035 m within the 3.5e-4 s of the stationary-fuel runs (d1 and d3 move 1e-6 m there)
  crmove 0 1 100 0         > "$OUT/CRmove.d4"
  ;;
zones)
  mkdir -p "$OUT"
  {
    echo 'FoamFile { version 2.0; format ascii; class regIOobject; location "constant/neutroRegion/polyMesh"; object cellZones; }'
    echo '4'
    echo '('
    z() { echo "$1"; echo '{'; echo '    type cellZone;'; echo "    cellLabels List<label> $2"; echo '('; shift 2; for c in "$@"; do echo "$c"; done; echo ')'; echo ';'; echo '}'; }
    z reflector 300 $(seq 0 149) $(seq 301 450)
    z coreLow 50 $(seq 150 199)
    z follower 50 $(seq 200 249)
    z controlRod 51 $(seq 250 300)
    echo ')'
  } | gzip -9 -n > "$OUT/cellZones.gz"
  ;;
run)
  V=$2; CR=$3
  source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
  W=/home/user/upstream-build/gh1785/runs/$V
  rm -rf "$W"; mkdir -p "$W"
  cp -r /home/user/upstream-build/GeN-Foam-src/Tutorials/guidedCases/2_reactorSlabReflected_1D_1Gr_neutronicDiffusion/. "$W/"
  cd "$W" || exit 1
  cp -r 0.orig 0
  ND=${V%%_*}   # variant name before an underscore selects the nuclearData
  cp "$OUT/nuclearData.${ND}" constant/neutroRegion/nuclearData
  zcat "$OUT/cellZones.gz" > constant/neutroRegion/polyMesh/cellZones
  # The SAME polyMesh files the port reads (cases/guided2, 17-digit points). The tutorial's own points are written with
  # 10 decimals and differ from them by up to 3.3e-10 m: the first round-39 run used them, and the cell-centre z of the
  # follower (the rod position) then differed by 2.3e-10 relative and every flux by 1e-7 (a harness defect, not a port one).
  for f in points faces owner neighbour boundary; do
    zcat "$HERE/cases/guided2/polyMesh/$f.gz" > constant/neutroRegion/polyMesh/$f
  done
  [ "$CR" != none ] && cp "$OUT/CRmove.$CR" constant/neutroRegion/CRmove
  PRE=$HERE/scripts/tight_protocol.sh
  bash "$PRE"
  # defaultFlux2 is for SP3 only; the two-group case needs one defaultFlux (read for every group)
  GeN-Foam > log.GeN-Foam 2>&1
  echo "GeN-Foam exit $?"; tail -3 log.GeN-Foam
  ;;
esac
