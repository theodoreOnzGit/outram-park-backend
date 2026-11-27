#!/bin/bash
# gh:#1842 / gh:#1361 round 29: the 2D_MSFR `steadyStateTH` case VARIANTS no shipped case exercises, as OVERLAY files over
# `cases/msfr_th` (the TURB=off root case of rounds 19-21). One overlay dir per variant under `cases/msfr_th_variants/<name>/`
# holds ONLY the files the variant changes, in the layout of the case (`0/fluidRegion/..`, `constant/fluidRegion/..`,
# `system/fluidRegion/..`). BOTH the port's tests (`tests/genfoam_msfr_th_options_vs_upstream.rs`) and `run_msfr_th_tight.sh`
# (env VARIANT=<name>) copy the overlay over the same base, so the two codes read the same bytes.
# usage: make_msfr_th_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/cases/msfr_th
OUT=$HERE/cases/msfr_th_variants
rm -rf "$OUT"; mkdir -p "$OUT"
PATCHES="topwall bottomwall reflector hx"

# put <variant> <relative file> : copy the base file into the overlay and return its path
put() { mkdir -p "$OUT/$1/$(dirname "$2")"; cp "$BASE/$2" "$OUT/$1/$2"; echo "$OUT/$1/$2"; }
addp() { # file patch text
  P="$2" X="$3" perl -0pi -e 's/(\n\s*\b$ENV{P}\b\s*\{[^}]*?type\s+\w+;)/$1$ENV{X}/' "$1"
}
turb_on() { sed -i 's/turbulence      false;/turbulence      true;/' "$1"; }
# every variant: turbulence true
base_turb() { turb_on "$(put "$1" constant/fluidRegion/turbulenceProperties)"; }
# wall-function variants run at mu 0.5 (see README round 29, "Why mu 0.5")
mu05() { sed -i 's/^\( *mu  *\)0.01;/\10.5;/' "$(put "$1" constant/fluidRegion/thermophysicalProperties)"; }

wf() { # name
  base_turb "$1"; mu05 "$1"
  put "$1" 0/fluidRegion/nut >/dev/null; put "$1" 0/fluidRegion/epsilon >/dev/null
}
nutf() { echo "$OUT/$1/0/fluidRegion/nut"; }
epsf() { echo "$OUT/$1/0/fluidRegion/epsilon"; }
nl() { printf '\n        %s' "$@"; }

# 0. base at mu 0.5, every option at its default
wf wf_base
# 1. lowReCorrection (epsilonWallFunction only), per patch; `off` written out on bottomwall, hx left at the default
wf wf_lowre
addp "$(epsf wf_lowre)" topwall "$(nl 'lowReCorrection on;')"
addp "$(epsf wf_lowre)" bottomwall "$(nl 'lowReCorrection off;')"
addp "$(epsf wf_lowre)" reflector "$(nl 'lowReCorrection yes;')"
# 2-5. the four other blenders on every patch of nut and epsilon
for b in max exponential tanh; do
  wf wf_$b
  for p in $PATCHES; do
    addp "$(nutf wf_$b)" $p "$(nl "blending $b;")"; addp "$(epsf wf_$b)" $p "$(nl "blending $b;")"
  done
done
wf wf_binomial
for p in $PATCHES; do
  addp "$(nutf wf_binomial)" $p "$(nl 'blending binomial;')"; addp "$(epsf wf_binomial)" $p "$(nl 'blending binomial;')"
done
addp "$(nutf wf_binomial)" topwall "$(nl 'n 3;')"       # nut default n is 4
addp "$(epsf wf_binomial)" bottomwall "$(nl 'n 3;')"    # epsilon default n is 2
# 6. a different blender on every patch, nut and epsilon independently, with n and lowReCorrection
wf wf_mixed
addp "$(nutf wf_mixed)" topwall "$(nl 'blending binomial;' 'n 2.5;')"
addp "$(nutf wf_mixed)" bottomwall "$(nl 'blending max;')"
addp "$(nutf wf_mixed)" reflector "$(nl 'blending exponential;')"
addp "$(nutf wf_mixed)" hx "$(nl 'blending tanh;')"
addp "$(epsf wf_mixed)" topwall "$(nl 'blending tanh;')"
addp "$(epsf wf_mixed)" bottomwall "$(nl 'blending binomial;')"
addp "$(epsf wf_mixed)" reflector "$(nl 'blending stepwise;' 'lowReCorrection on;')"
addp "$(epsf wf_mixed)" hx "$(nl 'blending exponential;')"
# 7. wallFunctionCoefficients: Cmu 0.0845 (RNGkEpsilon default, ESI v2506), kappa 0.4187 and E 9.793 (Launder & Spalding 1974)
wf wf_coeffs
for p in $PATCHES; do addp "$(nutf wf_coeffs)" $p "$(nl 'Cmu 0.0845;' 'kappa 0.4187;' 'E 9.793;')"; done
addp "$(epsf wf_coeffs)" topwall "$(nl 'kappa 0.4187;')"
addp "$(epsf wf_coeffs)" bottomwall "$(nl 'Cmu 0.0845;')"
addp "$(epsf wf_coeffs)" reflector "$(nl 'E 9.793;' 'lowReCorrection on;')"
addp "$(epsf wf_coeffs)" hx "$(nl 'Cmu 0.0845;' 'kappa 0.4187;' 'E 9.793;')"
# 8. nutWallFunction `U` (read by `nutkWallFunction::yPlus()` only): a name that does not exist as a field, and `U`
wf wf_uname
addp "$(nutf wf_uname)" topwall "$(nl 'U UNoSuchField;')"
addp "$(nutf wf_uname)" bottomwall "$(nl 'U U;')"

# DhStruct / laminarReStruct (shipped mu 0.01)
dh() { base_turb "$1"; f=$(put "$1" constant/fluidRegion/turbulenceProperties)
       turb_on "$f"; perl -0pi -e "s/(convergenceLength\s+0.5;)/\$1\n        DhStruct                    0.01;$2/" "$f"; }
dh dh_a ""
dh dh_b "\n        laminarReStruct             50;"

# Final solver / relaxation entries (shipped mu 0.01)
fin() { base_turb "$1"; }
fin fin_solver
f=$(put fin_solver system/fluidRegion/fvSolution)
perl -0pi -e 's/(    "\.\*"\n    \{.*?\n    \}\n)/$1\n    k\n    {\n        solver          PBiCGStab;\n        preconditioner  diagonal;\n        tolerance       1e-14;\n        relTol          0.01;\n    }\n\n    epsilon\n    {\n        solver          PBiCGStab;\n        preconditioner  diagonal;\n        tolerance       1e-14;\n        relTol          0.01;\n    }\n\n    kFinal\n    {\n        solver          PBiCGStab;\n        preconditioner  diagonal;\n        tolerance       1e-14;\n        relTol          0;\n    }\n\n    epsilonFinal\n    {\n        solver          PBiCGStab;\n        preconditioner  diagonal;\n        tolerance       1e-14;\n        relTol          0;\n    }\n/s' "$f"
fin fin_relax
f=$(put fin_relax system/fluidRegion/fvSolution)
perl -0pi -e 's/(\"\.\*\"            0\.9;)/$1\n        kFinal          0.6;\n        epsilonFinal    0.6;/' "$f"
fin fin_norelax
f=$(put fin_norelax system/fluidRegion/fvSolution)
perl -0pi -e 's/\"\.\*\"            0\.9;/"h.*"            0.9;\n        "U.*"            0.9;\n        k               0.9;\n        epsilon         0.9;/' "$f"
echo "variants written to $OUT:"; ls "$OUT"
