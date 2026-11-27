#!/bin/bash
# gh:#1903 round 52: build the hxMappingDump driver (libfiniteVolume/libsampling/libmeshTools of OpenFOAM-ESI v2506 only) and run it on
# cases/esfr_hx for the two exchangers ABHX (hxA -> hxB) and BAHX (hxB -> hxA). usage: build_and_run_hx_mapping_dump.sh <out.txt>
# Scratch: /home/user/upstream-build/gh1903 (objects deleted after the link).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=/home/user/upstream-build/gh1903
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
mkdir -p $W; rm -rf $W/driver; mkdir $W/driver
cp -r $HERE/upstream_driver/hx_mapping_dump/. $W/driver/
(cd $W/driver && wmake > $W/build-driver.log 2>&1; echo "driver exit $?"; find . -name '*.o' -delete)
rm -rf $W/mapcase; mkdir $W/mapcase; cp -r $HERE/cases/esfr_hx/. $W/mapcase/
S0=$(date +%s)
hxMappingDump -case $W/mapcase "$1" ABHX:hxA:hxB,BAHX:hxB:hxA > $W/mapcase/log.hxMappingDump 2>&1
echo "hxMappingDump exit $? in $(( $(date +%s)-S0 )) s"
tail -5 $W/mapcase/log.hxMappingDump
