#!/bin/sh
# LIGGGHTS `dump custom id x y z vx vy vz` -> the snapshot CSV layout used by
# reference-data/liggghts/. Pure reshape: fields are copied as printed, no
# rounding and no reformatting (the dumps already carry %.17g).
set -eu
src=$1; dst=$2
{
  printf 'id,x,y,z,vx,vy,vz\n'
  awk 'f { printf "%s,%s,%s,%s,%s,%s,%s\n", $1,$2,$3,$4,$5,$6,$7 }
       /^ITEM: ATOMS/ { f=1 }' "$src" | sort -t, -k1,1n
} > "$dst"
echo "$dst: $(( $(wc -l < "$dst") - 1 )) particles"
