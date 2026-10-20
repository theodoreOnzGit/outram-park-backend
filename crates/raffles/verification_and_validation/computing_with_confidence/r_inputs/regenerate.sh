#!/usr/bin/env bash
# Regenerates the R reference outputs recorded beside this script.
#
#   git clone https://github.com/Adolphus8/Computing-with-Confidence /tmp/cwc
#   mkdir /tmp/cwc_work && cp "/tmp/cwc/Coherent systems/"*.r /tmp/cwc_work/
#   rm /tmp/cwc_work/sra.r            # all rights reserved: never used or kept
#   cp case3_pba_part.r case4_cboxes.r /tmp/cwc_work/
#   (case3_pba_part.r = Case study 3 lines 1-38, i.e. before source("sra.r"),
#    plus 3 added lines printing the 95% interval; case4_cboxes.r = Case study 4
#    with its duplicate scalar block, lines 34-46, deleted: `sed '34,46d'`.)
#   ./regenerate.sh /tmp/cwc_work
#
# Needs only base R 4.x (no CRAN packages): pba BETTER.r loads with the
# `methods` package that ships with R.  Run time: about one minute, almost all
# of it Case study 4 (Pbox$steps = 1000).
set -euo pipefail
W="$1"
HERE="$(cd "$(dirname "$0")" && pwd)"
run() { Rscript "$HERE/run_case.R" "$W" "$1" 2>&1 | grep '^interval_\|^R_VERSION' > "$HERE/$2.r_output.txt"; }
run "Illustrative Examples.r" illustrative
run "Case study 1 - Pressure tank system.r" case1
run "Case study 2 - TRIGA Research Reactor.r" case2
run "case3_pba_part.r" case3_pba_part
run "Case study 4 - Radiation Release Nuclear Reactor.r" case4
run "case4_cboxes.r" case4_cboxes
