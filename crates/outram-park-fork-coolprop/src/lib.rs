//! # outram-park-fork-coolprop
//!
//! A pure-Rust fork/translation of [CoolProp](https://github.com/CoolProp/CoolProp)
//! (MIT) — thermophysical properties from Helmholtz-energy-explicit equations
//! of state — built to OUTRAM PARK's design rules:
//!
//! - **Enum dispatch, no trait objects.** Fluids are a [`Fluid`] enum and
//!   EOS-term forms are [`eos::ResidualTerm`] / [`eos::IdealTerm`] enums,
//!   dispatched by `match` (see [`eos`]).
//! - **Hardcoded data, no runtime JSON.** Each fluid's EOS coefficients are
//!   `const` Rust in [`fluids`], generated once from CoolProp's fluid JSON by
//!   `dev/gen_fluid.py` (the CoolProp clone lives in the gitignored
//!   `reference/` for development only). A few KB per fluid.
//! - **Pure `std`, no BLAS / C deps** — so it builds on Android too.
//!
//! ## Status (initial port)
//!
//! First vertical slice: the Helmholtz EOS engine (residual **Power** +
//! **Gaussian** terms and the **Lead / LogTau / Planck–Einstein** ideal-gas
//! terms) and `(T, ρ)` property evaluation ([`props::state_trho`]), verified
//! for **Water** away from the critical point. Known gaps, each a tracked
//! follow-up (bead op-kbc):
//!
//! - The **non-analytic** critical-region terms are carried in the fluid data
//!   but not yet evaluated, so accuracy within ~1 % of the critical point is
//!   degraded (a no-op, not a wrong number, everywhere else).
//! - Only `(T, ρ)` inputs; the `(T, p)` / `(p, h)` … flashes need a density
//!   solve (not yet ported).
//! - Only `Water` so far; more fluids via `dev/gen_fluid.py`.
//! - Verification against `rfluids` (CoolProp oracle) as a dev-dependency, and
//!   a `uom`-typed public API, are planned.
//!
//! ## Example
//!
//! ```
//! use outram_park_fork_coolprop::{Fluid, props::state_trho};
//! // Superheated steam at 500 K, 2 kg/m³ (~0.46 MPa).
//! let s = state_trho(Fluid::Water, 500.0, 2.0);
//! assert!(s.pressure > 0.0);
//! assert!(s.speed_of_sound > 0.0);
//! ```
//!
//! (Note: liquid water's `p(ρ)` is extremely steep, so a density just below the
//! saturated-liquid value gives a small *negative* pressure — the metastable
//! tension region. `(T, ρ)` inputs are literal EOS evaluations, not a
//! phase-stable flash.)

pub mod eos;
pub mod fluid;
pub mod fluids;
pub mod props;

pub use eos::{FluidEos, HelmholtzDerivs, IdealTerm, ResidualTerm};
pub use fluid::Fluid;
pub use props::{state_trho, pressure_trho, FluidState};
