#!/bin/bash
# gh:#1929 round 65: run the REAL GeN-Foam binary on each variant of make_fftf_turb_variants.py (20 steps) and extract
# <PREFIX><variant>.ref.txt / _log.ref.txt (fields incl. k epsilon nut alphat, plus `P` lines: nut/alphat patch values) into <out dir>.
# usage: [VARIANTS="w8_zg ..."] [PREFIX=fftf_r65_] run_fftf_turb_variants.sh <variants dir> <out dir>
VAR=$1
OUT=$2
HERE=$(dirname "$0")
source /home/user/upstream-build/scripts/of-env.sh >/dev/null 2>&1
mkdir -p "$OUT"
for v in ${VARIANTS:-w8_zg w9_calc w10_mapped}; do
  c="$VAR/$v"
  sed -i "s/^endTime .*/endTime         ${END_TIME:-4.9};/" "$c/system/controlDict"
  (cd "$VAR" && GeN-Foam -case "$v" > "$v/log.GeN-Foam" 2>&1; echo "$v exit $?")
  FIELDS="U p p_rgh T alphaPhi htc T.passiveStructure T.fuelAvForNeutronics k epsilon nut alphat" bash "$HERE/extract_fullcoupling_fluid.sh" "$c" "$OUT/${PREFIX:-fftf_r65_}$v"
  TIMES=$(ls "$c" | grep -E '^[0-9.]+$' | grep -v '^0$' | sort -g | awk '{n++; t[n]=$1} END{for(i=1;i<=n;i++) if(i<=100 || i%10==0 || i==n) print t[i]}' | tr '\n' ' ')
  python3 "$HERE/extract_patch_values.py" "$c" "$OUT/${PREFIX:-fftf_r65_}$v.ref.txt" "$TIMES" "nut alphat"
done
