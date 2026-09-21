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
| `calculation_functions.py` — `nuclide_import`, `nuclide_import_accident` | `triso_atops_fork::run_selection` (`select_nuclides`, `select_nuclides_accident`, `normalise_nuclide_name`) | **Ported + verified** (2026-09-21) — both classification tests compared decision-for-decision against upstream. Parent-decay wiring is behind [`ParentDecayPolicy`] because upstream's is defeated by an `==` bug; see the defect note below. |
| `calculation_functions.py` — `inventory_processing` | `triso_atops_fork::run_selection` / `accident::distribute_inventory_axially` | **Ported + verified** (2026-09-21) |
| `calculation_functions.py` — `release_activity`, `coolant_release`; `trisoatops.py` — `accident_case` | `triso_atops_fork::accident` (`release_activity`, `coolant_release`, `mean_temperature_rate`, `accident_release_curies`, `atoms_to_curies`) | **Ported + verified** (2026-09-21) ~~Scaffold~~ ~~NOT PORTED~~ — 6 068 code-to-code cases. `main`'s argparse shell is not ported and will not be; see "What is deliberately not ported". |
| `run_functions.py` — `convert_time`, `read_save_file`, `process_run_file`, `check_run_file`, `read_profile` | `triso_atops_fork::run_file` (`TimeUnit`, `RunFile`, `RunConfig`, `RunFile::to_config`) | **Ported** (2026-09-21) — serde-derived, so a GUI-written JSON file deserialises directly; validation collects every problem rather than the first. |
| `run_functions.py` — `nuclide_sort` | `triso_atops_fork::run_selection::sort_parents_before_daughters` | **Ported** (2026-09-21) — does what upstream *intends*; upstream's own version is a no-op (defect 7 below). |
| `run_functions.py` — `create_log`, `count_errors`, `trisoatops`; `trisoatops.py` — `main` | — | **Deliberately not ported** — Python `logging` setup, an error counter that `Result` replaces, a version banner, and an argparse shell. See "What is deliberately not ported". |
| `trisoatops_gui.py` (1432 LOC) | — | **Excluded (GUI, out of scope)** |

### What is deliberately not ported

Four upstream entries have no Rust counterpart and will not get one:

- **`run_functions.py::create_log`** configures Python's `logging` module. A
  Rust library that installs a global logger is badly behaved; diagnostics
  surface as `RunFileError` / `SelectionError` values instead, which a caller
  logs however it likes.
- **`run_functions.py::count_errors`** threads an error counter through every
  function because Python has no `Result`. `Result` does that job.
- **`run_functions.py::trisoatops()`** prints a version banner to stdout.
- **`trisoatops.py::main`** is an `argparse` shell around the physics. The
  composition it performs is available as library calls; wrapping them in a CLI
  is `outram-foam-cli`'s business, not this crate's.
- **`trisoatops_gui.py`** (1 432 LOC) — Tkinter GUI, out of scope as recorded
  below.

### Upstream defects found while porting (2026-09-21)

~~Seven defects~~ **CORRECTED 2026-09-21 — ten.** Defects 1–7 were found by
reading `calculation_functions.py`, `run_functions.py` and `trisoatops.py` at
`de374c8`; **8–10 were found by the second verification pass**, when the
fixture was widened from 11 855 to 14 130 cases and the end-to-end
`accident_case` composition was driven for the first time. Two of the three
were found by *running* upstream rather than reading it, which is the point of
keeping the harness executable.

**None affects the previously-verified subset.** Where the port diverges, the
divergence is selectable rather than silent, and named in the item's own
documentation. All ten are filed for upstream in GitHub issue #219; none has
been reported to INL directly.

**1. `parent_decay` is assigned with `==`, so the runtime logic never fires.**
`nuclide_import` (lines 275 and 277) writes

```python
if nuclide_out[parent].sl == True:
    nuclide_out[nuclide].parent_decay == True      # `==`, not `=`
else:
    nuclide_out[nuclide].parent_decay == False     # `==`, not `=`
```

Both are comparisons whose result is discarded. In the branch where the parent
*is* present in the run's nuclide list — the branch meant to decide whether
parent decay applies — `parent_decay` is therefore never set and keeps the
value hard-coded in the `nuclides` table. The two `else` paths (lines 280, 283)
do use `=` and correctly set `False`.

The consequence is not that parent decay is off, but that **the short-lived
parent test is dead**: `trisoatops.py:109` branches on `parent_decay is True`
to feed parent circulating/plate-out/clean-up pools into `higher_activities`,
so for the 13 table rows defaulted `True` the coupling is applied whenever the
parent is in the list, regardless of the parent's half-life ratio that the
`sl` test was written to check.

**2. Rh-105 carries a parent but is defaulted `parent_decay=False`.** It has
`['Ru-105']` and the same `# og with parent decay` comment as the 13 rows
defaulted `True`. Given defect 1 leaves the table default in control, Rh-105
never receives parent decay even when Ru-105 is present. The 13/1 split looks
like an oversight rather than a decision, but it is upstream's, so a port
should reproduce it and say so rather than silently "fixing" it.

**3. `release_activity` uses `z == 48` where every other site uses `z == 46`.**
The silver/palladium group is spelled `z == 47 or z == 46` at
`calculation_functions.py` lines 208, 722, 747 and 775. Line 906, inside
`release_activity`, instead reads `z == 47 or z == 48` — silver and
**cadmium**. Both affected nuclides ship in the table (`Pd-107`, z=46, line
163; `Cd-113`, z=48, line 166), so this is reachable, not theoretical:
Pd-107 falls through to `fract = np.sum(fractions)` instead of `fract = 1`,
and Cd-113 wrongly receives the silver treatment. Four sites against one makes
`48` the likely typo.

**4. `nuclide_import` mutates the shared module-level table.**
`nuclide_out[nuclide_name] = nuclides[nuclide_name]` (lines 262 and 314) binds
a reference, not a copy, so the subsequent `.sl` and `.parent_decay` writes
land on the global `nuclides` dict. Two runs in one process — which is exactly
what `trisoatops_gui.py` does — leak the first run's classification into the
second. A Rust port gets this right for free by owning the value; that is a
divergence worth recording rather than an inherited bug.

**5. `nuclide_import_accident` indexes the table without the guard its sibling
has.** `nuclide_import` checks `if nuclide_name in nuclides` before lookup;
`nuclide_import_accident` (line 313) does not, so a name that satisfies the
regex but is absent from the table raises `KeyError` instead of being skipped
with a warning. Its `else` branch also logs `{match}`, which is `None`
whenever that branch is taken.

**6. `nuclide_sort` never reorders anything.** `run_functions.py:412` reads

```python
par = calc.nuclides[n].parents          # a LIST, e.g. ['Kr-89']
if par is not None and par in list(nuke_list[:, 0]):
```

which asks whether the *list* `['Kr-89']` is an element of a list of *strings*.
That is never true, so the branch that moves a parent ahead of its daughter is
dead and the function returns its input order unchanged — defeating the stated
purpose of letting the driver accumulate parent activities first.
`run_selection::sort_parents_before_daughters` does what the function says it
does; a caller wanting upstream's no-op simply does not call it.

**7. `accident_case` discards the temperature history when nothing is
truncated.** `trisoatops.py` narrows the transient to the venting window with

```python
rmv = np.size(times) - np.size(times_short)
accident_temp = accident_temp[:, :-rmv, :]
```

If every sample is a venting sample — which is exactly what a monotonic
heat-up gives — then `rmv == 0`, and `[:-0]` is `[:0]`, the **empty** slice.
The whole temperature history vanishes and every downstream integral is empty.
The port cannot reproduce this (slices arrive aligned and a mismatch is an
assertion), but a reader comparing against a stock run on a monotonic transient
will see upstream produce nothing, and should know why.

**8. The nuclide-name regex is unanchored, so a malformed name is silently
truncated into a *different* valid nuclide.** `nuclide_import` and
`nuclide_import_accident` both normalise with

```python
match = re.match(r'([a-z]{1,2})(?:[-]?)([0-9]+)([a-z]?)', nuclide_name)
```

`re.match` anchors only at the start, and there is no `$`, so trailing junk is
dropped rather than rejected. Measured against upstream at `de374c8`:

| input | upstream accepts as | what the user meant |
|---|---|---|
| `Cs-137xyz` | `Cs-137x` | — (typo) |
| `Cs-137-extra` | `Cs-137` | — (typo) |
| `cs137mm` | **`Cs-137m`** | — (typo) |

The third is the dangerous one: a typo is silently promoted to the *metastable
state*, a physically different nuclide with a different half-life, and the run
proceeds with no warning. (The first two then raise a `KeyError` on the
`nuclides` lookup, which at least fails loudly.) `normalise_nuclide_name`
anchors the match and rejects all three; the divergence is asserted
deliberately in `name_normalisation_matches_upstream_regex`.

**9. `accident_case` pairs the venting *times* with the wrong temperature
*slices* whenever the venting mask is gappy.** After narrowing the transient,
`trisoatops.py` has

```python
frac, times_short = calc.coolant_release(times, accident_temp)
rmv = np.size(times) - np.size(times_short)
times = times_short                       # the VENTING samples
accident_temp = accident_temp[:, :-rmv, :]  # the FIRST n - rmv samples
```

`times_short` is `times[vent]` — a *selection*. `accident_temp[:, :-rmv, :]` is
a *prefix*. The two coincide only while the venting mask is contiguous. A
transient that heats, cools, and re-heats produces a gappy mask, and upstream
then integrates a diffusion coefficient evaluated at one instant's temperature
over a `Δt` taken from a different pair of instants. Measured on the fixture's
`gappy_vent_mask` scenario (vent mask `[0, 1, 3]` out of six samples): the
nodal kernel release for Xe-133 at the last retained sample is `-3.20e-8 Ci`
with upstream's pairing and `-4.99e-8 Ci` with the consistent one — a **36 %**
difference on a single node.

The port reproduces this deliberately in the code-to-code test, because the
test's job is to establish what upstream computes. Library callers assembling
their own accident case should use the venting times with the venting slices.

**10. `booth_transient`'s `RF < 1e-6 → 0` guard is dead code.** The series is
truncated at `num_terms = 5000`, so as `int_Dp → 0` the sum tends to
`Σ_{i=1}^{4999} (iπ)^{-2}`, not to `1/6`, and the result tends to

$$ RF(0^+) \;=\; \frac{6}{\pi^2} \sum_{i=N}^{\infty} i^{-2} \;\approx\; \frac{6}{\pi^2 N} \;=\; 1.216 \times 10^{-4} $$

for `N = 5000` — two orders of magnitude above the `1e-6` the guard tests
against. `int_Dp == 0` is caught by an earlier early return and `RF` is
monotone in `int_Dp`, so nothing can land inside the guard band. Measured
minimum over the fixture's 52 non-zero `booth_transient` rows:
`1.21628e-4`, against the predicted `1.21585e-4`.

The guard is harmless but misleading — it reads as if tiny releases are
clamped to zero, and they are not. The port keeps it (bug-compatibility) and
pins the premise with `booth_transient_zero_floor_is_unreachable`, so raising
`BOOTH_SERIES_TERMS` past `6/(π²·10⁻⁶) ≈ 6.1e5` cannot quietly change
behaviour. It is also the reason deleting that guard is an **equivalent
mutant** in the mutation table rather than a surviving one.

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
- **op-b4a.2.3** — run-file JSON API + accident-case driver entry point
  (~~still scaffolded~~ **CORRECTED 2026-09-21: not ported — no code exists**;
  blocked by nothing, but out of scope this pass).
- **op-b4a.2.4** — verification (done).

## Verification approach & results

> **See also: [`triso-atops-code-to-code.md`](triso-atops-code-to-code.md)** —
> an exhaustive **code-to-code** verification of the whole calculation core
> against the upstream Python: 5 699 cases over 32 function groups, covering
> every pointwise model, the transient dispatcher, the cumulative diffusion
> integral, the end-to-end `normal_operation_node` chain and all 84 nuclide
> decay constants (added 2026-09-15). That is the primary evidence that the
> *translation* is faithful; the analytical checks below are the evidence that
> the *physics* is right. Two deliberate divergences from upstream, and two
> genuinely ill-conditioned upstream formulas, are analysed there.

There is no single published end-to-end benchmark source term in the upstream
repo or User Manual, so verification (V&V stage: *verified = implemented
correctly*) is against (a) the NP-MHTGR Arrhenius correlations recomputed
independently, (b) the analytical limits of the Booth / breakthrough models,
and (c) direct execution of the upstream Python over a branch-covering input
grid (the code-to-code suite linked above).

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
