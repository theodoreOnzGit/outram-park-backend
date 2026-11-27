#!/bin/bash
# gh:#1879 round 37: extract the fields of a `run_msfr_th_radial_variants.sh` run into a small reference text.
#   G <time> <field> v v v ...        every 11th cell of the whole mesh (U: 3 values per cell)    [U p_rgh T]
#   Z <time> <field> v v v ...        every 7th cell of the `hx` zone, in zone order              [scalar volFields of the power model]
#   P <time> <field> v v v ...        every 29th cell of `hx`, the radial profile of each cell, `|`-separated   [Trad.<model>, T.<lumped>]
# A `uniform X` field prints `U X`. usage: extract_radial_fields.sh <run dir> <out> "<times>" "<global fields>" "<zone fields>" "<profile fields>"
R=$1; OUT=$2; TIMES=$3; GF=$4; ZF=$5; PF=$6
HERE=$(cd "$(dirname "$0")/.." && pwd)
# hx cells in zone order (cellZones.gz of the msfr mesh)
zcat "$HERE/cases/msfr_neutro/polyMesh/cellZones.gz" | awk '/^hx$/{z=1} z&&/cellLabels/{getline; getline; r=1; next} z&&r&&/^\)/{exit} z&&r{print $1}' > "$OUT.hx"
echo "TIMES $TIMES" > "$OUT"
rd() { # file [ff] -> stream of the internalField entries (one per line), or `uniform X`
  case $1 in *.gz) zcat "$1";; *) cat "$1";; esac | awk -v FF="${2:-0}" '
    BEGIN{if(FF==1)inl=1}
    /^internalField +uniform/{gsub(";",""); print "UNIFORM " $3; exit}
    /^internalField/{inl=1; next}
    inl&&!s&&/^\($/{s=1; next}
    inl&&s&&!ml&&/^\)$/{exit}
    s&&FF==1&&/^[ \t]*$/{next}
    s&&FF==1{   # IOFieldField: `N(a b c)`, `N{v}` (uniform) or the multi-line `N`, `(`, one value per line, `)`
      if (ml) { if ($0 ~ /^\)$/) { print out; ml=0 } else if ($0 != "(") out = out " " $0; next }
      if ($0 ~ /^[0-9]+$/) { ml=1; out=$0; next }
      if ($0 ~ /^[0-9]+\{.*\}$/) { n=$0; sub(/\{.*/,"",n); v=$0; sub(/^[0-9]+\{/,"",v); sub(/\}$/,"",v); out=n; for(i=0;i<n;i++) out=out " " v; print out; next }
      print; next }
    s{print}'
}
for t in $TIMES; do
  for f in $GF; do
    printf "G %s %s " "$t" "$f" >> "$OUT"
    rd "$R/$t/fluidRegion/$f" | awk '{ if (c%11==0) {gsub(/[()]/,""); printf "%s ", $0}; c++ }' >> "$OUT"; echo >> "$OUT"
  done
  for f in $ZF; do
    printf "Z %s %s " "$t" "$f" >> "$OUT"
    rd "$R/$t/fluidRegion/$f" | awk -v IDX="$OUT.hx" -v S=7 '
      BEGIN{n=0; while ((getline l < IDX) > 0) {idx[n++]=l}}
      NR==1&&/^UNIFORM/{u=1; print; next}
      {v[NR-1]=$0}
      END{if(!u){for(i=0;i<n;i+=S) printf "%s ", v[idx[i]]}}' >> "$OUT"; echo >> "$OUT"
  done
  for f in $PF; do
    printf "P %s %s " "$t" "$f" >> "$OUT"
    rd "$R/$t/fluidRegion/$f" 1 | awk -v IDX="$OUT.hx" -v S=29 '
      BEGIN{n=0; while ((getline l < IDX) > 0) {idx[n++]=l}}
      {v[NR-1]=$0}
      END{for(i=0;i<n;i+=S){x=v[idx[i]]; gsub(/[()]/," ",x); printf "%s |", x}}' >> "$OUT"; echo >> "$OUT"
  done
done
rm -f "$OUT.hx"
