#!/bin/bash
# gh:#1855 round 31: like extract_msfr_th_fields.sh but every STRIDE-th cell only (cells 0, S, 2S, ...), to keep a reference small.
# usage: extract_msfr_th_fields_sub.sh <run dir> <out.txt> "<time dirs>" "<field names>" <stride>
R=$1; OUT=$2; TIMES=$3; FIELDS=$4; S=${5:-11}
echo "TIMEPRINT $TIMES" > "$OUT"
echo "STRIDE $S" >> "$OUT"
for t in $TIMES; do
  for f in $FIELDS; do
    file=$R/$t/fluidRegion/$f
    [ -f "$file" ] || file=$file.gz
    printf "F %s %s " "$t" "$f" >> "$OUT"
    (case $file in *.gz) zcat "$file";; *) cat "$file";; esac) \
      | awk -v S=$S '/^\($/{if(!s){s=1;next}} /^\)$/{if(s)exit} s{ if (c%S==0) {gsub(/[()]/,""); printf "%s ", $0}; c++ }' >> "$OUT"
    echo >> "$OUT"
  done
done
