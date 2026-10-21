#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Extract ONE timestep from a multi-frame LIGGGHTS `dump custom id x y z vx vy vz`
# into the snapshot CSV layout used by reference-data/liggghts/.
#
# dump2csv.sh handles the single-frame output of `write_dump`; this handles a
# running `dump`, so a trajectory can be compared frame by frame rather than
# only at its endpoint. Pure reshape, like dump2csv.sh: fields are copied
# exactly as LIGGGHTS printed them.
#
# Usage: ./dumpframe.sh <dump> <timestep> <out.csv>
set -eu
src=$1; want=$2; dst=$3
{
  printf 'id,x,y,z,vx,vy,vz\n'
  awk -v want="$want" '
    /^ITEM: TIMESTEP/ { getline; ts = $1 + 0; inatoms = 0; next }
    /^ITEM: ATOMS/    { inatoms = (ts == want); next }
    /^ITEM:/          { inatoms = 0; next }
    inatoms           { printf "%s,%s,%s,%s,%s,%s,%s\n", $1,$2,$3,$4,$5,$6,$7 }
  ' "$src" | sort -t, -k1,1n
} > "$dst"
echo "$dst: timestep $want, $(( $(wc -l < "$dst") - 1 )) particles"
