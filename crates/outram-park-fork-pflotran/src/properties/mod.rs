//! Fluid & material properties (v1) — bead op-v6s.7.
//!
//! The constitutive closures the RICHARDS flow slice needs, `uom`-typed at the
//! public boundary and pure `f64` (SI base units) inside:
//!
//! - [`LiquidWaterEos`] — a slightly-compressible, constant-viscosity liquid
//!   equation of state giving density `rho(p)`, its pressure derivative, and a
//!   constant viscosity. See [`eos`].
//! - [`CharacteristicCurves`] — the variably-saturated retention `S_e(p_c)` and
//!   liquid relative-permeability `k_r(S_e)` curves, dispatched by enum over the
//!   [`VanGenuchten`] (van Genuchten–Mualem) and [`BrooksCorey`]
//!   (Brooks–Corey–Burdine) model families. See [`curves`].
//!
//! # Conventions (shared by the curves)
//!
//! - Capillary pressure `p_c = p_gas - p_liq`; `p_c >= 0` unsaturated,
//!   `p_c <= 0` fully saturated.
//! - Effective saturation `S_e = (S_l - S_r)/(1 - S_r)` in `[0, 1]`; convert to
//!   liquid saturation with `S_l = S_r + (1 - S_r) S_e` using
//!   [`CharacteristicCurves::residual_saturation`].
//! - Outputs are clamped (`S_e`, `k_r` to `[0, 1]`); saturated/residual limits
//!   are handled without `NaN`. The one genuinely singular slope
//!   (van Genuchten–Mualem `dk_r/dS_e -> +inf` as `S_e -> 1`) is documented on
//!   [`VanGenuchten`] and returns `f64::INFINITY`, never `NaN`.
//!
//! # Design notes for reviewers
//!
//! - **Relative-permeability model pairing.** van Genuchten retention is paired
//!   with **Mualem** (1976) pore-connectivity `k_r`; Brooks–Corey retention with
//!   **Burdine** `k_r`. These are the classical, self-consistent pairings and
//!   match PFLOTRAN's defaults; deviating (e.g. VG+Burdine) is possible upstream
//!   but not offered here in v1.
//! - **Untrusted AI-generated draft** until a human reviews it (workspace
//!   `RESPONSIBLE_USE.md`): no human V&V yet. The analytical derivatives are
//!   unit-tested against central finite differences, but the curves have not
//!   been validated against a published PFLOTRAN reference case.

mod curves;
mod eos;

pub use curves::{BrooksCorey, CharacteristicCurves, VanGenuchten};
pub use eos::LiquidWaterEos;
