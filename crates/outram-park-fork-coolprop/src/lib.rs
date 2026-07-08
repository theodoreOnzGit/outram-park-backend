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
//! ## Status
//!
//! - **Engine** ([`eos`]): every Helmholtz term form CoolProp's fluids use
//!   (residual Power / Gaussian / Exponential / DoubleExponential / GaoB, ideal
//!   Lead / LogTau / Planck–Einstein(+Generalized) / Power / CP0*), with all
//!   first/second `δ`,`τ` derivatives.
//! - **Fluids** ([`fluid::Fluid`]): all 137 CoolProp pure fluids, hardcoded and
//!   wired into the enum (`Fluid::ALL`).
//! - **Properties**: `(T, ρ)` native ([`props::state_trho`]) plus single-phase
//!   `(p, T)` / `(p, h)` / `(p, s)` flashes ([`flash`]).
//! - **Saturation / VLE** ([`vle`], [`ancillaries`]): fast saturation-ancillary
//!   lookups and a thermodynamically-consistent Maxwell two-phase solve
//!   (`p_sat`, `ρ'`, `ρ''`, `T_sat(p)`, and `(p,h)` quality).
//! - **Transport** ([`transport`]): dynamic viscosity `μ` and thermal
//!   conductivity `λ` (CoolProp correlations; critical enhancement omitted).
//! - **Control volume** ([`single_cv::OPCPFluidSingleCV`]): a `uom`-typed 0-D
//!   equilibrium lump of one fluid (the CoolProp analogue of
//!   `tampines-steam-tables`' `TampinesSteamTableCV`) — two-phase-aware, with
//!   `μ`/`λ` getters.
//! - **Transient flow** ([`openfoam_algorithms`]): a vendored pure-Rust
//!   OpenFOAM finite-volume layer + the 1-D compressible `OPCPFluidArray`
//!   solver (no `openfoam-basic-lib` dependency), driven by this crate's EOS.
//!
//! Known gaps (tracked, bead op-kbc): the **non-analytic** critical-region terms
//! are carried but not yet evaluated (accuracy degraded within ~1 % of Tc, a
//! no-op elsewhere); transport covers a common model subset (viscosity 20,
//! conductivity 38 of 137 fluids; hardcoded fluids like Water/CO₂ excluded) and
//! omits the near-critical enhancement; `rfluids` oracle verification is planned.
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

pub mod ancillaries;
pub mod eos;
pub mod flash;
pub mod fluid;
pub mod fluids;
pub mod props;
pub mod single_cv;
pub mod transport;
pub mod vle;

/// Vendored pure-Rust OpenFOAM finite-volume layer + 1-D compressible solvers,
/// copied from `tampines-steam-tables` (no `openfoam-basic-lib` dependency).
/// The transient-flow backbone whose thermo plug-in point is backed by this
/// crate's CoolProp EOS. See its module `CLAUDE.md` for provenance.
pub mod openfoam_algorithms;

pub use ancillaries::{FluidAncillaries, SatAncillary};
pub use eos::{FluidEos, HelmholtzDerivs, IdealTerm, ResidualTerm};
pub use flash::{state_ph, state_ps, state_pt, density_pt, FlashError};
pub use fluid::Fluid;
pub use openfoam_algorithms::rhoPimpleFoam::OPCPFluidArray;
pub use props::{state_trho, pressure_trho, FluidState};
pub use single_cv::OPCPFluidSingleCV;
pub use transport::{conductivity, viscosity, FluidTransport};
pub use vle::{phase_at_ph, saturation_at_temperature, saturation_temperature, PhaseAtPh, SaturationState};
