"""Reference for gh:#1622: OpenMC's own pool.deplete with TransferRates and
ExternalSourceRates on chain_simple.xml, two burnable materials, one step,
OpenMC's IPF CRAM-48. Pure Python (no libopenmc; run with READTHEDOCS=True,
upstream's documentation-build mock, since openmc.deplete imports
openmc.lib). The operator is a stand-in: TransferRates reads only its
burnable_mats, local_mats and chain.

Usage: python ref_transfer_rates.py <chain_simple.xml> <out.tsv>

Output: "key<TAB>value" lines (atomic_mass_U235, avogadro), then
"case<TAB>material<TAB>nuclide<TAB>density" lines, floats as Python repr.
"""
import sys
from types import SimpleNamespace

import numpy as np
import openmc.data
from openmc.deplete import Chain, ReactionRates
from openmc.deplete.cram import CRAM48
from openmc.deplete.pool import deplete
from openmc.deplete.transfer_rates import ExternalSourceRates, TransferRates

chain = Chain.from_xml(sys.argv[1])
nucs = [n.name for n in chain.nuclides]
mats = ["1", "2"]
op = SimpleNamespace(burnable_mats=mats, local_mats=mats, chain=chain)

# Per-atom reaction rates [1/s], the same in both materials (material 2 is
# irradiated too: this checks the block system, not a holding tank).
RATES = {
    "U234": {"(n,gamma)": 2.0e-9, "fission": 1.0e-11},
    "U235": {"(n,gamma)": 3.0e-10, "fission": 1.5e-9},
    "U238": {"(n,gamma)": 1.0e-10, "fission": 1.0e-12},
    "Xe135": {"(n,gamma)": 2.0e-5},
    "I135": {"(n,gamma)": 1.0e-9},
    "Gd157": {"(n,gamma)": 1.0e-5},
    "Gd156": {"(n,gamma)": 1.0e-9},
}
rates = ReactionRates(mats, nucs, chain.reactions)
for m in range(2):
    for nuc, rx in RATES.items():
        for name, r in rx.items():
            rates[m, rates.index_nuc[nuc], rates.index_rx[name]] = r

N0 = {"U234": 1e18, "U235": 5e20, "U238": 2e22, "Xe135": 1e15, "I135": 1e16, "Gd157": 1e17, "Gd156": 1e17, "Cs135": 1e16}
n0 = [np.array([N0.get(x, 0.0) for x in nucs]), np.array([0.5 * N0.get(x, 0.0) for x in nucs])]
dt = 86400.0

def transfer():
    tr = TransferRates(op, [], 1)
    tr.set_transfer_rate("1", ["Xe"], 1.0, transfer_rate_units="1/d", destination_material="2")
    return tr

def run(tr, es):
    out = deplete(CRAM48, chain, [x.copy() for x in n0], rates, dt, 0, None, tr, es)
    return [list(map(float, v)) for v in out]

cases = {}
cases["transfer"] = run(transfer(), None)
tr = transfer()
tr.set_transfer_rate("2", ["I135"], 0.5, transfer_rate_units="1/d")
cases["transfer_and_removal"] = run(tr, None)
es = ExternalSourceRates(op, [], 1)
es.set_external_source_rate("1", {"U235": 100.0}, 1.0, "g/d")
cases["transfer_and_feed"] = run(transfer(), es)
tr = transfer()
tr.set_redox("1", {"Cs135": 1.0}, {"Cs": 1, "I": -1, "Xe": 0, "U": 4, "Gd": 3})
cases["transfer_and_redox"] = run(tr, None)

with open(sys.argv[2], "w") as f:
    f.write(f"atomic_mass_U235\t{openmc.data.atomic_mass('U235')!r}\n")
    f.write(f"avogadro\t{openmc.data.AVOGADRO!r}\n")
    for case, out in cases.items():
        for m, v in enumerate(out):
            for nuc, x in zip(nucs, v):
                f.write(f"{case}\t{m}\t{nuc}\t{x!r}\n")
print("cases:", list(cases))
