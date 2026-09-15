# TRISO-ATOPS code-to-code verification

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## What this is

An exhaustive **code-to-code** verification of the Rust port in
`src/triso_atops_fork/` against the **upstream INL TRISO-ATOPS Python** at
commit `de374c8`. Every reference number is produced by *executing upstream*,
not by re-deriving the physics and not by transcribing values from the User
Manual.

This complements the earlier analytical work in
[`../tests/triso_atops_fork_verification.rs`](../tests/triso_atops_fork_verification.rs),
which checks the models against their closed-form limits. The two answer
different questions:

| Test file | Question answered |
|---|---|
| `triso_atops_fork_verification.rs` | Is the *physics* right — do the models reproduce their analytical limits? |
| `triso_atops_code_to_code.rs` | Is the *translation* right — does the port compute what upstream computes? |

For a port, the second question is the one that governs. The workspace rule
"Debugging a port: read upstream first" makes upstream the specification; this
test makes that specification executable and continuously checked.

## How it works

```
dev/gen_triso_atops_reference.py          # imports upstream, sweeps the grid
    -> tests/data/triso_atops_reference.csv   # 4 547 committed reference values
        -> tests/triso_atops_code_to_code.rs  # replays them through the Rust port
```

The generator imports
`trisoatops/utility_functions/calculation_functions.py` from the **gitignored**
reference clone at `upstream_source/TRISO-ATOPS` (never compiled, never
published), sweeps each function over an input grid chosen to straddle every
branch and clamp in the upstream source, and writes each case at full `repr()`
round-trip precision.

Regenerate (after re-cloning upstream at `de374c8`):

```bash
python3 dev/gen_triso_atops_reference.py            # rewrite the fixture
python3 dev/gen_triso_atops_reference.py --check    # regenerate + diff only
cargo test --release -p boon-lay --test triso_atops_code_to_code
```

> **On the Python.** The workspace forbids Python for *documentation generation*
> and *repository accounting*. This is neither — it executes a third-party
> reference implementation to produce V&V data, the same shape as
> `crates/outram-park-fork-coolprop/dev/*.py`, which the workspace `CLAUDE.md`
> explicitly records as out of that rule's scope. Upstream also `import pandas`
> in `calculation_functions.py` but never references it (0 `pd.` occurrences at
> `de374c8`), so the harness injects an empty stub rather than pulling in an
> unused heavy dependency.

## Branch coverage

The grid deliberately hits every conditional in the upstream calculation core:

- the kernel-temperature clamps at 700 °C (Cs/Rb, Sr/Ba/Eu) and the graphite
  clamps at 490 °C (Ag/Pd), 550 °C (Cs/Rb) and 800 °C (Sr/Ba/Eu);
- the 1500 °C switch between the low- and high-temperature kernel correlations
  for Kr/Te/I/Xe/Se;
- the `D = 1e-19` fallback for elements with no correlation, and the fixed
  `<R/B>_fail = 1e-5` fallback in the group dispatcher;
- the `[0, 1]` clamps in both breakthrough models;
- the `1e8` cap and negative-value fallback in `attenuation_factor`;
- the `beta - lam == 0` guard in `plate_out` (and its absence in `clean_up`);
- the `int_Dp == 0` short circuit and the `RF < 1e-6 -> 0` threshold in the
  transient models;
- all five transport groups in `R_B_fail`, both short- and long-lived, including
  the silver `sqrt(lambda_Ag110m / lambda)` scaling.

`diffusion_coefficient` is *separable* — the kernel coefficient reads only `T`
and the graphite coefficient only `T_graph`, except for the Kr/Te/I/Xe/Se group
where `D_graph = D` and both read `T`. The generator therefore sweeps each
variable with the other held fixed, plus a small full-cross block on one `z` per
branch to catch any coupling that argument would miss. A naive full cross
product emitted 6 760 rows of which 688 were exact duplicates and none added
branch coverage.

## Results

Taken **2026-09-15**, upstream `de374c8`, **4 547 cases, all passing**.
`max_rel_dev` is the largest relative deviation observed in that group.

| Function group | Cases | max_rel_dev | Tolerance |
|---|---:|---:|---:|
| `diffusion_coefficient.kernel` | 516 | 1.40e-16 | 1e-12 |
| `diffusion_coefficient.graphite` | 516 | 1.55e-16 | 1e-12 |
| `diffusion_coefficient_sic_ag` | 20 | 2.22e-16 | 1e-12 |
| `rb_fail_noble_gases` | 270 | 1.75e-16 | 1e-12 |
| `breakthrough_model` | 120 | 0 (exact) | 1e-9 |
| `booth_longlived` | 60 | 7.16e-12 | 1e-9 |
| `booth_shortlived_fast_diffuse` | 75 | 6.74e-13 | 1e-11 |
| `attenuation_factor` | 60 | 4.16e-10 | 1e-9 |
| `booth_transient` | 8 | 3.62e-11 | 1e-9 |
| `breakthrough_model_transient` | 64 | 0 (exact) | 1e-9 |
| `rf_graph` | 18 | 8.05e-15 | 1e-9 |
| `circulating_steadystate` | 144 | 0 (exact) | 1e-12 |
| `circulating` | 432 | 1.12e-14 | 1e-12 |
| `plate_out_steadystate` | 144 | 0 (exact) | 1e-12 |
| `plate_out` | 432 | 1.15e-14 | 1e-9 |
| `clean_up_steadystate` | 144 | 0 (exact) | 1e-12 |
| `clean_up` | 360 | 1.27e-14 | 1e-9 |
| `rb_fail` (dispatcher) | 468 | 6.48e-11 | 1e-9 |
| `release_rate` | 180 | 0 (exact) | 1e-12 |
| `base_activities.source` | 216 | 1.83e-12 | 1e-9 |
| `base_activities.graphite` | 216 | 1.65e-16 | 1e-9 |
| nuclide decay constants (all 84) | 84 | 0 (exact) | 1e-12 |

**Interpretation.** Every function agrees with upstream to between exact
equality and 4.2e-10 relative; seven groups are bit-exact. The largest residual
is `attenuation_factor`, which evaluates `1/(1 - S)` and so amplifies the
last-bit difference between NumPy's pairwise `np.sum` and the port's sequential
summation — that it would be the worst case was predicted from reading the code
before measuring, not discovered afterwards. The Rust port of the TRISO-ATOPS
calculation core is therefore **verified as a faithful translation** of upstream
`de374c8`, subject to the two divergences below.

Tolerances were set from the observed deviation and rounded up by roughly an
order of magnitude. Nothing was loosened to make a test pass.

**The whole nuclide database is checked, not a spot sample.** All 84 decay
constants are compared against the upstream table entry for the same nuclide,
and the test additionally fails if upstream carries a nuclide the port lacks. A
transcription slip in any single half-life is a silent, physics-changing defect
that no other test here would catch.

### The suite is not vacuous

Every test passed on its first run, so non-vacuity was demonstrated by mutation
on 2026-09-15. Three independent mutations were applied to the port:

| Mutation | Relative size | Tests that failed |
|---|---|---|
| iodine kernel pre-exponential `1.3e-12` -> `1.3001e-12` | 7.7e-5 | `diffusion_coefficient.kernel`, `.graphite`, `rb_fail` |
| noble-gas fit exponent `0.302` -> `0.3021` | 3.3e-4 | `rb_fail_noble_gases`, `rb_fail` |
| Cs-137 half-life `949_232_333` -> `949_232_444` s | 1.2e-7 | nuclide decay constants |

Exactly the five groups that read those values failed; the other 19 tests stayed
green. The suite is both sensitive and specific. The mutations were reverted and
the tree re-verified clean.

## Where the port deliberately differs from upstream

Two inputs exist for which upstream has **no value to compare against**, because
it raises rather than returning. Both are pinned by explicit tests rather than
skipped, so a future change on either side breaks the suite.

### 1. `RB_fail_Noble_Gases` for He, Ne, Ar, Rn (`Z` in {2, 10, 18, 86})

Upstream branches `if z == 36: ... elif z == 54 or z in halogens: ...` with no
`else`, so the result variable is never bound and Python raises
`UnboundLocalError`. These are genuine noble gases the correlation is nominally
meant to cover, so the omission looks unintended.

The Rust port's `if z == 36 { ... } else { ... }` is total and applies the
xenon/halogen fit. That is the more defensible behaviour, but it **is** a
divergence: the port returns a number where upstream crashes. Pinned by
`noble_gases_upstream_cannot_evaluate_use_the_xenon_fit`, which asserts the port
stays finite, returns exactly the Xe value, and does not swallow krypton's
distinct fit.

### 2. `clean_up` with `k_plate == k_clean == 0`

`plate_out` guards `beta - lam == 0` and returns `0` (silently dropping its
parent term). Its sibling `clean_up` carries no such guard, so the same input
raises `ZeroDivisionError` upstream, while the Rust port — reproducing the
missing guard literally — computes `0.0 / 0.0` and yields `NaN`.

Both sides decline to produce a value; the port is a faithful translation of the
defect. It is **latent, not live**: upstream only calls `clean_up` from
`higher_activities` when a clean-up system is present, which implies
`k_clean > 0`, so no upstream run reaches it. Pinned by
`clean_up_without_guard_is_nan_where_upstream_raises`, which also asserts that
`plate_out` really does take its guard on identical inputs — that asymmetry is
what makes this an upstream defect rather than a porting slip.

Neither divergence has been reported upstream; both are candidates for an issue
against `IdahoLabResearch/TRISO-ATOPS` at the maintainer's discretion.

## What this does NOT establish

This is **verification, not validation**. It shows the Rust port computes what
the upstream Python computes. It says nothing about whether *either* reproduces
measured TRISO fission-product release — that requires a public benchmark and is
not claimed here. Per `RESPONSIBLE_USE.md`, AI-assisted output remains untrusted
draft material until a human reviews it, and the `Bookkeeping status` block in
the crate README records that review as outstanding.

Not covered by this file, and still open:

- the JSON run-file driver and accident-case entry point (scaffolded);
- `coolant_release`, `nuclide_import` / `nuclide_import_accident`,
  `inventory_processing` and `integrate`, which are upstream I/O- and
  array-shaped orchestration rather than pointwise physics;
- `higher_activities`, exercised indirectly through `normal_operation_node` in
  `triso_atops_fork_verification.rs` but not swept pointwise here;
- the upstream Tkinter GUI, intentionally not ported.
