#!/bin/bash
# gh:#1866 / gh:#1361 round 33: the 2D_MSFR `steadyStateTH` case VARIANTS for `PIMPLE.nNonOrthogonalCorrectors` (onePhase `pEqn_1p.H`),
# as OVERLAY files over `cases/msfr_th` (turbulence true, tight 1e-14 solvers), in `cases/msfr_th_variants_r33/<name>/`.
# Same protocol as rounds 29 and 31: the port's tests and `run_msfr_th_tight.sh` (VARIANT_DIR=<this dir> VARIANT=<name>) read the same bytes.
# usage: make_msfr_th_nonorth_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/cases/msfr_th
OUT=$HERE/cases/msfr_th_variants_r33
rm -rf "$OUT"; mkdir -p "$OUT"

put() { mkdir -p "$OUT/$1/$(dirname "$2")"; cp "$BASE/$2" "$OUT/$1/$2"; echo "$OUT/$1/$2"; }
turb_on() { sed -i 's/turbulence      false;/turbulence      true;/' "$1"; }
base() { turb_on "$(put "$1" constant/fluidRegion/turbulenceProperties)"; put "$1" system/fluidRegion/fvSolution >/dev/null; }
fvs() { echo "$OUT/$1/system/fluidRegion/fvSolution"; }
nonorth() { sed -i "s/^\( *nNonOrthogonalCorrectors *\)0;/\1$2;/" "$(fvs "$1")"; grep -q "nNonOrthogonalCorrectors *$2;" "$(fvs "$1")"; }
pimple_add() { X="$2"$'\n' perl -0pi -e 's/(    momentumMode                cellCentered;\n)/$1$ENV{X}/' "$(fvs "$1")"; }
rc() { pimple_add "$1" "$(printf '\n    residualControl\n    {\n%s\n    }\n' "$2")"; }
finals() {
  X='
    hFinal
    {
        solver          PBiCGStab;
        preconditioner  diagonal;
        tolerance       1e-14;
        relTol          0.1;
    }

    kFinal
    {
        solver          PBiCGStab;
        preconditioner  diagonal;
        tolerance       1e-14;
        relTol          0.1;
    }

    epsilonFinal
    {
        solver          PBiCGStab;
        preconditioner  diagonal;
        tolerance       1e-14;
        relTol          0.1;
    }
' perl -0pi -e 's/(    ".\*"\n    \{.*?\n    \}\n)/$1$ENV{X}/s' "$(fvs "$1")"
}
relax() { X="$2" perl -0pi -e 's/relaxationFactors\n\{.*?\n\}\n/relaxationFactors\n{\n$ENV{X}}\n/s' "$(fvs "$1")"; }
prgh_final() { perl -0pi -e 's/(p_rghFinal\n    \{\n        \$p_rgh;\n        relTol +)0;/${1}'"$2"';/' "$(fvs "$1")"; }

# no_1, no_2: the base with 1 and 2 non-orthogonal correctors
base no_1; nonorth no_1 1
base no_2; nonorth no_2 2
# no_1_final: no_1 + a loose p_rghFinal: finalInnerIter() is true only in the LAST pass of the last corrector, so pass 1 of that corrector
# runs the tight p_rgh entry and pass 2 the loose p_rghFinal one
base no_1_final; nonorth no_1_final 1; prgh_final no_1_final 0.05
# no_rc_p: residualControl with h always met (tolerance 10) and p_rgh 0.004 on the FIRST PISO residual, distinct Final entries
base no_rc_p; nonorth no_rc_p 1; finals no_rc_p
rc no_rc_p "$(printf '        h\n        {\n            tolerance   10;\n            relTol      0;\n        }\n        p_rgh\n        {\n            tolerance   0.004;\n            relTol      0;\n            useFirstPISOInitialResidual true;\n        }')"
# no_1_fld_p: no_1 + fields { p_rgh 0.7; } : p_rgh.relax() runs in the final pass only
base no_1_fld_p; nonorth no_1_fld_p 1
relax no_1_fld_p '    fields
    {
        p_rgh           0.7;
    }
    equations
    {
        ".*"            0.9;
    }
'
# no_ulof: the PIMPLE block of Tutorials/testing/2D_MSFR_ULOF_diffusion_and_pk verbatim (nCorrectors 6, nNonOrthogonalCorrectors 1,
# partialEliminationMode implicit; nOuterCorrectors 1 and correctUntilConvergence false by default) plus the two switches the stage needs
base no_ulof
perl -0pi -e 's/PIMPLE\n\{.*?\n\}\n/PIMPLE\n{\n    solveEnergy                 true;\n    solveFluidMechanics         true;\n    nCorrectors      6;\n    nNonOrthogonalCorrectors 1;\n    partialEliminationMode      implicit;\n    momentumMode                cellCentered;\n}\n/s' "$(fvs no_ulof)"
grep -q "nNonOrthogonalCorrectors 1;" "$(fvs no_ulof)"
echo "variants in $OUT: $(ls "$OUT" | tr '\n' ' ')"
