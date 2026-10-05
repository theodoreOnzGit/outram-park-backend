# Rung 1 — ENDF: what the evaluator hands us

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.**

**Modules:** `endf`, `moder`, `reference_data`, `acquire`.
**Demo:** [open the nuclear data demo at this rung](../../demos/nuclear-data/?rung=endf): every section on U-238's tape, and its resonance-range flags.

## The question

[Rung 0](./where-sigma-comes-from.md) said the curve σ(E) is not stored
anywhere: an evaluator publishes a compact description of the nucleus instead.
**What exactly is in that file, and how does the code read it?**

## The shortest answer

An ENDF-6 file is a deck of 80-column text lines. Every line carries six
11-character numeric fields and, in columns 67–80, three labels: the
**material** (MAT), the **file** (MF, the kind of data) and the **section**
(MT, the reaction). NJOY's manual describes the hierarchy in one paragraph:

> "ENDF “tapes" are subdivided internally into “materials” (MAT), “files” (MF),
> and “sections” (MT). A MAT contains all data for a particular evaluation for
> an element or isotope (for example, MAT=825 is an evaluation for ¹⁶O in
> ENDF/B-VII). A file contains a particular type of data for that MAT: MF=3 is
> cross-section versus energy data; MF=15 contains secondary photon energy
> distributions. A section refers to a particular reaction [for example, MT=2
> is elastic scattering and MT=107 is the (n, α) reaction]."
>
> — LA-UR-17-20093, §2.4 "ENDF Input-Output", p. 23 (PDF p. 37).

The files a neutron transport calculation touches most:

| MF | contains | read by |
|---|---|---|
| 1 | general information; ν̄ (MT=452/455/456), delayed-neutron data | every module; `nuclear_data::secondary` |
| 2 | **resonance parameters** (MT=151) | RECONR (rung 2), UNRESR/PURR (rung 4) |
| 3 | smooth cross sections σ(E) as tables (MT=1 total, 2 elastic, 18 fission, 102 capture, …) | RECONR |
| 4, 5, 6 | angular, energy and correlated energy-angle distributions of what comes out | ACER (rung 9), transport |
| 7 | **thermal scattering law** S(α,β) | THERMR (rung 5) |
| 12–15 | photon production | HEATR, GROUPR, ACER |
| 31–35 | **covariances** | ERRORR (rung 8) |

## The formula: a table plus an interpolation law

Most ENDF data is a **TAB1 record**: pairs `(x_i, y_i)` and a short table
saying which **interpolation law** joins them. The evaluator's function between
two tabulated points is *defined* by that law, so reading it right is part of
reading the data:

```text
INT = 1  histogram   y = y1
INT = 2  lin-lin     y = y1 + (x - x1) (y2 - y1) / (x2 - x1)
INT = 3  lin-log     y = y1 + ln(x/x1) (y2 - y1) / ln(x2/x1)
INT = 4  log-lin     y = y1 · exp( (x - x1) ln(y2/y1) / (x2 - x1) )
INT = 5  log-log     y = y1 · exp( ln(x/x1) ln(y2/y1) / ln(x2/x1) )
```

The port of `terp1` (`endf.f90`) is shown up to its lin-lin branch. Notice the
comment: **even the order of a multiply and a divide is upstream's**, because a
different grouping rounds differently and the last bit shows up in a written
ACE word (rung 9 compares those words exactly):

```rust,ignore
{{#include ../../../src/endf/interp.rs:lesson_terp1}}
```

## The flags that change what a section means

Several MF=2 header fields are **format flags**: they change how the numbers
after them must be read.

- **`LRU`** — 0: no resonances (scattering radius only); 1: **resolved**
  resonances (each one listed); 2: **unresolved** (only averages).
- **`LRF`** — which resonance formalism the resolved parameters are written
  for: 1 SLBW, 2 MLBW, 3 Reich-Moore, 4 Adler-Adler, 7 R-matrix limited.
  Rung 2 is about these.
- **`LSSF`** — in the unresolved range, whether MF=3 already holds the
  infinitely-dilute cross section (`LSSF = 1`) or the resonance parameters
  must supply it (`LSSF = 0`). NJOY's manual, in its PURR chapter:

  > "The ENDF format contains an option called LSSF. When LSSF=1, the
  > resonance parameters are to be used to compute a fluctuation factor or
  > self-shielding factor that is to be applied to the cross section given in
  > File 3 of the ENDF tape. When LSSF=0, the parameters are used to compute
  > cross sections that are to be added to any possible background corrections
  > that may be given in File 3."
  >
  > — LA-UR-17-20093, §23.1, p. 641 (PDF p. 655).

**Why this belongs on rung 1.** The workspace's "read upstream first" rule was
written after a debugging session that skipped this step. A U-238 discrepancy
was attributed to "unresolved self-shielding not reconstructed", and hours went
into patches. Reading the evaluation first would have shown `LSSF = 1` in
U-238's unresolved range: MF=3 already carries the dilute cross section, and
adding self-shielding there would have moved the answer **further** from the
reference. The record is in the root
[`CLAUDE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/CLAUDE.md),
"Debugging a port: read upstream first". The same flag has the opposite
consequence on U-234, where `LSSF = 0`: rung 4 tells that half.

## The code walk: reading a tape

<!-- code-walk: from=crates/njoy-outram-park-fork/src/endf/tape.rs::Tape::read_file to=crates/njoy-outram-park-fork/src/endf/parse.rs::parse_endf_float
-->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `tape.rs::Tape::read_file` to `parse.rs::parse_endf_float`: 3 hops, 1 shortest chain.

- [`tape.rs::Tape::read_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L98) `pub fn read_file(path: &std::path::Path) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from a file on disk.
  - [`tape.rs::Tape::read`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L113) `pub fn read<R: Read>(reader: R) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from any `Read` source. · called at [L102](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L102)
    - [`parse.rs::parse_line`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/parse.rs#L322) `pub fn parse_line(line: &str) -> Result<RawLine, NjoyError>` — Parse one 80-character ENDF ASCII line. · called at [L133](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L133)
      - [`parse.rs::parse_endf_float`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/parse.rs#L27) `pub fn parse_endf_float(s: &str) -> Result<f64, NjoyError>` — Parse one ENDF 11-column float field. · called at [L332](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/parse.rs#L332)
<!-- /code-walk -->

**Reading the walk.** The block above is generated by `kovan-cli code-walk-check --update`
(rust-analyzer's call hierarchy; every hop links to its lines at the commit this site was built
from). The table below is the same path with what each hop does:

| hop | function | where | resolved (kopitiam pass, 2026-10-04) |
|---|---|---|---|
| 0 | `Tape::read_file` | [`endf/tape.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#@@L:crates/njoy-outram-park-fork/src/endf/tape.rs:fn=read_file@@) | entry point |
| 1 | `Tape::read` | [`endf/tape.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#@@L:crates/njoy-outram-park-fork/src/endf/tape.rs:fn=read@@) | kopitiam |
| 2 | `parse_line`, splits the six fields and MAT/MF/MT | [`endf/parse.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/parse.rs#@@L:crates/njoy-outram-park-fork/src/endf/parse.rs:fn=parse_line@@) | kopitiam |
| 3 | `parse_endf_float`, one 11-column number | [`endf/parse.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/parse.rs#@@L:crates/njoy-outram-park-fork/src/endf/parse.rs:fn=parse_endf_float@@) | filled by hand (inside `parse_line`; below the walk's resolution) |
| — | per consumer: `Tape::section(mat, mf, mt)` then `SectionCursor::read_cont / read_tab1` | [`endf/records.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/records.rs#@@L:crates/njoy-outram-park-fork/src/endf/records.rs:fn=read_tab1@@) | kopitiam (hop 1 of the `reconr` walk) |
| — | `interp::eval_tab1` then `terp1`, to evaluate a TAB1 | [`endf/interp.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/interp.rs#@@L:crates/njoy-outram-park-fork/src/endf/interp.rs:fn=eval_tab1@@) | filled by hand |


The Fortran keeps a tape on a logical unit and searches it (`findf`,
`tosend`, …). The port reads the whole tape into memory once and indexes the
sections by `(MAT, MF, MT)`, so `tape.section(9237, 2, 151)` is a hash lookup.
Sentinel lines (SEND, FEND, MEND, TEND) are dropped at read time.

## The check: a number read twice

ENDF writes `1.090000+7` for 1.09 × 10⁷, with no `E`. The obvious way to read it
is mantissa × 10^exponent. **That rounds twice**, once for the mantissa and once
for the product, and an ENDF energy can come out one unit in the last place
away from the decimal the evaluator wrote. Since 2026-09-26 the whole decimal
is converted once, as a Fortran formatted read does:

```rust,ignore
{{#include ../../../src/endf/parse.rs:lesson_float_once}}
```

**Why one ulp mattered.** ACER's MT=18 fission-spectrum tail test is strict,
`dele > 2e5`. On U-235 the 10.9–11.1 MeV panels are *exactly* 200 keV wide in
the evaluation. Read as `m · 10^e`, the width came out a hair over 200 000 eV,
so this crate split panels NJOY left alone: **16 extra points per incident
energy and 2 568 extra words** in the ACE energy-distribution block. It was
found only because rung 9 compares ACE files word by word with NJOY2016's. The
record is the comment above (2026-09-26) and the crate's
[`CLAUDE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/CLAUDE.md),
"The ACE tables reproduce NJOY's".

A second reading defect is recorded just above it in the same function: some
ENDF/B-VIII.0 tapes (C-12, C-13, O-16, Li-7 MF=3) write an explicit exponent
letter (`1.00000E-5`). Before the fix those fields **silently became 0.0**.

## The supporting modules

- **`moder`** — NJOY's tape-mode converter. In the port the "logical unit"
  indirection disappears, so MODER is material selection
  ([`moder::select_materials`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/moder/mod.rs#@@L:crates/njoy-outram-park-fork/src/moder/mod.rs:fn=select_materials@@))
  plus ASCII writing (`Tape::write`, `format_endf_float`, a port of `a11`).
  Blocked-binary conversion is **not planned**: the in-memory model replaces
  it ([`moder/README.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/moder/README.md)).
  V&V: ~~verified by hand per its README; **no NJOY2016 comparison** of a
  MODER-written tape is recorded~~ **compared with NJOY2016's own MODER on
  2026-10-05** ([#536](https://github.com/theodoreOnzGit/outram-park-backend/issues/536)):
  selecting H-2 and Li-6 from two ENDF/B-VIII.0 tapes gives the same 11 793
  lines with the same MAT/MF/MT on each, all **59 098** `a11` number fields
  character-identical and every value bit-identical. ~~It is **not** a
  byte-faithful MODER: one line in 11 793 is byte-identical, because a row of
  six floats cannot say which fields NJOY writes as integers (`i11`), which
  it leaves blank, or which are text. The text is the one that matters:
  **the evaluation's MF=1/MT=451 description does not survive** a write by
  this port.~~ **Since #553 (later on 2026-10-05) it is byte-faithful on that
  tape: all 11 793 lines identical**, the description included. The port
  recovers each line's record type by walking the section the way MODER
  does ([#553](https://github.com/theodoreOnzGit/outram-park-backend/issues/553)).
  MF=32 and GENDF/ERRORR materials are still written in the old all-`a11`
  form. Record:
  [`moder_vs_njoy2016.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/verification_and_validation/moder_vs_njoy2016.md).
- **`reference_data`** — where the workspace's reference tapes live. They sit
  at the repository root in `reference-data/endf/` (82 entries on
  2026-10-04: ENDF/B-VII.0, -VII.1, -VIII.0, -VIII.1, JENDL-3.3, TENDL-2023,
  thermal-scattering, photo-atomic and three synthetic tapes with their
  generators), **never** inside a crate, because a crates.io package is capped
  at 10 MB. `reference_endf("n-092_U_238.endf")` returns the path or `None`, and
  data-gated tests skip rather than fail. `tests/no_endf_inside_crates.rs`
  enforces the layout.
- **`acquire`** — the crate's single on-disk artifact cache (`EndfCache`:
  platform cache directory, advisory lock, atomic rename, SHA-256 sidecar). Its
  download half, behind the opt-in `net-fetch` feature, fetches raw tapes from
  a pinned upstream; the default build carries no network code.

## Predict

MF=2 of ENDF/B-VIII.0 U-238 (`n-092_U_238.endf`, MAT 9237) holds one
Reich-Moore (`LRF = 3`) resolved range from 1e-5 eV to 20 keV with **3 345
resonances** in two `l`-states, then an unresolved range (`LRU = 2`) up to
149 keV. (Counted 2026-10-04 by summing the `NRS` field of each `l`-state
LIST record of MF=2/MT=151; ENDF/B-VIII.0 U-235 has 3 195 below 2.25 keV.)
Each resonance has an energy and a few widths. **To draw the
curve they describe to 0.1 %, will the code need about as many points as
resonances, ten times as many, or a hundred times as many?**

**Next:** [Rung 2 — RECONR: from resonance parameters to σ(E)](./reconr.md).
