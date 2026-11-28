# boon-lay

**BOON LAY** — ***BO**mbardment of neutrons **O**n **N**uclides with
**L**agrangian transport **a**nd transmutation **Y**ields*.

Radionuclide behaviour in TRISO fuel particles for HTGRs and FHRs: fission,
decay, transmutation and diffusion, all happening at once during reactor
operation.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

What this crate makes studyable and challengeable is how radionuclides get out
of a TRISO particle, told three ways that can be checked against each other: a
line-by-line Rust fork of INL's TRISO-ATOPS that a reader can set beside the
upstream Python, a port of Wee Zhern Ray's STRATO eigenfunction series for a
layered sphere, and boon-lay's own Lagrangian walkers, which show single atoms
decaying, transmuting and diffusing through the coatings. Alongside them sit
fuel-failure formulas transcribed from the PANAMA-I report and two chemical
rate laws, each with its source and validity range, and a theory book and a
lesson book that derive the models.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Ported upstreams: INL's TRISO-ATOPS (MIT, Battelle Energy Alliance, commit `de374c8`; `LICENSE.triso-atops`, `TRISO_ATOPS_DERIVATION.md`) and STRATO by Wee Zhern Ray (AGPL-3.0-only, commit `36f1a98`; `src/strato/mod.rs`). Transcribed sources: PANAMA-I (Verfondern and Nabielek, Jülich HTA-IB-03/90, 1990) for `fuel_failure`; Wang and Sun (2023) and IAEA-TECDOC-978 Eq. (5-2) for `chemistry`; Liu and Cao (2002) for the HTR-10 per-cycle table; IAEA CRP-6 Case 1a/1b. `crates/kovan-literature/CATALOGUE.md` ties TECDOC-978 to `boon_lay::chemistry::kernel_hydrolysis`. Decay data: ENDF/B-VIII.0 via `openmc-endf-8-depletion-lib-b`. Since 2026-11-28: the BISON Theory Manual (Hales et al. 2013, INL/EXT-13-29930, maintainer's private corpus), whose equations `src/triso_particle_mechanics/` implements from scratch (not BISON; its source was never read). |
| Mathematical equations | `docs/theory/` (an mdBook: fission generation, recoil, layers and chemistry, SiC, random walk, Eulerian FRESCO, the ray equations, pressure-vessel stress, PANAMA), `docs/derivations/`, `docs/triso-atops-derivation.md`, and the module docs (Booth release, Sturm-Liouville eigenfunction series in `src/strato/`, finite-volume multilayer diffusion in `src/triso_atops_extensions/`). `src/triso_particle_mechanics/` module docs: spherical equilibrium and its weak form, the implicit creep step, mean dilatation, the layered Lamé closed form with the `C r ln r` term, steady conduction, the volume Weibull integral. |
| Source code | `src/triso_atops_fork/`, `src/triso_atops_extensions/`, `src/strato/`, `src/lagrangian_decay_simulator/`, `src/lagrangian_transmutation_and_fission_simulator/`, `src/fuel_failure/`, `src/chemistry/`, `src/nuclide_reaction_and_decay_data/`, `src/decay_xml_info_serde/`, `src/gpu.rs`; `boon_lay::rustbca` re-exports `kembangan::rustbca`. `src/triso_particle_mechanics/` (2026-11-28, on `farrer-park`'s FEM layer). |
| Interactive simulations | egui examples `examples/triso_simulator/`, `examples/boon_lay_decay_simulator/` and `examples/first_passage_realtime.rs`; the `dhoby-ghaut` web demos `triso_atops_web`, `lesson_triso_atops` and `lesson_triso_kernel` are built on this crate. |
| Visualisations | `docs/theory/src/img/pools-widget.png`; `examples/triso_cell_slice.rs` draws the assembled TRISO cell; `dev/mhtgr_stoyer_fig67_overlay.py` overlays the paper reproduction. `verification_and_validation/triso_particle_mechanics/particle_mesh.png` and `pyc_shrinkage_stress.png` (from `examples/triso_particle_mechanics.rs`) draw the assembled particle meshes and one stress profile. |
| Verification and validation evidence | `docs/triso-atops-code-to-code.md`: TRISO-ATOPS fork against upstream `de374c8`, 14 130 cases and 51 tests (third pass, 2026-09-21), worst group `accident_case.nodal_kernel` at 2.44e-9 relative. `verification_and_validation/strato_triso_diffusion/` (2026-11-26): the STRATO port bit-identical to upstream on 978 of 978 values. `verification_and_validation/mhtgr_stoyer/` and `docs/triso-atops-2026-paper-reproduction.md`: the `atops2026paper` variant against the published paper's own release numbers (gh:#1015, 2026-10-22). `verification_and_validation/crp6_case1_kernel_release_vs_crank.md`. Full list under "Verification status" below. `verification_and_validation/triso_particle_mechanics/README.md` (2026-11-28): particle mechanics against closed forms, displacement order 2.000, midpoint stress order 1.99, creep first order in fluence; verification only, synthetic inputs, nothing validated. |
| Human reviews and corrections | 163 functions carry a kovan human-review stamp. `docs/human-corrections-to-ai-work.md` has one entry on this crate (2026-10-05, "Every element plates out at one rate", `triso_atops_fork::normal_operation::normal_operation_node`), which led to `triso_atops_extensions`. Struck-through corrections below record the TRISO-ATOPS code-to-code numbers (2026-09-28) and the licence change. |

**Status in one sentence:** the TRISO-ATOPS fork and the STRATO port are
verified code to code against their upstreams; the extensions, the Lagrangian
walkers and the fuel-failure formulas have only reduction or transcription
checks, and nothing is validated against measured release data.

## Two ways of looking at the same problem

Tracking fission products through a TRISO particle with an **Eulerian**
(control-volume) method means solving a burnup matrix. That matrix is large and
stiff, so it's expensive to compute, and it's hard to visualise. BOON LAY
started from the other side, and now carries both:

| Approach | Module | What it is |
|---|---|---|
| **Lagrangian** | `lagrangian_decay_simulator`, `lagrangian_transmutation_and_fission_simulator` | Tracks representative atoms. Each one walks its decay chain, samples competing decay / `(n,gamma)` / `(n,2n)` / fission clocks, and diffuses through the kernel and coating shells. No burnup matrix: the population emerges from the ensemble. Built for watching radionuclide transport in real time, for outreach and open research. |
| **Eulerian / continuum** | `triso_atops_fork` | A Rust fork of INL's **TRISO-ATOPS** (MIT, upstream commit `de374c8`). Closed-form Booth, breakthrough and attenuation release fractions; normal-operation and accident release; coolant activity and source terms for 84 nuclides. |
| **Eulerian, extended** (NOT a port) | `triso_atops_extensions` | boon-lay's own additions to the TRISO-ATOPS model, kept apart from the port, which they do not modify. Today: plate-out and purification rates **per transport group or per element** instead of one `k_plate` and one `k_clean` per reactor, conversions from per-cycle data, and Liu & Cao's (2002) cited HTR-10 per-cycle table (GitHub #583). With upstream's rates it reproduces the port bit for bit on all 84 nuclides. **CORRECTED 2026-11-26:** this row omitted `multilayer_diffusion` (since 2026-11-04, gh:#1209): a cell-centred finite-volume solver of diffusion through every layer of the particle, the numerical reference for the port's closed forms. |
| **Analytical, multilayer** (a port) | `strato` | The exact Sturm-Liouville eigenfunction series for release from a layered sphere, ported from **STRATO by Wee Zhern Ray (`weewzr`)** (<https://github.com/weewzr/STRATO>, AGPL-3.0-only, commit `36f1a98`), `no_std` + `alloc`. It reproduces upstream bit for bit, and is the analytical reference `multilayer_diffusion` is verified against (gh:#1800). Added 2026-11-26. |

The two are complements, not rivals. The Lagrangian model shows *how* atoms
get out. The TRISO-ATOPS fork gives the release fractions that the offsite
chain consumes.

## boon-lay fuel failure (TRISO particle failure)

`fuel_failure` is **boon-lay fuel failure**: boon-lay's own model of how many
TRISO particles fail, through pressure-vessel overstress and SiC thermal
decomposition. It was **coded agentically** (by an AI coding agent, then
reviewed) from the formulas published in the PANAMA-I report (Verfondern &
Nabielek, Jülich HTA-IB-03/90, 1990).

**It is not PANAMA.** This project does not have the PANAMA source code, which
is closed-source and was never consulted. Keep the names apart:

- **PANAMA-I** means the report and the results it prints.
- **boon-lay fuel failure** means this code and every number it computes.

Matching the report's printed tables and figures shows the formulas were
transcribed faithfully. It does not show agreement with the PANAMA code on any
case the report does not print. Its output feeds the TRISO-ATOPS fork's
in-service failure fraction through
`FailureFractions::with_fuel_failure_incremental`, which is meant for accident
transients only (see the module docs for why not for normal operation).

## Chemical attack (added 2026-09-29, GitHub #401)

`chemistry` holds two cited closed-form rate laws for the water-ingress source
term, each with its validity range and a flag when it is left:

- `chemistry::graphite_steam`: IG-110 oxidation by steam, `C + H2O -> CO + H2`,
  using the Boltzmann-enhanced Langmuir-Hinshelwood fit of Wang & Sun (2023),
  Table 8. It is a kinetic-regime fit (850-1100 degC, 0.5-20 kPa steam). The
  reaction enthalpy is +131.3 kJ/mol, from NIST formation enthalpies.
- `chemistry::kernel_hydrolysis`: stored noble gas released when water vapour
  reaches exposed UO2 kernels, from IAEA-TECDOC-978 Eq. (5-2). The fit covers
  820-1040 degC and 2.8-1051 Pa and is clamped to the whole inventory.

Both are transcriptions checked against an independent hand evaluation. They
are not validated against any accident.

## Charged particles: a copy of RustBCA (added 2026-10-16, GitHub #876)

> **MOVED 2026-11-22 (gh:#1699, maintainer direction):** the RustBCA copy,
> its differential test, `examples/rustbca_cascades.rs` and both V&V records
> now live in [`kembangan`](../kembangan/README.md), the materials-damage
> crate. `boon_lay::rustbca` is a re-export of `kembangan::rustbca`, so
> callers are unchanged, and `rustbca-std` here forwards to kembangan's. The
> paths in this section are now under `crates/kembangan/`.

**`rustbca` is a copy of RustBCA, not boon-lay's own code, and boon-lay
re-exports it** as `boon_lay::rustbca`. It is copied from `lcpp-org/RustBCA`
~~v3.0.0~~ (`a356280d`, which is v3.0.0 plus 17 upstream commits, corrected 2026-11-13 gh:#1426; GPL-3.0, Jon Drobny and Davide Curreli) and made `no_std` +
`alloc`. Attribution and every local change are listed in
[`src/rustbca/README.md`](../kembangan/src/rustbca/README.md) (in `kembangan`) and in the module docs.

It is a binary-collision Monte Carlo for fission fragments, alphas and recoil
knock-ons. **Fragment results are illustrative until they are V&V'd.**

`examples/rustbca_cascades.rs` prices one full cascade in an HTR-10 kernel and
its coatings. A fission fragment costs 1.5–2 s on one desktop core. Results
are in
[`verification_and_validation/rustbca_cascades/`](../kembangan/verification_and_validation/rustbca_cascades/README.md) (in `kembangan`).

## `no_std` + `alloc` (gh:#1634)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p boon-lay --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). **Everything that computes builds without
it**: every decay, diffusion, first-passage and transmutation simulator, the
`DecayLibrary` type and its lookups, the RustBCA copy's physics, the fuel
failure models, the TRISO-ATOPS release models and the chemistry rate laws.
What `std` (default) carries:

- ~~**the OpenMC decay data source**: `decay_xml_info_serde`,
  `parse_nuclides_to_decay_data` and `DecayLibrary::new`, which read
  `openmc-endf-8-depletion-lib-b` through `serde-xml-rs` (no `no_std` build).
  Without `std` a `DecayLibrary` has to be built from another source;~~
  **CHANGED 2026-11-22:** the decay data source builds without `std` too,
  since `openmc-endf-8-depletion-lib-a`/`-b` 1.1.0 parse with `roxmltree`
  (no_std); `DecayLibrary::new` works on bare metal;
- the TRISO-ATOPS run-file readers (`triso_atops_fork::run_file`: files,
  `serde_json`) and `serde` itself (RustBCA input files only);
- the rayon thread pool (serial `wasm_par` stand-in without it, as on wasm),
  `available_parallelism` (one core without it), and the wgpu `gpu` module;
- the wall clock: a loop timer reads zero, and `DiffusionRandomCache::new`,
  which seeds from the clock, gets a fixed seed;
- printing, and `std` in `anyhow` and `outram-mc-libs`.

`rustbca-std` implies `std`. The root workspace entry is
`default-features = false`, so a member that wants `std` asks for it. With
`std` on (the default) nothing changes. Three kovan-reviewed functions were
edited mechanically for this, with the maintainer's confirmation
(`std::` paths to `core::`; listed on gh:#1634).

## Where it sits

`boon-lay` supplies the release physics for the offsite chain. `sembawang`
orchestrates it and hands the result to `changi`:

```text
boon-lay (TRISO release)  ->  sembawang (source term)  ->  changi (dispersion, deposition)
```

It depends on `outram-mc-libs` for random numbers. The exception is the
RustBCA copy, which keeps upstream's ChaCha8 generator, ~~from `rand`~~ since
2026-10-20 from `petir::rng`, a draw-for-draw port of it (gh:#995). Decay data are
ENDF/B-VIII.0, via the `openmc-endf-8-depletion-lib-b` crate.

## Verification status

- **TRISO-ATOPS fork: verified code-to-code against upstream.** ~~5 699 cases
  across 32 function groups, all passing (2026-09-15, upstream `de374c8`). Every
  function agrees to between exact equality and 4.2e-10 relative, outside the
  ill-conditioned inputs analysed in the write-up. A second pass on 2026-09-21
  covered the accident path.~~ **CORRECTED 2026-09-28** — those were the first
  pass's numbers (2026-09-15, upstream `de374c8`: 5 699 cases, 32 function
  groups, exact equality to 4.2e-10 relative outside the ill-conditioned
  inputs). Two further passes on 2026-09-21, against the same upstream, grew the
  fixture to **11 855 cases** (second pass, the accident path) and then to
  **14 130 cases** and **51 tests** (third pass, exhaustive widening and the
  end-to-end `accident_case` composition). The third pass's worst group is
  `accident_case.nodal_kernel` at **2.44e-9** relative (180 cases), above the
  `1e-9` group tolerance and admitted only by the per-case conditioning widening
  (an Ag-110m row). Methodology, results and the two deliberate
  divergences from upstream are in
  [`docs/triso-atops-code-to-code.md`](docs/triso-atops-code-to-code.md).
- **TRISO-ATOPS extensions (not a port; 2026-10-05, GitHub #583):** no
  upstream to compare with, so each extension is checked by reducing to the
  port. With upstream's rates, `normal_operation_node_with_rates` equals the
  ported `normal_operation_node` bit for bit on all 84 supported nuclides, at
  700 °C and 1100 °C, with purification on and off and parent pools chained
  (336 cases). The element-dependent rates themselves are not validated, and
  the HTR-10 cycle time and purified flow fraction they need are not sourced.
- **Multilayer diffusion against STRATO (2026-11-26, gh:#1800):** the
  `strato` port reproduces upstream STRATO bit for bit (978 of 978 values),
  and matches a homogeneous-sphere closed form (the same formula as
  upstream's own test oracle) and the ported
  ATOPS Booth series to round-off. `multilayer_diffusion` converges to it at
  second order in space and first order in time on equal, two-layer,
  five-layer and contrast (10^±6) cases, as does STRATO's own finite volume,
  run unmodified. On layered cases the series and the FV corroborate each
  other; neither is an independent reference for the other yet. A first run
  failed its spatial-order gate because of the
  test protocol; that failure is recorded with its diagnosis. Upstream
  TRISO-ATOPS's `breakthrough_model` is a different problem (one planar
  layer, infinite source) and is not covered. Plan, results and limitations:
  [`verification_and_validation/strato_triso_diffusion/`](verification_and_validation/strato_triso_diffusion/README.md).
- **Lagrangian simulator:** unit-tested. Monte Carlo half-lives are checked
  against ENDF/B-VIII.0, and the release fraction against the IAEA CRP-6
  Case 1a/1b analytical solution. There is no cross-code comparison yet.

Neither is **validated** against measured release data.

## Running

```bash
cargo test --release -p boon-lay --lib --tests
cargo run  --release -p boon-lay --example first_passage_realtime
cargo run  --release -p boon-lay --example triso_simulator
cargo run  --release -p boon-lay --example boon_lay_decay_simulator
```

The egui examples fall under the workspace headless-mode rule; see
`crates/outram-park-digital-twin-engine/CLAUDE.md`.

## Licence

~~GPL-3.0~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973). ~~As a combined work: the copied-in RustBCA (`src/rustbca/`) stays GPL-3.0-or-later and `LICENSE.GPL-3.0` ships beside `LICENSE`.~~ **CHANGED 2026-11-22:** RustBCA moved to `kembangan` (gh:#1699), and every file here now carries `AGPL-3.0-only` (own work, or ports of MIT-licensed TRISO-ATOPS with its notice kept); see `docs/relicensing-agpl-2026-10-20.md`, addendum 2026-11-22. The TRISO-ATOPS fork is derived from MIT-licensed code by Battelle
Energy Alliance (INL). Its terms are in [`LICENSE.triso-atops`](LICENSE.triso-atops)
and [`NOTICE.triso-atops`](NOTICE.triso-atops), and the derivation is recorded in
[`TRISO_ATOPS_DERIVATION.md`](TRISO_ATOPS_DERIVATION.md). MIT into ~~GPL-3.0~~ **AGPL-3.0-only** is one-way.
`src/strato/` ports part of **STRATO, by Wee Zhern Ray (GitHub `weewzr`)**,
<https://github.com/weewzr/STRATO>, which is AGPL-3.0-only like this crate;
its provenance and local changes are in `src/strato/mod.rs`.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
