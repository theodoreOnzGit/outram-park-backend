#!/usr/bin/env python3
"""gh:#1944 round 74, ONE-EDIT variant V1 of a case made by make_esfr_coupled_case.sh: the `neutroRegion <- thermoMechanicalRegion` mapping
`meshDisp -> disp` is removed from constant/multiRegionCouplingDict, so the neutronics `disp` stays 0: `axExp` and `radExp` (the two XS
feedback variables taken from `disp`) are 0 and the neutronics mesh does not move. Isolates the displacement feedback of the cross sections.
usage: make_esfr_coupled_variant_v1.py <case dir>
"""
import re
import sys

p = sys.argv[1] + '/constant/multiRegionCouplingDict'
t = open(p).read()
new, n = re.subn(r'\n\s*thermoMechanicalRegion\s*\{\s*sourceFields\s*\(\s*meshDisp\s*\);\s*targetFields\s*\(\s*disp\s*\);\s*\}', '', t, count=1)
assert n == 1, 'the meshDisp -> disp mapping was not found'
open(p, 'w').write(new)
