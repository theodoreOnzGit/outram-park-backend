#!/bin/bash
# gh:#1878 round 38 item 1: upstream references for the face-centred `nNonOrthogonalCorrectors` loop (pEqnf_1p.H), real unmodified binary,
# tight protocol (scripts/fc_protocol.sh TIGHT=1 PSOLVER=pcg), scratch runs under gh1361/runs/, extracted to refs/ and deleted.
#   fc_ch_no1       : the round-4 channel (tests/data/genfoam_1d_onephase), nNonOrthogonalCorrectors 1, 108 steps, every field
#   fc_cav_no1      : the shipped cavity, nNonOrthogonalCorrectors 1, 100 steps
#   fc_cav_no2      : the shipped cavity, nNonOrthogonalCorrectors 2, 100 steps
#   fc_cav_no1_final: the shipped cavity, nNonOrthogonalCorrectors 1 and `p_rghFinal` relTol 0.05
# usage: run_fc_nonorth_variants.sh  (from any directory)
HERE="$(cd "$(dirname "$0")" && pwd)"
CASES="$HERE/../../../tests/data"
RUNS=/home/user/upstream-build/gh1361/runs
cd /home/user/upstream-build/gh1361 || exit 1
run() { # name case end_time extra-env...
  local name=$1 case=$2 end=$3; shift 3
  env PRE_RUN="$HERE/fc_protocol.sh" TIGHT=1 PSOLVER=pcg END_TIME=$end "$@" bash "$HERE/run_case.sh" "$CASES/$case" "$name" > /dev/null 2>&1
  grep -c '^Time = ' "$RUNS/$name/log.GeN-Foam"
}
run r38_ch_no1 genfoam_1d_onephase 1 NNONORTH=1
bash "$HERE/extract_fc_reference.sh" r38_ch_no1 fc_ch_no1 40 "$(seq -s' ' 1 108)" 108
rm -rf "$RUNS/r38_ch_no1"
run r38_cav_no1 genfoam_2d_cavity_boussinesq 1.2 NNONORTH=1
bash "$HERE/extract_fc_reference.sh" r38_cav_no1 fc_cav_no1 1600 "1 2 3 10 50 100" 100
rm -rf "$RUNS/r38_cav_no1"
run r38_cav_no2 genfoam_2d_cavity_boussinesq 1.2 NNONORTH=2
bash "$HERE/extract_fc_reference.sh" r38_cav_no2 fc_cav_no2 1600 "1 2 3 10 50 100" 100
rm -rf "$RUNS/r38_cav_no2"
run r38_cav_no1_final genfoam_2d_cavity_boussinesq 1.2 NNONORTH=1 FINAL_RELTOL=0.05
bash "$HERE/extract_fc_reference.sh" r38_cav_no1_final fc_cav_no1_final 1600 "1 2 3 10 50 100" 100
rm -rf "$RUNS/r38_cav_no1_final"
