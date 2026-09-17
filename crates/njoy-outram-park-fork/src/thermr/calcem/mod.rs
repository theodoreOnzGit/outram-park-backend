//! `calcem` — THERMR's incoherent-inelastic energy-angle kernel builder, plus
//! its `sigl`/`sigu` helpers.
//!
//! Faithful port of NJOY2016 `thermr.f90::calcem` (lines 1541-2480),
//! `thermr.f90::sigl` (lines 2660-2872), and `thermr.f90::sigu` (lines
//! 2874-2996). `calcem` computes, over a fixed incident-energy grid
//! ([`egrid::EGRID`]), the secondary energy-angle distribution for
//! incoherent-inelastic scattering from `S(α,β)`, in **either** of two output
//! forms selected by the card input `iform`:
//!
//! - **`iform=0`** ([`iform0::compute_iform0`]): a compact
//!   equally-probable-cosine representation (`LANG=3`) — `nbin` cosines per
//!   outgoing energy, adaptively linearised in `E'`.
//! - **`iform=1`** ([`iform1::compute_iform1`]): a fixed, adaptively-derived
//!   `μ` grid, each with its own continuous, adaptively-linearised
//!   secondary-energy law (`LAW=7`). This is the more accurate of the two —
//!   the named cure for [GitHub #188] — because it never collapses the
//!   angular structure of the double-differential kernel into a handful of
//!   cosines the way `iform=0` (and this crate's pre-existing
//!   [`super::inelastic::IncoherentInelastic::equiprobable_emission`]) does.
//!
//! [GitHub #188]: https://github.com/theodoreOnzGit/outram-park-backend/issues/188
//!
//! ## Module layout
//!
//! | Module | Ported from | Contents |
//! |---|---|---|
//! | [`egrid`] | `thermr.f90:1575,1587-1607,1613-1614` | The fixed 118-point incident-energy grid, `break`/`therm`, and the `temp > break` rescale formulas |
//! | [`sigl`] | `thermr.f90:2617-2872` (`terpq` is not re-derived here — [`super::inelastic`] already carries it) | Cross section + equally-probable-cosine/Legendre-moment computation for one `(E, E')` pair |
//! | [`sigu`] | `thermr.f90:2874-2996` | Adaptively-linearised secondary-energy distribution at one fixed `μ` |
//! | [`iform0`] | `thermr.f90:1918-2266` | The `iform=0` incident-energy loop, panel walk, and `ubar`/`p2`/`p3` accumulation |
//! | [`iform1`] | `thermr.f90:2276-2404` | The `iform=1` mu-grid reconstruction and per-`μ` continuous `E'` law |
//! | [`types`] | — (no direct Fortran counterpart) | The Rust output types both paths build |
//!
//! `calcem`'s own `sig` function — the double-differential kernel `sig` is
//! called against at every adaptively-chosen point — is **not** reimplemented
//! here: both [`sigl`] and [`sigu`] call
//! [`super::mf7::IncoherentInelastic::double_differential`], which is
//! [`super::inelastic`]'s existing faithful port of `thermr.f90`'s `sig`
//! (label 150/170), including the 3×3 ln-`S` `terpq` stencil, the liquid
//! small-α branch, the floored-corner gate, and the SCT tail. Nothing in this
//! module duplicates that logic.
//!
//! ## What is deliberately NOT ported here
//!
//! - **MF=6 tape record packing.** Upstream writes both formats as ENDF
//!   `CONT`/`TAB1`/`TAB2`/`LIST` words to a scratch tape (`nscr`), including
//!   record-count bookkeeping (`ncds`) and `TAB1` paging at `npage` words
//!   (`thermr.f90:2385-2404`). This crate has no MF=6/LAW=1/LAW=7 writer, so
//!   [`types::Iform0Table`] and [`types::Iform1Table`] carry the same
//!   physical content — already `sigfig`-rounded and clamped exactly as NJOY
//!   leaves it — as plain Rust data, for a future ACE/MF=6 writer to consume.
//! - **The final elastic+inelastic cross-section merge onto the old PENDF
//!   tape** (`thermr.f90` label 610 to the end of `calcem`, using
//!   `finda`/`loada`/`bufo`/`bufn`). That is old/new-PENDF-tape bookkeeping,
//!   not part of the physics kernel; each output record's
//!   `cross_section_b` is the same `σ_inel(E)` that merge would otherwise
//!   splice onto the tape's own energy grid.
//! - **The free-gas `iinc=1` main-scatterer path**
//!   (`thermr.f90:1857-1916`, and the `calcem`-top preamble that reads
//!   `S(α,β)` straight off the ENDF tape, `thermr.f90:1652-1856`). Both
//!   `sigl` and `sigu` here take an already-parsed
//!   [`super::mf7::IncoherentInelastic`] — this crate's [`super::mf7`] module
//!   is the reader, and it has no representation of NJOY's free-gas
//!   synthetic 45-point β grid (`iinc=2`, i.e. "compute from the tabulated
//!   ENDF `S(α,β)`", is the only mode this crate's `IncoherentInelastic`
//!   represents). [`sigl::sigl`]'s doc comment states this simplification
//!   explicitly at the one place it matters (the `iinc.eq.2` guard inside
//!   `sigl` collapsing to `lat.eq.1`).
//! - **Per-temperature `itemp==1` global-state caching** (`nl`, `nne`,
//!   `esi`/`xsi` arrays shared across a `THERMR` run's multiple output
//!   temperatures, `thermr.f90:1623-1648`). [`iform0::compute_iform0`] and
//!   [`iform1::compute_iform1`] are single-temperature, single-call
//!   functions (temperature comes from `ii.temperature_k`); a caller
//!   processing several temperatures calls them once per temperature.
//! - **`iprint` diagnostic printing.** NJOY's `iprint==2` branch both prints
//!   *and*, for `iform=0`, is the only place `ubar`/`p2`/`p3` are normalized
//!   by the cross section (`thermr.f90:2207-2209`) — see
//!   [`types::IncidentEnergyRecord::mubar`] for why this port always
//!   normalizes instead (a packaging choice, not an algorithm change).

pub mod egrid;
pub mod iform0;
pub mod iform1;
pub mod sigl;
pub mod sigu;
pub mod types;

pub use iform0::compute_iform0;
pub use iform1::compute_iform1;
pub use sigl::sigl;
pub use sigu::{sigu, SiguResult};
pub use types::{
    EqualProbableRow, Iform0Table, Iform1Table, IncidentEnergyAngleRecord, IncidentEnergyRecord,
    MuDistribution,
};
