# CLAUDE.md — `kaki-bukit` (KAKI BUKIT)

**KAKI BUKIT** — **K**ernel for **A**gent-based **K**ey-resource **I**nventory,
**B**ackend for **U**tility, **K**ey **I**nfrastructure and **T**ransactions.

Renamed from the provisional `outram-park-fork-cyclus` on 2026-09-18 by the
maintainer, when the porting branch was merged. The old name appears nowhere in
the crate except vendored `upstream_source/`; if you find it elsewhere it is a
stale reference and should be corrected.

Crate-specific guidance. The workspace `CLAUDE.md` at the repository root
governs everything not stated here; this file never relaxes it.

## What this crate is

An independent `no_std` Rust translation of **CYCLUS** (the agent-based
fuel-cycle simulation kernel and its dynamic resource exchange) and
**CYCAMORE** (its facility-agent library). Both upstreams are BSD-3-Clause;
this crate is GPL-3.0-only and the relicensing is **one-way**.

Tracked as epic **`op-r830`**.

## The naming is provisional

The maintainer deferred the naming decision. `kaki-bukit` follows
the workspace `outram-park-fork-<project>` convention as a placeholder. If it
is renamed, the package name, the directory, the root `Cargo.toml` member
entry, the `docs/` mirror filename and every `kaki_bukit::` path
in doc examples all move together — the compiler catches the last of those, so
do the rename with `cargo check` as the reference checker rather than a blind
`sed`.

## Hard rules for this crate

### It is `no_std`, unconditionally, and there is no escape hatch

`#![no_std]` with `extern crate alloc`. There is no `std` feature that turns
`std` on for the library — the `std` feature exists only to add
`impl std::error::Error for CyclusError` for downstream convenience, and the
library must keep compiling and passing without it.

The practical traps, all of which have bitten already:

- **`f64::abs`, `f64::exp`, `f64::ln`, `f64::sqrt`, `f64::sin` are `std`-only
  inherent methods.** They will not compile. Use `crate::limits::abs` and
  `petir::real::*`.
- `std::collections::HashMap` does not exist. Use
  `alloc::collections::BTreeMap`, which is also what you want for a different
  reason — see below.
- `format!`, `String` and `Vec` come from `alloc`, not the prelude.

Verify with the targets that have no `std` at all:

```bash
cargo build --release -p kaki-bukit --target thumbv7em-none-eabihf
cargo build --release -p kaki-bukit --target wasm32-unknown-unknown
```

### All numerics come from PETIR. No exceptions.

`petir` is the only dependency and that is the design, not an accident. Do not
add `libm` directly, do not reimplement a kernel inline, and do not reach for
a third-party numerics crate. If PETIR lacks a routine, **the deliverable is
a routine added to PETIR**, not a local copy — same reasoning as the workspace
rule about KOVAN.

PETIR is pulled with `default-features = false`, which drops its `transfer-fn`
module and with it `uom`. **Do not turn that back on.** Cyclus's kernel is
dimensionless bookkeeping by upstream's own convention — `Material::units()`
returns the literal string `"kg"` — so there is nothing here for `uom` to
type. Units are documented in prose on every public item instead, which the
workspace's human-interface rule requires regardless.

### `BTreeMap`, never a hash map

Compositions are keyed on nuclide and summed constantly. A hash map's
iteration order would make `comp_math::sum` differ in the low-order bits
between runs, which destroys the cross-platform reproducibility that choosing
PETIR bought. `BTreeMap` also reproduces `std::map`'s ordered iteration, so the
port matches upstream's traversal order exactly.

### The C++-to-Rust substitutions are uniform — follow them

| Upstream C++ | Here |
|---|---|
| `virtual` dispatch over `Agent*` | `AgentKind` enum, matched at each call |
| `boost::shared_ptr<ExchangeNode>` | `NodeId`, an index into an arena |
| `Resource::Ptr` | `Resource`, an owned enum |
| thrown exceptions | `Result<T>` |
| `boost::math::float_distance` | `limits::float_distance` |
| a `shared_ptr<Converter>` functor | a `Converter` enum |

No `dyn`, no `Box<T>`, no lifetime parameters. When a new archetype needs
adding, add an enum variant and let the compiler find every `match` that must
handle it.

### `CyclusError` variants carry `&'static str`, not `String`

An allocation on the error path is the one a constrained target can least
afford. Where a *value* identifies the fault, add a field
(`InvalidNuclide(i32)`) rather than formatting it into a message.

## This crate ships NO nuclear data, and must not start

The workspace rule is that all nuclear data lives in `njoy-outram-park-fork`.
Two places where upstream embeds data are therefore **explicit parameters**
here:

- **Atomic masses.** Upstream calls `pyne::atomic_mass(nuc)` against a
  compiled-in table. Here every atom↔mass conversion takes an
  `AtomicMasses` argument. `AtomicMasses::MassNumber` is the zero-data
  approximation (error is the mass defect: under 1 % everywhere, under 0.1 %
  for the actinides) and is what tests use. It is **not** adequate for a mass
  balance anyone will quote.
- **Decay chains.** Upstream's `Decayer` carries a bundled chain table.
  `decay` here takes a caller-supplied `DecayChain`.

Do not "improve" either by vendoring a table into this crate.

## Read upstream before changing anything

This is a translation, so the workspace's "Debugging a port: read upstream
first" rule applies with full force: when something here misbehaves, the
overwhelmingly likely cause is that upstream does something this port does
not. Upstream is at `upstream_source/cyclus` and
`upstream_source/cycamore` (gitignored; re-clone with the commands in
`upstream_source/README.md`).

Three upstream subtleties are load-bearing and easy to lose in a refactor.
Each is called out in the code where it lives, and none of them may be
"simplified":

1. **The unlimited-capacity sentinel is compared by exact float equality.**
   Upstream tests group capacities against `std::numeric_limits<double>::max()`
   with `==`. That is normally a defect; here it is a deliberate sentinel
   protocol. `limits::UNLIMITED` is that value.
2. **Exclusive-order matching compares in ULPs, not with an epsilon.**
   Upstream's own comment says the careful float comparison "is vital for
   preventing false positive constraint violations w.r.t. exclusivity-related
   capacity". `limits::float_distance` plus `limits::FLOAT_ULP_EQ` is that
   comparison.
3. **The greedy solver's sorts are `std::stable_sort`.** Use Rust's stable
   `sort_by`, never `sort_unstable_by`, and compare floats with `total_cmp`
   rather than `partial_cmp().unwrap()`.

A fourth, in `comp_math::almost_eq`, is a *preserved upstream quirk* rather
than a correct behaviour — see that function's docs. Do not silently fix it;
if it should change, raise it upstream first.

## Maturity

**This crate is NOT declared mature.** The workspace's dogfooding rule
therefore does not apply to it yet, and it has no maturity bar. It is an
AI-assisted draft with no human V&V.

What exists is verification against closed forms — the analytic exponential
and two-species Bateman solution for decay, the closed-form separative-work
expression for enrichment, hand-worked matching cases for the exchange solver,
and exact mass conservation for the resource operations. Each test states its
methodology and its measured numbers in its doc comment, per the workspace
V&V-documentation rule.

**The missing leg is cross-code comparison.** There is no comparison against
an actual Cyclus run. Building upstream Cyclus and diffing a simple
simulation's trade schedule against this port is the highest-value next step,
and is exactly how `petir` and `outram-park-fork-liggghts` earned their
evidence: build the upstream, run it, commit its output under
`reference-data/`, and regenerate the comparison rather than trusting it.

Proposing maturity is allowed; **declaring it is the maintainer's call alone.**

## Testing notes

No test in this crate is long-running — everything is arithmetic on small
maps, and the whole suite runs in well under a second. The workspace's
`long-tests` gating rule therefore has nothing to gate here. If a future
cross-code comparison against a built Cyclus crosses five minutes, gate it
then, on a measured time and not a guessed one.

## Regenerating the API mirror on this container

`docs/kaki-bukit-api.md` is the committed markdown mirror of the
public API. The workspace's normal command is:

```bash
kovan-cli api-docs kaki-bukit
```

**That does not work in the Claude-Code-on-the-web container as of
2026-09-17, for a reason that has nothing to do with this crate.** The
container ships rustc 1.94.1; the workspace's `egui`/`eframe` 0.36.1 require
rustc 1.95, so `kovan` cannot be built here at all — with or without
`--no-default-features`, because cargo resolves the workspace lockfile's MSRV
either way. The same mismatch makes a bare `cargo check --workspace` fail;
excluding `outram-park-digital-twin-engine` and `kovan` makes it pass.

Both prerequisites the workspace rule names **are** installed (checked, not
assumed: `which rustdoc-md` and `rustup toolchain list`), so the mirror is
generated through the two commands `kovan-cli api-docs` wraps:

```bash
RUSTDOCFLAGS="-Z unstable-options --output-format json" \
  cargo +nightly doc -p kaki-bukit --no-deps --lib
rustdoc-md --path target/doc/kaki_bukit.json \
  --output crates/kaki-bukit/docs/kaki-bukit-api.md
```

Prefer `kovan-cli api-docs` wherever it builds. Use the above only as the
fallback, and regenerate the mirror whenever a public doc comment changes.

**Keep the intra-doc links resolving.** The rustdoc run above must report zero
`unresolved link` warnings. The workspace's human-interface rule is that a
developer can navigate this API with rust-analyzer alone, and a broken
`[`Type`]` link is exactly that promise failing.
