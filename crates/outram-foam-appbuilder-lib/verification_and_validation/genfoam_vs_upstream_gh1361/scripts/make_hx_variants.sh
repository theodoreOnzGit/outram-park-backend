#!/bin/bash
# gh:#1847 / README round 44: the non-conformal variant `nc` of featureCases/1D_HX/onePhase: the secondary exchanger zone `sHX`
# meshed with 30 cells instead of 20 (blockMeshDict `sHX (1 1 20)` -> `(1 1 30)`, `blockMesh -region fluidRegion`); nothing else changes.
# usage: make_hx_variants.sh   (writes tests/data/genfoam_1d_hx_nonconformal; idempotent; needs the ESI v2506 environment)
set -e
HERE="$(cd "$(dirname "$0")/.." && pwd)"
CRATE="$(cd "$HERE/../.." && pwd)"
SRC="$CRATE/tests/data/genfoam_1d_hx_onephase"
DST="$CRATE/tests/data/genfoam_1d_hx_nonconformal"
rm -rf "$DST"
mkdir -p "$DST"
cp -r "$SRC/0" "$SRC/constant" "$SRC/system" "$DST/"
sed -i 's/hex ( 28 29 30 31 32 33 34 35 ) sHX (1 1 20)/hex ( 28 29 30 31 32 33 34 35 ) sHX (1 1 30)/' "$DST/system/fluidRegion/blockMeshDict"
rm -rf "$DST/constant/fluidRegion/polyMesh"
(cd "$DST" && bash /home/user/upstream-build/scripts/with-of.sh blockMesh -region fluidRegion > /dev/null)
cat > "$DST/PROVENANCE.md" <<'EOF'
# Provenance: `featureCases/1D_HX/onePhase` with the secondary exchanger zone refined (README round 44, gh:#1847)

- **Derived from** `tests/data/genfoam_1d_hx_onephase` (the shipped case, commit 652b3da of GeN-Foam, GPL-3.0, (C) 2015-2022 EPFL).
- **Only change:** `system/fluidRegion/blockMeshDict`, block `sHX`: `(1 1 20)` -> `(1 1 30)`, and the mesh regenerated with
  `blockMesh -region fluidRegion` (OpenFOAM-ESI v2506) by `scripts/make_hx_variants.sh`. The primary and secondary exchanger zones keep
  equal volumes (the one condition `heatExchanger.C` checks) and their meshes become non-conformal, so the `meshToMesh` weights are
  fractional. Justified from `heatExchanger.C` ("the two meshes of the heat exchanger could be non-conformal") and the shipped
  tutorial's comment (the zones "can have any relative displacement").
EOF
echo "wrote $DST"
