#!/usr/bin/env python3
"""OpenMC reference for outram-mc-libs' waste classification port (gh:#1391).

Usage:  waste_reference.py <out.txt>

Builds activated-material recipes at a ladder of scales and records what
`openmc/waste.py` returns through `Material.waste_classification` and
`Material.waste_disposal_rating`, plus the inputs it computed them from
(`get_activity(units='Ci/m3', by_nuclide=True)` and `get_mass_density()`), so
the Rust side can start from exactly OpenMC's activities and test only the
port of `waste.py`. Floats are Python `repr` (they round-trip).

Reference: OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (vendored clone on
PYTHONPATH; no compiled library needed), numpy 2.4.6, Python 3.11. Half-lives
are `openmc/data/half_life.json` (ENDF/B-VIII.0), copied beside this script.
"""
import sys
import types

_lib = types.ModuleType('openmc.lib')  # see decay_photon_reference.py in punggol
_lib.__getattr__ = lambda name: type(name, (), {})
sys.modules['openmc.lib'] = _lib

import openmc

out_path = sys.argv[1]
lines = []

RECIPES = {
    # activated steel: a stable matrix and activation products
    'steel': {'Fe56': 8.0e-2, 'Co60': 1e-12, 'Ni63': 5e-10, 'Ni59': 1e-8, 'Nb94': 1e-11,
              'C14': 1e-9, 'Mn54': 1e-12, 'Fe55': 1e-10, 'H3': 1e-11},
    # a fission-product and transuranic mix
    'fp_tru': {'U238': 2.0e-2, 'Cs137': 1e-12, 'Sr90': 1e-12, 'Tc99': 1e-10, 'I129': 1e-12,
               'Pu239': 1e-10, 'Am241': 1e-11, 'Pu241': 1e-12, 'Cm244': 1e-13},
    # nothing in any table
    'inert': {'Fe56': 8.0e-2, 'Ni58': 1e-3},
}
FETTER_AND_NRC = ['Fetter', 'NRC_long', 'NRC_short_A', 'NRC_short_B', 'NRC_short_C']


def fl(v):
    return repr(float(v))


for recipe, base in RECIPES.items():
    for k in range(-6, 9):
        scale = 10.0 ** k
        dens = {n: (d if n in ('Fe56', 'U238', 'Ni58') else d * scale) for n, d in base.items()}
        m = openmc.Material()
        for n, d in dens.items():
            m.add_nuclide(n, d)
        m.set_density('sum')
        label = f"{recipe}_k{k}"
        act = m.get_activity(units='Ci/m3', by_nuclide=True)
        lines.append(f"CASE {label}")
        lines.append(f"{label}.names {' '.join(act)}")
        lines.append(f"{label}.atoms {' '.join(fl(v) for v in m.get_nuclide_atom_densities().values())}")
        lines.append(f"{label}.activity {' '.join(fl(v) for v in act.values())}")
        lines.append(f"{label}.massdensity {fl(m.get_mass_density())}")
        for metal in (False, True):
            lines.append(f"{label}.class.{metal} {m.waste_classification(metal=metal).replace(' ', '_')}")
            for lim in FETTER_AND_NRC:
                r = m.waste_disposal_rating(lim, metal=metal)
                lines.append(f"{label}.rating.{lim}.{metal} {fl(r)}")
                by = m.waste_disposal_rating(lim, metal=metal, by_nuclide=True)
                lines.append(f"{label}.bynuc.{lim}.{metal} "
                             + (' '.join(f"{n}={fl(v)}" for n, v in by.items()) or '-'))

with open(out_path, 'w') as f:
    f.write('\n'.join(lines) + '\n')
print(f"wrote {len(lines)} lines to {out_path}")
