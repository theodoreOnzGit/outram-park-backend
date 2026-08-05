//! Fuel-pin radial geometry and the material property correlations attached to
//! it.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | the `geometry.fuel` block of `neacrpa2.m` / `neacrpa2t.m` / `neacrpa1t.m` / `neacrpd1.m`, and the `geometry.fuel.rhocp` block of the transient cases |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # Units
//!
//! Radii and lengths \[cm\], areas \[cm²\], volumes per unit length \[cm²\],
//! thermal conductivity \[W/cm/K\], gap conductance \[W/cm²/K\], volumetric
//! heat capacity \[J/cm³/K\], temperature \[K\].

use super::params::FuelDiscretisation;

/// Which of the three radial materials a fuel-pin node is.
///
/// MATLAB `geometry.fuel.whichk`, an array of `1` (fuel), `0` (gap) and `2`
/// (cladding). The numbering is not an ordering: it is an index into the
/// `geometry.fuel.tcon` cell array, with the gap handled by a separate branch
/// that reads `tcon{end}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RodRegion {
    /// Fuel pellet. MATLAB `whichk == 1`.
    Fuel,
    /// Pellet–cladding gap, modelled as a conductance rather than a
    /// conducting solid. MATLAB `whichk == 0`.
    Gap,
    /// Cladding. MATLAB `whichk == 2`.
    Clad,
}

impl RodRegion {
    /// The MATLAB `whichk` code for this region.
    #[must_use]
    pub const fn matlab_code(self) -> usize {
        match self {
            Self::Gap => 0,
            Self::Fuel => 1,
            Self::Clad => 2,
        }
    }
}

/// A thermal-conductivity correlation, or the gap conductance.
///
/// MATLAB stores these as anonymous function handles in the
/// `geometry.fuel.tcon` cell array. Function handles have no place in the
/// port — the workspace Rust rules forbid trait objects and boxed closures —
/// so the closed set of correlations is an enum, dispatched by `match`.
///
/// # Units differ between variants
///
/// [`UraniumDioxide`](Self::UraniumDioxide) and [`Zircaloy`](Self::Zircaloy)
/// return a conductivity \[W/cm/K\]; [`GapConductance`](Self::GapConductance)
/// returns a **conductance** \[W/cm²/K\]. That mismatch is in the reference:
/// `fuelrodheat_1dcylnd.m` multiplies `tcon{end}` by a radius to recover a
/// conductivity-like quantity (`kplus = tcon{end}*Ctr(ir+1)`). Recorded, not
/// repaired.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalConductivity {
    /// UO₂ pellet: `(1.05 + 2150/(T - 73.15))/100` \[W/cm/K\].
    ///
    /// Valid for `T > 73.15 K`; it is singular at `T = 73.15 K` and negative
    /// below, neither of which a reactor calculation reaches. Both NEACRP
    /// cases use this same correlation.
    UraniumDioxide,
    /// Zircaloy cladding:
    /// `(7.51 + 2.09e-2 T - 1.45e-5 T² + 7.67e-9 T³)/100` \[W/cm/K\].
    Zircaloy,
    /// Constant pellet–cladding gap conductance \[W/cm²/K\].
    ///
    /// `1` for the NEACRP PWR cases, `0.35` for the BWR case — both taken
    /// from the benchmark specification.
    GapConductance(f64),
}

impl ThermalConductivity {
    /// Evaluate at temperature `t` \[K\].
    ///
    /// For [`GapConductance`](Self::GapConductance) the temperature is ignored
    /// and the constant returned.
    #[must_use]
    pub fn evaluate(self, t: f64) -> f64 {
        match self {
            Self::UraniumDioxide => (1.05 + 2150.0 / (t - 73.15)) / 100.0,
            Self::Zircaloy => (7.51 + 2.09e-2 * t - 1.45e-5 * t * t + 7.67e-9 * t * t * t) / 100.0,
            Self::GapConductance(h) => h,
        }
    }
}

/// A volumetric-heat-capacity correlation, `rho*cp` \[J/cm³/K\].
///
/// MATLAB `geometry.fuel.rhocp`, set only by the transient cases
/// (`neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1t.m` — all three use the same two
/// correlations). Indexed like `tcon`: entry 1 is fuel, entry 2 cladding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricHeatCapacity {
    /// UO₂ pellet:
    /// `10.412*(1 - 0.01248)*(162.3 + 0.3038 T - 2.391e-4 T² + 6.404e-8 T³)/1000`
    /// \[J/cm³/K\].
    ///
    /// 10.412 g/cm³ is the undished UO₂ density, reduced by the 1.248 % pellet
    /// dishing; the bracket is the specific heat \[J/kg/K\] from NEACRP-L-335
    /// §2.7, and the `/1000` converts g·J/(kg·cm³·K) to J/cm³/K.
    UraniumDioxide,
    /// Zircaloy cladding: `6.6*(252.54 + 0.11474 T)/1000` \[J/cm³/K\], with
    /// 6.6 g/cm³ the Zircaloy-4 density.
    Zircaloy,
}

impl VolumetricHeatCapacity {
    /// Evaluate at temperature `t` \[K\].
    #[must_use]
    pub fn evaluate(self, t: f64) -> f64 {
        match self {
            Self::UraniumDioxide => {
                10.412
                    * (1.0 - 0.01248)
                    * (162.3 + 0.3038 * t - 2.391e-4 * t * t + 6.404e-8 * t * t * t)
                    / 1000.0
            }
            Self::Zircaloy => 6.6 * (252.54 + 0.11474 * t) / 1000.0,
        }
    }
}

/// Radial geometry of one fuel pin, plus its subchannel.
///
/// MATLAB `geometry.fuel`. Built identically by all four NEACRP case
/// constructors; only the dimensions and the gap conductance differ.
#[derive(Debug, Clone, PartialEq)]
pub struct FuelGeometry {
    /// Pellet radius \[cm\]. MATLAB `geometry.fuel.fuelrad`.
    pub fuel_radius: f64,
    /// Radial gap thickness \[cm\]. MATLAB `geometry.fuel.fuelgap`.
    pub gap_thickness: f64,
    /// Cladding thickness \[cm\]. MATLAB `geometry.fuel.clad`.
    pub clad_thickness: f64,
    /// Rod pitch \[cm\]. MATLAB `geometry.fuel.pitch`.
    pub pitch: f64,
    /// Weight on the pellet surface temperature in the effective Doppler
    /// temperature \[dimensionless\]. MATLAB `geometry.fuel.doppleralpha`;
    /// `0.7` in both cases.
    pub doppler_alpha: f64,
    /// Outer rod radius, `fuel + gap + clad` \[cm\]. MATLAB
    /// `geometry.fuel.Rtot`.
    pub outer_radius: f64,
    /// Radial thickness of each node \[cm\], innermost first. MATLAB
    /// `geometry.fuel.Lr`.
    pub node_thickness: Vec<f64>,
    /// Outer-edge-referenced centre radius of each node \[cm\], computed as
    /// `sum(Lr(1:ir)) - 0.5*Lr(ir)`. MATLAB `geometry.fuel.Ctr`.
    pub node_center: Vec<f64>,
    /// Per-node cross-sectional area \[cm²\] (a volume per unit rod length).
    /// MATLAB `geometry.fuel.Vi`.
    ///
    /// # Unfinished in the reference — this is wrong as written
    ///
    /// The MATLAB computes, for `i >= 2`,
    ///
    /// ```text
    /// rminus = sum(geometry.fuel.Lr(i-1));   % a scalar: just Lr(i-1)
    /// rplus  = sum(geometry.fuel.Lr(i));     % a scalar: just Lr(i)
    /// Vi(i)  = pi*(rplus^2 - rminus^2);
    /// ```
    ///
    /// `sum` of a single element is that element, so these are node
    /// *thicknesses*, not cumulative radii — almost certainly a typo for
    /// `sum(Lr(1:i-1))` and `sum(Lr(1:i))`. Because every pellet node has the
    /// same thickness, the consequence is that **`Vi(2:fueln)` is exactly
    /// zero** and the gap/clad entries are meaningless. `Vi(1)` is correct.
    ///
    /// Reproduced exactly as written, per `docs/bedok-port-scoping.md` §1.0:
    /// repairing it here would make a later disagreement with the benchmark
    /// impossible to attribute. [`node_area_corrected`](Self::node_area_corrected)
    /// provides the annulus areas the formula was evidently reaching for, for
    /// comparison only — nothing in the reference path uses it.
    pub node_area: Vec<f64>,
    /// Material of each radial node, innermost first. MATLAB
    /// `geometry.fuel.whichk`.
    pub region: Vec<RodRegion>,
    /// Coolant flow area per rod \[cm²\], `pitch² - pi*Rtot²`. MATLAB
    /// `geometry.fuel.subarea`.
    pub subchannel_area: f64,
    /// Subchannel hydraulic diameter \[cm\]. MATLAB `geometry.fuel.hydia`,
    /// computed as `4*subarea/(2*pi*Rtot + 4*pitch - 8*Rtot)`.
    ///
    /// # Questionable in the reference
    ///
    /// The usual wetted perimeter of a square-pitch subchannel is the rod
    /// circumference alone, `2*pi*Rtot`. The extra `4*pitch - 8*Rtot` adds the
    /// square cell's perimeter minus the rod's projected width, which has no
    /// standard justification for an interior subchannel. Recorded, not
    /// changed.
    pub hydraulic_diameter: f64,
    /// Thermal conductivity per `whichk` code, in the MATLAB's cell order:
    /// entry 0 is fuel (`tcon{1}`), entry 1 cladding (`tcon{2}`), entry 2 the
    /// gap conductance (`tcon{3}`, read as `tcon{end}`).
    pub conductivity: [ThermalConductivity; 3],
    /// Volumetric heat capacity, entry 0 fuel and entry 1 cladding. MATLAB
    /// `geometry.fuel.rhocp`. Empty for a steady-only case, which does not set
    /// the field at all.
    pub heat_capacity: Vec<VolumetricHeatCapacity>,
}

impl FuelGeometry {
    /// Build the radial mesh and derived quantities from the pin dimensions.
    ///
    /// Rust translation of the `geometry.fuel` block shared by all four NEACRP
    /// case constructors. Reproduces the `node_area` defect documented on that
    /// field.
    ///
    /// # Parameters
    ///
    /// - `fuel_radius`, `gap_thickness`, `clad_thickness`, `pitch` \[cm\]
    /// - `gap_conductance` \[W/cm²/K\] — `tcon{3}`
    /// - `doppler_alpha` \[dimensionless\]
    #[must_use]
    pub fn build(
        discretisation: FuelDiscretisation,
        fuel_radius: f64,
        gap_thickness: f64,
        clad_thickness: f64,
        pitch: f64,
        doppler_alpha: f64,
        gap_conductance: f64,
    ) -> Self {
        let FuelDiscretisation {
            gap_nodes,
            clad_nodes,
            fuel_nodes,
            total_nodes,
        } = discretisation;

        let outer_radius = fuel_radius + gap_thickness + clad_thickness;

        let mut node_thickness = vec![0.0f64; total_nodes];
        for t in node_thickness.iter_mut().take(fuel_nodes) {
            *t = fuel_radius / fuel_nodes as f64;
        }
        for t in node_thickness.iter_mut().skip(fuel_nodes).take(gap_nodes) {
            *t = gap_thickness / gap_nodes as f64;
        }
        for t in node_thickness
            .iter_mut()
            .skip(fuel_nodes + gap_nodes)
            .take(clad_nodes)
        {
            *t = clad_thickness / clad_nodes as f64;
        }

        // Ctr(ir) = sum(Lr(1:ir)) - 0.5*Lr(ir) — a genuine cumulative radius.
        let mut node_center = vec![0.0f64; total_nodes];
        let mut cumulative = 0.0;
        for (ir, c) in node_center.iter_mut().enumerate() {
            cumulative += node_thickness[ir];
            *c = cumulative - 0.5 * node_thickness[ir];
        }

        // Vi — reproduced with the reference's `sum` of a single element. See
        // the field docs; Vi(2..) is identically zero for a uniform pellet.
        let mut node_area = vec![0.0f64; total_nodes];
        if total_nodes > 0 {
            node_area[0] = std::f64::consts::PI * node_thickness[0] * node_thickness[0];
        }
        for i in 1..total_nodes {
            let r_minus = node_thickness[i - 1];
            let r_plus = node_thickness[i];
            node_area[i] = std::f64::consts::PI * (r_plus * r_plus - r_minus * r_minus);
        }

        let region: Vec<RodRegion> = (0..total_nodes)
            .map(|i| {
                // MATLAB indexes from 1: gap is fueln < i <= fueln+gapn.
                let one_based = i + 1;
                if one_based > fuel_nodes && one_based <= fuel_nodes + gap_nodes {
                    RodRegion::Gap
                } else if one_based > fuel_nodes + gap_nodes {
                    RodRegion::Clad
                } else {
                    RodRegion::Fuel
                }
            })
            .collect();

        let subchannel_area = pitch * pitch - std::f64::consts::PI * outer_radius * outer_radius;
        let hydraulic_diameter = 4.0 * subchannel_area
            / (2.0 * std::f64::consts::PI * outer_radius + 4.0 * pitch - 8.0 * outer_radius);

        Self {
            fuel_radius,
            gap_thickness,
            clad_thickness,
            pitch,
            doppler_alpha,
            outer_radius,
            node_thickness,
            node_center,
            node_area,
            region,
            subchannel_area,
            hydraulic_diameter,
            conductivity: [
                ThermalConductivity::UraniumDioxide,
                ThermalConductivity::Zircaloy,
                ThermalConductivity::GapConductance(gap_conductance),
            ],
            heat_capacity: Vec::new(),
        }
    }

    /// Attach the transient volumetric heat capacities.
    ///
    /// MATLAB `geometry.fuel.rhocp = cell(2,1); …` in the three transient case
    /// files. Identical in all three.
    pub fn with_transient_heat_capacity(&mut self) {
        self.heat_capacity = vec![
            VolumetricHeatCapacity::UraniumDioxide,
            VolumetricHeatCapacity::Zircaloy,
        ];
    }

    /// The annulus areas `pi*(r_i² - r_{i-1}²)` the reference's `Vi` formula
    /// was evidently reaching for, using true cumulative radii \[cm²\].
    ///
    /// **Not used by the reference path.** Provided only so a reader can see
    /// the size of the defect documented on [`node_area`](Self::node_area)
    /// without re-deriving it. Completing the gap is a separate, separately
    /// documented step (`docs/bedok-port-scoping.md` §1.0).
    #[must_use]
    pub fn node_area_corrected(&self) -> Vec<f64> {
        let mut areas = Vec::with_capacity(self.node_thickness.len());
        let mut r_minus = 0.0f64;
        for lr in &self.node_thickness {
            let r_plus = r_minus + lr;
            areas.push(std::f64::consts::PI * (r_plus * r_plus - r_minus * r_minus));
            r_minus = r_plus;
        }
        areas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pwr_pin() -> FuelGeometry {
        FuelGeometry::build(
            FuelDiscretisation::neacrp_default(),
            4.11950E-01,
            6.8E-03,
            5.71E-02,
            1.2665,
            0.7,
            1.0,
        )
    }

    #[test]
    fn regions_run_fuel_then_gap_then_clad() {
        let pin = pwr_pin();
        assert_eq!(pin.region.len(), 22);
        assert_eq!(pin.region[0], RodRegion::Fuel);
        assert_eq!(pin.region[19], RodRegion::Fuel);
        assert_eq!(pin.region[20], RodRegion::Gap);
        assert_eq!(pin.region[21], RodRegion::Clad);
    }

    #[test]
    fn outer_radius_and_centers_are_cumulative() {
        let pin = pwr_pin();
        assert!((pin.outer_radius - (0.41195 + 0.0068 + 0.0571)).abs() < 1e-15);
        // The last node centre sits half a cladding thickness inside Rtot.
        let last = pin.node_center[21];
        assert!((last - (pin.outer_radius - 0.5 * 0.0571)).abs() < 1e-12);
    }

    /// The `Vi` defect: every pellet node past the first has zero area,
    /// because the reference squares node thicknesses rather than radii.
    #[test]
    fn node_area_defect_is_reproduced_not_repaired() {
        let pin = pwr_pin();
        assert!(pin.node_area[0] > 0.0);
        for (i, v) in pin.node_area.iter().enumerate().take(20).skip(1) {
            assert_eq!(
                *v, 0.0,
                "node {i} should be zero under the reference formula"
            );
        }
        // The corrected annuli are all positive and sum to the rod area.
        let corrected = pin.node_area_corrected();
        assert!(corrected.iter().all(|v| *v > 0.0));
        let total: f64 = corrected.iter().sum();
        let disc = std::f64::consts::PI * pin.outer_radius * pin.outer_radius;
        assert!((total - disc).abs() < 1e-12);
    }

    #[test]
    fn conductivity_correlations_are_positive_at_operating_temperature() {
        assert!(ThermalConductivity::UraniumDioxide.evaluate(900.0) > 0.0);
        assert!(ThermalConductivity::Zircaloy.evaluate(600.0) > 0.0);
        assert_eq!(
            ThermalConductivity::GapConductance(0.35).evaluate(1.0),
            0.35
        );
        // UO2 conductivity falls with temperature over the operating range.
        assert!(
            ThermalConductivity::UraniumDioxide.evaluate(1500.0)
                < ThermalConductivity::UraniumDioxide.evaluate(700.0)
        );
    }

    #[test]
    fn heat_capacities_are_attached_only_for_transients() {
        let mut pin = pwr_pin();
        assert!(pin.heat_capacity.is_empty());
        pin.with_transient_heat_capacity();
        assert_eq!(pin.heat_capacity.len(), 2);
        assert!(VolumetricHeatCapacity::UraniumDioxide.evaluate(900.0) > 0.0);
        assert!(VolumetricHeatCapacity::Zircaloy.evaluate(600.0) > 0.0);
    }
}
