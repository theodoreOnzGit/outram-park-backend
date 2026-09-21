# SCRAM port — code-to-code verification

**Date:** 2026-09-21.
**Status:** verification only. No human V&V, no crate-owner review. Under
`RESPONSIBLE_USE.md` this is AI-assisted draft material.

**Scope note — this was the workspace maintainer's direction, not the crate
owner's.** RAFFLES belongs to Adolphus Lye and fault-tree quantification is not
part of the RAVEN-derived statistical core the crate was scoped to. Whether
`scram` belongs in RAFFLES at all is theirs to confirm or reverse; it is
recorded in the crate `README.md` and `CLAUDE.md` so it is visible rather than
absorbed silently.

## What was ported

`raffles::scram` takes a fault tree to a top-event probability and a ranking of
which basic events matter. Three of its four modules are ports of
[SCRAM](https://github.com/rakhimov/scram); one deliberately is not.

| RAFFLES | upstream | port? |
|---|---|---|
| `scram::probability::cut_set_probability` | `CutSetProbabilityCalculator::Calculate` | yes |
| `scram::probability::top_event_probability` (`RareEvent`) | `RareEventCalculator::Calculate` | yes |
| `scram::probability::top_event_probability` (`Mcub`) | `McubCalculator::Calculate` | yes |
| `scram::probability::top_event_probability` (`Exact`) | — | **no** — inclusion-exclusion, a second route to what the BDD gives |
| `scram::bdd::Bdd::probability` | `ProbabilityAnalyzer<Bdd>::CalculateProbability` | yes |
| `scram::bdd` (the diagram itself) | — | **no** — Bryant's algorithm from the tree, where upstream builds from a preprocessed `Pdag` |
| `scram::importance::importance_factors` (the five derived factors) | `ImportanceAnalyzerBase::Analyze` | yes |
| `scram::importance::importance_factors` (the Birnbaum factor) | — | **no** — the definition `P(top\|e) - P(top\|not e)` where upstream differentiates the BDD |
| `scram::fault_tree::Connective` | `src/pdag.h` `enum Connective` | taxonomy only |
| `scram::fault_tree` (the rest) | — | **no** — upstream builds its tree from XML through `Initializer`/`Model`/`Formula`; this is a plain indexed structure a caller builds in Rust |
| `scram::mocus` | `src/mocus.cc` | **no** — see below |

**`scram::mocus` is not a translation of `src/mocus.cc`, and says so in its
own module doc.** Upstream's MOCUS is a 108-line driver over a ZBDD
`CutSetContainer` operating on a `Pdag` that a 2,411-line preprocessor has
already rewritten. Porting it faithfully means porting `zbdd` (1,953 lines),
`pdag` (2,283), `preprocessor` (3,477) and `bdd` (1,363) — 9,076 lines of
dense C++, the preprocessor being the part most likely to be silently wrong.
Instead this module implements the **classical top-down MOCUS expansion** from
the published literature (Fussell & Vesely 1972; see Rauzy 1993 for why a
BDD/ZBDD formulation supersedes it on large models) and is verified *against*
upstream's products. That is a stronger result than a translation would give,
for the same reason the `Exact` and Birnbaum rows above are: two unrelated
algorithms agreeing is evidence, a translation agreeing with its original is
much less.

**Still absent.** XML input handling (explicitly out of RAFFLES' scope), event
trees, alignments, CCF groups, substitutions, house events, the `expression`
library, the ZBDD algorithm, the preprocessor, and prime implicants
(upstream's `--prime-implicants`).

~~and complement elimination (so non-coherent trees are **refused**, not
approximated)~~ **CORRECTED 2026-09-21** — complement elimination landed the
same day; see
[Non-coherent trees](#non-coherent-trees--handled-conservatively-and-verified)
below.

~~and upstream's probability cut-off on products (`Settings::cut_off_`,
default `1e-8` — this port truncates by cut-set order only)~~ **CORRECTED
2026-09-21** — that claim was wrong, and the correction is a finding about
upstream rather than about this port. See
[Upstream's probability cut-off does nothing](#upstreams-probability-cut-off-does-nothing)
below.

## Licence

SCRAM is **GPL-3.0-or-later**; RAFFLES is **GPL-3.0-only**. Code may flow
SCRAM → RAFFLES freely — this is a same-licence port, **not** the one-way
Apache-2.0 → GPLv3 relicensing that governs the RAVEN-derived parts of this
crate. Every ported file carries the upstream attribution header with
`Copyright (C) 2014-2018 Olzhas Rakhimov`.

## The oracle

Upstream was **built from source and run**. Nothing here is derived from
reading SCRAM's source and reasoning about what it would produce.

| | |
|---|---|
| Repository | <https://github.com/rakhimov/scram> |
| Commit | `b85b78940de38996eeffec54d946824bd4280a1c` (2019-07-03) |
| Version reported | SCRAM 0.16.2 |
| Toolchain | GCC, Boost 1.83.0, libxml2 2.9.14 |
| CMake | `-DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF -DWITH_TCMALLOC=OFF -DWITH_JEMALLOC=OFF` |
| Models | upstream's **own** `input/` suite — not inputs invented for this port |

### One build patch was needed, and it touches no arithmetic

SCRAM 0.16.2 predates Boost 1.83 and fails to compile against it with 42
instances of one error:

```
'BOOST_THROW_EXCEPTION_CURRENT_FUNCTION' was not declared in this scope
```

That macro was Boost's internal alias for `BOOST_CURRENT_FUNCTION` and has
since been removed. `src/error.h` line 45 was changed to use
`BOOST_CURRENT_FUNCTION` directly; `boost/current_function.hpp` was already
included. **The macro expands to a function-name string attached to thrown
exceptions for diagnostics** — it cannot affect a computed value, and after the
change the build produced only warnings.

### The test suite was NOT built, and that is a gap

`tests/` fails to compile: the vendored Catch2 uses `SIGSTKSZ` in a constant
expression, which modern glibc no longer permits. This is a Catch2/glibc
incompatibility, unrelated to SCRAM's algorithms.

Consequence: **upstream's own unit tests were never run**, so there is no
independent confirmation that this build of SCRAM behaves as its authors
intended. The oracle rests on the `scram` CLI producing correct results, which
is the same code path but not the same check. Upgrading the vendored Catch2
would close this; it was not attempted.

### Extraction — two fixtures, deliberately separate

| file | parsed from | trusted for |
|---|---|---|
| `reference-data/scram/oracle.txt` | SCRAM's own **XML report** | the answers — cut sets, basic-event probabilities, totals, importance |
| `reference-data/scram/models.txt` | SCRAM's own **input models** | the question — gates, connectives, arguments, the top gate |

`extract_oracle.sh` runs the binary and reads only its report: basic-event
probabilities from the `<importance>` section, cut sets from `<product>`
elements, totals from three separate runs (`--rare-event`, `--mcub`, and no
flag for the exact BDD path). Nothing is read out of the input models, so no
answer in it can have been copied from the question.

`extract_models.sh` is the mirror image: it reads only the input models and
emits only structure, never a number. That is what lets
`tests/scram_mocus_oracle.rs` generate cut sets here and compare them against
the ones SCRAM found, with SCRAM's answer entering nowhere except as the thing
being compared to. Basic-event probabilities still come from the **oracle**,
because several models (`HIPPS` especially) define them through `periodic-test`
and `GLM` expressions that SCRAM evaluates and this crate does not.

**Both scripts refuse rather than guess**, and each refusal is recorded in the
fixture instead of being silently dropped:

- **More than one `<sum-of-products>`** — a model defining several fault trees
  produces one result set per tree, and the fixture format has nowhere to say
  which product belongs to which. `TransTest/trans_one` (2 trees) and
  `ThreeLevels/top` (3) are excluded by this. The guard was added *after* both
  had been compared successfully, when the merge was noticed; the earlier pass
  on `TransTest` had been luck, since its report's total covers one tree while
  its product list merges two.
- **More than 40 cut sets** — the Aralia benchmarks reach 75,379 products and
  exhausted memory on a first attempt.
- **A gate the structure parser cannot read** — a nested formula, a house event,
  an `xi:include`d model, or an ambiguous top gate. `ThreeMotor/three_motor`
  trips this on all of house events, and four candidate top gates.

**Eight models were captured**: `TwoTrain`, `Theatre`, `SmallTree`,
`ThreeMotor`, `BSCU`, `Lift`, `HIPPS`, `ne574` — 58 basic events and 58
products. Seven of them are structurally readable and take part in the
end-to-end comparison; `ThreeMotor` is quantification-only.

## Results (2026-09-21)

Everything below was produced by running the tests, not by reading the code.
Tolerance throughout is `5e-6` relative — the resolution of upstream's
6-significant-figure report, not a number chosen to make anything pass.

| check | test | result |
|---|---|---|
| Cut sets generated here vs SCRAM's products | `scram_mocus_oracle::generated_cut_sets_match_the_ones_scram_found` | **8 models, 438 cut sets, exact set equality** |
| Input model to top-event probability, end to end | `scram_mocus_oracle::quantifying_generated_cut_sets_reproduces_scrams_totals` | **23 of 23 model/mode combinations** |
| Importance from cut sets generated here, end to end | `scram_mocus_oracle::ranking_generated_cut_sets_reproduces_scrams_importance_factors` | **47 basic events x 5 factors, 7 models** |
| Top-event probability from SCRAM's own cut sets | `scram_oracle_suite::every_model_total_probability_matches_scram` | **26 of 26 model/mode combinations** |
| Importance from SCRAM's own cut sets | `scram_oracle_suite::every_importance_factor_matches_scram` | **58 basic events x 5 factors** |
| Minimality, independent of SCRAM | `scram_mocus_oracle::generated_cut_sets_are_minimal_by_construction_not_by_luck` | 438 cut sets, 2,580 tree evaluations |
| Order truncation | `scram_mocus_oracle::truncating_by_order_drops_exactly_the_long_cut_sets` | 20 (model, limit) cases |
| **Non-coherent** cut sets vs SCRAM's | `scram_noncoherent::generated_cut_sets_of_a_non_coherent_tree_match_scram` | exact set equality |
| **Non-coherent** conservatism, measured | `scram_noncoherent::our_exact_value_exceeds_scrams_because_cut_sets_are_conservative` | **+23.6 %**, and necessarily above |
| Quantification at scale | `scram_noncoherent::quantifying_4259_cut_sets_matches_scram` | **4,259 cut sets, 108 basic events** |
| **Exact probability, no cut-set ceiling** | `scram_bdd_oracle::exact_probability_matches_scrams_bdd_on_every_model` | **10 of 10 models**, up to 386,261 nodes |
| BDD against inclusion-exclusion | `scram_bdd_oracle::the_bdd_and_inclusion_exclusion_agree_where_both_can_run` | 7 models, to **1e-12** |
| BDD gets what cut sets cannot | `scram_bdd_oracle::the_bdd_gets_the_non_coherent_answer_that_cut_sets_cannot` | `0.5032` exactly |

The whole SCRAM suite — all 15 tests across three files plus the module's own
unit tests — runs in about 1.5 s in release mode.

### Cut-set generation — 438 of 438, no extra and none missing

Compared **as a set of sets**, by basic-event name. A missing cut set and a
spurious one are different defects and both fail.

| model | cut sets, RAFFLES and SCRAM | max order |
|---|---|---|
| TwoTrain/two_train | 4 | 2 |
| Theatre/theatre | 2 | 2 |
| SmallTree/SmallTree | 2 | 2 |
| BSCU/BSCU | 10 | 2 |
| Lift/lift | 12 | 1 |
| HIPPS/HIPPS | 9 | 2 |
| ne574/ne574 | 7 | 3 |
| **Aralia/chinese** | **392** | **6** |

`Aralia/chinese` carries most of the weight and was added for exactly that
reason: at 392 cut sets of up to order 6 it is nearly nine times the rest of
the fixture combined, and it is the only model with enough depth for a
generation defect to have room to show. The seven smaller models bottom out at
order 3.

**This is the strongest result in this document.** SCRAM generates these with a
ZBDD over a Boolean graph its preprocessor has rewritten; this port runs the
classical top-down expansion with absorption. The two share no code and no
data structure, and they agree exactly.

`HIPPS` earns its place twice over: it is the only model here with an
`atleast` gate (`min="2"` of three pressure switches), so its nine cut sets are
the only check that the combination expansion is right, and its basic-event
probabilities are ones SCRAM *computes* from `periodic-test` and `GLM`
expressions rather than reading as literals.

### Top-event probability — 26 of 26 combinations agree

RAFFLES / SCRAM, from SCRAM's own cut sets:

| model | exact | rare-event | MCUB |
|---|---|---|---|
| TwoTrain | 0.722500000 / 0.722500000 | 1.000000000 / 1.000000000 | 0.838393750 / 0.838394000 |
| Theatre | 0.002070000 / 0.002070000 | 0.002100000 / 0.002100000 | 0.002099100 / 0.002099100 |
| SmallTree | 0.026776840 / 0.026776800 | 0.026958531 / 0.026958500 | 0.026776840 / 0.026776800 |
| ThreeMotor | 0.021153782 / 0.021153800 | 0.021201280 / 0.021201300 | 0.021176783 / 0.021176800 |
| BSCU | 0.112408535 / 0.112409000 | 0.135371755 / 0.135372000 | 0.128587773 / 0.128588000 |
| Lift | 0.000012000 / 0.000012000 | 0.000012000 / 0.000012000 | 0.000012000 / 0.000012000 |
| HIPPS | 0.001620905 / 0.001620910 | 0.001621881 / 0.001621880 | 0.001620905 / 0.001620910 |
| ne574 | 0.662208000 / 0.662208000 | 1.000000000 / 1.000000000 | 0.726479149 / 0.726479000 |
| Aralia/chinese | *skipped, 392 cut sets* | 0.001200259 / 0.001200260 | 0.001199599 / 0.001199600 |

**The exact column is the strong one.** SCRAM computes it by BDD traversal;
this port by inclusion-exclusion over the cut sets. Two different algorithms
for the same quantity, agreeing to the report's resolution.

Twenty-three of these 26 were then reproduced **end to end** — cut sets
generated here rather than read from the oracle — which is strictly stronger,
because a generation defect that happened to cancel against a quantification
defect would pass the first form and fail the second. The three not repeated
are `ThreeMotor`'s, whose structure the parser refuses.

`Aralia/chinese`'s exact value is skipped in both forms: inclusion-exclusion is
`2^n` and 392 cut sets is `2^392` terms. Its rare-event and MCUB values are
checked, and they are the two that matter for a model of that size anyway —
which is also the honest reading of `Approximation::Exact`'s 20-cut-set cap.

**TwoTrain's rare-event row exercises the clamp.** The raw sum is
`0.25 + 0.35 + 0.35 + 0.49 = 1.44`, and both report 1 — so upstream's
`return sum > 1 ? 1 : sum;` is tested rather than merely translated. It is also
the approximation failing loudly: these basic events are far too likely for
"rare event" to mean anything. `ne574` clamps for the same reason.

### Importance factors — 58 from SCRAM's cut sets, 47 end to end

Every event of every model, all five measures, plus **exact** agreement on the
integer occurrence counts.

Upstream obtains the Birnbaum factor by differentiating its BDD
(`CalculateMif` walking ite vertices); this port obtains it from the definition
`P(top | event) - P(top | not event)` evaluated on the cut sets. The other four
factors are all derived from MIF, so an error in it would move every column at
once — and none moved.

**47 of those events are also ranked from cut sets generated here**, closing
the last step of the pipeline that had only ever been checked against SCRAM's
own products. That form can distinguish what the first cannot: a spurious cut
set that duplicated an existing one's events would leave the totals intact but
move an occurrence count, and the counts are compared exactly. It is capped at
the seven models within the 20-cut-set exact limit, since upstream runs its
importance analysis on the exact BDD value and matching it requires
`Approximation::Exact`.

### Minimality, checked without reference to SCRAM

Agreeing with SCRAM would not by itself prove the cut sets are *minimal* — a
generator that forgot absorption produces a superset-laden list that still
quantifies to roughly the right answer under the rare-event approximation. So
all 438 are also checked against the definition directly, with an evaluator
written in the test file rather than taken from the library:

1. no cut set properly contains another;
2. no cut set appears twice;
3. every cut set, set true with all other events false, really does make the
   tree evaluate true (438 evaluations);
4. removing any single member makes it evaluate false (2,142 evaluations).

All four hold for all 438. Check 1 is `O(n^2)` in the cut-set count, so
`Aralia/chinese` alone contributes about 153,000 subset comparisons.

### Non-coherent trees — handled, conservatively, and verified

A tree is **non-coherent** when a `not`, `nand`, `nor` or `xor` appears in its
logic, so a component *working* can contribute to the top event. `scram::mocus`
expands a negated gate through its De Morgan dual, drops any partial set
requiring an event both to occur and not to, then deletes the complements and
re-minimises — which is what upstream does in ZBDD form:
`Zbdd::EliminateComplement` OR-merges the two branches of a negative-index node
(deleting the literal) and `Zbdd::Minimize` absorbs.

**Upstream has no small non-coherent model, so one was written.** Of its seven
inputs containing a negating connective, four produce no products, two are
event-tree or alignment models this port does not handle, and the only usable
one is `Aralia/das9601` at 288 gates and 4,259 products — from which a
disagreement could not be diagnosed. The small model is committed at
`reference-data/scram/models-for-this-port/noncoherent_small.xml` with its
reasoning in an XML comment. **SCRAM remains the oracle**: only the question is
ours.

It is `a AND NOT b`, OR'd with `c XOR d` — small enough to check by eye. The
first branch gives the implicant `{a+, b-}`; the `xor` gives `{c+, d-}` and
`{c-, d+}`. Deleting the complements and absorbing leaves `{a}`, `{c}`, `{d}`,
which is exactly what SCRAM reports.

#### The conservatism is real, measured, and must not be "fixed"

For a non-coherent tree, SCRAM's reported probability with no approximation
flag is the **true function's**, from its BDD. This port has no BDD, and its
`Approximation::Exact` is inclusion-exclusion over the *minimal cut sets* —
which describe a strictly larger function, because deleting the negative
literals throws away the requirement that a component be working.

| quantity | RAFFLES | SCRAM | |
|---|---|---|---|
| rare-event | 0.800000 | 0.800000 | agree — both sum the same 3 cut sets |
| MCUB | 0.622000 | 0.622000 | agree — both combine the same 3 cut sets |
| "exact" | **0.622000** | **0.503200** | **differ, necessarily: +23.6 %** |

The two approximations are computed *from the cut sets* by both codes, so they
must agree; the exact value is not, and must not. The test asserts ours is
strictly **above** SCRAM's — a run where it was not would mean the elimination
had lost something — and pins the gap to the measured 23.6 %.

The same shows up on a real model: `das9601`'s rare-event figure is 11 % above
its own exact BDD value.

#### `das9601` is beyond this algorithm, and it says so

288 gates carrying 12 `xor`s, each of which doubles the branch count, is the
case Rauzy's 1993 paper was written about and the reason upstream uses a ZBDD.
This port's top-down expansion **exceeds its 5,000,000-state ceiling** on it —
at the default order limit of 20 after **28.9 s**, and again at order limit 9
(SCRAM's own highest product order for this model, hence the tightest limit
that could still reproduce its answer) after a further **26.3 s**.

Both return the named expansion-limit error rather than hanging, being killed,
or returning a wrong answer, and that is what
`scram_noncoherent::das9601_is_beyond_this_algorithm_and_says_so` asserts. It
is `#[ignore]`d — 57 s to demonstrate a negative, against about 1 s for the
rest of the SCRAM suite — and run with `-- --ignored`.

`das9601` stays in the fixture regardless, because its **4,259 cut sets over
108 basic events** exercise the quantification layer at a scale nothing else
here reaches: rare-event `0.004783225` against SCRAM's `0.004783220`, MCUB
`0.004772037` against `0.004772040`.

### Exact probability by BDD — 10 of 10 models, no ceiling

Every other exact-probability result in this document goes through
inclusion-exclusion over minimal cut sets, which is `2^n` in their number and
refuses past 20. That left the strongest quantification claim resting on models
of at most 12 cut sets, and left the two largest models unchecked exactly at
all. `scram::bdd` removes the ceiling: it evaluates the Boolean function
directly and needs no cut sets.

| model | | RAFFLES | SCRAM | nodes | products |
|---|---|---|---|---|---|
| TwoTrain/two_train | coherent | 0.722500000 | 0.722500000 | 8 | 4 |
| Theatre/theatre | coherent | 0.002070000 | 0.002070000 | 5 | 2 |
| SmallTree/SmallTree | coherent | 0.026776840 | 0.026776800 | 8 | 2 |
| BSCU/BSCU | coherent | 0.112408535 | 0.112409000 | 34 | 10 |
| Lift/lift | coherent | 0.000012000 | 0.000012000 | 62 | 12 |
| HIPPS/HIPPS | coherent | 0.001620905 | 0.001620910 | 41 | 9 |
| ne574/ne574 | coherent | 0.662208000 | 0.662208000 | 18 | 7 |
| **Aralia/chinese** | coherent | **0.001170582** | **0.001170580** | 259 | **392** |
| noncoherent_small | **non-coherent** | 0.503200000 | 0.503200000 | 10 | 3 |
| **Aralia/das9601** | **non-coherent** | **0.004234403** | **0.004234400** | **386,261** | **4,259** |

`das9601` is the result to read first: 288 gates, 14 `not`, 12 `xor`, a model
whose cut sets `scram::mocus` cannot reach at any order limit, now exact to
upstream's reported precision. The whole file runs in **0.3 s**.

**Both sides are BDDs, so is this a cross-check?** The representation agrees by
construction; nothing else does. SCRAM builds its diagram from a `Pdag` a
2,411-line preprocessor has rewritten, with modules, complement edges and its
own variable ordering; this builds a plain Bryant diagram straight from the
tree, with explicit terminals and a first-appearance order. Only the
probability recurrence is a port. Two further checks pin it down:

- **Against a genuinely unrelated algorithm.** On the seven coherent models
  within the cut-set limit, the BDD and inclusion-exclusion over
  independently-generated cut sets agree to **1e-12** — tighter than the `5e-6`
  used against SCRAM, because there no report precision intervenes.
- **Against closed forms.** Independent AND, OR, XOR, 2-of-3, De Morgan for
  NAND and NOR, and the two tautologies `A OR NOT A` and `A AND NOT A`, which
  must reduce to *terminals* rather than merely evaluate to 1 and 0.

#### It also gets the non-coherent answer, which cut sets cannot

On `noncoherent_small` the BDD gives `0.503200` — SCRAM's value exactly — where
cut-set quantification gives `0.622000`. The gap is not an error in either:
minimal cut sets of a non-coherent tree are conservative by definition. The
test asserts the BDD matches SCRAM **and** that the cut-set figure lies strictly
above it, so the `+23.6 %` is pinned from both sides.

#### A harness defect this found, worth recording

The first run gave `0.487` against SCRAM's `0.5032` on that model. The BDD was
right; the *test* was wrong. SCRAM's report prices only basic events that
survive into some product, and `b` survives into none — the cut-set tests
substitute an arbitrary sentinel for such an event and assert no arithmetic
reaches it. **A BDD evaluates the whole function**, so an event outside every
cut set still moves the answer, and `0.487` is precisely the value for
`p(b) = 0.5`.

The fix is that `extract_models.sh` now also emits `PARAM` records — the
probability the *model declares*, for events given a direct `<float>`. That is
part of the question, not an answer, and where the report also has a value the
two are asserted to agree. An expression-defined event outside every product
has no value from either source, and the model is skipped rather than guessed
at.

### One deliberate divergence: RRW at the singularity

`Theatre/theatre`, `Mains_Fail`: MIF, CIF, DIF and RAW all match SCRAM
exactly. **RRW does not** — upstream reports `0`, this port reports infinity.
It is the only singular point in the fixture.

The denominator `p_total - p * MIF` vanishes exactly when the event lies in
every cut set: `p = 0.03`, `MIF = 0.069`, `p_total = 0.00207`, and
`0.03 x 0.069 = 0.00207`. Then `P(top | not event) = 0` — removing the event
removes all risk — so RRW genuinely diverges. Upstream guards the division and
leaves `rrw` at its zero-initialised value, reporting a 0 that RRW cannot
otherwise take, since it is bounded below by 1.

**This port cannot copy upstream's guard, and finding out why was the most
useful thing this exercise produced.** SCRAM tests the denominator with exact
float equality:

```cpp
if (p_total != p_var * imp.mif)
  imp.rrw = p_total / (p_total - p_var * imp.mif);
```

That works only because SCRAM's BDD traversal happens to produce bit-identical
values on both sides. Computing the same quantities a different way lands a few
ulp off, falls straight through `!=`, and divides by a denominator of order
1e-19. Measured, before the fix: **RRW = -4.77e15** — not merely imprecise but
negative, for a quantity bounded below by 1.

The port therefore detects the singularity by relative magnitude
(`SINGULARITY_TOLERANCE = 1e-12`) and returns infinity. The divergence is
asserted, not merely described: the test requires that wherever this port goes
singular, SCRAM reported exactly 0 **and** the event occurs in every cut set —
so if the explanation is ever wrong, the test fails rather than the claim
quietly rotting.

### Upstream's probability cut-off does nothing

An earlier revision of this document listed SCRAM's product probability
cut-off among the things not ported, and said a model where it bites would
diverge. **That was wrong**, and finding out why is a result about upstream.

`Settings::cut_off_` defaults to `1e-8`, is settable from the CLI
(`--cut-off`) and from a project file, and is range-validated on the way in.
Its getter `Settings::cut_off()` is declared at `src/settings.h:122` and has
**no callers anywhere in SCRAM 0.16.2** — the only mentions in the whole tree
are the declaration, the setter, the default, the CLI binding in `scram.cc`
and the project-file binding in `project.cc`. Nothing in the analysis reads
it, so no product is ever discarded by probability.

Confirmed by running it rather than only by grep, because a grep can miss a
call through an alias:

| run | products | total |
|---|---|---|
| `scram --probability` | 392 | 0.00117058 |
| `scram --probability --cut-off 1e-8` | 392 | 0.00117058 |
| `scram --probability --cut-off 1e-4` | 392 | 0.00117058 |
| `scram --probability --cut-off 0.5` | 392 | 0.00117058 |

A cut-off of 0.5 on a model whose top-event probability is `1.17e-3` should
discard every product it has. It discards none.

So there is **nothing to port**, truncating by cut-set order only is not a
divergence, and the agreement measured on all eight models is not luck about
where the cut-off happens not to bite. This is recorded rather than raised
upstream: `rakhimov/scram`'s last commit is from 2019 and filing against a
third-party project was not part of this task.

## What this does NOT establish

- **It is not validation.** Agreement with SCRAM shows this port reproduces
  SCRAM's answers. It says nothing about whether fault-tree quantification is
  the right model for any particular system, and nothing about whether any
  particular tree describes a real one.
- **Upstream's own tests never ran** (Catch2/glibc, above), so the oracle is
  only as trustworthy as the CLI path.
- **Nine models, 450 cut sets, 83 basic events.** Real PRA models reach tens of
  thousands of cut sets, and the largest Aralia benchmarks in upstream's own
  suite already reach 75,379 — those exhausted memory during extraction and
  are not in the fixture. `Approximation::Exact` cannot scale in principle
  (it is `2^n`, capped at 20 cut sets), and `scram::mocus` is exponential in
  the worst case, which is exactly what upstream's ZBDD exists to avoid.
- **Only 438 of those 450 cut sets were generated here.** `ThreeMotor`'s 12
  are checked against the quantification layer only, because the structure
  parser refuses the model (house events, four candidate top gates).
- **`Approximation::Exact` is still capped at 20 cut sets**, and that is
  inherent — it is `2^n`. It is no longer the only exact route, so the cap is
  a property of that function rather than of the crate.
- **Non-coherent results are conservative, by definition.** Their minimal cut
  sets bound the top-event probability from above; only prime implicants
  (not ported) recover the exact function. Verified on one small written-here
  model and one 4,259-product upstream model, the latter for quantification
  only.
- **`scram::mocus` does not scale to a real PRA model.** It is the classical
  top-down expansion and is exponential; `Aralia/das9601` (288 gates, 12
  `xor`) exhausts its 5,000,000-state ceiling at every order limit tried. Its
  *probability* is no longer blocked by that — `scram::bdd` answers it exactly
  — but its **cut sets** are, and getting those at scale needs the ZBDD, which
  is not ported.
- **The BDD's variable ordering is not optimised.** First appearance in a
  depth-first walk, where upstream spends a preprocessor on the problem.
  `das9601` needs 386,261 nodes under it; a model that blew up would need that
  work, and `Bdd::node_count` is how it would show.
- **Order truncation is checked to order 6 and no further.** `Aralia/chinese`
  gives limits 1-5 that each cut inside the distribution of cut-set orders,
  which is where an over-eager prune would show; the other eight models bottom
  out at order 3. A real PRA truncation at order 8-10 on a model with hundreds
  of thousands of products is still untested.
- **`Connective::Null` has no oracle coverage in the committed fixture.** It
  did have — `ThreeLevels/top` is exactly that shape and agreed — but that
  model was excluded when the multi-result guard was added, and the evidence is
  no longer regenerable. It is covered by a unit test only.
- **`imprecise::SystemStructure` is a different thing** and was checked for
  overlap before any of this was written: it does interval-valued reliability
  of series/parallel/k-of-n structures, not cut-set quantification.

## Reproducing

```bash
# Build upstream (apply the error.h patch above first).
cmake <scram-src> -DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF \
      -DWITH_TCMALLOC=OFF -DWITH_JEMALLOC=OFF
make -j4
cp <scram-src>/share/*.rng share/scram/     # the CLI needs its RelaxNG schemas

# Regenerate both fixtures, over the same model list.
MODELS="input/TwoTrain/two_train.xml input/Theatre/theatre.xml \
        input/SmallTree/SmallTree.xml input/ThreeMotor/three_motor.xml \
        input/BSCU/BSCU.xml input/Lift/lift.xml input/HIPPS/HIPPS.xml \
        input/ne574/ne574.xml input/Aralia/chinese.xml"
reference-data/scram/extract_oracle.sh ./bin/scram $MODELS \
    > reference-data/scram/oracle.txt
reference-data/scram/extract_models.sh $MODELS \
    > reference-data/scram/models.txt

# Check the port against them.
cargo test -p raffles --release --test scram_bdd_oracle -- --nocapture
cargo test -p raffles --release --test scram_mocus_oracle -- --nocapture
cargo test -p raffles --release --test scram_oracle_suite -- --nocapture
cargo test -p raffles --release --test scram_cross_code -- --nocapture
cargo test -p raffles --release --lib scram
```

Each test prints its own comparison table, so the numbers in this document can
be re-derived rather than taken on trust.
