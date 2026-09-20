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
'$NAME ENDF/B-VIII.0 0K RECONR-only, matched to outram-park write_ace'/
$MAT 0./
1 1/
/
stop
INP
"${NJOY:?set NJOY}" < input > njoy.stdout 2>&1
echo "$NAME exit=$? ace=$(ls -la tape24 2>/dev/null | awk '{print $5}')"
