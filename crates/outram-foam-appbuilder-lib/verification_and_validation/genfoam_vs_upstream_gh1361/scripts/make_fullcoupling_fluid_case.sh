#!/bin/bash
# gh:#1913 / gh:#1361 round 55: stage the FLUID-ONLY variant of the shipped Tutorials/featureCases/2D_fullCoupling `fluidRegion`
# (onePhase, cellCentered, solveFluidMechanics true, `laminar`, icoPolynomial/hPolynomial/polynomial sodium,
# NusseltReynoldsPrandtlPower, passiveProperties, nuclearFuelPin) so that the REAL GeN-Foam binary and the port read THE SAME directory.
# usage: make_fullcoupling_fluid_case.sh <out case dir> [POWER_FILE]
#   POWER_FILE: the prescribed `powerDensityNeutronics` (default: the field the real coupled steady state wrote at t = 100,
#   /home/user/upstream-build/gh1912/fcase/steadyState/100/fluidRegion/powerDensityNeutronics; 144 values, 7 significant digits).
# The ONLY edits to the shipped files (each an edit Allrun or the round-18/19 tight protocol makes, or the fluid-only reduction):
#   controlDict: regionSolvers = fluidRegion only; removeBaffles fluidRegion false (the mesh has no baffle); writeControl timeStep,
#                writeInterval 1, writePrecision 17 (every step, full precision); endTime/deltaT as shipped (100 / 1) unless END_TIME is set
#   fvSolution : every tolerance 1e-14, relTol 0; p_rgh/p_rghFinal GAMG -> PCG/DIC (the named deviation of rounds 17/21/44)
#   0/fluidRegion/powerDensityNeutronics: ADDED (upstream's `READ_IF_PRESENT` field; there is no neutronics in a fluid-only run)
# Dropped: neutroRegion, thermoMechanicalRegion (0, constant, system), 0/uniform, the multiRegionCouplingDict files.
SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/featureCases/2D_fullCoupling/rootCase
OUT=$1
POWER=${2:-/home/user/upstream-build/gh1912/fcase/steadyState/100/fluidRegion/powerDensityNeutronics}
rm -rf "$OUT"
mkdir -p "$OUT/0" "$OUT/constant" "$OUT/system"
cp -r "$SRC/0/fluidRegion" "$OUT/0/fluidRegion"
cp -r "$SRC/constant/fluidRegion" "$OUT/constant/fluidRegion"
cp -r "$SRC/system/fluidRegion" "$OUT/system/fluidRegion"
cp "$SRC/system/controlDict" "$OUT/system/controlDict"
# the prescribed power field: the internal list verbatim, zeroGradient on the open patches, empty on the 2D patch
{
  sed -n '1,/^internalField/p' "$POWER" | sed 's/^    location .*//'
  sed -n '/^internalField/,$p' "$POWER" | sed '1d' | sed -n '1,/^)/p'
  echo ";"
  cat <<'EOT'

boundaryField
{
    inlet
    {
        type            zeroGradient;
    }
    outlet
    {
        type            zeroGradient;
    }
    fixedWalls
    {
        type            zeroGradient;
    }
    frontAndBack
    {
        type            empty;
    }
}

// ************************************************************************* //
EOT
} > "$OUT/0/fluidRegion/powerDensityNeutronics"
C=$OUT
ctl() { sed -i "s/^$2 .*/$2 $3;/" "$1/system/controlDict"; }
blk() { perl -0pi -e "s/^$2\s*\{[^}]*\}/$2\n{\n$3\n}/m" "$1/system/controlDict"; }
[ -n "${END_TIME:-}" ] && ctl $C endTime "$END_TIME"
ctl $C writeControl timeStep
ctl $C writeInterval 1
ctl $C writePrecision 17
blk $C regionSolvers "    fluidRegion     onePhase;"
blk $C removeBaffles "    fluidRegion     false;"
f=$C/system/fluidRegion/fvSolution
sed -i -E 's/^( *)tolerance +[0-9.e+-]+; *$/\1tolerance        1e-14;/; s/^( *)relTol +[0-9.e+-]+; *(\/\/.*)?$/\1relTol           0;/' $f
perl -0pi -e 's/solver\s+GAMG;\s*smoother\s+DIC;/solver          PCG;\n        preconditioner  DIC;/g' $f
# a fluid-only run has nothing to map: upstream still reads the file
cat > $C/constant/multiRegionCouplingDict <<'EOT'
FoamFile
{
    version     2.0;
    format      ascii;
    class       dictionary;
    object      multiRegionCouplingDict;
}

mappings
{
    fluidRegion
    {
    }
}
EOT
