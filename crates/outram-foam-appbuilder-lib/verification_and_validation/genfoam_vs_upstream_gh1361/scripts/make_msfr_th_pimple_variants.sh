#!/bin/bash
# gh:#1855 / gh:#1361 round 31: the 2D_MSFR `steadyStateTH` case VARIANTS for `PIMPLE.residualControl` (GeN-Foam's customPimpleControl),
# the `Final` solver / relaxation entries it selects, and the `relaxationFactors` `default` entry (equations and fields), as OVERLAY files
# over `cases/msfr_th` (turbulence true, mu 0.01 as shipped, tight 1e-14 solvers), in `cases/msfr_th_variants_r31/<name>/` (a directory
# of its own: `make_msfr_th_variants.sh` of round 29 wipes `cases/msfr_th_variants`). Same protocol as round 29: the port's tests and
# `run_msfr_th_tight.sh` (VARIANT_DIR=<this dir> VARIANT=<name>) read the same bytes.
#
# THRESHOLDS ARE DERIVED FROM UPSTREAM'S LOGGED INITIAL RESIDUALS of the base run (turbulence on, no residualControl; round 29's
# refs/msfr_th_opt_fin_norelax.ref.txt.gz, written before any residualControl existed), step 1:
#   outer           1         2         3         4          5
#   h             0.99999   0.99995   0.21805   0.089539   0.039031
#   p_rgh 1st PISO 0.98363  0.013334  0.0032098 0.0013982  0.00039476
#   p_rgh last     8.3e-4   2.1e-4    7.4e-5    3.6e-5     1.5e-5
#   k             1.0       0.34440   0.13853   0.016127   0.0062832
#   epsilon       1.5e-5    0.10990   0.058826  0.0053280  0.0021400
# usage: make_msfr_th_pimple_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/cases/msfr_th
OUT=$HERE/cases/msfr_th_variants_r31
rm -rf "$OUT"; mkdir -p "$OUT"

put() { mkdir -p "$OUT/$1/$(dirname "$2")"; cp "$BASE/$2" "$OUT/$1/$2"; echo "$OUT/$1/$2"; }
turb_on() { sed -i 's/turbulence      false;/turbulence      true;/' "$1"; }
base() { turb_on "$(put "$1" constant/fluidRegion/turbulenceProperties)"; put "$1" system/fluidRegion/fvSolution >/dev/null; }
fvs() { echo "$OUT/$1/system/fluidRegion/fvSolution"; }

# insert text after the last PIMPLE entry (`momentumMode cellCentered;`)
pimple_add() { X="$2"$'\n' perl -0pi -e 's/(    momentumMode                cellCentered;\n)/$1$ENV{X}/' "$(fvs "$1")"; }
# residualControl: <variant> <body of the block, one entry per line>
rc() { pimple_add "$1" "$(printf '\n    residualControl\n    {\n%s\n    }\n' "$2")"; }
# distinct Final solver entries (PBiCGStab/diagonal, relTol 0.1: visibly fewer iterations) and Final equation relaxation factors
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
  perl -0pi -e 's/(\"\.\*\"            0\.9;)/$1\n        hFinal          0.6;\n        UFinal          0.7;\n        kFinal          0.5;\n        epsilonFinal    0.5;/' "$(fvs "$1")"
}
# replace the whole relaxationFactors block
relax() { X="$2" perl -0pi -e 's/relaxationFactors\n\{.*?\n\}\n/relaxationFactors\n{\n$ENV{X}}\n/s' "$(fvs "$1")"; }
# p_rghFinal relTol
prgh_final() { perl -0pi -e 's/(p_rghFinal\n    \{\n        \$p_rgh;\n        relTol +)0;/${1}'"$2"';/' "$(fvs "$1")"; }

HQ='        h { tolerance 0.25; relTol 0; }'

# ---- residualControl (every one with distinct Final entries) --------------------------------------------------------------------
# rc_a: step 1 criteria first met at the start of outer iteration 4 (h(3) = 0.218 < 0.25 and first-PISO p_rgh(3) = 0.0032 < 0.004)
base rc_a; finals rc_a
rc rc_a "$(printf '        h\n        {\n            tolerance   0.25;\n            relTol      0;\n        }\n        p_rgh\n        {\n            tolerance   0.004;\n            relTol      0;\n            useFirstPISOInitialResidual true;\n        }')"
# rc_b: first met at the start of outer iteration 5 = nOuterCorrectors (h(4) = 0.0895 < 0.1, p(4) = 0.0013982 < 0.0015): the stale-flag case
base rc_b; finals rc_b
rc rc_b "$(printf '        h\n        {\n            tolerance   0.1;\n            relTol      0;\n        }\n        p_rgh\n        {\n            tolerance   0.0015;\n            relTol      0;\n            useFirstPISOInitialResidual true;\n        }')"
# rc_c: never met (control: residualControl present, never converges, so no Final is ever selected: the unconverged run of the base)
base rc_c; finals rc_c
rc rc_c "$(printf '        h\n        {\n            tolerance   1e-3;\n            relTol      0;\n        }\n        p_rgh\n        {\n            tolerance   1e-6;\n            relTol      0;\n            useFirstPISOInitialResidual true;\n        }')"
# rc_d: four fields, p_rgh WITHOUT useFirstPISOInitialResidual (the last corrector's residual), relTol 0.5 (inert)
base rc_d; finals rc_d
rc rc_d "$(printf '        h\n        {\n            tolerance   0.25;\n            relTol      0.5;\n        }\n        p_rgh\n        {\n            tolerance   5e-4;\n            relTol      0.5;\n        }\n        k\n        {\n            tolerance   0.2;\n            relTol      0.5;\n        }\n        epsilon\n        {\n            tolerance   0.07;\n            relTol      0.5;\n        }')"
# rc_rel: relTol only (absolute tolerances unreachable): `residualControl_[f].initialResidual` stays 0, so relTol never fires
base rc_rel; finals rc_rel
rc rc_rel "$(printf '        h\n        {\n            tolerance   1e-30;\n            relTol      0.99;\n        }\n        p_rgh\n        {\n            tolerance   1e-30;\n            relTol      0.99;\n        }')"
# rc_stop (stopIfFirstResRelChangeAboveRelTol) is NOT a variant: upstream aborts with SIGFPE at outer iteration 2 of step 1 (first-PISO residual pair (0, r1): r1/0), see README round 31
# rc_fin_prgh: rc_a with a distinct p_rghFinal and finalOnLastPimpleIterOnly (p_rghFinal only in the flagged iteration)
base rc_fin_prgh; finals rc_fin_prgh; prgh_final rc_fin_prgh 0.05
rc rc_fin_prgh "$(printf '        h\n        {\n            tolerance   0.25;\n            relTol      0;\n        }\n        p_rgh\n        {\n            tolerance   0.004;\n            relTol      0;\n            useFirstPISOInitialResidual true;\n        }')"
pimple_add rc_fin_prgh "    finalOnLastPimpleIterOnly   true;
"
# fin_prgh: no residualControl, distinct p_rghFinal: used at the last corrector of EVERY outer iteration (finalInnerIter())
base fin_prgh; prgh_final fin_prgh 0.05

# ---- relaxationFactors default ----------------------------------------------------------------------------------------------------
# rel_def: equations { default 0.9; } and nothing else: numerically the base case (".*" 0.9)
base rel_def; relax rel_def '    equations
    {
        default         0.9;
    }
'
# rel_mix: default 0.8, h 0.6 (exact), U.* 0.7 (pattern)
base rel_mix; relax rel_mix '    equations
    {
        default         0.8;
        h               0.6;
        "U.*"           0.7;
    }
'
# rel_fin: rel_mix + rc_a thresholds + Final solver entries: in the flagged iteration `hFinal` is not a key, so `default` (0.8) is used, not h 0.6
base rel_fin; finals rel_fin; relax rel_fin '    equations
    {
        default         0.8;
        h               0.6;
        "U.*"           0.7;
    }
'
rc rel_fin "$(printf '        h\n        {\n            tolerance   0.25;\n            relTol      0;\n        }\n        p_rgh\n        {\n            tolerance   0.004;\n            relTol      0;\n            useFirstPISOInitialResidual true;\n        }')"
# fld_def: fields { default 0.8; } : he, p_rgh, Kd, htc are relaxed
base fld_def; relax fld_def '    fields
    {
        default         0.8;
    }
    equations
    {
        ".*"            0.9;
    }
'
# fld_mix: fields { p_rgh 0.7; h 0.85; } : Kd, htc are not relaxed
base fld_mix; relax fld_mix '    fields
    {
        p_rgh           0.7;
        h               0.85;
    }
    equations
    {
        ".*"            0.9;
    }
'
# fld_h: fields { h 0.85; } only (bisect of the fields variants)
base fld_h; relax fld_h '    fields
    {
        h               0.85;
    }
    equations
    {
        ".*"            0.9;
    }
'
# fld_p: fields { p_rgh 0.7; } only (bisect of the fields variants)
base fld_p; relax fld_p '    fields
    {
        p_rgh           0.7;
    }
    equations
    {
        ".*"            0.9;
    }
'
# fld_kd: fields { Kd 0.6; } only (bisect of the fields variants)
base fld_kd; relax fld_kd '    fields
    {
        Kd              0.6;
    }
    equations
    {
        ".*"            0.9;
    }
'
# fld_htc: fields { htc 0.6; } only (bisect of the fields variants)
base fld_htc; relax fld_htc '    fields
    {
        htc             0.6;
    }
    equations
    {
        ".*"            0.9;
    }
'
echo "variants written to $OUT:"; ls "$OUT"
