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
//! Root-finding uses the [`roots`] crate's Brent implementation
//! (BSD-2-Clause licensed; already an OUTRAM PARK workspace dependency) --
//! reused rather than reimplemented, since Brent's method itself is
//! standard numerical-methods machinery, not DWSIM-specific.

use roots::{find_root_brent, SearchError, SimpleConvergency};
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
pub fn solve_intercell_mass_flow<F>(
    p_upstream: Pressure,
    p_downstream: Pressure,
    mut pressure_drop: F,
    w_bracket: (MassRate, MassRate),
    tolerance: Pressure,
    max_iterations: usize,
) -> Result<MassRate, SearchError>
where
    F: FnMut(MassRate) -> Pressure,
{
    let target_dp = (p_upstream - p_downstream).get::<pascal>();
    let mut convergency = SimpleConvergency {
        eps: tolerance.get::<pascal>(),
        max_iter: max_iterations,
    };
    let w = find_root_brent(
        w_bracket.0.get::<kilogram_per_second>(),
        w_bracket.1.get::<kilogram_per_second>(),
        |w_si| {
            pressure_drop(MassRate::new::<kilogram_per_second>(w_si)).get::<pascal>() - target_dp
        },
        &mut convergency,
    )?;
    Ok(MassRate::new::<kilogram_per_second>(w))
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
