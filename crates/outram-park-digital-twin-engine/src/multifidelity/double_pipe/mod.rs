//! Rung 1 of gh:#1177 (gh:#1178): closures from the double-pipe CFD into the
//! TUAS 1-D exchanger, the 1-D model run **uncalibrated first**.
//!
//! - [`cfd`] runs `outram_foam_appbuilder_lib::cases::double_pipe` at a
//!   design point and reads off what a 1-D model needs: UA, the duty, the
//!   effectiveness, the outlet temperatures, the pressure drop and the local
//!   and mean Nusselt numbers of both sides.
//! - [`one_d`] runs TUAS's `SimpleShellAndTubeHeatExchanger` (one tube,
//!   counterflow) at the same point: [`one_d::evaluate`] with TUAS's own
//!   textbook correlations scaled by [`Theta`], and
//!   [`one_d::evaluate_with_closures`] with closures read off the CFD.
//! - [`closure`] builds the closures from CFD records and interpolates them
//!   (log-multilinear over the full-factorial training table).
//! - [`design`] is the design grid and its held-out split, fixed before any
//!   run; [`dataset`] reads and writes the committed dataset.
//!
//! V&V record, with the predictions written before the runs:
//! `verification_and_validation/hf_lf_double_pipe_gh1178.md`.
//!
//! # The rung-3 interface (gh:#1180, gh:#1194)
//!
//! - **Inputs x:** [`DoublePipeInput`] (`re_hot`, `re_cold`, `k_wall`,
//!   `length`).
//! - **CFD outputs:** [`DoublePipeOutput`], one per design point, in the
//!   committed dataset (`dataset::load_committed`), each with its
//!   [`design::Split`] (training / held-out, fixed before any run and stored
//!   in the dataset) and the dataset's provenance ([`dataset::PROVENANCE`]).
//! - **The 1-D model as a function of (x, θ):** [`one_d::evaluate`]`(&x, &θ)`
//!   → [`DoublePipeOutput`], θ = [`Theta`] with its nominal (uncalibrated)
//!   values [`Theta::nominal`].
//!
//! The CSV is SI with the unit in every column header; everything crossing
//! the Rust API is `uom`.
//!
//! **Through `raffles::fidelity` (gh:#1194):** the three records derive
//! `raffles::fidelity::Quantities` (`raffles::quantities!` below), and
//! [`fidelity`] adapts the pair: [`fidelity::TuasOneD`] is the 1-D model as a
//! `ParametricModel`, [`fidelity::committed_dataset`] the committed CFD rows
//! as a `Dataset` with the stored split.
//!
//! **State estimation (gh:#1191):** [`filtering`] tracks a synthetic,
//! drifting wall resistance from noisy outlet temperatures with
//! `raffles::filtering`'s FilterPy ports, through
//! `raffles::fidelity::StateSpaceModel` (synthetic truth only).
//!
//! **OpenDA (gh:#1192):** [`openda`] runs OpenDA's EnKF (ported into
//! `raffles::openda`) on the same twin experiment beside FilterPy's, and
//! OpenDA's DUD calibrates θ on the CFD training UA for comparison with the
//! Kennedy–O'Hagan posterior (synthetic truth only).
use uom::si::f64::{Length, Power, Pressure, Ratio, ThermalConductance, ThermalConductivity, ThermodynamicTemperature};

pub mod cfd;
pub mod closure;
pub mod cokriging;
pub mod dataset;
pub mod design;
pub mod fidelity;
pub mod filtering;
pub mod koh;
pub mod lf_grid;
pub mod one_d;
pub mod openda;
pub mod study;

/// Area-specific thermal resistance \[m² K/W\]: the inverse of a heat
/// transfer coefficient, e.g. a fouling factor. `uom` has no named quantity
/// for it, so this aliases its dimension, L⁰ M⁻¹ T³ I⁰ Θ¹ (ISQ order L, M,
/// T, I, Θ, N, J).
pub type AreaSpecificThermalResistance = uom::si::Quantity<
    uom::si::ISQ<uom::typenum::Z0, uom::typenum::N1, uom::typenum::P3, uom::typenum::Z0, uom::typenum::P1, uom::typenum::Z0, uom::typenum::Z0>,
    uom::si::SI<f64>,
    f64,
>;

/// The geometry and fluids every design point shares: stage 2's exchanger
/// (gh:#1156), `DoublePipeExchanger::demo()`'s fluids and wall heat
/// capacity. Indicative property values for a demonstration, not a material
/// data set.
pub mod fixed {
    /// Tube inner radius \[m\].
    pub const R_TUBE_INNER: f64 = 0.005;
    /// Tube outer radius \[m\].
    pub const R_TUBE_OUTER: f64 = 0.006;
    /// Shell inner radius \[m\] (adiabatic wall).
    pub const R_SHELL: f64 = 0.010;
    /// Fluid density \[kg/m³\].
    pub const RHO: f64 = 1000.0;
    /// Fluid specific heat \[J/(kg K)\].
    pub const CP: f64 = 4000.0;
    /// Fluid conductivity \[W/(m K)\].
    pub const K_FLUID: f64 = 0.6;
    /// Fluid kinematic viscosity \[m²/s\].
    pub const NU: f64 = 1.0e-6;
    /// Wall density \[kg/m³\] (heat capacity only matters in a transient).
    pub const RHO_WALL: f64 = 8000.0;
    /// Wall specific heat \[J/(kg K)\].
    pub const CP_WALL: f64 = 500.0;
    /// Hot (tube) inlet temperature \[K\].
    pub const T_HOT_IN: f64 = 350.0;
    /// Cold (annulus) inlet temperature \[K\].
    pub const T_COLD_IN: f64 = 300.0;
    /// Axial layers per metre: the 20k demo grid's Δz = 0.2 m / 75 (V&V
    /// record, "Mesh").
    pub const LAYERS_PER_METRE: f64 = 375.0;

    /// Tube diameter \[m\], the hot side's Reynolds-number length.
    #[must_use]
    pub fn d_hot() -> f64 {
        2.0 * R_TUBE_INNER
    }

    /// Annulus hydraulic diameter `D_shell − D_tube,o` \[m\], the cold side's.
    #[must_use]
    pub fn d_cold() -> f64 {
        2.0 * (R_SHELL - R_TUBE_OUTER)
    }

    /// Nominal (circular) flow areas \[m²\]: tube, annulus.
    #[must_use]
    pub fn areas() -> (f64, f64) {
        let pi = std::f64::consts::PI;
        (pi * R_TUBE_INNER * R_TUBE_INNER, pi * (R_SHELL * R_SHELL - R_TUBE_OUTER * R_TUBE_OUTER))
    }

    /// Axial layers for a length: `round(375 L)`, at least 3.
    #[must_use]
    pub fn layers(length_m: f64) -> usize {
        ((LAYERS_PER_METRE * length_m).round() as usize).max(3)
    }
}

raffles::quantities! {
    /// A design point x.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct DoublePipeInput {
        /// Tube (hot) Reynolds number on D = 2 r_tube,i, nominal circular area.
        pub re_hot: Ratio,
        /// Annulus (cold) Reynolds number on D_h = D_shell − D_tube,o, nominal
        /// circular area.
        pub re_cold: Ratio,
        /// Tube-wall conductivity.
        pub k_wall: ThermalConductivity,
        /// Exchanger length.
        pub length: Length,
    }
}

impl DoublePipeInput {
    /// From SI numbers (Re, Re, W/(m K), m).
    #[must_use]
    pub fn si(re_hot: f64, re_cold: f64, k_wall: f64, length: f64) -> Self {
        use uom::si::{length::meter, ratio::ratio, thermal_conductivity::watt_per_meter_kelvin};
        Self {
            re_hot: Ratio::new::<ratio>(re_hot),
            re_cold: Ratio::new::<ratio>(re_cold),
            k_wall: ThermalConductivity::new::<watt_per_meter_kelvin>(k_wall),
            length: Length::new::<meter>(length),
        }
    }

    /// `(Re_hot, Re_cold, k_wall [W/(m K)], L [m])`.
    #[must_use]
    pub fn to_si(&self) -> [f64; 4] {
        use uom::si::{length::meter, ratio::ratio, thermal_conductivity::watt_per_meter_kelvin};
        [self.re_hot.get::<ratio>(), self.re_cold.get::<ratio>(), self.k_wall.get::<watt_per_meter_kelvin>(), self.length.get::<meter>()]
    }

    /// Volumetric flow rates `Q = Re ν A / D` \[m³/s\]: tube, annulus.
    #[must_use]
    pub fn flow_rates(&self) -> (f64, f64) {
        let [rh, rc, _, _] = self.to_si();
        let (ah, ac) = fixed::areas();
        (rh * fixed::NU * ah / fixed::d_hot(), rc * fixed::NU * ac / fixed::d_cold())
    }

    /// Heat-capacity rates `ρ c_p Q` \[W/K\]: hot, cold.
    #[must_use]
    pub fn capacity_rates(&self) -> (f64, f64) {
        let (qh, qc) = self.flow_rates();
        (fixed::RHO * fixed::CP * qh, fixed::RHO * fixed::CP * qc)
    }
}

raffles::quantities! {
    /// What either model says about a design point.
    ///
    /// For the CFD, `nu_hot` / `nu_cold` are the ΔT-weighted mean Nusselt
    /// numbers converted to the 1-D model's circular geometry (the closures,
    /// [`closure::Closures`]); for the 1-D model they are the ones it used.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct DoublePipeOutput {
        /// Overall conductance: CFD `UA_local`; 1-D `N / (1/G_tube + 1/G_shell)`
        /// from TUAS's nodal conductances.
        pub ua: ThermalConductance,
        /// Heat duty, the heat the hot stream loses.
        pub q: Power,
        /// ε = Q / (C_min ΔT_in).
        pub effectiveness: Ratio,
        /// Hot (tube) mixing-cup outlet temperature.
        pub t_hot_out: ThermodynamicTemperature,
        /// Cold (annulus) mixing-cup outlet temperature.
        pub t_cold_out: ThermodynamicTemperature,
        /// Tube static pressure drop.
        pub dp_hot: Pressure,
        /// Annulus static pressure drop.
        pub dp_cold: Pressure,
        /// Mean tube-side Nusselt number (1-D convention, D = 2 r_tube,i).
        pub nu_hot: Ratio,
        /// Mean annulus-side Nusselt number (1-D convention, D_h, heated inner
        /// wall).
        pub nu_cold: Ratio,
    }
}

raffles::quantities! {
    /// The 1-D model's parameters θ. [`Theta::nominal`] is the uncalibrated
    /// model (TUAS's textbook correlations as they are).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Theta {
        /// Multiplier on the tube-side Nusselt number. Nominal 1.
        pub nu_mult_hot: Ratio,
        /// Multiplier on the annulus-side Nusselt number. Nominal 1.
        pub nu_mult_cold: Ratio,
        /// Extra wall resistance (a fouling factor) per unit **tube outer**
        /// area, in series with the rest. Nominal 0.
        pub wall_resistance: AreaSpecificThermalResistance,
        /// Multiplier on the tube-side `(f L/D + K)`. Nominal 1.
        pub friction_mult_hot: Ratio,
        /// Multiplier on the annulus-side `(f L/D + K)`. Nominal 1.
        pub friction_mult_cold: Ratio,
    }
}

impl Theta {
    /// The uncalibrated values: 1, 1, 0, 1, 1.
    #[must_use]
    pub fn nominal() -> Self {
        let one = Ratio::new::<uom::si::ratio::ratio>(1.0);
        Self { nu_mult_hot: one, nu_mult_cold: one, wall_resistance: fouling_si(0.0), friction_mult_hot: one, friction_mult_cold: one }
    }
}

/// An [`AreaSpecificThermalResistance`] from m² K/W.
#[must_use]
pub fn fouling_si(m2_k_per_w: f64) -> AreaSpecificThermalResistance {
    let one_over_h = 1.0 / uom::si::f64::HeatTransfer::new::<uom::si::heat_transfer::watt_per_square_meter_kelvin>(1.0);
    one_over_h * m2_k_per_w
}

/// An [`AreaSpecificThermalResistance`] in m² K/W.
#[must_use]
pub fn fouling_to_si(r: AreaSpecificThermalResistance) -> f64 {
    r.value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_and_fixed_geometry() {
        let x = DoublePipeInput::si(30.0, 15.0, 10.0, 0.2);
        assert_eq!(x.to_si(), [30.0, 15.0, 10.0, 0.2]);
        let (qh, qc) = x.flow_rates();
        // The tube at Re 30 carries the stage-2 demo's flow rate.
        assert!((qh - 2.356_194_49e-7).abs() < 1e-14, "{qh}");
        // Annulus: Re ν A / D_h with A = π (r3² − r2²), D_h = 8 mm.
        assert!((qc - 15.0e-6 * std::f64::consts::PI * 6.4e-5 / 0.008).abs() < 1e-18);
        let (ch, _) = x.capacity_rates();
        assert!((ch - 4.0e6 * qh).abs() < 1e-12);
        assert_eq!(fixed::layers(0.2), 75);
        assert_eq!(fixed::layers(0.1), 38);
        assert_eq!(fixed::layers(0.3), 113);
        assert!((fouling_to_si(fouling_si(2.5e-4)) - 2.5e-4).abs() < 1e-18);
        let t = Theta::nominal();
        assert_eq!(fouling_to_si(t.wall_resistance), 0.0);
    }
}
