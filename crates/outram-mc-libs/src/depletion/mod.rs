//! # Depletion / transmutation driver
//!
//! Evolves nuclide number densities under combined **radioactive decay** and
//! **neutron-induced transmutation** (the Bateman equations), and couples that
//! evolution to the Monte Carlo transport in [`crate::physics`] to form a
//! **burnup loop**: transport gives one-group reaction rates, the rates build a
//! depletion matrix, the matrix is exponentiated over a burnup step, and the
//! updated densities feed the next transport solve.
//!
//! ```text
//!   transport (k, one-group rates)  ->  DepletionChain::build_matrix
//!            ^                                     |
//!            |                                     v
//!     updated densities   <--  cram16( A, N, dt )  (matrix exponential)
//! ```
//!
//! ## What belongs here / what does not
//!
//! * [`matrix`] — the dense burnup matrix `A` (`dN/dt = A N`, units `1/s`).
//! * [`cram`] — the **Chebyshev Rational Approximation Method** matrix-exponential
//!   solver `N(t+dt) = exp(A dt) N(t)`, the numerically sound scheme OpenMC uses
//!   for the stiff decay/transmutation system.
//! * [`chain`] — the [`DepletionChain`](chain::DepletionChain): decay constants,
//!   branching, neutron-reaction targets, and fission yields, assembled into a
//!   [`DepletionMatrix`](matrix::DepletionMatrix). Consumes external open data
//!   (OpenMC ENDF/B-8 depletion-chain decay data and ENDF/B-VIII.0 fission
//!   yields) rather than rebuilding it.
//! * [`operator`] — the burnup loop (predictor / forward-Euler integrator)
//!   coupling transport to the transmutation step.
//!
//! Nuclear-data *parsing* does **not** belong here — decay data and fission
//! yields arrive pre-parsed from their data crates; cross sections arrive from
//! `njoy-outram-park-fork`. This module is the transmutation *algorithm* plus a
//! thin coupling layer, mirroring OpenMC's `openmc/deplete/` split.
//!
//! ## Provenance
//!
//! * CRAM coefficients + algorithm: OpenMC `openmc/deplete/cram.py` (MIT).
//! * Depletion-chain structure: OpenMC `openmc/deplete/chain.py` and the
//!   `chain_simple.xml` regression chain (`examples/pincell_depletion/`, MIT).
//! * Decay data: `openmc-endf-8-depletion-lib-a`/`-b` (ENDF/B-8, via OpenMC).
//! * Fission yields: `fission-yields-data` (ENDF/B-VIII.0 independent yields).
//!
//! This is an OUTRAM PARK translation, not the official OpenMC software.
//!
//! ## Units (raw `f64`, per this crate's convention)
//!
//! Following the crate-wide raw-`f64` convention (see the crate `CLAUDE.md`),
//! depletion quantities are plain `f64` with documented units:
//!
//! | Quantity | Unit |
//! |---|---|
//! | Number density `N` | atoms / (barn·cm) (or any consistent atom unit) |
//! | Decay constant, reaction rate | 1/s |
//! | Burnup-matrix coefficient | 1/s |
//! | Time step `dt` | s |
//! | One-group scalar flux | neutrons / (cm²·s) |
//! | One-group microscopic cross section | barn |

pub mod matrix;
pub mod cram;
pub mod chain;
pub mod operator;

pub use matrix::DepletionMatrix;

use std::collections::HashMap;

/// One-group microscopic **reaction rates** for a single nuclide, in `1/s`.
///
/// Each field is already flux-multiplied: `rate = flux * micro_xs`, i.e. the
/// per-atom probability per second of that reaction. These are the coefficients
/// [`chain::DepletionChain::build_matrix`] places into the burnup matrix.
///
/// Units: `1/s`. A value of `0.0` means the channel is absent or unresolved.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MicroRate {
    /// Radiative-capture `(n,gamma)` rate `1/s` — drives the capture daughter.
    pub gamma: f64,
    /// Fission rate `1/s` — removes the actinide and produces fission products
    /// via the chain's fission yields.
    pub fission: f64,
    /// `(n,2n)` rate `1/s` — removes one atom, producing the A-1 daughter.
    pub n2n: f64,
}

impl MicroRate {
    /// Total neutron-absorption-driven **removal** rate `1/s` for this nuclide
    /// (`gamma + fission + n2n`) — the amount subtracted on the burnup-matrix
    /// diagonal in addition to the decay constant.
    pub fn total_removal(&self) -> f64 {
        self.gamma + self.fission + self.n2n
    }
}

/// One-group reaction rates for a whole material, keyed by nuclide name
/// (`"U235"`, `"Xe135"`, …), plus the scalar flux they were derived from.
///
/// This is the transport → transmutation hand-off: the burnup operator fills it
/// from a transport solve (or a representative one-group estimate), and
/// [`chain::DepletionChain::build_matrix`] reads it to assemble `A`.
///
/// Units: `flux` in neutrons/(cm²·s); every [`MicroRate`] field in `1/s`.
#[derive(Debug, Clone, Default)]
pub struct ReactionRates {
    /// One-group scalar flux `neutrons/(cm²·s)` the rates were normalised to.
    pub flux: f64,
    /// Per-nuclide one-group reaction rates (`1/s`), keyed by nuclide name.
    pub micro_rates: HashMap<String, MicroRate>,
}

impl ReactionRates {
    /// An empty rate set at zero flux (pure-decay depletion — no transmutation).
    pub fn zero() -> Self {
        Self { flux: 0.0, micro_rates: HashMap::new() }
    }

    /// The reaction rates for `nuclide`, or the all-zero [`MicroRate`] if the
    /// nuclide has no tabulated rates (treated as inert under irradiation).
    pub fn rate_for(&self, nuclide: &str) -> MicroRate {
        self.micro_rates.get(nuclide).copied().unwrap_or_default()
    }

    /// Insert or overwrite the reaction rates for `nuclide`.
    pub fn set(&mut self, nuclide: &str, rate: MicroRate) {
        self.micro_rates.insert(nuclide.to_string(), rate);
    }
}
