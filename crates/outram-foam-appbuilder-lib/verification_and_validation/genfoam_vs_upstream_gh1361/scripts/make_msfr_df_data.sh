#!/bin/bash
# gh:#1893 (README round 43): cases/msfr_neutro/nuclearData.df = the committed 2D_MSFR nuclearData with
# `useGivenDiscFactors true` and a NON-UNIT `discFactor` in every zone and group. The numbers are NOT physical: they are
# a deterministic input of both codes, df[zone][group] = 1 + 0.02 * (((3 zone + 2 group) mod 5) - 2) (0.96 to 1.04),
# the same in the three states (reference, TFuel1500K, rhoCool3419kgm3: 4 zones each) so that no interpolation between
# states changes them. Zone = the order of the `discFactor` lines inside a state (reflector, blanket, ... as the file).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
IN="$HERE/cases/msfr_neutro/nuclearData"
OUT="$HERE/cases/msfr_neutro/nuclearData.df"
awk '
  /^useGivenDiscFactors/ { print "useGivenDiscFactors     true;"; next }
  /discFactor nonuniform List<scalar> 6/ {
    z = n % 4; n++
    line = "                discFactor nonuniform List<scalar> 6 ("
    for (g = 0; g < 6; g++) {
      m = (3 * z + 2 * g) % 5
      line = line sprintf("%.2f ", 1 + 0.02 * (m - 2))
    }
    print line " );"
    next
  }
  { print }
' "$IN" > "$OUT"
grep -c 'discFactor' "$OUT"
