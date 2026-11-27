#!/bin/bash
# gh:#1913 / gh:#1361 round 55: the three one-edit variants of `cases/fullcoupling_fluid` that isolate a piece of rows 1-5:
#   v1_nopassive : `passiveProperties` removed from structureProperties.zone0 (isolates the passive structure)
#   v2_consthtc  : heatTransferModels.zone0 = `type constant; value 25000;` (isolates NusseltReynoldsPrandtlPower)
#   v3_smooth    : explicit `h` and `hFinal` entries AFTER the catch-all, `smoothSolver`/`symGaussSeidel` at 1e-14, relTol 0
#                  (the only way this tutorial can select the translated smoothSolver: the shipped `"h.*"` entry is shadowed by `".*"`)
# usage: make_fullcoupling_fluid_variants.sh <base case dir> <out dir>   (the variant dirs are made inside <out dir>)
BASE=$1
OUT=$2
mkdir -p "$OUT"
for v in v1_nopassive v2_consthtc v3_smooth; do
  rm -rf "$OUT/$v"
  mkdir -p "$OUT/$v"
  cp -r "$BASE/0" "$BASE/constant" "$BASE/system" "$OUT/$v/"
done
pp=constant/fluidRegion/phaseProperties
perl -0pi -e 's/\n\s*passiveProperties\s*\{[^}]*\}//' "$OUT/v1_nopassive/$pp"
perl -0pi -e 's/(heatTransferModels\s*\{\s*"zone0"\s*\{)[^}]*\}/$1\n            type    constant;\n            value   25000;\n        }/' "$OUT/v2_consthtc/$pp"
fv=system/fluidRegion/fvSolution
perl -0pi -e 's/(    "\.\*"\s*\{[^}]*\}\n)/$1    h\n    {\n        solver          smoothSolver;\n        smoother        symGaussSeidel;\n        tolerance       1e-14;\n        relTol          0;\n    }\n    hFinal\n    {\n        solver          smoothSolver;\n        smoother        symGaussSeidel;\n        tolerance       1e-14;\n        relTol          0;\n    }\n/' "$OUT/v3_smooth/$fv"
