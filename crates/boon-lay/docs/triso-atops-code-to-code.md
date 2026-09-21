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

## Second pass — the accident path and run set-up (2026-09-21)

The gaps listed at the bottom of this document were closed on 2026-09-21. The
fixture grew from 5 699 to **11 855 cases** and the suite from 34 to **42
tests**, generated by the same harness against the same upstream `de374c8`.

| Function group | Cases | max_rel_dev |
|---|---:|---:|
| `release_activity.kernel` | 3 024 | **0 (bit-exact)** |
| `release_activity.graphite` | 3 024 | **0 (bit-exact)** |
| `coolant_release.vent_time` | 10 | **0 (bit-exact)** |
| `inventory_processing` | 16 | **0 (bit-exact)** |
| `coolant_release.fraction` | 10 | 4.69e-16 |
| `coolant_release.mean_dtdt` | 12 | 1.12e-15 |
| `nuclide_import.short_lived` | 30 | exact (decisions) |
| `nuclide_import_accident.retained` | 30 | exact (decisions) |

The two classification groups are compared **exactly**, not to a tolerance: the
quantity under test is a boolean decision encoded `1.0`/`0.0`, and a tolerance
on a decision would be meaningless.

`release_activity` being bit-exact across 6 048 cases spanning every transport
group, both materials, the clean-up toggle and both sides of the plate-out
condition is the strongest single result in this document. It is also the group
where the port carries a **deliberate correction** — see below.

### Non-vacuity, second pass

Four mutations, 2026-09-21:

| Mutation | Result |
|---|---|
| `AccidentFractions::volatile_sum` swapped `incremental_accident` for `incremental_sic_accident` | `release_activity.kernel` **FAILED** |
| `distribute_inventory_axially` divisor `n_axial` -> `n_axial + 0.001` | `inventory_processing` **FAILED** |
| `select_nuclides` default ratio `0.2` -> `0.9` | `nuclide_import.short_lived` **FAILED** |
| `R_GAS` `8.31447` -> `8.31448` | **NOT CAUGHT — and correctly so** |

The fourth is the interesting one. `R_GAS` cannot be caught by any fixture,
because it **cancels**: `frac = |integral / n_0|` carries `P/R` in both
numerator and denominator. Measured against upstream the same day, feeding
`P = 1.0`, `101.325`, `202.65` and `5000.0` returns **bit-identical**
fractions — so upstream's `P` argument, which looks like a physical tunable
with a physical default, cannot affect its own answer either.

That is pinned by `accident::tests::pressure_does_not_affect_the_fraction`
rather than by the fixture, because a fixture comparison would pass whatever
the port did with `P`. The port agrees across `P` to **1 ulp rather than
bit-exactly** — the cancellation is algebraic, and its exactness depends on
operation order, which differs from NumPy's. Stated rather than rounded away.

### Where the port deliberately differs, second pass

Two corrections, both selectable so that code-to-code comparison still works:

1. **`release_activity`'s silver branch.** Upstream reads `z == 47 or z == 48`
   — silver and **cadmium** — where its four sibling sites all read
   `z == 47 or z == 46`, silver and palladium. The fixture is generated by the
   stock Python, so the tests replay with `upstream_cadmium_typo = true`; the
   corrected behaviour is **not code-to-code verifiable by construction**,
   since upstream cannot produce it, and is pinned by
   `accident::tests::the_cadmium_typo_changes_palladium` instead.
2. **Parent-decay wiring.** Upstream's `nuclide_import` assigns the flag with
   `==`, so its short-lived-parent test never runs. `ParentDecayPolicy`
   defaults to the intended logic and offers `UpstreamTableDefault` for
   bug-compatible runs. Neither is fixture-verifiable: the flag never leaves
   upstream's shared table, so there is nothing to read out.

Both are the same shape as the first pass's `RB_fail_Noble_Gases` and
`clean_up` divergences — reproduce where it is a physics choice, correct where
the code states an intent its own syntax defeats, and never silently.

## Third pass — exhaustive widening and the end-to-end composition (2026-09-21)

The second pass verified every *function* on the accident path. It did not
verify the **composition** — the order those functions are called in, the
arguments passed between them, or the array layouts they assume — and several
of its groups were thin enough that a branch could hide inside one. Both were
closed in a third pass the same day. The fixture grew from 11 855 to
**14 130 cases** and the suite from 42 to **51 tests**, against the same
upstream `de374c8`.

### What was widened, and why it mattered

| Group | Cases before | after | What the widening reached |
|---|---:|---:|---|
| `nuclide_import.short_lived` | 30 | **504** | every one of the 84 supported nuclides × 6 irradiation times, not a 10-nuclide sample |
| `nuclide_import_accident.retained` | 30 | **420** | the same, × 5 accident durations |
| `booth_transient` | 8 | **53** | three points per decade over `1e-12 .. 1e2`, which is what exposed defect 10 |
| `rf_graph` | 18 | **135** | both arguments swept independently, into the saturated regime |
| `diffusion_coefficient_sic_ag` | 20 | **102** | 25 K steps across 250–2500 °C, ~30 decades of `D` |
| `inventory_processing` | 16 | **~1 900** | **every** axial slot, not just slot 0 — the operation is a divide *and* a repeat |
| `coolant_release.*` | 32 | **331** | 1-, 2-, 3- and 6-node fields, every hot-node choice, three pressures |

Three surfaces earlier described as "not verifiable by construction" turned out
to be directly testable and were added: the nuclide-name regex
(`name_normalisation`, 31 cases), `convert_time`'s unit factors (5), and
`nuclide_sort`'s dead reordering branch (5). The first of those found defect 8.

### The end-to-end `accident_case` composition

`trisoatops.accident_case` is driven whole, over three scenarios — a
contiguous heat-then-cool transient with and without the clean-up system, and
one whose venting mask is deliberately gappy — for six nuclides spanning every
upstream branch (noble gas, halogen, the Te-counted-as-halogen case, silver,
and two ordinary fission metals). The port side is composed from public
functions **in the order the module doc comments prescribe**, so this group
tests the documented wiring, not just the formulas.

| Group | Cases | max_rel_dev | Worst case's share of its tolerance |
|---|---:|---:|---:|
| `accident_case.dropped` | 18 | exact (decisions) | — |
| `accident_case.total` | 54 | 3.63e-15 | 0.0 % |
| `accident_case.nodal_graphite` | 180 | 2.22e-13 | 0.0 % |
| `accident_case.nodal_kernel` | 180 | 2.44e-9 | 19 % |

The kernel group's `2.44e-9` exceeds the `1e-9` group tolerance and is admitted
only by the per-case conditioning widening described above: it is an Ag-110m
row, whose release fraction comes from `breakthrough_model_transient`, the most
cancellation-prone expression in the code. The conditioning is **inherited, not
re-derived** — the generator reads the integrals `accident_case` actually used
off the returned dataset and feeds them to the same `cond_breakthrough` the
standalone `release_fraction.kernel` group uses.

Upstream's `vectorized_format` — `np.format_float_scientific(x, precision=2)`,
i.e. three significant digits, as strings — is a *display* step. It is patched
to an identity for the duration of the call so the fixture records upstream's
full-precision values, and restored immediately afterwards. Nothing else about
the upstream call is altered. Comparing against the formatted output instead
would have capped this whole group's resolution at ~1e-3 and hidden every
result in the table above.

Each scenario is written to the fixture **once**, as its own
`accident_case.scenario:<id>` row, and the 432 value rows refer to it by
index. Repeating the ~170-number scenario on every value row made the
committed CSV 44 % scenario prefix — 2.6 MB against 1.5 MB — and the whole
file is `include_str!`d into the test binary, so that is compile time and
binary size, not just disk.

**This is the group that found defect 9.** The first end-to-end run disagreed
by 36 % on one node of the gappy-mask scenario, and the cause was upstream
pairing the *venting times* with the *first-n temperature slices* — two
different sample sets whenever the mask has a hole in it. The port reproduces
the pairing deliberately, with the reason stated at the call site.

### Non-vacuity, third pass

Eleven mutations, 2026-09-21. Each edits the port, runs the named test, and is
reverted:

| Mutation | Test | Result |
|---|---|---|
| `normalise_nuclide_name` drops the metastable suffix | `name_normalisation_matches_upstream_regex` | **killed** |
| `TimeUnit::Year` → 365.25 days | `convert_time_factors_match_upstream` | **killed** |
| `distribute_inventory_axially` fills only slot 0 | `inventory_processing_matches_upstream` | **killed** |
| `mean_temperature_rate` scaled by `1 + 1e-6` | `coolant_mean_temperature_rate_matches_upstream` | **killed** |
| `coolant_release` drops one power of `T` | `coolant_release_fraction_matches_upstream` | **killed** |
| `rf_graph` sums even terms too | `rf_graph_matches_upstream` | **killed** |
| `diffusion_coefficient_sic_ag` 215 → 216 kJ/mol | `diffusion_coefficient_sic_ag_matches_upstream` | **killed** |
| `sort_parents_before_daughters` made a no-op | `upstream_nuclide_sort_never_reorders` | **killed** |
| `accident_release_curies` scales the circuit term by `frac` | `accident_case_total_matches_upstream` | **killed** |
| `release_activity` drops the plate-out subtraction | `accident_case_nodal_kernel_matches_upstream` | **killed** |
| `booth_transient`'s `RF < 1e-6 → 0` guard deleted | `booth_transient_matches_upstream` | **equivalent mutant** |

The last one is not a gap. The guard is **unreachable** — the 5000-term
truncation leaves a residual of `6/(π²·5000) = 1.216e-4`, two orders of
magnitude above the `1e-6` it tests against — so deleting it cannot change any
output. That is upstream defect 10, and it is asserted directly by
`booth_transient_zero_floor_is_unreachable` rather than left as a mutation
score footnote. A surviving mutant that is provably equivalent is a statement
about the *code*, not about the test suite.

### A documented port divergence: the `dTdt_avg >= 0` knife edge

Upstream's venting mask tests a floating-point mean against zero. On a
symmetric field — one node heating at `+0.2 K/s`, another cooling at
`-0.2 K/s` — the true mean is exactly zero. Upstream works in degrees Celsius
throughout and gets a clean `0.0`, so every sample vents. The port stores
temperature as `uom`'s `ThermodynamicTemperature`, i.e. in kelvin, and
`800 °C` and `900 °C` are not exactly representable after the `+273.15` shift
(they return as `800.000000000000114`). The differences therefore do not cancel
exactly, the mean lands at `-2.220446049250313e-16`, and the mask drops those
samples: **upstream vents 4 of 4, the port vents 1.**

This is not a physics difference and not a formula error — it is a `>= 0` test
on a quantity whose true value is zero, which no reimplementation in any
language can be relied on to reproduce. Computing in Celsius would trade it for
the same defect in the other direction and break the workspace's `uom` rule;
the conditioning, not the unit, is the problem. It is recorded rather than
tuned away, under its own fixture group `coolant_release.knife_edge.*`, and
asserted by `coolant_release_knife_edge_divergence_is_representation_noise` —
which fails if the disagreement ever exceeds `3e-16` or disappears.

## What this does NOT establish

This is **verification, not validation**. It shows the Rust port computes what
the upstream Python computes. It says nothing about whether *either* reproduces
measured TRISO fission-product release — that requires a public benchmark and is
not claimed here. Per `RESPONSIBLE_USE.md`, AI-assisted output remains untrusted
draft material until a human reviews it, and that review is outstanding here.

> ~~"…and the `Bookkeeping status` block in the crate README records that
> review as outstanding."~~ **CORRECTED 2026-09-21** — verified by `ls`:
> **`crates/boon-lay/` has no `README.md` at all**, so there is no such block
> and nothing records the sign-off state. Seven other crates are in the same
> position (`dhoby-ghaut`, `kovan-codegen`, `kovan-common`, `kovan-literature`,
> `kovan-semantics`, `redhill`, `sembawang`). Filed as a follow-up; creating
> the block is a bookkeeping-pass task, and **only the maintainer may clear
> either axis**. Until then, treat this crate as INCOMPLETE on both axes by
> default — which is what the block would say anyway.

~~Still not covered, and tracked in `bn:op-b4a.2.7`:~~
**CLOSED 2026-09-21** — every physics item below was ported and verified in the
second pass above. Kept struck through rather than deleted, because the list
shaped the decision about what to port next:

- ~~`coolant_release`~~ — ported; vent times bit-exact, fraction 4.69e-16;
- ~~`nuclide_import` / `nuclide_import_accident`~~ — ported; all 60
  classification decisions match upstream exactly;
- ~~`inventory_processing`~~ — ported, bit-exact;
- ~~`release_activity`~~ — ported, bit-exact across 6 048 cases;
- ~~the JSON run-file driver and accident-case entry point~~ — ported as
  `run_file::{RunFile, RunConfig}` and `accident::accident_release_curies`.
  Neither is in the code-to-code fixture: the run file is I/O with no numeric
  output to compare, and the accident composition is verified through its
  parts. Unit-tested instead.
- **the upstream Tkinter GUI — still intentionally not ported**, together with
  `create_log`, `count_errors`, `trisoatops()` and `main`'s argparse shell.
  See "What is deliberately not ported" in `triso-atops-fork.md`.

`higher_activities` has no standalone counterpart in the port and is **not**
untested: it is covered end-to-end through `normal_operation_node`, which is the
composition upstream's own `trisoatops.py` driver performs. That distinction was
worth getting right — the group-dependent zeroing of `k_plate` for noble gases
and `k_clean` for non-halogens happens at the *driver* call site upstream, not
inside `higher_activities`, so a test written against `higher_activities` alone
would have compared the wrong thing and made the port look wrong when it is
correct.
