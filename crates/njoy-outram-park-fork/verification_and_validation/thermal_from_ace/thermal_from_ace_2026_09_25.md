# Reading a thermal S(α,β) ACE table back into a scatterer

GitHub **#307**, ACE gap 2. Implemented and measured 2026-09-25.

## The gap

This crate could **write** every thermal ACE form — `IFENG = 0/1/2`, verified
byte-for-byte against NJOY2016 in
[`acer_thermal_ifeng_vs_njoy2016.md`](../acer_thermal_ifeng_vs_njoy2016.md) —
and could not **read** one back. `outram_mc_libs::ThermalScattering` had
`from_endf_file`, `from_tape` and `from_leapr`, so a thermal scatterer could only
come from an evaluation or from LEAPR output. A `.t` table found on disk, of the
kind MCNP and OpenMC libraries are distributed as, was unusable.

That is the asymmetry the ACE route has repeatedly turned out to have: the block
is in the file, the writer knows its layout exactly, and nothing reads it.

## What was implemented

`njoy_outram_park_fork::acer::thermal_read::decode_thermal`, and
`outram_mc_libs::material::thermal::ThermalScattering::from_ace` on top of it.

| block | JXS (0-based) | holds |
|---|---|---|
| ITIE | 0 | inelastic: `NE`, the incident grid, σ_inel(E) |
| ITIX | 1 | (unused locator; the emission table follows ITIE) |
| ITXE | 2 | per incident energy, `N_out × (E', μ_1…μ_{N_mu})` |
| ITCE | 3 | elastic: `NE`, the incident grid |
| ITCX | 4 | elastic cross section or cumulative Bragg sum |
| ITCA | 5 | elastic cosines, when `NCL > 0` |

`NXS`: `IDPNI` (inelastic form), `NIL` = `N_mu`, `NIEB` = `N_out`, `IDPNC`
(elastic form: 3 coherent/Bragg, 4 incoherent, 0 none), `NCL`, `IFENG`.

Two decisions worth recording:

- **`IFENG = 2` is refused, not approximated.** The continuous form stores a
  *different number of outgoing points per incident energy*, decided by NJOY's
  panel-merging threshold; reading it with the fixed `NIEB × (1 + NIL)` stride
  would silently resample the evaluation's own distribution. The refusal names
  the form.
- **The temperature comes from the table's own `kT`**, not from an argument.
  A thermal ACE table is single-temperature by construction, so accepting a
  temperature from the caller would let the two disagree.

## Methodology

Two levels, both against something other than the reader itself.

1. **`njoy`'s `tests/thermal_ace_read_round_trip.rs`** — build a thermal ACE
   table with this crate's own verified writer from
   `reference-data/endf/tsl-013_Al_027-ENDF8.0.endf` (MAT 53, 40-point log grid
   1e-4 → 4 eV, 16 outgoing × 8 cosines), serialise it, parse it back with the
   production `acer::read::parse_type1`, and decode. The writer is byte-verified
   against NJOY2016, so what the round trip tests is the reader against a
   reference-grade file.
2. **`outram-mc-libs`'s `tests/thermal_from_ace.rs`** — build a
   `ThermalScattering` through `from_ace` and compare its `total_xs(E)` against
   the **same tape read directly** through `from_tape`, at the tape's own first
   temperature. The two routes share no code below the constructor: one goes
   through THERMR/ACER and a fixed 16 × 8 emission grid, the other reads MF=7
   and builds the kernel directly.

## Results (2026-09-25)

Reader, Al-27 at the tape's first temperature:

```
Al-27 thermal: kT = 1.723500e-3 eV, IFENG = 0, 40 incident energies, 16 bins x 8 cosines
sigma_inel: 40/40 points positive, range 0.0002 .. 1.2945 b
coherent elastic: 568 Bragg edges, 3.7590e-3 .. 1.0000e1 eV
IFENG = 2 refused by name
```

Transport-side constructor, ACE route vs tape route:

```
from ACE: cutoff 4.0000 eV, T = 20.00 K
worst |diff| in total sigma vs the tape route: 0.952 % at 1.9862e-3 eV
```

**0.952 % worst is the ACE grid, not an error in either route.** The ACE table
carries 40 log-spaced incident energies where the tape's kernel is evaluated
wherever it is asked; the disagreement is largest at 1.99 meV, between grid
points and just below the 3.76 meV first Bragg edge, where σ changes fastest.
The bound in the test is 5 %, and the measured 0.952 % is recorded here so that
a regression that widens it is visible as a number rather than as a pass.

The temperature is asserted to round-trip: `20.00 K` out of the ACE `kT` against
the tape's own `20.000 K`.

## What this does not claim

- Only `IFENG = 0` is read end-to-end. `IFENG = 1` (skewed) decodes — the stride
  is identical — but no reference file in this repository carries one, so it is
  read and unexercised; `IFENG = 2` is refused.
- Nothing here measures a `k_eff`. The scatterer is compared against the other
  route's scatterer, not against a benchmark.
