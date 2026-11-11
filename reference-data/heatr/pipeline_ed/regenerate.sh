#!/bin/bash
# Regenerate the NJOY2016 oracles of crates/njoy-outram-park-fork/tests/heatr_displacement_energy_vs_njoy2016.rs
# (OpenMC #4022: make_ace(displacement_energy=...), gh:#1456).
#
# usage: regenerate.sh <path to NJOY2016 executable>
#
# Each case runs its committed `<case>.njoy-input` on the ENDF tape it names
# (reference-data/endf/, copied to tape20): RECONR, BROADR at 293.6 K, then HEATR
# with the displacement energy on card 2 reading the ASCII PENDF. The oracle kept
# is the MF=3 / MT=444 section of HEATR's tape (passed through MODER), as
# `<case>.mt444.txt`: the full tape is megabytes, the section a few kilobytes.
set -euo pipefail
NJ=${1:?NJOY2016 executable}
HERE=$(cd "$(dirname "$0")" && pwd)
ENDF=$HERE/../../endf
for deck in "$HERE"/*.njoy-input; do
  case=$(basename "$deck" .njoy-input)
  endf=$(sed -n 's/^# endf: //p' "$deck")
  work=$(mktemp -d)
  cp "$ENDF/$endf" "$work/tape20"
  grep -v '^#' "$deck" > "$work/input"
  (cd "$work" && "$NJ" < input > output)
  # lines 70-75 hold MAT, 71-72 MF (2 chars), 73-75 MT: select MF=3, MT=444
  awk '{ if (substr($0,67,4)+0 > 0 && substr($0,71,2)+0 == 3 && substr($0,73,3)+0 == 444) print }' \
    "$work/tape34" > "$HERE/$case.mt444.txt"
  rm -rf "$work"
done
