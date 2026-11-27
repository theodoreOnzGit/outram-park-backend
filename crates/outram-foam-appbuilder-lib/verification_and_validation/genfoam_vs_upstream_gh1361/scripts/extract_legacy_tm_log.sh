#!/bin/bash
# gh:#1781 round 23: the solver log of a `legacyTmDump` run, one record per line, at the 17 digits the log prints:
#   T <step>                          `Time = <step>`
#   S <field> <initial> <final> <n>   `DICPCG|DILUPBiCG: Solving for <field>, Initial residual = ..., No Iterations n`
#   M <max sigmaEq>                   `Max sigmaEq = ...`
#   R <residual>                      `residual ...` (the driver prints `getResidual()`)
# usage: extract_legacy_tm_log.sh <log file> <out.txt>
sed -n \
  -e 's/^Time = \([0-9.e+-]*\)$/T \1/p' \
  -e 's/^.*Solving for \([A-Za-z]*\), Initial residual = \([^,]*\), Final residual = \([^,]*\), No Iterations \([0-9]*\).*$/S \1 \2 \3 \4/p' \
  -e 's/^Max sigmaEq = \(.*\)$/M \1/p' \
  -e 's/^residual \(.*\)$/R \1/p' "$1" > "$2"
