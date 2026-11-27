#!/bin/bash
# gh:#1879 / gh:#1361 round 37: the 2D_MSFR `steadyStateTH` case (cases/msfr_th, turbulence false) with the `hx` zone's `fixedTemperature`
# powerModel replaced by the SHIPPED dictionaries of the four radial-conduction power models, as OVERLAY files in
# `cases/msfr_th_variants_r37/<name>/constant/fluidRegion/phaseProperties` (a directory of its own, like round 31). The port's tests and
# `run_msfr_th_radial_variants.sh` (VARIANT_DIR=<this dir>) copy the overlay over the same base, so both codes read the same bytes.
#
# Where each dictionary comes from (geometry, materials and conductances verbatim; NOTHING tuned to a result):
#   hp      heatedPin                 Tutorials/featureCases/2D_KNS37-L22 `"lowIn:topIn"` powerModel (innerRadius 0, outerRadius 0.003, k 20,
#                                     meshSize 6, T 653.15, Cp 500, rho 7700). ADDED: `powerDensity 3e8` (the tutorial gives it through a field
#                                     file `powerDensity.heatedPin`, not present here) and `volumeFraction 0.5` (the powerModel override, which
#                                     upstream reads in powerModel.C; the structure alpha stays 0.6).
#   nfp     nuclearFuelPin            Tutorials/featureCases/2D_fullCoupling zone0 (hollow fuel 0.0012..0.004715, clad 0.004865..0.005365,
#                                     fuelMeshSize 16, cladMeshSize 4, fuelRho 10480 fuelCp 250, cladRho 7500 cladCp 500, gapH 3000, fuelK 3,
#                                     cladK 20, fuelT = cladT = 600). CHANGED: `powerDensity 3e8` (the tutorial's 0 is overwritten by the neutronics).
#   nfp_tab the same with `gapHPowerDensity table ((0 1500) (1e8 3000) (4e8 6000))` instead of `gapH` (the Function1 table form of the
#                                     nuclearFuelPin documentation; ordinates are the power density).
#   lns     lumpedNuclearStructure    Tutorials/reactorCases/1D_thermalMSR_pointKinetics "core" (nodesNumber 1, heatConductances (450000 225000),
#                                     rhoCp (5357692), volumeFractions (1), powerFractions (1), powerDensity 5e6, T0 900). ADDED: `kappaMatrix 12`
#                                     (round 35's usage-example value; without it the mesh Laplacian is never used).
#   lns3    lumpedNuclearStructure    the same file's commented 3-node alternative (nodesNumber 3, nodeFuel 0, nodeClad 1,
#                                     heatConductances (600000 240000 600000 1200000), rhoCp 3 x 5357692, volumeFractions (0.75 0.125 0.125),
#                                     powerFractions (1 0 0)), with `kappaMatrix 12` and `nodeMatrix 2`.
# usage: make_msfr_th_radial_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/cases/msfr_th
OUT=$HERE/cases/msfr_th_variants_r37
rm -rf "$OUT"; mkdir -p "$OUT"

variant() { # name <powerModel block text>
  f=$OUT/$1/constant/fluidRegion/phaseProperties
  mkdir -p "$(dirname "$f")"
  cp "$BASE/constant/fluidRegion/phaseProperties" "$f"
  PM="$2" perl -0pi -e 's/(\n\s*)powerModel\s*\{[^}]*\}/$1 . $ENV{PM}/e' "$f"
}

variant hp '        powerModel
        {
            type                heatedPin;
            powerDensity        3e8;
            volumeFraction      0.5;
            innerRadius         0;
            outerRadius         0.003;
            k                   20;
            meshSize            6;
            T                   653.15;
            Cp                  500;
            rho                 7700;
        }'

NFP_COMMON='            type                nuclearFuelPin;
            volumetricArea      300;
            powerDensity        3e8;
            fuelInnerRadius     0.0012;
            fuelOuterRadius     0.004715;
            cladInnerRadius     0.004865;
            cladOuterRadius     0.005365;
            fuelMeshSize        16;
            cladMeshSize        4;
            fuelRho             10480;
            fuelCp              250;
            cladRho             7500;
            cladCp              500;
            fuelK               3;
            cladK               20;
            fuelT               600;
            cladT               600;'
variant nfp "        powerModel
        {
$NFP_COMMON
            gapH                3000;
        }"
variant nfp_tab "        powerModel
        {
$NFP_COMMON
            gapHPowerDensity    table
            (
                (0 1500)
                (1e8 3000)
                (4e8 6000)
            );
        }"

variant lns '        powerModel
        {
            type                lumpedNuclearStructure;
            volumetricArea      40;
            powerDensity        5e6;
            nodesNumber         1;
            nodeFuel            0;
            nodeClad            0;
            kappaMatrix         12;
            heatConductances    (450000 225000);
            rhoCp               (5357692);
            volumeFractions     (1);
            powerFractions      (1);
            T0                  900;
        }'
variant lns3 '        powerModel
        {
            type                lumpedNuclearStructure;
            volumetricArea      40;
            powerDensity        5e6;
            nodesNumber         3;
            nodeFuel            0;
            nodeClad            1;
            nodeMatrix          2;
            kappaMatrix         12;
            heatConductances    (600000 240000 600000 1200000);
            rhoCp               (5357692 5357692 5357692);
            volumeFractions     (0.75 0.125 0.125);
            powerFractions      (1 0 0);
            T0                  900;
        }'
echo "variants written to $OUT:"; ls "$OUT"
