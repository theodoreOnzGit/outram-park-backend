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

## `oracle.txt`

Six models, 42 basic events, 42 cut sets. Models with more than 40 cut sets
were skipped: the Aralia benchmarks reach 75,379 products and exhausted memory
during a first attempt.

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

## `extract_oracle.sh`

The generator, committed so the fixture can be regenerated rather than trusted.
It parses **SCRAM's own XML report** — basic-event probabilities from the
`<importance>` section, cut sets from `<product>` elements, totals from three
separate runs (`--rare-event`, `--mcub`, and no flag for the exact BDD path).
Nothing is read out of the input models, so the fixture cannot silently drift
from what SCRAM computed.

```bash
cmake <scram-src> -DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF \
      -DWITH_TCMALLOC=OFF -DWITH_JEMALLOC=OFF
make -j4
cp <scram-src>/share/*.rng share/scram/     # the CLI needs its RelaxNG schemas
reference-data/scram/extract_oracle.sh ./bin/scram <scram-src>/input/*/*.xml \
    > reference-data/scram/oracle.txt
```

## Consumed by

- `crates/raffles/tests/scram_oracle_suite.rs` — every model, every mode,
  every basic event.
- `crates/raffles/tests/scram_cross_code.rs` — the TwoTrains model in detail,
  with the oracle numbers written into the test doc comments.
- `crates/raffles/docs/scram-port-verification.md` — the full V&V record,
  including what this does **not** establish.
