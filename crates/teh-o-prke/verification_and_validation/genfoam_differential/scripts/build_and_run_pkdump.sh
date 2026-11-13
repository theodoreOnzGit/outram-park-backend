#!/bin/bash
# gh:#1439: build the pkDump driver against GeN-Foam 652b3da's libNeutronicsSolvers
# (already built under /root/OpenFOAM/user-v2506, ESI v2506) and write
# refs/pkdump.ref.txt. Upstream sources are not modified. The case is the
# neutroRegion of Tutorials/reactorCases/2D_FFTF with its GEMReactivityMap removed
# (GEM needs the fluid mesh), a minimal controlDict (liquidFuel false) and a
# reactorState with pTarget 1e6 and no decay-power profile.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
WORK=/home/user/upstream-build/gh1439
R=$GENFOAM_SRC/Tutorials/reactorCases/2D_FFTF/rootCase
rm -rf "$WORK/case" "$WORK/pkDump"
mkdir -p "$WORK/case/constant" "$WORK/case/system" "$WORK/case/0/uniform" "$WORK/case/0/neutroRegion"
cp -r "$R/constant/neutroRegion" "$WORK/case/constant/"
cp -r "$R/system/neutroRegion" "$WORK/case/system/"
cp "$R"/0/neutroRegion/defaultFlux* "$WORK/case/0/neutroRegion/"
python3 -I - <<'EOF'
p = '/home/user/upstream-build/gh1439/case/constant/neutroRegion/nuclearData'
s = open(p).read()
i = s.index('GEMReactivityMap\n(')
j = s.index('\n);', i) + 3
open(p, 'w').write(s[:i] + s[j:])
EOF
cat > "$WORK/case/system/controlDict" <<'EOF'
FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }
application GeN-Foam;
startFrom startTime;
startTime 0;
stopAt endTime;
endTime 1000;
deltaT 0.001;
writeControl none;
writeInterval 1000;
writeFormat ascii;
writePrecision 17;
timeFormat general;
timePrecision 8;
runTimeModifiable false;
fluidRegionName fluidRegion;
eigenvalueNeutronics false;
liquidFuel false;
EOF
cat > "$WORK/case/0/uniform/reactorState" <<'EOF'
FoamFile { version 2.0; format ascii; class dictionary; object reactorState; }
keff 1;
pTarget 1.0e6;
EOF
cp -r "$HERE/upstream_driver/pkDump" "$WORK/pkDump"
cd "$WORK/pkDump" || exit 1
/home/user/upstream-build/scripts/with-of.sh wmake > "$WORK/build.log" 2>&1
cd "$WORK" || exit 1
/home/user/upstream-build/scripts/with-of.sh pkDump1439 -case case > "$WORK/run.log" 2>&1
grep -E '^(COEF|REF|MAP|KIN|FB|K) ' "$WORK/run.log" > "$HERE/refs/pkdump.ref.txt"
wc -l "$HERE/refs/pkdump.ref.txt"
