#!/bin/bash
# gh:#1361 round 20: internal fields of chosen written steps of a `run_msfr_th_tight.sh` run, in the format of
# `refs/msfr_th_on_head.ref.txt.gz`:  `F <time> <name> v1 v2 ...` (a vector is flattened x y z per cell).
# usage: extract_msfr_th_fields.sh <run dir> <out.txt> "<time dirs>" "<field names>"
R=$1; OUT=$2; TIMES=$3; FIELDS=$4
echo "TIMEPRINT $TIMES" > "$OUT"
for t in $TIMES; do
  for f in $FIELDS; do
    file=$R/$t/fluidRegion/$f
    [ -f "$file" ] || file=$file.gz
    printf "F %s %s " "$t" "$f" >> "$OUT"
    (case $file in *.gz) zcat "$file";; *) cat "$file";; esac) \
      | awk '/^\($/{if(!s){s=1;next}} /^\)$/{if(s)exit} s{gsub(/[()]/,""); printf "%s ", $0}' >> "$OUT"
    echo >> "$OUT"
  done
done
