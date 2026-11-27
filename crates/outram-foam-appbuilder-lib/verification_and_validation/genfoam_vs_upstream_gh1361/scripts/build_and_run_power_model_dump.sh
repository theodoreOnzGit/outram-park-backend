#!/bin/bash
# gh:#1230 round 35: build the powerModelDump driver against the existing libFFSSrc2/libCommon (unmodified GeN-Foam 652b3da
# sources), stage the case, run each power model, and write the gzipped references into refs/.
# usage: build_and_run_power_model_dump.sh [nSteps] [nOuter]       (scratch under /home/user/upstream-build/gh1230, deleted at the end)
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1230
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
NSTEPS=${1:-12}
NOUTER=${2:-1}
rm -rf $W; mkdir -p $W/driver
cp -r $HERE/upstream_driver/power_model_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
bash $HERE/scripts/make_power_model_case.sh $W/case $NSTEPS $NOUTER ${DT:-0.5} >/dev/null
(cd $W/case && blockMesh > $W/log.blockMesh 2>&1; echo "blockMesh exit $?"; checkMesh > $W/log.checkMesh 2>&1; tail -3 $W/log.checkMesh)
# interpolatedNuclearFuelPin::correct divides by alpha_ (0 in a cell with no structure) and traps SIGFPE under the default
# `trapFpe 1`; the NaN it produces is in an unzoned cell only, which the port's reader ignores (README round 35).
for m in lumpedNuclearStructure heatedPin nuclearFuelPin interpolatedNuclearFuelPin; do
  [ $m = interpolatedNuclearFuelPin ] && export FOAM_SIGFPE=false
  powerModelDump -case $W/case $W/case/constant/structureProperties.$m $W/$m.txt > $W/log.$m 2>&1
  echo "$m exit $?"; wc -c $W/$m.txt
  gzip -9 -n -c $W/$m.txt > $HERE/refs/pmd_${m}_o${NOUTER}.ref.txt.gz
done
