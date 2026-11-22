"""Reference for gh:#1625: libopenmc's own Nuclide::collapse_rate
(openmc.lib.Nuclide.collapse_rate, nuclide.cpp:1045 at e4febe9) on the
pincell library of gh:#1626 (pincell_ace_to_hdf5.py), CCFE-709 group
bounds, for every chain_simple nuclide with data and MT 102, 18, 16.

The group flux is a fixed, positive, made-up spectrum (a 0.0253 eV
Maxwellian plus 1/E plus a Watt-like fission tail, per group by midpoint);
any positive flux tests the arithmetic, and writing it out lets the port
use the identical doubles.

Usage: python ref_collapse_rate.py <model dir with materials/geometry/settings.xml> <out.tsv>
(with OPENMC_CROSS_SECTIONS pointing at the converted library). The model
directory only serves openmc.lib.init; the pincell run's own XML is used.

Output, floats as Python repr:
  edge<TAB>i<TAB>eV
  flux<TAB>g<TAB>value
  rate<TAB>nuclide<TAB>MT<TAB>value
"""
import os
import sys

import numpy as np
import openmc.lib
from openmc.mgxs import GROUP_STRUCTURES

model_dir, out_path = sys.argv[1], sys.argv[2]
edges = np.array(GROUP_STRUCTURES['CCFE-709'], dtype=float)
mid = np.sqrt(edges[:-1] * edges[1:])
width = edges[1:] - edges[:-1]
kt = 0.0253
phi_e = (mid / kt**2) * np.exp(-mid / kt) + 0.05 / mid + 1e-8 * np.exp(-mid / 1.0e6) * np.sinh(np.sqrt(2.0e-6 * mid))
flux = phi_e * width

os.chdir(model_dir)
openmc.lib.init(output=False)
with open(out_path, "w") as out:
    for i, e in enumerate(edges):
        out.write(f"edge\t{i}\t{float(e)!r}\n")
    for g, f in enumerate(flux):
        out.write(f"flux\t{g}\t{float(f)!r}\n")
    for nuc in ["U234", "U235", "U238", "I135", "Xe135", "Xe136", "Cs135", "Gd156", "Gd157"]:
        if nuc not in openmc.lib.nuclides:
            openmc.lib.load_nuclide(nuc)
        n = openmc.lib.nuclides[nuc]
        for mt in [102, 18, 16]:
            out.write(f"rate\t{nuc}\t{mt}\t{n.collapse_rate(mt, 293.6, edges, flux)!r}\n")
openmc.lib.finalize()
