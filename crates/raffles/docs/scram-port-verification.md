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
| `scram::probability::top_event_probability` (`Exact`) | — | **no** — inclusion-exclusion where upstream traverses a BDD |
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
library, the BDD and ZBDD algorithms, the preprocessor, complement elimination
(so non-coherent trees are **refused**, not approximated), and upstream's
probability cut-off on products (`Settings::cut_off_`, default `1e-8` — this
port truncates by cut-set order only).

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
| Cut sets generated here vs SCRAM's products | `scram_mocus_oracle::generated_cut_sets_match_the_ones_scram_found` | **7 models, 46 cut sets, exact set equality** |
| Input model to top-event probability, end to end | `scram_mocus_oracle::quantifying_generated_cut_sets_reproduces_scrams_totals` | **21 of 21 model/mode combinations** |
| Top-event probability from SCRAM's own cut sets | `scram_oracle_suite::every_model_total_probability_matches_scram` | **24 of 24 model/mode combinations** |
| Importance factors | `scram_oracle_suite::every_importance_factor_matches_scram` | **58 basic events x 5 factors** |
| Minimality, independent of SCRAM | `scram_mocus_oracle::generated_cut_sets_are_minimal_by_construction_not_by_luck` | 46 cut sets, 120 tree evaluations |
| Order truncation | `scram_mocus_oracle::truncating_by_order_drops_exactly_the_long_cut_sets` | 14 (model, limit) cases |

### Cut-set generation — 46 of 46, no extra and none missing

Compared **as a set of sets**, by basic-event name. A missing cut set and a
spurious one are different defects and both fail.

| model | cut sets, RAFFLES and SCRAM |
|---|---|
| TwoTrain/two_train | 4 |
| Theatre/theatre | 2 |
| SmallTree/SmallTree | 2 |
| BSCU/BSCU | 10 |
| Lift/lift | 12 |
| HIPPS/HIPPS | 9 |
| ne574/ne574 | 7 |

**This is the strongest result in this document.** SCRAM generates these with a
ZBDD over a Boolean graph its preprocessor has rewritten; this port runs the
classical top-down expansion with absorption. The two share no code and no
data structure, and they agree exactly.

`HIPPS` earns its place twice over: it is the only model here with an
`atleast` gate (`min="2"` of three pressure switches), so its nine cut sets are
the only check that the combination expansion is right, and its basic-event
probabilities are ones SCRAM *computes* from `periodic-test` and `GLM`
expressions rather than reading as literals.

### Top-event probability — 24 of 24 combinations agree

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

**The exact column is the strong one.** SCRAM computes it by BDD traversal;
this port by inclusion-exclusion over the cut sets. Two different algorithms
for the same quantity, agreeing to the report's resolution.

Twenty-one of these 24 were then reproduced **end to end** — cut sets generated
here rather than read from the oracle — which is strictly stronger, because a
generation defect that happened to cancel against a quantification defect would
pass the first form and fail the second. The three not repeated are
`ThreeMotor`'s, whose structure the parser refuses.

**TwoTrain's rare-event row exercises the clamp.** The raw sum is
`0.25 + 0.35 + 0.35 + 0.49 = 1.44`, and both report 1 — so upstream's
`return sum > 1 ? 1 : sum;` is tested rather than merely translated. It is also
the approximation failing loudly: these basic events are far too likely for
"rare event" to mean anything. `ne574` clamps for the same reason.

### Importance factors — 58 basic events x 5 factors agree

Every event of every model, all five measures, plus exact agreement on the
integer occurrence counts.

Upstream obtains the Birnbaum factor by differentiating its BDD
(`CalculateMif` walking ite vertices); this port obtains it from the definition
`P(top | event) - P(top | not event)` evaluated on the cut sets. The other four
factors are all derived from MIF, so an error in it would move every column at
once — and none moved.

### Minimality, checked without reference to SCRAM

Agreeing with SCRAM would not by itself prove the cut sets are *minimal* — a
generator that forgot absorption produces a superset-laden list that still
quantifies to roughly the right answer under the rare-event approximation. So
all 46 are also checked against the definition directly, with an evaluator
written in the test file rather than taken from the library:

1. no cut set properly contains another;
2. no cut set appears twice;
3. every cut set, set true with all other events false, really does make the
   tree evaluate true (46 evaluations);
4. removing any single member makes it evaluate false (74 evaluations).

All four hold for all 46.

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

## What this does NOT establish

- **It is not validation.** Agreement with SCRAM shows this port reproduces
  SCRAM's answers. It says nothing about whether fault-tree quantification is
  the right model for any particular system, and nothing about whether any
  particular tree describes a real one.
- **Upstream's own tests never ran** (Catch2/glibc, above), so the oracle is
  only as trustworthy as the CLI path.
- **Eight models, 58 cut sets, 58 basic events.** Real PRA models reach tens of
  thousands of cut sets. Nothing here exercises that scale;
  `Approximation::Exact` cannot in principle (it is `2^n`), and `scram::mocus`
  is exponential in the worst case, which is exactly what upstream's ZBDD
  exists to avoid.
- **Only 46 of those 58 cut sets were generated here.** `ThreeMotor`'s 12 are
  checked against the quantification layer only, because the structure parser
  refuses the model (house events, four candidate top gates).
- **Only coherent trees.** Complement elimination is not ported, so a tree with
  `not`, `nand`, `nor` or `xor` is refused. Nothing here says what this port
  would do with one, because it will not attempt one.
- **Upstream's probability cut-off is not implemented.** SCRAM discards
  products below `Settings::cut_off_` (default `1e-8`); this truncates by
  cut-set order only. On the seven models compared the resulting sets are
  equal, which is measured — but a model where the cut-off bites would diverge.
- **Order truncation is only weakly checked.** The fixture's cut-set orders run
  from 1 to 3, so nothing exercises a limit deep inside a long cut set, which
  is where truncation actually bites on a real model.
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
        input/ne574/ne574.xml"
reference-data/scram/extract_oracle.sh ./bin/scram $MODELS \
    > reference-data/scram/oracle.txt
reference-data/scram/extract_models.sh $MODELS \
    > reference-data/scram/models.txt

# Check the port against them.
cargo test -p raffles --release --test scram_mocus_oracle -- --nocapture
cargo test -p raffles --release --test scram_oracle_suite -- --nocapture
cargo test -p raffles --release --test scram_cross_code -- --nocapture
cargo test -p raffles --release --lib scram
```

Each test prints its own comparison table, so the numbers in this document can
be re-derived rather than taken on trust.
