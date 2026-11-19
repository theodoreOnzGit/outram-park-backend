#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# Proves the transcribed statements of linear_kernel.F90 are present, byte for
# byte (after the documented this%X substitutions), in PFLOTRAN 669fbce
# characteristic_curves_common.F90.   usage: check_verbatim_linear.sh <file>
# <file> is src/pflotran/characteristic_curves_common.F90 at 669fbce (a clone
# checkout, or the bitbucket raw URL of that commit saved to disk).
set -euo pipefail
f=${1:?characteristic_curves_common.F90 at 669fbce}
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
sed -n '2737,2841p;5191,5228p' "$f" | sed 's/^ *//' > "$tmp/up"
# transcribed statements = the lines of linear_kernel.F90 between `contains`
# and `end program`, minus declarations, subroutine headers and blanks.
awk '/^contains/{on=1;next} /^end program/{on=0} on' "$here/linear_kernel.F90" |
  sed 's/^ *//' | grep -v -E '^(double precision|subroutine|end subroutine|$)' |
  sed -e 's/\bSr\b/this%Sr/g' -e 's/\balpha\b/this%alpha/g' -e 's/\bpcmax\b/this%pcmax/g' > "$tmp/tr"
miss=0; n=0
while IFS= read -r line; do
  n=$((n+1))
  if ! grep -q -x -F -- "$line" "$tmp/up"; then echo "NOT VERBATIM: $line"; miss=$((miss+1)); fi
done < "$tmp/tr"
rm -rf "$tmp"
echo "checked $n transcribed statements, $miss not found in upstream"
test "$miss" -eq 0
