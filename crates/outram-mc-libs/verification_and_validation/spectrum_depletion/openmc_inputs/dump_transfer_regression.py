"""Dump OpenMC's committed reference results for its regression test
tests/regression_tests/deplete_with_transfer_rates (gh:#1626 arm 3).

Those .h5 files were written by OpenMC's own CoupledOperator + Predictor runs
(transport on upstream's CI library). Each holds the beginning-of-step atoms
and normalised per-atom reaction rates of the one 1-day step, and the
end-of-step atoms. Predictor's end-of-step state is a single matrix
exponential on the beginning-of-step rates, so the port can be fed exactly
the rates OpenMC used and asked for the same atoms: no transport, no
statistics. Run with OpenMC e4febe9.

Usage: python dump_transfer_regression.py <regression_dir> <out.tsv>

Output lines, floats as Python repr:
  mass<TAB>nuclide<TAB>g/mol           (openmc.data.atomic_mass)
  abundance<TAB>nuclide<TAB>fraction   (openmc.data.NATURAL_ABUNDANCE, U)
  atoms<TAB>case<TAB>step<TAB>material<TAB>nuclide<TAB>atoms
  rate<TAB>case<TAB>material<TAB>nuclide<TAB>reaction<TAB>rate   (step 0)
  time<TAB>case<TAB>t0<TAB>t1<TAB>source_rate
"""
import sys
from pathlib import Path

import openmc.data
import openmc.deplete

d = Path(sys.argv[1])
out = open(sys.argv[2], "w")
for nuc in ["U234", "U235", "U238"]:
    out.write(f"mass\t{nuc}\t{float(openmc.data.atomic_mass(nuc))!r}\n")
    out.write(f"abundance\t{nuc}\t{float(openmc.data.NATURAL_ABUNDANCE[nuc])!r}\n")
for path in sorted(d.glob("ref_*.h5")):
    case = path.stem[len("ref_"):]
    res = openmc.deplete.Results(path)
    s0 = res[0]
    out.write(f"time\t{case}\t{float(s0.time[0])!r}\t{float(s0.time[1])!r}\t{float(s0.source_rate)!r}\n")
    for step in range(len(res)):
        s = res[step]
        for mat in s.index_mat:
            for nuc in s.index_nuc:
                out.write(f"atoms\t{case}\t{step}\t{mat}\t{nuc}\t{float(s[mat, nuc])!r}\n")
    rr = s0.rates
    for mat, im in rr.index_mat.items():
        for nuc, inuc in rr.index_nuc.items():
            for rx, irx in rr.index_rx.items():
                out.write(f"rate\t{case}\t{mat}\t{nuc}\t{rx}\t{float(rr[im, inuc, irx])!r}\n")
out.close()
