/// Field-level fluid thermodynamic interface (Layer 4).
///
/// Mirrors `Foam::fluidThermo` / `Foam::psiThermo` / `Foam::rhoThermo` from
/// `src/thermophysicalModels/basic/`.
///
/// Each struct owns the primary thermodynamic fields (`p`, `T`, `he`, `rho`,
/// `psi`) and uses a per-species `TransportModel` (from Layer 1h) to evaluate
/// properties cell-by-cell.
pub mod traits;
pub mod psi_thermo;
pub mod rho_thermo;
pub mod solid_thermo;

pub use traits::FluidThermo;
pub use psi_thermo::PsiThermo;
pub use rho_thermo::RhoThermo;
pub use solid_thermo::{SolidThermo, ConstSolidThermo};
