# Decay-photon spectra against OpenMC (gh:#1391)

**Status: AI draft, human review outstanding.** Code: `crates/punggol/src/decay_spectrum.rs`.
Gate: `crates/punggol/tests/decay_photon_vs_openmc.rs`. Research, education and V&V only.

## What is ported

| OpenMC (commit `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`) | punggol |
|---|---|
| `openmc.data.decay_photon_energy` (`openmc/data/decay.py:558`) | `decay_spectrum::decay_photon_energy` |
| `Material.get_decay_photon_energy` (`openmc/material.py:402`) | `decay_spectrum::get_decay_photon_energy` |
| `combine_distributions`, `Discrete.merge/clip`, `Mixture.clip/mean/integral`, `Tabular.integral/mean`, `_intensity_clip` (`openmc/stats/univariate.py`) | `combine_distributions`, `DiscreteSpectrum`, `TabularSpectrum`, `MixtureSpectrum`, `intensity_clip` |
| `openmc.stats.DecaySpectrum` (`univariate.py:2204-2510`) | `DecaySpectrum` |
| C++ `DecaySpectrum` (`src/distribution.cpp:778-888`), `ChainNuclide::photon_energy_` (`src/chain.cpp:49`) | `ResolvedDecaySpectrum` |

The chain `<source>` elements are read by `outram_mc_libs::depletion::DepletionChainXml`
(the existing codec; no second parser). The C++ sampler reuses
`outram_mc_libs::source::distribution::Distribution`.

## Method

`openmc_inputs/decay_photon_reference.py` (below) runs the OpenMC Python API on two of
OpenMC's own test chains and writes every number to `openmc_outputs/{simple_decay,ni}.txt`.
The Rust test reads the same chain XML and repeats each call with the same inputs (the
reference's own atom densities and mass density, so `Material`'s density bookkeeping is
not compared), then compares kind, structure, every `x`, `p`, probability, integral, mean
and the `Mixture.clip` warning value.

- `chain_simple_decay.xml` (OpenMC `tests/chain_simple_decay.xml`): discrete lines (I135,
  U235, U238) and a histogram continuum (Xe135): reaches the discrete merge, the
  Tabular + Discrete mixture, a lone Tabular, a lone Discrete, a mixture clipped to a
  single component, and "no photon source" (`None`).
- `chain_ni.xml` (OpenMC `tests/chain_ni.xml`): 13 activation products with discrete lines.
- Cases: 8 `get_decay_photon_energy` calls on simple_decay (units Bq, Bq/cm3, Bq/m3, Bq/g,
  Bq/kg; clip 0, 1e-3, 1e-1; include and exclude filters) and 5 on ni; 3 + 2
  `DecaySpectrum`s (`integral`, `to_distribution`, `mean`, `clip(1e-3)`, `clip(1e-9)`).

**Pass criterion.** Bit identity is the goal; the gate allows a relative difference of
1e-9 (the workspace round-off allowance) and counts what is not bit-identical. Structure
(kinds, lengths, kept indices) must match exactly. After measurement the test **pins bit
identity** (`report` asserts `exact == total`).

**Why the summation is ported, not approximated.** Upstream sums with `np.sum`, which is
pairwise (blocks of eight, recursive split above 128). A sequential sum differs in the last
bits for lines this long. `np_sum` reproduces numpy's order (checked against numpy 2.4.6 for
n = 1..299, 1000 and 5000 on random data of mixed magnitude before use), and `np.trapezoid`'s
`(d * (y1 + y0) / 2).sum()`.

## Results (2026-11-20)

Hardware (`lscpu`, `free`): Intel Xeon Processor @ 2.10 GHz, 4 logical cores (a shared cloud
VM; the test runs single-threaded), 15 GB RAM, Linux 6.18, CPU only. No timing is claimed;
correctness only.

| Case set | Values compared | Bit-identical | Worst relative difference |
|---|---|---|---|
| simple_decay, per-nuclide `decay_photon_energy` | 394 | 394 | 0 |
| simple_decay, `Material.get_decay_photon_energy` (8 cases) | 1221 | 1221 | 0 |
| simple_decay, `DecaySpectrum` (3) | 756 | 756 | 0 |
| ni, per-nuclide | 410 | 410 | 0 |
| ni, `Material.get_decay_photon_energy` (5 cases) | 534 | 534 | 0 |
| ni, `DecaySpectrum` (2) | 270 | 270 | 0 |
| **total** | **3585** | **3585** | **0** |

Clip decisions (which lines, components and nuclides survive) match in every case, including
the `Mixture.clip` warning (`g_heavy_clip`: 0.11660932940725492 on both sides) and the
single-component pick. Interpretation: for these chains the port computes the same
distributions as OpenMC, not merely close ones.

**C++ `DecaySpectrum` is not run.** No OpenMC binary of this commit nor a photon library is
available here. Its absolute emission rate (`sum atoms * integral`) is checked against
Python's `DecaySpectrum.integral()` to 1e-12 (two summation routes over one quantity, not a
bit-identity claim), and its sampler by a statistical test (parent nuclides drawn in
proportion to emission rate within 1 %, lines drawn from the nuclide's own spectrum), in the
unit tests of `decay_spectrum.rs`.

## What this does not establish

- **Not photon physics.** Nothing is transported; this is the source term only.
- Two chains. A chain with `linear-linear` or `log-*` continua, mixture sources or a
  nuclide with several photon sources is not exercised (the last is a known upstream
  difference, see the module docs).
- The chains are OpenMC's small test chains, not a full ENDF/B-VIII.0 chain.
- `log-linear`, `log-log` mean/integral and `linear-log` entirely are refused by name, not
  checked.
- Python 3.11's sequential `sum` is what the Mixture sums copy; 3.12+ compensates it and
  would differ in the last bit.

## Upstream difference found

`Material.get_decay_photon_energy` returns `combined.distribution[0]` when clipping leaves
one mixture component, so the **mixture weight of that component is dropped**
(`material.py:480`). Ported as is, recorded in the module docs; it changes the integral only
when a continuous component survives alone. Python's reader keeps the last photon `<source>`
of a nuclide, the C++ the first (`chain.cpp:53`).

## Provenance

- OpenMC: <https://github.com/openmc-dev/openmc>, commit `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`
  (MIT; notice in `openmc_inputs/LICENSE.openmc`); numpy 2.4.6; Python 3.11.
- Chains: OpenMC `tests/chain_simple_decay.xml` and `tests/chain_ni.xml`, copied verbatim.
- No cross-section library, no particles, batches or Shannon entropy: no transport is run.
- Run: `PYTHONPATH=<OpenMC clone> python decay_photon_reference.py <chain.xml> <simple_decay|ni> <out.txt>`.
  `openmc.lib` is stubbed in the script (explained there): no `libopenmc` of this commit is
  built, and nothing here calls into it.
- Uncertainty: none; the quantities are deterministic arithmetic.

## Driver (verbatim `openmc_inputs/decay_photon_reference.py`)

```python
#!/usr/bin/env python3
"""OpenMC reference for punggol's decay-photon spectra (gh:#1391).

Usage:  decay_photon_reference.py <chain.xml> <case-set> <out.txt>

Runs the OpenMC Python API on a depletion chain and writes every number the
Rust port is compared against, one `tag value...` line each, floats as Python
`repr` (round-trips exactly). Case sets: `simple_decay` (chain_simple_decay.xml:
discrete lines for I135, U235, U238 and a histogram continuum for Xe135) and
`ni` (chain_ni.xml: 13 photon-emitting activation products).

Reference: OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (the vendored
clone, put on PYTHONPATH; no compiled library is needed), numpy 2.4.6,
Python 3.11. Python 3.11's built-in `sum` is sequential; 3.12+ compensates it.

    PYTHONPATH=<OpenMC clone> python decay_photon_reference.py \
        chain_simple_decay.xml simple_decay ../openmc_outputs/simple_decay.txt
"""
import re
import sys
import warnings

import numpy as np
import sys
import types
import xml.etree.ElementTree as ET

# `openmc.deplete` (imported by `openmc.data.decay_photon_energy` to read the
# chain) imports `openmc.lib`, which loads `libopenmc.so`. No library of this
# commit is built here, and the code under test never calls into it, so
# `openmc.lib` is replaced by a module whose every attribute is an empty class.
_lib = types.ModuleType('openmc.lib')
_lib.__getattr__ = lambda name: type(name, (), {})
sys.modules['openmc.lib'] = _lib

import openmc
from openmc.stats import Discrete, Mixture, Tabular

chain_path, case_set, out_path = sys.argv[1:4]
openmc.config['chain_file'] = chain_path

lines = []


def fl(v):
    return repr(float(v))


def arr(a):
    return ' '.join(fl(v) for v in a)


def emit(prefix, d):
    """Serialise a Discrete, Tabular or Mixture and its integral and mean."""
    if isinstance(d, Discrete):
        lines.append(f"{prefix} D")
        lines.append(f"{prefix}.x {arr(d.x)}")
        lines.append(f"{prefix}.p {arr(d.p)}")
    elif isinstance(d, Tabular):
        lines.append(f"{prefix} T {d.interpolation}")
        lines.append(f"{prefix}.x {arr(d.x)}")
        lines.append(f"{prefix}.p {arr(d.p)}")
    elif isinstance(d, Mixture):
        lines.append(f"{prefix} M {len(d.distribution)}")
        lines.append(f"{prefix}.prob {arr(d.probability)}")
        for i, c in enumerate(d.distribution):
            emit(f"{prefix}.{i}", c)
    else:
        raise TypeError(type(d))
    lines.append(f"{prefix}.integral {fl(d.integral())}")
    lines.append(f"{prefix}.mean {fl(d.mean())}")


def material(densities):
    m = openmc.Material()
    for name, d in densities.items():
        m.add_nuclide(name, d)
    m.set_density('sum')
    return m


def run_material(label, densities, **kwargs):
    m = material(densities)
    atom = m.get_nuclide_atom_densities()
    lines.append(f"CASE {label}")
    lines.append(f"{label}.names {' '.join(atom)}")
    lines.append(f"{label}.atoms {arr(atom.values())}")
    lines.append(f"{label}.massdensity {fl(m.get_mass_density())}")
    lines.append(f"{label}.opts {kwargs!r}")
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter('always')
        d = m.get_decay_photon_energy(**kwargs)
    diffs = [re.search(r'fraction of ([^ ]+) when', str(x.message)) for x in w]
    diffs = [g.group(1) for g in diffs if g]
    lines.append(f"{label}.warn {' '.join(repr(float(x)) for x in diffs) if diffs else 'none'}")
    if d is None:
        lines.append(f"{label} NONE")
    else:
        emit(label, d)


# per-nuclide spectra straight from the chain
for nuc in ET.parse(chain_path).getroot().iter('nuclide'):
    d = openmc.data.decay_photon_energy(nuc.get('name'))
    if d is not None:
        emit(f"NUC.{nuc.get('name')}", d)

if case_set == 'simple_decay':
    dens = {'I135': 1e-8, 'Xe135': 2e-9, 'U235': 1e-3, 'U238': 2e-2, 'Xe135_m1': 1e-9}
    run_material('a_bq', dens, units='Bq', volume=100.0)
    run_material('b_cm3_clip', dens, units='Bq/cm3', clip_tolerance=1e-3)
    run_material('c_m3_include', dens, units='Bq/m3', include_nuclides=['I135', 'Xe135'],
                 clip_tolerance=0.0)
    run_material('d_g', dens, units='Bq/g', clip_tolerance=0.0)
    run_material('e_kg_exclude', dens, units='Bq/kg', exclude_nuclides=['Xe135'])
    run_material('f_tabular_only', dens, units='Bq/cm3', include_nuclides=['Xe135'])
    run_material('g_heavy_clip', dens, units='Bq/cm3', clip_tolerance=0.1)
    run_material('h_none', dens, units='Bq/cm3', include_nuclides=['Xe135_m1'])
    spec_sets = {
        'ds1': ({'I135': 1e-8, 'Xe135': 2e-9, 'U235': 1e-3, 'U238': 2e-2, 'Xe135_m1': 1e-9}, 37.5),
        'ds2': ({'U235': 1e-3, 'U238': 2e-2}, 1.0),
        'ds3': ({'I135': 1e-8, 'Xe135': 2e-9}, 4.0),
    }
elif case_set == 'ni':
    dens = {'Ni58': 5e-2, 'Ni60': 2e-2, 'Fe55': 3e-9, 'Fe59': 1e-11, 'Co58': 4e-10,
            'Co60': 7e-9, 'Ni57': 2e-12, 'Ni59': 8e-8, 'Ni63': 6e-8, 'Ni65': 1e-13,
            'Co61': 1e-14}
    run_material('a_bq', dens, units='Bq', volume=250.0)
    run_material('b_cm3_clip', dens, units='Bq/cm3', clip_tolerance=1e-2)
    run_material('c_exclude', dens, units='Bq/m3', exclude_nuclides=['Co60'])
    run_material('d_include', dens, units='Bq/cm3', include_nuclides=['Co60', 'Ni63', 'Fe55'])
    run_material('e_g', dens, units='Bq/g', clip_tolerance=1e-9)
    spec_sets = {
        'ds1': (dens, 12.0),
        'ds2': ({'Co60': 7e-9, 'Ni63': 6e-8}, 2.0),
    }
else:
    raise SystemExit(f"unknown case set {case_set}")

# openmc.stats.DecaySpectrum (the Python class)
for label, (nuclides, volume) in spec_sets.items():
    ds = openmc.stats.DecaySpectrum(nuclides, volume)
    lines.append(f"DS {label}")
    lines.append(f"{label}.names {' '.join(nuclides)}")
    lines.append(f"{label}.dens {arr(nuclides.values())}")
    lines.append(f"{label}.volume {fl(volume)}")
    lines.append(f"{label}.integral {fl(ds.integral())}")
    d = ds.to_distribution()
    if d is None:
        lines.append(f"{label} NONE")
    else:
        emit(f"{label}.dist", d)
        lines.append(f"{label}.mean {fl(ds.mean())}")
    for tol in (1e-3, 1e-9):
        c = ds.clip(tol)
        lines.append(f"{label}.clip{tol!r} {' '.join(c.nuclides)}")

with open(out_path, 'w') as f:
    f.write('\n'.join(lines) + '\n')
print(f"wrote {len(lines)} lines to {out_path}")
```
