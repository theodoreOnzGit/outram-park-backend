# CLAUDE.md — boon-lay

**BOmbardment of neutrons On Nuclides with Lagrangian transport and transmutation
Yields** — Lagrangian Monte Carlo radionuclide transport for TRISO fuel particles
in HTGRs and FHRs.

The standalone source lives at:
`/home/teddy0/Documents/research/boon-lay/`

**Version:** 0.2.1  
**License:** GPL-3.0 (same as workspace default)

---

## What this crate does

Simulates fission product behaviour in TRISO particles from a **Lagrangian**
(particle-tracking) perspective rather than a continuum Eulerian approach.
Covers:

1. **Decay chains** — stochastic radioactive decay; each simulated atom walks
   its decay chain until it reaches a stable nuclide.
2. **Transmutation** — neutron bombardment producing daughter nuclides
   (e.g. Xe-135 → Cs-135 under n-capture).
3. **Lagrangian diffusion** — individual atoms diffuse through TRISO SiC and
   PyC layers modelled as concentric spherical shells using CSG geometry.
4. **Release fraction** — fraction of fission products that escape the TRISO
   particle, benchmarked against the IAEA CRP-6 Case 1a/1b analytical solution.

### Key external crates (from crates.io)

| Crate | Role |
|---|---|
| `fission-yields-data` | `Nuclide` enum covering ~3000 nuclides; boon-lay re-exports it |
| `openmc-endf-8-depletion-lib-b` | ENDF/B-VIII.0 depletion chain XML data (half-lives, decay modes, Q-values) |
| `oorandom` | Simple fast RNG for decay-chain sampling |
| `outram-mc-libs` | RNG (LCG + Normal + Exp distributions) — replaces `oorandom`, `rand`, `rand_core`, `rand_distr` |
| `serde` / `serde-xml-rs` | Deserialise the ENDF-8 XML into `SerdeNuclideData` structs |
| `anyhow` | Error propagation in XML parsing |

---

## Porting status

Source copy, RNG/uom/egui/edition migration, and both example ports are
**complete** — see `docs/porting-history.md` for the historical checklists.

## Module map

```
src/
  lib.rs
  prelude/mod.rs                          ← re-exports for downstream users
  decay_xml_info_serde/mod.rs             ← serde structs for ENDF-8 XML
  nuclide_reaction_and_decay_data/
    mod.rs                                ← NuclideReactionAndDecayData, DecayData, DecayType
    get_decay_info/                       ← accessor methods on NuclideReactionAndDecayData
    decay_library/                        ← HashMap<Nuclide, NuclideReactionAndDecayData>
    parse_nuclides_to_decay_data.rs       ← XML → struct conversion
    <element_group>_test.rs  (×11)        ← per-element-group data + tests
  lagrangian_decay_simulator/
    stochastic_decay_chain/               ← iterator-based decay chain walker
    monte_carlo_single_radionuclide_decay_simulator/  ← MC half-life verification
    lagrangian_diffusion/
      central_limit_theorem/              ← Gaussian step sampling
      single_particle_simulator/
        constructive_solid_geometry/      ← sphere CSG intersection
        interaction_with_decaying_nuclide_simulator/
        movement_within_triso_particle/
        release_fraction_crp_6_case_1a_1b/  ← CRP-6 benchmark
      temperature_dependent_collisions/
        diffusion_coeffs/                 ← Cs, Ag, Sr diffusion coefficients in SiC/PyC
    tests/
  lagrangian_transmutation_and_fission_simulator/
    mod.rs                                ← empty stub (future work)
  triso_atops_fork/                       ← Eulerian/continuum TRISO release (fork of INL TRISO-ATOPS, MIT, commit de374c8)
    mod.rs                                ← module map + type aliases (DecayConstant, ReleaseFraction)
    nuclide_model/
      mod.rs                              ← TrisoAtopsNuclide, ElementGroup (5 transport groups)
      nuclide_database.rs                 ← 84-nuclide supported table (IAEA half-lives)
    diffusion/
      mod.rs                              ← Arrhenius D(T): kernel/graphite/SiC-Ag + ∫D dt (integrate)
    release_models/
      mod.rs                              ← rb_fail + release_fraction_transient dispatchers (by ElementGroup)
      steady_state.rs                     ← Booth (long/short), breakthrough, attenuation, noble-gas <R/B>
      transient.rs                        ← accident variants: booth_transient, breakthrough_transient, rf_graph
    activities/mod.rs                     ← SCAFFOLD (activity bookkeeping; bead op-b4a.2.2)
    normal_operation/mod.rs               ← SCAFFOLD (nodal orchestration + JSON driver; beads op-b4a.2.2/.2.3)
```

## triso_atops_fork — Eulerian TRISO release (fork of INL TRISO-ATOPS)

A Rust fork of INL's MIT-licensed TRISO-ATOPS providing the **Eulerian /
continuum-diffusion** release model (closed-form Booth/breakthrough/attenuation
release fractions) as the complement to boon-lay's Lagrangian model. Provenance:
`LICENSE.triso-atops`, `NOTICE.triso-atops`, per-file headers; upstream clone at
`upstream_source/TRISO-ATOPS/` is gitignored/reference-only. The GUI was
intentionally not ported (headless-library + Android rule). The cleanly-
dimensioned physics core (diffusion + release models + nuclide model) is ported,
uom-typed, and verified; the activity/nodal/run-file layer is scaffolded pending
a dimensional-analysis pass (its upstream units mix atoms/Ci/Bq). Full details,
Python→Rust module map, and V&V results: **`docs/triso-atops-fork.md`**.

---

## Test coverage notes

- `nuclide_reaction_and_decay_data/<element>_test.rs` — each checks that the
  parsed half-life and decay mode for representative nuclides matches ENDF/B-VIII.0.
- `monte_carlo_single_radionuclide_decay_simulator/tests.rs` — verifies that
  the MC-simulated half-life (N=10000 histories) matches the tabulated value
  within ~5%.
- `release_fraction_crp_6_case_1a_1b/monte_carlo_test.rs` — compares MC
  release fraction to the IAEA CRP-6 analytical solution.
- `lagrangian_diffusion/temperature_dependent_collisions/diffusion_coeffs/
  cesium_tests.rs`, `silver_tests.rs`, `strontium_tests.rs` — validate diffusion
  coefficient correlations against literature data.

Run the test suite with:
```bash
cargo test -p boon-lay --lib --tests --release
```

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

## Planned future work

### `lagrangian_transmutation_and_fission_simulator`

**No burnup matrix needed.** The whole point of the Lagrangian MC approach is
that you track individual atoms stochastically, so the Bateman ODE system
(`dN/dt = A·N`) and its large, stiff matrix `A` never appear.  Each simulated
atom just samples waiting times from exponential distributions — the population
distribution emerges from the ensemble.

#### MC transmutation design sketch

Each simulated atom has:
- a current nuclide identity
- a position (for diffusion coupling)

At each timestep `dt`, for each atom compete the following rates:

| Event | Rate λ | On firing |
|---|---|---|
| Radioactive decay | λ_decay = ln(2) / t½ | replace nuclide with decay daughter |
| Neutron capture (n,γ) | λ_ng = φ · σ_ng | replace with capture product |
| (n,2n) reaction | λ_n2n = φ · σ_n2n | replace + spawn one new atom of same nuclide |
| Fission | λ_f = φ · σ_f | remove atom + spawn two fission-fragment atoms sampled from yield distribution |

The total rate is `λ_total = λ_decay + λ_ng + λ_n2n + λ_f`.  Sample a waiting
time `t ~ Exp(λ_total)`.  If `t < dt`, fire the event (chosen by the usual
competing-rates alias method); otherwise the atom survives the step unchanged.

Fission fragment yields come from the ENDF/B-VIII.0 fission yield data already
available via `openmc-endf-8-depletion-lib-b`.

The neutron flux `φ` (scalar or spatially resolved) is an external input —
coupling to `outram-mc-libs` flux maps is the natural integration point.

This design scales linearly with the number of simulated atoms and requires no
matrix exponential, no CRAM solver, and no stiffness handling.

### Other planned items

- Coupling to `outram-mc-libs` for spatially resolved neutron flux maps that drive
  per-region transmutation rates.
- Real-time 3-D TRISO diffusion visualisation (extends `boon_lay_decay_simulator`
  example).
