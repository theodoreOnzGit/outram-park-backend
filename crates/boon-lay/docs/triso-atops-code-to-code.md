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
    -> tests/data/triso_atops_reference.csv   # 5 699 committed reference values
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
  the silver `sqrt(lambda_Ag110m / lambda)` scaling;
- both materials and all four branches of the transient `release_fraction`
  dispatcher (Booth transient, silver-through-SiC breakthrough, the identically
  zero volatile-in-graphite case, and `RF_Graph` for fission metals);
- the cumulative diffusion integral at **every** output time step, not just the
  endpoint, over three temperature histories including a non-monotonic one;
- the whole `normal_operation_node` chain for six nuclides spanning every
  transport group, with and without a clean-up system, and with and without
  parent-decay pools.

`diffusion_coefficient` is *separable* — the kernel coefficient reads only `T`
and the graphite coefficient only `T_graph`, except for the Kr/Te/I/Xe/Se group
where `D_graph = D` and both read `T`. The generator therefore sweeps each
variable with the other held fixed, plus a small full-cross block on one `z` per
branch to catch any coupling that argument would miss. A naive full cross
product emitted 6 760 rows of which 688 were exact duplicates and none added
branch coverage.

## Results

Taken **2026-09-15**, upstream `de374c8`, **5 699 cases across 32 function
groups, all passing**. `max_rel_dev` is the largest relative deviation observed
in that group; "widened" counts cases whose tolerance was raised to the
arithmetic noise floor (see *Ill-conditioned inputs* below).

| Function group | Cases | max_rel_dev | Tolerance | Widened |
|---|---:|---:|---:|---:|
| `diffusion_coefficient.kernel` | 516 | 1.40e-16 | 1e-12 | |
| `diffusion_coefficient.graphite` | 516 | 1.55e-16 | 1e-12 | |
| `diffusion_coefficient_sic_ag` | 20 | 2.22e-16 | 1e-12 | |
| `rb_fail_noble_gases` | 270 | 1.75e-16 | 1e-12 | |
| `breakthrough_model` | 120 | 0 (exact) | 1e-9 | 27 |
| `booth_longlived` | 60 | 7.16e-12 | 1e-9 | |
| `booth_shortlived_fast_diffuse` | 75 | 6.74e-13 | 1e-11 | |
| `attenuation_factor` | 60 | 4.16e-10 | 1e-9 | |
| `booth_transient` | 8 | 3.62e-11 | 1e-9 | |
| `breakthrough_model_transient` | 64 | 0 (exact) | 1e-9 | 10 |
| `rf_graph` | 18 | 4.05e-15 | 1e-9 | 2 |
| `release_fraction.kernel` | 120 | 3.32e-4 | 1e-9 | 16 |
| `release_fraction.graphite` | 120 | 2.01e-12 | 1e-9 | 40 |
| `integrate.kernel` | 60 | 0 (exact) | 1e-9 | |
| `integrate.graphite` | 60 | 0 (exact) | 1e-9 | |
| `circulating_steadystate` | 144 | 0 (exact) | 1e-12 | |
| `circulating` | 432 | 1.12e-14 | 1e-12 | |
| `plate_out_steadystate` | 144 | 0 (exact) | 1e-12 | |
| `plate_out` | 432 | 1.15e-14 | 1e-9 | |
| `clean_up_steadystate` | 144 | 0 (exact) | 1e-12 | |
| `clean_up` | 360 | 1.27e-14 | 1e-9 | |
| `rb_fail` (dispatcher) | 468 | 6.48e-11 | 1e-9 | |
| `release_rate` | 180 | 0 (exact) | 1e-12 | |
| `base_activities.source` | 216 | 1.83e-12 | 1e-9 | |
| `base_activities.graphite` | 216 | 1.65e-16 | 1e-9 | |
| `node.release_rate` | 144 | 3.10e-11 | 1e-9 | |
| `node.source_rate` | 144 | 3.10e-11 | 1e-9 | |
| `node.graphite_activity` | 144 | 3.10e-11 | 1e-9 | |
| `node.circulating_activity` | 144 | 3.10e-11 | 1e-9 | |
| `node.plate_out_activity` | 144 | 3.10e-11 | 1e-9 | |
| `node.clean_up_activity` | 72 | 1.93e-16 | 1e-9 | |
| nuclide decay constants (all 84) | 84 | 0 (exact) | 1e-12 | |

**Interpretation.** Outside the ill-conditioned inputs analysed below, every
function agrees with upstream to between exact equality and 4.2e-10 relative,
and seven groups are bit-exact — including the cumulative diffusion integral,
which is the one most exposed to a silent ordering or off-by-one slip. The
end-to-end `normal_operation_node` chain agrees to 3.1e-11 across all six of its
outputs. The Rust port of the TRISO-ATOPS calculation core is therefore
**verified as a faithful translation** of upstream `de374c8`, subject to the two
divergences documented further down.

Tolerances were set from the observed deviation and rounded up by roughly an
order of magnitude. Nothing was loosened to make a test pass.

**The whole nuclide database is checked, not a spot sample.** All 84 decay
constants are compared against the upstream table entry for the same nuclide,
and the test additionally fails if upstream carries a nuclide the port lacks. A
transcription slip in any single half-life is a silent, physics-changing defect
that no other test here would catch.

Nuclides are matched by `(Z, A)`, never by `Z` alone: Xe-133 and Xe-135 share
`Z = 54` with decay constants two orders of magnitude apart, so a `Z`-keyed
lookup would have silently compared the wrong isotope.

### Ill-conditioned inputs: where f64 runs out, and how that is handled

Two upstream formulas lose precision catastrophically in part of their input
range. This is a property of the **formulas**, not of the port — but it is real,
and simply asserting a tight tolerance there would be asserting noise.

1. **`RF_Graph` (and `release_fraction` on graphite) at small `int D dt`.** The
   series evaluates `1 - exp(-x)`. At `val = 1e-18`, `a = 0.0045` the first
   harmonic has `x = 1.2e-13`, so `exp(-x)` is within 1e-13 of 1 and the
   subtraction discards ~13 digits. Measured: the naive form differs from the
   well-conditioned `-expm1(-x)` by **2.2e-7** relative — the same order as the
   port-vs-upstream difference, which is therefore just NumPy's and Rust's
   `exp` differing in the last ulp and being amplified.

2. **`breakthrough_model_transient` (and `release_fraction` on the silver kernel
   path).** The result is `line - cst - ser`, three terms of order 0.1-0.2 that
   can cancel to order 1e-13. At `int_Dp = 1e-2`, `int_Dt = 1e-10`,
   `a = 1e-4`, `r = 2.13e-4` the cancellation ratio is **2.8e12**, so the best
   achievable relative precision is `eps * 2.8e12 = 6.2e-4`. The observed
   port-vs-upstream deviation of 3.3e-4 is *inside* that bound — the two
   implementations agree as closely as f64 permits.

**How the test handles it.** The generator records a per-case **cancellation
ratio** (`cond`) in a fourth fixture column, and the test asserts against
`max(group_tol, 8 * eps * cond)`. That is algebraically an *absolute* check
against the arithmetic noise floor, `|got - want| <= 8 * eps * largest_term`, so
it remains fully sensitive to real defects: a wrong constant or sign shifts an
intermediate term by order 0.1, roughly 1e15 times the noise floor. 169 of the
5 699 cases carry `cond > 1`; the rest are asserted at full tightness. This was
verified, not assumed — see the mutation results below, where the widened groups
still failed under mutation.

A note for whoever revisits this: replacing `1 - exp(-x)` with `-expm1(-x)` in
`rf_graph` would make the port strictly more accurate than upstream, and would
therefore *increase* the measured divergence. That is a deliberate
accuracy-versus-fidelity trade and a maintainer decision, not a bug fix to apply
silently.

### The suite is not vacuous

Every test passed on its first run, so non-vacuity was demonstrated by mutation
on 2026-09-15, in two rounds. Round one targeted the well-conditioned core:

| Mutation | Relative size | Groups that failed |
|---|---|---|
| iodine kernel pre-exponential `1.3e-12` -> `1.3001e-12` | 7.7e-5 | `diffusion_coefficient.kernel`, `.graphite`, `rb_fail` |
| noble-gas fit exponent `0.302` -> `0.3021` | 3.3e-4 | `rb_fail_noble_gases`, `rb_fail` |
| Cs-137 half-life `949_232_333` -> `949_232_444` s | 1.2e-7 | nuclide decay constants |

Round two specifically targeted the **ill-conditioned** groups and the newer
surfaces, to confirm the widened bound does not mask defects:

| Mutation | Groups that failed |
|---|---|
| `rf_graph` series numerator `8.0` -> `8.0001` | `rf_graph`, `release_fraction.graphite` |
| `breakthrough_model_transient` time-lag `a/2` -> `a/2.0001` | `breakthrough_model_transient`, `release_fraction.kernel` |
| `integrate` trapezoid weight `0.5` -> `0.5001` | `integrate.kernel`, `integrate.graphite` |
| `normal_operation_node` noble-gas `k_plate` zeroing removed | `node.circulating_activity`, `node.clean_up_activity` |

Both rounds behaved correctly: exactly the groups reading the mutated value
failed, and every unrelated test stayed green. In particular
`release_fraction.kernel` failed despite carrying 16 ill-conditioned cases on a
widened bound, which is the specific reassurance that mechanism needed. The
fourth mutation left `node.graphite_activity`, `node.release_rate` and
`node.source_rate` green, correctly — those are computed upstream of the group
dispatch and cannot depend on it. All mutations were reverted and the tree
re-verified.

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

Still not covered, and tracked in `bn:op-b4a.2.7`:

- `coolant_release` — coolant activation along a temperature/pressure history;
- `nuclide_import` / `nuclide_import_accident` — the short-lived/long-lived
  classification against runtime, and the 0.2 / 0.04 ratio defaults;
- `inventory_processing` — axial inventory reshaping;
- `release_activity` — the accident-path activity assembly;
- the JSON run-file driver and accident-case entry point, still scaffolded
  (`bn:op-b4a.2.3`);
- the upstream Tkinter GUI, intentionally not ported.

`higher_activities` has no standalone counterpart in the port and is **not**
untested: it is covered end-to-end through `normal_operation_node`, which is the
composition upstream's own `trisoatops.py` driver performs. That distinction was
worth getting right — the group-dependent zeroing of `k_plate` for noble gases
and `k_clean` for non-halogens happens at the *driver* call site upstream, not
inside `higher_activities`, so a test written against `higher_activities` alone
would have compared the wrong thing and made the port look wrong when it is
correct.
