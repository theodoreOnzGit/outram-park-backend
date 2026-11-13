#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
# Proves the transcribed kernel statements in isotherm_kernel.F90 are present,
# byte for byte (after the two documented substitutions), in PFLOTRAN 669fbce
# reaction_isotherm.F90 lines 326-343.   usage: check_verbatim.sh <clone dir>
set -euo pipefail
UP=${1:?clone dir at 669fbce}
f=$UP/src/pflotran/reaction_isotherm.F90
here=$(cd "$(dirname "$0")" && pwd)
test "$(git -C "$UP" rev-parse HEAD)" = 669fbce204af39428020c6b9d74beae363376689
tmp=$(mktemp -d)
sed -n '326,343p' "$f" | sed 's/^ *//' > "$tmp/up"
# statements of the kernel: assignments whose left side is a kernel variable,
# plus the two continuation lines; loop/shim assignments are excluded.
grep -E '^ +(res|dres_dc|tempreal|one_over_n|molality_one_over_n) *=|^ +\(1\.d0 \+ tempreal\)|^ +res / \(1\.d0' \
  "$here/isotherm_kernel.F90" | sed 's/^ *//' |
  sed -e 's/langmuir_b/isotherm_rxn%eqisothermlangmuirb(irxn)/' \
      -e 's/freundlich_n/isotherm_rxn%eqisothermfreundlichn(irxn)/' > "$tmp/tr"
miss=0; n=0
while IFS= read -r line; do
  n=$((n+1))
  if ! grep -q -x -F -- "$line" "$tmp/up"; then echo "NOT VERBATIM: $line"; miss=$((miss+1)); fi
done < "$tmp/tr"
rm -rf "$tmp"
echo "checked $n transcribed statements, $miss not found in upstream"
test "$miss" -eq 0
