# CLAUDE.md — outram-mc-libs

Pure-Rust port of the OpenMC Monte Carlo neutron transport kernels.

The reference C++ source lives at:
`/home/teddy0/Documents/research/openmc/`

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: k-eff within **500 pcm** of the ICSBEP Godiva
  bare-HEU-sphere benchmark (HEU-MET-FAST-001), reconstructed from an ENDF
  evaluation rather than a pre-built ACE library. Evidence class: **cross-code
  comparison** (this crate is a port of OpenMC's kernels), supported by unit
  tests and internal consistency.

  Measured at declaration: **k_eff = 0.99659 ± 0.00300, i.e. −341 pcm**, via
  `examples/endf_to_keff.rs` reading `n-092_U_235`/`n-092_U_238` from disk. **286 tests pass** (13 ignored).

  The bar is set at 500 pcm because that is what the crate demonstrably
  achieves today, not because 500 pcm is a good criticality tolerance — it is
  not. Expect this to tighten once the scatter matrix and unstructured-mesh
  tallies land.


**Upstream license:** OpenMC is MIT-licensed. This Rust port is GPL-3.0-only
per the workspace default; the port constitutes new copyrightable expression.

## Standing goal: OpenMC-like API + notebooks-as-verification-tests (MANDATORY)

`outram-mc-libs` should **eventually function API-wise like OpenMC** (mirror its
Python / `capi` surface in idiomatic Rust), and **every notebook in
https://github.com/openmc-dev/openmc-notebooks becomes a verification test** for
the OUTRAM PARK Monte Carlo path. This is a durable direction, not a one-off.

- **This crate owns** the transport / geometry / tally / depletion /
  variance-reduction notebooks: `pincell`, `hexagonal-lattice`, `triso`,
  `candu`, `cad-based-geometry`, `unstructured-mesh-part-i/ii`,
  `tally-arithmetic`, `tally-power-normalization`, `expansion-filters`,
  `flux-spectrum`, `gamma-detector`, `post-processing`, `pandas-dataframes`,
  `mg-mode-part-i/ii/iii`, `depletion`, `shielded_room_weight_window`, `capi`.
- **njoy-outram-park-fork owns** the data notebooks (`nuclear-data`,
  `nuclear-data-resonance-covariance`, `search`, mgxs/mdgxs generation).
- Approach: a notebook→test→required-API **mapping doc**, then a
  `tests/openmc_notebooks/` harness — tractable notebooks as live tests, the
  rest `#[ignore]` with a documented "requires API X" reason + a per-notebook
  bead. `pincell`/Godiva k_eff (op-u6s.1) is the natural first live case.
- Notebooks are OpenMC-project open-source (MIT) — cite provenance
  (source notebook + commit) per RESPONSIBLE_USE.md; V&V docs state methodology
  **and** measured results. Tracked under beads epic **op-6tz**.

### Reference values + comparison outputs (MANDATORY)

- **For ALL outram-mc comparisons, reference values come from the openmc
  `.ipynb` files themselves** — the k-effective / tally results stored in the
  notebook cell outputs (fetch the raw notebook; cite the notebook + commit).
  Do not invent or approximate a reference; use the number the notebook printed,
  and match the notebook's geometry / material / data as closely as the
  available data allows so the comparison is apples-to-apples.
- **Write each comparison as a CSV** to
  `verification_and_validation/openmc_notebook_comparisons/` (one CSV per
  notebook; columns: date, notebook, case, our k ± σ, reference k ± σ, Δk (pcm),
  combined σ, σ-distance, data used on each side, stats note). That folder is
  **gitignored** — the CSVs are reproducible generated outputs, kept local, not
  committed. The interpretation/write-up still goes in the committed V&V docs +
  the relevant bead.

---

## Porting rule (mandatory) — mirror the canonical source, do not reinvent

**Every transport / physics / geometry behaviour in this crate must be ported
from the canonical OpenMC C++ source at `/home/teddy0/Documents/research/openmc/`
(`src/*.cpp`, `include/openmc/*.h`).** Before implementing anything, grep the
OpenMC source for the corresponding function and mirror its logic — do not
re-derive or reinvent physics that already exists upstream. Cite the reference
`file:line` in the Rust doc comment so a reader can diff against the original.

**Only when a behaviour is genuinely absent upstream** (e.g. the pebble-bed
`delta_tracking` / `stochastic_media` specialization) do you scaffold new parts
and build them out — and mark them clearly as new work, not a port.

Rationale: the crate's entire value is *fidelity* to OpenMC. Reinvented logic
silently drifts from the reference. The C++ is the source of truth; this crate is
a translation of it.

---

## Scope

### In scope
| Module | C++ source | What it does |
|---|---|---|
| RNG | `src/random_lcg.cpp` | LCG with O(log n) jump-ahead for particle splitting |
| Distributions | `src/random_dist.cpp` | Maxwell, Watt, tabulated samplers |
| Geometry / position | `include/openmc/position.h` | 3-D position and direction vectors (cm) |
| Geometry / surfaces | `src/surface.cpp` | Quadric CSG surfaces + distance/sense |
| Geometry / cells | `src/cell.cpp` | Boolean RPN region evaluation |
| Geometry / universes | `src/universe.cpp` | Universe nesting hierarchy |
| Geometry / lattices | `src/lattice.cpp` | Rect + hex lattice indexing |
| Geometry / geometry | `src/geometry.cpp` | `locate_particle`, `distance_to_boundary` |
| Particle state | `src/particle.cpp` | Phase-space state (r, u, E, wgt, seed, …) |
| Particle bank | `src/bank.cpp` | Fission site banking for k-eigenvalue |
| Material | `src/material.cpp` | Nuclide mixture, macroscopic XS |
| Nuclide XS | `src/nuclide.cpp` | Point-energy grid + log-log interpolation |
| Reactions | `src/reaction.cpp` | MT table, Q-value, secondary sampling |
| S(α,β) thermal | `src/thermal.cpp` | Thermal scattering law tables |
| Source sampling | `src/source.cpp` | External source: spatial/energy/angle |
| Tallies | `src/tallies/tally.cpp` | Filter composition + accumulator |
| Tally filters | `src/tallies/filter_*.cpp` | Cell, energy, material, universe, mesh |
| Scoring | `src/tallies/tally_scoring.cpp` | Flux, reaction rate, current accumulation |
| Transport loop | `src/physics.cpp` | `collision()`, `transport_history_based()` |
| Scattering | `src/physics_common.cpp` | Elastic, inelastic, CM-frame kinematics |
| Fission | `src/physics.cpp` | ν sampling, fission bank creation |
| Multigroup | `src/physics_mg.cpp` | Group-averaged cross-section transport (stub — pending) |
| Depletion | `src/chain.cpp`, `openmc/deplete/` | **Implemented** — CRAM `exp(A·dt)` burnup, `DepletionChain`, transmutation matrix, one-group operator (`src/depletion/`: `chain.rs`, `cram.rs`, `matrix.rs`, `operator.rs`); live one-group burnup test vs the `depletion` notebook |

### Out of scope (will NOT be ported)
- **ENDF nuclear data parsing** — `src/endf.cpp`, `include/openmc/endf.h`
- **HDF5 I/O** — cross-section library loading; data arrives pre-loaded
- **XML configuration parsing** — `src/xml_interface.cpp`
- **CMFD accelerator** — `src/cmfd_solver.cpp`
- **Random ray extension** — `src/random_ray/`
- **Photon/electron transport** — `src/photon.cpp`
- **Python/ctypes C API** — `openmc/lib/` Python package
- **Geometry overlap checker** — `src/geometry_aux.cpp` (overlap detection only; the core intersection logic is in scope)

---

## Design decisions

### Units: raw `f64`, not `uom`
Unlike `outram-foam-basic-lib` (which uses `uom` for thermophysics), this crate uses
plain `f64` throughout the inner transport loop.  Monte Carlo simulates billions
of particle histories; a single neutron transport simulation may call
`distance_to_boundary` and `xs_at_energy` O(10⁸) times.  `uom` quantity wrappers
add zero-cost abstraction in principle, but in practice the compile-time overhead
and ergonomic friction in deeply nested loops is not worth it.

Documented unit conventions (enforced by naming, not types):
| Quantity | Unit |
|---|---|
| Length | cm (OpenMC default) |
| Energy | eV |
| Cross-section | barn = 1 × 10⁻²⁴ cm² |
| Macroscopic XS | cm⁻¹ |
| Atom density | atoms / barn·cm |
| Temperature | eV (1 eV ≈ 11604 K) |
| Particle weight | dimensionless (1.0 = fully weighted) |

### No HDF5 dependency in this crate
Cross-section data is loaded externally and passed in by value or reference.
This crate is pure algorithmic: no file I/O, no XML, no HDF5.

### Neutron-only initially
Photon and electron physics (`src/photon.cpp`) are deferred.  The `ParticleType`
enum reserves slots for them, but only `Neutron` transport is implemented.

### Parallelism: per-particle RNG streams
OpenMC's reproducibility guarantee relies on each particle having a completely
independent LCG stream obtained by jump-ahead.  This Rust port preserves that
design: `init_seed(id, offset, master)` derives a unique starting seed for each
particle.  The jump-ahead in `future_seed(n, seed)` is O(log n), implemented in
`src/rng/lcg.rs`.

### RNG goal: statistical correctness, NOT particle-for-particle parity

**Maintainer's decision, 2026-08-06.** This crate does **not** need to reproduce
OpenMC's random number sequence draw-for-draw. What it needs is for the
**statistics to be right**.

That distinction decides how RNG work is justified and tested here:

- **Do not** treat "our uniforms differ from OpenMC's" as a defect in itself, and
  do not add tests that pin our output to OpenMC golden values as an end in
  itself. A converged result agreeing with OpenMC *within statistics* is the
  standard, not bitwise agreement.
- **Do** treat statistical quality as a hard requirement. The generator must
  behave like a good uniform source in the ways Monte Carlo transport actually
  depends on — equidistribution, and no exploitable structure in the *tuples* a
  history consumes (position, then direction, then energy come from consecutive
  draws, so k-tuple structure matters, not just single-draw uniformity).
- **Do** keep the *stream separation* guarantees. Independence between particle
  streams is a statistical property, not a parity one, and it is the thing that
  makes a reported uncertainty mean anything. See `op-rbo`: a defect there left
  neighbouring histories reading near-identical streams, which barely moved the
  central value but made the quoted sigma meaningless.

This does **not** relax the porting rule above. Mirroring OpenMC remains the
default for *physics* — geometry, kinematics, cross-section treatment — because
fidelity is this crate's whole value. The exemption is narrow and applies to the
**bit-level output of the RNG only**, where matching upstream is a means to
statistical quality rather than the goal itself.

Practical consequence: where OpenMC's RNG design exists *for* statistical
quality — as the PCG output permutation does, see `op-jis` — port it, and gate it
with statistical tests rather than golden-value comparisons.

---

## Port reference (read on demand)

The full Rust-module → OpenMC C++ source map, the bottom-up porting order (with
per-module implementation status), and the prioritised test backlog all live in
**`docs/port-reference.md`**.

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p outram-mc-libs --lib
cargo test  -p outram-mc-libs --lib --release
```

## Porting workflow (mandatory)

After implementing any module, update `src/prelude.rs` with new public items,
then `cargo check -p outram-mc-libs` to verify.
