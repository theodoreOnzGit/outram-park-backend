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

**Photon cutoff above the lowest tabulated electron energy** (the truncation
branch; harness argument `photon_cutoff`, 2026-11-21), U:

| `settings::energy_cutoff[photon]` | incident energies kept | table differences (grid, `dcs`, radiative stopping power) | `pdf` / `cdf` / `ln yield` | atomic relaxation, 3480 cascades | reaction samples (7200 + 10800) |
|---|---|---|---|---|---|
| 1 keV (default, no truncation) | 200 | 0 | 5.8e-16 / 6.1e-16 / 2.4e-15 | 0 mismatches | 0 mismatches, worst 1.0e-13 |
| 5 keV | 177 | 0 | 5.8e-16 / 6.8e-16 / 7.2e-16 | 0 mismatches | 0 mismatches, worst 5.0e-14 |
| 50 keV | 144 | 0 | 5.8e-16 / 6.3e-16 / 1.5e-15 | 0 mismatches | 0 mismatches, worst 4.3e-15 |

A photon below the cutoff is killed by `sample_photon_reaction` before
`sample_element` draws its number and without setting an event (upstream
leaves `event` and `event_mt` unset), so the test draws nothing for those and
checks only `E = 0`, `wgt = 0`.

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

## Material of two elements, weight-fraction input (2026-11-21)

`verification_and_validation/openmc_photon/harness_multi.cpp` loads U and H
into `data::elements`, builds two `openmc::Material`s of both elements and
dumps what `Material::init_bremsstrahlung` produces: (a) atom densities
(U 0.0225, H 0.0675 atom/b-cm) and (b) **weight fractions** (negative
`atom_density_` entries, -0.9 and -0.1, the branch `collision_stopping_power`
takes). It then runs `sample_element` and `sample_photon_reaction` on material
(a) for 9 energies x 400 seeds x TTB on/off = 7200 calls. The Rust side is
`tests/openmc_photon_multi_element_vs_openmc.rs`
(`OPENMC_REFERENCE_DUMPS=U.dump,H.dump`, `OPENMC_REFERENCE_MULTI_TABLES`,
`OPENMC_REFERENCE_MULTI_SAMPLES`) and the new `reaction::sample_element`
(`physics.cpp:558-588`, ported here; `sample_photon_reaction` still takes the
chosen element as an argument).

| quantity | result |
|---|---|
| shared electron-energy grid, log grid | 0 (exact) |
| TTB `pdf` / `cdf` / `ln yield`, atom densities | 0 (exact) |
| TTB `pdf` / `cdf` / `ln yield`, weight fractions | 1.2e-15 / 6.7e-16 / 3.6e-15 |
| `sample_element` choice | 0 mismatches in 7200 (U 7176, H 24) |
| event/MT, bank size, final seed | 0 / 0 / 0 mismatches |
| worst state/secondary difference | 4.3e-15 (round-off in `ln`/`exp`) |

H is chosen in only 24 of 7200 calls because U dominates the macroscopic cross
section at these densities; the weight-fraction branch is exercised in the
tables only (upstream converts weight fractions to atom densities before
tracking, so sampling with negative densities has no meaning).

## Not yet compared

Further elements beyond U and H (C, O, Si, Fe, Pb, ...): ENDF/B-VIII.0
`photoat` and `atrelax` tapes for them are not available in this session;
data request filed as gh:#1695. Nothing else in `punggol::openmc_photon` is
left uncompared.

## Phase 3: review pass (gh:#1685, 2026-11-21)

One reading pass over what phase 2 did not reach (the handover on gh:#1685),
against OpenMC `8f79d9d0` (`src/`, `include/openmc/`, and the three Python
files `decay_spectrum` ports) read from a fresh sparse clone. No passing
comparison was re-run, and none could be: the OpenMC build and the U/H dumps
were lost with scratch. The one behavioural fix (the cutoff defaults, below)
cannot change any phase 2 number: see its entry. `cargo test --release -p
punggol --lib --tests`: all pass (258 lib tests, 8 integration).

### What was read

| Item (from the handover) | Read against | Finding |
|---|---|---|
| Elements other than U and H: no Compton profiles, no relaxation tables, conduction electrons | `photon.cpp:160-262` (constructor), `:924-997` (`atomic_relaxation`), `material.cpp:526-621, 1197-1318` | the port's branches match: `has_atomic_relaxation` gates relaxation and the photoelectric binding energy; `subshell_map` is used only when relaxation data exist (`physics.cpp`, Compton branch); `collision_stopping_power`, `sternheimer_adjustment` and `density_effect` match line for line, including the `n_conduction == 0` (non-metal, `beta_0`) and `> 0` paths |
| The 7-deep vacancy stack | `photon.h:112`, `photon.cpp:210-214` | same constant; upstream warns at load when an element needs more, and the port's `calc_max_stack_size` is the same check (unit-tested) |
| Transition probabilities | `photon.cpp:180-200` | upstream normalises them at load; the port takes them as given. Now said on `Transition::probability`. The port clamps `i_trans` to the last transition where upstream would index one past the end (both only when the sum is below the draw) |
| Weight-fraction input at the material level | `physics.cpp` `sample_element`, `Material::normalize_density` | the port's `sample_element` takes atom densities and does not convert; now said in its doc. Only the TTB builder takes negative entries, as upstream |
| Photon cutoffs other than 1, 5, 50 keV; `PhotonSettings` | `settings.cpp:67, 113-114, 750-775`, `particle_type.h:152-165` | **defect found: electron and positron cutoff defaults** (below) |
| Correct-physics defaults | `settings.cpp:67, 113` | TTB on, atomic relaxation on, pinned by `correct_physics_is_default`, with `without_ttb` and `without_atomic_relaxation` |
| Ordering of secondaries (the class of the two phase 2 defects) | `physics.cpp` Compton, photoelectric, pair branches; `sample_electron_reaction`, `sample_positron_reaction`; `bremsstrahlung.cpp:34-60` | the order of draws and of `create_secondary` calls matches in every branch; TTB returns before drawing when `E` is below the photon cutoff, as upstream |
| `pair_production`'s `r[Z_]` | `photon.cpp:802-889` | **confirmed upstream defect**: the PENELOPE table starts at Z = 1 and is read as `r[Z_]`, so it uses element Z + 1's radius, and Z = 99 reads past the end. The port reproduces it (Z = 99 clamped) for code-to-code identity. Flagged for the maintainer |
| Particle split | `crates/*/Cargo.toml` | holds: `punggol` depends on `outram-mc-libs`; nothing depends the other way |

### What was fixed

1. **`PhotonSettings::new()` electron and positron cutoffs: 1000 eV, now
   0.** Upstream's `energy_cutoff {0.0, 1000.0, 0.0, 0.0}`
   (`settings.cpp:114`) is indexed neutron, photon, electron, positron; only
   photons default to 1 keV. The port's doc cited `settings.cpp:113` for 1 keV
   on all three. Effect: sub-keV Compton and Auger electrons were left off the
   charged-secondary list (the hand-off to `punggol::egsnrc`), and, with a
   photon cutoff below 1 keV, sub-keV electrons did not radiate.
   **Effect on phase 2: none.** The harness set only the photon cutoff, so the
   reference ran with 0 eV electron cutoffs and the Rust side with 1 keV. At
   the photon cutoffs compared (1, 5, 50 keV) the two cannot differ: a sub-keV
   electron's TTB returns before drawing (`E` below the photon cutoff), and the
   comparison does not include the charged list. Pinned in
   `correct_physics_is_default`.
2. **`TtbTables` without a lifetime parameter:** it borrowed its tables
   (`TtbTables<'a>`); now `Arc<[f64]>` and `Arc<BremsstrahlungData>`, the
   workspace's pattern for shared read-only data. Same values; tests updated.
3. **Attribution header** added to `decay_spectrum.rs` (it ports OpenMC's
   Python and had none); its five line citations checked against `8f79d9d0`.
4. **Stale docs corrected** (struck through): `openmc_photon` module status
   ("not differentially tested"), the `r[Z_]` note ("for phase 2 to
   confirm"), the README table, `lib.rs` status and `SCOPE`, and in
   `crates/outram-mc-libs/AGENTS.md` "Nothing is ported yet".

### What remains untested, and why

- **Elements other than U and H**: data-blocked (gh:#1695). The branches they
  would reach were read and match upstream (table above).
- **The cutoff fix against a built OpenMC**: not run (build lost). A
  differential run with a sub-keV photon cutoff and the charged list
  compared would exercise it.
- **`uom` at the public API: not met** (energies are `f64` eV), as for
  EGSnrc; the same proposal: a `uom` facade where a user calls in. Maintainer
  decision.
- **Downstream heating and KERMA:** nothing to re-measure. No coupled n-γ
  path exists (`outram-mc-libs` does not record photon birth sites for
  `punggol`, and `punggol` supplies no geometry), so no V&V number moved off
  its neutron-only basis; gh:#323 is already closed (not planned, 2026-11-14).
  The D1S and R2S unblock of gh:#1391 needs that path, so it is **not**
  unblocked by this phase.
