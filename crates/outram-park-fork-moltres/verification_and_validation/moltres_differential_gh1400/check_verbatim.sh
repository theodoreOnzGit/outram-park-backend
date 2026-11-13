#!/bin/sh
# Checks that every line marked with MARKER in a dump driver occurs
# (whitespace normalised, trailing // comment stripped) in the pinned upstream
# source tree.
# Usage: check_verbatim.sh driver.cpp MARKER upstream_root scratch_file
#   MARKER UPSTREAM_MOLTRES -> upstream_root = <moltres clone>/src
#   MARKER UPSTREAM_MOOSE   -> upstream_root = dir holding LinearInterpolation.C
drv="$1"; marker="$2"; root="$3"; scratch="$4"
norm() { sed 's/[[:space:]]\+/ /g; s/^ //; s/ $//'; }
find "$root" \( -name '*.cc' -o -name '*.C' -o -name '*.h' -o -name '*.H' \) -print0 | xargs -0 cat | norm > "$scratch"
grep "$marker" "$drv" | sed 's,//.*$,,' | norm | {
  n=0; bad=0
  while read -r line; do
    [ -z "$line" ] && continue
    n=$((n+1))
    if ! grep -qF -- "$line" "$scratch"; then echo "NOT FOUND: $line"; bad=$((bad+1)); fi
  done
  echo "checked $n $marker lines, $bad not found"
}
