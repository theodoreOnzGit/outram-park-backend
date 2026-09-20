#!/bin/sh
# Matched 0 K reference: RECONR -> ACER only. NO BROADR, NO PURR.
#
# This deliberately mirrors what crates/njoy-outram-park-fork's
# examples/write_ace.rs does (reconr at temperature 0.0, kT 0, suffix 0), so a
# comparison against it isolates ACER/table construction from Doppler
# broadening and unresolved probability tables. Comparing our 0 K RECONR-only
# table against the production 293.6 K RECONR+BROADR+PURR table would confound
# three differences at once and could not attribute any of them.
MAT=$1; TAPE=$2; NAME=$3
mkdir -p "$NAME" && cd "$NAME"
cp "${ENDF_DIR:?set ENDF_DIR}/$TAPE" tape20
# ── Provenance stamp, written INTO the ACE file's own comment field ──────────
#
# The ACE `hk` field is 70 characters of free text that every consumer carries
# along, so it is the one place provenance cannot be separated from the data.
# A table found loose on disk, or handed to someone a year later, still says
# which NJOY built it, from which workspace revision, and when.
#
# Derived, never hand-typed: the NJOY2016 revision comes from that checkout,
# the workspace revision from this repository, the date from the clock.
NJOY_DIR=${NJOY_DIR:-$(dirname "$(dirname "$NJOY")")}
NJOY_VER=$(git -C "$NJOY_DIR" describe --tags --always 2>/dev/null || echo unknown)
NJOY_SHA=$(git -C "$NJOY_DIR" rev-parse --short=7 HEAD 2>/dev/null || echo unknown)
OPB_SHA=$(git -C "$(dirname "$0")" rev-parse --short=7 HEAD 2>/dev/null || echo unknown)
STAMP=$(printf '%s E8.0 NJOY2016 %s %s %s opb %s' \
          "$NAME" "$NJOY_VER" "$NJOY_SHA" "$(date -u +%Y-%m-%d)" "$OPB_SHA" | cut -c1-70)

cat > input <<INP
reconr
20 21
'$NAME pendf 0K'/
$MAT 0/
.001/
0/
acer
20 21 0 24 25
1 1 1 .00 0/
'$STAMP'/
$MAT 0./
1 1/
/
stop
INP
"${NJOY:?set NJOY}" < input > njoy.stdout 2>&1
echo "$NAME stamp=\"$STAMP\""
echo "$NAME exit=$? ace=$(ls -la tape24 2>/dev/null | awk '{print $5}')"
