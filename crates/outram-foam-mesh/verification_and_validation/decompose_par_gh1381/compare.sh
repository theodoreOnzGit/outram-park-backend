#!/bin/bash
# compare.sh <workdir> <decompose_par binary>: run the port on every case run_upstream.sh
# produced in <workdir> and diff the processorN trees byte for byte.
# Prints one line per variant: files compared, files identical.
set -u
work=${1:?workdir}; bin=${2:?decompose_par example binary}
src=$(dirname "$0")/run_upstream.sh
total=0; same=0; badvar=0
while read -r line; do
  read -r c name method np n order delta <<<"$line"
  d=$work/${c}_$name; r=$work/port_${c}_$name
  rm -rf "$r"; mkdir -p "$r/constant"; cp -r "$d/constant/polyMesh" "$r/constant/"
  n=${n//[()]/}; set -- ${n//_/ }
  args=("$r" "$method" "$1" "$2" "$3")
  [ "$order" = "-" ] && args+=(xyz) || args+=("$order")
  [ "$delta" = "-" ] && args+=(0.001) || args+=("$delta")
  "$bin" "${args[@]}" >/dev/null || { echo "PORT FAILED $c $name"; continue; }
  nf=0; ns=0
  for f in $(cd "$d" && find processor* -type f | sort); do
    nf=$((nf+1))
    if cmp -s "$d/$f" "$r/$f"; then ns=$((ns+1)); else echo "  DIFF $c $name $f"; fi
  done
  extra=$(cd "$r" && find processor* -type f | sort | while read -r f; do [ -f "$d/$f" ] || echo "$f"; done | wc -l)
  echo "$c $name: $ns of $nf files identical, $extra extra port files"
  total=$((total+nf)); same=$((same+ns)); [ "$ns" = "$nf" ] || badvar=$((badvar+1))
done < <(sed -n '/^variants=(/,/^)/p' "$src" | grep '^ "' | tr -d '"')
echo "TOTAL: $same of $total files byte-identical; variants with a difference: $badvar"
