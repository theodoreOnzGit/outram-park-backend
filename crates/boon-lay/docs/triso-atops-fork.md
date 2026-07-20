# TRISO-ATOPS fork — `boon_lay::triso_atops_fork`

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## What this is

`boon_lay::triso_atops_fork` is a Rust **fork of Idaho National Laboratory's
TRISO-ATOPS** (TRISO Analysis TOol for Predictive Source terms). It provides the
**Eulerian / continuum-diffusion** TRISO fission-product release model as the
complement to the rest of `boon-lay`, which models the same physics from a
**Lagrangian** (single-atom Monte-Carlo tracking) perspective.

- **Lagrangian (existing boon-lay):** walk individual atoms through the TRISO
  layers with CSG geometry and stochastic diffusion/decay.
- **Eulerian (this fork):** closed-form analytical solutions to the Fickian
  diffusion equation — the Booth equivalent-sphere model, a breakthrough model,
  and a graphite attenuation model — giving per-nuclide release fractions
  directly.

The physics equations originate from the NP-MHTGR New Production Reactor Program
(Anderson et al., *Generic Reactor Plant Description and Source Terms Volume 1*,
EG&G Idaho, 1989); half-lives are from the IAEA Live Chart of Nuclides.

> **Where the physics is derived.** This file is the **module map / provenance /
> V&V** reference. For the *step-by-step derivation* of the release model — from
> `∂C/∂t = D∇²C` and `dN/dt = −λN` up to the assembled source term — see:
> - [`../TRISO_ATOPS_DERIVATION.md`](../TRISO_ATOPS_DERIVATION.md) — the
>   **Python-model view** (each step tied to the upstream Python function), and
> - [`triso-atops-derivation.md`](triso-atops-derivation.md) — the **Rust-port
>   view** (each step mapped to the `triso_atops_fork` module/type/function).

## Provenance & license

| Field | Value |
|---|---|
| Upstream | TRISO-ATOPS — https://github.com/IdahoLabResearch/TRISO-ATOPS |
| Commit | `de374c8` |
| Upstream license | MIT — © 2026 Battelle Energy Alliance, LLC (DOE contract DE-AC07-05ID14517) |
| Authors | Benjamin D. Stoyer, David A. Petti, Alexandra C. Raichart (manual also credits Kyler E. Egan) |
| This fork's license | GPL-3.0 (combined work); MIT notice retained |

Attribution artifacts in the crate:

- `LICENSE.triso-atops` — verbatim upstream MIT license.
- `NOTICE.triso-atops` — INL/Battelle/DOE attribution + authors + NP-MHTGR / IAEA
  source note.
- Per-file provenance headers on every ported `.rs` file (upstream project, URL,
  commit, source `.py`, MIT copyright, GPL-3.0 combined-work note).
- `upstream_source/TRISO-ATOPS/PROVENANCE.md` — reference-clone provenance; the
  clone is gitignored and never compiled.

MIT is GPLv3-compatible, so porting MIT-licensed TRISO-ATOPS into GPL-3.0
`boon-lay` is permitted with attribution retained.

## Module map (Python → Rust)

| Upstream Python | Rust module | Status |
|---|---|---|
| `calculation_functions.py` — `class Nuclide`, `nuclides` dict, `noble_gases`/`halogens`/`special_metals` | `triso_atops_fork::nuclide_model` (`mod.rs`, `nuclide_database.rs`) | **Ported + tested** |
| `calculation_functions.py` — `diffusion_coefficient`, `diffusion_coefficient_SiC_Ag`, `integrate` | `triso_atops_fork::diffusion` | **Ported + verified** |
| `calculation_functions.py` — `RB_fail_Noble_Gases`, `breakthrough_model`, `booth_longlived`, `booth_shortlived_fastdiffuse`, `attenuation_factor` | `triso_atops_fork::release_models::steady_state` | **Ported + verified** |
| `calculation_functions.py` — `breakthrough_model_transient`, `booth_transient`, `RF_Graph` | `triso_atops_fork::release_models::transient` | **Ported + verified** |
| `calculation_functions.py` — `R_B_fail`, `release_fraction` (group dispatchers) | `triso_atops_fork::release_models` (`rb_fail`, `release_fraction_transient`) | **Ported + tested** |
| `calculation_functions.py` — `circulating*`, `plate_out*`, `clean_up*`, `release_rate`, `base_activities` | `triso_atops_fork::activities` (`coolant_activity`, `source_terms`) | **Ported + verified** (bead op-b4a.2.2) |
| `calculation_functions.py` — `higher_activities` routing | `triso_atops_fork::normal_operation::normal_operation_node` | **Ported + verified** (bead op-b4a.2.2) |
| `trisoatops.py` — `normal_operation` (per-node body) | `triso_atops_fork::normal_operation::normal_operation_node` | **Ported + verified** (bead op-b4a.2.2) |
| `calculation_functions.py` — `release_activity`, `coolant_release`; `trisoatops.py` — `accident_case`, `main` | `triso_atops_fork::normal_operation` (accident driver) | **Scaffold** (bead op-b4a.2.3) |
| `run_functions.py` — `process_run_file`, `check_run_file`, `convert_time`, `nuclide_sort`, `read_save_file`, `read_profile`, `inventory_processing` | `triso_atops_fork::normal_operation` (JSON run-file API) | **Scaffold** (bead op-b4a.2.3) |
| `trisoatops_gui.py` (1432 LOC) | — | **Excluded (GUI, out of scope)** |

### Why the GUI was excluded

`boon-lay` is a headless library, and the OUTRAM PARK workspace requires non-GUI
library code to build for Android (no windowing/`egui` in the unconditional
library build). The TRISO-ATOPS GUI is a Tkinter input-file wizard with no
physics of its own, so it is intentionally not ported.

## Units (uom)

Every public function takes and returns `uom` dimensioned quantities. Named
aliases keep editor hovers readable:

- `DecayConstant` = `Frequency` (`s^-1`) — the decay constant `λ = ln2/t½`.
- `ReleaseFraction` = `Ratio` (dimensionless, physically `[0, 1]`).
- Temperatures: `ThermodynamicTemperature`. The upstream correlations are
  written in °C; the functions read the input both as °C (for valid-range clamp
  thresholds) and K (for the Arrhenius exponent), so any real temperature works.
- Diffusion coefficients: `DiffusionCoefficient` (m^2/s); time-integrated
  `∫D dt`: `Area` (m^2); lengths (kernel radius, layer thickness): `Length` (m);
  time: `Time` (s).

## Units decision for the activity layer (op-b4a.2.2, done with human sign-off)

The **cleanly-dimensioned physics core** — diffusion coefficients and all
release-fraction / release-to-birth models — was ported, unit-typed, and
verified first. The **activity bookkeeping** (`circulating`/`plate_out`/
`clean_up`, `release_rate`, `base_activities`) and the **per-node orchestration**
were then ported with `uom` under explicit human sign-off (the crate `CLAUDE.md`
guardrail requires sign-off before touching unit conventions). The upstream
"activity" quantities mixed **atoms**, **atoms/s**, **curies**, and
**becquerels** through three hard-coded factors (`× 3.7e10`, `÷ (1 − e^{−λt})`,
trailing `× λ / 3.7e10`); the port makes each explicit:

- **Activity (Bq) → `uom` `Frequency`.** A becquerel is one decay per second, so
  activity is dimensionally a frequency (`s^-1`) — the same quantity the physics
  core already uses for the decay constant. The alias `Activity = Frequency`
  makes `A = λN` (`Bq = s^-1 · count`) dimensionally honest, and no wrong
  dimension is invented (there is no SI base unit for "amount of a decaying
  species"). See `activity_from_atom_count`.
- **Ci↔Bq → the single constant `BQ_PER_CI = 3.7e10`** (`becquerels_from_curies`
  / `curies_from_becquerels`), replacing every hard-coded `3.7e10`.
- **Atom counts / inventories → documented plain `f64` counts.** A count is
  dimensionless; forcing a `uom` dimension would be wrong, and "atoms/s" is
  dimensionally identical to a rate constant (`s^-1`) so `uom` cannot distinguish
  a release rate from a decay constant. The effective-unit pool quantities
  (release/source rate, circulating/plate-out/clean-up/graphite) are therefore
  `f64`, while the genuinely-dimensioned inputs — `λ`, `k_plate`, `k_clean`, and
  the times — are `uom` `Frequency`/`Time`. The real dimensional check this
  buys: `β = λ + k_plate + k_clean` can only add frequencies, and every exponent
  `β·t`, `λ·t` is checked dimensionless.

Bead status:

- **op-b4a.2** — parent (Eulerian fork).
- **op-b4a.2.1** — calculation core (done).
- **op-b4a.2.2** — activity bookkeeping + per-node orchestration (**done this
  pass**, uom-typed + verified against upstream Python).
- **op-b4a.2.3** — run-file JSON API + accident-case driver entry point (still
  scaffolded; blocked by nothing now, but out of scope this pass).
- **op-b4a.2.4** — verification (done).

## Verification approach & results

There is no single published end-to-end benchmark source term in the upstream
repo or User Manual, so verification (V&V stage: *verified = implemented
correctly*) is against (a) the NP-MHTGR Arrhenius correlations recomputed
independently, and (b) the analytical limits of the Booth / breakthrough models.

Results, taken **2026-07-15** against upstream commit `de374c8` (see the doc
comments in `tests/triso_atops_fork_verification.rs` and the inline
`#[cfg(test)]` modules):

| Check | Reference | Pass tol | Result |
|---|---|---|---|
| Iodine kernel `D`, 1000 °C (low-T branch) | 8.8011e-18 m^2/s | 1e-3 rel | pass |
| Silver-in-SiC `D`, 1200 °C | 8.5710e-17 m^2/s | 1e-3 rel | pass |
| Booth long-lived, `D'·t → ∞` | → 1 | 1e-9 | pass |
| Booth long-lived, early time | `6√(D't/π) − 3D't` | 5e-3 rel | pass |
| Booth short-lived, `x = 100` | → `3/x` | 2e-2 rel | pass |
| Cs-137 `λ` from DB half-life | 7.3022e-10 s^-1 | 1e-9 rel | pass |
| Graphite RF of Xe (volatile) | 0 | exact | pass |
| Supported-nuclide table size | 84 nuclides | exact | pass |

### Activity layer & nodal orchestration (op-b4a.2.2)

The activity bookkeeping and per-node orchestration are verified against values
produced by the **upstream TRISO-ATOPS Python** (commit `de374c8`) on identical
inputs — the upstream functions are the reference implementation, so this checks
the Rust port is *implemented correctly*. Ground-truth numbers were generated by
calling the upstream `circulating*` / `plate_out*` / `clean_up*` / `release_rate`
/ `base_activities` / `higher_activities` directly (single node, 900 °C, 44.5 Ci
inventory, NP-MHTGR geometry). Data taken **2026-07-15**:

| Check | Reference (upstream Python) | Pass tol | Result |
|---|---|---|---|
| Ci↔Bq round trip (44.5 Ci) | 1.64650e12 Bq | 1e-12 rel | pass |
| `A = λN` (Cs-137, N=1e18) | 7.30219e8 Bq | 1e-12 rel | pass |
| `circulating_steadystate` | 1.4675704602 | 1e-12 rel | pass |
| `plate_out` (t=40 yr) | 343.9504438998 | 1e-12 rel | pass |
| `clean_up` (t=40 yr) | 40.2219856400 | 1e-12 rel | pass |
| `release_rate` Kr-88 (short-lived) | 635.91123 | 1e-10 rel | pass |
| `release_rate` Cs-137 (long-lived) | 63842.3183814 | 1e-10 rel | pass |
| `base_activities` Cs-137 S, G | 6.38423e-4, 5.83994e12 | 1e-9 rel | pass |
| End-to-end node Cs-137 (metal, no HPS) → Ci | plate-out 0.10237 Ci | 1e-6 rel | pass |
| End-to-end node Kr-88 (noble gas, HPS) → Ci | HPS 1.14021e-5 Ci | 1e-6 rel | pass |
| End-to-end node I-131 (halogen, HPS) → Ci | plate-out 2.69903e-5 Ci | 1e-6 rel | pass |

The three end-to-end node checks exercise all group paths (special metal / noble
gas / halogen), the HPS on/off toggle, the noble-gas plate-out zeroing, and the
`× λ / 3.7e10` curie conversion. Test code: the `#[cfg(test)]` modules in
`src/triso_atops_fork/activities/{coolant_activity,source_terms}.rs` and
`src/triso_atops_fork/normal_operation/mod.rs`.

Total: 25 core + 17 activity/nodal library unit tests + 7 integration tests, all
green under `cargo test -p boon-lay --lib --tests --release`.

**Not claimed:** *validation* of a full reactor source term against measured
release data — that needs a public benchmark case (and the still-scaffolded
accident/JSON driver, op-b4a.2.3).
