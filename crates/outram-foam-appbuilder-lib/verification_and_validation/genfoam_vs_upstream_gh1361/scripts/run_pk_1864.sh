#!/bin/bash
# gh:#1864 (round 36): build upstream_driver/pkRun1864 against GeN-Foam 652b3da's
# libNeutronicsSolvers (ESI v2506) and run it on one case variant per option, on the
# neutroRegion mesh of Tutorials/reactorCases/1D_MSR_pointKinetics (640 cells, five cell
# zones). Upstream is not modified. Usage: run_pk_1864.sh [variant ...]   (default: all)
# Output: refs/pk1864_<variant>.ref.txt.gz (only the CFG/PROF/T/K/F lines).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
export GENFOAM_SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}
WORK=${WORK:-/home/user/upstream-build/gh1864}
SRC=$GENFOAM_SRC/Tutorials/reactorCases/1D_MSR_pointKinetics/rootCase
OF=/home/user/upstream-build/scripts/with-of.sh
mkdir -p "$WORK"
if [ ! -x /root/OpenFOAM/user-v2506/platforms/linux64GccDPInt32Opt/bin/pkRun1864 ] || [ -n "$REBUILD" ]; then
  rm -rf "$WORK/pkRun1864"; cp -r "$HERE/upstream_driver/pkRun1864" "$WORK/pkRun1864"
  (cd "$WORK/pkRun1864" && $OF wmake > "$WORK/build.log" 2>&1) || { tail -30 "$WORK/build.log"; exit 1; }
fi

hdr() { printf 'FoamFile { version 2.0; format ascii; class dictionary; object %s; }\n' "$1"; }

BETA="(2.114580e-04 4.231390e-04 3.737000e-04 5.789020e-04 8.929690e-04 1.426090e-04 1.926010e-04 3.738980e-05)"
LAMBDA="(1.246670e-02 2.829170e-02 4.252440e-02 1.330420e-01 2.924670e-01 6.664880e-01 1.634780e+00 3.099610e+00)"

make_case() { # $1 variant
  local v=$1 c=$WORK/$1
  rm -rf "$c"; mkdir -p "$c/constant/neutroRegion" "$c/system/neutroRegion" "$c/0/uniform" "$c/0/neutroRegion"
  cp -r "$SRC/constant/neutroRegion/polyMesh" "$c/constant/neutroRegion/"
  cp "$SRC"/system/neutroRegion/fv* "$c/system/neutroRegion/"
  { hdr controlDict; cat <<'EOF'
application GeN-Foam;
startFrom startTime; startTime 0; stopAt endTime; endTime 1000; deltaT 0.001;
writeControl none; writeInterval 1000; writeFormat ascii; writePrecision 17;
timeFormat general; timePrecision 8; runTimeModifiable false;
liquidFuel false;
EOF
  } > "$c/system/controlDict"
  local fast=true ND="" RS="" NP="eigenvalueNeutronics false;" ES="" RODS=0 FBZ=1
  case $v in
    feedback) ;;
    thermal) fast=false ;;
    rods_a) RODS=1 ;;
    rods_d) RODS=2 ;;
    profiles) ND='externalReactivityTimeProfile { type table; startTime 0.05; table table ((0 0) (1 1e-4) (5 -2e-4) (30 3e-4)); }
boronReactivityTimeProfile { type table; table table ((0 0) (2 -1.5e-4) (60 1e-4)); }' ;;
    decay) RS='decayPowerTimeProfile { type table; startTime 0.02; table table ((0 6e7) (3 5e7) (10 3e7) (100 1e7)); }' ;;
    src_transient) NP='eigenvalueNeutronics false; externalSourceNeutronics true;'
      ND='nuFission 2.45; energyPerFission 3.2e-11;'
      RS='subcriticalIndex 0.02;'
      ES='externalSourceMode transient; nuSource 30; beamEnergy 1.0e9;
externalSourceModulationTimeProfile { type table; table table ((0 1) (1 1.5) (3 0.5) (30 2)); }' ;;
    src_monitor) NP='eigenvalueNeutronics false; externalSourceNeutronics true;'
      ND='nuFission 2.45; energyPerFission 3.2e-11;'
      RS='ksrc 0.97;'
      ES='externalSourceMode powerMonitoring; nuSource 30; beamEnergy 1.0e9; modulationFactor 2.0; powerModulationDelay 0.05;' ;;
    restart_both) NP='eigenvalueNeutronics false; externalSourceNeutronics true;'
      ND='nuFission 2.45; energyPerFission 3.2e-11;'
      RS='ksrc 0.97; subcriticalIndex 0.05; precursorPowers (4.1e7 8.5e7 7.0e7 4.6e7 2.5e7 3.1e6 1.2e6 1.0e5);'
      ES='externalSourceMode transient; nuSource 30; beamEnergy 1.0e9; externalSourceModulationTimeProfile { type table; table table ((0 1) (30 1.2)); }' ;;
    combined) RODS=1; NP='eigenvalueNeutronics false; externalSourceNeutronics true;'
      ND='nuFission 2.45; energyPerFission 3.2e-11;
externalReactivityTimeProfile { type table; table table ((0 0) (1 1e-4) (30 -1e-4)); }'
      RS='subcriticalIndex 0.01; decayPowerTimeProfile { type table; table table ((0 5e7) (100 1e7)); }'
      ES='externalSourceMode powerMonitoring; nuSource 30; beamEnergy 1.0e9; modulationFactor 1.0; powerModulationDelay 0.1;' ;;
    *) echo "unknown variant $v"; exit 1 ;;
  esac
  local MAP="" DRV=""
  [ $RODS = 1 ] && MAP='controlRodReactivityMap ((-0.01 1e-3) (0 0) (0.01 -1e-3));'
  [ $RODS = 2 ] && MAP='controlRodReactivityMap ((0.01 -1e-3) (0 0) (-0.01 1e-3));'
  [ $RODS != 0 ] && DRV='drivelineFeedbackZones ("pump" "hx");'
  { hdr nuclearData; cat <<EOF
fastNeutrons $fast;
energyGroups 6;
promptGenerationTime 1.26e-6;
Beta $BETA;
lambda $LAMBDA;
feedbackCoeffDoppler 4.0e-3;
feedbackCoeffTFuel -3.54253e-05;
feedbackCoeffTClad -1.0e-05;
feedbackCoeffTCool -2.0e-05;
feedbackCoeffRhoCool 1.5e-07;
feedbackCoeffTStruct -4.0e-05;
feedbackCoeffTStructMech 3.0e-05;
absoluteDrivelineExpansionCoeff $([ $RODS != 0 ] && echo 1.0e-3 || echo 0);
$MAP
$DRV
initialOneGroupFluxByZone { "core" 1.0; "HotLeg" 0.7; "pump" 0.3; "hx" 0.1; "ColdLeg" 0.4; }
fuelFeedbackZones ("core" "HotLeg");
coolantFeedbackZones ("core" "HotLeg" "hx");
structMechFeedbackZones ("hx");
$ND
EOF
  } > "$c/constant/neutroRegion/nuclearData"
  { hdr neutronicsProperties; echo "$NP"; } > "$c/constant/neutroRegion/neutronicsProperties"
  [ -n "$ES" ] && { hdr externalSource; echo "$ES"; } > "$c/constant/neutroRegion/externalSource"
  { hdr reactorState; echo "keff 1; pTarget 1.0e9;"; echo "$RS"; } > "$c/0/uniform/reactorState"
}

VARIANTS=${*:-feedback thermal rods_a rods_d profiles decay src_transient src_monitor restart_both combined}
mkdir -p "$HERE/refs"
for v in $VARIANTS; do
  make_case $v
  (cd "$WORK" && $OF pkRun1864 -case $v > "$WORK/$v.log" 2>&1) || { tail -20 "$WORK/$v.log"; echo "FAILED $v"; continue; }
  grep -E '^(CFG|PROF|T|K|F) ' "$WORK/$v.log" | gzip -9n > "$HERE/refs/pk1864_$v.ref.txt.gz"
  echo "$v: $(grep -c '^K ' "$WORK/$v.log") steps, $(stat -c %s "$HERE/refs/pk1864_$v.ref.txt.gz") bytes"
done
