#!/bin/bash
# gh:#1872 round 50: build the h2Dump driver against the existing libFFSSrc2 (unmodified GeN-Foam 652b3da sources,
# ESI v2506), run it and write the gzipped reference refs/h2_dump.ref.txt.gz. Scratch under /home/user/upstream-build/gh1872.
HERE="$(cd "$(dirname "$0")/.." && pwd)"
W=${W:-/home/user/upstream-build/gh1872}
export GENFOAM_SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}
OF=/home/user/upstream-build/scripts/with-of.sh
export WM_NCOMPPROCS=2
rm -rf "$W"; mkdir -p "$W"
cp -r "$HERE/upstream_driver/h2_dump" "$W/h2_dump"
(cd "$W/h2_dump" && $OF wmake > "$W/build.log" 2>&1; echo "driver exit $?")
$OF h2Dump > "$W/h2_dump.txt" 2> "$W/h2_dump.err"; echo "run exit $?"
wc -l "$W/h2_dump.txt"
gzip -9 -n -c "$W/h2_dump.txt" > "$HERE/refs/h2_dump.ref.txt.gz"
