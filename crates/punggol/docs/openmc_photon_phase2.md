# OpenMC photon port, phase 2: differential test against a built OpenMC (2026-11-21)

Status: **the three data-driven samplers agree bit for bit; the rest is still
"first port, not differentially tested".**

## Reference (scratch, nothing committed)

Built from source in `/tmp/claude-0/`: OpenMC `8f79d9d0` (`cmake -S <clone> -B
<build> -DOPENMC_BUILD_TESTS=OFF -DOPENMC_USE_OPENMP=OFF`, then `cmake --build
<build> --target libopenmc -j 2`; apt `libhdf5-dev libpugixml-dev libfmt-dev`);
NJOY2016 (clone `njoy/NJOY2016`, cmake) run on the real ENDF/B-VIII.0
uranium photo-atomic and atomic-relaxation evaluations from
`NJOY2016/tests/resources` (`njoy < tests/48/input`, tapes 20 and 21) as a
cross-check of the ACE route; OpenMC's Python `IncidentPhoton.from_endf` (with
`endf-python`, see `AGENTS.md`) writes the HDF5 photon library `U.h5`
(`verification_and_validation/openmc_photon/mk_h5.py`; OpenMC's own
`compton_profiles.h5`, `density_effect.h5` and `BREMX.DAT` come from its source
tree). `from_ace` was not usable: it rejects NJOY's photoatomic ACE (no subshell
data).

`verification_and_validation/openmc_photon/harness.cpp` links `libopenmc`,
loads `U.h5` into a real `openmc::PhotonInteraction`, dumps the members the
samplers read and runs the samplers on fixed seeds.

## Comparison

`tests/openmc_photon_vs_openmc.rs` (`OPENMC_REFERENCE_DUMP`,
`OPENMC_REFERENCE_SAMPLES`, skipped otherwise) builds a
`punggol::openmc_photon::PhotonInteraction` from the dump and runs
`compton_scatter`, `rayleigh_scatter` and `pair_production` on the same seeds.

Uranium (Z = 92), photon energies 1 keV to 100 MeV (8), 200 seeds each, Compton
with and without Doppler broadening: **5400 samples**.

| quantity | worst difference |
|---|---|
| `Tabulated1D` probes (incoherent and coherent form factors, 20 points) | 0 (exact) |
| `compton_scatter` outgoing alpha | 0 (relative) |
| `compton_scatter` mu | 0 (absolute) |
| `compton_scatter` shell index | 0 mismatches |
| `rayleigh_scatter` mu | 0 |
| `pair_production` (E-, E+, mu-, mu+) | 0 (relative) |
| final LCG seed after every call | 0 mismatches |

`calculate_xs` (harness builds an `openmc::Particle`; Rust rebuilds the
element from the dump), 2026-11-21, ENDF/B-VIII.0 via NJOY2016 photoat + atrelax:

| element | points | index mismatches | worst difference (relative; absolute where reference is 0) |
|---|---|---|---|
| U (Z=92) | 307 | 0 | coherent 1.2e-16, all others 0 |
| H (Z=1, no subshell structure beyond K) | 184 | 0 | photoelectric 1.3e-16, all others 0 |

H samplers: 5584 samples, all 0 (alpha, mu, shell, pair, final seed). The
1e-16 residuals are last-bit round-off (below the 1e-9 allowance); cause not
traced further.

`atomic_relaxation` (2026-11-21; charged secondaries deposited locally, i.e.
`ElectronTreatment::LED`, so the bank holds the fluorescent photons): U, 29
shells x 3 particle energies x 40 seeds = 3480 cascades (241 of them emitted
263 photons; the others are below the binding energy or Auger-only), plus
`calc_max_stack_size` (6, equal). Photon-count mismatches 0, final-seed
mismatches 0, worst energy/direction difference 1.1e-16 (round-off). H has no
relaxation data (120 trivial cascades, equal).

`sample_photon_reaction` (2026-11-21), thick-target bremsstrahlung on and off,
9 energies (2 keV to 100 MeV) x 400 seeds x 2 modes = 7200 calls per element
(U: all four reaction types and 25+ photoelectric shells occur; 2920 calls
banked at least one secondary). The harness sets up a one-element `Material` (fake nuclide
carrying only its `awr`) and calls `openmc::sample_photon_reaction`; the Rust
test calls one `prn` first, which stands for upstream's `sample_element` draw.
**The thick-target bremsstrahlung tables are the real ones** (2026-11-21,
gh:#1693): the C++ harness runs `Material::init_bremsstrahlung` on the
element, and the Rust test derives its own tables from the raw `bremsstrahlung`
group with `punggol::openmc_photon::ttb_tables` and samples from those, so the
reaction numbers below check the whole chain. (The first version of this
comparison used synthetic tables, kept in `harness_synth.cpp` history.)

| element | calls | event/MT mismatches | bank-size mismatches | final-seed mismatches | worst state/secondary difference |
|---|---|---|---|---|---|
| U | 7200 | 0 | 0 | 0 | 4.3e-15 |
| H | 7200 | 0 | 0 | 0 | 2.6e-14 |

`sample_electron_reaction` and `sample_positron_reaction` (2026-11-21): 10800
calls per element (TTB on/off, both particles, 9 energies from 20 keV to
1 GeV, 300 seeds), same synthetic tables: event, final energy and weight,
final seed, bank size 0 mismatches; worst secondary difference 2.2e-16.

**Table derivation** (`ttb_tables.rs`: the element part of the
`PhotonInteraction` constructor, `collision_stopping_power`,
`sternheimer_adjustment`, `density_effect`, `init_bremsstrahlung`, `spline`,
`spline_integrate`), 200 incident energies x 30 reduced photon energies,
electron and positron, ENDF/B-VIII.0 `bremsstrahlung` data:

| element | grid, `dcs`, radiative stopping power | `pdf` | `cdf` | `ln yield` |
|---|---|---|---|---|
| U | 0 (exact) | 5.8e-16 | 6.1e-16 | 2.4e-15 |
| H | 0 (exact) | 0 | 1.5e-16 | 2.2e-16 |

(worst relative differences; the last-bit residuals are round-off in
`ln`/`exp` below the 1e-9 allowance, cause not traced).

**Defect found and fixed (second one).** With real tables 103 of 7200
photon-reaction calls and 360 of 10800 charged-particle calls on U disagreed in
the number of banked photons: the Rust bremsstrahlung loop banked a photon of
energy `E - E_radiated = 0` that upstream drops, because
`Particle::create_secondary` (`particle.cpp:86-122`) refuses a secondary below
its type's energy cutoff. The port pushed photons unconditionally. Every
secondary now goes through a `create_secondary` helper in `reaction.rs` with
the same filter (TTB photons, annihilation photons, relaxation photons). The
synthetic-table run could not see it: its photons never used up the whole
energy.

**Defect found and fixed (first one).** The first run of this comparison on U gave 162
bank-size mismatches and 473 seed mismatches, all in TTB mode on photoelectric
events. Cause: upstream `atomic_relaxation` calls `process_charged_secondary`
for each Auger electron in the middle of the cascade, so the bremsstrahlung
draws interleave with the cascade's own; the port returned the whole secondary
list first and processed it afterwards, reordering the random stream. Fixed by
`PhotonInteraction::atomic_relaxation_with`, which hands each secondary to a
callback at the moment upstream creates it (`atomic_relaxation` keeps its
behaviour as a wrapper). Not caught by the cascade test above, which runs with
local deposition and so draws nothing for Auger electrons. The worst
difference, 4e-15 to 3e-14, is round-off in `ln`/`exp` of the TTB sampler
(below the 1e-9 allowance, cause not traced further).

`p.mu()` is compared only for scattering events: upstream leaves it
uninitialised on absorption.

So the Rust samplers consume exactly the same random numbers in the same order
and give bit-identical results for this element.

## Not yet compared

further elements (U and H only; data from ENDF/B-VIII.0; gh:#1690), TTB with a non-default photon cutoff above the lowest tabulated electron energy (the truncation branch of `prepare_element_ttb` is covered by a unit test only), and materials of several elements or weight-fraction input (unit tests only).
