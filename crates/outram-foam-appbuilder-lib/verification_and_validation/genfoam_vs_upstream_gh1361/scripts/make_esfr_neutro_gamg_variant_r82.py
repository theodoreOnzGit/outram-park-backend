#!/usr/bin/env python3
"""gh:#1956 round 82: the ONE-EDIT neutronics GAMG variant of the shipped 3D_SmallESFR_NewSolverVerification/legacySolver.

usage: make_esfr_neutro_gamg_variant_r82.py <case>/system/neutroRegion/fvSolution

The shipped `"flux.*|adjoint_flux.*"` entry selects `PCG`/`DIC` and carries the GAMG block commented out in `/* ... */`. This comments
the four PCG lines out and uncomments the shipped GAMG block, text otherwise unchanged (round 79 made the same edit to 2D_MSFR).
Run it BEFORE the TIGHT sed of make_esfr_coupled_case.sh (`NEUTRO_GAMG=1` does that), so the tight protocol tightens the GAMG lines.
"""
import sys

path = sys.argv[1]
text = open(path).read()
pcg = """        solver          PCG;
        preconditioner  DIC;
        tolerance       1e-6;
        relTol          1e-3;
   /*
        solver           GAMG;"""
assert text.count(pcg) == 1, "the shipped PCG lines + the opening of the commented GAMG block were not found exactly once"
text = text.replace(pcg, """        /* gh:#1956 round 82 variant (the ONE edit): the PCG lines below commented out, the shipped GAMG block uncommented
        solver          PCG;
        preconditioner  DIC;
        tolerance       1e-6;
        relTol          1e-3; */

        solver           GAMG;""")
close = "\t    processorAgglomerator masterCoarsest; */"
assert text.count(close) == 1, "the closing of the commented GAMG block was not found exactly once"
text = text.replace(close, "\t    processorAgglomerator masterCoarsest;")
open(path, "w").write(text)
