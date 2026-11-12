#!/bin/bash
# Build GeN-Foam 652b3da0cc55 libCommon (unmodified sources; only the
# link-only -lfvOptions is dropped from Make/options, see README.md) and the
# genfoamCommonDump driver, run it, and write common_dump.ref.txt.gz.
# Needs the ESI v2506 environment of /home/user/upstream-build/scripts.
set -e
HERE="$(cd "$(dirname "$0")/.." && pwd)"
export GENFOAM_SRC=/home/user/upstream-build/GeN-Foam-src
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
export WM_NCOMPPROCS=2
WORK=/home/user/upstream-build/gh1361/common_dump
rm -rf "$WORK"
mkdir -p "$WORK"
cp -r "$HERE/upstream_driver/common_dump/." "$WORK/"
cd "$GENFOAM_SRC/src/classes/common"
sed -i '/-lfvOptions/d' Make/options
wmakeLnInclude .
/usr/bin/time -v wmake -j2 libso . > /home/user/upstream-build/build-genfoam-common.log 2>&1
cd "$WORK"
wmake > /home/user/upstream-build/build-genfoam-commondump.log 2>&1
genfoamCommonDump "$WORK/case" | grep -E '^(#|RBF|X |V |Q |INV|W |F |TBL|XS|YS|QIN|VIN|QOUT|VOUT|K |INT|NQ|NP|TP|DICT)' > "$WORK/common_dump.ref.txt"
gzip -9 -n -c "$WORK/common_dump.ref.txt" > "$HERE/refs/common_dump.ref.txt.gz"
ls -l "$HERE/refs/common_dump.ref.txt.gz"
