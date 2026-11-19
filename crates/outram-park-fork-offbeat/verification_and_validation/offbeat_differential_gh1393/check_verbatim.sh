#!/bin/sh
# Checks that every line marked UPSTREAM in a dump driver occurs (whitespace
# normalised, trailing // comment stripped) in the upstream OFFBEAT source tree.
# Usage: check_verbatim.sh driver.cpp upstream_root scratch_file
drv="$1"; root="$2"; scratch="$3"
norm() { sed 's/[[:space:]]\+/ /g; s/^ //; s/ $//'; }
find "$root/offbeatLib" \( -name '*.C' -o -name '*.H' \) -print0 | xargs -0 cat | norm > "$scratch"
missing=0
grep 'UPSTREAM' "$drv" | sed 's,//.*$,,' | norm | while read -r line; do
  [ -z "$line" ] && continue
  if ! grep -qF -- "$line" "$scratch"; then echo "NOT FOUND: $line"; fi
done
echo "check done"
