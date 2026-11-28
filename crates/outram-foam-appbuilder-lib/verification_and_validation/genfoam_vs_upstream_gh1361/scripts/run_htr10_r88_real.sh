#!/bin/bash
# gh:#1991 (README round 88): the real GeN-Foam binary on 3D_HTR-10 with the SHIPPED `corrected` Laplacian and snGrad.
# usage: run_htr10_r88_real.sh <scratch run dir> <variant>
#   ct2   the round-87 tight case (refs/htr10_r87/case) with the shipped schemes restored, endTime 0.5 (one step)
#   clam  ct2 with `simulationType laminar`
#   ct3   the round-87 tight case with the shipped schemes restored, endTime 0.55 (7 steps), purgeWrite 1
#   sh20  the unedited shipped rootCase (shipped fvSolution, fvSchemes) with endTime 0.645 (20 steps), writeControl timeStep,
#         writeInterval 1, purgeWrite 1, writePrecision 17: only the output and the end time differ from the shipped run
# The run directory is left in place for make_htr10_r88_refs.py; delete it afterwards.
D=$1; V=$2
S=$(dirname "$(readlink -f "$0")")
REFS=$(readlink -f "$S/../refs/htr10_r87")
ROOT=/home/user/upstream-build/GeN-Foam-src/Tutorials/reactorCases/3D_HTR-10/rootCase
W=/home/user/upstream-build/scripts/with-of.sh
export LD_LIBRARY_PATH=/home/user/upstream-build/gh1781/lib:$LD_LIBRARY_PATH
rm -rf "$D"; mkdir -p "$D"
if [ "$V" = sh20 ]; then
  cp -r "$ROOT/0" "$ROOT/constant" "$ROOT/system" "$D/"
  sed -i "s/^endTime .*/endTime         0.645;/; s/^writeControl .*/writeControl    timeStep;/; s/^writeInterval .*/writeInterval   1;/; s/^purgeWrite .*/purgeWrite      1;/; s/^writePrecision .*/writePrecision  17;/" "$D/system/controlDict"
else
  cp -r "$REFS/case/." "$D/"
  find "$D" -name "*.gz" -exec gunzip {} +
  F=$D/system/fluidRegion/fvSchemes
  sed -i "s/Gauss linear uncorrected;/Gauss linear corrected;/; s/^\(    default  *\)uncorrected;/\1corrected;/" "$F"
  if ! grep -q "Gauss linear corrected;" "$F" || ! grep -q "default         corrected;" "$F"; then echo "fvSchemes edit failed"; exit 1; fi
  case "$V" in
    ct2|clam) sed -i "s/^endTime .*/endTime         0.5;/" "$D/system/controlDict" ;;
    ct3) ;;
    *) echo "unknown variant $V"; exit 1 ;;
  esac
  if [ "$V" = clam ]; then
    printf 'FoamFile\n{\n    version 2.0;\n    format ascii;\n    class dictionary;\n    object turbulenceProperties;\n}\nsimulationType laminar;\n' \
      > "$D/constant/fluidRegion/turbulenceProperties"
  fi
fi
cd "$D" && /usr/bin/time -v $W GeN-Foam -case "$D" > log.GeN-Foam 2> time.txt
tail -3 log.GeN-Foam
