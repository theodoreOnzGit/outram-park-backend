#!/bin/bash
# Regenerate the NJOY2016 oracles of tests/heatr_driver_vs_njoy2016.rs.
#
# usage: regenerate.sh <path to NJOY2016 executable>
#
# Each case is run from its committed `<case>.njoy-input` deck on the ENDF
# tape it names (reference-data/endf/, copied to tape20). The deck writes
# RECONR's PENDF in ASCII (tape32), runs HEATR reading that ASCII PENDF
# (nin > 0, so HEATR sees exactly the 7-figure values the port is given),
# and passes HEATR's tape through MODER (tape34), whose sequence numbering
# is the one Tape::write reproduces. An `nplot` deck also writes tape35.
# HEATR's part of NJOY's listing is kept as <case>.listing.gz.
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
  gzip -9n < "$work/tape32" > "$HERE/$case.pendf.gz"
  gzip -9n < "$work/tape34" > "$HERE/$case.heatr.gz"
  if [ -s "$work/tape35" ]; then gzip -9n < "$work/tape35" > "$HERE/$case.plot.gz"; fi
  # HEATR's listing: from its banner to the next module's.
  awk '/heatr\.\.\.prompt kerma/{p=1;print;next} p && /^ [a-z]+\.\.\./{exit} p && /^ njoy 20/{exit} p' \
    "$work/output" | gzip -9n > "$HERE/$case.listing.gz"
  rm -rf "$work"
done
