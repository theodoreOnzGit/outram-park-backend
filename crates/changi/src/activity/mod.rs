// SPDX-License-Identifier: GPL-3.0

//! Radionuclide activity in air and on the ground, from a released source term.
//!
//! This module is **not a port**. Everything under it was written here, and it
//! therefore has **no upstream and no code-to-code verification** — the strongest
//! evidence it can carry is internal consistency, which is what its tests
//! assert. Read every number it produces in that light. `changi`'s two ported
//! modules ([`crate::puff`] and [`crate::flexpart`]) keep their own harnesses
//! and are untouched by this one.
//!
//! ## What it computes
//!
//! Given a release of activity at a point, over one or more time windows:
//!
//! | Quantity | Unit | Where |
//! |---|---|---|
//! | dilution factor, `chi/Q` | s/m^3 | [`chi_over_q`] |
//! | time-integrated air concentration | Bq·s/m^3 | [`units`] |
//! | dry ground deposition | Bq/m^2 | (day 2) |
//!
//! **It computes no dose quantity of any kind**, and none is planned here. See
//! the scope limit below, which is binding.
//!
//! ## Relationship to the two ports — a consumer, not a shared abstraction
//!
//! The crate rule is to keep `puff` and `flexpart` separate, because merging
//! their shared-looking pieces would make each one's comparison against *its
//! own* upstream harder to read. This module **consumes** both — [`crate::puff`]
//! for dispersion and [`crate::flexpart::decay`] for decay in transit — and that
//! is deliberately a different thing:
//!
//! - It sits **above** both and defines no type either port uses.
//! - It changes **no ported signature**. The only edits it required in
//!   `puff::simulate` were widening `Puff` to `pub(crate)`, splitting
//!   `emit_with_classes` out of the existing `emit`, and adding
//!   `puff_unit_response` beside `sum_over_puffs`. Nothing was removed,
//!   generalised, or made to serve two upstreams at once.
//! - Neither fixture, tolerance, or reference script moved.
//!   `tests/puff_code_to_code.rs` and `tests/flexpart_code_to_code.rs` are the
//!   check on that claim, and both must stay green.
//!
//! If a future change here would require a *common* stability class, a *common*
//! dispersion coefficient, or any other type spanning the two ports, that is the
//! rule biting and the answer is to duplicate rather than unify.
//!
//! ## Which dispersion model actually runs
//!
//! [`crate::puff`], which is ported from the **R package `puff` 0.1.1**
//! (Hammerling Research Group, MIT) — a Gaussian puff model with empirical
//! Pasquill-Gifford sigmas, written for **methane** leak detection, fitted over
//! roughly 0.1-10 km and carrying no turbulence closure.
//!
//! It is **not FLEXPART**. `changi::flexpart` is four scalar-kernel modules that
//! its own documentation calls *"the first verified slice of a port"*; it cannot
//! transport a plume and is not used for transport here. Say which model ran
//! when reporting a number from this module.
//!
//! ## Scope limit — binding, do not soften
//!
//! Research, education and V&V only, exactly as the crate root states. Nothing
//! in this module may be described, in code, docs, commit messages or chat, as
//! supporting emergency planning, emergency response, dose assessment for real
//! populations, or Level 3 PSA. Implementing the physics is not what promotes
//! an item out of the crate's *future* scope list; that is a maintainer
//! decision taken in `RESPONSIBLE_USE.md`.

pub mod chi_over_q;
pub mod units;
