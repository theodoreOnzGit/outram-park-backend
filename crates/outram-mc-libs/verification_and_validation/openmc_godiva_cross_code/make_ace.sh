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
'$NAME ENDF/B-VIII.0 from outram-park reference-data'/
$MAT 293.6/
1 1/
/
stop
INP
"${NJOY:?set NJOY to the NJOY2016 executable}" < input > njoy.stdout 2>&1
echo "$NAME exit=$? ace=$(ls -la tape24 2>/dev/null | awk '{print $5}')"
