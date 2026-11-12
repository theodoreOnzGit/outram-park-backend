#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# gh:#1411 -- run the liquid-bridge cohesion decks on upstream LIGGGHTS-PUBLIC
# and convert each dump to its committed CSV (dump2cap.sh).
#   ./run_capillary.sh <lmp_serial> [deck-name ...]
# default decks: cap_easo_pair cap_easo_hard cap_washino_pair cap_washino_limit
#                cap_easo_bed cap_washino_bed
# (cap_bed.data comes from make_capillary_decks.sh.)
set -eu
lmp=$(realpath "$1"); shift
here=$(cd "$(dirname "$0")" && pwd)
decks=${*:-cap_easo_pair cap_easo_hard cap_washino_pair cap_washino_limit cap_easo_bed cap_washino_bed}
work=$(mktemp -d)
cp "$here/cap_bed.data" "$work/"
cd "$work"
for d in $decks; do
  "$lmp" -in "$here/in.$d" > "log.$d" 2>&1
  sh "$here/dump2cap.sh" "dump.$d" "$here/$d.csv"
done
