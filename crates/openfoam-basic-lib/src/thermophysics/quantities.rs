use uom::si::{ISQ, Quantity, SI};
use uom::typenum::{N2, P2, Z0};

/// Compressibility ψ = ∂ρ/∂p|_T  —  SI units: s²/m²  (L⁻²·T²)
///
/// Computed as `MassDensity / Pressure` via uom operator arithmetic; this type
/// alias names the resulting quantity so trait signatures are readable.
pub type Compressibility = Quantity<ISQ<N2, Z0, P2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;
