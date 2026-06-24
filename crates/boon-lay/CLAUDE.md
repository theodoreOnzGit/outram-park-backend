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
| `rand` / `rand_core` / `rand_distr` | Normal distribution for Gaussian diffusion steps |
| `serde` / `serde-xml-rs` | Deserialise the ENDF-8 XML into `SerdeNuclideData` structs |
| `anyhow` | Error propagation in XML parsing |

---

## Source copy checklist

Copy each directory from `../boon-lay/src/` into `crates/boon-lay/src/`:

```
../boon-lay/src/
  prelude/mod.rs                                    → src/prelude/mod.rs
  decay_xml_info_serde/mod.rs                       → src/decay_xml_info_serde/mod.rs
  nuclide_reaction_and_decay_data/                  → src/nuclide_reaction_and_decay_data/
    mod.rs
    get_decay_info/mod.rs
    get_decay_info/tests.rs
    decay_library/mod.rs
    decay_library/get_random_number.rs
    decay_library/indexing_using_nuclide.rs
    decay_library/tests.rs
    parse_nuclides_to_decay_data.rs
    alkali_metals_and_hydrogen.rs
    alkaline_earth_metals.rs
    transition_metals_test.rs
    boron_group_test.rs
    carbon_group_test.rs
    pnictogens_test.rs
    chalcogens_test.rs
    halogens_test.rs
    noble_gases_test.rs
    lanthanides_test.rs
    actinides_test.rs
    heavier_than_actinides.rs
  lagrangian_decay_simulator/                       → src/lagrangian_decay_simulator/
    mod.rs
    stochastic_decay_chain/mod.rs
    stochastic_decay_chain/iterator_for_decay_chain.rs
    monte_carlo_single_radionuclide_decay_simulator/mod.rs
    monte_carlo_single_radionuclide_decay_simulator/postprocessing.rs
    monte_carlo_single_radionuclide_decay_simulator/tests.rs
    lagrangian_diffusion/mod.rs
    lagrangian_diffusion/central_limit_theorem/mod.rs
    lagrangian_diffusion/central_limit_theorem/oorandom_rng.rs
    lagrangian_diffusion/chatgpt_5_diffusion_*.rs  (4 files)
    lagrangian_diffusion/isotropic_scattering.rs
    lagrangian_diffusion/triso_particle_widget.rs
    lagrangian_diffusion/single_particle_simulator/mod.rs
    lagrangian_diffusion/single_particle_simulator/cached_normals.rs
    lagrangian_diffusion/single_particle_simulator/release_fraction_analytical_solution.rs
    lagrangian_diffusion/single_particle_simulator/release_fraction_crp_6_case_1a_1b.rs
    lagrangian_diffusion/single_particle_simulator/release_fraction_crp_6_case_1a_1b/
      monte_carlo_test.rs
      simulation_code.rs
    lagrangian_diffusion/single_particle_simulator/constructive_solid_geometry/
      mod.rs  chatgpt_vibe_coded_sphere_crossing.rs  norms.rs  sphere.rs
    lagrangian_diffusion/single_particle_simulator/interaction_with_decaying_nuclide_simulator/
      mod.rs  tests_for_auto_timestepping.rs
    lagrangian_diffusion/single_particle_simulator/movement_within_triso_particle/mod.rs
    lagrangian_diffusion/temperature_dependent_collisions/mod.rs
    lagrangian_diffusion/temperature_dependent_collisions/diffusion_coeffs/
      mod.rs  cesium_tests.rs  silver_tests.rs  strontium_tests.rs
    tests/mod.rs
  lagrangian_transmutation_and_fission_simulator/mod.rs  → src/lagrangian_transmutation_and_fission_simulator/mod.rs
```

Also copy the examples:
```
../boon-lay/examples/boon_lay_decay_simulator/main.rs  → examples/boon_lay_decay_simulator/main.rs
../boon-lay/examples/triso_simulator/main.rs           → examples/triso_simulator/main.rs
```

---

## Migration required after copying

### 1. uom 0.37 → 0.38 (likely zero changes)

The standalone crate uses `uom = "0.37.0"`; the workspace pins `0.38.0`.
Boon-lay only uses `Time`, `Energy`, `Ratio` — all of which have unchanged
APIs in 0.38.  Expect no compile errors from the version bump.

If errors appear, compare the uom 0.36→0.38 migration notes in the workspace
`CLAUDE.md` (the main change was a type inference tightening around `Quantity`
arithmetic; usually fixed by adding an explicit `.get::<unit>()` call).

### 2. egui 0.29 → 0.34 (examples only)

Both examples use `egui + eframe + egui_plot` at 0.29. Apply the same migration
pattern used for the other crates (documented in workspace `CLAUDE.md`):

- Rename `fn update(&mut self, ctx, frame)` → `fn ui(&mut self, ui, frame)`;
  add `let ctx = ui.ctx();` at the top.
- `egui_plot::Line::new(series)` → `Line::new("label", series)`.
- `TopBottomPanel`/`SidePanel` → `Panel::top/bottom/left/right`.
- `ScrollArea::drag_to_scroll(true)` → `scroll_source(ScrollSource::ALL)`.
- Replace `eframe` feature `"glow"` → `"wgpu"` (already done in workspace
  `Cargo.toml`).

### 3. Edition 2021 (was 2024)

The standalone crate used `edition = "2024"`. The workspace default is `"2021"`
which this scaffold inherits via `edition.workspace = true`.

Rust 2024 edition adds: precise captures in `use<..>`, `gen` blocks,
`unsafe_op_in_unsafe_fn` lint. If any source file relies on 2024-only
behaviour (most likely `use<..>` in closures), add `edition = "2024"` back
explicitly to `Cargo.toml` here — one crate can differ from the workspace
edition without issue.

### 4. Missing `openmc-endf-8-depletion-lib-a`

The standalone boon-lay only depended on `-lib-b` (not `-a`). Both are in the
workspace already (`openmc-endf-8-depletion-lib-a` and `-b`).  `Cargo.toml`
here only includes `-b`; add `-a` if a source file imports it.

---

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
```

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
cargo test -p boon-lay --lib --tests
```

---

## Planned future work

- `lagrangian_transmutation_and_fission_simulator` — full transmutation matrix
  under neutron flux; fission fragment Lagrangian tracking.
- Coupling to `openmc-libs` for neutron flux maps that drive the transmutation
  rates.
- Real-time 3-D TRISO diffusion visualisation (extends `boon_lay_decay_simulator`
  example).
