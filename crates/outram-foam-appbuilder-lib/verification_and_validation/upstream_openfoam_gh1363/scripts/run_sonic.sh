#!/bin/bash
# gh:#1363 row 3b. Build driver/sonicDump.C against the ESI v2506 thermo and
# turbulence closure built by scripts/build_closure.sh, then run it on the
# sonicFoam laminar shockTube (100 cells instead of 1000, fixed dt) and write
# the committed gzipped reference.
#   usage: run_sonic.sh [deltaT] [nSteps] ["dump steps"] [nCells]
#   env: DUMPSTAGES (yes/no), DUMPSTAGEFROM, DUMPSTAGESTEPS (last step with stage dumps),
#        NOREF=1 leaves the committed reference untouched (diagnostic runs)
set -e
DT=${1:-1e-5}
NSTEPS=${2:-700}
DUMPS=${3:-"1 2 3 4 5 10 100 700"}
NC=${4:-100}
HERE=$(cd "$(dirname "$0")/.." && pwd)
W=/home/user/upstream-build/gh1363
WITH=/home/user/upstream-build/scripts/with-of.sh
OF=/home/user/upstream-build/OpenFOAM-v2506
TUT=$OF/tutorials/compressible/sonicFoam/laminar/shockTube
mkdir -p $W/bin
rm -rf $W/src/sonicDump; mkdir -p $W/src/sonicDump/Make
cp $HERE/driver/sonicDump.C $W/src/sonicDump/
printf 'sonicDump.C\n\nEXE = %s/bin/sonicDump\n' $W > $W/src/sonicDump/Make/files
cat > $W/src/sonicDump/Make/options <<EOF
EXE_INC = \\
    -I\$(LIB_SRC)/finiteVolume/lnInclude \\
    -I\$(LIB_SRC)/meshTools/lnInclude \\
    -I\$(LIB_SRC)/transportModels/compressible/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/basic/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/specie/lnInclude \\
    -I\$(LIB_SRC)/thermophysicalModels/thermophysicalProperties/lnInclude \\
    -I\$(LIB_SRC)/TurbulenceModels/turbulenceModels/lnInclude \\
    -I\$(LIB_SRC)/TurbulenceModels/compressible/lnInclude

EXE_LIBS = \\
    -lfiniteVolume \\
    -lmeshTools \\
    -lcompressibleTransportModels \\
    -lfluidThermophysicalModels \\
    -lspecie \\
    -lturbulenceModels \\
    -lcompressibleTurbulenceModels
EOF
(cd $W/src/sonicDump && WM_NCOMPPROCS=2 $WITH wmake)
rm -rf $W/src/sonicDump/Make/linux* $W/sonic; mkdir -p $W/sonic/0 $W/sonic/system $W/sonic/constant
sed "s#(1000 1 1)#($NC 1 1)#" $TUT/system/blockMeshDict > $W/sonic/system/blockMeshDict
cp $TUT/system/fvSchemes $TUT/system/fvSolution $W/sonic/system/
cp $TUT/constant/thermophysicalProperties $TUT/constant/turbulenceProperties $W/sonic/constant/
cat > $W/sonic/system/controlDict <<EOF
FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }
application sonicDump;
startFrom startTime;
startTime 0;
stopAt endTime;
endTime $(awk -v d=$DT -v n=$NSTEPS 'BEGIN{printf "%.17g", d*(n-0.25)}');
deltaT $DT;
writeControl timeStep;
writeInterval 100000;
writeFormat ascii;
writePrecision 17;
timeFormat general;
timePrecision 8;
runTimeModifiable false;
adjustTimeStep no;
dumpSteps ($DUMPS);
dumpStages ${DUMPSTAGES:-yes};
dumpStageSteps ${DUMPSTAGESTEPS:-2};
dumpStageFrom ${DUMPSTAGEFROM:-1};
EOF
(cd $W/sonic && $WITH blockMesh > blockMesh.log 2>&1)
HALF=$((NC/2))
mkfield() { # name dimensions lowvalue highvalue
  {
    echo "FoamFile { version 2.0; format ascii; class volScalarField; object $1; }"
    echo "dimensions $2;"
    echo "internalField nonuniform List<scalar> $NC"; echo "("
    for i in $(seq 0 $((NC-1))); do if [ $i -lt $HALF ]; then echo "$4"; else echo "$3"; fi; done
    echo ")"; echo ";"
    echo "boundaryField { sides { type zeroGradient; } empty { type empty; } }"
  } > $W/sonic/0/$1
}
mkfield T "[0 0 0 1 0 0 0]" 278.746 348.432
mkfield p "[1 -1 -2 0 0 0 0]" 10000 100000
cp $TUT/0.orig/U $W/sonic/0/U
(cd $W/sonic && $WITH ../bin/sonicDump > sonic.log 2>&1)
grep -E "^# STEP|^# GEOMETRY|^S [0-9]+ (end|geom)|^C " $W/sonic/sonic.log > $W/sonic/ref_sonic.txt
# solver log of the first ten steps as bit patterns: L <step> <field> <initial> <final> <iterations>
python3 -I - $W/sonic/sonic.log >> $W/sonic/ref_sonic.txt <<'PY'
import re, struct, sys
step = 0
for line in open(sys.argv[1]):
    if line.startswith("Time = "):
        step += 1
    m = re.match(r"(?:smoothSolver|diagonal):  Solving for (\w+), Initial residual = (\S+), Final residual = (\S+), No Iterations (\d+)", line)
    if m and step <= 10:
        h = lambda x: struct.pack(">d", float(x)).hex()
        print("L", step, m.group(1), h(m.group(2).rstrip(",")), h(m.group(3).rstrip(",")), m.group(4))
PY
[ -n "$NOREF" ] || gzip -9n -c $W/sonic/ref_sonic.txt > $HERE/shock/reference/sonic_v2506_shockTube.txt.gz
