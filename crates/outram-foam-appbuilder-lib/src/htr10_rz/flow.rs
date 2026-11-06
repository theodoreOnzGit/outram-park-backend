// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! KONVEK-like quasi-steady R-Z porous helium flow over the HTR-10 flow
//! regions (TECDOC-1694 Sec 2.2.1.2, Tab 2.5, Fig 2.6), and the gas energy
//! equation that couples it to the solid field (gh:#1150).
//!
//! **Original work, not a port.** It is built on the part-A
//! [`RzMesh`](crate::genfoam::thermal_hydraulics::solid_conduction::RzMesh)
//! of the gh:#1149 solid model, cell for cell: every gas cell is a solid
//! cell, so the gas-solid exchange needs no interpolation.
//!
//! ## Why not the GeN-Foam `onePhase` solver
//!
//! `genfoam::thermal_hydraulics::solver::OnePhaseSolver` was checked first
//! (search-before-building). It does not fit this case, for three stated
//! reasons: (1) it runs with **constant** `rho`, `mu`, `c_p` (its module
//! docs, "Honest scope"), while the helium density here halves between the
//! 250 deg C inlet and the 750 deg C bed outlet, and the KTA resistance
//! depends on both `rho` and `mu`; (2) its drag `Kd` is isotropic, while the
//! reflector borings, risers and control-rod channels carry axial flow only;
//! (3) it is a transient PIMPLE march, which would have to be driven to a
//! steady state through throttles of void fraction 0.0024. The steady,
//! inertia-free porous momentum balance THERMIX/KONVEK solves is a pressure
//! equation, which is what this module assembles.
//!
//! ## Equations
//!
//! **Momentum** (Darcy-Forchheimer, inertia and viscous diffusion dropped,
//! as in a porous-medium network code), with `G` the superficial mass flux,
//! `z` TECDOC's downward axis and gravity `+z`:
//!
//! ```text
//!   -grad p + rho g e_z = R(|G|, rho, mu) G        (per direction; R = infinity
//!                                                   radially in channel regions)
//!   div G = 0                                       (steady mass)
//! ```
//!
//! - **Pebble bed** (Tab 2.5 regions 1): KTA 3102.3 (TECDOC-1694 Tab 2.10,
//!   [`kta_3102_3_pressure_gradient`]), `R = (dp/dH)/|G|`, isotropic.
//! - **Channel regions** (risers, control-rod channels, reflector borings,
//!   throttles, leak gaps): straight vertical channels of hydraulic
//!   diameter `D_h` holding the Tab 2.5 void `eps`; channel mass flux
//!   `G/eps`; Darcy friction factor from the ported GeN-Foam Churchill (1977)
//!   correlation ([`FsWallFriction::Churchill`], smooth), so
//!   `R_z = f |G| / (2 rho eps^2 D_h)`; no radial flow.
//! - **Open volumes** (plena, cavities, the annulus): isotropic, the same
//!   Churchill friction on a wide slot of hydraulic diameter `2 w` (`w` the
//!   volume's width), `R = f |G| / (2 rho D_h)`. Its resistance is small
//!   against the bed's; it lets pressure equalise across a plenum.
//!   ~~The plane-Poiseuille laminar slot resistance `R = 12 mu/(rho w^2)`~~
//!   **REPLACED 2026-11-03 before any result was recorded:** with that
//!   laminar lower bound (about 20 times below the turbulent value at the
//!   hot plenum's `Re ~ 1e5`), buoyancy between the 250 deg C leakage and
//!   the 750 deg C core gas drove a circulation in the hot plenum that
//!   flipped from one outer pass to the next, and the coupled solve fell
//!   into a 15-pass limit cycle of +/-300 K (the V&V README records it).
//!
//! **No buoyancy in the open volumes.** On a face that touches an open
//! volume the gravity term is dropped. A plenum is a turbulent mixing
//! volume that a Darcy model with buoyancy does not represent. With the
//! term kept, the 250 deg C leakage entering the hot plenum below the
//! 750 deg C core gas set up a Rayleigh-Darcy circulation. That circulation
//! had no steady state: the outer iteration wandered by about 100 K in the
//! plenum for 400 passes, under both the laminar and the turbulent slot
//! resistance and with 0.5 under-relaxation (2026-11-03, before any result
//! was recorded; the V&V README keeps the record). TECDOC-1694
//! Sec 2.1.1.3 describes the hot plenum as where the streams **mix** to the
//! 700 deg C mean. Buoyancy stays on in the bed and in the channels.
//!
//! Face mass flow `F = T_f [(p_a - p_b) + rho_f g (z_b - z_a)]` with the
//! series transmissibility `T_f = A_f / (d_a R_a + d_b R_b)`. The pressure
//! equation is symmetric positive definite and is solved by DIC-CG
//! (`outram_foam_basic_lib::ldu_matrix::conjugate_gradient`). `R` depends on
//! `|G|` and on the gas temperature through `rho` and `mu`: Picard.
//!
//! **Gas energy** (steady, upwind, axial gas conduction neglected at Peclet
//! numbers of order 1e3): per gas cell or node
//!
//! ```text
//!   c_p (sum_out F T - sum_in F T_up) = h a_v V (T_s - T)
//! ```
//!
//! with `T_s` the solid temperature of the same cell, solved by an upwind
//! (topologically ordered) sweep. The same `h a_v` enters the solid field
//! implicitly through `SolidConductionField::set_cell_exchange`, so the two
//! sides exchange exactly the same heat.
//!
//! - **Bed:** KTA 3102.2 (Tab 2.10), `a_v = 6 (1 - eps)/d`. `T_s` is the
//!   pebble **surface** temperature (KTA's `T_K`); the pebble interior is
//!   [`super::pebble`].
//! - **Channels and open volumes:** Dittus-Boelter (1930),
//!   `Nu = 0.023 Re^0.8 Pr^0.4`, floored at the laminar constant-wall value
//!   3.66, both through the ported GeN-Foam
//!   [`FsForcedConvectionHtc::Nusselt`]; `a_v = 4 eps / D_h` (for an open
//!   volume of width `w`, `D_h = 2 w` and `a_v = 2/w`, the slot's two walls).
//!   In an open volume the "solid" cell is the gh:#1149 THERMIX-style
//!   cavity medium (gas conduction plus wall-to-wall radiation), so this
//!   exchange stands for gas-to-wall convection. **Assumption**, stated.
//!
//! ## Flow paths and the bypass split (TECDOC-1694 Sec 2.1.1.3; input sheet Sec 3.3, Sec 10)
//!
//! The inlet path is a 1-D chain of gas nodes over its cells, in flow order:
//! entry (Tab 2.5 region 16, one mixed node) -> annulus (17, by row,
//! downward) -> RPV bottom cavity (7, one mixed node) -> bottom annular path
//! (8, by row, upward) -> rounding path (9, by column, outward) -> risers
//! (10, by row, upward) -> top-reflector plenum (14, one mixed node; in this
//! mesh the region-43 cells). The split is **imposed** (D5: never measured,
//! never tuned) through [`BypassSplit`]:
//!
//! - the discharge-tube fraction leaves the bottom cavity node and enters the
//!   2-D domain through the bottom face of the tube throttle (11);
//! - the leakage fraction leaves after the rounding cells at `r < 50 cm`
//!   and enters the 2-D domain through the bottom face of the leak zone
//!   (19), reaching the hot plenum without cooling the core;
//! - the control-rod and bed fractions leave the top plenum node into the
//!   tops of the control-rod channels (13, `z > -65 cm`) and of the
//!   top-reflector flow zone (18), each spread uniformly over its face area.
//!
//! The 2-D domain holds regions 18, 5, 1 (core, cone and discharge tube),
//! 11, 2, 15, 3, 4, 13, 12 and 19. Flow crosses a face between two regions
//! only where the flow path connects them (see [`connected`]); the
//! discharge-tube wall, for instance, is closed. The outlet is the hot
//! plenum's outer face at `r = 90 cm` (the hot-gas duct), held at the
//! reference pressure.
//!
//! **Simplifications, stated:** the duct from the riser tops to the top
//! plenum exchanges no heat; the mixed nodes (entry, bottom cavity, top
//! plenum) are well mixed; flow areas of the mixed nodes are their axial
//! cross-sections; region 7 (r 0-210, z 507-657) is crossed by the whole
//! flow as one node. The hydraulic diameters not in the input sheet are
//! **Assumed** (see [`flow_region`]).

use petir::mathf::RealMath;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    DynamicViscosity, Length, MassDensity, MassFlux, MassRate, Power, Pressure, Ratio,
    ThermalConductivity, ThermodynamicTemperature,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::{bar, megapascal};
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use outram_foam_basic_lib::ldu_matrix::{conjugate_gradient, LduMatrix, SolverSettings};

use super::geometry::idx;
use super::Htr10RzError;
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::FsForcedConvectionHtc;
use crate::genfoam::thermal_hydraulics::solid_conduction::RzMesh;
use crate::genfoam::thermal_hydraulics::units::reynolds_number;
use crate::htgr_closures::{kta_3102_2_nusselt, kta_3102_3_pressure_gradient, KtaHelium};

/// Pebble diameter, m (TECDOC-1694 Tab 2.2 / Tab 2.3: 6.0 cm).
pub const PEBBLE_DIAMETER_M: f64 = 0.06;
/// Helium `c_p`, J/(kg K) (Tab 2.8, constant).
pub const HELIUM_CP: f64 = 5195.0;
/// Standard gravity, m/s^2.
pub const GRAVITY: f64 = 9.806_65;

/// How gas moves through, and exchanges heat in, one flow region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowKind {
    /// The pebble bed: KTA 3102.3 resistance and KTA 3102.2 exchange,
    /// isotropic.
    PebbleBed,
    /// Straight vertical channels: axial flow only.
    AxialChannels {
        /// Hydraulic diameter of one channel, m.
        hydraulic_diameter_m: f64,
    },
    /// An open gas volume (plenum, cavity, annulus) of the given width.
    OpenVolume {
        /// Width across which the volume's walls face each other, m.
        width_m: f64,
    },
}

/// One Tab 2.5 flow region as this model represents it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowRegion {
    /// Region number in TECDOC-1694 Tab 2.5.
    pub tab25: u8,
    /// Void fraction (Tab 2.5, Quoted).
    pub void_fraction: f64,
    /// Transport model.
    pub kind: FlowKind,
}

impl FlowRegion {
    /// Hydraulic diameter, m.
    #[must_use]
    pub fn hydraulic_diameter_m(&self) -> f64 {
        match self.kind {
            FlowKind::PebbleBed => PEBBLE_DIAMETER_M,
            FlowKind::AxialChannels { hydraulic_diameter_m } => hydraulic_diameter_m,
            FlowKind::OpenVolume { width_m } => 2.0 * width_m,
        }
    }

    /// Heat-transfer area per unit volume, 1/m.
    #[must_use]
    pub fn area_density(&self) -> f64 {
        match self.kind {
            FlowKind::PebbleBed => 6.0 * (1.0 - self.void_fraction) / PEBBLE_DIAMETER_M,
            _ => 4.0 * self.void_fraction / self.hydraulic_diameter_m(),
        }
    }
}

/// The flow region of a model region (cell centre `z_cm`, TECDOC `z`), or
/// `None` for a non-flowing cell (Tab 2.5 region 6 and everything not
/// labelled). Void fractions are Tab 2.5 (Quoted); hydraulic diameters:
///
/// | Region | `D_h` | Source |
/// |---|---|---|
/// | 18 top-reflector flow zone | 2.5 cm | Tab 2.3, 460 x 25 mm holes (reproduce the 0.089 void, input sheet Sec 3.4) |
/// | 13 control-rod channels | 13 cm | Tab 2.3 control-rod channel diameter (the 0.207 void is not reproduced, N15) |
/// | 10 risers | 8 cm | Tab 2.3, 20 x 8 cm (reproduce the 0.138 void) |
/// | 2, 3 bottom-reflector borings | 1.6 cm | Tab 2.3, 640 x 16 mm core-to-hot-plenum holes; for region 2 **Assumed** the same |
/// | 11, 12 throttles; 19 leak gaps | 1 cm | **Assumed** (not in the input sheet) |
/// | open volumes | `2 w` | `w` the meshed width (Fig 2.5/2.6 readings, Uncertain) |
///
/// The control-rod ring above `z = -65 cm` is solid model region 3 but not
/// Tab 2.5 region 13 (Fig 2.6 extent -65 to 222.2), so it carries no flow.
#[must_use]
pub fn flow_region(model_region: usize, z_cm: f64) -> Option<FlowRegion> {
    use idx::*;
    let ch = |tab25, void_fraction, d_cm: f64| FlowRegion {
        tab25,
        void_fraction,
        kind: FlowKind::AxialChannels {
            hydraulic_diameter_m: d_cm / 100.0,
        },
    };
    let open = |tab25, void_fraction, w_cm: f64| FlowRegion {
        tab25,
        void_fraction,
        kind: FlowKind::OpenVolume { width_m: w_cm / 100.0 },
    };
    let bed = FlowRegion {
        tab25: 1,
        void_fraction: 0.39,
        kind: FlowKind::PebbleBed,
    };
    Some(match model_region {
        BED | DISCHARGE_TUBE => bed,
        UPPER_BOTTOM_REFL => ch(2, 0.23, 1.6),
        LOWER_BOTTOM_REFL => ch(3, 0.068, 1.6),
        HOT_PLENUM => open(4, 1.0, 30.0),
        CORE_ENTRY_PLENUM => open(5, 1.0, 40.0),
        BOTTOM_HE_CAVITY => open(7, 1.0, 150.0),
        BOTTOM_ANNULAR_HE => open(8, 0.99, 11.0),
        BOTTOM_ROUNDING_HE => open(9, 1.0, 15.0),
        RISERS => ch(10, 0.138, 8.0),
        DISCHARGE_THROTTLE => ch(11, 0.0067, 1.0),
        CR_EXIT_THROTTLE => ch(12, 0.0024, 1.0),
        CR_RING if z_cm > -65.0 => ch(13, 0.207, 13.0),
        TOP_PLENUM => open(14, 1.0, 10.0),
        SMALL_PLENUM => open(15, 1.0, 5.0),
        HE_ENTRY_PATH => open(16, 1.0, 13.0),
        ANNULAR_PATH => open(17, 0.99, 13.0),
        TOP_FLOW_ZONE => ch(18, 0.089, 2.5),
        LEAK_STEEL => ch(19, 0.046, 1.0),
        _ => return None,
    })
}

/// Whether gas may cross a face between two different model regions of the
/// 2-D domain: the flow path of TECDOC-1694 Sec 2.1.1.3 (input sheet
/// Sec 3.3). Every other pair is closed (a wall).
#[must_use]
pub fn connected(a: usize, b: usize) -> bool {
    use idx::*;
    const PAIRS: [(usize, usize); 11] = [
        (TOP_FLOW_ZONE, CORE_ENTRY_PLENUM),
        (CORE_ENTRY_PLENUM, BED),
        (BED, DISCHARGE_TUBE),
        (DISCHARGE_TUBE, DISCHARGE_THROTTLE),
        (BED, UPPER_BOTTOM_REFL),
        (UPPER_BOTTOM_REFL, SMALL_PLENUM),
        (CR_RING, CR_EXIT_THROTTLE),
        (CR_EXIT_THROTTLE, SMALL_PLENUM),
        (SMALL_PLENUM, LOWER_BOTTOM_REFL),
        (LOWER_BOTTOM_REFL, HOT_PLENUM),
        (LEAK_STEEL, HOT_PLENUM),
    ];
    a == b || PAIRS.iter().any(|(x, y)| (*x == a && *y == b) || (*x == b && *y == a))
}

fn in_2d_domain(model_region: usize) -> bool {
    use idx::*;
    matches!(
        model_region,
        TOP_FLOW_ZONE
            | CORE_ENTRY_PLENUM
            | BED
            | DISCHARGE_TUBE
            | DISCHARGE_THROTTLE
            | UPPER_BOTTOM_REFL
            | SMALL_PLENUM
            | LOWER_BOTTOM_REFL
            | HOT_PLENUM
            | CR_RING
            | CR_EXIT_THROTTLE
            | LEAK_STEEL
    )
}

/// How the total helium flow divides (TECDOC-1694 Sec 2.1.1.3; input sheet
/// Sec 10). **Unmeasured (D5): an input and a sensitivity, never a tuning
/// knob.** The fractions must sum to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BypassSplit {
    /// Through the pebble bed (the cooling flow).
    pub bed: Ratio,
    /// Up the fuel discharge tube.
    pub discharge_tube: Ratio,
    /// Down the control-rod channels.
    pub control_rods: Ratio,
    /// Leakage through graphite gaps, cold inlet straight to the hot plenum.
    pub leakage: Ratio,
}

impl BypassSplit {
    fn of(b: f64, d: f64, c: f64, l: f64) -> Self {
        let r = Ratio::new::<ratio>;
        Self {
            bed: r(b),
            discharge_tube: r(d),
            control_rods: r(c),
            leakage: r(l),
        }
    }

    /// The benchmark's assumed split, 87 / 1 / 2 / 10 % ([T1] Sec 2.1.1.3).
    /// **The default** (D4), an assumption.
    #[must_use]
    pub fn tecdoc_1694() -> Self {
        Self::of(0.87, 0.01, 0.02, 0.10)
    }

    /// `BYP-G02`: 86 % to the bed (Gao 2002 Sec 4.2), the other 14 % over
    /// the three paths in [T1]'s proportions (1 : 2 : 10).
    #[must_use]
    pub fn gao_2002() -> Self {
        Self::of(0.86, 0.14 / 13.0, 0.28 / 13.0, 1.4 / 13.0)
    }

    /// `BYP-NOLEAK`: the 10 % leakage, which [T1] calls a maximum, set to
    /// zero and sent through the bed (97 / 1 / 2 / 0).
    #[must_use]
    pub fn no_leakage() -> Self {
        Self::of(0.97, 0.01, 0.02, 0.0)
    }

    fn fractions(&self) -> [f64; 4] {
        [
            self.bed.get::<ratio>(),
            self.discharge_tube.get::<ratio>(),
            self.control_rods.get::<ratio>(),
            self.leakage.get::<ratio>(),
        ]
    }

    fn check(&self) -> Result<(), Htr10RzError> {
        let f = self.fractions();
        if f.iter().any(|x| !(*x >= 0.0) || !x.is_finite()) || (f.iter().sum::<f64>() - 1.0).abs() > 1e-9 || !(f[0] > 0.0) {
            return Err(Htr10RzError::Input(format!(
                "bypass fractions {f:?} must be non-negative, sum to 1, with flow through the bed"
            )));
        }
        Ok(())
    }
}

impl Default for BypassSplit {
    fn default() -> Self {
        Self::tecdoc_1694()
    }
}

/// The plant state of a steady full-power case.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperatingPoint {
    /// Thermal power, distributed with the Tab 2.6 shape.
    pub power: Power,
    /// Core inlet helium temperature.
    pub inlet: ThermodynamicTemperature,
    /// Primary pressure.
    pub pressure: Pressure,
    /// Total helium flow.
    pub mass_flow: MassRate,
}

impl OperatingPoint {
    /// The benchmark specification (D4 base): 10 MW, 250 deg C inlet,
    /// 3.0 MPa ([T1] Tab 2.1), and the **rated** 4.32 kg/s ([T1] Tab 2.1,
    /// Sec 2.1.1.3, Quoted). The outlet temperature is an output.
    #[must_use]
    pub fn fpic_specification() -> Self {
        Self {
            power: Power::new::<megawatt>(10.0),
            inlet: ThermodynamicTemperature::new::<degree_celsius>(250.0),
            pressure: Pressure::new::<megapascal>(3.0),
            mass_flow: MassRate::new::<kilogram_per_second>(4.32),
        }
    }

    /// `SS-ASRUN` (D4 sensitivity): the 72 h run during which the
    /// thermocouples were read ([C14] Tab 2): 10.3 MW, 236 deg C, 2.94 MPa,
    /// flow 4.273 kg/s by energy balance to 700 deg C (input sheet Sec 9.2,
    /// Derived).
    #[must_use]
    pub fn fpic_as_run() -> Self {
        Self {
            power: Power::new::<megawatt>(10.3),
            inlet: ThermodynamicTemperature::new::<degree_celsius>(236.0),
            pressure: Pressure::new::<megapascal>(2.94),
            mass_flow: MassRate::new::<kilogram_per_second>(4.273),
        }
    }

    /// `FLOW-EB`: the specification with the energy-balance flow 4.278 kg/s
    /// (input sheet Sec 5.1, Derived: 10 MW over 250-700 deg C) instead of
    /// the rated 4.32.
    #[must_use]
    pub fn fpic_energy_balance_flow() -> Self {
        Self {
            mass_flow: MassRate::new::<kilogram_per_second>(4.278),
            ..Self::fpic_specification()
        }
    }
}

/// Helium state at a temperature (K) and the operating pressure, from the
/// Tab 2.8 fits ([`KtaHelium`]), temperature held to the fits' range.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gas {
    pub rho: f64,
    pub mu: f64,
    pub k: f64,
    pub pr: f64,
}

pub(crate) fn gas(t_k: f64, p_bar: f64) -> Result<Gas, Htr10RzError> {
    let he = KtaHelium::new(
        Pressure::new::<bar>(p_bar),
        ThermodynamicTemperature::new::<kelvin>(
            t_k.clamp(KtaHelium::MIN_TEMPERATURE_K, KtaHelium::MAX_TEMPERATURE_K),
        ),
    )
    .map_err(|e| Htr10RzError::Property(format!("helium: {e}")))?;
    Ok(Gas {
        rho: he.density().get::<kilogram_per_cubic_meter>(),
        mu: he.viscosity().get::<pascal_second>(),
        k: he.thermal_conductivity().get::<watt_per_meter_kelvin>(),
        pr: he.prandtl().get::<ratio>(),
    })
}

/// Smallest mass flux used to evaluate a closure, kg/(m^2 s): the closures
/// stay finite (Darcy / laminar limits) but `|G| = 0` would divide by zero.
const G_FLOOR: f64 = 1.0e-6;

/// Momentum resistance `R` (1/s) of a cell along one direction, `None` when
/// the direction is closed.
fn resistance(fr: &FlowRegion, g_mag: f64, g_dir: f64, axial: bool, gas: &Gas) -> Result<Option<f64>, Htr10RzError> {
    let eps = fr.void_fraction;
    Ok(match fr.kind {
        FlowKind::PebbleBed => {
            let g = g_mag.max(G_FLOOR);
            let dpdh = kta_3102_3_pressure_gradient(
                MassFlux::new::<kilogram_per_square_meter_second>(g),
                Length::new::<meter>(PEBBLE_DIAMETER_M),
                Ratio::new::<ratio>(eps),
                MassDensity::new::<kilogram_per_cubic_meter>(gas.rho),
                DynamicViscosity::new::<pascal_second>(gas.mu),
            )
            .map_err(|e| Htr10RzError::Property(format!("KTA 3102.3: {e}")))?;
            Some(dpdh.value / g)
        }
        FlowKind::AxialChannels { hydraulic_diameter_m } => {
            if !axial {
                return Ok(None);
            }
            let g = g_dir.abs().max(G_FLOOR);
            let re = g / eps * hydraulic_diameter_m / gas.mu;
            let f = FsWallFriction::Churchill { surface_roughness: 0.0 }
                .friction_factor(reynolds_number(re))
                .get::<ratio>();
            Some(f * g / (2.0 * gas.rho * eps * eps * hydraulic_diameter_m))
        }
        FlowKind::OpenVolume { width_m } => {
            // A wide slot of hydraulic diameter 2 w, all-regime Churchill
            // friction on the local |G| (isotropic).
            let dh = 2.0 * width_m;
            let g = g_mag.max(G_FLOOR);
            let re = g / eps * dh / gas.mu;
            let f = FsWallFriction::Churchill { surface_roughness: 0.0 }
                .friction_factor(reynolds_number(re))
                .get::<ratio>();
            Some(f * g / (2.0 * gas.rho * eps * eps * dh))
        }
    })
}

/// Largest relative change of a cell's momentum resistances, radial and
/// axial, between the mass flux `old` and `new` (`(G_r, G_z)`,
/// kg/(m^2 s)): `max |R(new) - R(old)| / R(old)` over the open directions.
/// This is the convergence measure of the gas-flow Picard (gh:#1175).
///
/// **Why the resistance and not the flux.** The Picard iterates
/// `G -> R(|G|) -> p -> G`; given `R` the pressure solve is linear, so the
/// fixed point is reached when `R` stops changing. The earlier measure, the
/// per-cell change of `G` over `max(|G|, 1e-3)`, has a floor that iteration
/// cannot lower: on the 10 cm mesh with the loop stopped (2026-11-03) it
/// fell geometrically (factor ~0.52 per pass, the 0.5 relaxation) to
/// ~3e-7 by pass 30 and then wandered between 1e-7 and 7e-7 for the
/// remaining 170 passes, its maximum hopping between unrelated cells. Two
/// sources, both measured:
///
/// - open volumes: the change was 1-3 times `T_f eps (|p_a| + |p_b|) / A_f`,
///   the flux error of one rounding of the ~63 Pa hydrostatic pressure on a
///   low-resistance face, i.e. round-off;
/// - laminar channels (`Re ~ 1`): ~5e-10 kg/(m^2 s) of noise on
///   `G ~ 2e-4`, the CG's 1e-13 relative residual on pressures ~1e4 times
///   the column's own drop, read against the 1e-3 floor as ~7e-7.
///
/// In both cells `R` does not depend on `G` (laminar limit: `f |G|` is
/// constant), so the flux noise moves `R` by nothing: there is nothing left
/// for the Picard to converge. Where `R` does depend on `G` (turbulent
/// channels, the KTA bed), `|dln R / dln G| <= 1`, so the tolerance on `R`
/// is the same tolerance, 1e-7, on what is iterated.
///
/// # Errors
/// [`Htr10RzError::Property`] from a closure.
fn resistance_change(fr: &FlowRegion, old: (f64, f64), new: (f64, f64), gas: &Gas) -> Result<f64, Htr10RzError> {
    let (mo, mn) = (old.0.r_hypot(old.1), new.0.r_hypot(new.1));
    let mut change: f64 = 0.0;
    for (axial, go, gn) in [(false, old.0, new.0), (true, old.1, new.1)] {
        if let (Some(ro), Some(rn)) = (resistance(fr, mo, go, axial, gas)?, resistance(fr, mn, gn, axial, gas)?) {
            change = change.max((rn - ro).abs() / ro);
        }
    }
    Ok(change)
}

/// Volumetric gas-solid exchange coefficient `h a_v`, W/(m^3 K), at the
/// superficial mass flux `g`.
pub(crate) fn exchange_coefficient(fr: &FlowRegion, g: f64, gas: &Gas) -> Result<f64, Htr10RzError> {
    let g = g.max(G_FLOOR);
    let d = fr.hydraulic_diameter_m();
    let h = match fr.kind {
        FlowKind::PebbleBed => {
            let re = g * d / gas.mu;
            kta_3102_2_nusselt(
                Ratio::new::<ratio>(re),
                Ratio::new::<ratio>(gas.pr),
                Ratio::new::<ratio>(fr.void_fraction),
            )
            .map_err(|e| Htr10RzError::Property(format!("KTA 3102.2: {e}")))?
            .get::<ratio>()
                * gas.k
                / d
        }
        _ => {
            let re = reynolds_number(g / fr.void_fraction * d / gas.mu);
            let pr = Ratio::new::<ratio>(gas.pr);
            let k = ThermalConductivity::new::<watt_per_meter_kelvin>(gas.k);
            let dh = Length::new::<meter>(d);
            let db = FsForcedConvectionHtc::Nusselt { a: 0.0, b: 0.023, c: 0.8, d: 0.4, e: 0.0 }
                .heat_transfer_coefficient(re, pr, k, dh, None);
            let lam = FsForcedConvectionHtc::Nusselt { a: 3.66, b: 0.0, c: 0.0, d: 0.0, e: 0.0 }
                .heat_transfer_coefficient(re, pr, k, dh, None);
            db.value.max(lam.value)
        }
    };
    Ok(h * fr.area_density())
}

/// Which inlet stream a boundary inflow carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Bed,
    DischargeTube,
    ControlRods,
    Leakage,
}

/// One internal face of the 2-D gas domain.
#[derive(Debug, Clone, Copy)]
struct GasFace {
    /// Compact gas-cell indices, `a < b`.
    a: usize,
    b: usize,
    area: f64,
    /// Centre-to-face distances, m.
    da: f64,
    db: f64,
    axial: bool,
    /// `z_b - z_a`, m (TECDOC `z`, downward).
    dz: f64,
}

/// A node of the 1-D inlet chain.
#[derive(Debug, Clone)]
struct ChainNode {
    cells: Vec<usize>,
    /// Fraction of the total flow through it.
    fraction: f64,
    /// Flow area for its Reynolds number, m^2 (superficial).
    flow_area: f64,
    region: FlowRegion,
}

/// The assembled gas network of one mesh and one split.
#[derive(Debug, Clone)]
pub struct GasNetwork {
    n_cells: usize,
    /// Model cell -> compact 2-D gas index.
    gas_of_cell: Vec<Option<usize>>,
    /// Compact 2-D gas index -> model cell.
    cell_of_gas: Vec<usize>,
    /// Flow region of each 2-D gas cell.
    region_of_gas: Vec<FlowRegion>,
    faces: Vec<GasFace>,
    /// `(gas cell, area, stream)` inflow faces.
    inflow: Vec<(usize, f64, Stream)>,
    /// `(gas cell, area, half distance, cell centre z m)` outlet faces.
    outlet: Vec<(usize, f64, f64, f64)>,
    /// Inflow area per stream.
    stream_area: [f64; 4],
    chain: Vec<ChainNode>,
    /// Chain node whose outlet feeds each tap: discharge tube, leakage, top
    /// plenum (bed and control rods).
    tap_discharge: usize,
    tap_leak: usize,
    split: BypassSplit,
    volumes: Vec<f64>,
    regions: Vec<usize>,
    /// Whether each model cell belongs to an inlet-chain node.
    in_chain: Vec<bool>,
}

impl GasNetwork {
    /// Assemble the network on the solid model's mesh.
    ///
    /// # Errors
    /// [`Htr10RzError::Input`] for a bad split, [`Htr10RzError::Mesh`] if a
    /// flow path the network needs is missing from the mesh.
    pub fn build(mesh: &RzMesh, split: BypassSplit) -> Result<Self, Htr10RzError> {
        use idx::*;
        split.check()?;
        let re: Vec<f64> = (0..=mesh.r_edges().n_cells()).map(|i| mesh.r_edges().edge(i).get::<meter>()).collect();
        let ze: Vec<f64> = (0..=mesh.z_edges().n_cells()).map(|j| mesh.z_edges().edge(j).get::<meter>()).collect();
        let (nr, nz) = (re.len() - 1, ze.len() - 1);
        let n = mesh.n_cells();
        let rc = |i: usize| 0.5 * (re[i] + re[i + 1]);
        let zcf = |j: usize| 0.5 * (ze[j] + ze[j + 1]);
        let regions: Vec<usize> = (0..n).map(|c| mesh.cell_region(c)).collect();
        let fr_of = |c: usize| flow_region(regions[c], zcf(c / nr) * 100.0);
        let mut gas_of_cell = vec![None; n];
        let mut cell_of_gas = Vec::new();
        let mut region_of_gas = Vec::new();
        for c in 0..n {
            if in_2d_domain(regions[c]) {
                if let Some(fr) = fr_of(c) {
                    gas_of_cell[c] = Some(cell_of_gas.len());
                    cell_of_gas.push(c);
                    region_of_gas.push(fr);
                }
            }
        }
        let is_open = |c: usize| matches!(fr_of(c), Some(FlowRegion { kind: FlowKind::OpenVolume { .. }, .. }));
        let mut faces = Vec::new();
        let mut inflow = Vec::new();
        let mut outlet = Vec::new();
        let ring = |i: usize| std::f64::consts::PI * (re[i + 1] * re[i + 1] - re[i] * re[i]);
        for j in 0..nz {
            for i in 0..nr {
                let c = mesh.cell_index(i, j);
                let Some(ga) = gas_of_cell[c] else { continue };
                // Radial neighbour (i + 1).
                if i + 1 < nr {
                    let cn = mesh.cell_index(i + 1, j);
                    if let Some(gb) = gas_of_cell[cn] {
                        if connected(regions[c], regions[cn]) {
                            faces.push(GasFace {
                                a: ga,
                                b: gb,
                                area: 2.0 * std::f64::consts::PI * re[i + 1] * (ze[j + 1] - ze[j]),
                                da: re[i + 1] - rc(i),
                                db: rc(i + 1) - re[i + 1],
                                axial: false,
                                dz: 0.0,
                            });
                        }
                    }
                }
                // Axial neighbour (j + 1, lower in the reactor).
                if j + 1 < nz {
                    let cn = mesh.cell_index(i, j + 1);
                    if let Some(gb) = gas_of_cell[cn] {
                        if connected(regions[c], regions[cn]) {
                            faces.push(GasFace {
                                a: ga,
                                b: gb,
                                area: ring(i),
                                da: ze[j + 1] - zcf(j),
                                db: zcf(j + 1) - ze[j + 1],
                                axial: true,
                                // No buoyancy on a face touching an open
                                // volume (module docs).
                                dz: if is_open(c) || is_open(cn) { 0.0 } else { zcf(j + 1) - zcf(j) },
                            });
                        }
                    }
                }
                // Inflow faces: the top faces at z = -65 cm of regions 18
                // and 13; the bottom faces of the tube throttle (z 507) and
                // of the leak zone (z 446).
                let top_z = ze[j] * 100.0;
                let bot_z = ze[j + 1] * 100.0;
                let above = (j > 0).then(|| regions[mesh.cell_index(i, j - 1)]);
                let below = (j + 1 < nz).then(|| regions[mesh.cell_index(i, j + 1)]);
                match regions[c] {
                    TOP_FLOW_ZONE if above != Some(TOP_FLOW_ZONE) && (top_z + 65.0).abs() < 1e-6 => {
                        inflow.push((ga, ring(i), Stream::Bed));
                    }
                    CR_RING if (top_z + 65.0).abs() < 1e-6 => inflow.push((ga, ring(i), Stream::ControlRods)),
                    DISCHARGE_THROTTLE if below != Some(DISCHARGE_THROTTLE) => {
                        inflow.push((ga, ring(i), Stream::DischargeTube));
                    }
                    LEAK_STEEL if below != Some(LEAK_STEEL) && (bot_z - 446.0).abs() < 1e-6 => {
                        inflow.push((ga, ring(i), Stream::Leakage));
                    }
                    HOT_PLENUM if (re[i + 1] * 100.0 - 90.0).abs() < 1e-6 => {
                        outlet.push((ga, 2.0 * std::f64::consts::PI * re[i + 1] * (ze[j + 1] - ze[j]), re[i + 1] - rc(i), zcf(j)));
                    }
                    _ => {}
                }
            }
        }
        let mut stream_area = [0.0; 4];
        for (_, a, s) in &inflow {
            stream_area[*s as usize] += a;
        }
        let fr_split = split.fractions();
        for (k, a) in stream_area.iter().enumerate() {
            if *a == 0.0 && fr_split[k] > 0.0 {
                return Err(Htr10RzError::Mesh(format!("no inflow face for stream {k}")));
            }
        }
        if outlet.is_empty() {
            return Err(Htr10RzError::Mesh("no hot-plenum outlet face at r = 90 cm".into()));
        }

        // The inlet chain.
        let cells_of = |reg: usize| -> Vec<usize> { (0..n).filter(|c| regions[*c] == reg).collect() };
        let region = |reg: usize| fr_of(cells_of(reg)[0]).expect("chain region flows");
        let rows = |reg: usize, upward: bool| -> Vec<Vec<usize>> {
            let mut js: Vec<usize> = cells_of(reg).iter().map(|c| c / nr).collect();
            js.sort_unstable();
            js.dedup();
            if upward {
                js.reverse();
            }
            js.iter()
                .map(|j| (0..nr).map(|i| mesh.cell_index(i, *j)).filter(|c| regions[*c] == reg).collect())
                .collect()
        };
        let row_area = |cells: &[usize], void: f64| -> f64 {
            cells.iter().map(|c| ring(c % nr)).sum::<f64>() * void
        };
        for reg in [HE_ENTRY_PATH, ANNULAR_PATH, BOTTOM_HE_CAVITY, BOTTOM_ANNULAR_HE, BOTTOM_ROUNDING_HE, RISERS, TOP_PLENUM] {
            if cells_of(reg).is_empty() {
                return Err(Htr10RzError::Mesh(format!("chain region {reg} has no cells")));
            }
        }
        let [_, f_dis, _, f_leak] = fr_split;
        let mut chain = Vec::new();
        let mixed = |reg: usize, fraction: f64| {
            let cells = cells_of(reg);
            let fr = region(reg);
            let mut is: Vec<usize> = cells.iter().map(|c| c % nr).collect();
            is.sort_unstable();
            is.dedup();
            ChainNode {
                flow_area: is.iter().map(|i| ring(*i)).sum::<f64>() * fr.void_fraction,
                cells,
                fraction,
                region: fr,
            }
        };
        chain.push(mixed(HE_ENTRY_PATH, 1.0));
        for cells in rows(ANNULAR_PATH, false) {
            let fr = region(ANNULAR_PATH);
            chain.push(ChainNode { flow_area: row_area(&cells, fr.void_fraction), cells, fraction: 1.0, region: fr });
        }
        chain.push(mixed(BOTTOM_HE_CAVITY, 1.0));
        let tap_discharge = chain.len() - 1;
        for cells in rows(BOTTOM_ANNULAR_HE, true) {
            let fr = region(BOTTOM_ANNULAR_HE);
            chain.push(ChainNode { flow_area: row_area(&cells, fr.void_fraction), cells, fraction: 1.0 - f_dis, region: fr });
        }
        // Rounding path: by column, outward; the leakage leaves after the
        // columns inside r = 50 cm.
        let rnd = cells_of(BOTTOM_ROUNDING_HE);
        let mut is: Vec<usize> = rnd.iter().map(|c| c % nr).collect();
        is.sort_unstable();
        is.dedup();
        let fr_rnd = region(BOTTOM_ROUNDING_HE);
        let mut tap_leak = None;
        for i in is {
            let cells: Vec<usize> = rnd.iter().copied().filter(|c| c % nr == i).collect();
            let h: f64 = cells.iter().map(|c| ze[c / nr + 1] - ze[c / nr]).sum();
            let inside = rc(i) < 0.50;
            if !inside && tap_leak.is_none() {
                tap_leak = Some(chain.len() - 1);
            }
            chain.push(ChainNode {
                flow_area: 2.0 * std::f64::consts::PI * rc(i) * h,
                cells,
                fraction: if inside { 1.0 - f_dis } else { 1.0 - f_dis - f_leak },
                region: fr_rnd,
            });
        }
        let tap_leak = tap_leak.ok_or_else(|| Htr10RzError::Mesh("rounding path has no column beyond r = 50 cm".into()))?;
        for cells in rows(RISERS, true) {
            let fr = region(RISERS);
            chain.push(ChainNode { flow_area: row_area(&cells, fr.void_fraction), cells, fraction: 1.0 - f_dis - f_leak, region: fr });
        }
        chain.push(mixed(TOP_PLENUM, 1.0 - f_dis - f_leak));

        let fv = mesh.mesh();
        let mut in_chain = vec![false; n];
        for nd in &chain {
            for c in &nd.cells {
                in_chain[*c] = true;
            }
        }
        Ok(Self {
            n_cells: n,
            gas_of_cell,
            cell_of_gas,
            region_of_gas,
            faces,
            inflow,
            outlet,
            stream_area,
            chain,
            tap_discharge,
            tap_leak,
            split,
            volumes: fv.cell_volumes.clone(),
            regions,
            in_chain,
        })
    }

    /// The split it was built with.
    #[must_use]
    pub fn split(&self) -> BypassSplit {
        self.split
    }

    /// Drop the gravity term on every face: the explicit ablation `NC-OFF`
    /// of gh:#1151 part B, which removes the in-vessel natural circulation
    /// so its contribution can be measured. Buoyancy stays ON by default.
    #[must_use]
    pub fn without_buoyancy(mut self) -> Self {
        for f in &mut self.faces {
            f.dz = 0.0;
        }
        self
    }

    /// Whether any face carries a gravity term.
    #[must_use]
    pub fn has_buoyancy(&self) -> bool {
        self.faces.iter().any(|f| f.dz != 0.0)
    }

    /// The inlet-chain nodes in flow order: their model cells and the
    /// fraction of the loop flow through each.
    #[must_use]
    pub fn chain_nodes(&self) -> Vec<(Vec<usize>, f64)> {
        self.chain.iter().map(|nd| (nd.cells.clone(), nd.fraction)).collect()
    }

    /// Number of 2-D gas cells.
    #[must_use]
    pub fn n_gas_cells(&self) -> usize {
        self.cell_of_gas.len()
    }

    /// Number of inlet-chain nodes.
    #[must_use]
    pub fn n_chain_nodes(&self) -> usize {
        self.chain.len()
    }

    /// The flow region of model cell `c` as the network uses it (`None`
    /// for a non-flowing cell), and whether it is a 2-D domain cell
    /// (`true`) or an inlet-chain cell (`false`).
    #[must_use]
    pub fn cell_flow(&self, c: usize) -> Option<(FlowRegion, bool)> {
        if let Some(g) = self.gas_of_cell[c] {
            return Some((self.region_of_gas[g], true));
        }
        self.chain
            .iter()
            .find(|nd| nd.cells.contains(&c))
            .map(|nd| (nd.region, false))
    }
}

/// The gas state on a [`GasNetwork`].
#[derive(Debug, Clone)]
pub struct GasState {
    /// Gas temperature per model cell, K (`NaN` where no gas flows).
    pub temperature: Vec<f64>,
    /// Superficial mass flux per model cell `(G_r, G_z)`, kg/(m^2 s)
    /// (`G_z` positive downward; zero where no gas flows).
    pub mass_flux: Vec<(f64, f64)>,
    /// Pressure per model cell relative to the outlet, Pa (`NaN` outside
    /// the 2-D domain).
    pub pressure: Vec<f64>,
    /// Volumetric exchange coefficient per model cell, W/(m^3 K).
    pub exchange: Vec<f64>,
    /// Mixed outlet temperature, K.
    pub outlet_temperature: f64,
    /// Bed outlet: mass-weighted temperature of the gas leaving the bed and
    /// discharge tube through the cone floor into region 2, K.
    pub bed_outlet_temperature: f64,
    /// Heat taken up by the gas, `sum h a_v V (T_s - T_g)`, W.
    pub heat_to_gas: f64,
    /// `W c_p (T_out - T_in)`, W: equals `heat_to_gas` when the gas side
    /// conserves energy.
    pub enthalpy_rise: f64,
    /// Picard passes of the last flow solve and its last relative change of
    /// the cell resistances (`resistance_change`, gh:#1175).
    pub flow_picard: (usize, f64),
    /// Evaluations of [`solve_gas`] in the last [`solve_gas_consistent`]
    /// and its last density residual, K (`(1, NaN)` for a plain
    /// [`solve_gas`]).
    pub density_iterations: (usize, f64),
    /// Pressure drop inlet-of-bed-stream to outlet, Pa (mean over the
    /// region-18 inflow cells).
    pub core_pressure_drop: f64,
    /// Gas temperatures of the tap nodes: bottom cavity, after the leak
    /// columns, top plenum, K.
    pub tap_temperatures: [f64; 3],
    /// Mass leaving through the outlet, kg/s: equals the total flow when
    /// the pressure solve conserves mass.
    pub outlet_mass_flow: f64,
}

/// Tolerance of [`solve_gas_consistent`]: the largest change of a gas
/// temperature between the density the flow saw and the energy solution,
/// K. The same 1e-4 K as the steady outer iteration's.
pub const GAS_DENSITY_TOLERANCE_K: f64 = 1.0e-4;

/// **The gas solved consistently in its own density** (gh:#1176): the
/// fixed point `T = E(T)` of [`solve_gas`], where the flow sees the
/// densities (and `mu`) of `T` and the energy solve returns `E(T)`.
///
/// [`solve_gas`] alone is one evaluation of that map: its flow uses the
/// gas temperatures of `prev`. Where buoyancy drives the flow and a cell's
/// gas temperature depends on the direction of its through-flow (a cell
/// whose upwind side changes with the sign of a small flux), the map has a
/// slope below -1, and repeating it oscillates. Measured on the finest
/// mesh of the gh:#1176 study (`finest_mesh_gas_alone_at_fixed_solid`,
/// solid held): plain repeats swing by 50-100 K at the axis cells of the
/// discharge tube, with the 0.5 relaxation of the steady solve by 8.8 K
/// (the axial flux there flipping sign), and with a fixed 0.2 relaxation
/// they converge geometrically (factor ~0.75) to 2e-5 K. So the
/// consistent state exists; the lagged map does not reach it.
///
/// The iteration is relaxed with Aitken's dynamic factor (Irons & Tuck
/// 1969; Küttler & Wall 2008, Comput. Mech. 43, eq. 41): `x_{k+1} = x_k +
/// w_k r_k`, `r_k = E(x_k) - x_k`, `w_k = -w_{k-1} r_{k-1}.(r_k -
/// r_{k-1}) / |r_k - r_{k-1}|^2`, clamped to `[0.02, 1]`, `w_0 = 1`. It
/// stops when `max |r_k| < tol_k` and returns the state evaluated at
/// `x_k` (whose energy solution differs from its density temperatures by
/// less than `tol_k`). The first evaluation is exactly [`solve_gas`].
///
/// # Errors
/// As [`solve_gas`]; [`Htr10RzError::Solve`] if 200 evaluations do not
/// converge.
pub fn solve_gas_consistent(
    net: &GasNetwork,
    op: &OperatingPoint,
    t_solid: &[f64],
    prev: Option<&GasState>,
    tol_k: f64,
) -> Result<GasState, Htr10RzError> {
    let mut state = solve_gas(net, op, t_solid, prev)?;
    // The density temperatures of the first evaluation: `prev`'s, or the
    // inlet temperature where `prev` has none (as `solve_gas` starts).
    let t_in = op.inlet.get::<kelvin>();
    let mut x: Vec<f64> = match prev {
        Some(p) => p.temperature.iter().map(|t| if t.is_finite() { *t } else { t_in }).collect(),
        None => vec![t_in; state.temperature.len()],
    };
    let mut r_old: Option<Vec<f64>> = None;
    let mut w = 1.0;
    let mut last = f64::INFINITY;
    let mut history: Vec<(f64, f64)> = Vec::new();
    for k in 1..=200 {
        let r: Vec<f64> = state
            .temperature
            .iter()
            .zip(&x)
            .map(|(e, xi)| if e.is_finite() { e - xi } else { 0.0 })
            .collect();
        last = r.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        history.push((last, w));
        if last < tol_k {
            state.density_iterations = (k, last);
            return Ok(state);
        }
        if let Some(ro) = &r_old {
            let (mut num, mut den) = (0.0, 0.0);
            for (a, b) in r.iter().zip(ro) {
                let d = a - b;
                num += b * d;
                den += d * d;
            }
            if den > 0.0 {
                w = (-w * num / den).clamp(0.02, 1.0);
            }
        }
        for (xi, ri) in x.iter_mut().zip(&r) {
            *xi += w * ri;
        }
        r_old = Some(r);
        let mut seed = state.clone();
        for (s, xi) in seed.temperature.iter_mut().zip(&x) {
            if s.is_finite() {
                *s = *xi;
            }
        }
        state = solve_gas(net, op, t_solid, Some(&seed))?;
    }
    Err(Htr10RzError::Solve(format!(
        "gas density iteration did not converge in 200 evaluations (last change {last:e} K; residual, relaxation of the last 12: {:?})",
        &history[history.len().saturating_sub(12)..]
    )))
}

/// Solve the flow (Picard on the resistances) and the gas energy for the
/// solid temperatures `t_solid` (K per model cell), starting from `prev`.
///
/// # Errors
/// [`Htr10RzError::Solve`] if the pressure solve or the Picard iteration
/// fails; [`Htr10RzError::Property`] from a closure.
pub fn solve_gas(
    net: &GasNetwork,
    op: &OperatingPoint,
    t_solid: &[f64],
    prev: Option<&GasState>,
) -> Result<GasState, Htr10RzError> {
    let p_bar = op.pressure.get::<bar>();
    let w = op.mass_flow.get::<kilogram_per_second>();
    if !(w >= 0.0) || !w.is_finite() {
        // The inlet chain and the taps are built for the forced direction;
        // a reversed loop flow is not represented (gh:#1151 part B).
        return Err(Htr10RzError::Input(format!(
            "loop mass flow must be finite and non-negative (forced direction), got {w} kg/s"
        )));
    }
    let t_in = op.inlet.get::<kelvin>();
    let ng = net.n_gas_cells();
    let n = net.n_cells;
    let fr = net.split.fractions();
    let mut t_gas = prev.map_or_else(|| vec![t_in; n], |s| s.temperature.iter().map(|t| if t.is_finite() { *t } else { t_in }).collect());
    let mut g_cell: Vec<(f64, f64)> = prev.map_or_else(
        || {
            // Start from the bed stream spread over the core area.
            let g0 = fr[0] * w / net.stream_area[0];
            vec![(0.0, g0); n]
        },
        |s| s.mass_flux.clone(),
    );

    // --- Flow: Picard on R(|G|, rho, mu) with the gas temperature held.
    let mut p = vec![0.0; ng];
    if let Some(s) = prev {
        for (k, c) in net.cell_of_gas.iter().enumerate() {
            p[k] = s.pressure[*c];
        }
    }
    let mut flux = vec![0.0; net.faces.len()];
    let mut out_flux = vec![0.0; net.outlet.len()];
    let mut picard = (0, f64::INFINITY);
    let gases: Vec<Gas> = net
        .cell_of_gas
        .iter()
        .map(|c| gas(t_gas[*c], p_bar))
        .collect::<Result<_, _>>()?;
    let inflow_mass = |s: Stream| fr[s as usize] * w / net.stream_area[s as usize];
    for it in 1..=200 {
        let mut rr = vec![0.0; ng];
        let mut rz = vec![0.0; ng];
        let mut open_r = vec![true; ng];
        for k in 0..ng {
            let c = net.cell_of_gas[k];
            let (gr, gz) = g_cell[c];
            let mag = gr.r_hypot(gz);
            let f = &net.region_of_gas[k];
            match resistance(f, mag, gr, false, &gases[k])? {
                Some(v) => rr[k] = v,
                None => open_r[k] = false,
            }
            rz[k] = resistance(f, mag, gz, true, &gases[k])?.unwrap_or(f64::INFINITY);
        }
        let mut ldu_owner = Vec::new();
        let mut ldu_nb = Vec::new();
        let mut tf = Vec::new();
        let mut face_ids = Vec::new();
        for (fi, f) in net.faces.iter().enumerate() {
            let t = if f.axial {
                f.area / (f.da * rz[f.a] + f.db * rz[f.b])
            } else if open_r[f.a] && open_r[f.b] {
                f.area / (f.da * rr[f.a] + f.db * rr[f.b])
            } else {
                0.0
            };
            tf.push(t);
            if t > 0.0 {
                ldu_owner.push(f.a);
                ldu_nb.push(f.b);
                face_ids.push(fi);
            }
        }
        let mut ldu = LduMatrix::new(ng, ldu_owner, ldu_nb);
        let mut b = vec![0.0; ng];
        for (slot, fi) in face_ids.iter().enumerate() {
            let f = &net.faces[*fi];
            let t = tf[*fi];
            ldu.upper[slot] = -t;
            ldu.lower[slot] = -t;
            ldu.diag[f.a] += t;
            ldu.diag[f.b] += t;
            // Gravity: F_ab gains T rho_f g (z_b - z_a).
            let gterm = t * 0.5 * (gases[f.a].rho + gases[f.b].rho) * GRAVITY * f.dz;
            b[f.a] -= gterm;
            b[f.b] += gterm;
        }
        for (gc, area, s) in &net.inflow {
            b[*gc] += inflow_mass(*s) * area;
        }
        for d in &mut ldu.diag {
            if *d == 0.0 {
                *d = 1.0;
            }
        }
        // The outlet (hot-plenum side face, an open volume without
        // buoyancy, see the module docs) is held at the reference pressure.
        let mut t_out = vec![0.0; net.outlet.len()];
        for (k, (gc, area, d, _)) in net.outlet.iter().enumerate() {
            t_out[k] = area / (d * rr[*gc]);
            ldu.diag[*gc] += t_out[k];
        }
        // No inflow and no gravity term (the loop stopped with buoyancy
        // ablated, gh:#1151 part B): the solution is p = 0 exactly, and the
        // solver's residual normalisation by |b| = 0 is undefined.
        if b.iter().all(|v| *v == 0.0) {
            p = vec![0.0; ng];
            flux.iter_mut().for_each(|f| *f = 0.0);
            out_flux.iter_mut().for_each(|f| *f = 0.0);
            for c in &net.cell_of_gas {
                g_cell[*c] = (0.0, 0.0);
            }
            picard = (it, 0.0);
            break;
        }
        let (x, perf) = conjugate_gradient(
            &ldu,
            &b,
            Some(&p),
            &SolverSettings {
                tolerance: 1e-13,
                max_iter: 200_000,
            },
        );
        if !perf.converged {
            return Err(Htr10RzError::Solve(format!(
                "gas pressure solve: residual {:e} after {} iterations",
                perf.final_residual, perf.n_iterations
            )));
        }
        p = x;
        for (fi, f) in net.faces.iter().enumerate() {
            flux[fi] = tf[fi] * ((p[f.a] - p[f.b]) + 0.5 * (gases[f.a].rho + gases[f.b].rho) * GRAVITY * f.dz);
        }
        for (k, (gc, ..)) in net.outlet.iter().enumerate() {
            out_flux[k] = t_out[k] * p[*gc];
        }
        // Cell mass flux: mean of the two opposite faces per direction.
        let mut sum_r = vec![(0.0, 0usize); ng];
        let mut sum_z = vec![(0.0, 0usize); ng];
        for (fi, f) in net.faces.iter().enumerate() {
            let gflux = flux[fi] / f.area;
            let s = if f.axial { &mut sum_z } else { &mut sum_r };
            s[f.a].0 += gflux;
            s[f.a].1 += 1;
            s[f.b].0 += gflux;
            s[f.b].1 += 1;
        }
        for (gc, area, s) in &net.inflow {
            let gflux = inflow_mass(*s) * area / area;
            let dir = if matches!(s, Stream::Bed | Stream::ControlRods) { 1.0 } else { -1.0 };
            sum_z[*gc].0 += dir * gflux;
            sum_z[*gc].1 += 1;
        }
        for (k, (gc, area, ..)) in net.outlet.iter().enumerate() {
            sum_r[*gc].0 += out_flux[k] / area;
            sum_r[*gc].1 += 1;
        }
        // Convergence is measured on what the Picard iterates: the cell
        // resistances `R(|G|)` (gh:#1175). See [`resistance_change`].
        let mut change: f64 = 0.0;
        for k in 0..ng {
            let c = net.cell_of_gas[k];
            let new = (
                if sum_r[k].1 > 0 { sum_r[k].0 / sum_r[k].1 as f64 } else { 0.0 },
                if sum_z[k].1 > 0 { sum_z[k].0 / sum_z[k].1 as f64 } else { 0.0 },
            );
            let old = g_cell[c];
            change = change.max(resistance_change(&net.region_of_gas[k], old, new, &gases[k])?);
            // Under-relaxed Picard on the resistance argument.
            g_cell[c] = (0.5 * (old.0 + new.0), 0.5 * (old.1 + new.1));
        }
        picard = (it, change);
        // ~~1e-7 on the cell flux: the pressure solve (relative residual
        // 1e-13) puts a round-off floor near 1e-9 under the cell flux of
        // small cells.~~ CORRECTED 2026-11-03 (gh:#1175): that floor, read
        // against the 1e-3 scale floor, is 1e-7 to 7e-7 on the 10 cm mesh,
        // so the flux measure could not reach 1e-7 there. The same 1e-7 now
        // applies to the resistances, which carry no such floor.
        if change < 1e-7 {
            break;
        }
        if it == 200 {
            return Err(Htr10RzError::Solve(format!("gas flow Picard did not converge: last change {change:e}")));
        }
    }

    // --- Gas energy: the inlet chain, then the 2-D domain (upwind sweep).
    let mut exchange = vec![0.0; n];
    let mut heat_to_gas = 0.0;
    let mut t_up = t_in;
    let mut taps = [0.0; 3];
    for (k, nd) in net.chain.iter().enumerate() {
        let f = nd.fraction * w;
        let gmid = gas(t_up, p_bar)?;
        let g = f / nd.flow_area;
        // No flow through the chain (the loop is stopped, gh:#1151 part B):
        // a mixed node would otherwise move heat between its cells through
        // the gas instantly. The stagnant gas is already in the cavity
        // medium's conductivity, so the chain exchanges nothing.
        let hv = if f > 0.0 { exchange_coefficient(&nd.region, g, &gmid)? } else { 0.0 };
        let (mut num, mut den) = (f * HELIUM_CP * t_up, f * HELIUM_CP);
        for c in &nd.cells {
            num += hv * net.volumes[*c] * t_solid[*c];
            den += hv * net.volumes[*c];
        }
        let t = if den > 0.0 { num / den } else { t_up };
        for c in &nd.cells {
            exchange[*c] = hv;
            t_gas[*c] = t;
            g_cell[*c] = (0.0, g);
            heat_to_gas += hv * net.volumes[*c] * (t_solid[*c] - t);
        }
        t_up = t;
        if k == net.tap_discharge {
            taps[0] = t;
        }
        if k == net.tap_leak {
            taps[1] = t;
        }
    }
    taps[2] = t_up;
    let t_stream = |s: Stream| match s {
        Stream::Bed | Stream::ControlRods => taps[2],
        Stream::DischargeTube => taps[0],
        Stream::Leakage => taps[1],
    };
    // Per gas cell: inflow from boundary (mass, mass x T), in-faces, out mass.
    let mut b_in = vec![(0.0, 0.0); ng];
    for (gc, area, s) in &net.inflow {
        let m = inflow_mass(*s) * area;
        b_in[*gc].0 += m;
        b_in[*gc].1 += m * t_stream(*s);
    }
    let mut out_mass = vec![0.0; ng];
    let mut ins: Vec<Vec<(usize, f64)>> = vec![Vec::new(); ng];
    for (fi, f) in net.faces.iter().enumerate() {
        let m = flux[fi];
        if m > 0.0 {
            out_mass[f.a] += m;
            ins[f.b].push((f.a, m));
        } else if m < 0.0 {
            out_mass[f.b] -= m;
            ins[f.a].push((f.b, -m));
        }
    }
    for (k, (gc, ..)) in net.outlet.iter().enumerate() {
        // Signed: a face with back-flow takes gas in at the cell's own
        // temperature (zero gradient), keeping the cell's mass balance.
        out_mass[*gc] += out_flux[k];
    }
    let hv_gas: Vec<f64> = (0..ng)
        .map(|k| {
            let c = net.cell_of_gas[k];
            let (gr, gz) = g_cell[c];
            exchange_coefficient(&net.region_of_gas[k], gr.r_hypot(gz), &gases[k])
        })
        .collect::<Result<_, _>>()?;
    // Kahn order on the upwind graph.
    let mut indeg: Vec<usize> = ins.iter().map(Vec::len).collect();
    let mut outs: Vec<Vec<usize>> = vec![Vec::new(); ng];
    for (b, list) in ins.iter().enumerate() {
        for (a, _) in list {
            outs[*a].push(b);
        }
    }
    let mut queue: Vec<usize> = (0..ng).filter(|k| indeg[*k] == 0).collect();
    let mut order = Vec::with_capacity(ng);
    while let Some(k) = queue.pop() {
        order.push(k);
        for &b in &outs[k] {
            indeg[b] -= 1;
            if indeg[b] == 0 {
                queue.push(b);
            }
        }
    }
    let mut tg: Vec<f64> = net.cell_of_gas.iter().map(|c| t_gas[*c]).collect();
    let update = |k: usize, tg: &[f64]| -> f64 {
        let c = net.cell_of_gas[k];
        let hv = hv_gas[k] * net.volumes[c];
        let mut num = HELIUM_CP * b_in[k].1 + hv * t_solid[c];
        for (a, m) in &ins[k] {
            num += HELIUM_CP * m * tg[*a];
        }
        num / (HELIUM_CP * out_mass[k] + hv)
    };
    for &k in &order {
        tg[k] = update(k, &tg);
    }
    if order.len() < ng {
        // Recirculation (gravity can drive it): Gauss-Seidel on the rest.
        let rest: Vec<usize> = (0..ng).filter(|k| indeg[*k] > 0).collect();
        let mut ok = false;
        for _ in 0..100_000 {
            let mut ch: f64 = 0.0;
            for &k in &rest {
                let v = update(k, &tg);
                ch = ch.max((v - tg[k]).abs());
                tg[k] = v;
            }
            if ch < 1e-10 {
                ok = true;
                break;
            }
        }
        if !ok {
            return Err(Htr10RzError::Solve("gas energy on a recirculating set did not converge".into()));
        }
    }
    let mut bed_out = (0.0, 0.0);
    for (k, c) in net.cell_of_gas.iter().enumerate() {
        t_gas[*c] = tg[k];
        exchange[*c] = hv_gas[k];
        heat_to_gas += hv_gas[k] * net.volumes[*c] * (t_solid[*c] - tg[k]);
    }
    for (fi, f) in net.faces.iter().enumerate() {
        let (ra, rb) = (net.regions[net.cell_of_gas[f.a]], net.regions[net.cell_of_gas[f.b]]);
        if (ra == idx::BED || ra == idx::DISCHARGE_TUBE) && rb == idx::UPPER_BOTTOM_REFL && flux[fi] > 0.0 {
            bed_out.0 += flux[fi];
            bed_out.1 += flux[fi] * tg[f.a];
        }
    }
    let (mut mo, mut mt) = (0.0, 0.0);
    let (mut ao, mut at) = (0.0, 0.0);
    for (k, (gc, area, ..)) in net.outlet.iter().enumerate() {
        mo += out_flux[k];
        mt += out_flux[k] * tg[*gc];
        ao += area;
        at += area * tg[*gc];
    }
    // With the loop stopped (w = 0) the outlet carries no net flow: report
    // the area mean of the outlet cells' gas instead of a mixed-cup value
    // (gh:#1151 part B). Same for the bed outlet when nothing leaves it.
    let t_out = if w > 0.0 && mo > 1e-6 * w { mt / mo } else { at / ao };
    if !(bed_out.0 > 0.0) {
        bed_out = (1.0, f64::NAN);
    }
    let mut pressure = vec![f64::NAN; n];
    let mut p_bed_in = (0.0, 0.0);
    for (k, c) in net.cell_of_gas.iter().enumerate() {
        pressure[*c] = p[k];
    }
    for (gc, area, s) in &net.inflow {
        if *s == Stream::Bed {
            p_bed_in.0 += area;
            p_bed_in.1 += area * p[*gc];
        }
    }
    for c in 0..n {
        if net.gas_of_cell[c].is_none() && !net.in_chain[c] {
            t_gas[c] = f64::NAN;
            g_cell[c] = (0.0, 0.0);
        }
    }
    Ok(GasState {
        temperature: t_gas,
        mass_flux: g_cell,
        pressure,
        exchange,
        outlet_temperature: t_out,
        bed_outlet_temperature: bed_out.1 / bed_out.0,
        heat_to_gas,
        enthalpy_rise: w * HELIUM_CP * (t_out - t_in),
        flow_picard: picard,
        density_iterations: (1, f64::NAN),
        core_pressure_drop: p_bed_in.1 / p_bed_in.0,
        tap_temperatures: taps,
        outlet_mass_flow: mo,
    })
}
