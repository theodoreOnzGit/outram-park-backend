#!/bin/bash
# gh:#1884 / round 41 (run_tutorial.sh PRE_RUN hook): copy the overlay files of cases/boiling_radial_r41/$VARIANT over the shipped
# 1D_boiling work dir, then the round-7 protocol (boiling_mules_protocol.sh: writePrecision 17, EVERY_STEP, TIGHT, END_TIME; MULES as shipped).
# usage: VARIANT=hp EVERY_STEP=1 TIGHT=1 END_TIME=12 PRE_RUN=<this script> run_tutorial.sh featureCases/1D_boiling r41_hp
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cp -r "$HERE/cases/boiling_radial_r41/$VARIANT/." .
bash "$HERE/scripts/boiling_mules_protocol.sh"
grep -n "type  *\(heatedPin\|nuclearFuelPin\|lumpedNuclearStructure\)" constant/fluidRegion/phaseProperties
