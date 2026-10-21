#!/bin/sh
# Produce one ACE file per nuclide with NJOY2016: RECONR -> BROADR -> PURR -> ACER.
#
#   ENDF_DIR=../../../../reference-data/endf \
#   NJOY=../../../njoy-outram-park-fork/upstream_source/NJOY2016/build/njoy \
#     ./make_ace.sh 9228 n-092_U_235-ENDF8.0.endf U235
#
# PURR is included deliberately: outram-mc-libs has no probability tables at
# all, so OpenMC is run BOTH ways (see godiva.py --ptables) to separate the
# transport comparison from what URR self-shielding is worth.
# $1 = MAT, $2 = tape filename, $3 = short name, $4 = ZA suffix id
MAT=$1; TAPE=$2; NAME=$3
mkdir -p $NAME && cd $NAME
cp "${ENDF_DIR:?set ENDF_DIR to the repo's reference-data/endf}/$TAPE" tape20
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
'$NAME pendf'/
$MAT 0/
.001/
0/
broadr
20 21 22
$MAT 1 0 0 0./
.001/
293.6/
0/
purr
20 22 23
$MAT 1 1 20 64/
293.6/
1.e10/
0/
acer
20 23 0 24 25
1 1 1 .00 0/
'$STAMP'/
$MAT 293.6/
1 1/
/
stop
INP
"${NJOY:?set NJOY to the NJOY2016 executable}" < input > njoy.stdout 2>&1
echo "$NAME stamp=\"$STAMP\""
echo "$NAME exit=$? ace=$(ls -la tape24 2>/dev/null | awk '{print $5}')"
