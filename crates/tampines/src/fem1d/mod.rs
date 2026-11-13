//! 1-D continuous-Galerkin finite elements for HEM pipe flow, built to be
//! **compared against** the finite-volume default (gh:#1069).
//!
//! > Research, education and V&V only. AI draft, unreviewed. Not for nuclear
//! > facility operation, reactor control, licensing or safety-critical
//! > decisions. See `RESPONSIBLE_USE.md`.
//!
//! # What this answers
//!
//! *On the same physics, the same equation of state and the same boundary
//! conditions, how does the RELAP-7 style continuous-Galerkin discretisation
//! compare with this workspace's finite-volume HEM pipe?* Maintainer decision:
//! flow is finite volume (OpenFOAM style) **by default**; this module exists so
//! the two can be measured side by side, not to replace it. The comparison and
//! its results are `verification_and_validation/fem1d_vs_fv.md`, the tests
//! `tests/fem1d_vs_fv.rs`.
//!
//! # Entry points
//!
//! - [`Discretisation`]: `FiniteVolume` (the default) or
//!   `FiniteElement(Stabilisation)`.
//! - [`Pipe1d`]: one enum over both solvers, so one case runs on either.
//! - [`FemPipe`]: the finite-element solver itself.
//! - [`Stabilisation`], [`FemNumerics`], [`MassMatrix`].
//!
//! # The discretisation (RELAP-7 Theory Manual §5.1, Eqs 235–245)
//!
//! Linear Lagrange elements on the uniform mesh of the
//! [`HemPipeGeometry`]: `n + 1` nodes at `x_j = jL/n`, so element and
//! finite-volume cell have the same width. The unknowns are the nodal
//! area-weighted conserved variables `V = [ρA, ρuA, ρEA]` (Eqs 243–245); the
//! test functions are the same hat functions (Eq 238). The Galerkin weak form
//! (Eqs 239–242, frictionless, adiabatic, horizontal) is, for every node `i`,
//!
//! ```text
//! ∫ φ_i ∂V/∂t dx = ∫ [ρuA φ_i', ((ρu² + p)A φ_i' + p A' φ_i), ρuHA φ_i'] dx − [G·n̂ φ_i]_ends
//! ```
//!
//! evaluated with 2-point Gauss–Legendre quadrature per element (the line rule
//! of `farrer_park::quadrature`, exact to degree 3), the equation of state
//! closing `(ρ, e)` **at each quadrature point** from the interpolated `V`
//! and the analytic area there. `G` at an end is the boundary flux (below).
//!
//! **Mass matrix:** [`MassMatrix::Lumped`] by default (the row sums,
//! `∫φ_j`, so `M` is diagonal); [`MassMatrix::Consistent`] solves the
//! tridiagonal `∫φ_iφ_j` system by the Thomas algorithm. Either way the
//! global totals `Σ_j m_j V_j = ∫V dx` are what the scheme conserves.
//!
//! **Time stepping:** explicit SSP Runge–Kutta, by default
//! [`TimeIntegrator::Ssp33`] (`petir::ssprk`). Not SSP22, `HemPipe`'s default:
//! the Galerkin operator is a central difference, its eigenvalues lie on the
//! imaginary axis, and the two-stage scheme's stability region does not
//! contain any of the imaginary axis but the origin; the three-stage one
//! contains `|Im| ≤ √3`. Explicit rather than implicit (RELAP-7 uses BDF and
//! JFNK) because the comparison is against an explicit finite-volume scheme
//! at the same Courant number; an implicit FEM would compare time
//! integrators, not spatial discretisations. The step is
//! `Δt = C h / max(|u| + c)` over nodes and quadrature points, `C` the
//! classical Courant number ([`FemNumerics::courant`], default 0.4, which is
//! `HemPipe`'s default central Courant number 0.2 in classical terms), divided
//! by `√3` with the consistent mass matrix.
//!
//! # Stabilisation (§5.4), chosen by [`Stabilisation`]
//!
//! - [`Stabilisation::Supg`] — §5.4.1, Eqs 275–298: the weighting
//!   `Σ_K ∫ Aᵀ ∂W/∂x · τ R̃` with `τ = (h/2)|A|⁻¹`, `|A| = P|D|P⁻¹`, the
//!   quasi-linear residual `R̃ = V_t + A V_x + (F − AU)A' − S`. The flux
//!   Jacobian is Eq 290 with the pressure partials of a general EOS through
//!   its sound speed and Grüneisen coefficient (the latter by a forward
//!   difference of the EOS); the eigenvectors are normalised to stay finite at
//!   rest, where the manual's Eq 293 divides by zero (see `eigen.rs`). `V_t`
//!   inside `R̃` is the Galerkin-only nodal rate.
//! - [`Stabilisation::EntropyViscosity`] — §5.4.2, single phase (Eqs 330–338
//!   with `α = 1`, `l = 0`, `β = 0`, `κ = µ`): dissipative fluxes
//!   `f = κρ_x`, `g = ρκu_x + uf`, `h = κ(ρe)_x − (u²/2)f + ug` added in
//!   divergence form, `κ = min(κ_max, κ_e)` per element with the first-order
//!   bound `(h/2)(|u| + c)` (Eq 334) and the entropy-residual viscosity with
//!   the jump and the low-Mach denominator (Eqs 335–338). Frozen over a step;
//!   the residual's time derivatives are backward differences over the
//!   previous step, and the first step uses `κ_max`.
//! - [`Stabilisation::Lapidus`] — the manual lists it as an option (§5.4) but
//!   gives no equations. Implemented as the classical Lapidus viscosity
//!   `ν = C_L h² |∂u/∂x|` acting on `∂V/∂x` in divergence form, with
//!   `C_L =` [`FemNumerics::lapidus_coefficient`] (default 1, fixed before any
//!   run, not tuned). **No reference for the form or the coefficient is in the
//!   kovan corpus**; the attribution (Lapidus 1967; Löhner, Morgan &
//!   Zienkiewicz 1985 for its finite-element use) is from memory and not
//!   verified.
//! - [`Stabilisation::Galerkin`] — none: the ablation.
//!
//! # Boundary conditions
//!
//! Every [`HemBoundary`] is accepted. The end flux `G` is built as
//! `HemPipe` builds its boundary face (re-implemented, since that function is
//! private): `Wall` takes the end node with `u = 0`, so its mass and energy
//! fluxes are exactly zero; `Transmissive` the end node itself;
//! `FixedPressure`, `Reservoir` and `ChokedBreak` the same face states; `Flux`
//! the prescribed flux. The stabilisation's own boundary fluxes are taken as
//! zero (natural boundary condition), so they cannot add or remove mass.
//!
//! # Conservation
//!
//! Continuous Galerkin is not locally conservative (no cell has a flux
//! balance of its own), but because the hats sum to one, summing the nodal
//! equations cancels every interior term, Galerkin and stabilisation alike,
//! and leaves only the two end fluxes: **global** mass and energy are
//! conserved to round-off. `tests/fem1d_vs_fv.rs` measures it.
//!
//! # What is not here
//!
//! Wall friction, gravity, wall heat, two-phase (7-equation) entropy
//! viscosity, implicit time integration, higher-order elements. The PCICE
//! finite-volume solver (gh:#1068) is not part of [`Discretisation`] yet; the
//! comparison test leaves a marked hook for it.

mod eigen;
mod pipe;

pub use pipe::FemPipe;

use uom::si::f64::{Energy, Length, Mass, Ratio, Time};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;

use tampines_steam_tables::hem::{
    HemBoundary, HemEquationOfState, HemError, HemInitialCondition, HemNumerics, HemPipe,
    HemPipeGeometry, HemRunSummary, HemSnapshot, PipeEnd, TimeIntegrator,
};

/// Which stabilisation the continuous-Galerkin pipe adds (see the module
/// doc for each one's equations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stabilisation {
    /// No stabilisation: plain Galerkin, the ablation.
    Galerkin,
    /// Streamline-upwind Petrov–Galerkin, RELAP-7 §5.4.1.
    Supg,
    /// Entropy viscosity, RELAP-7 §5.4.2 (single phase).
    EntropyViscosity,
    /// Lapidus artificial viscosity (named in RELAP-7 §5.4, no equations
    /// there).
    Lapidus,
}

/// Lumped (diagonal, row-sum) or consistent (`∫φ_iφ_j`) mass matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MassMatrix {
    /// Row-sum lumped: `m_j = ∫φ_j`. The default.
    #[default]
    Lumped,
    /// Consistent tridiagonal, solved by the Thomas algorithm.
    Consistent,
}

/// Numerical settings of [`FemPipe`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FemNumerics {
    /// Classical Courant number `(|u| + c)Δt/h`, `> 0`. Default 0.4
    /// (`HemPipe`'s default central 0.2 in classical terms).
    pub courant: Ratio,
    /// Largest step allowed. Default 1 ms, as `HemPipe`.
    pub max_dt: Time,
    /// Default [`MassMatrix::Lumped`].
    pub mass_matrix: MassMatrix,
    /// Default [`TimeIntegrator::Ssp33`] (see the module doc for why not SSP22).
    pub time_integrator: TimeIntegrator,
    /// `C_L` of [`Stabilisation::Lapidus`], `≥ 0`. Default 1.
    pub lapidus_coefficient: Ratio,
}

impl Default for FemNumerics {
    fn default() -> Self {
        Self {
            courant: Ratio::new::<ratio>(0.4),
            max_dt: Time::new::<second>(1.0e-3),
            mass_matrix: MassMatrix::Lumped,
            time_integrator: TimeIntegrator::Ssp33,
            lapidus_coefficient: Ratio::new::<ratio>(1.0),
        }
    }
}

impl FemNumerics {
    fn validate(&self) -> Result<(), HemError> {
        let c = self.courant.get::<ratio>();
        let m = self.max_dt.get::<second>();
        let cl = self.lapidus_coefficient.get::<ratio>();
        if !(c.is_finite() && c > 0.0) || !(m.is_finite() && m > 0.0) || !(cl.is_finite() && cl >= 0.0) {
            return Err(HemError::InvalidInput(format!(
                "finite-element numerics need courant > 0 ({c}), max_dt > 0 ({m} s), lapidus_coefficient >= 0 ({cl})"
            )));
        }
        Ok(())
    }
}

/// Finite volume (the default) or continuous-Galerkin finite elements, for
/// the same physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Discretisation {
    /// `tampines_steam_tables::hem::HemPipe`, KNP central-upwind. The default.
    #[default]
    FiniteVolume,
    /// [`FemPipe`] with the given stabilisation.
    FiniteElement(Stabilisation),
}

impl Discretisation {
    /// Where the solution lives on `geometry`: cell centres (finite volume,
    /// `n` points) or nodes (finite element, `n + 1`). A
    /// [`HemInitialCondition::PerCell`] needs one state per point.
    pub fn solution_points(&self, geometry: &HemPipeGeometry) -> Vec<Length> {
        let n = geometry.n_cells();
        match self {
            Discretisation::FiniteVolume => (0..n).map(|i| geometry.cell_centre(i)).collect(),
            Discretisation::FiniteElement(_) => {
                let h = geometry.length().get::<meter>() / n as f64;
                (0..=n).map(|j| Length::new::<meter>(j as f64 * h)).collect()
            }
        }
    }
}

/// The numerics of both solvers; [`Pipe1d::new`] uses the one its
/// [`Discretisation`] picks.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pipe1dNumerics {
    /// For [`Discretisation::FiniteVolume`].
    pub finite_volume: HemNumerics,
    /// For [`Discretisation::FiniteElement`].
    pub finite_element: FemNumerics,
}

/// One HEM pipe on either discretisation: an enum, not a trait object.
#[derive(Debug, Clone)]
pub enum Pipe1d<E: HemEquationOfState> {
    /// The finite-volume `HemPipe`.
    FiniteVolume(HemPipe<E>),
    /// The continuous-Galerkin [`FemPipe`].
    FiniteElement(FemPipe<E>),
}

impl<E: HemEquationOfState> Pipe1d<E> {
    /// Build the pipe on `discretisation`. Both ends start as `Wall`.
    ///
    /// # Errors
    ///
    /// As `HemPipe::new` or [`FemPipe::new`].
    pub fn new(
        geometry: HemPipeGeometry,
        eos: E,
        initial: HemInitialCondition,
        discretisation: Discretisation,
        numerics: Pipe1dNumerics,
    ) -> Result<Self, HemError> {
        Ok(match discretisation {
            Discretisation::FiniteVolume => {
                let mut p = HemPipe::new(geometry, eos, initial, numerics.finite_volume)?;
                // HemPipe's own default ends are walls too; set them
                // explicitly so the two solvers start identically.
                p.set_boundary(PipeEnd::Inlet, HemBoundary::Wall)?;
                p.set_boundary(PipeEnd::Outlet, HemBoundary::Wall)?;
                Pipe1d::FiniteVolume(p)
            }
            Discretisation::FiniteElement(st) => {
                Pipe1d::FiniteElement(FemPipe::new(geometry, eos, initial, st, numerics.finite_element)?)
            }
        })
    }

    /// The discretisation this pipe runs.
    pub fn discretisation(&self) -> Discretisation {
        match self {
            Pipe1d::FiniteVolume(_) => Discretisation::FiniteVolume,
            Pipe1d::FiniteElement(p) => Discretisation::FiniteElement(p.stabilisation()),
        }
    }

    /// Ablate the finite-volume wall friction (on by default for a closure
    /// with a viscosity, such as IF97). The finite-element path carries no
    /// friction, so this is a no-op there; call it to compare the same
    /// physics on both.
    pub fn without_wall_friction(self) -> Self {
        match self {
            Pipe1d::FiniteVolume(p) => Pipe1d::FiniteVolume(p.without_wall_friction()),
            fe => fe,
        }
    }

    /// Set the boundary condition at `end`.
    ///
    /// # Errors
    ///
    /// As the underlying solver.
    pub fn set_boundary(&mut self, end: PipeEnd, boundary: HemBoundary) -> Result<(), HemError> {
        match self {
            Pipe1d::FiniteVolume(p) => p.set_boundary(end, boundary),
            Pipe1d::FiniteElement(p) => p.set_boundary(end, boundary),
        }
    }

    /// Advance to exactly `t_end`. `max_courant` is the central Courant
    /// number for finite volume, the classical one for finite elements.
    ///
    /// # Errors
    ///
    /// As the underlying solver.
    pub fn run_until(&mut self, t_end: Time) -> Result<HemRunSummary, HemError> {
        match self {
            Pipe1d::FiniteVolume(p) => p.run_until(t_end),
            Pipe1d::FiniteElement(p) => p.run_until(t_end),
        }
    }

    /// Simulation time.
    pub fn time(&self) -> Time {
        match self {
            Pipe1d::FiniteVolume(p) => p.time(),
            Pipe1d::FiniteElement(p) => p.time(),
        }
    }

    /// Every solution point's state (cells or nodes).
    ///
    /// # Errors
    ///
    /// As [`FemPipe::snapshot`]; never for finite volume.
    pub fn snapshot(&self) -> Result<HemSnapshot, HemError> {
        match self {
            Pipe1d::FiniteVolume(p) => Ok(p.snapshot()),
            Pipe1d::FiniteElement(p) => p.snapshot(),
        }
    }

    /// Mass in the pipe.
    pub fn total_mass(&self) -> Mass {
        match self {
            Pipe1d::FiniteVolume(p) => p.total_mass(),
            Pipe1d::FiniteElement(p) => p.total_mass(),
        }
    }

    /// Total energy in the pipe.
    pub fn total_energy(&self) -> Energy {
        match self {
            Pipe1d::FiniteVolume(p) => p.total_energy(),
            Pipe1d::FiniteElement(p) => p.total_energy(),
        }
    }

    /// Mass that has left through `end` since `t = 0`.
    pub fn cumulative_mass_outflow(&self, end: PipeEnd) -> Mass {
        match self {
            Pipe1d::FiniteVolume(p) => p.cumulative_mass_outflow(end),
            Pipe1d::FiniteElement(p) => p.cumulative_mass_outflow(end),
        }
    }

    /// Total energy that has left through `end` since `t = 0`.
    pub fn cumulative_energy_outflow(&self, end: PipeEnd) -> Energy {
        match self {
            Pipe1d::FiniteVolume(p) => p.cumulative_energy_outflow(end),
            Pipe1d::FiniteElement(p) => p.cumulative_energy_outflow(end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tampines_steam_tables::hem::{IdealGasHem, PrimitiveState};
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, MassDensity, Pressure, Velocity};
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::pascal;
    use uom::si::velocity::meter_per_second;

    /// **Finite volume is the default; the enum builds both; bad numerics are
    /// refused; solution points are cells or nodes.**
    #[test]
    fn finite_volume_is_the_default_and_both_build() {
        assert_eq!(Discretisation::default(), Discretisation::FiniteVolume);
        assert_eq!(FemNumerics::default().time_integrator, TimeIntegrator::Ssp33);
        assert_eq!(FemNumerics::default().mass_matrix, MassMatrix::Lumped);
        let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4).unwrap();
        let init = HemInitialCondition::Uniform(PrimitiveState::PressureDensity {
            pressure: Pressure::new::<pascal>(1e5),
            density: MassDensity::new::<kilogram_per_cubic_meter>(1.0),
            velocity: Velocity::new::<meter_per_second>(0.0),
        });
        assert_eq!(Discretisation::FiniteVolume.solution_points(&geo).len(), 4);
        assert_eq!(Discretisation::FiniteElement(Stabilisation::Supg).solution_points(&geo).len(), 5);
        for d in [Discretisation::FiniteVolume, Discretisation::FiniteElement(Stabilisation::Lapidus)] {
            let mut p = Pipe1d::new(geo, IdealGasHem::air(), init.clone(), d, Pipe1dNumerics::default())
                .unwrap()
                .without_wall_friction();
            assert_eq!(p.discretisation(), d);
            p.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive).unwrap();
            let m0 = p.total_mass().value;
            p.run_until(Time::new::<second>(1e-4)).unwrap();
            assert!((p.time().value - 1e-4).abs() < 1e-18);
            assert!((p.total_mass().value / m0 - 1.0).abs() < 1e-12);
            assert!(p.total_energy().value > 0.0);
            assert_eq!(p.snapshot().unwrap().cells.len(), p.discretisation().solution_points(&geo).len());
            assert!(p.cumulative_mass_outflow(PipeEnd::Outlet).value.abs() < 1e-9);
            assert!(p.cumulative_energy_outflow(PipeEnd::Inlet).value.abs() < 1e-6);
        }
        let mut bad = FemNumerics::default();
        bad.courant = Ratio::new::<ratio>(0.0);
        assert!(Pipe1d::new(geo, IdealGasHem::air(), init, Discretisation::FiniteElement(Stabilisation::Supg), Pipe1dNumerics { finite_element: bad, ..Default::default() }).is_err());
    }
}
