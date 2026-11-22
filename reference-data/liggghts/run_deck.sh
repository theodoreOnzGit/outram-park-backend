#!/bin/sh
# Run one small deterministic deck with the patched upstream build and convert
# its dump to the committed wide trajectory CSV.
#   ./run_deck.sh <lmp_serial> <deck-name> [workdir]
# e.g. ./run_deck.sh ../../crates/outram-park-fork-liggghts/upstream_source/LIGGGHTS-PUBLIC/src/lmp_serial twotype_oblique
# Needs in.<deck-name> beside this script; writes <deck-name>.csv beside it.
set -eu
lmp=$(realpath "$1"); name=$2
here=$(cd "$(dirname "$0")" && pwd)
work=${3:-$(mktemp -d)}
cd "$work"
"$lmp" -in "$here/in.$name" > "log.$name" 2>&1
sh "$here/dump2traj.sh" "dump.$name" "$here/$name.csv"
