#!/bin/bash
# gh:#1847 / README round 44: the 2D_MSFR `steadyStateTH` case (cases/msfr_th, onePhase `cellCentered`, tight 1e-14 solvers) with a heat
# exchanger whose primary AND secondary zone is the `hx` cell zone, as an OVERLAY file in `cases/msfr_th_variants_r44/<name>/`.
# No two zones of the MSFR mesh have equal volumes (hx 2035 cells, pump 715, intermed 165, main_fd 5680; upstream aborts when
# (pV - sV)/pV >= 1e-2), so the only exchanger the mesh admits is a zone paired with itself: the translation is zero and the meshToMesh
# mapping the identity, so THX = HT/H exactly. That exercises the cell-centred WIRING (the guard max(THX) > 1e-69, the one-sum source
# assembly, the current-T lookup, the position of structure::correct) and NOT the weighted averaging, which the face-centred cases of
# README round 44 test. `volumetricArea 250` and `wallConductance 2.5e4` are the values of the shipped 1D_HX tutorial.
# usage: make_msfr_th_hx_variants.sh   (idempotent; run from anywhere)
set -e
HERE=$(cd "$(dirname "$0")/.." && pwd)
BASE=$HERE/cases/msfr_th
OUT=$HERE/cases/msfr_th_variants_r44
rm -rf "$OUT"
f=$OUT/hx_self/constant/fluidRegion/phaseProperties
mkdir -p "$(dirname "$f")"
cp "$BASE/constant/fluidRegion/phaseProperties" "$f"
perl -0pi -e 's/(momentumSource\s+\(0 0 -300000\);\s*\})(\s*\})/$1\n\n    heatExchangers\n    {\n        "selfHX"\n        {\n            primary             "hx";\n            secondary           "hx";\n            volumetricArea      250;\n            wallConductance     2.5e4;\n        }\n    }$2/' "$f"
grep -n "heatExchangers" -A9 "$f"
