#!/bin/bash
# gh:#1939, #1940 round 73: the one-edit variants of `cases/fullcoupling_fluid` (cell-centred, tight, round 55 protocol) that isolate a piece of the
# faceCentered branch of `onePhase` and the thermo/closure pieces the shipped 3D_SmallESFR fluid region needs:
#   f1_facecentered : the ONE edit `momentumMode faceCentered;` added to PIMPLE (UEqn_1p.H without ddt, pEqnf_1p.H, reconstructU_1p.H)
#   f2_extrap       : the ONE edit `fixedFluxPressure` -> `fixedFluxExtrapolatedPressure` on the inlet `p_rgh` (cell-centred)
#   f3_varrho       : the ONE edit `rhoCoeffs<8> (1000 0 ...)` -> `(1000 -0.2 0 ...)` (icoPolynomial, rho(T), cell-centred)
#   f4_fc_varrho    : f1 + f3 (two named edits: variable rho in the faceCentered equations)
#   f5_byregime     : TWO named edits: `regimeMapModels.lamTurb` (oneParameter on `Re`, laminar (0 20000), turbulent (30000 31000): the case's Re ~ 2.5e4 sits in
#                     the INTERPOLATION region) and `heatTransferModels.zone0` = `byRegime` (laminar: Nu = 4; turbulent: the shipped Nusselt power law)
#   f6_fc_nonorth   : f1 + `nNonOrthogonalCorrectors 1;` (two named edits)
# usage: make_round73_variants.sh <base case dir> <out dir>
BASE=$1
OUT=$2
mkdir -p "$OUT"
for v in f1_facecentered f2_extrap f3_varrho f4_fc_varrho f5_byregime f6_fc_nonorth; do
  rm -rf "$OUT/$v"
  mkdir -p "$OUT/$v"
  cp -r "$BASE/0" "$BASE/constant" "$BASE/system" "$OUT/$v/"
done
fv=system/fluidRegion/fvSolution
pp=constant/fluidRegion/phaseProperties
th=constant/fluidRegion/thermophysicalProperties
for v in f1_facecentered f4_fc_varrho f6_fc_nonorth; do
  perl -0pi -e 's/(PIMPLE\s*\{)/$1\n    momentumMode        faceCentered;/' "$OUT/$v/$fv"
done
perl -0pi -e 's/nNonOrthogonalCorrectors\s+\d+;//; s/(PIMPLE\s*\{)/$1\n    nNonOrthogonalCorrectors 1;/' "$OUT/f6_fc_nonorth/$fv"
perl -0pi -e 's/(inlet\s*\{\s*type\s+)fixedFluxPressure;/$1fixedFluxExtrapolatedPressure;/' "$OUT/f2_extrap/0/fluidRegion/p_rgh"
for v in f3_varrho f4_fc_varrho; do
  perl -0pi -e 's/rhoCoeffs<8>(\s+)\(1000 0 /rhoCoeffs<8>$1(1000 -0.2 /' "$OUT/$v/$th"
done
perl -0pi -e 's/(heatTransferModels\s*\{\s*"zone0"\s*\{)[^}]*\}/$1\n            type        byRegime;\n            regimeMap   "lamTurb";\n            "laminar"\n            {\n                type    NusseltReynoldsPrandtlPower;\n                const   4;\n                coeff   0;\n                expRe   0;\n                expPr   0;\n            }\n            "turbulent"\n            {\n                type    NusseltReynoldsPrandtlPower;\n                const   4.82;\n                coeff   0.0185;\n                expRe   0.827;\n                expPr   0.827;\n            }\n        }/' "$OUT/f5_byregime/$pp"
perl -0pi -e 's/(physicsModels\s*\{)/regimeMapModels\n{\n    "lamTurb"\n    {\n        type                oneParameter;\n        parameter           "Re";\n        regimeBounds\n        {\n            "laminar"       (0      20000);\n            "turbulent"     (30000  31000);\n        }\n    }\n}\n\n$1/' "$OUT/f5_byregime/$pp"
