# Waste classification and disposal rating against OpenMC (gh:#1391)

**Status: AI draft, human review outstanding.** Code: `crates/outram-mc-libs/src/waste.rs`.
Gate: `crates/outram-mc-libs/tests/waste_vs_openmc_1391.rs`. Research, education and V&V
only: **not** a licensing, classification or disposal authority.

## What is ported

OpenMC `openmc/waste.py` (286 lines, MIT, commit `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`):
`_waste_disposal_rating` (`waste.py:79`) and `_waste_classification` (`waste.py:7`), reached in
OpenMC as `Material.waste_disposal_rating` and `Material.waste_classification`
(`material.py:1862`, `:1838`).

Data tables, with upstream's citations (details in the module docs):

- **Fetter et al. (1990), Table 2**: 81 nuclides, Ci/m^3 (`waste.py:128-213`). Upstream gives
  "Fetter et al. (1990)" only; the full reference is **not re-checked** here.
- **10 CFR 61.55 Table 1** (`NRC_long`): C-14, Tc-99, I-129 in Ci/m^3 (C-14, Ni-59 and Nb-94 take
  the metal column), transuranics in nCi/g scaled by mass density.
- **10 CFR 61.55 Table 2** (`NRC_short_A/B/C`): Class A also adds every nuclide with a half-life
  below five years at 700 Ci/m^3.

The tables were transferred by a script reading `waste.py` with `ast`, not retyped, and the
comparison below exercises them.

Inputs: the workspace's one inventory type. `WasteInventory::from_inventory` takes a
`RadionuclideInventory` (one region), the volume and the mass density (`uom`), and gives Ci/m^3
per nuclide (`A_Bq / V * 1e6 / 3.7e10`). Half-lives for the Class A short-lived rule are a
`HalfLifeTable`, read from OpenMC's `half_life.json` (committed in `openmc_inputs/`, not
embedded in the crate) or from a depletion chain.

## Method

`openmc_inputs/waste_reference.py` (below) builds 45 activated-material cases in OpenMC: three
recipes (activated steel, a fission-product and transuranic mix, an inert material) with the
activation products scaled by 1e-6 ... 1e8, and records, for `metal` false and true,
`waste_classification`, `waste_disposal_rating` for the five named limit sets, and the per-nuclide
ratios, together with the inputs OpenMC computed them from (`get_activity(units='Ci/m3',
by_nuclide=True)` and `get_mass_density()`).

1. **The port of `waste.py`**: fed OpenMC's own activities and mass density and OpenMC's
   half-lives, the Rust functions must return the same class, per-nuclide ratios and ratio sums.
   Criterion: **bit identity**.
2. **The inventory adapter**: the same cases rebuilt as a `RadionuclideInventory` (Bq =
   `ln 2 / T_half * atoms`) and converted; compared with OpenMC's `get_activity`, criterion 1e-12
   relative (the operation order differs from OpenMC's, so round-off is expected).

## Results (2026-11-20)

| Comparison | Values | Bit-identical | Worst relative difference |
|---|---|---|---|
| classification (45 cases x metal false/true) | 90 | 90 | n/a (a string) |
| ratio sums (5 limit sets x 90) | 450 | 450 | 0 |
| per-nuclide ratios | 1020 | 1020 | 0 |
| inventory adapter, Ci/m^3 per nuclide | 300 | 146 | 4.5e-16 |

The classifications cover **all four outcomes** (the ladder gives Class A 58 times, Class B once,
Class C 4 times and GTCC 27 times across both `metal` values), so every branch of the
61.55(a)(3)-(7) rules is compared. The adapter's 154 differing values are the last-bit effect of
dividing a stored `lambda * atoms` by the volume where OpenMC multiplies
`lambda * 1e24 * n` by `1e6 / 3.7e10` (about 2 ulp), explained, and 2e12 times below the gate.
Class B is reached once only, which is a thin sample of that branch.

Hardware (`lscpu`, `free`): Intel Xeon Processor @ 2.10 GHz, 4 logical cores (a shared cloud VM;
the test runs single-threaded), 15 GB RAM, Linux 6.18, CPU only. No timing is claimed.

## What this does not establish

- The tables are checked against OpenMC's transcription of them, **not** against the regulation or
  the Fetter paper. If upstream mis-transcribed a limit, so does this port, identically.
- Mass density is supplied, not computed from atomic masses; OpenMC's `get_mass_density()` value
  is the input here.
- Half-lives are OpenMC's ENDF/B-VIII.0 JSON; a nuclide absent from the table reads as stable (as
  upstream's `dict.get`), which silently drops it from the Class A short-lived sum.
- The `metal` default differs between OpenMC's two entry points (`Material.waste_classification`
  `False`, `_waste_classification` `True`); here `metal` is always an argument.

## Provenance

- OpenMC: <https://github.com/openmc-dev/openmc>, commit `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`
  (MIT, `openmc_inputs/LICENSE.openmc`); numpy 2.4.6; Python 3.11.
- `openmc_inputs/half_life.json`: copied verbatim from `openmc/data/half_life.json` (ENDF/B-VIII.0
  decay sublibrary half-lives, as OpenMC ships them).
- No transport is run: no cross-section library, particles, batches or entropy apply. The
  quantities are deterministic arithmetic, so no statistical uncertainty.
- Run: `PYTHONPATH=<OpenMC clone> python waste_reference.py openmc_outputs/waste.txt`. `openmc.lib`
  is stubbed in the script (no `libopenmc` of this commit is built; nothing here calls into it).

## Driver (verbatim `openmc_inputs/waste_reference.py`)

```python
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
```
