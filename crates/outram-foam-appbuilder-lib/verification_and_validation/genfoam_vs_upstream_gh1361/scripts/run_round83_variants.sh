#!/bin/bash
# gh:#1957, gh:#1967 round 83: run the real GeN-Foam binary (652b3da on ESI v2506, serial) on the variants of make_round83_variant.py and
# extract the references refs/r83_<variant>.ref.txt.gz (fields) and refs/r83_<variant>_log.ref.txt.gz (every `h*`, `k*`, `epsilon*` solve:
# `S <step> <field> <initial residual> <iterations>`, log order).   usage: run_round83_variants.sh <variant>...
#   c*  : tests/data/genfoam_1d_onephase, fc_protocol.sh TIGHT=1 PSOLVER=pcg (round 38's fc_channel_pcg protocol), END_TIME 1 (108 steps)
#   b*,q*: tests/data/genfoam_1d_boiling, boiling_mules_protocol.sh EVERY_STEP=1 TIGHT=1 (round 7's protocol), END_TIME 0.45 (60 steps + 1)
#   k*  : tests/data/genfoam_psbt2_ex1_gh1802, psbt2_pcg_protocol.sh EVERY_STEP=1 TIGHT=1 (round 16's protocol), END_TIME 0.09 (25 steps + 1)
set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
DATA=$HERE/../../tests/data
RUNS=/home/user/upstream-build/gh1361/runs
for v in "$@"; do
  d=$RUNS/r83_$v
  rm -rf "$d"; mkdir -p "$d"
  case $v in
    c*) cp -r "$DATA/genfoam_1d_onephase/." "$d/"; (cd "$d" && TIGHT=1 PSOLVER=pcg END_TIME=1 bash "$HERE/scripts/fc_protocol.sh") ;;
    b*|q*) cp -r "$DATA/genfoam_1d_boiling/." "$d/"; (cd "$d" && EVERY_STEP=1 TIGHT=1 END_TIME=0.45 bash "$HERE/scripts/boiling_mules_protocol.sh") ;;
    k*) cp -r "$DATA/genfoam_psbt2_ex1_gh1802/." "$d/"; (cd "$d" && EVERY_STEP=1 TIGHT=1 END_TIME=0.09 bash "$HERE/scripts/psbt2_pcg_protocol.sh" >/dev/null) ;;
  esac
  python3 -I "$HERE/scripts/make_round83_variant.py" "$d" "$v" || exit 1
  (cd "$d" && /home/user/upstream-build/scripts/with-of.sh GeN-Foam > log.GeN-Foam 2>&1; echo "$v exit $? steps $(grep -c '^Time = ' log.GeN-Foam)")
  case $v in
    c*) bash "$HERE/scripts/extract_fc_reference.sh" "r83_$v" "r83_$v" 40 "$(seq -s ' ' 1 108)" 108 >/dev/null ;;
    b*|q*) NC=40 EXTRA_FIELDS="Re.liquid.structure Re.vapour.structure alpha.passiveStructure" \
             bash "$HERE/scripts/extract_boiling_reference.sh" "r83_$v" "r83_$v" . 1 60 >/dev/null ;;
    k*) NC=1170 EXTRA_FIELDS="k.liquid k.vapour epsilon.liquid epsilon.vapour nut.liquid nut.vapour alphat.liquid alphat.vapour" \
             bash "$HERE/scripts/extract_boiling_reference.sh" "r83_$v" "r83_$v" . 1 25 >/dev/null ;;
  esac
  awk '/^Time = /{step++; next}
       /Solving for (h|k|epsilon)/ {f=$4; sub(/,/,"",f); a=""; n="";
         for(i=1;i<=NF;i++){ if($i=="residual"&&$(i+1)=="="&&a==""){a=$(i+2); sub(/,/,"",a)} if($i=="Iterations") n=$(i+1)}
         print "S", step, f, a, n}' "$d/log.GeN-Foam" > "/home/user/upstream-build/gh1361/r83_${v}_log.ref.txt"
  gzip -9 -n -c "/home/user/upstream-build/gh1361/r83_${v}_log.ref.txt" > "$HERE/refs/r83_${v}_log.ref.txt.gz"
  rm -f "/home/user/upstream-build/gh1361/r83_${v}_log.ref.txt" "/home/user/upstream-build/gh1361/r83_$v.ref.txt"
  [ -n "${KEEP:-}" ] || rm -rf "$d"
done
