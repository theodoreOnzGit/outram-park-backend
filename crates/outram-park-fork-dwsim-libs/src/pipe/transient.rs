//! Transient (dynamic) pipe network: a chain of accumulation-volume cells
//! with inter-cell mass flow solved by Brent root-finding.
//!
//! Ported from DWSIM `UnitOperations/Pipe.vb`'s `RunDynamicModel()`, which
//! represents a pipe as a chain of "accumulation stream" cells (one per
//! length increment) and, each substep, solves for the inter-cell mass flow
//! that reconciles the pressure difference between adjacent cells via a
//! Brent root-find on `Pdrop_transition - dpt(mass_flow) = 0` (falling back
//! to a least-squares solve if Brent doesn't converge -- not replicated
//! here, see below). After the mass transfer, DWSIM updates each cell's
//! pressure via a volume-temperature flash -- this transient pipe port is
//! deliberately **not** coupled to the crate's flash kernel (the `(V, T) -> P`
//! step is left to the caller), so [`PipeCell`] only does the mass-balance
//! bookkeeping; a caller (e.g. `tampines`, or the crate's own
//! [`crate::thermo`] flash) does the `(V, T) -> P` update itself after calling
//! [`solve_intercell_mass_flow`].
//!
//! ~~Root-finding uses the [`roots`] crate's Brent implementation
//! (BSD-2-Clause licensed; already an OUTRAM PARK workspace dependency) --
//! reused rather than reimplemented, since Brent's method itself is
//! standard numerical-methods machinery, not DWSIM-specific.~~
//! **CORRECTED 2026-10-20 (gh:#991, epic #990)** -- root-finding now uses
//! [`petir::roots::BracketingSolver`] with [`BracketingMethod::Brent`], PETIR's
//! port of GSL `roots/brent.c` (verified iterate-for-iterate against compiled
//! GSL 2.8), in place of the `roots` crate (Vorotilov, 0.0.8). How the old
//! stopping rule maps onto the new one, and why the new one is at least as
//! strict, is set out on [`solve_intercell_mass_flow`].

use core::cell::RefCell;

use petir::roots::{test_interval, test_residual, BracketingMethod, BracketingSolver};
use petir::PetirError;
use uom::si::f64::{Mass, MassRate, Pressure, Time, Volume};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;

/// One control volume in a transient pipe network -- fixed geometric
/// volume, tracked mass. Pressure/temperature/density are the caller's
/// responsibility (via a `(V, T) -> P` flash after each
/// [`solve_intercell_mass_flow`] call), since this crate has no
/// property-package access of its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeCell {
    /// Fixed geometric volume of this cell.
    pub volume: Volume,
    /// Current mass held in this cell.
    pub mass: Mass,
}

impl PipeCell {
    /// A new cell with the given volume and initial mass.
    pub fn new(volume: Volume, initial_mass: Mass) -> Self {
        Self {
            volume,
            mass: initial_mass,
        }
    }

    /// Mass density implied by this cell's current mass and (fixed) volume.
    pub fn density(&self) -> uom::si::f64::MassDensity {
        self.mass / self.volume
    }

    /// Advance this cell's mass balance over `dt`, given the mass flow rate
    /// into the cell from its upstream neighbour and out of the cell to its
    /// downstream neighbour (both signed: negative means flow the other
    /// way). Does not touch pressure/temperature -- call the caller's own
    /// `(V, T) -> P` flash afterwards.
    pub fn advance_mass_balance(&mut self, inflow: MassRate, outflow: MassRate, dt: Time) {
        self.mass += (inflow - outflow) * dt;
    }
}

/// Solve for the mass flow rate `w` between two adjacent cells such that a
/// flow correlation's predicted pressure drop `pressure_drop(w)` equals the
/// actual pressure difference `p_upstream - p_downstream` between them --
/// DWSIM's `Pdrop_transition - dpt(mass_flow) = 0`.
///
/// `pressure_drop` should be monotonically increasing in `w` over the
/// search bracket `w_bracket = (w_min, w_max)` (as for any standard
/// friction correlation -- e.g. [`super::friction_factor::frictional_pressure_drop`]
/// composed with a mass-flow-to-velocity conversion) for Brent's method to
/// bracket a root correctly; `w_bracket` can include negative values to
/// allow for reverse flow.
///
/// # Convergence (gh:#991): what `tolerance` means, before and after
///
/// `tolerance` is a **pressure**: the solve stops when the residual
/// `|pressure_drop(w) - (p_upstream - p_downstream)|` at the current best
/// estimate is below it (GSL `gsl_root_test_residual`), or when the bracket on
/// `w` has collapsed to four machine epsilons of `|w|` (GSL
/// `gsl_root_test_interval` with `epsabs = 0`, `epsrel = 4 * f64::EPSILON`),
/// beyond which `f64` cannot place the root more closely. Otherwise it fails
/// after `max_iterations` Brent steps.
///
/// | | `roots` 0.0.8 (until 2026-10-20) | PETIR (now) |
/// |---|---|---|
/// | residual exit | `\|f(a)\| < tol` or `\|f(b)\| < tol`, at either bracket end | `\|f(root)\| < tol` at GSL's best estimate (one extra evaluation of `pressure_drop` per step) |
/// | bracket exit | `\|a - b\| < tol`, i.e. **the pressure tolerance in Pa compared against a mass-flow width in kg/s** | only at the `f64` floor, `\|w_upper - w_lower\| < 4 eps_machine * min(\|w_lower\|, \|w_upper\|)` |
/// | cap | `max_iterations`, else `SearchError::NoConvergency` | `max_iterations`, else [`PetirError::MaxIterations`] |
/// | no sign change | `SearchError::NoBracketing` | [`PetirError::Invalid`] |
/// | non-finite `pressure_drop` | carried on | [`PetirError::Domain`] |
///
/// **Why the new rule is at least as strict.** The residual exit uses the same
/// threshold. The old bracket exit was dimensionally inconsistent: with
/// `tolerance = 1 Pa` it accepted any bracket narrower than 1 kg/s, whatever
/// the residual there. It is replaced by the floating-point floor, which
/// accepts only a bracket that cannot be narrowed further. Whenever PETIR
/// stops on that floor, the old bracket test would have passed too: the floor
/// width `4 eps_machine |w|` is below the numeric tolerance for any
/// `|w| < tol / (4 eps_machine)`, about 1e15 kg/s at `tol = 1 Pa`.
/// The `w_bracket` endpoints may be given in either order.
pub fn solve_intercell_mass_flow<F>(
    p_upstream: Pressure,
    p_downstream: Pressure,
    pressure_drop: F,
    w_bracket: (MassRate, MassRate),
    tolerance: Pressure,
    max_iterations: usize,
) -> Result<MassRate, PetirError>
where
    F: FnMut(MassRate) -> Pressure,
{
    let target_dp = (p_upstream - p_downstream).get::<pascal>();
    let tol_pa = tolerance.get::<pascal>();
    // PETIR's solver takes `Fn`; the caller's correlation may be `FnMut`.
    let pressure_drop = RefCell::new(pressure_drop);
    let residual = |w_si: f64| -> f64 {
        (pressure_drop.borrow_mut())(MassRate::new::<kilogram_per_second>(w_si)).get::<pascal>()
            - target_dp
    };
    let w0 = w_bracket.0.get::<kilogram_per_second>();
    let w1 = w_bracket.1.get::<kilogram_per_second>();
    let (w_lower, w_upper) = if w0 <= w1 { (w0, w1) } else { (w1, w0) };
    let mut solver = BracketingSolver::new(BracketingMethod::Brent, &residual, w_lower, w_upper)?;
    for _ in 0..max_iterations {
        solver.iterate()?;
        let w = solver.root();
        if test_residual(residual(w), tol_pa)?.is_converged()
            || test_interval(solver.x_lower(), solver.x_upper(), 0.0, 4.0 * f64::EPSILON)?
                .is_converged()
        {
            return Ok(MassRate::new::<kilogram_per_second>(w));
        }
    }
    Err(PetirError::MaxIterations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass::kilogram as mass_kilogram;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::time::second as time_second;
    use uom::si::volume::cubic_meter;

    #[test]
    fn solve_intercell_mass_flow_finds_root_of_linear_resistance() {
        // Linear "resistance": dP(w) = R * w, R = 1e5 Pa / (kg/s)
        let r = 1.0e5;
        let pressure_drop =
            |w: MassRate| Pressure::new::<pascal>(r * w.get::<kilogram_per_second>());
        let p_up = Pressure::new::<pascal>(200_000.0);
        let p_down = Pressure::new::<pascal>(150_000.0);
        let w = solve_intercell_mass_flow(
            p_up,
            p_down,
            pressure_drop,
            (
                MassRate::new::<kilogram_per_second>(-10.0),
                MassRate::new::<kilogram_per_second>(10.0),
            ),
            Pressure::new::<pascal>(1.0),
            100,
        )
        .unwrap();
        // Expect w = (p_up - p_down)/R = 50000/1e5 = 0.5 kg/s
        assert!((w.get::<kilogram_per_second>() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn solve_intercell_mass_flow_handles_reverse_flow() {
        let r = 1.0e5;
        let pressure_drop =
            |w: MassRate| Pressure::new::<pascal>(r * w.get::<kilogram_per_second>());
        let p_up = Pressure::new::<pascal>(150_000.0);
        let p_down = Pressure::new::<pascal>(200_000.0);
        let w = solve_intercell_mass_flow(
            p_up,
            p_down,
            pressure_drop,
            (
                MassRate::new::<kilogram_per_second>(-10.0),
                MassRate::new::<kilogram_per_second>(10.0),
            ),
            Pressure::new::<pascal>(1.0),
            100,
        )
        .unwrap();
        assert!(w.get::<kilogram_per_second>() < 0.0);
    }

    /// The residual test, not the bracket width, decides convergence: with a
    /// steep correlation (1e9 Pa per kg/s) and `tolerance = 1 Pa`, the root
    /// must be found to within 1e-9 kg/s. Under `roots` the same call could
    /// stop once the bracket was narrower than "1" (kg/s, read from the Pa
    /// tolerance), and was never checked here.
    #[test]
    fn solve_intercell_mass_flow_tolerance_is_a_pressure_residual() {
        let r = 1.0e9;
        let pressure_drop =
            |w: MassRate| Pressure::new::<pascal>(r * w.get::<kilogram_per_second>());
        let p_up = Pressure::new::<pascal>(200_000.0);
        let p_down = Pressure::new::<pascal>(150_000.0);
        let tol = 1.0;
        let w = solve_intercell_mass_flow(
            p_up,
            p_down,
            pressure_drop,
            (
                MassRate::new::<kilogram_per_second>(10.0),
                MassRate::new::<kilogram_per_second>(-10.0),
            ),
            Pressure::new::<pascal>(tol),
            100,
        )
        .unwrap();
        let resid = r * w.get::<kilogram_per_second>() - 50_000.0;
        assert!(resid.abs() < tol, "residual {resid} Pa");
    }

    #[test]
    fn solve_intercell_mass_flow_rejects_an_unbracketed_interval() {
        let pressure_drop = |w: MassRate| Pressure::new::<pascal>(w.get::<kilogram_per_second>());
        let err = solve_intercell_mass_flow(
            Pressure::new::<pascal>(200_000.0),
            Pressure::new::<pascal>(150_000.0),
            pressure_drop,
            (
                MassRate::new::<kilogram_per_second>(-10.0),
                MassRate::new::<kilogram_per_second>(10.0),
            ),
            Pressure::new::<pascal>(1.0),
            100,
        )
        .unwrap_err();
        assert_eq!(err, PetirError::Invalid);
    }

    #[test]
    fn pipe_cell_mass_balance_accumulates_correctly() {
        let mut cell = PipeCell::new(
            Volume::new::<cubic_meter>(1.0),
            Mass::new::<mass_kilogram>(500.0),
        );
        let inflow = MassRate::new::<kilogram_per_second>(2.0);
        let outflow = MassRate::new::<kilogram_per_second>(1.0);
        cell.advance_mass_balance(inflow, outflow, Time::new::<time_second>(10.0));
        // net inflow of 1 kg/s for 10s = +10 kg
        assert!((cell.mass.get::<mass_kilogram>() - 510.0).abs() < 1e-9);
    }

    #[test]
    fn pipe_cell_density_matches_mass_over_volume() {
        let cell = PipeCell::new(
            Volume::new::<cubic_meter>(2.0),
            Mass::new::<mass_kilogram>(1996.0),
        );
        assert!((cell.density().get::<kilogram_per_cubic_meter>() - 998.0).abs() < 1e-9);
    }
}
