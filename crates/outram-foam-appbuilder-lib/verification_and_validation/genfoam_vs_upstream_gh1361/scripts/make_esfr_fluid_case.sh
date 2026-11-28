#!/bin/bash
# gh:#1911 round 71: stage the FLUID-ONLY variant of the shipped Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver
# `fluidRegion` (onePhase, `momentumMode faceCentered`, `laminar`, icoPolynomial/hPolynomial/polynomial sodium, byRegime Nusselt,
# slip walls, fixedFluxExtrapolatedPressure inlet, passiveProperties, `nuclearFuelPin` (innerCore) and `nuclearFuelPinTest` (outerCore))
# so that the REAL GeN-Foam binary and the port read THE SAME directory.   usage: make_esfr_fluid_case.sh <out case dir>
# Edits to the shipped files (each is the fluid-only reduction or a prescribed input, none is a fit):
#   controlDict : regionSolvers = fluidRegion only; removeBaffles fluidRegion false (the solved mesh keeps its baffle patches either way:
#                 README round 62 finding 4); endTime 3 (the runner sets the real one)
#   0/fluidRegion/powerDensityNeutronics: DELETED (shipped: `uniform 0`, which has priority over the dictionary value; there is no neutronics here)
#   phaseProperties: the two `powerDensity 0;` of the pin models -> innerCore 1e8, outerCore 8e7 W/m^3 (PRESCRIBED, order of an SFR core mean
#                 power density, chosen BEFORE any port run; they are inputs of both codes, not a fit)
#   constant/multiRegionCouplingDict: an empty `fluidRegion` mapping (upstream still reads the file); regionProperties: fluid only
# Dropped: neutroRegion, thermoMechanicalRegion, 0/uniform.
SRC=${GENFOAM_UPSTREAM:-/home/user/upstream-build/GeN-Foam-src}/Tutorials/reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver
OUT=$1
rm -rf "$OUT"
mkdir -p "$OUT/0" "$OUT/constant" "$OUT/system"
cp -r "$SRC/0/fluidRegion" "$OUT/0/fluidRegion"
cp -r "$SRC/constant/fluidRegion" "$OUT/constant/fluidRegion"
cp -r "$SRC/system/fluidRegion" "$OUT/system/fluidRegion"
cp "$SRC/system/controlDict" "$OUT/system/controlDict"
cp "$SRC/constant/regionProperties" "$OUT/constant/regionProperties"
rm -rf "$OUT/constant/fluidRegion/polyMesh/sets" "$OUT/system/fluidRegion/"{changeDictionaryDict,createBafflesDict,decomposeParDict,funkySetBoundaryDict,fvSchemes_old,fvSolution_old,topoSetDict}
rm -f "$OUT/0/fluidRegion/powerDensityNeutronics" "$OUT/0/fluidRegion/powerDensityNeutronicsToLiquid" "$OUT/0/fluidRegion/cellToRegion" "$OUT/0/fluidRegion/rhok"
sed -i '/neutroRegion\|thermoMechanicalRegion/d' "$OUT/constant/regionProperties"
perl -0pi -e 's/regionSolvers\s*\{[^}]*\}/regionSolvers\n{\n    fluidRegion     onePhase;\n}/; s/removeBaffles\s*\{[^}]*\}/removeBaffles\n{\n    fluidRegion     false;\n}/' "$OUT/system/controlDict"
sed -i 's/^endTime .*/endTime 3;/' "$OUT/system/controlDict"
python3 -I - "$OUT/constant/fluidRegion/phaseProperties" <<'PY'
import re, sys
p = sys.argv[1]
s = open(p).read()
vals = iter(['1e8', '8e7'])
s = re.sub(r'(powerDensity\s+)0;', lambda m: m.group(1) + next(vals) + ';', s)
open(p, 'w').write(s)
PY
cat > "$OUT/constant/multiRegionCouplingDict" <<'EOT'
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
