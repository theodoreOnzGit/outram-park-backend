# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `kovan_metrics`

# KOVAN metrics — repository accounting

Per-commit **API-token accounting** and the pre-merge **historian report**,
for the OUTRAM PARK workspace. This replaced `docs/historian/token_usage.py`
and `docs/historian/historian.py` on 2026-08-13, and both were deleted the
same day — this crate is now the only implementation. It exists so the
toolchain needs no Python interpreter, which on Windows in particular is a
recurring failure mode (a `python3` that resolves to a Microsoft Store alias
stub silently turns the git hooks into no-ops).

## What belongs here

Read-mostly accounting *about* a repository: token usage attributed to
commits, and lines/KLOC written over a window of history. It fits KOVAN's
stated focus on **traceability** and **engineering reproducibility**.

## What does not belong here

Anything that modifies the repository's content, any network access, and any
estimation. This crate reports measurements or reports nothing.

## Module map

| Module | Responsibility |
|---|---|
| [`date`] | `DDMMYY` window notation and civil-date arithmetic, dependency-free |
| [`git`] | The git queries used here; repo discovery reuses `kovan-discovery` |
| [`trailer`] | The `API-Usage-*` commit trailers — parse, format, token arithmetic |
| [`transcript`] | Reading token usage out of the Claude Code session transcripts |
| [`baseline`] | The per-clone baseline that makes "since the last commit" meaningful |
| [`tokens`] | Write side (git hooks) and query side (history) |
| [`historian`] | The pre-merge-to-`main` report generator |

## The two rules that govern this crate

**Never block a commit.** The write-side entry points run inside
`prepare-commit-msg` and `post-commit`. They swallow their own errors and
degrade to a zero/`source=none` trailer rather than failing. A caller in the
hook path must preserve that.

**Never invent a number.** Token figures come from the session transcripts
and, once recorded, from the commit trailers themselves. A commit made
outside a Claude session honestly reads `total=0 source=none`, and a commit
predating the hooks honestly has no trailer at all. Neither is a gap to be
filled with an estimate.

## Example

```no_run
use kovan_metrics::{date::Date, tokens};

// Sum what the commit trailers on `develop` recorded for August 2026.
let result = tokens::query(
    Date::parse_ddmmyy("010826").ok(),
    Date::parse_ddmmyy("310826").ok(),
    "develop",
);
println!("{} tokens over {} commits", result.grand_total, result.commits_total);
```

## Modules

## Module `baseline`

The per-commit baseline — how "since the last commit" is computed.

The transcripts only ever grow, so a *cumulative* reading is not by itself
attributable to a commit. The baseline is the cumulative reading as of the
previous commit; the delta stamped into a trailer is `now - baseline`, and
the `post-commit` hook advances the baseline afterwards.

It lives at `<git-dir>/claude-token-baseline.json` — inside `.git/`, so it
is per-clone, never committed, and cannot collide across worktrees.

Attribution is therefore **temporal, not per-diff**: a commit is charged the
tokens spent between the previous commit and itself, whatever files those
tokens actually touched.

```rust
pub mod baseline { /* ... */ }
```

### Functions

#### Function `path`

Absolute path to the baseline file.

```rust
pub fn path() -> std::path::PathBuf { /* ... */ }
```

#### Function `load`

Read the stored baseline, or `None` when this clone has never stamped one.

Any failure — missing file, unreadable, malformed JSON — reads as `None`,
which the caller treats as "first run" rather than an error.

```rust
pub fn load() -> Option<crate::trailer::TokenCounts> { /* ... */ }
```

#### Function `save`

Write `counts` as the new baseline, recording `records` for diagnostics.

Errors are swallowed: failing to advance the baseline must never abort a
commit. The next commit simply attributes a larger window.

```rust
pub fn save(counts: &crate::trailer::TokenCounts, records: u64) { /* ... */ }
```

## Module `date`

Calendar dates, without a date crate.

This crate needs exactly three things from a calendar: parse the workspace's
`DDMMYY` window notation, format a date back out, and know what today is for
the default report window. That is not enough to justify pulling `chrono` /
`time` / `jiff` into `[workspace.dependencies]` — none of which is currently
there — so the civil-date conversion is done here with Howard Hinnant's
`days_from_civil` / `civil_from_days` algorithms (public domain, from
<http://howardhinnant.github.io/date_algorithms.html>).

**Timezone caveat.** [`today`] is **UTC**, because `std` exposes no local
timezone. The Python it replaces used `datetime.date.today()`, which is
*local*. The two differ only for the hours either side of midnight, and only
for the default `--to` bound and the generated filename tag — never for the
token figures themselves, which come from commit trailers. Pass `--to`
explicitly if the boundary matters.

```rust
pub mod date { /* ... */ }
```

### Types

#### Struct `Date`

A proleptic-Gregorian calendar date: year, month (1-12), day (1-31).

Ordering is chronological, so `from <= commit_date <= to` window tests are
just comparisons.

```rust
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `year` | `i32` | Full year, e.g. `2026`. |
| `month` | `u32` | Month of year, `1..=12`. |
| `day` | `u32` | Day of month, `1..=31`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(year: i32, month: u32, day: u32) -> Result<Self, DateError> { /* ... */ }
  ```
  Build a date, rejecting anything that is not a real calendar day.

- ```rust
  pub fn parse_ddmmyy(s: &str) -> Result<Self, DateError> { /* ... */ }
  ```
  Parse the workspace's `DDMMYY` window notation — day, month, then a

- ```rust
  pub fn iso(self: &Self) -> String { /* ... */ }
  ```
  `YYYY-MM-DD`, the form git accepts for `--since` / `--until`.

- ```rust
  pub fn ddmmyy(self: &Self) -> String { /* ... */ }
  ```
  `DDMMYY`, the form used in generated report filenames.

- ```rust
  pub fn human(self: &Self) -> String { /* ... */ }
  ```
  `13 Aug 2026` — the human-facing form used in report headings.

- ```rust
  pub fn to_epoch_days(self: Self) -> i64 { /* ... */ }
  ```
  Days since the Unix epoch (1970-01-01), negative before it. Hinnant's

- ```rust
  pub fn from_epoch_days(days: i64) -> Self { /* ... */ }
  ```
  Inverse of [`to_epoch_days`](Date::to_epoch_days). Hinnant's

- ```rust
  pub fn today() -> Self { /* ... */ }
  ```
  Today's date **in UTC** — see the module note on the timezone caveat.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Date { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Date) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Date) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Date) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `DateError`

Why a `DDMMYY` string could not be read as a date.

```rust
pub enum DateError {
    NotSixDigits(String),
    NotACalendarDate(u32, u32, i32),
}
```

##### Variants

###### `NotSixDigits`

The input was not exactly six ASCII digits.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotACalendarDate`

The digits parsed but do not name a real calendar day (month 13, 31
February, 29 February in a common year, …).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u32` |  |
| 1 | `u32` |  |
| 2 | `i32` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Error**
- **ErrorExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DateError) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `git`

The git queries this crate needs, and nothing more.

**Reuse note (workspace search-first rule).** Repository-root discovery goes
through [`kovan_discovery::git::GitProvider`], which already wraps gitoxide —
this crate does not open a second git layer for that. What `kovan-discovery`
does *not* expose is arbitrary `git log --format=…` / `--numstat` queries,
because its `CommitInfo` is a fixed typed record with no message body and no
diff statistics. Those two needs are served here by [`git_output`], a thin
wrapper over the `git` binary.

**Why the binary rather than gitoxide for those.** This code runs inside
`prepare-commit-msg` and `post-commit`, so `git` is by construction present
and already in the process's environment. Shelling out also keeps the
rename-compaction and date-window semantics byte-identical to the Python
implementation being replaced, rather than re-deriving them.

Every helper here **degrades to empty output rather than failing**. That is
deliberate and load-bearing: the hook path must never block a commit.

```rust
pub mod git { /* ... */ }
```

### Types

#### Struct `CommitRecord`

One commit as this crate reads it: enough to attribute tokens and render a
ledger row.

```rust
pub struct CommitRecord {
    pub short: String,
    pub date: String,
    pub subject: String,
    pub body: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `short` | `String` | Abbreviated commit hash (`%h`). |
| `date` | `String` | Author date as `YYYY-MM-DD` — the first 10 characters of `%aI`. |
| `subject` | `String` | Commit subject, the first line of the message (`%s`). |
| `body` | `String` | Commit message body (`%b`) — where the `API-Usage-*` trailers live. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CommitRecord { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CommitRecord) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NumStat`

Per-commit diff statistics, as used by the historian report.

```rust
pub struct NumStat {
    pub added: u64,
    pub removed: u64,
    pub rs_added: u64,
    pub rs_removed: u64,
    pub per_crate_added: Vec<(String, u64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `added` | `u64` | Lines added across all text files. |
| `removed` | `u64` | Lines removed across all text files. |
| `rs_added` | `u64` | Lines added in `.rs` files only. |
| `rs_removed` | `u64` | Lines removed in `.rs` files only. |
| `per_crate_added` | `Vec<(String, u64)>` | Lines added, keyed by the `crates/<name>/` directory they landed in. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> NumStat { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> NumStat { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NumStat) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `git_output`

Run `git` with `args` and return its stdout, or an empty string on any
failure (git missing, non-zero exit, non-UTF-8 output).

Mirrors the Python `git()` helper: errors are swallowed, never raised.

```rust
pub fn git_output(args: &[&str]) -> String { /* ... */ }
```

#### Function `repo_root`

Absolute path to the repository working-tree root.

Tries `kovan-discovery`'s gitoxide-backed discovery first (the workspace's
canonical git layer), then falls back to `git rev-parse --show-toplevel`,
then to the current directory.

```rust
pub fn repo_root() -> std::path::PathBuf { /* ... */ }
```

#### Function `git_dir`

Absolute path to the `.git` directory (resolved against the repo root when
git reports it relatively, as it does from inside a hook).

```rust
pub fn git_dir() -> std::path::PathBuf { /* ... */ }
```

#### Function `ref_for_branch`

Resolve a branch name to the ref that should be reported on, preferring the
remote-tracking ref so a report reflects what is actually published.

Tries `origin/<branch>`, then `<branch>`, and returns `<branch>` unchanged
when neither resolves (letting git produce the eventual error).

```rust
pub fn ref_for_branch(branch: &str) -> String { /* ... */ }
```

#### Function `parse_records`

Parse `git log`/`git show` output formatted as
`%h<FS>%aI<FS>%s<FS>%b<RS>` into records.

Records with fewer than three fields are skipped. Anything after the third
separator is re-joined into `body`, so a body containing a literal unit
separator cannot truncate the record.

```rust
pub fn parse_records(raw: &str) -> Vec<CommitRecord> { /* ... */ }
```

#### Function `record_format`

The `--format` string that [`parse_records`] expects.

```rust
pub fn record_format() -> String { /* ... */ }
```

#### Function `numstat`

Diff statistics for a single commit, from `git show --numstat`.

Binary files (which git reports as `-` / `-`) are skipped rather than
counted as zero-line changes.

```rust
pub fn numstat(sha: &str) -> NumStat { /* ... */ }
```

### Constants and Statics

#### Constant `FS`

ASCII unit separator — delimits fields within one commit record.

```rust
pub const FS: char = '\u{1f}';
```

#### Constant `RS`

ASCII record separator — delimits commit records from each other.

```rust
pub const RS: char = '\u{1e}';
```

## Module `historian`

The historian report — pre-merge-to-`main` accounting.

Before `develop` is merged into `main`, the workspace generates a report
accounting for the **API tokens spent** and the **lines / KLOC written**
across the window of history being released.

# Sources, not estimates

- **Lines** come from `git log --numstat --no-merges` over the range.
- **Tokens** come from the `API-Usage-Since-Last-Commit` commit trailers.

Commits predating the token hooks legitimately carry *no token data*. They
are counted in the line totals and shown with `—` in the token column. That
is correct output, not a gap to be filled in.

# Default window

With no `--from`, the window is "everything on `<branch>` not yet on
`<base>`" (i.e. `base..branch`), which is exactly what the pending merge
would deliver.

```rust
pub mod historian { /* ... */ }
```

### Functions

#### Function `generate`

Generate a historian report and write it to disk.

Returns the path written and the number of non-merge commits covered.

With `from` unset the window defaults to `base..branch` — everything on
`branch` not yet on `base` — and `to` defaults to today only when `from` was
given, matching the Python this replaces.

```rust
pub fn generate(from: Option<crate::date::Date>, to: Option<crate::date::Date>, branch: &str, base: &str, outfile: Option<std::path::PathBuf>) -> Result<(std::path::PathBuf, usize), String> { /* ... */ }
```

#### Function `default_output_path`

Resolve the default output path for a window, without writing anything.

```rust
pub fn default_output_path(root: &std::path::Path, from: Option<crate::date::Date>, to: Option<crate::date::Date>, base: &str) -> std::path::PathBuf { /* ... */ }
```

### Constants and Statics

#### Constant `REPORT_DIR_REL`

Where generated reports live, relative to the repository root.

```rust
pub const REPORT_DIR_REL: &str = "docs/historian";
```

## Module `kloc`

`kovan kloc` — the productivity accounting behind the Outram Park paper.

Reproduces the tables and figure the manuscript reports: how many lines the
pre-agentic repositories hold and over how many active days, how many lines
the agentic month produced, and what that output is made of.

# Why this exists at all

The manuscript's tables were originally compiled with AI assistance from
repository measurements. This code exists so those numbers can be
**re-derived from the repositories themselves, by anyone**, without trusting
that summary — which is what a journal asks for when a table or figure is
"directly derived from underlying data using reproducible analytical,
computational, or statistical methods".

It is a Rust port of the retired `scripts/kloc_accounting.py`, which was
deleted under the workspace's "no Python for documentation or accounting"
rule. **The port is gated on byte-for-byte parity** with that script's
output, frozen in `docs/kloc-parity-baseline/` — a capture that reproduces
every published figure exactly, all eight drift-check deltas `+0`.

# Source of truth

The git repositories. Nothing is estimated and nothing is hard-coded from
the manuscript. The single editorial input is the classification of each
crate as translated, original or an extension, which lives in [`config`]
beside each crate's own `Cargo.toml` description so it can be audited.

The one set of numbers copied from the manuscript,
[`MANUSCRIPT`](config::MANUSCRIPT), is used **only** to report drift and
never in a computation.

# The trap worth knowing about

An extension crate subtracts its standalone pre-agentic original, so a
**missing baseline repository silently moves those lines into the agentic
total** rather than merely shrinking the baseline. Measured on this
workspace, three absent repositories moved the baseline from 181,298 to
162,163 code lines and the agentic total from 175,997 to 187,378. The report
refuses to validate its drift check while any repository is missing; do not
quote a run that carries that warning.

```rust
pub mod kloc { /* ... */ }
```

### Modules

## Module `config`

The measurement's configuration: which repositories form the baseline, how
each crate is classified, and which commits are pinned.

**This is the one editorial input to the whole accounting.** Everything else
is measured from git. The classification of a crate as translated, original
or an extension is a judgement, and it is carried here — beside each crate's
own `Cargo.toml` description in the output — precisely so that it can be
audited rather than taken on trust.

Ported from the retired `scripts/kloc_accounting.py`. The reasoning comments
are reproduced rather than summarised: each one records why a number is what
it is, and losing them would leave the constants looking arbitrary.

```rust
pub mod config { /* ... */ }
```

### Types

#### Struct `RepoSpec`

A checkout to measure.

```rust
pub struct RepoSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub marker: &'static str,
    pub cite: &'static str,
    pub measure_ref: Option<&'static str>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `&'static str` | Directory and GitHub repository name. |
| `label` | `&'static str` | How the manuscript names it, as LaTeX. |
| `note` | `&'static str` | Footnote text for the rate table's "AI?" column. |
| `marker` | `&'static str` | Footnote marker (a, b, c) in the table. |
| `cite` | `&'static str` | Bib key, cited in the table row. |
| `measure_ref` | `Option<&'static str>` | Count lines at this commit instead of at the branch tip.<br><br>Needed only where a repository was emptied after its code moved<br>elsewhere — see [`BASELINE_REPOS`]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RepoSpec { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Provenance`

How a crate's lines are attributed.

```rust
pub enum Provenance {
    Translated,
    Original,
    Extension,
}
```

##### Variants

###### `Translated`

A pure-Rust fork or port of a named upstream project.

###### `Original`

Newly written for Outram Park.

###### `Extension`

A vendored pre-agentic crate: only the excess over the standalone
original is agentic.

##### Implementations

###### Methods

- ```rust
  pub fn key(self: Self) -> &'static str { /* ... */ }
  ```
  Short machine-readable name, as written to CSV.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Section heading used in the console report.

- ```rust
  pub fn tex_section(self: Self) -> &'static str { /* ... */ }
  ```
  Section heading used in the LaTeX table.

- ```rust
  pub fn tex_subtotal(self: Self) -> &'static str { /* ... */ }
  ```
  Subtotal row label used in the LaTeX table.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Provenance { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Provenance) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Provenance) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Provenance) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `provenance_of`

Look up a crate's classification.

```rust
pub fn provenance_of(crate_name: &str) -> Option<(Provenance, Option<&'static str>)> { /* ... */ }
```

### Constants and Statics

#### Constant `GITHUB_USER`

GitHub account hosting the pre-agentic repositories.

```rust
pub const GITHUB_USER: &str = "theodoreOnzGit";
```

#### Constant `BASELINE_REPOS`

The pre-agentic baseline, in the order the manuscript's table lists them.

# Why `thermal_hydraulics_rs` is pinned to a commit

On 2024-10-11 that repository was **emptied** — 260 `.rs` files down to 9 —
when its code moved into TUAS. Its tip therefore holds ~1.6 KLOC, which
would understate the predecessor by ~58 KLOC and, worse, would make the
"TUAS net of what it inherited" subtraction meaningless. `4d534af` is the
last commit at full extent (2024-10-08, 260 `.rs` files).

```rust
pub const BASELINE_REPOS: &[RepoSpec] = _;
```

#### Constant `BASELINE_FOOTNOTES`

The lettered footnotes under the baseline tables.

```rust
pub const BASELINE_FOOTNOTES: &[(&str, &str)] = _;
```

#### Constant `TUAS_KEY`

TUAS, and the predecessor it is reported net of.

```rust
pub const TUAS_KEY: &str = "tuas_boussinesq_solver";
```

#### Constant `TUAS_PREDECESSOR_KEY`

The predecessor whose imported tree is subtracted from TUAS.

```rust
pub const TUAS_PREDECESSOR_KEY: &str = "thermal_hydraulics_rs";
```

#### Constant `TUAS_IMPORT_REF`

TUAS's second commit, which imported the predecessor wholesale 42 minutes
after the initial commit.

# What is subtracted, and why it is this and not the predecessor's extent

What is removed is **the code that actually came across** — the tree at this
commit — not the predecessor's own full extent. The two differ: the
predecessor held 260 `.rs` files at `4d534af`; 236 were imported.
Subtracting its full extent would remove 7,754 code lines that were written
in the predecessor and never carried forward, erasing real pre-agentic work
from the baseline and — because the baseline is the denominator of the
productivity claim — flattering the agentic figure. It would also contradict
the table caption, which says "net of the code it inherited": inherited
means what arrived, not what the predecessor happened to contain.

```rust
pub const TUAS_IMPORT_REF: &str = "c451c8e203d5772955c2c9f3c6739e92b8180c78";
```

#### Constant `AGENTIC_MEASURE_REF`

The agentic repository is pinned to a commit, not to the branch tip.

`develop` is under active daily development — it moved four commits during a
single afternoon of preparing these tables, changing the translated subtotal
by 462 lines. A manuscript that quotes the tip quotes a number nobody can
reproduce afterwards. Set to `None` to measure the tip instead; the drift
check will then report how far the repository has moved since the pin.

```rust
pub const AGENTIC_MEASURE_REF: Option<&str> = _;
```

#### Constant `AGENTIC_KEY`

The agentic repository.

```rust
pub const AGENTIC_KEY: &str = "outram-park-backend";
```

#### Constant `AGENTIC_LABEL`

How the manuscript names it.

```rust
pub const AGENTIC_LABEL: &str = r"\texttt{outram-park-backend}";
```

#### Constant `AGENTIC_SINCE`

Start of the agentic window reported in the manuscript.

```rust
pub const AGENTIC_SINCE: &str = "2026-06-19";
```

#### Constant `AGENTIC_UNTIL`

End of the agentic window reported in the manuscript.

```rust
pub const AGENTIC_UNTIL: &str = "2026-07-23";
```

#### Constant `CRATE_PROVENANCE`

Provenance of each crate in `outram-park-backend/crates`, with the upstream
it derives from (for [`Provenance::Translated`]) or the pre-agentic
repository it extends (for [`Provenance::Extension`]).

```rust
pub const CRATE_PROVENANCE: &[(&str, Provenance, Option<&str>)] = _;
```

#### Constant `CRATE_SUBPATH_PROVENANCE`

Parts of a crate classified differently from the crate as a whole.

On 2026-07-23 the two terminal interfaces stopped being workspace crates and
became feature-gated binaries inside the libraries they drive, so a library
consumer no longer sees them as separate packages. Their code is newly
written, but it now lives inside crates that are ports of an existing
upstream. Counting them with their host crate would credit ~2.5 KLOC of
original interface work as translation and overstate the translated share.
They are therefore split out at the path boundary and reported separately.

```rust
pub const CRATE_SUBPATH_PROVENANCE: &[(&str, &str, Provenance, &str)] = _;
```

#### Constant `ASSISTANCE_GROUPS`

Repositories grouped by how much non-agentic AI help each had.

The discussion compares repositories written with no AI help at all against
those that had some non-agentic help from NUS AI-know. Grouped here so the
rates quoted in the prose come out of the same run as the tables.

```rust
pub const ASSISTANCE_GROUPS: &[(&str, &[&str])] = _;
```

#### Constant `MANUSCRIPT`

Values as printed in the manuscript, for the drift check only.

**These are not used in any computation.** They exist so the run can report
drift. The tables themselves are emitted by this code, so they cannot drift
through transcription error any more; what these still catch is the case
that matters — the repositories moving after the manuscript's *prose*
figures, percentages and headline ratio were written against them. Recorded
from the run of 2026-07-23 on `develop`.

```rust
pub const MANUSCRIPT: &[(&str, i64)] = _;
```

#### Constant `CRATE_TEX_NAME`

Crates whose names need something other than a plain `\texttt{}` rendering.

```rust
pub const CRATE_TEX_NAME: &[(&str, &str)] = _;
```

#### Constant `GENERATED_BY`

Header stamped onto every generated LaTeX file.

```rust
pub const GENERATED_BY: &str = "% Generated by kovan kloc -- do not edit by hand.\n\
                                % Re-run the command to update; edits here will be overwritten.\n";
```

#### Constant `MONTHS`

Abbreviated month names, for the table's period column.

```rust
pub const MONTHS: &[&str] = _;
```

## Module `figure`

`fig_kloc_productivity.svg` — the productivity figure, hand-rolled as SVG.

Two stacked horizontal bar charts:

- **(a)** code lines per active day, one bar per pre-agentic repository,
  then the pre-agentic aggregate, then the agentic month.
- **(b)** what the agentic output is made of, one bar per class.

# Why SVG rather than the matplotlib PNG this replaces

Three reasons, in order of weight.

**Determinism.** A plotting library's output moves with its version, its
font metrics and its rasteriser. This code emits the same bytes for the same
numbers on any machine, which is what a reproducibility artifact needs and
what a 200-dpi raster cannot promise.

**No dependency.** No plotting crate is in the workspace's shared
dependencies, and a chart of two bar groups does not justify adding one.

**It suits the consumer.** The figure goes into a LaTeX manuscript, where
vector art scales and prints better than a raster.

**This visibly changes the artifact** in a submitted paper — the maintainer
chose it on 2026-08-14 with that stated. The layout below follows the
matplotlib original closely (bar order, the rule separating the summary rows,
the hatched aggregate bar, value labels outside the bars, no top or right
spine) so the change is of format rather than of content.

# Why one bar per class in (b), rather than one stacked bar

Inherited from the original, and the reasoning still holds: the two small
classes are roughly 15% and 7% of the total, far too narrow to hold a legible
inline label, and leader lines out of adjacent thin segments cross each
other. Separate bars carry the same split with no collision risk, and each
label states its share.

```rust
pub mod figure { /* ... */ }
```

### Functions

#### Function `productivity_svg`

Render the whole figure.

```rust
pub fn productivity_svg(base_stats: &[super::measure::RepoStats], base_totals: &super::measure::BaselineTotals, agentic: &super::measure::AgenticSummary) -> String { /* ... */ }
```

## Module `measure`

Turning repositories into the numbers the paper reports.

Two measurements, and the awkward bit is how they meet:

- [`measure_baseline`] walks the pre-agentic repositories.
- [`measure_agentic`] walks `outram-park-backend`'s crates.

An **extension** crate is a pre-agentic repository vendored into the backend
and then worked on, so only the excess over the standalone original is
agentic. That subtraction is why a missing baseline repository is not merely
an incomplete baseline: its lines move silently into the agentic total,
inflating it by very nearly what the baseline loses. [`BaselineTotals::missing`]
exists so callers can refuse to report a comparison that is not valid.

```rust
pub mod measure { /* ... */ }
```

### Types

#### Struct `RepoStats`

One pre-agentic repository, as measured.

```rust
pub struct RepoStats {
    pub key: String,
    pub label: String,
    pub note: String,
    pub marker: String,
    pub cite: String,
    pub path: Option<std::path::PathBuf>,
    pub lines: super::source::LineCount,
    pub head_lines: super::source::LineCount,
    pub days: std::collections::BTreeSet<String>,
    pub first: String,
    pub last: String,
    pub reference: String,
    pub missing: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `String` | The repository's directory / GitHub name. |
| `label` | `String` | How the manuscript names it, as LaTeX. |
| `note` | `String` | Footnote text for the rate table's "AI?" column. |
| `marker` | `String` | Footnote marker (a, b, c) in the table. |
| `cite` | `String` | Bib key, cited in the table row. |
| `path` | `Option<std::path::PathBuf>` | Absolute path to the checkout, if one was found. |
| `lines` | `super::source::LineCount` | Line counts, **after** any net-of-predecessor adjustment. |
| `head_lines` | `super::source::LineCount` | Line counts as measured, before that adjustment.<br><br>Extensions must subtract the standalone original at its head, not the<br>net figure, so both are kept. |
| `days` | `std::collections::BTreeSet<String>` | Distinct calendar dates carrying a commit. |
| `first` | `String` | Earliest and latest active day, or empty when there are no commits. |
| `last` | `String` | Latest active day. |
| `reference` | `String` | The ref lines were counted at. |
| `missing` | `bool` | No checkout was found for this repository. |

##### Implementations

###### Methods

- ```rust
  pub fn rate(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Code lines per active day, or `None` when no day carried a commit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RepoStats { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> RepoStats { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TuasNet`

The TUAS net-of-predecessor arithmetic, kept for the report and the caption.

```rust
pub struct TuasNet {
    pub head: super::source::LineCount,
    pub imported: super::source::LineCount,
    pub net: super::source::LineCount,
    pub abandoned_code: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `head` | `super::source::LineCount` | TUAS as it stands at its head. |
| `imported` | `super::source::LineCount` | The tree TUAS imported wholesale at its second commit. |
| `net` | `super::source::LineCount` | Head less imported. |
| `abandoned_code` | `u64` | Written in the predecessor and never carried across at spin-out. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> TuasNet { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TuasNet { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BaselineTotals`

Totals across the pre-agentic baseline.

```rust
pub struct BaselineTotals {
    pub lines: super::source::LineCount,
    pub union_days: std::collections::BTreeSet<String>,
    pub first: String,
    pub last: String,
    pub tuas_net: Option<TuasNet>,
    pub missing: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lines` | `super::source::LineCount` | Summed line counts over the repositories that were found. |
| `union_days` | `std::collections::BTreeSet<String>` | Union of active dates — **not** a column sum, because these projects<br>overlapped in time. |
| `first` | `String` | Earliest active day across the baseline. |
| `last` | `String` | Latest active day across the baseline. |
| `tuas_net` | `Option<TuasNet>` | The TUAS adjustment, if both it and its predecessor were present. |
| `missing` | `Vec<String>` | Repositories that could not be found. |

##### Implementations

###### Methods

- ```rust
  pub fn active_days(self: &Self) -> usize { /* ... */ }
  ```
  Number of distinct active days.

- ```rust
  pub fn rate(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Code lines per active day.

- ```rust
  pub fn as_of(self: &Self) -> &str { /* ... */ }
  ```
  The "measured &lt;date&gt;" stamp for the table caption: the newest commit

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BaselineTotals { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> BaselineTotals { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CrateStats`

One crate in the agentic repository, as measured.

```rust
pub struct CrateStats {
    pub name: String,
    pub display: String,
    pub class: super::config::Provenance,
    pub upstream: Option<String>,
    pub total_lines: u64,
    pub code_lines: u64,
    pub raw_code_lines: u64,
    pub baseline_code_lines: u64,
    pub description: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Crate directory name, or the split-out binary's name. |
| `display` | `String` | How the table names it. |
| `class` | `super::config::Provenance` | How its lines are attributed. |
| `upstream` | `Option<String>` | Upstream project, or the pre-agentic repository it extends. |
| `total_lines` | `u64` | Total lines, blank and comment included. |
| `code_lines` | `u64` | Agentic code lines: `raw_code_lines` less any pre-agentic original. |
| `raw_code_lines` | `u64` | Code lines as they stand in the backend. |
| `baseline_code_lines` | `u64` | The standalone pre-agentic original's code lines, if any. |
| `description` | `String` | The crate's own `Cargo.toml` description, carried so the classification<br>can be audited against what the crate says it is. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrateStats { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AgenticSummary`

Totals across the agentic repository.

```rust
pub struct AgenticSummary {
    pub missing: bool,
    pub n_crates: usize,
    pub unclassified: Vec<String>,
    pub stale: Vec<String>,
    pub total_rust_code: u64,
    pub subtotals: std::collections::BTreeMap<String, u64>,
    pub agentic_total: u64,
    pub active_days: usize,
    pub head: String,
    pub reference: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `missing` | `bool` | No checkout was found. |
| `n_crates` | `usize` | Number of crate directories found. |
| `unclassified` | `Vec<String>` | Present in the checkout but absent from the classification table, and<br>therefore excluded from every total. |
| `stale` | `Vec<String>` | Classified but not present in the checkout. |
| `total_rust_code` | `u64` | All Rust code in `crates/`, before any pre-agentic subtraction. |
| `subtotals` | `std::collections::BTreeMap<String, u64>` | Agentic code lines per class. |
| `agentic_total` | `u64` | Sum of `subtotals`. |
| `active_days` | `usize` | Active days inside the reported window. |
| `head` | `String` | Commit date of the measured ref. |
| `reference` | `String` | The ref measured. |

##### Implementations

###### Methods

- ```rust
  pub fn rate(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Agentic code lines per active day.

- ```rust
  pub fn subtotal(self: &Self, class: Provenance) -> u64 { /* ... */ }
  ```
  Subtotal for one class.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AgenticSummary { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> AgenticSummary { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `measure_repo`

Measure one pre-agentic repository.

Lines are counted at `measure_ref` where one is pinned; **active days are
always counted over the branch's whole history**, because a day the author
committed is a day worked whether or not that commit survives to the pinned
tree.

```rust
pub fn measure_repo(spec: &super::config::RepoSpec, path: Option<&std::path::Path>) -> RepoStats { /* ... */ }
```

#### Function `measure_baseline`

Measure the whole pre-agentic baseline, applying the TUAS adjustment.

```rust
pub fn measure_baseline(specs: &[super::config::RepoSpec], paths: &std::collections::BTreeMap<String, std::path::PathBuf>) -> (Vec<RepoStats>, BaselineTotals) { /* ... */ }
```

#### Function `measure_agentic`

Measure the agentic repository's crates.

Every crate is counted out of the **same** tree at the same ref, so no crate
can be measured at a different commit from its neighbours.

```rust
pub fn measure_agentic(path: Option<&std::path::Path>, measure_ref: Option<&str>, baseline: &std::collections::BTreeMap<String, RepoStats>) -> (Vec<CrateStats>, AgenticSummary) { /* ... */ }
```

## Module `outputs`

The machine-readable and manuscript-facing artifacts: two CSVs and three
LaTeX tables.

# Every byte here is compared

These files are gated against `docs/kloc-parity-baseline/`, so nothing about
their shape is discretionary — not the CSV line terminator, not the quoting
rule, not a word of the table captions. Ported from `write_csvs` and the
`*_table_tex` functions of the retired `scripts/kloc_accounting.py`.

# Two details that are easy to get wrong

**CSV line endings are CRLF.** Python's `csv.writer` defaults to the `excel`
dialect, whose terminator is `\r\n`. Writing `\n` produces a file that looks
identical in an editor and fails the byte comparison.

**LaTeX numbers use a thin space, not a comma.** `303\,463`, via
[`tex_num`] — a comma inside a numeric column reads as a decimal separator
to a European reader.

# The one intended difference from the fixture

These files name the tool that produced them, in the `% Generated by`
header and again in each caption's `\emph{...}` note — text that **prints in
the paper**. The fixture says `kloc_accounting.py`; this says `kovan kloc`,
because the script no longer exists and a caption pointing a reader at a
deleted file is worse than a failed byte comparison.

So the parity check on the three `.tex` files is: **identical except the
lines naming the tool.** Every number, row, subtotal and caption sentence is
byte-for-byte. Verified 2026-08-14 — the only diff hunks were those lines.

```rust
pub mod outputs { /* ... */ }
```

### Functions

#### Function `baseline_csv`

Render `baseline_repositories.csv`.

```rust
pub fn baseline_csv(base_stats: &[super::measure::RepoStats]) -> String { /* ... */ }
```

#### Function `agentic_csv`

Render `agentic_crates.csv`.

```rust
pub fn agentic_csv(crate_rows: &[super::measure::CrateStats]) -> String { /* ... */ }
```

#### Function `tex_num`

Format an integer with LaTeX thin-space thousands separators: `303\,463`.

```rust
pub fn tex_num(n: u64) -> String { /* ... */ }
```

#### Function `tex_name`

How a crate is named in a table cell.

```rust
pub fn tex_name(crate_name: &str) -> String { /* ... */ }
```

#### Function `tex_month`

`2023-06-27` becomes `Jun 2023`; an empty date becomes `---`.

```rust
pub fn tex_month(date: &str) -> String { /* ... */ }
```

#### Function `baseline_table_tex`

Render `baseline_table.tex` — `tab:preagentic_baseline`.

```rust
pub fn baseline_table_tex(base_stats: &[super::measure::RepoStats], base_totals: &super::measure::BaselineTotals) -> String { /* ... */ }
```

#### Function `rate_table_tex`

Render `rate_table.tex` — the per-repository rate table, fastest first.

```rust
pub fn rate_table_tex(base_stats: &[super::measure::RepoStats], base_totals: &super::measure::BaselineTotals, agentic: &super::measure::AgenticSummary) -> String { /* ... */ }
```

#### Function `agentic_table_tex`

Render `agentic_table.tex` — `tab:agentic_crates`.

```rust
pub fn agentic_table_tex(crate_rows: &[super::measure::CrateStats], agentic: &super::measure::AgenticSummary) -> String { /* ... */ }
```

## Module `repo`

Reading committed state out of a git repository, without touching its
working directory.

# Why committed state, and why not a checkout

The measurement has to be reproducible by someone who is not the author, so
it reads a **named ref**, never a working tree — a repository the author has
open may carry uncommitted work, and counting that would make the figure
unreproducible the moment they saved a file.

The Python this ports from ran `git archive` into a temporary directory and
walked that. This reads the tree directly instead, with `git ls-tree` for
the file list and a single `git cat-file --batch` for the contents. Two
processes per repository, no temporary directory, no tar dependency, and
nothing written to disk.

# Which ref gets measured, and why it matters

These repositories **squash-merge into `main`**, so `main` carries a handful
of release commits while the development history — and therefore every
active-day count — lives on `develop`. Measuring `main` understates
`thermal_hydraulics_rs` by 135 active days. [`PREFERRED_REFS`] encodes that
preference order.

```rust
pub mod repo { /* ... */ }
```

### Functions

#### Function `git`

Run git in `repo` and return stdout, or an empty string if it fails.

Failure is deliberately quiet and empty rather than an error: every caller
here treats "no output" as "nothing to count", which is the correct reading
for a missing ref or a directory that is not a repository.

```rust
pub fn git(repo: &std::path::Path, args: &[&str]) -> String { /* ... */ }
```

#### Function `ref_exists`

Does `reference` resolve to a commit in `repo`?

```rust
pub fn ref_exists(repo: &std::path::Path, reference: &str) -> bool { /* ... */ }
```

#### Function `select_ref`

The first of [`PREFERRED_REFS`] that resolves in this checkout.

```rust
pub fn select_ref(repo: &std::path::Path) -> String { /* ... */ }
```

#### Function `active_days`

Calendar dates (`YYYY-MM-DD`) carrying at least one commit on `reference`.

A [`BTreeSet`] so the result is ordered and set operations across
repositories are deterministic. Counting **distinct dates** rather than
commits is the point: a day the author committed is a day worked, whether
that day carried one commit or thirty.

```rust
pub fn active_days(repo: &std::path::Path, reference: &str, since: Option<&str>, until: Option<&str>) -> std::collections::BTreeSet<String> { /* ... */ }
```

#### Function `head_date`

Commit date of `reference`, as `YYYY-MM-DD`.

```rust
pub fn head_date(repo: &std::path::Path, reference: &str) -> String { /* ... */ }
```

#### Function `rust_files_at`

Repository-relative paths of every Rust source file in the tree at
`reference`, excluding [`SKIP_DIRS`](super::source::SKIP_DIRS).

`prefix` restricts the listing to a subtree (e.g. `crates/tampines`); pass
an empty string for the whole tree.

```rust
pub fn rust_files_at(repo: &std::path::Path, reference: &str, prefix: &str) -> Vec<String> { /* ... */ }
```

#### Function `count_tree`

Count the Rust source in the tree at `reference`, under `prefix`, excluding
any path under `exclude`.

`exclude` holds paths **relative to `prefix`**, matching how a crate's
differently-classified subtree is named.

# How the contents are read

One `git cat-file --batch` process for the whole listing. Requests are
written from a separate thread while this one reads responses — writing them
all first would fill the pipe buffer and deadlock against a backed-up stdout
on any repository of real size.

```rust
pub fn count_tree(repo: &std::path::Path, reference: &str, prefix: &str, exclude: &std::collections::BTreeSet<String>) -> super::source::LineCount { /* ... */ }
```

#### Function `read_blob`

Read one blob out of the tree at `reference`, or `None` if it is absent.

Decoded lossily: a manifest need not be valid UTF-8, and a replacement
character in a description is preferable to dropping the crate.

```rust
pub fn read_blob(repo: &std::path::Path, reference: &str, path: &str) -> Option<String> { /* ... */ }
```

#### Function `list_dir`

Immediate entry names under `path` in the tree at `reference`.

```rust
pub fn list_dir(repo: &std::path::Path, reference: &str, path: &str) -> Vec<String> { /* ... */ }
```

### Constants and Statics

#### Constant `PREFERRED_REFS`

Which ref to measure, in order of preference.

`develop` first: see the module docs on squash-merging. `HEAD` last, as a
fallback for a repository following none of these conventions.

```rust
pub const PREFERRED_REFS: &[&str] = _;
```

## Module `report`

The console report, also written to `summary.txt`.

# Byte-for-byte parity is the requirement here

This text is one of the artifacts gated by `docs/kloc-parity-baseline/`, so
the column widths, rule lengths, comma grouping and wording are **not** free
to improve. A tidier table that differs by one space is a failed port. Every
`{:<46}` and `"=".repeat(78)` below is load-bearing.

Ported from `report()` in the retired `scripts/kloc_accounting.py`.

```rust
pub mod report { /* ... */ }
```

### Functions

#### Function `fmt`

Format an integer with comma thousands separators, matching Python's
`f"{n:,}"`.

```rust
pub fn fmt(n: u64) -> String { /* ... */ }
```

#### Function `report`

Render the full report.

`check` adds the drift comparison against the manuscript's published
figures.

```rust
pub fn report(base_stats: &[super::measure::RepoStats], base_totals: &super::measure::BaselineTotals, crate_rows: &[super::measure::CrateStats], agentic: &super::measure::AgenticSummary, check: bool) -> String { /* ... */ }
```

## Module `source`

Counting lines of Rust source the way the productivity accounting defines
them: **code lines exclude blank and comment-only lines**.

# Why a comment stripper rather than a line-prefix test

"Does this line start with `//`?" is wrong in both directions. It misses the
body of a block comment, and it fires on a `//` that is inside a string
literal. Since Rust doc comments carry executable doctests, a project that
documents heavily would have that work counted or discarded almost at
random depending on which mistake dominated.

So the source is stripped properly — nested block comments, line comments,
ordinary and raw strings, byte strings and character literals — and a line
counts as code if anything survives. **Newlines inside removed comments are
preserved**, so the stripped text stays line-aligned with the original and
the blank/comment-only test lands on the right lines.

# The lifetime problem

`'a` in `&'a str` opens no character literal, and a naive `'`-scanner
swallows the rest of the file looking for a closing quote. A character
literal is therefore matched as a *shape* — `'x'`, `'\n'`, `'\x41'`,
`'\u{1F600}'` — and anything not matching that shape is treated as ordinary
code, which is exactly what a lifetime is.

# Parity

This is a direct port of `strip_rust_comments` from the retired
`scripts/kloc_accounting.py`, and its output must match that script's
byte-for-byte on the same inputs — see `docs/kloc-parity-baseline/`. Do not
"improve" the classification; a better stripper that disagrees with the
published figures is a regression here.

```rust
pub mod source { /* ... */ }
```

### Types

#### Struct `LineCount`

Line counts for one body of Rust source.

```rust
pub struct LineCount {
    pub total: u64,
    pub code: u64,
    pub files: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `u64` | Every line, blank and comment-only included. Rust doc comments carry<br>executable doctests, so they appear here and not in `code`. |
| `code` | `u64` | Lines with something left after comments are stripped. |
| `files` | `u64` | Number of `.rs` files counted. |

##### Implementations

###### Methods

- ```rust
  pub fn add_file(self: &mut Self, contents: &str) { /* ... */ }
  ```
  Accumulate one file's contents.

- ```rust
  pub fn merge(self: Self, other: Self) -> Self { /* ... */ }
  ```
  Sum of two counts.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LineCount { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> LineCount { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LineCount) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `is_skipped`

Whether a repository-relative path lies inside a skipped directory.

Applied to paths listed out of a git tree, so it takes the *path* rather
than walking a filesystem: the measurement reads committed state, never a
working directory.

```rust
pub fn is_skipped(path: &str) -> bool { /* ... */ }
```

#### Function `is_rust_source`

Whether a repository-relative path is Rust source to be counted.

```rust
pub fn is_rust_source(path: &str) -> bool { /* ... */ }
```

#### Function `strip_rust_comments`

Remove Rust comments from `src`, preserving line structure.

Handles nested block comments, line comments, ordinary and raw strings,
byte strings and character literals. Newlines inside removed comments are
kept, so line numbering — and therefore the caller's blank/comment-only
test — stays aligned with the original file.

```rust
pub fn strip_rust_comments(src: &str) -> String { /* ... */ }
```

#### Function `under_any`

Whether `path` lies under any of `roots` (repository-relative, `/`-joined).

Used to exclude a subtree that is classified differently from the crate
containing it, so nothing is counted twice.

```rust
pub fn under_any(path: &str, roots: &std::collections::BTreeSet<String>) -> bool { /* ... */ }
```

#### Function `is_rust_source_path`

Convenience for callers holding a filesystem path rather than a git path.

```rust
pub fn is_rust_source_path(path: &std::path::Path) -> bool { /* ... */ }
```

### Constants and Statics

#### Constant `SKIP_DIRS`

Directories never descended into when counting source.

Build output, dependency caches and vendored third-party trees are not the
work being measured. `.git` is excluded because a packfile is not source.

```rust
pub const SKIP_DIRS: &[&str] = _;
```

### Types

#### Struct `Options`

How a run is configured.

```rust
pub struct Options {
    pub out_dir: std::path::PathBuf,
    pub search_dirs: Vec<std::path::PathBuf>,
    pub vendor_dir: std::path::PathBuf,
    pub github_only: bool,
    pub check: bool,
    pub figure: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `out_dir` | `std::path::PathBuf` | Where the CSVs, LaTeX tables, summary and figure are written. |
| `search_dirs` | `Vec<std::path::PathBuf>` | Directories searched for existing checkouts, in order. |
| `vendor_dir` | `std::path::PathBuf` | Where cloned repositories live, and the last place searched. |
| `github_only` | `bool` | Ignore local checkouts entirely and measure only the vendor clones.<br><br>This is the **reproduction path**: it needs nothing on the machine but<br>git and network access, so a reader's run cannot accidentally pick up<br>the author's working copies. |
| `check` | `bool` | Add the drift comparison against the manuscript's published figures. |
| `figure` | `bool` | Emit the SVG figure. |

##### Implementations

###### Methods

- ```rust
  pub fn new(out_dir: PathBuf) -> Self { /* ... */ }
  ```
  Defaults writing into `out_dir`, searching the author's usual locations.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Options { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Outcome`

What a run produced.

```rust
pub struct Outcome {
    pub report: String,
    pub written: Vec<std::path::PathBuf>,
    pub missing: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `report` | `String` | The console report, also written to `summary.txt`. |
| `written` | `Vec<std::path::PathBuf>` | Files written, in the order they were written. |
| `missing` | `Vec<String>` | Repositories that could not be found. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Outcome { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `repo_url`

The clone URL for a repository key.

```rust
pub fn repo_url(key: &str) -> String { /* ... */ }
```

#### Function `find_repo`

Locate a checkout of `key`, or `None`.

Searches `search_dirs` in order, then `vendor_dir`. With `github_only`, only
the vendor directory is consulted.

```rust
pub fn find_repo(options: &Options, key: &str) -> Option<std::path::PathBuf> { /* ... */ }
```

#### Function `all_repo_keys`

Every repository the accounting needs, baseline and agentic.

```rust
pub fn all_repo_keys() -> Vec<&'static str> { /* ... */ }
```

#### Function `resolve`

Resolve every repository to a path, where one can be found.

```rust
pub fn resolve(options: &Options) -> std::collections::BTreeMap<String, std::path::PathBuf> { /* ... */ }
```

#### Function `run`

Measure, render, and write every artifact.

```rust
pub fn run(options: &Options) -> io::Result<Outcome> { /* ... */ }
```

### Constants and Statics

#### Constant `REPO_SEARCH_SUBDIRS`

Where to look for an already-present checkout before falling back to the
vendor directory.

Relative to the user's home directory. A checkout the author already has is
preferred over a fresh clone because it is what they are actually working
in — and it is never written to.

```rust
pub const REPO_SEARCH_SUBDIRS: &[&str] = _;
```

### Re-exports

#### Re-export `Provenance`

```rust
pub use config::Provenance;
```

#### Re-export `AGENTIC_KEY`

```rust
pub use config::AGENTIC_KEY;
```

#### Re-export `AGENTIC_MEASURE_REF`

```rust
pub use config::AGENTIC_MEASURE_REF;
```

#### Re-export `BASELINE_REPOS`

```rust
pub use config::BASELINE_REPOS;
```

#### Re-export `GITHUB_USER`

```rust
pub use config::GITHUB_USER;
```

#### Re-export `measure_agentic`

```rust
pub use measure::measure_agentic;
```

#### Re-export `measure_baseline`

```rust
pub use measure::measure_baseline;
```

#### Re-export `AgenticSummary`

```rust
pub use measure::AgenticSummary;
```

#### Re-export `BaselineTotals`

```rust
pub use measure::BaselineTotals;
```

#### Re-export `CrateStats`

```rust
pub use measure::CrateStats;
```

#### Re-export `RepoStats`

```rust
pub use measure::RepoStats;
```

#### Re-export `report`

```rust
pub use report::report;
```

#### Re-export `strip_rust_comments`

```rust
pub use source::strip_rust_comments;
```

#### Re-export `LineCount`

```rust
pub use source::LineCount;
```

#### Re-export `SKIP_DIRS`

```rust
pub use source::SKIP_DIRS;
```

## Module `tokens`

Token accounting: the write side (git hooks) and the query side (history).

# Write side — driven by the git hooks

- [`stamp_trailer`] (`prepare-commit-msg`) appends the `API-Usage-*` trailers
  to a commit message. **Idempotent**: a message that already carries the key
  is left untouched, so amend and rebase are safe.
- [`record`] (`post-commit`) advances the baseline and regenerates the ledger.
- [`report`] regenerates `docs/token-usage.md` from the commit trailers.
- [`init`] stamps the baseline (used by the installer).
- [`show`] prints the live cumulative reading.

# Query side — reads the durable git record

[`query`] sums the usage **recorded in commit trailers** over a date window
on any branch. It never reads the live transcripts, so it works for any
historical window, on any branch, from any clone.

# The non-blocking contract

Every function here returns `()` or a value and swallows its own errors. A
commit must never fail because token accounting failed — the worst
acceptable outcome is a missing or zero trailer. Callers in the hook path
must preserve this.

```rust
pub mod tokens { /* ... */ }
```

### Types

#### Struct `QueryRow`

One commit's contribution to a [`query`] result.

```rust
pub struct QueryRow {
    pub date: String,
    pub commit: String,
    pub subject: String,
    pub total: Option<u64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `date` | `String` | Author date, `YYYY-MM-DD`. |
| `commit` | `String` | Abbreviated commit hash. |
| `subject` | `String` | Commit subject. |
| `total` | `Option<u64>` | Recorded total, or `None` when the commit carries no usable trailer. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> QueryRow { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `QueryResult`

The outcome of a [`query`] over a window of history.

```rust
pub struct QueryResult {
    pub branch: String,
    pub from: Option<crate::date::Date>,
    pub to: Option<crate::date::Date>,
    pub commits_total: usize,
    pub commits_with_data: usize,
    pub totals: crate::trailer::TokenCounts,
    pub grand_total: u64,
    pub rows: Vec<QueryRow>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `branch` | `String` | The ref actually reported on (e.g. `origin/develop`). |
| `from` | `Option<crate::date::Date>` | Window start, if bounded. |
| `to` | `Option<crate::date::Date>` | Window end, if bounded. |
| `commits_total` | `usize` | Non-merge commits in the window. |
| `commits_with_data` | `usize` | How many of those carried real token data. |
| `totals` | `crate::trailer::TokenCounts` | Summed components. |
| `grand_total` | `u64` | Summed `total=` fields as recorded. |
| `rows` | `Vec<QueryRow>` | Per-commit rows, oldest first. |

##### Implementations

###### Methods

- ```rust
  pub fn to_json(self: &Self, per_commit: bool) -> String { /* ... */ }
  ```
  Render as JSON, optionally including the per-commit breakdown.

- ```rust
  pub fn print(self: &Self, per_commit: bool) { /* ... */ }
  ```
  Print the human-facing summary.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> QueryResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `stamp_trailer`

Append the `API-Usage-*` trailers to the commit message at `msgfile`.

Does nothing when the message already carries the trailer key (amend and
rebase safety), when the file cannot be read, or when it cannot be written.

On the very first commit in a clone there is no baseline to subtract, so the
delta is stamped as zero and the source is suffixed `:baseline-initialised`
— an honest "we started measuring here" rather than attributing the whole
transcript history to one commit.

```rust
pub fn stamp_trailer(msgfile: &std::path::Path) { /* ... */ }
```

#### Function `record`

Advance the baseline to the current cumulative, then regenerate the ledger.

```rust
pub fn record() { /* ... */ }
```

#### Function `report`

Regenerate `docs/token-usage.md` from the commit trailers.

Writing failures are swallowed — this runs from `post-commit`, after the
commit already exists.

```rust
pub fn report() { /* ... */ }
```

#### Function `init`

Stamp the baseline at the current cumulative reading (installer entry point).

```rust
pub fn init() { /* ... */ }
```

#### Function `show`

Print the live cumulative reading and the delta since the last commit.

```rust
pub fn show() { /* ... */ }
```

#### Function `query`

Sum the token usage recorded in commit trailers over a date window.

Reads the **durable git record**, not the live transcripts, so it is valid
for any window on any branch. Commits with no trailer, or with
`source=none`, contribute zero and are reported as having no data — which is
correct for commits made before the hooks existed or outside a Claude
session.

```rust
pub fn query(from: Option<crate::date::Date>, to: Option<crate::date::Date>, branch: &str) -> QueryResult { /* ... */ }
```

### Constants and Statics

#### Constant `LEDGER_REL`

The generated ledger's path, relative to the repository root.

**Generated and gitignored** — regenerable from the commit trailers at any
time, deliberately not tracked (committing it on many branches caused
recurring merge conflicts). Never `git add` this file.

```rust
pub const LEDGER_REL: &str = "docs/token-usage.md";
```

## Module `trailer`

The `API-Usage-*` commit trailers — the workspace's durable token record.

Every commit carries its own accounting in its message:

```text
API-Usage-Since-Last-Commit: total=1234 in=10 out=20 cache_read=1200 cache_write=4 source=session-transcript
API-Usage-Session-Cumulative: total=98765
```

The **trailer is the source of truth**, not the generated
`docs/token-usage.md` ledger (which is gitignored and regenerable). Anything
that reports usage over a window reads these lines back out of git.

**Honesty rule.** `total = in + out + cache_read + cache_write`. A commit made
outside a Claude session legitimately reads `total=0 source=none`; that is a
correct measurement of zero, not missing data, and must never be replaced
with an estimate.

```rust
pub mod trailer { /* ... */ }
```

### Types

#### Struct `TokenCounts`

The four token components the API bills separately.

Kept as a struct rather than a map so a missing component is a compile
error rather than a silent zero. Cache-read normally dominates by an order
of magnitude and is always reported separately — never fold it into a
single headline number.

```rust
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `u64` | Uncached input tokens. |
| `output` | `u64` | Generated output tokens — the closest proxy for net produced content. |
| `cache_read` | `u64` | Tokens read back from the prompt cache. |
| `cache_write` | `u64` | Tokens written into the prompt cache. |

##### Implementations

###### Methods

- ```rust
  pub fn total(self: &Self) -> u64 { /* ... */ }
  ```
  `input + output + cache_read + cache_write`.

- ```rust
  pub fn add(self: &mut Self, other: &TokenCounts) { /* ... */ }
  ```
  Component-wise sum.

- ```rust
  pub fn saturating_sub(self: &Self, baseline: &TokenCounts) -> TokenCounts { /* ... */ }
  ```
  Component-wise `self - baseline`, clamped at zero.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> TokenCounts { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TokenCounts { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TokenCounts) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ParsedTrailer`

A parsed `API-Usage-Since-Last-Commit` line.

```rust
pub struct ParsedTrailer {
    pub counts: TokenCounts,
    pub total: u64,
    pub source: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `counts` | `TokenCounts` | The four components as recorded. |
| `total` | `u64` | The `total=` field as recorded, which is trusted over the recomputed sum<br>so a report reproduces exactly what the commit claimed. |
| `source` | `String` | The `source=` field, e.g. `session-transcript` or `none`. |

##### Implementations

###### Methods

- ```rust
  pub fn has_data(self: &Self) -> bool { /* ... */ }
  ```
  Whether this trailer carries real measured usage.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ParsedTrailer { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ParsedTrailer) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `parse`

Extract the `API-Usage-Since-Last-Commit` trailer from a commit message
body, or `None` when the commit carries no trailer.

Scans line by line for the key (the Python used a `MULTILINE` regex; this is
the same match without the dependency). Unparseable numeric fields read as
`0` rather than failing the whole record — a mangled trailer should degrade
one row, not abort a report.

```rust
pub fn parse(body: &str) -> Option<ParsedTrailer> { /* ... */ }
```

#### Function `format`

Render the two trailer lines appended to a commit message.

```rust
pub fn format(delta: &TokenCounts, cumulative: &TokenCounts, source: &str) -> String { /* ... */ }
```

#### Function `group`

Format an integer with thousands separators, e.g. `1234567` -> `1,234,567`.

```rust
pub fn group(n: u64) -> String { /* ... */ }
```

### Constants and Statics

#### Constant `TRAILER_KEY`

Trailer key for the per-commit delta.

```rust
pub const TRAILER_KEY: &str = "API-Usage-Since-Last-Commit";
```

#### Constant `CUMULATIVE_KEY`

Trailer key for the running session total.

```rust
pub const CUMULATIVE_KEY: &str = "API-Usage-Session-Cumulative";
```

## Module `transcript`

Reading token usage out of the Claude Code session transcripts.

Claude Code writes one JSONL file per session under
`~/.claude/projects/<slug>/`, where `<slug>` is the project's absolute path
with every run of non-alphanumeric characters replaced by `-`. Each line is
a JSON object; the ones that carry billing information have a
`message.usage` object with the four token counters.

This is the same data `ccusage` reads. **Nothing here is estimated** — if no
transcript directory is found, the result is a hard zero with
[`Source::None`], which is a correct measurement, not a failure.

```rust
pub mod transcript { /* ... */ }
```

### Types

#### Enum `Source`

Where a cumulative reading came from.

```rust
pub enum Source {
    SessionTranscript,
    None,
}
```

##### Variants

###### `SessionTranscript`

Read from at least one session transcript.

###### `None`

No transcript directory or no `.jsonl` files — a true zero.

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: &Self) -> &'static str { /* ... */ }
  ```
  The string written into the trailer's `source=` field.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Source { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Source) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Cumulative`

A cumulative reading across every transcript for this project.

```rust
pub struct Cumulative {
    pub counts: crate::trailer::TokenCounts,
    pub records: u64,
    pub source: Source,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `counts` | `crate::trailer::TokenCounts` | Summed token counts. |
| `records` | `u64` | How many JSONL records carried a `message.usage` object. |
| `source` | `Source` | Provenance of the reading. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Cumulative { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Cumulative) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `slug_for_path`

Slugify an absolute project path the way Claude Code names its transcript
directories: **each** non-alphanumeric character becomes one `-`.

```
# use kovan_metrics::transcript::slug_for_path;
assert_eq!(slug_for_path("/home/me/proj"), "-home-me-proj");
// Windows: the drive colon and the separator are two characters, so two
// dashes — this is the case a run-collapsing slug gets wrong.
assert_eq!(slug_for_path("C:/Users/me/proj"), "C--Users-me-proj");
```

**This deliberately differs from the Python it replaces**, whose
`re.sub(r"[^A-Za-z0-9]+", "-", path)` collapsed runs and therefore computed
`C-Users-…` on Windows — a directory that does not exist. The Python only
ever worked via its basename fallback, which is itself ambiguous whenever a
nested project shares the repository's name (as
`…-outram-park-backend-crates-outram-park-digital-twin-engine` does here),
leaving it with no transcript directory at all. Verified against the real
`~/.claude/projects` layout on 2026-08-13.

```rust
pub fn slug_for_path(path: &str) -> String { /* ... */ }
```

#### Function `project_transcript_dir`

Locate this project's transcript directory, or `None`.

Resolution order, matching the Python it replaces:
1. `~/.claude/projects/<slug>` for the slug of `CLAUDE_PROJECT_DIR` (or the
   repo root when that variable is unset);
2. failing that, a directory under `~/.claude/projects` whose name contains
   the project directory's basename — but **only if exactly one matches**,
   since an ambiguous match could attribute another project's tokens here.

```rust
pub fn project_transcript_dir(repo_root: &std::path::Path) -> Option<std::path::PathBuf> { /* ... */ }
```

#### Function `read_cumulative`

Sum `message.usage` across every transcript line for this project.

Malformed lines, unreadable files and records without usage are skipped
silently — a corrupt transcript must not block a commit.

```rust
pub fn read_cumulative(repo_root: &std::path::Path) -> Cumulative { /* ... */ }
```

