#!/bin/sh
# LIGGGHTS `dump custom id x y z vx vy vz omegax omegay omegaz [fx fy fz]`
# -> the WIDE trajectory CSV layout used by reference-data/liggghts/:
# one row per sampled step, `step` then per atom id ascending,
# `x y z vx vy vz omegax omegay omegaz`.
#
# Pure reshape: the nine kept fields are copied as printed, no rounding and no
# reformatting (the dumps already carry %.17g from the precision patch). Any
# force columns the dump also carries are dropped -- they are not compared.
set -eu
src=$1; dst=$2
awk '
  /^ITEM: TIMESTEP/      { mode="step"; next }
  /^ITEM: NUMBER OF/     { mode="n";    next }
  /^ITEM: BOX BOUNDS/    { mode="box";  next }
  /^ITEM: ATOMS/         { mode="atom"; next }
  mode=="step" { if (nstep++) emit(); step=$1; delete a; mode=""; next }
  mode=="n"    { n=$1; mode=""; next }
  mode=="atom" { a[$1]=$2","$3","$4","$5","$6","$7","$8","$9","$10 }
  function emit(  i,line) {
    if (!hdr) {
      line="step"
      for (i=1;i<=n;i++) line=line",x"i",y"i",z"i",vx"i",vy"i",vz"i",omegax"i",omegay"i",omegaz"i
      print line; hdr=1
    }
    line=step
    for (i=1;i<=n;i++) line=line","a[i]
    print line
  }
  END { emit() }
' "$src" > "$dst"
echo "$dst: $(( $(wc -l < "$dst") - 1 )) frames"
