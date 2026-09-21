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

The **fault-tree quantification layer** of [SCRAM](https://github.com/rakhimov/scram):
given the minimal cut sets of a fault tree and the probabilities of the basic
events, compute the top-event probability and rank the events by importance.

| RAFFLES | upstream |
|---|---|
| `scram::probability::cut_set_probability` | `CutSetProbabilityCalculator::Calculate` |
| `scram::probability::top_event_probability` (`RareEvent`) | `RareEventCalculator::Calculate` |
| `scram::probability::top_event_probability` (`Mcub`) | `McubCalculator::Calculate` |
| `scram::probability::top_event_probability` (`Exact`) | *not a port* — inclusion-exclusion where upstream uses a BDD |
| `scram::importance::importance_factors` | `ImportanceAnalyzerBase::Analyze` (the five derived factors) |

**Not ported, and the list matters more than the list above.** Cut-set
*generation* — MOCUS, BDD, ZBDD, the preprocessor and the Boolean graph
(`pdag`) — is the bulk of SCRAM and is absent. So is XML input handling
(explicitly out of RAFFLES' scope), event trees, alignments, CCF groups,
substitutions, and the whole `expression` library. This port starts where the
cut sets already exist.

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

### Extraction

`reference-data/scram/extract_oracle.sh` runs the binary and parses **its own
XML report** — basic-event probabilities from the `<importance>` section, cut
sets from `<product>` elements, totals from three separate runs
(`--rare-event`, `--mcub`, and no flag for the exact BDD path). Nothing is read
out of the input models, so the fixture cannot silently drift from what SCRAM
computed. Output is `reference-data/scram/oracle.txt`.

Models with more than 40 cut sets were skipped: the Aralia benchmarks reach
75,379 products and exhausted memory during a first attempt. Six models were
captured — `TwoTrain`, `Theatre`, `SmallTree`, `ThreeMotor`, `BSCU`, `Lift`.

## Results (2026-09-21)

### Top-event probability — 18 of 18 model/mode combinations agree

Tolerance `5e-6` relative, the resolution of upstream's 6-significant-figure
report.

| model | mode | RAFFLES | SCRAM |
|---|---|---|---|
| TwoTrain | exact | 0.722500000 | 0.722500000 |
| TwoTrain | rare-event | 1.000000000 | 1.000000000 |
| TwoTrain | mcub | 0.838393750 | 0.838394000 |
| Theatre | exact | 0.002070000 | 0.002070000 |
| Theatre | rare-event | 0.002100000 | 0.002100000 |
| Theatre | mcub | 0.002099100 | 0.002099100 |
| SmallTree | exact | 0.026776840 | 0.026776800 |
| SmallTree | rare-event | 0.026958531 | 0.026958500 |
| SmallTree | mcub | 0.026776840 | 0.026776800 |
| ThreeMotor | exact | 0.021153782 | 0.021153800 |
| ThreeMotor | rare-event | 0.021201280 | 0.021201300 |
| ThreeMotor | mcub | 0.021176783 | 0.021176800 |
| BSCU | exact | 0.112408535 | 0.112409000 |
| BSCU | rare-event | 0.135371755 | 0.135372000 |
| BSCU | mcub | 0.128587773 | 0.128588000 |
| Lift | exact | 0.000012000 | 0.000012000 |
| Lift | rare-event | 0.000012000 | 0.000012000 |
| Lift | mcub | 0.000012000 | 0.000012000 |

**The exact column is the strong one.** SCRAM computes it by BDD traversal;
this port computes it by inclusion-exclusion over the cut sets. Two different
algorithms for the same quantity, agreeing to the report's resolution.

**TwoTrain's rare-event row exercises the clamp.** The raw sum is
`0.25 + 0.35 + 0.35 + 0.49 = 1.44`, and both report 1 — so upstream's
`return sum > 1 ? 1 : sum;` is tested rather than merely translated. It is also
the approximation failing loudly: these basic events are far too likely for
"rare event" to mean anything.

### Importance factors — 42 basic events × 5 factors agree

Every event of every model, all five measures, same `5e-6` tolerance, plus
exact agreement on the integer occurrence counts.

**This is the strongest result here.** Upstream obtains the Birnbaum factor by
differentiating its BDD (`CalculateMif` walking ite vertices); this port
obtains it from the definition `P(top | event) - P(top | not event)` evaluated
on the cut sets. The other four factors are all derived from MIF, so an error
in it would move every column at once — and none moved.

### One deliberate divergence: RRW at the singularity

`Theatre/theatre`, `Mains_Fail`: MIF, CIF, DIF and RAW all match SCRAM
exactly. **RRW does not** — upstream reports `0`, this port reports infinity.

The denominator `p_total - p * MIF` vanishes exactly when the event lies in
every cut set: `p = 0.03`, `MIF = 0.069`, `p_total = 0.00207`, and
`0.03 × 0.069 = 0.00207`. Then `P(top | not event) = 0` — removing the event
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

- **It is not validation.** Agreement with SCRAM shows this port implements
  SCRAM's arithmetic. It says nothing about whether fault-tree quantification
  is the right model for any particular system.
- **Upstream's own tests never ran** (Catch2/glibc, above), so the oracle is
  only as trustworthy as the CLI path.
- **Six models, 42 cut sets.** Real PRA models reach tens of thousands of cut
  sets; nothing here exercises that scale, and `Approximation::Exact` cannot
  in principle — it is `2^n`.
- **No cut-set generation is ported**, so this cannot analyse a fault tree
  end-to-end. The caller must supply cut sets from elsewhere.
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

# Regenerate the fixture.
reference-data/scram/extract_oracle.sh ./bin/scram <models...> > oracle.txt

# Check the port against it.
cargo test -p raffles --release --test scram_oracle_suite
cargo test -p raffles --release --test scram_cross_code
```
