#!/bin/bash
# gh:#1882 (round 48): build upstream_driver/pkRunLF1882 against GeN-Foam 652b3da's libNeutronicsSolvers
# (ESI v2506) and run it with `liquidFuel true` on the neutroRegion mesh of
# Tutorials/reactorCases/1D_MSR_pointKinetics (640 cells, cyclic top/bottom), prescribed U/alpha/alphat/mu.
# Upstream is not modified. Usage: run_pk_lf_1882.sh [variant ...]   (default: all)
# Output: refs/pk1882_<variant>.ref.txt.gz (driver lines plus the real class' own Info lines).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
export GENFOAM_SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}
WORK=${WORK:-/home/user/upstream-build/gh1882}
SRC=$GENFOAM_SRC/Tutorials/reactorCases/1D_MSR_pointKinetics/rootCase
OF=/home/user/upstream-build/scripts/with-of.sh
mkdir -p "$WORK"
if [ ! -x /root/OpenFOAM/user-v2506/platforms/linux64GccDPInt32Opt/bin/pkRunLF1882 ] || [ -n "$REBUILD" ]; then
  rm -rf "$WORK/pkRunLF1882"; cp -r "$HERE/upstream_driver/pkRunLF1882" "$WORK/pkRunLF1882"
  (cd "$WORK/pkRunLF1882" && $OF wmake > "$WORK/build.log" 2>&1) || { tail -30 "$WORK/build.log"; exit 1; }
fi

hdr() { printf 'FoamFile { version 2.0; format ascii; class dictionary; object %s; }\n' "$1"; }

BETA="(2.114580e-04 4.231390e-04 3.737000e-04 5.789020e-04 8.929690e-04 1.426090e-04 1.926010e-04 3.738980e-05)"
LAMBDA="(1.246670e-02 2.829170e-02 4.252440e-02 1.330420e-01 2.924670e-01 6.664880e-01 1.634780e+00 3.099610e+00)"

# variant -> FLOW ALPHA INIT SCNO MATRIX GEOM
make_case() {
  local v=$1 c=$WORK/$1
  FLOW=flow; ALPHA=const; INIT=true; SCNO=1; MATRIX=1; GEOM=0
  case $v in
    flow) GEOM=1 ;;
    rev) FLOW=rev ;;
    still) FLOW=still ;;
    slow) FLOW=slow ;;
    alpha_assign) ALPHA=assign ;;
    alpha_direct) ALPHA=direct; MATRIX=0 ;;
    scno) SCNO=0.7; MATRIX=0 ;;
    noinit) INIT=false; MATRIX=0 ;;
    *) echo "unknown variant $v"; exit 1 ;;
  esac
  rm -rf "$c"; mkdir -p "$c/constant/neutroRegion" "$c/system/neutroRegion" "$c/0/uniform" "$c/0/neutroRegion"
  cp -r "$SRC/constant/neutroRegion/polyMesh" "$c/constant/neutroRegion/"
  cp "$SRC"/system/neutroRegion/fv* "$c/system/neutroRegion/"
  { hdr controlDict; cat <<'EOF'
application GeN-Foam;
startFrom startTime; startTime 0; stopAt endTime; endTime 1000; deltaT 0.001;
writeControl none; writeInterval 1000; writeFormat ascii; writePrecision 17;
timeFormat general; timePrecision 8; runTimeModifiable false;
liquidFuel true;
EOF
  } > "$c/system/controlDict"
  { hdr nuclearData; cat <<EOF
fastNeutrons true;
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
absoluteDrivelineExpansionCoeff 0;
initPrecursorsLiquidFuel $INIT;
ScNo $SCNO;
initialOneGroupFluxByZone { "core" 1.0; "HotLeg" 0.7; "pump" 0.3; "hx" 0.1; "ColdLeg" 0.4; }
fuelFeedbackZones ("core" "HotLeg");
coolFeedbackZones ("core" "HotLeg" "hx");
structMechFeedbackZones ("hx");
EOF
  } > "$c/constant/neutroRegion/nuclearData"
  { hdr neutronicsProperties; echo "eigenvalueNeutronics false;"; } > "$c/constant/neutroRegion/neutronicsProperties"
  { hdr reactorState; echo "keff 1; pTarget 1.0e9;"; } > "$c/0/uniform/reactorState"
}

VARIANTS=${*:-flow rev still slow alpha_assign alpha_direct scno noinit}
mkdir -p "$HERE/refs"
for v in $VARIANTS; do
  make_case $v
  (cd "$WORK" && if [ $GEOM = 1 ]; then export PK_GEOM=1; else unset PK_GEOM; fi; PK_FLOW=$FLOW PK_ALPHA=$ALPHA PK_MATRIX=$MATRIX $OF pkRunLF1882 -case $v > "$WORK/$v.log" 2>&1) || { tail -20 "$WORK/$v.log"; echo "FAILED $v"; continue; }
  grep -E '^(CFG|VOL|INITN|ZONE|G|GP|PHI|PHIB|DIFF|M|MD|MU|ML|MS|MI|MB|K|P|PS|F|STEP) |Neutron iteration|neutroResidual|Intermediate Power|Solving for precPKStar|^Beta\[|^liquidFuelBeta|^precEquilibriumReactivity' "$WORK/$v.log" | gzip -9n > "$HERE/refs/pk1882_$v.ref.txt.gz"
  echo "$v: $(grep -c '^K ' "$WORK/$v.log") steps, $(stat -c %s "$HERE/refs/pk1882_$v.ref.txt.gz") bytes"
done
