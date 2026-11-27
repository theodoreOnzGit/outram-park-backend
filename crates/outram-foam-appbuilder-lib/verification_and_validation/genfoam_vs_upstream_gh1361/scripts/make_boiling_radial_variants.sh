#!/bin/bash
# gh:#1884 / gh:#1361 round 41: the shipped `1D_boiling` (twoPhase, MULES `solvePhase both`, as shipped) with the `mid` zone's `fixedPower`
# powerModel replaced by the SHIPPED dictionaries of the radial-conduction power models, as OVERLAY files in
# `cases/boiling_radial_r41/<name>/constant/fluidRegion/phaseProperties` (+ `0/fluidRegion/powerDensity.heatedPin` for hp_field). The port's
# tests and `boiling_radial_variant_protocol.sh` (VARIANT=<name>) copy the overlay over the same base (`tests/data/genfoam_1d_boiling`), so both codes
# read the same bytes. Only the powerModel block of `mid` (cells 10..29, volumeFraction 0.5, Dh 0.005) changes; the shipped power density
# (4.5e8 W/m3) and initial temperature (652.15 K) of that block are kept where the model has the same key.
#
# WHY NOT 2D_KNS37-L22: it selects Rehme, Autruffe, Schor, annular and KottowskiSavatteri closures (unit-tested only in the port) and
# `localTortuosity` (refused by name). 1D_boiling is the twoPhase case the port runs against the binary (rounds 3-17, 34); upstream's own
# option is that a zone's powerModel is whatever `powerModel::New` selects.
#
# Where each dictionary comes from (geometry, materials and conductances verbatim; NOTHING tuned to a result):
#   hp       heatedPin  KNS37 "lowIn:topIn" (innerRadius 0, k 20, meshSize 6, Cp 500, rho 7700 - the same rho, Cp as the shipped fixedPower).
#            outerRadius 0.01: for a solid pin `iA = volumeFraction * 2/ro` = 100 1/m, the shipped fixedPower `volumetricArea`, so the
#            fluid-side geometry equals the shipped case and only the model changes. T 652.15 (shipped), powerDensity 4.5e8 (shipped).
#   hp3      heatedPin  as hp with KNS37's own outerRadius 0.003 (iA = 333 1/m).
#   hp_field heatedPin  as hp but the dictionary has NO `powerDensity`; the cell values come from `0/fluidRegion/powerDensity.heatedPin`
#            (the way 2D_KNS37-L22 supplies them: `READ_IF_PRESENT` field, `structure::setRegionField` does nothing when it is present).
#            The field is 4.5e8 * (0.8 + 0.4 (i-10)/19) on cells 10..29 (a 20 % linear tilt, written with 17 digits) and 0 elsewhere.
#   hp_off   as hp with the shipped `powerOffCriterionModel { type timer; time 7; }` set to time 0.3 (the shipped file has this entry; the KNS37 tutorial has time 12.475):
#            `structure::checkPowerOff` -> `heatedPin::powerOff` (`powerDensity_ *= 0`).
#   nfp      nuclearFuelPin  2D_fullCoupling zone0 (hollow fuel 0.0012..0.004715, clad 0.004865..0.005365, fuelMeshSize 16, cladMeshSize 4,
#            fuelRho 10480 fuelCp 250, cladRho 7500 cladCp 500, gapH 3000, fuelK 3, cladK 20, fuelT = cladT = 600), powerDensity 4.5e8 (shipped,
#            the neutronics' `powerDensityNeutronics` through `setRegionField`).
#   nfp_tab  the same with `gapHPowerDensity table ((0 1500) (1e8 3000) (4e8 6000))` instead of `gapH`.
#   nfp_hot  as nfp with powerDensity 9e8 (2x): a CASE-DESIGN choice made after the upstream run of `nfp` showed the fluid reaching 1017 K of the
#            1161 K saturation in 12 s without a boiling onset; the number is not an input of any comparison (the comparison is port against
#            upstream on the same case) and 2x is the smallest round factor, recorded as a post-run decision in README round 41.
#   lns      lumpedNuclearStructure  1D_thermalMSR_pointKinetics "core" VERBATIM (nodesNumber 1, heatConductances (450000 225000), rhoCp 5357692,
#            powerDensity 5e6, volumetricArea 40, T0 900) plus `kappaMatrix 12` (round 35/37's value, so the mesh Laplacian is exercised). The shipped
#            power is 90x smaller than 1D_boiling's, so this variant stays single phase: it is a single-phase test of the lumped wiring.
# usage: make_boiling_radial_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/../../tests/data/genfoam_1d_boiling
OUT=$HERE/cases/boiling_radial_r41
rm -rf "$OUT"; mkdir -p "$OUT"

variant() { # name <powerModel block text> [yes: add the powerOffCriterionModel]
  f=$OUT/$1/constant/fluidRegion/phaseProperties
  mkdir -p "$(dirname "$f")"
  cp "$BASE/constant/fluidRegion/phaseProperties" "$f"
  PM="$2" perl -0pi -e 's/(\n\s*)powerModel\s*\{[^}]*\}/$1 . $ENV{PM}/e' "$f"
  if [ "$3" = yes ]; then
    # the shipped phaseProperties ALREADY ends with `powerOffCriterionModel { type timer; time 7; }` (the later duplicate key wins in OpenFOAM and in the
    # port): change THAT entry's time, do not add a second block (the first version of this script did, and was silently overridden: README round 41)
    perl -0pi -e 's/(type\s+timer;\s*\n\s*\/\/-\s*\n\s*time\s+)7;/${1}0.3;/' "$f"
  fi
}
HP='        powerModel
        {
            type                heatedPin;
            powerDensity        4.5e8;
            innerRadius         0;
            outerRadius         0.01;
            k                   20;
            meshSize            6;
            T                   652.15;
            Cp                  500;
            rho                 7700;
        }'
HP3=${HP/0.01;/0.003;}
variant hp "$HP"
variant hp_off "$HP" yes
variant hp3 "$HP3"
variant hp_field '        powerModel
        {
            type                heatedPin;
            innerRadius         0;
            outerRadius         0.01;
            k                   20;
            meshSize            6;
            T                   652.15;
            Cp                  500;
            rho                 7700;
        }'
mkdir -p "$OUT/hp_field/0/fluidRegion"
{
  sed -n '1,/^dimensions/p' "$BASE/0/fluidRegion/p" | sed 's/object      p;/object      powerDensity.heatedPin;/; s/^dimensions.*/dimensions      [1 -1 -3 0 0 0 0];/'
  echo; echo "internalField   nonuniform List<scalar> 40"; echo "("
  awk 'BEGIN{for(i=0;i<40;i++){ v=(i>=10&&i<=29)?4.5e8*(0.8+0.4*(i-10)/19):0; printf "%.17g\n", v }}'
  echo ")"; echo ";"; echo
  echo "boundaryField"; echo "{"
  echo "    outlet { type zeroGradient; }"; echo "    inlet { type zeroGradient; }"; echo "    fixedWalls { type empty; }"
  echo "}"
} > "$OUT/hp_field/0/fluidRegion/powerDensity.heatedPin"
NFP_COMMON='            type                nuclearFuelPin;
            powerDensity        4.5e8;
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
variant nfp_hot "        powerModel
        {
${NFP_COMMON/4.5e8/9e8}
            gapH                3000;
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
echo "variants written to $OUT:"; ls "$OUT"
