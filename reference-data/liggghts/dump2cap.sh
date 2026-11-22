#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
#
# LIGGGHTS `dump custom id x y z vx vy vz omegax omegay omegaz fx fy fz
# f_surfaceLiquidContent f_liquidFlux` (gh:#1411) -> the WIDE CSV: one row per
# sampled step, `step` then per atom id ascending the fourteen columns
# `x y z vx vy vz omegax omegay omegaz fx fy fz L flux`, copied as printed
# (%.17g from the output-precision patch, no reformatting).
set -eu
src=$1; dst=$2
awk '
  /^ITEM: TIMESTEP/      { mode="step"; next }
  /^ITEM: NUMBER OF/     { mode="n";    next }
  /^ITEM: BOX BOUNDS/    { mode="box";  next }
  /^ITEM: ATOMS/         { mode="atom"; next }
  mode=="step" { if (nstep++) emit(); step=$1; delete a; mode=""; next }
  mode=="n"    { n=$1; mode=""; next }
  mode=="atom" { a[$1]=$2","$3","$4","$5","$6","$7","$8","$9","$10","$11","$12","$13","$14","$15 }
  function emit(  i,line) {
    if (!hdr) {
      line="step"
      for (i=1;i<=n;i++) line=line",x"i",y"i",z"i",vx"i",vy"i",vz"i",omegax"i",omegay"i",omegaz"i",fx"i",fy"i",fz"i",L"i",flux"i
      print line; hdr=1
    }
    line=step
    for (i=1;i<=n;i++) line=line","a[i]
    print line
  }
  END { emit() }
' "$src" > "$dst"
echo "$dst: $(( $(wc -l < "$dst") - 1 )) frames"
