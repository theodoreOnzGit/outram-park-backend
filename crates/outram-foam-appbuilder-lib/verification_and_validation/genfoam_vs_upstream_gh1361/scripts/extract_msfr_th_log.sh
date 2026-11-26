#!/bin/bash
# gh:#1361 round 20 (gh:#1838/#1839): the solver log of a `run_msfr_th_tight.sh` run, one line per linear solve:
#   PRGH <step> <outer> <corrector> <initial residual> <iterations>     (DICPCG, p_rgh)
#   EPS  <step> <outer> <initial residual> <iterations>                 (PBiCGStab, epsilon)   [TURB=on only]
#   K    <step> <outer> <initial residual> <iterations>                 (PBiCGStab, k)         [TURB=on only]
#   H    <step> <outer> <initial residual> <iterations>
#   TSTAT <step> <outer> <avg> <min> <max>                               (`T (avg min max)`)
# usage: extract_msfr_th_log.sh <log.GeN-Foam> <out.txt>
awk '
  /^Time = /{s++; o=0}
  /^PIMPLE: iteration /{o=$3; c=0}
  /Solving for p_rgh,/{c++; split($0,a,"Initial residual = "); split(a[2],b,","); split($0,it,"No Iterations "); printf "PRGH %d %d %d %s %s\n", s,o,c,b[1],it[2]}
  /Solving for epsilon,/{split($0,a,"Initial residual = "); split(a[2],b,","); split($0,it,"No Iterations "); printf "EPS %d %d %s %s\n", s,o,b[1],it[2]}
  /Solving for k,/{split($0,a,"Initial residual = "); split(a[2],b,","); split($0,it,"No Iterations "); printf "K %d %d %s %s\n", s,o,b[1],it[2]}
  /Solving for h,/{split($0,a,"Initial residual = "); split(a[2],b,","); split($0,it,"No Iterations "); printf "H %d %d %s %s\n", s,o,b[1],it[2]}
  /^T \(avg min max\)/{printf "TSTAT %d %d %s %s %s\n", s,o,$6,$7,$8}
' "$1" > "$2"
