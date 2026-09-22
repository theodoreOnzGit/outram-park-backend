# A nuclide `.h5` this workspace wrote, transported by OpenMC

GitHub **#270** scope item 3, and its acceptance criterion:

> Cross-code: OpenMC reads a file this workspace wrote and produces a `k`
> consistent with the run that produced it. This is the check that makes the
> format claim real rather than self-consistent.

Measured 2026-09-22.

## Why a round trip would not have been enough

Writing a file and reading it back with this crate's own reader proves the two
halves of **one** codec agree. It cannot detect a format that is
self-consistent and wrong. This one nearly was: the first file this writer
produced stored `kTs/294K` as a **1-element array** where OpenMC writes a
**scalar**, and OpenMC failed with

```text
TypeError: type numpy.ndarray doesn't define __round__
```

raised inside `IncidentNeutron.temperatures` — a confusing error a long way
from its cause, and one no round trip through this crate could have found. It
is now fixed and the reason is recorded in the writer's source rather than
just here.

## Methodology

1. `write_nuclide` emits `Syn1.h5`: a synthetic nuclide with
   `Z = 1`, `A = 1`, `AWR = 0.999167`, a 64-point log grid from 1e−5 to
   2e7 eV, at 294 K.
   - **MT=2** elastic, `σ = 2 barn` flat, isotropic in the centre of mass.
   - **MT=102** capture, `σ = 1/√E` barn (316.2 b at 1e−5 eV, 7.07e−4 b at
     2 MeV).
2. `openmc.data.IncidentNeutron.from_hdf5` reads it back.
3. OpenMC transports a fixed source with it: a 2 MeV point source at the
   centre of a 10 cm vacuum-bounded sphere of the material at
   0.05 atoms/barn-cm, 20 000 particles × 10 batches, tallying flux and
   absorption in the cell.

Deck: [`openmc_inputs/run.py`](openmc_inputs/run.py) and
[`openmc_inputs/cs.xml`](openmc_inputs/cs.xml).

## Results

**OpenMC read every field back exactly as written:**

```text
READ OK: Syn1 temps: ['294K']
  MT=2   Q=0.0        cm=True   xs[0]=2       products=1
     neutron prompt UncorrelatedAngleEnergy yield(1eV)=1.0
       angle energies [1.e-05 2.e+07], mu[0] [-1.  1.]
  MT=102 Q=2200000.0  cm=False  xs[0]=316.2   products=0
```

Every one of those is the value the writer was given: the flat 2 barn elastic,
the 316.2 barn capture at the bottom of the grid, `Q = 2.2 MeV`, the
centre-of-mass flag on elastic and off on capture, the unit yield, and the
isotropic `[-1, +1]` cosine grid.

**And OpenMC transported with it:**

| quantity | OpenMC, on our file |
|---|---|
| cell flux | **10.8256 ± 0.0044** cm per source neutron |
| absorption | **0.0011 ± 0.0000** per source neutron |

Both are physically sensible and neither is a free parameter: a 2 MeV neutron
in this material has `Σ_t ≈ 0.05 × 2.0007 = 0.1000 cm⁻¹`, so the 10 cm sphere
is **one mean free path** and a non-scattering neutron would score exactly
10 cm of track. The measured 10.83 cm is that plus the extra path scattering
adds, and the 0.1 % absorption is what a `1/√E` capture of 7e−4 barn at
source energy gives before the neutrons leak out.

## What this does and does not establish

**Established.** The format is right — not merely self-consistent. OpenMC's
own reader accepts it, its own transport solver runs with it, and the values
it recovers are the values written.

**Not established.** A `k` comparison, because this writer covers **elastic
and capture only** (see the module docs for why: fission, inelastic and (n,xn)
need the correlated energy-angle laws of `openmc/data/`, which are not
ported). A scattering-plus-capture nuclide cannot sustain an eigenvalue
problem. `write_nuclide` **refuses** an MT it cannot express rather than
emitting a file missing a reaction, which would transport a different nuclide
from the one described and give a `k` wrong by an amount nothing reports.

**Also not done.** Running the *same* problem through `outram-mc-libs`'
transport for a two-code comparison on one library, which is the whole point
of the capability. That needs a public constructor for a `Nuclide` built from
supplied pointwise data — today the only paths are `from_core` and
`from_endf`/`from_tape`. That is a small, separate piece of work and it is
named here rather than left implicit.

## Provenance

OpenMC `afa7a14` (`/opt/openmc/bin/openmc`, `openmc` 0.1.dev1+gafa7a14ac,
h5py 3.16.0). #270 cites `608a1c33`, which is not an object in that clone.
