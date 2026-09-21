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

## Three fixtures, deliberately separate

| file | parsed from | trusted for |
|---|---|---|
| `oracle.txt` | SCRAM's own **XML report** | the answers, for **coherent** models |
| `oracle-noncoherent.txt` | SCRAM's own **XML report** | the answers, for **non-coherent** models |
| `models.txt` | SCRAM's own **input models** | the question — gates, connectives, arguments, the top gate |

The coherent/non-coherent split is not tidiness. For a non-coherent tree
SCRAM's reported exact probability is the **true function's**, from its BDD,
while its products are the *conservative* minimal cut sets — so a test that
quantifies the products and compares against that total must expect them to
**differ**, and by how much. Keeping the two files apart is what stops a
coherent test from silently inheriting the wrong expectation.

`extract_oracle.sh` reads nothing from the inputs, so no answer in it can have
been copied from the question. `extract_models.sh` emits no answer, so nothing
in it can prejudge a comparison. That split is what lets
`crates/raffles/tests/scram_mocus_oracle.rs` generate cut sets in Rust and
compare them against the ones SCRAM found.

Basic-event probabilities come from `oracle.txt`, **not** from the models:
several models (`HIPPS` especially) define them through `periodic-test` and
`GLM` expressions that SCRAM evaluates and `raffles` does not.

## `oracle.txt`

Nine models, 83 basic events, 450 products.

| model | basic events | products | max order |
|---|---|---|---|
| `TwoTrain/two_train` | 4 | 4 | 2 |
| `Theatre/theatre` | 3 | 2 | 2 |
| `SmallTree/SmallTree` | 4 | 2 | 2 |
| `ThreeMotor/three_motor` | 11 | 12 | 3 |
| `BSCU/BSCU` | 8 | 10 | 2 |
| `Lift/lift` | 12 | 12 | 1 |
| `HIPPS/HIPPS` | 9 | 9 | 2 |
| `ne574/ne574` | 7 | 7 | 3 |
| `Aralia/chinese` | 25 | 392 | 6 |

`Aralia/chinese` is nearly nine times the rest of the fixture combined and is
the only model deep enough to exercise order truncation properly; the others
bottom out at order 3. Extraction of all nine takes about 5 s.

Model selection is mechanical rather than curated. A model is skipped when:

- **it has more than 600 cut sets** — the largest Aralia benchmarks reach
  75,379 products and exhausted memory on a first attempt; or
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

## `oracle-noncoherent.txt`

| model | source | basic events | products | max order |
|---|---|---|---|---|
| `models-for-this-port/noncoherent_small` | **written here** | 3 | 3 | 1 |
| `Aralia/das9601` | upstream `input/` | 108 | 4,259 | 9 |

**One of these inputs is not upstream's, and that needs saying.** Of upstream's
seven models containing a negating connective, four produce no products, two
are event-tree or alignment models the port does not handle, and the only
usable one is `das9601` — at 288 gates, not something a disagreement could be
diagnosed from. `models-for-this-port/noncoherent_small.xml` is committed
beside these fixtures with its reasoning in an XML comment. **SCRAM is still
the oracle**: its expected values came from running the compiled binary, the
same as every other model here. Only the question is ours.

`das9601`'s cut sets are beyond `scram::mocus`, which exhausts its expansion
ceiling on it at every order limit — but **not** beyond `scram::zbdd`, which
reproduces all 4,259 in under a second. Its products are also the only
quantification check at that scale.

## `models.txt`

Fault-tree structure for all eleven models, so a reader can see what was
refused and why.

```
MODEL <suite>/<input basename>
GATE  <name> <connective> <min-or-dash> <g:arg|b:arg> ...
PARAM <basic-event> <probability>
TOP   <name>
END
```

**`PARAM` is the one number this script reports, and it is still the question,
not an answer** — it is what the model *declares*, not what SCRAM computed. It
exists because `crates/raffles/src/scram/bdd.rs` evaluates the whole Boolean
function and so needs every basic event's probability, while SCRAM's report
prices only the events that survive into some product. `Lift`'s `W_1` and the
small non-coherent model's `b` are exactly that case, and substituting a
sentinel for them gives a wrong answer — measured, not hypothesised, in the
first run of `scram_bdd_oracle`. Where the report also has a value the two are
asserted to agree. An event defined by an expression (`GLM`, `periodic-test`)
gets no `PARAM`, because this script does not evaluate expressions.

A gate the parser cannot read becomes a `CANNOT-PARSE <what> <why>` record
rather than a guess, and any model carrying one is skipped by the tests with
the reason printed. Present refusals:

| model | why |
|---|---|
| `ThreeMotor/three_motor` | house events (`E10`, `E12`), and four candidate top gates |

The `xi:include` refusal is also implemented and was exercised on
`TransTest/trans_one`, which the multi-result guard now excludes from the
oracle anyway; it stays as defence in depth.

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
        input/ne574/ne574.xml input/Aralia/chinese.xml"
reference-data/scram/extract_oracle.sh ./bin/scram $MODELS \
    > reference-data/scram/oracle.txt
reference-data/scram/extract_models.sh $MODELS \
    > reference-data/scram/models.txt
```

## Consumed by

- `crates/raffles/tests/scram_mocus_oracle.rs` — end to end: build the tree
  from `models.txt`, generate cut sets with `raffles::scram::mocus`, compare
  them against `oracle.txt`'s products, then quantify and compare the totals.
- `crates/raffles/tests/scram_bdd_oracle.rs` — exact top-event probability by
  binary decision diagram, on every model of both fixtures, with no cut-set
  ceiling; and minimal cut sets by zero-suppressed diagram, compared against
  both SCRAM's products and this crate's own top-down generator.
- `crates/raffles/tests/scram_noncoherent.rs` — the non-coherent models:
  cut-set generation on the small one, the measured conservatism of cut-set
  quantification, and 4,259-product quantification on `das9601`.
- `crates/raffles/tests/scram_oracle_suite.rs` — every model, every mode,
  every basic event, starting from SCRAM's own cut sets.
- `crates/raffles/tests/scram_cross_code.rs` — the TwoTrains model in detail,
  with the oracle numbers written into the test doc comments.
- `crates/raffles/docs/scram-port-verification.md` — the full V&V record,
  including what this does **not** establish.
