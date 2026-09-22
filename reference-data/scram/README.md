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

### Upstream's own test suite passes on this build

The vendored Catch2 uses `MINSIGSTKSZ` in a constant expression, which modern
glibc no longer permits — but that is confined to Catch2's POSIX signal
handling, which it can be told to omit. Adding
`-DBUILD_TESTING=ON -DCMAKE_CXX_FLAGS="-DCATCH_CONFIG_NO_POSIX_SIGNALS"` to
the configure line builds `scram_tests` with no source patched, and

```
All tests passed (540 assertions in 71 test cases)
```

(excluding `~[perf]`, which measures timings rather than asserting answers;
and symlinking the built `libscram_dummy_extern.so` to `<scram-src>/build/lib/
scram/`, which one test locates by a path relative to its input).

**This is what makes the fixtures trustworthy.** Upstream's suite has a test
per benchmark model and every model here has one — `RiskAnalysisTest.TwoTrain`,
`.Theatre`, `.SmallTree`, `.ThreeMotor`, `.BSCU`, `.Lift`, `.HIPPS`, `.ne574`,
`.ChineseTree` — asserting upstream's own expected products and
probabilities. So these numbers are not merely what this binary printed; they
are what SCRAM's authors say it should print, checked.

## Six fixtures, deliberately separate

| file | parsed from | trusted for |
|---|---|---|
| `oracle.txt` | SCRAM's own **XML report** | the answers, for **coherent** models |
| `oracle-noncoherent.txt` | SCRAM's own **XML report** | the answers, for **non-coherent** models |
| `oracle-prime-implicants.txt` | SCRAM's own **XML report**, under `--prime-implicants` | the **signed** products, which describe the function exactly |
| `oracle-mef.txt` | SCRAM's own **XML report** | the answers for `mef_features`, the model covering the MEF constructs upstream's inputs never use |
| `oracle-multi-tree.txt` | SCRAM's own **XML report** | the answers for models defining **several** fault trees, one record per tree |
| `models.txt` | SCRAM's own **input models** | the question — gates, connectives, arguments, the top gate, declared probabilities |

`models.txt` is no longer the only route to the question. Since 2026-09-22
`raffles::scram::mef` reads upstream's XML directly, and
`crates/raffles/tests/scram_mef.rs` takes the question from
`upstream-input/` — the models themselves, copied verbatim — rather than from
the transcription. The transcription stays because the tests built on it
stay, and because a second, independent reading of the same models is worth
keeping.

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

Basic-event probabilities come from `oracle.txt`, **not** from the models, for
every test built on `models.txt`: several models (`HIPPS` especially) define
them through `periodic-test` and `GLM` expressions, which that shell script
does not evaluate. ~~expressions that SCRAM evaluates and `raffles` does
not~~ **CORRECTED 2026-09-22** — `raffles::scram::expression` evaluates them,
and `scram_mef.rs` computes all 94 basic-event probabilities from the models'
own expressions and checks them against the ones SCRAM printed. The statement
above is about the *shell extractor*, not about the crate.

## `oracle.txt`

Ten models, 85 basic events, 452 products.

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
| `models-for-this-port/house_events_small` | 2 | 2 | 1 |

`Aralia/chinese` is nearly nine times the rest of the fixture combined and is
the only model deep enough to exercise order truncation properly; the others
bottom out at order 3. Extraction of all nine takes about 5 s.

Model selection is mechanical rather than curated. A model is skipped when:

- **it has more than 600 cut sets** — the largest Aralia benchmarks reach
  75,379 products and exhausted memory on a first attempt. Left out by
  **maintainer direction, 2026-09-21** — a closed decision rather than a
  pending one, so do not raise this cap to chase them; or
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

## `oracle-prime-implicants.txt`

Eleven models, 455 prime implicants, from `scram --probability --importance
--prime-implicants`. Ten are checked: `ThreeMotor` is captured but its
structure is unreadable, so 443 of the 455 take part. Products here carry **signs**: `PI 2 +a -b` means `a`
must occur and `b` must not.

For a coherent tree the prime implicants are the minimal cut sets, so ten of
these eleven duplicate `oracle.txt` — deliberately, because that duplication
is the invariant `scram_prime_implicants` checks. The eleventh,
`models-for-this-port/noncoherent_small`, is the only one with a negative
literal anywhere.

**`Aralia/das9601` is absent because upstream cannot produce it**: `scram
--prime-implicants` exceeds five minutes on a model whose minimal cut sets it
gives in about a second. The generator reports the timeout on stderr rather
than dropping it silently.

## Upstream's input models, copied verbatim

`upstream-input/` holds the `.xml` inputs themselves, copied unchanged from
upstream's `input/` at commit `b85b7894`, so that
`crates/raffles/tests/scram_mef.rs` can read the *models* rather than a
transcription of them. SCRAM is GPL-3.0-or-later and this repository is
GPL-3.0-only, so the copy is same-licence; the provenance is the table at the
top of this file.

Fifteen files: the nine models `oracle.txt` covers, `Aralia/das9601`,
`ThreeLevels/top` and the three of `TransTest/` (which `<xi:include>` splices),
and `TwoTrain/common_cause.xml`, which exists only so a test can check that a
model using CCF groups is **refused** rather than read as a smaller model.

## Models written for this port

Three, all under `models-for-this-port/`, each needed because upstream has
no *small* model exercising the feature — or, for the third, no model at all:

| model | why it was written |
|---|---|
| `noncoherent_small.xml` | of upstream's seven models with a negating connective, four produce no products, two are event-tree or alignment models, and the only usable one is `das9601` at 288 gates |
| `house_events_small.xml` | upstream's only house-event model is `ThreeMotor`, which the extractor refuses for its `<define-component>` namespaces |
| `mef_features.xml` | **no** upstream input uses `<iff>`, `<imply>`, `<cardinality>`, a `<constant>` formula argument or `<event type="…">`; a grep over all of `input/` finds none of the five. It also carries a second, independent instance of the private-namespace rule, with a shadowed parameter whose value differs between scopes |

**SCRAM is the oracle for both**: their expected answers come from running the
compiled binary, exactly as for upstream's own models. Only the question is
ours, and each file says so in an XML comment with its reasoning and a
by-hand prediction of the answer.

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
| `ThreeMotor/three_motor` | uses `<define-component role="private">`, whose private namespaces this script does not model — an inner gate `E1` is really `t.E1` and distinct from the outer `E1`, and the script was silently merging them |

**That refusal is now about the script alone.** `raffles::scram::mef` reads
`ThreeMotor` correctly — 18 gates, 11 public and 7 inside the private
component — and `scram_mef.rs` reproduces SCRAM's 12 products for it. The
`CANNOT-PARSE` record stays because `extract_models.sh` still cannot read the
model, which is the honest statement about that file.

House events are **no longer** a reason to refuse a model: they are emitted as
`h:` arguments and `HOUSE <name> <true|false>` records, and
`raffles::scram` handles them. `ThreeMotor` was previously refused partly for
having them, which was a symptom rather than the cause.

The `xi:include` refusal is also implemented and was exercised on
`TransTest/trans_one`, which the multi-result guard now excludes from the
oracle anyway; it stays as defence in depth.

Only the flat single-connective gate form these models use is handled — one
`<and>`/`<or>`/`<atleast>` per `<define-gate>`, or a bare `<event/>` child
meaning a pass-through (`null`) gate. ~~A nested formula would be silently
mis-parsed, so it is refused instead.~~ **CORRECTED 2026-09-22** — MEF has no
nested formulas to mis-parse: upstream's grammar (`share/input.rng`) lets a
connective take only an event reference, a `<not>` around one event, or a
`<constant>`, and SCRAM itself rejects a nested one. The script's guard is
harmless but the reason given for it was wrong. What the script really cannot
read is `<not>`, `<constant>`, `<iff>`, `<imply>` and `<cardinality>`, none of
which appears in the models it is run on; `raffles::scram::mef` reads all of
them.

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

# The per-tree fixture, for models defining several fault trees.
reference-data/scram/extract_multi_tree_oracle.sh ./bin/scram \
    input/TransTest/trans_one.xml input/ThreeLevels/top.xml \
    > reference-data/scram/oracle-multi-tree.txt

# The MEF-construct model, which lives here rather than upstream.
reference-data/scram/extract_oracle.sh ./bin/scram \
    reference-data/scram/models-for-this-port/mef_features.xml \
    > reference-data/scram/oracle-mef.txt
```

## Consumed by

- `crates/raffles/tests/scram_mocus_oracle.rs` — end to end: build the tree
  from `models.txt`, generate cut sets with `raffles::scram::mocus`, compare
  them against `oracle.txt`'s products, then quantify and compare the totals.
- `crates/raffles/tests/scram_bdd_oracle.rs` — exact top-event probability by
  binary decision diagram, on every model of both fixtures, with no cut-set
  ceiling; and minimal cut sets by zero-suppressed diagram, compared against
  both SCRAM's products and this crate's own top-down generator.
- `crates/raffles/tests/scram_prime_implicants.rs` — prime implicants with
  their signs, and the invariant that a coherent tree's are its cut sets.
- `crates/raffles/tests/scram_noncoherent.rs` — the non-coherent models:
  cut-set generation on the small one, the measured conservatism of cut-set
  quantification, and 4,259-product quantification on `das9601`.
- `crates/raffles/tests/scram_oracle_suite.rs` — every model, every mode,
  every basic event, starting from SCRAM's own cut sets.
- `crates/raffles/tests/scram_cross_code.rs` — the TwoTrains model in detail,
  with the oracle numbers written into the test doc comments.
- `crates/raffles/tests/scram_mef.rs` — the MEF reader: upstream's models read
  from their own XML, basic-event probabilities evaluated from the models'
  own expressions, and the cut sets, totals and importance factors that
  follow — `ThreeMotor` included, and `<xi:include>`, the private-namespace
  rule and the five MEF constructs upstream never uses with them.
- `crates/raffles/docs/scram-port-verification.md` — the full V&V record,
  including what this does **not** establish.
