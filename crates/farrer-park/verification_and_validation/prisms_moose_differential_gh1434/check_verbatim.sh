#!/bin/sh
# Checks that every line marked UPSTREAM in a dump driver occurs (whitespace
# normalised, trailing // comment stripped) in the pinned upstream source tree.
# Usage: check_verbatim.sh driver.cpp upstream_root scratch_file
# (PRISMS-Plasticity: <clone>/src ; MOOSE: <clone> after sparse checkout of
#  modules/solid_mechanics and framework.)
drv="$1"; root="$2"; scratch="$3"
norm() { sed 's/[[:space:]]\+/ /g; s/^ //; s/ $//'; }
find "$root" \( -name '*.cc' -o -name '*.C' -o -name '*.h' -o -name '*.H' \) -print0 | xargs -0 cat | norm > "$scratch"
grep 'UPSTREAM' "$drv" | sed 's,//.*$,,' | norm | {
  n=0; bad=0
  while read -r line; do
    [ -z "$line" ] && continue
    n=$((n+1))
    if ! grep -qF -- "$line" "$scratch"; then echo "NOT FOUND: $line"; bad=$((bad+1)); fi
  done
  echo "checked $n UPSTREAM lines, $bad not found"
}
