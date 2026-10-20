// SPDX-License-Identifier: GPL-3.0
//
// RustBCA copy — provenance
// -------------------------
// Upstream project : RustBCA — https://github.com/lcpp-org/RustBCA
// Upstream version : v3.0.0, commit a356280d564bbdfbe74b97f32db0776444da58a5
// Upstream source  : src/lib.rs (crate attributes, imports, module list and
//                    re-exports only; the C/Fortran/Python entry points are
//                    not carried) — the rest of this file is boon-lay's
// Authors          : Jon Drobny <drobny2@illinois.edu>, Jon Drobny
//                    <jdrobny@tae.com> (upstream Cargo.toml `authors`); upstream
//                    states no copyright line of its own
// Original license : GNU GPL v3 (upstream LICENSE, copied beside this file)
// This copy is distributed under GPL-3.0 as part of boon-lay. Every local
// change is listed in the module documentation below.

//! # `rustbca` — a copy of RustBCA, made `no_std` + `alloc`
//!
//! **This module is a copy of RustBCA**, the binary-collision-approximation
//! (BCA) Monte Carlo code for ion–material interactions by Jon Drobny and
//! Davide Curreli. It is not boon-lay's own code. boon-lay re-exports it as
//! `boon_lay::rustbca` (gh:#876, item 7). Charged particles are boon-lay's
//! remit and neutral ones `outram-mc-libs`' (maintainer decision 2026-10-16);
//! RustBCA is the first charged-particle code here.
//!
//! | | |
//! |---|---|
//! | Upstream | `lcpp-org/RustBCA`, <https://github.com/lcpp-org/RustBCA> |
//! | Version, commit | v3.0.0, `a356280d564bbdfbe74b97f32db0776444da58a5` (2026-10-02) |
//! | Authors | "Jon Drobny <drobny2@illinois.edu>", "Jon Drobny <jdrobny@tae.com>" (upstream `Cargo.toml` `authors`). The JOSS paper's authors are Jon T. Drobny and Davide Curreli. Upstream states no copyright line. |
//! | Paper | J. T. Drobny and D. Curreli, "RustBCA: A High-Performance Binary-Collision-Approximation Code for Ion-Material Interactions", *Journal of Open Source Software* **6**(64), 3298 (2021), doi:10.21105/joss.03298 |
//! | Licence | GNU GPL v3; upstream's `LICENSE` is copied as `src/rustbca/LICENSE`. Upstream does not say "-only" or "-or-later"; either is compatible with boon-lay's GPL-3.0. |
//!
//! Every copied file carries the workspace provenance header. The README beside
//! this file repeats the attribution for readers browsing the tree.
//!
//! ## Physics scope and the one validity warning that matters
//!
//! RustBCA follows ions and the recoils they knock on through **amorphous**
//! targets: screened-Coulomb binary collisions (Kr-C, ZBL, Lenz-Jensen,
//! Moliere; Lennard-Jones and Morse need the root-finders not carried here)
//! between straight free flights, with electronic stopping applied along each
//! flight. In boon-lay it serves fission fragments, alphas and recoil
//! knock-ons.
//!
//! **Fission fragments are inside RustBCA's stated envelope but unverified.**
//! At 0.5–1 MeV/u only the `INTERPOLATED` / `INTERPOLATEDPLUS` electronic
//! stopping modes apply (the low-energy modes are documented valid only up to
//! ~25 keV/u). Their high-energy term is Bethe with the **bare** projectile
//! charge `Za²` (`material.rs`, `bethe_bloch_stopping_power_cross_section`),
//! with no effective-charge treatment, which is exactly the regime where a
//! heavy ion's charge state matters. **Fragment results are illustrative until
//! they pass V&V against uncalibrated range data.** Do not tune
//! `INTERPOLATEDPLUS`'s `ci` to match a range: that is calibration before an
//! uncalibrated comparison, which the workspace model-hierarchy rule forbids.
//!
//! ## `no_std` + `alloc`
//!
//! Everything compiled without the `rustbca-std` feature uses only `core`,
//! `alloc` and `no_std` dependencies (`rand` and `rand_distr` without default
//! features, `petir`, `serde`, `anyhow`), so the tree builds inside a
//! `#![no_std]` crate. boon-lay itself stays `std` and re-exports this module;
//! `scripts/check-rustbca-no-std.sh` (at the workspace root) proves the claim
//! by compiling this exact tree as a `#![no_std]` library.
//!
//! Behind `rustbca-std` (off by default): TOML input files, the file writers,
//! the rayon/indicatif batch driver (`physics_loop`, `silent_physics_loop`,
//! native only), entropy seeding for `seed < 0`, the `simple_bca` helpers
//! (which seed from entropy) and the `geo`-backed 2D geometries `Mesh2D` and
//! `HomogeneousMesh2D`. The 2D geometries are also compiled under `cfg(test)`
//! so that upstream's own unit tests, which use `Mesh2D`, run in the ordinary
//! test suite (`geo` is a dev-dependency for that).
//!
//! **Float maths goes through `libm`.** Every transcendental call (`exp`,
//! `ln`, `powf`, `powi`, `sin`, `cos`, `sin_cos`, `acos`, `atan2`, `cbrt`) is
//! written `Real::f(x, …)`, with [`petir::real::Real`] — the workspace's one
//! `no_std` float shim, not a second one. Method syntax is NOT enough in a
//! `std` crate: `x.r_exp()` resolves to `f64`'s inherent `std` method before any
//! trait method, so it would silently go to the platform libm on native
//! builds. The fully-qualified form routes to `libm` in both builds, so a
//! cascade is bit-identical native and wasm. `sqrt`, `abs`, `floor` and the
//! rest of the exact or correctly rounded operations keep method syntax: they
//! give the same bits everywhere (see `petir::real`'s module docs).
//!
//! ## Every local change against upstream `a356280d`
//!
//! Read the two commits that brought this tree in: the first is the verbatim
//! copy (line endings only), the second holds every change below.
//!
//! 1. **Line endings** CRLF → LF in every copied file.
//! 2. **Provenance header** added to every copied file.
//! 3. **Files not copied.** `main.rs` (the CLI), `parry.rs` (the `parry3d`
//!    feature, ParryBall/ParryTriMesh geometries) and **all of `lib.rs` except**
//!    its crate attributes, imports, module list and re-exports (this file) and
//!    `simple_bca` / `simple_compound_bca` (now `simple.rs`). Dropped from
//!    `lib.rs`: **the whole C, Fortran and Python FFI** (`#[no_mangle]`
//!    `extern "C"` functions, `#[repr(C)]` I/O structs, `drop_output_*`,
//!    the `pyo3` module and every `*_py` function, `get_seed`), because
//!    nothing in the workspace calls RustBCA across a language boundary.
//! 4. **Upstream features not carried:** `cpr_rootfinder` (needs the `rcpr` git
//!    crate: the CPR and POLYNOMIAL root-finders, `polynomial_rootfinder`,
//!    `cpr_rootfinder`, upstream's `test_polynom` and the CPR comparison in
//!    `test_quadrature`), `parry3d` (ParryBall, ParryTriMesh, the two parry
//!    tests), `distributions` (ndarray `Distributions` output, the extra
//!    `Options` fields, `test_distributions`), `python`, and `no_list_output`
//!    (list output is always on in `physics_loop`, upstream's default). Where
//!    a panic message told the user to "build with cpr_rootfinder", it now
//!    says the root-finder is not carried here.
//! 5. **`std` → `core`/`alloc`.** The root imports `core::fmt`,
//!    `core::mem::discriminant`, `core::f64::consts` and `alloc`'s `Box`,
//!    `String`, `ToString`, `Vec`, `vec!`, `format!` explicitly;
//!    `math::triangular_index` uses `core::mem::swap`; `structs.rs` imports
//!    `Real` so its `.sqrt()` resolves without `std`.
//! 6. **`LazyLock` tables replaced** (5 statics, not the 3 the feasibility
//!    note counted): `COULOMB_CONSTANT_TABLE`,
//!    `LINDHARD_SCREENING_LENGTH_TABLE`, `ZBL_SCREENING_LENGTH_TABLE`
//!    (`interactions.rs`) and `LS_STOPPING_CONSTANT_TABLE`,
//!    `BV_EMPIRICAL_MEAN_IONIZATON_POT_TABLE` (`material.rs`). Each lookup now
//!    evaluates the table builder's own expression on the same truncated
//!    integer index, so values are **bit-identical** to the table entries and
//!    out-of-range indices still panic
//!    (`local_change_tests::lazylock_replacements_match_upstream_tables`
//!    rebuilds upstream's tables and compares all 36 300 entries). The
//!    builders `coulomb_constant` and
//!    `lindhard_scharff_stopping_power_constant` are now `pub(super)` so that
//!    test can reach them.
//!    The cost is a function evaluation per lookup instead of an array read.
//! 7. **`libm` routing** of the 177 transcendental call sites (163 in the
//!    library, 14 in upstream's tests) through
//!    `petir::real::Real` (fully qualified, see above).
//!    Not bit-identical to upstream on glibc: musl-derived
//!    `libm` differs from glibc in the last ulp on a few per cent of calls.
//! 8. **Gated behind boon-lay's `rustbca-std` feature:** `InputFile::new` and
//!    its impls (TOML parsing, `toml` 0.8 from the workspace instead of
//!    upstream's 1.1 — same `from_str` call), `read_input_file`, `input`,
//!    the file-output types and functions in `output.rs`,
//!    `physics_loop` / `silent_physics_loop` (also off wasm, where rayon is
//!    absent), and `simple.rs`. `Mesh2D`, `HomogeneousMesh2D`, their input
//!    structs, `Input2D`, `InputHomogeneous2D` and the matching `MaterialType`
//!    variants are gated on `any(test, feature = "rustbca-std")`.
//! 9. **Entropy seeding.** `process_input_file` with `seed < 0` called
//!    `rand::random()`. It now calls [`entropy_seed`], which does the same
//!    under `rustbca-std` and panics with an explanation without it (a `no_std`
//!    build has no entropy source). Seeds `>= 0` are unchanged.
//! 10. **Dependencies:** `rand` 0.10 (`chacha`, `alloc`; `thread_rng` only
//!     under `rustbca-std`) and `rand_distr` 0.6 (`alloc`; its `num-traits`
//!     maths is `libm`) without default features; `anyhow`, `serde` from the
//!     workspace; `itertools`, `indicatif`, `geo` 0.25 and `toml` only under
//!     `rustbca-std` (`geo` also as a dev-dependency); `float-cmp` 0.10 as a
//!     dev-dependency for upstream's tests. All versions live in the root
//!     `[workspace.dependencies]`.
//! 11. **`#[cfg(test)] mod tests`** instead of upstream's `pub mod tests`, and
//!     a boon-lay test file, `local_change_tests.rs`, that is **NOT upstream**:
//!     it pins the local changes above (table bit-identity, `no_std` source
//!     scan, provenance headers, entropy seeding, determinism, and, under
//!     `rustbca-std`, an upstream TOML input parsed and run).
//!
//! The physics code itself (collision kinematics, scattering integrals,
//! stopping powers, geometry logic) is otherwise unchanged.

// Upstream lib.rs crate attributes, kept for the copied code.
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
// Upstream's code style, which boon-lay does not restyle (a restyle would make
// the diff against upstream unreadable).
#![allow(unused_parens, clippy::all)]

// --- imports, as upstream lib.rs, with std replaced by core/alloc ----------
use petir::mathf::RealMath;
use core::fmt;
use core::mem::discriminant;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

//Error handling crate
use anyhow::{Result, Context, anyhow};

//Serializing/Deserializing crate
use serde::*;

//I/O
#[cfg(feature = "rustbca-std")]
use std::fs::OpenOptions;
#[cfg(feature = "rustbca-std")]
use std::io::prelude::*;
#[cfg(feature = "rustbca-std")]
use std::io::BufWriter;

//itertools
#[cfg(feature = "rustbca-std")]
use itertools::{izip};

//RNG
use rand::{SeedableRng, rngs::ChaCha8Rng};

//Math
use core::f64::consts::FRAC_2_SQRT_PI;
use core::f64::consts::PI;
use core::f64::consts::SQRT_2;
// boon-lay: libm-backed float maths, called fully qualified (see module docs).
use petir::real::Real;

//Load internal modules
pub mod material;
pub mod particle;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod local_change_tests;
pub mod interactions;
pub mod bca;
pub mod geometry;
pub mod input;
pub mod output;
pub mod enums;
pub mod consts;
pub mod structs;
pub mod sphere;
pub mod math;
pub mod physics;
/// `simple_bca` and `simple_compound_bca` from upstream `lib.rs`. They seed
/// their RNG from entropy, so they need `rustbca-std`.
#[cfg(feature = "rustbca-std")]
pub mod simple;

pub use self::enums::*;
pub use self::consts::*;
pub use self::structs::*;
pub use self::input::{Input1D, Input0D, Options, InputFile, GeometryInput};
#[cfg(any(test, feature = "rustbca-std"))]
pub use self::input::{Input2D, InputHomogeneous2D};
pub use self::output::{OutputUnits};
pub use self::geometry::{Geometry, GeometryElement, Mesh0D, Mesh1D};
#[cfg(any(test, feature = "rustbca-std"))]
pub use self::geometry::{Mesh2D, HomogeneousMesh2D};
pub use self::sphere::{Sphere, SphereInput, InputSphere};
pub use self::math::*;
pub use self::material::*;
pub use self::physics::*;
#[cfg(feature = "rustbca-std")]
pub use self::simple::{simple_bca, simple_compound_bca};

/// A seed for `options.seed < 0`, which upstream reads as "seed from entropy".
///
/// **boon-lay local change** (item 9 of the module docs). With `rustbca-std`
/// this is upstream's `rand::random()`. Without it there is no entropy source
/// in a `no_std` build, so it panics and says why; pass a seed `>= 0`.
#[cfg(feature = "rustbca-std")]
pub fn entropy_seed() -> u64 {
    rand::random()
}

/// A seed for `options.seed < 0`, which upstream reads as "seed from entropy".
///
/// **boon-lay local change** (item 9 of the module docs). With `rustbca-std`
/// this is upstream's `rand::random()`. Without it there is no entropy source
/// in a `no_std` build, so it panics and says why; pass a seed `>= 0`.
#[cfg(not(feature = "rustbca-std"))]
pub fn entropy_seed() -> u64 {
    panic!("Input error: seed < 0 asks for an entropy seed, which a no_std build cannot supply; \
        pass a seed >= 0, or enable boon-lay's rustbca-std feature")
}
