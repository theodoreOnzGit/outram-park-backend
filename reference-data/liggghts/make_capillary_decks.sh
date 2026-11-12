#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# gh:#1411 -- inputs of the liquid-bridge cohesion cross-code decks
# (easo/capillary/viscous, washino/capillary/viscous).
#
# Writes cap_bed_init.csv (id,x,y,z,vx,vy,vz): the 84 lowest pebbles (z < 0.03 m)
# of the committed settled D/d = 6 bed (pebble_bed_settled.csv), renumbered
# 1..84, with a deterministic velocity added, v = 0.3 (sin(1.7 i), sin(2.3 i),
# sin(0.9 i)) m/s on top of the settled (~1e-6) one. Free of walls and gravity
# (capillary bridges to walls are not ported), the blob expands and shears, so
# bridges stretch and rupture. awk's sin only builds this input file; both
# codes read the committed file.
#   ./make_capillary_decks.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
awk -F, 'BEGIN { print "id,x,y,z,vx,vy,vz" }
  NR>1 && $4<0.03 { n++;
    printf "%d,%.17g,%.17g,%.17g,%.17g,%.17g,%.17g\n", n, $2, $3, $4,
      $5+0.3*sin(1.7*n), $6+0.3*sin(2.3*n), $7+0.3*sin(0.9*n) }' \
  "$here/pebble_bed_settled.csv" > "$here/cap_bed_init.csv"
sh "$here/csv2data.sh" "$here/cap_bed_init.csv" "$here/cap_bed.data" 0.01 2500 -0.2 0.2
