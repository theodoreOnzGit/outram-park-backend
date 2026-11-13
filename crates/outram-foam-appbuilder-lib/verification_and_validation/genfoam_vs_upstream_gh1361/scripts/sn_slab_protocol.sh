#!/bin/bash
# PRE_RUN hook for the SN slab comparison (gh:#1361 round 2, S6'), run inside the copied
# guided tutorial 1 after tight_protocol.sh. Turns the 1-group diffusion slab into an S_N slab that
# uses (a) the Godiva_SN tutorial's own quadratureSet (4 directions per octant, 32 in all),
# (b) anisotropic scattering P0..P3 (legendreMoments 3, i.e. P0..P3 used: GeN-Foam stores 1 + legendreMoments matrices; P1..P3 are synthetic values, see the README),
# and (c) inletOutlet on the vacuum faces as Godiva_SN does. Same mesh, same P0/removal/fission data.
# CASES = the cases/sn_slab directory; STEPS = number of pseudo-time steps (default 12).
set -e
cp "$CASES/nuclearData"   constant/neutroRegion/nuclearData
cp "$CASES/quadratureSet" constant/neutroRegion/quadratureSet
cp "$CASES/defaultFlux"   0/neutroRegion/defaultFlux
rm -f 0/neutroRegion/defaultFlux2
sed -i "s/diffusionNeutronics/SNNeutronics/" system/controlDict
sed -i "s/^endTime .*/endTime         ${STEPS:-12};/; s/^deltaT .*/deltaT          1;/; s/^writeInterval .*/writeInterval   ${STEPS:-12};/; s/^adjustTimeStep .*/adjustTimeStep  false;/" system/controlDict
