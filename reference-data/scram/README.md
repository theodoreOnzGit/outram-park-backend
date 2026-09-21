# SCRAM reference output for RAFFLES' `scram` port

Numerical output produced by **building upstream SCRAM from source and running
it**, used as the code-to-code oracle for `raffles::scram`. Like `../gsl/` and
`../liggghts/`, this lives at the repository root, outside `crates/`, so it is
git-tracked but never part of a published crate tarball.

Nothing in `oracle.txt` was written by hand or derived by reading SCRAM's
source and reasoning about what it would produce.

## Provenance

| | |
|---|---|
| Project | SCRAM — Olzhas Rakhimov |
| Repository | <https://github.com/rakhimov/scram> |
| Commit | `b85b78940de38996eeffec54d946824bd4280a1c` (2019-07-03) |
| Version reported | SCRAM 0.16.2 |
| Licence | GPL-3.0-or-later |
| Built | 2026-09-21, GCC, Boost 1.83.0, libxml2 2.9.14, `-DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF -DWITH_TCMALLOC=OFF -DWITH_JEMALLOC=OFF` |
| Models | upstream's **own** `input/` suite — not inputs invented for this port |

SCRAM is GPL-3.0-or-later and RAFFLES is GPL-3.0-only, so this is a
same-licence port. It is **not** the one-way Apache-2.0 → GPLv3 relicensing
that governs the RAVEN-derived parts of `raffles`.

### One build patch was needed, and it touches no arithmetic

SCRAM 0.16.2 predates Boost 1.83 and fails to compile against it with 42
instances of `'BOOST_THROW_EXCEPTION_CURRENT_FUNCTION' was not declared in
this scope`. That macro was Boost's internal alias for `BOOST_CURRENT_FUNCTION`
and has since been removed; `src/error.h` was changed to use
`BOOST_CURRENT_FUNCTION` directly. The macro expands to a function-name string
attached to thrown exceptions for diagnostics — it cannot affect a computed
value.

### Upstream's own test suite was NOT built, and that is a gap

`tests/` fails to compile: the vendored Catch2 uses `SIGSTKSZ` in a constant
expression, which modern glibc no longer permits. So there is no independent
confirmation that this build of SCRAM behaves as its authors intended — the
oracle rests on the `scram` CLI, which is the same code path but not the same
check.

## Two fixtures, deliberately separate

| file | parsed from | trusted for |
|---|---|---|
| `oracle.txt` | SCRAM's own **XML report** | the answers — cut sets, probabilities, totals, importance |
| `models.txt` | SCRAM's own **input models** | the question — gates, connectives, arguments, the top gate |

`extract_oracle.sh` reads nothing from the inputs, so no answer in it can have
been copied from the question. `extract_models.sh` emits no answer, so nothing
in it can prejudge a comparison. That split is what lets
`crates/raffles/tests/scram_mocus_oracle.rs` generate cut sets in Rust and
compare them against the ones SCRAM found.

Basic-event probabilities come from `oracle.txt`, **not** from the models:
several models (`HIPPS` especially) define them through `periodic-test` and
`GLM` expressions that SCRAM evaluates and `raffles` does not.

## `oracle.txt`

Eight models, 58 basic events, 58 products.

| model | basic events | products |
|---|---|---|
| `TwoTrain/two_train` | 4 | 4 |
| `Theatre/theatre` | 3 | 2 |
| `SmallTree/SmallTree` | 4 | 2 |
| `ThreeMotor/three_motor` | 11 | 12 |
| `BSCU/BSCU` | 8 | 10 |
| `Lift/lift` | 12 | 12 |
| `HIPPS/HIPPS` | 9 | 9 |
| `ne574/ne574` | 7 | 7 |

Model selection is mechanical rather than curated. A model is skipped when:

- **it has more than 40 cut sets** — the Aralia benchmarks reach 75,379
  products and exhausted memory on a first attempt; or
- **its report contains more than one `<sum-of-products>`** — a model defining
  several fault trees produces one result set per tree, and this format has
  nowhere to say which product belongs to which, so merging them would be
  silently wrong. `TransTest/trans_one` (2 trees) and `ThreeLevels/top` (3)
  are excluded by this, and the script says so on stderr.

`MODEL`, `EVENT`, `PRODUCT`, `TOTAL`, `IMPORTANCE`, `END` — one record per
line:

```
MODEL      <suite>/<input basename>
EVENT      <name> <probability>
PRODUCT    <order> <event> [<event> ...]
TOTAL      <exact|rare-event|mcub> <probability>
IMPORTANCE <name> <occurrence> <MIF> <CIF> <DIF> <RAW> <RRW>
END
```

Probabilities and importance factors carry SCRAM's own report precision — six
significant figures — which is why the consuming tests use a `5e-6` relative
tolerance rather than a tighter one.

## `models.txt`

Fault-tree structure for the same eight models plus the two the oracle
excludes, so that a reader can see what was refused and why.

```
MODEL <suite>/<input basename>
GATE  <name> <connective> <min-or-dash> <g:arg|b:arg> ...
TOP   <name>
END
```

A gate the parser cannot read becomes a `CANNOT-PARSE <what> <why>` record
rather than a guess, and any model carrying one is skipped by the tests with
the reason printed. Present refusals:

| model | why |
|---|---|
| `ThreeMotor/three_motor` | house events (`E10`, `E12`), and four candidate top gates |
| `TransTest/trans_one` | assembled by `xi:include`; the structure is only partly in the file |

Only the flat single-connective gate form these models use is handled — one
`<and>`/`<or>`/`<atleast>` per `<define-gate>`, or a bare `<event/>` child
meaning a pass-through (`null`) gate. A nested formula would be silently
mis-parsed, so it is refused instead.

## The generators

Both are committed so the fixtures can be regenerated rather than trusted.

```bash
cmake <scram-src> -DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF \
      -DWITH_TCMALLOC=OFF -DWITH_JEMALLOC=OFF
make -j4
cp <scram-src>/share/*.rng share/scram/     # the CLI needs its RelaxNG schemas

MODELS="input/TwoTrain/two_train.xml input/Theatre/theatre.xml \
        input/SmallTree/SmallTree.xml input/ThreeMotor/three_motor.xml \
        input/BSCU/BSCU.xml input/Lift/lift.xml input/HIPPS/HIPPS.xml \
        input/ne574/ne574.xml"
reference-data/scram/extract_oracle.sh ./bin/scram $MODELS \
    > reference-data/scram/oracle.txt
reference-data/scram/extract_models.sh $MODELS \
    > reference-data/scram/models.txt
```

## Consumed by

- `crates/raffles/tests/scram_mocus_oracle.rs` — end to end: build the tree
  from `models.txt`, generate cut sets with `raffles::scram::mocus`, compare
  them against `oracle.txt`'s products, then quantify and compare the totals.
- `crates/raffles/tests/scram_oracle_suite.rs` — every model, every mode,
  every basic event, starting from SCRAM's own cut sets.
- `crates/raffles/tests/scram_cross_code.rs` — the TwoTrains model in detail,
  with the oracle numbers written into the test doc comments.
- `crates/raffles/docs/scram-port-verification.md` — the full V&V record,
  including what this does **not** establish.
