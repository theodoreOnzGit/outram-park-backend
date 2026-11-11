#!/bin/sh
# Run one deck with the patched upstream build and write BOTH the wide
# trajectory CSV (dump2traj.sh) and the thermo table (the numeric lines of
# the log, `%.17g`), gh:#1416.
#   ./run_deck_thermo.sh <lmp_serial> <deck-name> [ncols]
# Needs in.<deck-name> and the STL it names beside this script. Writes
# <deck-name>.csv and <deck-name>_thermo.txt beside it. `ncols` is the number
# of thermo columns (default 13: step + f_plate[1..12]).
set -eu
lmp=$(realpath "$1"); name=$2; ncols=${3:-13}
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
cp "$here"/*.stl "$work"/
cd "$work"
"$lmp" -in "$here/in.$name" > "log.$name" 2>&1
awk -v n="$ncols" 'NF==n && $1 ~ /^[0-9]+$/ && $2 ~ /^[-+0-9.eE]+$/' "log.$name" > "$here/${name}_thermo.txt"
sh "$here/dump2traj.sh" "dump.$name" "$here/$name.csv"
rm -rf "$work"
