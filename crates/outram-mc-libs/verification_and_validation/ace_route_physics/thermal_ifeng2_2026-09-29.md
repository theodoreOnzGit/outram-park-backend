# Continuous (IFENG = 2) S(α,β) ACE tables: read and sampled as OpenMC (GitHub #365 audit)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

## What was missing

`acer::thermal_read::decode_thermal` refused IFENG = 2, NJOY's `iwt = 2`
continuous incoherent-inelastic form. That is the form OpenMC's own libraries
use. The refusal existed because the transport side held only the binned form.

## The change

- **Decode.** A port of OpenMC's reader (`openmc/data/thermal.py:809-887`):
  - `nang = NIL - 1` cosines per point;
  - `NEI` absolute locators followed by `NEI` point counts at `JXS(3)`;
  - per point `(E', pdf, cdf, mu_1..mu_nang)`;
  - cosines sorted within each point;
  - an `E' = 0` point prepended when the cdf does not start at 0.

  The locator convention was measured on the table below: `JXS(3) = 214`, and
  the first locator is `425 = 214 + 2·106 − 1`.
- **Sample.** `ThermalScattering::sample_continuous` is a port of OpenMC's
  `IncoherentInelasticAE::sample_params` and `sample`
  (`src/secondary_thermal.cpp`, `get_energy_index` from `src/math_functions.cpp`):
  1. take the nearer incident table;
  2. invert the lin-lin cdf, including upstream's fall-through when the uniform
     is at or above the last cdf value;
  3. shift `E'` to the actual incident energy;
  4. interpolate the equiprobable cosine between rows `j` and `j+1`, then smear it.

  The binned forms (IFENG = 0) keep the #188 scheme, unchanged.

## Test table (NJOY2016 2016.79, built in `target/ace_extra/HH2O_iwt2/`)

```text
reconr / 20 21 / 'h1 pendf'/ 125 0/ .001/ 0/
broadr / 20 21 22 / 125 1 0 0 0./ .001/ 293.6/ 0/
thermr / 26 22 23 / 1 125 16 1 2 0 0 1 222 0/ 293.6/ .001 4.0/
acer   / 20 23 0 30 40/ 2 1 1 .00/ 'h in h2o, iwt=2 ...'/ 125 293.6 'hh2o' 1/ 1001/
         222 64 0 0 1 4.0 2/
```

Inputs: tape20 is `reference-data/endf/n-001_H_001-ENDF8.0-Beta6.endf`, and
tape26 is `reference-data/endf/tsl-HinH2O.endf`. The resulting table has 106
incident energies, 623–655 points each, and 16 cosines per point.

## Verification

`crates/outram-mc-libs/tests/thermal_ifeng2_vs_openmc.rs`:
- **Reference.** OpenMC's `ThermalScattering.from_ace` of the same file. Its
  sampler is written as a deterministic function of the uniform and integrated
  over 400 000 midpoints ([`openmc_inputs/thermal_ifeng2_reference.py`](openmc_inputs/thermal_ifeng2_reference.py)
  → [`data/thermal_ifeng2_openmc.csv`](data/thermal_ifeng2_openmc.csv)).
- **Metric.** `N = 1e6` draws per incident energy; the sample means of `E'`,
  `E'^2` and `mu` must each lie within 5 sample-sem of the reference.

| E_in | ⟨E′⟩ outram | ⟨E′⟩ OpenMC | z | z(⟨E′²⟩) | ⟨μ⟩ outram | ⟨μ⟩ OpenMC | z |
|---|---|---|---|---|---|---|---|
| 5 meV | 0.012552 | 0.012553 | −0.03 | −0.84 | 0.03971 | 0.04051 | −1.38 |
| 25.3 meV | 0.030199 | 0.030205 | −0.34 | −0.45 | 0.17817 | 0.17817 | +0.01 |
| 0.1 eV | 0.081201 | 0.081194 | +0.22 | +0.35 | 0.34646 | 0.34665 | −0.38 |
| 0.4 eV | 0.261000 | 0.261034 | −0.29 | −0.26 | 0.48637 | 0.48626 | +0.25 |
| 1.0 eV | 0.554334 | 0.554200 | +0.45 | +0.58 | 0.57615 | 0.57623 | −0.23 |
| 2.5 eV | 1.299673 | 1.299812 | −0.19 | −0.29 | 0.62474 | 0.62446 | +0.82 |

(Energies in eV.)

- **The test can fail.** Removing the energy shift below `E_l/2` fails at
  0.1 eV (z = +13.2). This was run and reverted.
- **The writer round trip still passes.** `thermal_ace_read_round_trip`
  checks that a continuous table this crate wrote now reads back. It replaces
  the old "IFENG = 2 is refused" test.

## Scope

- This makes an IFENG = 2 table **usable**. It does not change which form the
  five-route study uses.
- The binned forms, and outram-mc's #188 continuous-in-bin scheme for them, are
  untouched (maintainer direction).
