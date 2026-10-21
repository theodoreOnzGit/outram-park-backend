# `roots` 0.0.8 reference output

Code-to-code reference data for PETIR's [`poly::companion`] port, produced by
**compiling and running the upstream crate**, not by transcription.

This is the same discipline as `reference-data/gsl/`: the comparison
regenerates rather than being trusted, and the driver is committed beside its
output so anyone can reproduce it.

## Upstream

| | |
|---|---|
| crate | [`roots`](https://github.com/vorot/roots) 0.0.8 |
| author | Mikhail Vorotilov |
| licence | BSD-2-Clause |
| source | crates.io, read 2026-09-15 |

The routine being referenced, `find_roots_eigen`, has its own longer
provenance — Martin and Wilkinson's Algol `hqr2`, EISPACK, JAMA's
`EigenvalueDecomposition.java`, hand-transpiled to Rust by Stepan Yakovenko
and contributed to `roots` by Vorotilov. The full chain, with attribution, is
in the header of `crates/petir/src/poly/companion.rs`.

## Regenerating

`roots_reference_driver.rs` is a standalone `main.rs`. It is **not** a member
of this workspace and must not become one — PETIR consumes `roots` as a ported
source, never as a dependency.

```bash
mkdir -p /tmp/rootsref/src && cd /tmp/rootsref
cat > Cargo.toml <<'TOML'
[package]
name = "rootsref"
version = "0.1.0"
edition = "2021"
[dependencies]
roots = "=0.0.8"
TOML
cp <this directory>/roots_reference_driver.rs src/main.rs
cargo run --release > roots-0.0.8-reference.txt
```

## Format

Three lines per case, after a `case <name>` header:

| line | meaning |
|---|---|
| `poly` | coefficients, **descending**, explicit leading coefficient |
| `eigen` | real roots from `find_roots_eigen`, ascending |
| `sturm` | real roots from `find_roots_sturm`, ascending |

A line with no values means the routine returned no real roots — `x^4 + 1` is
the case that exercises this.

The driver converts `poly` into each routine's own convention, which differ:
`find_roots_eigen` takes **ascending, monic, implicit leading 1**, while
`find_roots_sturm` takes **descending, monic, implicit leading 1**. Getting
this wrong makes either routine look broken when it is not; it cost a
measurement here before it was noticed.

## Why `sturm` is recorded

PETIR does **not** port `find_roots_sturm`, and this file is the evidence for
that decision rather than an assertion of it.

On `prod (x - k)` for `k = 1..n`, `find_roots_sturm` never returns more than
three roots at any degree, and reports no error when it drops the rest:

| degree | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|
| `find_roots_eigen` | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
| `find_roots_sturm` | 3 | 3 | 3 | 1 | 3 | 1 | 1 |

`crates/petir/tests/roots_companion_code_to_code.rs` asserts on this, so that
if a future `roots` release fixes it, regenerating this file fails the test
and the porting decision gets reconsidered instead of silently inherited.

## Measured agreement

PETIR's port is **bit-identical** to upstream across all 71 real roots in this
file — worst absolute difference exactly `0`, measured 2026-09-15. See the
test module for what that establishes.
