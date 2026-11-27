#!/bin/bash
# gh:#1916 / gh:#1883 / gh:#1361 round 59: stage the `transientNoDriveline` stage of the shipped Tutorials/featureCases/2D_fullCoupling from the STEADY
# STATE made by make_fullcoupling_coupled_case.sh + run_fullcoupling_coupled.sh, exactly as the tutorial's Allrun `setTransientNoDriveline` does it:
#   neutronicsProperties: model pointKinetics, eigenvalueNeutronics false
#   controlDict         : startTime <the last written time of the steady state>, endTime 200, deltaT 1e-6, writeControl timeStep, writeInterval 1
#   nuclearData         : absoluteDrivelineExpansionCoeff 0.0
#   constant/multiRegionCouplingDict = multiRegionCouplingDict_transient
# Differences from Allrun, all recorded in README round 59: startTime is the steady state's LAST time (99.9583331749870752: this protocol writes
# every step with writeControl timeStep and does not adjust deltaT to land on 100, the tutorial's adjustableRunTime does), writeInterval 1
# (Allrun: 10), writePrecision 17 and the tight solver protocol (inherited from the steady-state case).
# usage: make_fullcoupling_transient_case.sh <steady-state case dir> <out case dir> [feedbackCoeffTFuel override]
SS=$1
C=$2
T=$(ls "$SS" | grep -E '^[0-9]' | sort -g | tail -1)
rm -rf "$C"
mkdir -p "$C"
cp -r "$SS/constant" "$SS/system" "$C/"
cp -r "$SS/$T" "$C/$T"
ctl() { sed -i "s/^$2 .*/$2 $3;/" "$1/system/controlDict"; }
sed -i 's/^model .*/model pointKinetics;/' $C/constant/neutroRegion/neutronicsProperties
sed -i 's/^eigenvalueNeutronics .*/eigenvalueNeutronics false;/' $C/constant/neutroRegion/neutronicsProperties
ctl $C startTime "$T"
ctl $C endTime 200
ctl $C deltaT 1e-6
ctl $C writeControl timeStep
ctl $C writeInterval 1
# FOUND 2026-11-27: Allrun only sets `model pointKinetics` in neutronicsProperties, which only the legacy mode (-legacy, modifyDictLegacy.H) reads;
# the shipped controlDict regionSolvers still says `neutroRegion diffusionNeutronics`, so the SHIPPED Allrun runs a diffusion TRANSIENT. The
# point-kinetics stage therefore needs this one further edit (not in Allrun):
[ -z "$SHIPPED" ] && sed -i 's/^\( *neutroRegion *\)diffusionNeutronics;/\1pointKinetics;/' $C/system/controlDict
sed -i 's/^absoluteDrivelineExpansionCoeff .*/absoluteDrivelineExpansionCoeff 0.0;/' $C/constant/neutroRegion/nuclearData
if [ -n "$3" ]; then sed -i "s/^feedbackCoeffTFuel .*/feedbackCoeffTFuel $3;/" $C/constant/neutroRegion/nuclearData; fi
cp $C/constant/multiRegionCouplingDict_transient $C/constant/multiRegionCouplingDict
echo "$T"
