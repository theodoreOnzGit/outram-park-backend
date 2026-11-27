#!/bin/bash
# gh:#1781 round 23: the fields the REAL `GeN-Foam` executable WROTE (writePrecision 17) for a single-region
# `legacyThermoMechanics` run, in the `F <step> <name> v1 v2 ...` format of `refs/legacy_tm_esfr.ref.txt.gz`
# (a vector is flattened x y z per cell; a `uniform` internalField is expanded to <nCells> values).
# usage: extract_legacy_tm_written.sh <run dir> <region> <out.txt> "<times>" "<fields>" <nCells>
R=$1; REG=$2; OUT=$3; TIMES=$4; FIELDS=$5; N=$6
: > "$OUT"
for t in $TIMES; do
  for f in $FIELDS; do
    file=$R/$t/$REG/$f
    printf "F %s %s " "$t" "$f" >> "$OUT"
    awk -v n="$N" '
      /^internalField[ \t]+uniform/ { v=$3; gsub(/[();]/," ",$0); sub(/^internalField +uniform */,"",$0); u=$0; for(i=0;i<n;i++) printf "%s ", u; exit }
      /^\($/ { if(!s){ s=1; next } }
      /^\)$/ { if(s) exit }
      s { gsub(/[()]/,""); printf "%s ", $0 }' "$file" >> "$OUT"
    echo >> "$OUT"
  done
done
