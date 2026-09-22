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
| `scram::zbdd::minimal_cut_sets` | `Zbdd::ConvertBdd`, `Zbdd::Minimize`, `Zbdd::Subsume` | yes |
| `scram::zbdd::prime_implicants` | `Zbdd::ConvertBddPrimeImplicants`, `Bdd::Consensus` | yes |
| `scram::importance::importance_factors` (the five derived factors) | `ImportanceAnalyzerBase::Analyze` | yes |
| `scram::importance::importance_factors` (the Birnbaum factor) | — | **no** — the definition `P(top\|e) - P(top\|not e)` where upstream differentiates the BDD |
| `scram::fault_tree::Connective` | `src/pdag.h` `enum Connective` | taxonomy only |
| `scram::fault_tree` (the rest) | — | **no** — a plain indexed structure a caller builds in Rust; `scram::mef` is what builds one from XML |
| `scram::expression` | `src/expression/*.cc` (`p_exp`, GLM, Weibull, periodic test, the numeric and Boolean operators) | yes |
| `scram::expression`'s seven random deviates (`value`, `Validate`, `interval`) | `src/expression/random_deviate.{h,cc}` | yes |
| `scram::expression::Interval` | `Interval`, `Contains`, `IsNonNegative`, `IsPositive`, `IsProbability` | yes |
| `Expression::Sample` / each deviate's `DoSample` | — | **not yet** — lands with the uncertainty analysis, where `scram --uncertainty` is the oracle |
| `scram::mef` | `src/initializer.{h,cc}`, `src/xml.{h,cc}`, `src/element.{h,cc}`, `src/model.{h,cc}`, `src/fault_tree.{h,cc}`, `src/event.{h,cc}` | yes |
| `scram::mef::Index` (name resolution) | `Initializer::GetEntity` / `GetEvent`, `mef::Id::id()`, `mef::Role` | yes |
| `scram::mef::Lowering` (iff, imply, cardinality) | `Pdag::ConstructComplexGate` | yes |
| `scram::ccf` | `src/ccf_group.{h,cc}` — all four models, `CalculateProbabilities`, `ApplyModel` | yes |
| `scram::substitution` | `src/substitution.{h,cc}`, `Pdag::ConstructSubstitution`/`CollectSubstitution`, `Zbdd::ApplySubstitutions` | yes |
| `scram::mef` schema validation | `xml::Validator` against `share/input.rng` | **no** — no RelaxNG; the structure checks here are narrower, and the module doc says so |
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

**What is left, classified honestly.** After the BDD, the ZBDD, prime
implicants, complement elimination and house events, the remainder is not one
list but three:

~~*Out of RAFFLES' declared scope, and would stay out even if written:* XML
input handling, event trees, alignments, common-cause-failure groups,
substitutions, the `expression` library, and `<define-component>` private
namespaces.~~ **CORRECTED 2026-09-22** — the maintainer's direction
("everything except gui is to be translated with v and v") moved all of these
**into** scope, and the first three of them have since landed:
`scram::expression` (2026-09-22), `scram::mef` including
`<define-component>`'s private namespaces and `<xi:include>` (2026-09-22).
What is still out of the module, and refused rather than skipped when a model
uses it: **common-cause-failure groups, substitutions, event trees,
alignments, the random deviates, and `<define-extern-function>`**. The last is
deliberate and stays out — it loads a shared library, which the workspace
`RESPONSIBLE_USE.md` rule on autonomous access forbids.

*An optimisation rather than a capability:* the **preprocessor**. Upstream
spends 2,411 lines finding a good variable order and extracting modules. This
port has neither, and computes the same answers without them — `das9601`'s
386,261 diagram nodes are the price. A model that blew up under the naive
order would need that work; nothing in the fixture does.

*Genuinely absent, and reachable only through code this port does not have:*
`Zbdd::EliminateComplements`, which belongs to upstream's non-BDD `Zbdd(const
Gate&)` constructor. This port's complement handling goes through the BDD
instead and is verified against upstream's answers, so the routine has no
caller here rather than a missing implementation.

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

### Upstream's own test suite was built and run — 540 of 540 assertions pass

~~`tests/` fails to compile: the vendored Catch2 uses `SIGSTKSZ` in a constant
expression, which modern glibc no longer permits. Consequence: **upstream's
own unit tests were never run**, so there is no independent confirmation that
this build of SCRAM behaves as its authors intended. The oracle rests on the
`scram` CLI producing correct results, which is the same code path but not the
same check.~~

**CORRECTED 2026-09-21.** That limitation stood through five commits of this
port and is now closed. The Catch2/glibc incompatibility is real —
`MINSIGSTKSZ` stopped being a compile-time constant — but it is confined to
Catch2's POSIX signal handling, which Catch2 itself can be told to omit:

```bash
cmake <scram-src> -DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF \
      -DWITH_TCMALLOC=OFF -DWITH_JEMALLOC=OFF -DBUILD_TESTING=ON \
      -DCMAKE_CXX_FLAGS="-DCATCH_CONFIG_NO_POSIX_SIGNALS"
make -j4 scram_tests
```

No source was patched. The flag turns off Catch2's crash-reporting handlers;
it does not touch a test, an assertion or any SCRAM code.

One further failure had to be cleared, and it was a build-layout assumption
rather than a defect: `RiskAnalysisTest.ExternFunctionProbability` loads a
shared library by a path relative to its input file
(`../../../build/lib/scram/scram_dummy_extern`), so it requires the build tree
to sit at `<scram-src>/build`. Symlinking the built
`libscram_dummy_extern.so` there satisfies it.

```
All tests passed (540 assertions in 71 test cases)
```

**This is what the oracle needed.** Upstream's suite contains a test per
benchmark model, and every model in these fixtures has one:
`RiskAnalysisTest.TwoTrain`, `.Theatre`, `.SmallTree`, `.ThreeMotor`, `.BSCU`,
`.Lift`, `.HIPPS`, `.ne574`, `.ChineseTree`. They assert upstream's own
expected products and probabilities, and they pass on this build. So the
numbers in `oracle.txt` are no longer merely "what this binary printed" — they
are what SCRAM's authors say SCRAM should print, checked.

`[perf]`-tagged performance tests are excluded (`~[perf]`); they measure
timings rather than assert answers.

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
and `GLM` expressions that `extract_models.sh` does not evaluate.
~~that SCRAM evaluates and this crate does not~~ **CORRECTED 2026-09-22** —
`scram::expression` evaluates them, and `tests/scram_mef.rs` computes every
basic-event probability from the models' own expressions instead of taking it
from the oracle. The limitation is the shell script's, not the crate's.

**Both scripts refuse rather than guess**, and each refusal is recorded in the
fixture instead of being silently dropped:

- **More than one `<sum-of-products>`** — a model defining several fault trees
  produces one result set per tree, and the fixture format has nowhere to say
  which product belongs to which. `TransTest/trans_one` (2 trees) and
  `ThreeLevels/top` (3) are excluded by this. The guard was added *after* both
  had been compared successfully, when the merge was noticed; the earlier pass
  on `TransTest` had been luck, since its report's total covers one tree while
  its product list merges two.
- **More than 600 cut sets** — the largest Aralia benchmarks reach 75,379
  products and exhausted memory on a first attempt. Left out by maintainer
  direction (2026-09-21) rather than pending.
- **A structure the parser cannot read** — a nested formula, an `xi:include`d
  model, an ambiguous top gate, or a `<define-component>` private namespace.
  `ThreeMotor/three_motor` trips the last of these.

**Eleven models across the three fixtures**: nine of upstream's own —
`TwoTrain`, `Theatre`, `SmallTree`, `ThreeMotor`, `BSCU`, `Lift`, `HIPPS`,
`ne574`, `Aralia/chinese`, plus `Aralia/das9601` in the non-coherent one — and
two written here, `noncoherent_small` and `house_events_small`. Ten are
structurally readable and take part in the end-to-end comparisons;
`ThreeMotor` is quantification-only.

## Results (2026-09-21)

Everything below was produced by running the tests, not by reading the code.
Tolerance throughout is `5e-6` relative — the resolution of upstream's
6-significant-figure report, not a number chosen to make anything pass.

| check | test | result |
|---|---|---|
| Cut sets generated here vs SCRAM's products | `scram_mocus_oracle::generated_cut_sets_match_the_ones_scram_found` | **9 models, 440 cut sets, exact set equality** |
| Input model to top-event probability, end to end | `scram_mocus_oracle::quantifying_generated_cut_sets_reproduces_scrams_totals` | **26 of 26 model/mode combinations** |
| Importance from cut sets generated here, end to end | `scram_mocus_oracle::ranking_generated_cut_sets_reproduces_scrams_importance_factors` | **49 basic events x 5 factors, 8 models** |
| Top-event probability from SCRAM's own cut sets | `scram_oracle_suite::every_model_total_probability_matches_scram` | **29 of 29 model/mode combinations** |
| Importance from SCRAM's own cut sets | `scram_oracle_suite::every_importance_factor_matches_scram` | **60 basic events x 5 factors** |
| Minimality, independent of SCRAM | `scram_mocus_oracle::generated_cut_sets_are_minimal_by_construction_not_by_luck` | 440 cut sets, 2,584 tree evaluations |
| Order truncation | `scram_mocus_oracle::truncating_by_order_drops_exactly_the_long_cut_sets` | 21 (model, limit) cases |
| **Non-coherent** cut sets vs SCRAM's | `scram_noncoherent::generated_cut_sets_of_a_non_coherent_tree_match_scram` | exact set equality |
| **Non-coherent** conservatism, measured | `scram_noncoherent::our_exact_value_exceeds_scrams_because_cut_sets_are_conservative` | **+23.6 %**, and necessarily above |
| Quantification at scale | `scram_noncoherent::quantifying_4259_cut_sets_matches_scram` | **4,259 cut sets, 108 basic events** |
| **Exact probability, no cut-set ceiling** | `scram_bdd_oracle::exact_probability_matches_scrams_bdd_on_every_model` | **11 of 11 models**, up to 386,261 nodes |
| BDD against inclusion-exclusion | `scram_bdd_oracle::the_bdd_and_inclusion_exclusion_agree_where_both_can_run` | 8 models, to **1e-12** |
| BDD gets what cut sets cannot | `scram_bdd_oracle::the_bdd_gets_the_non_coherent_answer_that_cut_sets_cannot` | `0.5032` exactly |
| **Cut sets at scale**, vs SCRAM **and** vs MOCUS | `scram_bdd_oracle::zbdd_cut_sets_match_scram_and_mocus` | **11 models, 4,702 cut sets** |
| **Prime implicants**, signs included | `scram_prime_implicants::prime_implicants_match_scram` | **10 models, 443 implicants** |
| Prime implicants == cut sets when coherent | `scram_prime_implicants::on_a_coherent_tree_prime_implicants_are_the_minimal_cut_sets` | 9 models, no complement |
| **Importance from the BDD**, both fixtures | `scram_bdd_oracle::bdd_importance_factors_match_scram_including_the_non_coherent_models` | **185 events, 11 models** |
| **Expressions** vs SCRAM's computed probabilities | `scram_expressions::hipps_basic_event_probabilities_match_scrams_computed_values` | **9 of 9 `HIPPS` events** |
| **Models read from their own XML**, probabilities evaluated | `scram_mef::probabilities_evaluated_from_the_xml_match_scrams` | **94 probabilities, 14 records** |
| Cut sets from the XML vs SCRAM's products | `scram_mef::cut_sets_generated_from_the_xml_match_scrams` | **14 records, exact set equality** |
| Totals from the XML, all three modes | `scram_mef::totals_from_the_xml_match_scrams` | **41 model/mode combinations** |
| Importance from the XML | `scram_mef::importance_from_the_xml_matches_scrams` | **59 basic events x 5 factors** |
| **`ThreeMotor`, unlocked** | `scram_mef::three_motor_is_unlocked_by_the_private_namespace_rule` | 18 gates, **12 products**, 4 structural assertions |
| `<xi:include>` and cross-tree references | `scram_mef::xi_include_and_cross_fault_tree_references_resolve` | `TransTest`, `ThreeLevels` |
| The five constructs upstream never uses | `scram_mef::the_mef_only_constructs_match_scram` | **9 products, 3 totals** |
| Refusals, each naming its cause | `scram_mef::the_unported_and_the_malformed_are_refused_not_skipped` | **10 refusals** |
| **All seven random deviates** vs SCRAM's computed values | `scram_deviates::every_deviate_value_matches_scrams` | **7 of 7** |
| The two upstream log-normal models, end to end | `scram_deviates::the_two_upstream_lognormal_models_are_no_longer_refused` | `SmallTree` + `BSCU`, **12 events** |
| Deviate means against their closed forms | `scram_deviates::each_deviate_value_matches_its_closed_form` | 7, to `1e-12` |
| Upstream's domain checks | `scram_deviates::the_domain_checks_are_upstreams` | **12 refusals**, in upstream's order |
| `interval()` and the domain half of validation | `scram_deviates::intervals_are_upstreams_and_are_what_makes_the_domain_checks_bite` | 7 cases |
| **CCF events** vs SCRAM's `--ccf` run | `scram_ccf::the_generated_ccf_events_match_scrams` | **30 events, 2 models** |
| The CCF-rewritten tree | `scram_ccf::the_rewritten_tree_matches_scrams` | **32 products**, 3 totals, importance |
| CCF applied by default, ablation checked | `scram_ccf::ccf_is_applied_by_default_and_the_ablation_is_the_independent_analysis` | `0.0622587` vs `0.0361`, **+72 %** |
| CCF factor bookkeeping and its refusals | `scram_ccf::factor_levels_are_upstreams_and_the_malformed_are_refused` | 6 refusals |
| **Substitutions**: products after substitution | `scram_substitutions::products_after_substitution_match_scrams` | **2 models**, exact set equality |
| Declarative substitution totals | `scram_substitutions::the_declarative_model_totals_match_scrams` | exact `0.329175` by BDD |
| **The MIF sign defect, diagnosed** | `scram_substitutions::the_declarative_model_importance_is_upstreams_magnitudes_with_the_signs_flipped` | 6 of 6 inverted, hand-checked |
| Upstream refuses an exact non-declarative analysis | `scram_substitutions::the_non_declarative_model_has_no_exact_total` | asserted in the fixture |
| Inferred vs declared substitution type | `scram_substitutions::the_inferred_substitution_type_matches_the_declared_one` | 4 agree, 1 untyped |
| Substitution validation | `scram_substitutions::the_malformed_are_refused_with_upstreams_reasons` | 6 refusals |

The whole SCRAM suite — all 15 tests across three files plus the module's own
unit tests — runs in about 1.5 s in release mode.

### Cut-set generation — 440 of 440, no extra and none missing

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
| house_events_small | 2 | 1 |

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

### Top-event probability — 29 of 29 combinations agree

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

### Importance factors — 60 from SCRAM's cut sets, 49 end to end

Every event of every model, all five measures, plus **exact** agreement on the
integer occurrence counts.

Upstream obtains the Birnbaum factor by differentiating its BDD
(`CalculateMif` walking ite vertices); this port obtains it from the definition
`P(top | event) - P(top | not event)` evaluated on the cut sets. The other four
factors are all derived from MIF, so an error in it would move every column at
once — and none moved.

**49 of those events are also ranked from cut sets generated here**, closing
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
all 440 are also checked against the definition directly, with an evaluator
written in the test file rather than taken from the library:

1. no cut set properly contains another;
2. no cut set appears twice;
3. every cut set, set true with all other events false, really does make the
   tree evaluate true (440 evaluations);
4. removing any single member makes it evaluate false (2,144 evaluations).

All four hold for all 440. Check 1 is `O(n^2)` in the cut-set count, so
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
flag is the **true function's**, from its BDD. ~~This port has no BDD, and
its~~ **CORRECTED 2026-09-22** — this port now has one
([`scram::bdd`](../src/scram/bdd.rs), verified in `tests/scram_bdd_oracle.rs`),
and `tests/scram_mef.rs` uses it for exactly this case. The statement that
still holds is the one the table below rests on: `Approximation::Exact` is
inclusion-exclusion over the *minimal cut sets* — which describe a strictly
larger function, because deleting the negative literals throws away the
requirement that a component be working.

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

### Exact probability by BDD — 11 of 11 models, no ceiling

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
| house_events_small | coherent | 0.370000000 | 0.370000000 | 4 | 2 |
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

### Cut sets at scale by ZBDD — 4,700 of them, including the ones MOCUS cannot get

`scram::mocus` is the classical top-down expansion and gives up on a real
model: `Aralia/das9601` exhausts its five-million-state ceiling at every order
limit tried. `scram::zbdd` takes the same answer off the BDD instead —
`ConvertBdd`, `Minimize` and `Subsume`, all three ported — where the work is
proportional to the diagram rather than to the number of intermediate sets.

| model | ZBDD | SCRAM | MOCUS |
|---|---|---|---|
| TwoTrain/two_train | 4 | 4 | agrees |
| Theatre/theatre | 2 | 2 | agrees |
| SmallTree/SmallTree | 2 | 2 | agrees |
| BSCU/BSCU | 10 | 10 | agrees |
| Lift/lift | 12 | 12 | agrees |
| HIPPS/HIPPS | 9 | 9 | agrees |
| ne574/ne574 | 7 | 7 | agrees |
| Aralia/chinese | 392 | 392 | agrees |
| noncoherent_small | 3 | 3 | agrees |
| **Aralia/das9601** | **4,259** | **4,259** | **cannot reach it** |

Two comparisons at once, which is the point: against SCRAM's own products, and
against this crate's own unrelated top-down generator wherever that one can
run. The ZBDD is also asked to **count** the family without materialising it,
and the count must equal the listing — a cheap check that the diagram is
reduced rather than merely enumerable.

The whole file runs in **0.9 s**. MOCUS is not attempted above a thousand
products, since demonstrating its failure costs ~29 s and has its own
reproduction test.

### Prime implicants — the exact answer where cut sets are conservative

A minimal cut set says which components failing is enough. A **prime
implicant** also says which must be *working*, so it carries complemented
literals and describes the function exactly. `zbdd::prime_implicants` ports
`Zbdd::ConvertBddPrimeImplicants` together with `Bdd::Consensus` — the
classical recursion in which an implicant of `f` either contains `x`, or
contains its complement, or is an implicant of the consensus `f_x AND
f_not-x`.

Checked against `scram --probability --importance --prime-implicants`:
**10 models, 443 implicants, compared as signed (positive, negative) name-set
pairs** so a sign error cannot hide. `models-for-this-port/noncoherent_small`
is the one carrying signs — `{+a,-b}`, `{+c,-d}`, `{-c,+d}` — and the other
eight are coherent, which gives the second check:

**On a coherent tree the prime implicants must be exactly the minimal cut
sets**, with no complemented literal anywhere, because a component working can
never help cause failure. That invariant holds on all 9, across 440
implicants, and it pins the consensus recursion against the plain conversion —
two code paths, same answer.

#### They are also measurably tighter

On `noncoherent_small`, whose true probability is `0.5032`:

| | sum of products |
|---|---|
| prime implicants `{+a,-b} {+c,-d} {-c,+d}` | 0.08 + 0.18 + 0.28 = **0.54** |
| minimal cut sets `{a} {c} {d}` | 0.1 + 0.3 + 0.4 = **0.80** |

Both bound the truth from above, as a rare-event sum always does, but the
implicants bound it far more tightly — which is the practical reason to want
them. Each implicant's own probability is checked against the value SCRAM
printed on the corresponding `<product>`.

#### The cost is the algorithm's, not this port's

`Aralia/das9601` is absent from the prime-implicant fixture because **upstream
does not finish it either**: `scram --prime-implicants` exceeds five minutes
on a model whose minimal cut sets it produces in about a second. The consensus
term adds a third recursive call at every node. Recorded so the absence is not
read as a limitation of this port.

### House events, and a defect they exposed

A **house event** is a condition fixed for the analysis rather than sampled —
a valve lined up or not, a timer reset or not. It has no probability and never
appears in a cut set; it decides which parts of the tree are live.
`Arg::Constant` carries it, and the BDD needs no special case at all because a
constant *is* a terminal.

Upstream's only house-event model is `ThreeMotor`, which also uses
`<define-component role="private">` — private namespaces in which an inner
gate `E1` is really `t.E1` and distinct from the outer `E1`. The structure
extractor has no notion of namespaces and was **silently merging them**; that
is now a refusal, and it is the real reason `ThreeMotor` stays excluded (the
earlier "house events, four candidate top gates" was a symptom). A small model
was written to give the feature genuine oracle coverage —
`models-for-this-port/house_events_small.xml`, exercising both values in both
connectives — and SCRAM confirms `{a}`, `{c}`, `p = 0.37`.

#### The defect: an unconditional top reported as conditional

Writing the house-event unit test caught something the oracle models never
would have. `OR(A, true-house-event)` makes the top event **unconditional**,
so its only minimal cut set is the *empty* set. `CutSet` cannot represent
that, and `minimize` was silently dropping it — returning `{A}`, which says
the system fails only when `A` does when in fact it always fails.

Upstream was consulted rather than guessed at:

```
scram --probability  ->  warning="The set is UNITY/Base."
                         basic-events="0"  products="1"  probability="1"
                         <product order="1" probability="1"/>   (no members)
```

Both `mocus::minimal_cut_sets` and `zbdd::minimal_cut_sets` now **refuse**
this, with a message naming upstream's warning.

**Two quite different trees land there**, and the message says so. One is
genuinely unconditional. The other is non-coherent with every implicant
complemented — `NOT A` — where the *function* is not unconditional at all but
its cut-set representation is, because deleting the complements loses
everything. Upstream agrees on both, and the contrast is sharp:

| `NOT A` | SCRAM | this port |
|---|---|---|
| minimal cut sets | `UNITY/Base` warning, empty product | refused, citing that warning |
| prime implicants | `{-A}`, probability 0.9 | `{-A}` |
| probability | 0.9 | 0.9 (BDD) |

So the refusal is not a limitation but a correct statement that the *question*
has no answer in that representation — and both routes that do answer it are
asserted in the same test.

### Importance from the BDD — and a sign divergence in upstream

Every other importance check here goes through minimal cut sets. On a
**non-coherent** tree those are conservative, so every factor derived from
them inherits the conservatism and does **not** match SCRAM, which computes
importance from its BDD. That was unverified until now, because the earlier
importance tests read only the coherent fixture.

`importance::importance_factors_from_bdd` closes it. On
`noncoherent_small`, event `a`:

| | Birnbaum factor |
|---|---|
| SCRAM (from its BDD) | **0.432000** |
| this port, from the BDD | **0.432000** |
| this port, from cut sets | 0.420000 |

**185 basic events across 11 models**, 111 on non-coherent trees. 77 agree on
all five factors. The other 108 agree in magnitude and differ in sign.

#### The 108 are all of `Aralia/das9601`, and all of it

Every one of that model's 108 events diverges; not one event of the other ten
does. Magnitudes agree to `5e-6` throughout, and the divergence runs **both
ways** — SCRAM reports negative factors too — so it is not an absolute value
on either side. It is a single **global sign inversion on one model**.

The sign this port reports is the Birnbaum factor's definition, checked
directly rather than argued. On `e18`:

```text
P(top | e18)     = 0.000014035880
P(top | not e18) = 0.004277032857
difference       = -0.004262996977      SCRAM reports +0.004263
```

`e18` occurring makes the top event about 300 times *less* likely, which a
non-coherent tree permits. The diagram is not in doubt: its top-event
probability `0.004234402887` matches SCRAM's `0.0042344`, and
`0.01 x 1.4036e-5 + 0.99 x 4.27703e-3` reproduces it.

**A candidate explanation, offered as that and not as a conclusion.**
`ProbabilityAnalyzer<Bdd>::CalculateTotalProbability` ends with
`if (bdd_graph_->root().complement) prob = 1 - prob;`.
`ImportanceAnalyzer<Bdd>::CalculateMif` reads the same
`bdd_graph_->root().vertex` and applies no corresponding negation, though its
recursion is signed throughout (`ite.factor(high - low)`, and `mif = -mif` for
a complemented module). A complemented root would flip every factor of that
model and nothing else — exactly the pattern measured. This port uses explicit
terminals and has no root complement to forget.

~~The alternative is that upstream intends a criticality convention rather
than the signed difference … this has **not been established either way**.~~
**UPGRADED 2026-09-22 — a second model and a hand calculation.**
`TwoTrain/substitutions` shows the same inversion: **six** basic events, every
one at ratio exactly `-1.000000`, with the top-event probability agreeing to
the last digit so the diagram is not in question. At six events the Birnbaum
factor can be computed by hand, and two were, in opposite directions:

```text
MIF(PumpTwo)  = P(top | P2) - P(top | ~P2) = 0.40050 - 0.16275 = +0.23775
                                                        SCRAM:   -0.23775
MIF(ValveOne) = P(top | V1) - P(top | ~V1) = 0.25200 - 0.40635 = -0.15435
                                                        SCRAM:   +0.15435
```

Both are the definition `P(top | e) - P(top | not e)`, both land on this
port's sign, and one is positive while the other is negative — so this is not
a convention difference, which would move both the same way. **The defect is
upstream's.** The derivations are in
`scram_substitutions::the_declarative_model_importance_is_upstreams_magnitudes_with_the_signs_flipped`,
which asserts the inversion rather than papering over it.

Still **not** established: that upstream's BDD root is in fact complemented on
these two models. Showing that needs SCRAM instrumented, not read. What is
established is the pattern a root complement predicts — magnitudes untouched,
every sign flipped, the total probability unaffected because
`CalculateTotalProbability` *does* apply the negation — now observed on two
unrelated models and on no other. Not raised upstream: `rakhimov/scram`'s last
commit is from 2019 and filing against a third-party project was not part of
this task.

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

### Reading upstream's own models — the transcription removed

Until 2026-09-22 the *question* reached the tests through `extract_models.sh`,
a shell transcription of a flat subset of the Model Exchange Format. It said
what it could not read, and one of its three `CANNOT-PARSE` records was
`ThreeMotor/three_motor` — a model with a full oracle, an upstream test of its
own (`RiskAnalysisTest.ThreeMotor`), and no analysis route in this crate.

`scram::mef` reads the XML itself, so `tests/scram_mef.rs` goes from
upstream's own `.xml` to the answers with nothing hand-written in between.
The models are committed verbatim under
`reference-data/scram/upstream-input/`.

**Basic-event probabilities moved sides.** Every other test in this crate
takes them from the oracle; this one *evaluates the models' own expressions*
and checks the result against what SCRAM printed. All **94** agree to `5e-6`
relative across **14** of the 16 fixture records.

#### Name resolution is upstream's, rule for rule

`Initializer::GetEntity` consults a **path table** keyed by
`base_path + "." + name` and the model's **id table** keyed by `Id::id()` — a
bare name for a public element, the full path for a private one — and tries
the local path first. `mef::Index` reproduces both tables and that order,
including two details that are easy to get wrong and that
`three_motor_is_unlocked_by_the_private_namespace_rule` asserts separately:

* a `<define-fault-tree>` **is** a path component (`DefineFaultTree` passes
  the tree's own name as `base_path`), even though its public members keep
  their bare names;
* a `<define-component>`'s role **inherits** its container's — `GetRole(s,
  parent_role)`, not a default of private.

On `ThreeMotor` this gives 18 gates: 11 public, and 7 whose ids are
`ThreeMotor.t.E1` … `ThreeMotor.t.E7`. `E4`'s `<gate name="t.E1"/>` reaches
the private gate by local path; the private `E1`'s
`<basic-event name="K1"/>` resolves **outward** to the public `K1`, since the
component declares none. SCRAM's 12 products follow exactly.

#### Five constructs no upstream input uses

A grep over all of upstream's `input/` finds no `<iff>`, `<imply>`,
`<cardinality>`, `<constant>` formula argument, or `<event type="…">`. None of
them could therefore be checked against a model SCRAM ships.
`models-for-this-port/mef_features.xml` uses all five, and SCRAM supplies the
answers in `oracle-mef.txt`.

The three connectives are the load-bearing ones: upstream calls them "rarely
used connectives specific to the MEF", they have **no PDAG representation at
all**, and `Pdag::ConstructComplexGate` rewrites each into the eight analysis
connectives — `iff` as the complement of an `xor`, `imply` as
`or(not a, b)`, `cardinality` as `and` of two `atleast` gates with upstream's
`well_form` collapse applied to each. `mef::Lowering` performs the identical
rewrite, and a wrong one changes the products. All 9 of SCRAM's products are
reproduced, with all three totals.

The same model carries a second, independent instance of the private-namespace
rule: a parameter `rate` declared **both** publicly (`1.0e-5`) and inside the
private component (`4.0e-5`), consumed by an `<exponential>` on each side. A
resolution mistake shows up as a wrong *probability* rather than a read
failure — SCRAM prints `MefFeatures.sub.p = 0.295594` and `shared =
0.0838727`, and both are reproduced.

#### Formulas do not nest, and an earlier claim here was wrong

An earlier revision of `mef.rs` carried machinery for arbitrarily nested
formulas and its module doc listed "nested formulas" among the things the
shell extractor could not reach. **That was wrong about the format.**
Upstream's grammar (`share/input.rng`, `define name="formula"`) admits a
connective whose arguments are each an `argument`, and an `argument` is an
event reference, `<not>` around one event reference, or a `<constant>` —
never another connective. SCRAM itself rejects a nested formula with
`ValidityError: Did not expect element and there`, which is how this was
found. The claim is corrected in the module doc, and
`the_unported_and_the_malformed_are_refused_not_skipped` pins the refusal.

What *does* exist is the `<not>` wrapper, which upstream records as a
complement flag on the argument and this port lowers into an anonymous
negating gate — verified on `Aralia/das9601` and `noncoherent_small`, the
only models that use it.

#### What it refuses, and why that matters more than what it reads

An element `mef.rs` does not recognise is an **error**, never a skip. A
skipped `<define-CCF-group>` would leave a model that still parses, still
analyses, and answers a different question — which is the failure this whole
port exists to avoid. Ten refusals are asserted, each naming its cause: CCF
groups, substitutions, event trees, alignments, extern functions, an unknown
element, a typed reference naming the wrong kind of declaration, a private
name reached from outside, an invalid `role`, and a nested formula.

Two of upstream's models cannot be read yet — `SmallTree/SmallTree` and
`BSCU/BSCU`, both using `<lognormal-deviate>`. The random deviates belong to
uncertainty analysis, a later chunk. That refusal is **asserted**
(`the_two_unreadable_models_fail_for_the_stated_reason`) rather than recorded
in prose, so it cannot quietly become a different refusal while the doc keeps
claiming this one. Both models remain covered by the tests that take their
probabilities from the oracle.

### The seven random deviates — and the two models they unlocked

`SmallTree/SmallTree` and `BSCU/BSCU` were, until 2026-09-22, the only two
records in the whole fixture that `scram::mef` refused: both define their
failure rates with `<lognormal-deviate>`. That refusal was **asserted** rather
than written down, so it could not drift — and it is now gone. Every record in
every fixture reads.

Of the seven deviates, upstream's own input suite uses two:
`<lognormal-deviate>` in its three-argument form and `<normal-deviate>`. The
uniform, two-argument log-normal, gamma, beta and histogram forms appear in
**no** upstream model, so `models-for-this-port/deviates.xml` uses all seven
and SCRAM supplies the answers in `oracle-deviates.txt`.

| event | deviate | RAFFLES | SCRAM |
|---|---|---|---|
| `u` | uniform(0.1, 0.3) | 0.200000 | 0.2 |
| `n` | normal(0.5, 0.01) | 0.500000 | 0.5 |
| `l3` | lognormal(2e-5, EF 3, 95 %) as a rate | 0.160711 | 0.160711 |
| `l2` | lognormal(mu −12, sigma 0.5) as a rate | 0.0591672 | 0.0591672 |
| `g` | gamma(k 2, theta 1e-6) as a rate | 0.0173674 | 0.0173674 |
| `b` | beta(1, 999) as a rate | 0.999843 | 0.999843 |
| `hist` | histogram over three bins as a rate | 0.0147817 | 0.0147817 |

Three of them sit inside an `<exponential>` rather than standing as a
probability, and that is upstream's doing rather than presentation:
`EnsureProbability` checks an expression's whole `interval()` against `[0, 1]`,
and the gamma, beta and histogram domains reach past 1.

#### One substitution, checked rather than assumed

Upstream gets the log-normal's scale parameter from Boost:

```cpp
double z = -std::sqrt(2) * boost::math::erfc_inv(2 * level_.value());
return std::log(ef_.value()) / z;
```

This port has no `erfc_inv` and uses `distributions`' standard normal quantile
on the identity `-sqrt(2) erfc^-1(2p) = Phi^-1(p)`. That is the kind of
substitution that is right in the algebra and wrong by a factor of two in the
code, so it is checked against the definition it claims to satisfy rather than
assumed: `Phi(Phi^-1(p))` returns `p` to better than `1e-12` at the five
levels MEF models use, `z_0.95` lands on the table value `1.6448536269514722`,
and the end-to-end consequence — `SmallTree`'s `e1` — lands on SCRAM's
`0.160711`.

#### `interval()`, and why it is here rather than with the uncertainty analysis

Upstream validates an argument **twice**: `EnsureNonNegative` and
`EnsureWithin` compare the value against the range, and then the whole
`interval()` against it. Without deviates the two are the same check, because
upstream's default interval is the degenerate `[value, value]`. With them they
differ, and the difference is the point: a `normal(0.5, 0.1)` has a mean that
is a perfectly good probability and a domain of `[-0.1, 1.1]` that is neither
a probability nor non-negative, and upstream rejects it. So `Interval` and the
whole `interval()` lattice are ported with the deviates, not deferred.

Two of the lattice's entries are ported **as upstream wrote them, and they
look wrong**. `GammaDeviate::interval` computes

```cpp
theta * pow(gamma_q(k, gamma_q(k, 0) - 0.99), -1)
```

and `gamma_q(k, 0)` is 1 for every `k`, so the inner argument is the constant
`0.01` and the result is `theta / Q(k, 0.01)` rather than the "99 percentile"
the comment claims; `BetaDeviate::interval` has the same shape. Reproducing
upstream's behaviour is the specification, so they are reproduced, with the
reading recorded here. It affects a validation bound only, never a reported
probability — which is why it is a note rather than a divergence.

#### What is deliberately not here

`Expression::Sample()` and each deviate's `DoSample()`. They have exactly one
consumer upstream — the uncertainty analysis — and exactly one oracle,
`scram --uncertainty`'s reported mean, sigma and quantiles. Writing them now
would mean writing them with nothing to check them against, which is the
failure this whole record exists to avoid. `Expression::is_deviate()` — the
hook that analysis uses to decide whether a model needs sampling at all — is
ported now, because it *is* checkable now.

### Common-cause failure groups — and one default this port changed on purpose

A fault tree that treats two pumps as independent understates the risk. A CCF
group says they are coupled, and applying it **rewrites the tree** rather than
adjusting a number: each member becomes a proxy gate over the shared-failure
events it belongs to, so the shared failure shows up as a *single* event in the
cut sets.

On `TwoTrain/common_cause`, upstream's own model:

| | products | orders | probability |
|---|---|---|---|
| CCF applied | 6 | 2 of order 1, 4 of order 2 | **0.0622587** |
| CCF ablated | 4 | all order 2 | **0.0361** |

Both match the corresponding SCRAM run exactly. The gap is **+72 %**.

#### This port applies CCF groups by default; upstream needs `--ccf`

That is a deliberate divergence, and the workspace rule it follows is the one
about physics the data supplies being applied unless a caller explicitly
ablates it. A model that declares a CCF group has already said its components
are coupled; a run that quietly ignores that reports a risk the model itself
says is wrong, and — as the table shows — by a wide margin.
`MefModel::without_ccf` is the visible ablation, it reproduces SCRAM's
non-`--ccf` answer, and the default is **asserted** so it cannot drift back
off, which is the other half of that rule.

#### Three of the four models had no upstream model to check against

Upstream's whole input suite uses `beta-factor` and nothing else.
`models-for-this-port/ccf_models.xml` uses all four, at group size 3 for the
three multi-level models so that the `1/C(n-1, i)` combination reciprocal is
exercised at a value other than 1.

| group | model | level | RAFFLES | SCRAM |
|---|---|---|---|---|
| `Valves` | beta-factor | 1 | 0.08 | 0.08 |
| `Valves` | beta-factor | 2 | 0.02 | 0.02 |
| `Mgl` | MGL | 1 | 0.045 | 0.045 |
| `Mgl` | MGL | 2 | 0.0015 | 0.0015 |
| `Mgl` | MGL | 3 | 0.002 | 0.002 |
| `Alpha` | alpha-factor | 1 | 0.039823 | 0.039823 |
| `Alpha` | alpha-factor | 2 | 0.00309735 | 0.00309735 |
| `Alpha` | alpha-factor | 3 | 0.0039823 | 0.0039823 |
| `Phi` | phi-factor | 1 | 0.04 | 0.04 |
| `Phi` | phi-factor | 2 | 0.0075 | 0.0075 |
| `Phi` | phi-factor | 3 | 0.0025 | 0.0025 |

#### A `<members>` element DECLARES its basic events

This is the detail that decides whether `TwoTrain/common_cause` loads at all.
Upstream's `ProcessCcfMembers` **constructs** a `BasicEvent` per member, with
the group's own base path and role, and registers it; the model declares
`ValveOne` nowhere else. Reading the members as *references* — which is what
they look like — makes the model fail with "undefined event `ValveOne`", which
is exactly how this was found.

#### A refusal established by running the binary, not by reading the asserts

An alpha-factor group with a single factor is malformed. Upstream's source
says so only through `assert(probabilities.size() > 1)`, which a release build
does not check — so the honest way to find out what SCRAM *does* was to run it
on that input. It refuses with **"Expression requires 2 or more arguments"**:
with one factor the model's weighted sum is an `Add` of a single argument, and
`detail::EnsureMultivariateArgs` throws from the `NaryExpression<T, -1>`
constructor. That check was missing from this port's `Expression::validate`
and is now there — along with the recursion it also lacked, which had meant a
malformed argument three levels down passed unexamined.

### Substitutions — and the finding that fell out of them

Upstream ships two models for substitutions and both are checked, end to end,
from their own XML.

| model | kind | products |
|---|---|---|
| `TwoTrain/substitutions` | declarative | 3 |
| `TwoTrain/nondeclarative_substitutions` | non-declarative | 4 |

The two kinds live in different layers upstream and do here too. A
**declarative** substitution is a logical implication conjoined with the root
(`Pdag::ConstructSubstitution`), which is a tree rewrite and makes the tree
non-coherent. A **non-declarative** one is a pass over the generated products
(`Zbdd::ApplySubstitutions`): for every product containing the whole
hypothesis, remove the source events, add the target, re-minimise. The second
model shows what that does — the unsubstituted tree gives
`{V1,V2} {V1,P2} {P1,V2} {P1,P2}`, the exchange rule turns `{V1,V2}` into
`{V1,V3}`, and the recovery rule collapses `{P1,P2}` to a single
**order-1** product `{HotBackupPump}`.

**Upstream refuses an exact analysis of a non-declarative model** — "Non-
declarative substitutions do not apply to exact analyses" — because the
rewritten product list is no longer the minimal cut sets of any Boolean
function. The fixture therefore carries no exact total for that model, and
that absence is asserted rather than left as a gap in a table.

**The free oracle.** MEF's `type` attribute on a substitution is optional and
purely descriptive — nothing in the analysis branches on it — while
`Substitution::type()` *infers* the type from the shape. Where a model
declares one, the two must agree, and they arrive by completely different
routes. Four of the five declared types are reproduced; the fifth declares
none, and the inference returns none for it too.

**And the finding.** Getting importance factors for the declarative model
meant going through the BDD, since the tree is non-coherent — and every one of
its six factors came out with SCRAM's magnitude and the opposite sign. That is
the same pattern `Aralia/das9601` shows on 108 events, which this record had
recorded as a *measurement with a candidate explanation and no conclusion*.
Six events can be checked by hand; 108 cannot. See
[Importance from the BDD](#importance-from-the-bdd--and-a-sign-divergence-in-upstream)
above, which is upgraded accordingly.

## What this does NOT establish

- **It is not validation.** Agreement with SCRAM shows this port reproduces
  SCRAM's answers. It says nothing about whether fault-tree quantification is
  the right model for any particular system, and nothing about whether any
  particular tree describes a real one.
- ~~**Upstream's own tests never ran**~~ **CORRECTED 2026-09-21** — they do
  now: 540 assertions in 71 test cases, all passing, including a test per
  benchmark model. See above.
- **Eleven models, 4,702 cut sets.** The coherent fixture holds 85 basic
  events and 452 products, the non-coherent one 111 and 4,262. Real PRA models
  reach tens of thousands of cut sets, and the largest Aralia benchmarks in
  upstream's own suite already reach 75,379.
  **Those are deliberately left out — maintainer direction, 2026-09-21** —
  having exhausted memory during a first extraction attempt. This is a closed
  decision, not an open gap: do not reopen it by raising the fixture's
  600-product cap or by chasing the memory ceiling.
  `Approximation::Exact` cannot scale in principle (it is `2^n`, capped at 20
  cut sets), and `scram::mocus` is exponential in the worst case, which is
  exactly what upstream's ZBDD exists to avoid.
- ~~**`ThreeMotor/three_motor` is the one model no route covers
  structurally.** … resolving those namespaces means reimplementing MEF name
  resolution, which is input parsing and outside this crate's scope~~
  **CORRECTED 2026-09-22** — MEF name resolution was reimplemented, rule for
  rule from `Initializer::GetEntity`, and `ThreeMotor` is now covered end to
  end: 18 gates read, 12 products reproduced, with the two halves of the
  private-namespace rule asserted separately. The *reason* the old entry gave
  still stands as a warning — getting it subtly wrong builds the wrong tree —
  which is why the test checks the resolution itself and not only the answers.
- **`Approximation::Exact` is still capped at 20 cut sets**, and that is
  inherent — it is `2^n`. It is no longer the only exact route, so the cap is
  a property of that function rather than of the crate.
- **A non-coherent tree's minimal cut sets are conservative, by definition.**
  That is not a gap — `zbdd::prime_implicants` and `bdd::Bdd::probability`
  both give the exact answer — but a caller who asks for cut sets on such a
  tree and quantifies them gets an upper bound, and nothing in the types says
  so.
- **Prime implicants are verified on small and medium models only**: 10 of
  them, 443 implicants, none larger than `Aralia/chinese`'s 392. Upstream
  itself cannot produce them for `das9601`, so there is no oracle at that
  scale to compare against.
- **`scram::mocus` does not scale to a real PRA model**, and is kept as a
  second opinion rather than as the working path. `scram::zbdd` is what to use
  above a few hundred cut sets.
- **The BDD's variable ordering is not optimised.** First appearance in a
  depth-first walk, where upstream spends a preprocessor on the problem.
  `das9601` needs 386,261 nodes under it; a model that blew up would need that
  work, and `Bdd::node_count` is how it would show.
- **Order truncation is checked to order 6 and no further.** `Aralia/chinese`
  gives limits 1-5 that each cut inside the distribution of cut-set orders,
  which is where an over-eager prune would show; the other nine models bottom
  out at order 3. A real PRA truncation at order 8-10 on a model with hundreds
  of thousands of products is still untested.
- ~~**`Connective::Null` has no oracle coverage in the committed fixture.** …
  `ThreeLevels/top` … was excluded when the multi-result guard was added, and
  the evidence is no longer regenerable.~~ **CORRECTED 2026-09-22** — it is
  regenerable: `extract_multi_tree_oracle.sh` emits one record per
  `<sum-of-products>`, which is what the multi-result guard was refusing to
  do, and `oracle-multi-tree.txt` carries `ThreeLevels/top`'s three trees and
  `TransTest/trans_one`'s two. `scram_mef` checks all five, so
  `Connective::Null` has oracle coverage again.
- ~~**Two of upstream's models still cannot be read from XML** —
  `SmallTree/SmallTree` and `BSCU/BSCU`, both using `<lognormal-deviate>`.~~
  **CORRECTED 2026-09-22** — the seven random deviates landed and both models
  read. Every record in every fixture now reads; there is no allowance left in
  the tests for one that does not.
- **Nothing here is sampled.** Every number in this record is a *deterministic*
  quantity: upstream's `value()` for each expression and the analysis built on
  it. `Expression::Sample()` is not implemented, so the uncertainty analysis —
  mean, sigma, quantiles over a Monte Carlo of the deviates — is not merely
  unverified but absent. A deviate's mean is not its distribution, and nothing
  here says anything about the spread.
- **`interval()` is ported but only lightly exercised.** Seven cases, all in
  `scram_deviates`. Upstream uses it inside `EnsureWithin`/`EnsureNonNegative`
  on every argument of every expression; this port's `validate` checks the
  values and the deviates' own domains, and does not yet run the domain check
  over every operator argument the way upstream does.
- **The trigonometric operators and `<switch>` are not ported.** They are in
  the MEF grammar and in no upstream input model, so there would be no oracle
  for them; `scram::mef` refuses them by name.
- **`scram::mef` does not validate against the RelaxNG schema.** Upstream runs
  `share/input.rng` through libxml2 before looking at anything; this checks
  only what it needs to build the model, which is narrower. A document SCRAM
  would reject may be read here. Adding a RelaxNG validator in pure Rust is
  not in prospect; what stands in for it is that every construct the reader
  does not understand is an error rather than a skip.
- ~~**CCF groups**, substitutions, event trees and alignments are refused, not
  read.~~ **CORRECTED 2026-09-22** — CCF groups and substitutions are read and
  applied; event trees and alignments are still refused, each refusal
  asserted. A model using one
  cannot be silently mis-read as a smaller model, but it also cannot be
  analysed.
- **CCF groups are verified on two models and 30 events.** Both are small —
  group sizes 2 and 3 — and the whole check rests on one upstream model plus
  one written here. A real PRA uses groups of 4 to 8, where the combination
  count grows as `2^n` and the MGL and alpha formulas have many more terms.
  Nothing here says how this behaves there.
- **`<define-extern-function>` is refused deliberately and will stay refused.**
  It loads a shared library named by the input file, which the workspace
  `RESPONSIBLE_USE.md` rule on autonomous access to systems forbids. This is a
  decision, not a gap.
- **Substitutions are verified on upstream's two models and nothing else** —
  three declarative and two non-declarative rules between them, over four
  basic events. Nothing here says how the product pass behaves on a model with
  hundreds of rules, where the order rules fire in could matter.
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
cargo test -p raffles --release --test scram_prime_implicants -- --nocapture
cargo test -p raffles --release --test scram_mocus_oracle -- --nocapture
cargo test -p raffles --release --test scram_oracle_suite -- --nocapture
cargo test -p raffles --release --test scram_cross_code -- --nocapture
cargo test -p raffles --release --lib scram
```

Each test prints its own comparison table, so the numbers in this document can
be re-derived rather than taken on trust.
