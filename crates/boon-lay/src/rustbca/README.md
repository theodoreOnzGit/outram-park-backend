# `boon_lay::rustbca` — a copy of RustBCA

**This directory is a copy of RustBCA, not boon-lay's own code.** boon-lay
re-exports it as `boon_lay::rustbca` (GitHub issue #876, item 7).

| | |
|---|---|
| Upstream | `lcpp-org/RustBCA`, <https://github.com/lcpp-org/RustBCA> |
| Version, commit | v3.0.0, `a356280d564bbdfbe74b97f32db0776444da58a5` (2026-10-02) |
| Authors | "Jon Drobny <drobny2@illinois.edu>", "Jon Drobny <jdrobny@tae.com>" (upstream `Cargo.toml` `authors`). The paper's authors are Jon T. Drobny and Davide Curreli. Upstream states no copyright line. |
| Paper | J. T. Drobny and D. Curreli, "RustBCA: A High-Performance Binary-Collision-Approximation Code for Ion-Material Interactions", *Journal of Open Source Software* 6(64), 3298 (2021), doi:10.21105/joss.03298 |
| Licence | GNU GPL v3. Upstream's `LICENSE` is copied beside this file. |

Every copied file starts with the workspace provenance header: upstream
project, source file, commit, authors and licence, and that file's local
changes.

## What changed against upstream

The full, numbered list is in the module documentation at the top of
`mod.rs`. In short:

- **`no_std` + `alloc`.** `std` paths became `core` / `alloc`. The five
  `LazyLock` lookup tables are now evaluated directly; the values are
  bit-identical, and a test rebuilds upstream's tables to check every entry.
  Float maths goes through `libm` via `petir::real::Real`, written
  fully qualified so that boon-lay's `std` build cannot silently use the
  platform libm.
- **Not copied:** the C, Fortran and Python bindings, the CLI (`main.rs`), and
  the `parry3d`, `cpr_rootfinder`, `distributions` and `python` features.
- **Behind boon-lay's `rustbca-std` feature (off by default):** TOML input
  files, file output, the rayon/indicatif batch driver, entropy seeding,
  `simple_bca`, and the `geo`-backed 2D meshes.
- **Physics:** unchanged.

The two commits that brought this tree in read as a diff against upstream.
The first is the verbatim copy, with only the line endings changed. The second
holds every local change.

## Checks

- Upstream's own 10 unit tests (`tests.rs`) pass on the copy.
- `local_change_tests.rs` (boon-lay's, **not upstream**) pins the local
  changes: table bit-identity, no method-syntax transcendentals, std-only code
  gated, provenance headers present, entropy seeding, and determinism.
- `scripts/check-rustbca-no-std.sh` (workspace root) compiles this tree as a
  `#![no_std]` library.

## Validity

**Fission-fragment results are illustrative until they pass V&V against
uncalibrated range data.** RustBCA's high-energy electronic stopping is
Bethe with the bare projectile charge `Za²`. Alphas and recoil knock-ons are
inside its documented regime, but no range here has been compared with
measured data yet.
