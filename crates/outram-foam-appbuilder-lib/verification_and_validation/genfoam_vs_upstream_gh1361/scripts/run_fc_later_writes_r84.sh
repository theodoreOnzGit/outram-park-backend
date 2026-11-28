#!/bin/bash
# gh:#1938 (README round 84): the real 2D_fullCoupling steady state (Allrun time control, ADJ=1), then four transient stages from its written `100`:
#   sh, pk        : `writeControl adjustableRunTime; writeInterval 10` (round 69's protocol) to endTime 200: every write time 110 ... 200
#   sh_ts9, pk_ts9: `writeControl timeStep; writeInterval 9`: the timeStep write rule counted from the restart's `Time::timeIndex`
#                   (`Time.C:304-306`, `:1111-1112`; the start directory's `uniform/time` index is 476, 476 % 9 = 8)
# usage: run_fc_later_writes_r84.sh <scratch dir>
D=$1
S=$(dirname "$(readlink -f "$0")")
mkdir -p "$D"
ADJ=1 bash "$S/make_fullcoupling_coupled_case.sh" "$D/ss"
bash "$S/run_fullcoupling_coupled.sh" "$D/ss"
for v in sh pk sh_ts9 pk_ts9; do
  case $v in sh*) export SHIPPED=1 ;; *) unset SHIPPED ;; esac
  bash "$S/make_fullcoupling_transient_case.sh" "$D/ss" "$D/$v" > /dev/null
  case $v in
    *_ts9) sed -i 's/^writeControl .*/writeControl timeStep;/; s/^writeInterval .*/writeInterval 9;/' "$D/$v/system/controlDict" ;;
    *)     sed -i 's/^writeControl .*/writeControl adjustableRunTime;/; s/^writeInterval .*/writeInterval 10;/' "$D/$v/system/controlDict" ;;
  esac
  echo "$v: $(bash "$S/run_fullcoupling_transient.sh" "$D/$v")"
done
