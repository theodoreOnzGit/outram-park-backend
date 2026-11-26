#!/bin/bash
# gh:#1781 round 23: build GeN-Foam 652b3da0cc55's libThermoMechanicsSolvers (legacyThermoMechanics, GFTractionDisplacement;
# unmodified sources copied to a private dir, only Make/files (extendedThermoMechanics out: it needs the lost offbeat API)
# and Make/options (offbeat/genericPatchFields/fvOptions out, -lCommon in) edited), the legacyTmDump driver, and run it.
# usage: build_and_run_legacy_tm_dump.sh <nSteps> <out.txt>      (objects deleted after the link: disk)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1781
S=/home/user/upstream-build/GeN-Foam-src/src/classes
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
export TM_LIB_DIR=$W/lib
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
mkdir -p $W/lib
if [ ! -f $W/lib/libThermoMechanicsSolvers.so ]; then
  rm -rf $W/tmlib; mkdir $W/tmlib
  cp -r $S/thermoMechanics/Make $S/thermoMechanics/thermoMechanics.C $S/thermoMechanics/thermoMechanics.H \
        $S/thermoMechanics/legacyThermoMechanics $S/thermoMechanics/tractionDisplacement $W/tmlib/
  rm -rf $W/tmlib/Make/linux64*
  printf 'thermoMechanics.C\nlegacyThermoMechanics/legacyThermoMechanics.C\ntractionDisplacement/GFTractionDisplacementFvPatchVectorField.C\n\nLIB = %s/lib/libThermoMechanicsSolvers\n' $W > $W/tmlib/Make/files
  python3 -I - <<'EOF'
import re
p='/home/user/upstream-build/gh1781/tmlib/Make/options'
s=open(p).read()
G='/home/user/upstream-build/GeN-Foam-src/src/classes'
s=s.replace('./../common/lnInclude',G+'/common/lnInclude')
s=re.sub(r'[^\n]*-I\./\.\./offbeat[^\n]*\n','',s)
s=s.replace('-loffbeat','').replace('-lgenericPatchFields','').replace('-lfvOptions','')
s=s.replace('-L$(FOAM_USER_LIBBIN)\\','-L$(FOAM_USER_LIBBIN) -lCommon \\')
open(p,'w').write(s)
EOF
  (cd $W/tmlib && wmake -j2 libso . > $W/build-tmlib.log 2>&1; echo "tmlib exit $?"; find . -name '*.o' -delete)
fi
rm -rf $W/driver; mkdir $W/driver
cp -r $HERE/upstream_driver/legacy_tm_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_legacy_tm_case.sh $W/case >/dev/null
export LD_LIBRARY_PATH=$W/lib:$LD_LIBRARY_PATH
S0=$(date +%s)
legacyTmDump -case $W/case "$2" "$1" > $W/case/log.legacyTmDump 2>&1
echo "legacyTmDump exit $? in $(( $(date +%s)-S0 )) s"
tail -5 $W/case/log.legacyTmDump
