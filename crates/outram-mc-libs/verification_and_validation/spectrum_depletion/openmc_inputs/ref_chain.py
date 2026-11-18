"""Reference for gh:#1621: OpenMC's own Chain.from_endf on the files the
Rust port reads (ENDF/B-VIII.0 decay and NFY sublibraries from
reference-data/ace, and the neutron tapes named below), in the same sorted
order, exported with Chain.export_to_xml.

Usage (OpenMC e4febe9 on PYTHONPATH):
    python ref_chain.py <endf-b-viii.0 dir> <reference-data/endf dir> <out.xml> [neutron tapes...]
"""
import sys
from pathlib import Path

import openmc.deplete

data = Path(sys.argv[1])
root = Path(sys.argv[2])
out = sys.argv[3]
neutron = [Path(p) if Path(p).is_absolute() else (data / p if (data / p).exists() else root / p) for p in sys.argv[4:]]

decay = sorted(p for p in (data / "decay").iterdir() if p.suffix == ".endf")
nfy = sorted(p for p in (data / "nfy").iterdir() if p.suffix == ".endf")
chain = openmc.deplete.Chain.from_endf(decay, nfy, neutron, progress=False)
chain.export_to_xml(out)
print(f"{len(chain.nuclides)} nuclides -> {out}")
