#!/bin/sh
# NJOY2016 reference outputs for the MSRE salt nuclides at 900 K (MSRE N-C2,
# gh:#1127): RECONR (0.001) -> BROADR (900 K) -> PURR (20 bands, 64 ladders)
# -> ACER, the deck of reference-njoy/endf-b-viii.0/293.6K/U236.njoy at 900 K.
#
#   NJOY=crates/njoy-outram-park-fork/upstream_source/NJOY2016/build/njoy \
#   OUT=reference-data/ace/reference-njoy/endf-b-viii.0/900K \
#     [TK=<kelvin, default 900>] sh make_njoy_900k.sh <MAT> <ENDF tape path> <NAME>
#
# Writes $OUT/<NAME>.njoy (the deck), $OUT/<NAME>.pendf.gz (BROADR tape22)
# and $OUT/<NAME>.ace.gz (ACER tape24). The ACE comment field carries the
# NJOY revision, the workspace revision and the date.
set -e
MAT=$1; TAPE=$2; NAME=$3
: "${NJOY:?set NJOY to the NJOY2016 executable}"
: "${OUT:?set OUT to the output directory}"
NJOY_DIR=$(dirname "$(dirname "$NJOY")")
NJOY_VER=$(git -C "$NJOY_DIR" describe --tags --always 2>/dev/null || echo unknown)
NJOY_SHA=$(git -C "$NJOY_DIR" rev-parse --short=7 HEAD 2>/dev/null || echo unknown)
OPB_SHA=$(git -C "$(dirname "$0")" rev-parse --short=7 HEAD 2>/dev/null || echo unknown)
STAMP=$(printf '%s E8.0 NJOY2016 %s %s %s opb %s' \
          "$NAME" "$NJOY_VER" "$NJOY_SHA" "$(date -u +%Y-%m-%d)" "$OPB_SHA" | cut -c1-70)
WORK=$(mktemp -d)
cp "$TAPE" "$WORK/tape20"
cat > "$WORK/input" <<INP
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
${TK:-900.}/
0/
purr
20 22 23
$MAT 1 1 20 64/
${TK:-900.}/
1.e10/
0/
acer
20 23 0 24 25
1 1 1 .90 0/
'$STAMP'/
$MAT ${TK:-900.}/
1 1/
/
stop
INP
(cd "$WORK" && "$NJOY" < input > njoy.stdout 2>&1)
mkdir -p "$OUT"
cp "$WORK/input" "$OUT/$NAME.njoy"
gzip -9n -c "$WORK/tape22" > "$OUT/$NAME.pendf.gz"
gzip -9n -c "$WORK/tape24" > "$OUT/$NAME.ace.gz"
echo "$NAME mat=$MAT stamp=\"$STAMP\" pendf=$(wc -c < "$WORK/tape22") ace=$(wc -c < "$WORK/tape24")"
grep -iE 'error|warning' "$WORK/njoy.stdout" | head -3
rm -rf "$WORK"
