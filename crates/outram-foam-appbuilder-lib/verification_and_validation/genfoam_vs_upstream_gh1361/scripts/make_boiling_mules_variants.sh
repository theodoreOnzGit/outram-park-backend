#!/bin/bash
# gh:#1799 / gh:#1361 round 34: VARIANTS of the shipped `1D_boiling` that select the `solveAlpha` lambda of `solveAlphaMULES.H`
# (`solvePhase <fluid>`) and a non-uniform initial `alpha.vapour`, as OVERLAY files over `tests/data/genfoam_1d_boiling`, in
# `cases/boiling_mules_r34/<name>/` (files: `0/fluidRegion/alpha.{vapour,liquid}` and `system/fluidRegion/fvSolution`). The port's
# tests and `boiling_mules_variant_protocol.sh` (VARIANT=<name>) read the SAME bytes.
#
# WHY THESE OPTIONS (upstream's own, never chosen from port results):
#  * `solvePhase "vapour"` is what `featureCases/1D_HX/twoPhase` and `2D_KNS37-L22` select (`alphaDict.lookupOrDefault<word>("solvePhase")`,
#    solveAlphaMULES.H:418). Those two cases need heat exchangers / heated pins, so the lambda path is compared on the case the port runs.
#    `solvePhase "liquid"` is the `fluid1_.name()` branch of the same `solve` lambda (solveAlphaMULES.H:371 vs :380).
#  * `alphaMaxCo 0.05` (a `fvSolution` `alpha` entry, solveAlphaMULES.H:430): `nSubCycles = min(ceil((CoNum_+SMALL)/alphaMaxCo), 50)` when
#    `adjustSubCycles` and both phases are present. The shipped 0.25 never gives 2 sub-cycles before the flow excursion (Co = U dt/dx = 0.2 at the
#    capped dt 0.01, U = 1 m/s, dx = 0.05); 0.05 gives 1 sub-cycle at dt = 1e-3 (Co = 0.02), 2 from dt ~ 2.5e-3, and 4 at the capped dt.
#  * a non-uniform `internalField nonuniform List<scalar> 40 (...)` in `alpha.vapour` (standard OpenFOAM field syntax): `bothPhasesArePresent_`
#    (`max(alpha1 alpha2) >= 1e-4`, twoPhase.C) is then true from step 1, so sub-cycling is active from the first step. The profile is a smooth
#    bump, alpha_i = 0.1 sin^2(pi (i - 9)/12) for cells i = 9..21 (zero at both ends, peak 0.1 in cell 15), 0 elsewhere, written with 17 digits;
#    alpha.liquid = 1 - alpha.vapour (twoPhase.C then normalises both to `movingAlpha`). Cells 0-8 and 22-39 stay pure liquid at t = 0.
# Variants:
#   a_sp_vap_uniform  solvePhase vapour, alpha.vapour uniform 0 (the shipped initial state), alphaMaxCo 0.25 as shipped
#   b_both_bump_sub   solvePhase both (default), bump, alphaMaxCo 0.05      (the existing `solveAlphas` path with sub-cycles > 1 from step ~4)
#   c_vap_bump_sub    solvePhase vapour, bump, alphaMaxCo 0.05
#   d_liq_bump_sub    solvePhase liquid, bump, alphaMaxCo 0.05
#   e_vap_bump_nosub  solvePhase vapour, bump, alphaMaxCo 0.25 (1 sub-cycle: the lambda path without sub-cycling)
# usage: make_boiling_mules_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/../../tests/data/genfoam_1d_boiling
OUT=$HERE/cases/boiling_mules_r34
rm -rf "$OUT"; mkdir -p "$OUT"

bump() { # $1 = alpha.vapour | alpha.liquid
  awk -v which="$1" 'function s2(i,  t){ t=sin(3.14159265358979323846*(i-9)/12); return 0.1*t*t }
    BEGIN{ for(i=0;i<40;i++){ v=(i>=9&&i<=21)?s2(i):0; if(i==9||i==21) v=0; vals[i]=(which=="alpha.vapour")?v:1-v } }
    /^internalField/{ print "internalField   nonuniform List<scalar> 40"; print "("; for(i=0;i<40;i++) printf "%.17g\n", vals[i]; print ")"; print ";"; next }
    { print }' "$BASE/0/fluidRegion/$1"
}
# alpha.liquid patches carry `value $internalField`, which would expand the whole list: give the (calculated) patches their value explicitly
fix_liquid() { sed 's/value           \$internalField;/value           uniform 1;/'; }

mk() { # name, solvePhase word or "", alphaMaxCo, bump (yes|no)
  d=$OUT/$1; mkdir -p "$d/0/fluidRegion" "$d/system/fluidRegion"
  if [ "$4" = yes ]; then
    bump alpha.vapour > "$d/0/fluidRegion/alpha.vapour"
    bump alpha.liquid | fix_liquid > "$d/0/fluidRegion/alpha.liquid"
  fi
  awk -v sp="$2" -v co="$3" '/alphaMaxCo/{ print "        alphaMaxCo      " co ";"; if (sp != "") print "        solvePhase      \"" sp "\";"; next } { print }' \
    "$BASE/system/fluidRegion/fvSolution" > "$d/system/fluidRegion/fvSolution"
}
mk a_sp_vap_uniform vapour 0.25 no
mk b_both_bump_sub  ""     0.05 yes
mk c_vap_bump_sub   vapour 0.05 yes
mk d_liq_bump_sub   liquid 0.05 yes
mk e_vap_bump_nosub vapour 0.25 yes
find "$OUT" -type f | sort
