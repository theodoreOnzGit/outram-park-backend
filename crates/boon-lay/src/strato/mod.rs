// SPDX-License-Identifier: AGPL-3.0-only
//
// PORT (in part) of STRATO, by Wee Zhern Ray (GitHub: weewzr).
//   Upstream: https://github.com/weewzr/STRATO
//   Commit:   36f1a98848bec9d66fe515994e075cff09d6cefb
//   Licence:  AGPL-3.0-only
//   Ported:   2026-11-26

//! # STRATO: analytical multilayer TRISO diffusion (a port)
//!
//! **Credit.** STRATO is the work of **Wee Zhern Ray** (GitHub
//! [`weewzr`](https://github.com/weewzr)), <https://github.com/weewzr/STRATO>,
//! licensed AGPL-3.0-only. This module ports part of it, at commit
//! `36f1a98848bec9d66fe515994e075cff09d6cefb`, into boon-lay on 2026-11-26.
//! STRATO's Rust sources carry no headers; the author is named from its
//! `CITATION.cff`.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`). Nothing here is
//! for reactor operation, licensing or safety decisions.
//!
//! ## What was ported
//!
//! [`sturm_liouville`], from upstream `src/analytical/sturm_liouville.rs`
//! ("Code C"): the exact Sturm-Liouville eigenfunction series for release from
//! a layered sphere with an initial inventory in the innermost layer (the
//! kernel), no source, `c` and `D dc/dr` continuous at each interface and
//! `c = 0` at the outer surface. Every upstream function is kept with its
//! name, arguments, `f64` SI arithmetic and order of operations: the Robin
//! root finder, the layer transfer and interface rules, the Sturm count, the
//! eigenvalue bisection, the 16-point Gauss-Legendre weights,
//! `ModalRelease` and `exact_release`. All seven upstream unit tests are
//! ported with their tolerances, in `strato/tests.rs`.
//!
//! ## What was deliberately NOT ported
//!
//! - **The finite-volume solver ("Code B", upstream `src/numerical/`).** It
//!   duplicates the algorithm class of boon-lay's own
//!   [`multilayer_diffusion`](crate::triso_atops_extensions::multilayer_diffusion)
//!   (cell-centred FV, backward Euler, series-resistance faces). It is run
//!   as unmodified upstream code by an external harness, as an independent
//!   implementation to compare against.
//! - Upstream's configuration, CLI and post-processing (`config.rs`,
//!   `main.rs`, `postprocessing/`).
//!
//! ## Local changes (numbered; each is marked `LOCAL CHANGE Ln` in the code)
//!
//! 1. **L1, errors without `anyhow`.** Upstream's `anyhow::{bail, Result}` is
//!    replaced by [`sturm_liouville::StratoError`] (one variant per `bail!`
//!    site, `Display` printing upstream's message text exactly, the two
//!    formatted messages carrying their `f64`) and
//!    `sturm_liouville::Result<T>`. Every error is raised at the same point
//!    and under the same condition as upstream.
//! 2. **L2, `no_std` + `alloc`.** `std::f64::consts::PI` becomes
//!    `core::f64::consts::PI`; `Vec` and `vec!` come from `alloc` through
//!    `crate::nostd_prelude` without `std`. Float functions keep METHOD
//!    syntax: with `std` they resolve to std's inherent methods, exactly as
//!    upstream; without `std`, to `petir::real::Real` (libm), which may
//!    differ from the platform libm in the last bits.
//! 3. **L3, comments only.** A panic justification on upstream's
//!    `expect("finite roots")`, doc lines on `validate_layers`, and this
//!    provenance. No arithmetic changed.
//! 4. **L4, `uom` facade (boon-lay addition, not upstream).**
//!    [`sturm_liouville::Layer::new`] takes `Length` and
//!    `DiffusionCoefficient`; [`sturm_liouville::ModalRelease::release_fraction_at`]
//!    takes a `Time` and returns `Option<Ratio>`, `None` before
//!    `min_valid_time()`, the rule upstream's `exact_release` applies. The
//!    upstream `f64` methods are kept.
//! 5. **L5, one-layer series (boon-lay addition, not upstream).**
//!    [`sturm_liouville::ModalRelease::single_sphere`] poses Booth's uniform
//!    sphere (one layer, `R1 = R`), which upstream's `validate_layers`
//!    (two layers or more) refuses. To run the SAME code path, upstream's
//!    `eigenvalues` body after `validate_layers` was moved unchanged into a
//!    private `eigenvalues_of_validated_layers`, and the `ModalRelease::new`
//!    body after the call to `eigenvalues` (whose first step is
//!    `validate_layers`) into a private `from_validated_layers`. `eigenvalues`
//!    and `new` call `validate_layers` and then these, so the upstream path
//!    performs the same operations in the same order. `single_sphere` calls
//!    a one-layer check (`validate_single_layer`: upstream's per-layer
//!    predicate with `last = 0`) instead. Upstream's internals handle one
//!    layer without change: there is no interface, so the `skip(1)` loop over
//!    interfaces is empty and `min_flux_ratio` stays `+inf`
//!    (`accuracy_floor()` = 2.2e-16).
//! 6. **L6, tests reorganised.** Upstream's tests are in `strato/tests.rs`;
//!    the five-layer input written inline twice upstream is shared through one
//!    helper with the same values. boon-lay's own tests (single sphere vs
//!    Booth, error texts, `exact_release`, the `uom` facade) follow, marked.
//! 7. **L7, visibility.** Unchanged: upstream's public items stay public
//!    and its private items (`LayerStart`, `shoot`, `gauss_legendre_16`,
//!    `integrate`) stay private. The two helpers added in L5 are private.
//! 8. **L8, transcendentals through PETIR** (2026-11-26, maintainer:
//!    "petir is intended", to clear the workspace `no_platform_libm` gate).
//!    The ten `tan`, `cos`, `sin`, `atan2`, `powi` and `exp` calls in
//!    `sturm_liouville.rs` go through `petir::mathf::RealMath` (`r_cos`,
//!    ...), not the platform libm that upstream's Python/NumPy uses. The
//!    arithmetic and order of operations are unchanged. Effect on U1 (the
//!    port against upstream, 978 values): bit-identical went from 978 to
//!    767; eigenvalues and weights stay within 1e-12 relative; the release
//!    fraction F(t) = 1 - sum w exp(-lambda t), whose early-time values are
//!    cancellation noise at ~1e-15, differs by at most 4.4e-15 absolute and
//!    is now judged absolutely (V&V record, U1).

/// STRATO's analytical Sturm-Liouville series (upstream "Code C"), ported.
pub mod sturm_liouville;

#[cfg(test)]
mod tests;
