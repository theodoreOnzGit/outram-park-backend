#!/bin/bash
# gh:#1927 round 63: stage, run with the real binary and extract the reference of one variant.
# usage: run_pk_variant_r63.sh <steady-state case dir> <scratch dir> <variant> <refs dir>      (reference refs/pk63_<variant>.ref.txt.gz)
SS=$1
W=$2
V=$3
R=$4
D=$(dirname "$0")
C=$W/$V
bash "$D/make_pk_variant_r63.sh" "$SS" "$C" "$V" || exit 1
bash "$D/run_fullcoupling_transient.sh" "$C"
python3 -I "$D/extract_fullcoupling_transient.py" pkv "$C" "$W/pk63_$V.ref.txt" || exit 1
gzip -9 -n -c "$W/pk63_$V.ref.txt" > "$R/pk63_$V.ref.txt.gz"
ls -la "$R/pk63_$V.ref.txt.gz"
tail -3 "$C/log.GeN-Foam"
