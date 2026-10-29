//! Fukushima Daiichi Unit 1 (GE BWR/3, Mark I containment): every number the
//! model uses, with where it came from.
//!
//! **Read this before quoting anything the simulator shows.** This is an AI
//! draft (2026-10-21). The numbers were gathered by a literature search that
//! could read search-result excerpts but **not the primary PDFs**. Each is
//! marked:
//!
//! - **\[P\]**: a published figure from a named source, not yet checked against
//!   the primary document;
//! - **\[U\]**: unverified (from memory, or an ambiguous excerpt);
//! - **\[A\]**: an assumption or placeholder of this model's, with no source.
//!   What would replace it is said at the line.
//!
//! The sources the design data should be checked against (all openly
//! redistributable): INPO 11-005 (2011) Tables 7.3-1/7.3-2; Gauntt et al.,
//! SAND2012-6173 (Sandia, MELCOR models of Units 1-3); IAEA, *The Fukushima
//! Daiichi Accident*, Technical Volume 1 (2015); the Japanese government's
//! June 2011 report to the IAEA, attachment 4; TEPCO's plant parameter PDFs
//! (the measured data, for comparison only). The OECD/NEA BSAF specification
//! is restricted and is **not** used.

use petir::mathf::RealMath;
use super::wire::{Reader, Writer};

// ── Reactor ────────────────────────────────────────────────────────────────

/// Rated thermal power, W. \[P\] TEPCO facility overview: 1380 MWt (460 MWe).
pub const THERMAL_POWER_W: f64 = 1380.0e6;
/// Fuel assemblies in the core. \[P\] TEPCO: 400.
pub const ASSEMBLIES: f64 = 400.0;
/// Fuel rods per assembly: a GE 8x8 bundle with 62 fuel rods (two water
/// rods). \[U\] Unit 1's 2011 core lattice is not confirmed (INPO Table 7.3).
pub const RODS_PER_ASSEMBLY: f64 = 62.0;
/// Active fuel length, m: GE's 12 ft (144 in). \[U\] for Unit 1.
pub const ACTIVE_LENGTH_M: f64 = 3.66;
/// Clad outer radius, m (12.27 mm OD, GE 8x8). \[U\]
pub const CLAD_OUTER_R_M: f64 = 0.006135;
/// Clad wall, m (0.81 mm). \[U\]
pub const CLAD_WALL_M: f64 = 0.00081;
/// Pellet radius, m (10.4 mm diameter). \[U\]
pub const PELLET_R_M: f64 = 0.0052;
/// UO2 density, kg/m^3: 95 % of 10,960 theoretical. \[A\] typical LWR value.
pub const UO2_DENSITY: f64 = 10_412.0;
/// Zircaloy density, kg/m^3. \[P\] MATPRO, about 6550.
pub const ZR_DENSITY: f64 = 6550.0;
/// Channel box: inner width, m (5.278 in), wall (80 mil), length (about 4.2 m).
/// \[U\] GE BWR/3-4 channel; counted in the zirconium that oxidises.
pub const CHANNEL_INNER_WIDTH_M: f64 = 0.134;
pub const CHANNEL_WALL_M: f64 = 0.002032;
pub const CHANNEL_LENGTH_M: f64 = 4.2;
/// Specific heats, J/(kg K), held constant. \[A\] UO2 is about 300 near 600 K
/// and 450 near 2500 K, Zircaloy about 330 with a peak at its alpha-beta
/// transition (MATPRO). Temperature-dependent values from the OFFBEAT port
/// (`outram-park-fork-offbeat`) would replace these.
pub const UO2_CP: f64 = 330.0;
pub const ZR_CP: f64 = 350.0;
/// UO2 thermal conductivity, W/(m K), constant. \[A\] about 3 near 1000 K
/// (MATPRO); replace with the OFFBEAT port's UO2 conductivity.
pub const UO2_K: f64 = 3.0;
/// Pellet-clad gap conductance, W/(m^2 K). \[A\] typical closed-gap value.
pub const GAP_H: f64 = 5000.0;

/// Nucleate-boiling heat transfer coefficient from a covered rod to the
/// water, W/(m^2 K). \[A\] Order of magnitude for pool boiling at decay-heat
/// flux; at this value a covered rod sits within a few kelvin of T_sat, so
/// the answer does not hinge on it. A boiling correlation (Chen, in the
/// GeN-Foam closures) would replace it.
pub const BOILING_H: f64 = 2.0e4;

/// Radial power factors of three equal-area rings, centre outwards, mean 1.
/// \[A\] an illustrative flattened BWR shape.
pub const RING_FACTORS: [f64; 3] = [1.2, 1.0, 0.8];
/// Axial nodes over the active length.
pub const AXIAL_NODES: usize = 12;
/// Extrapolation factor of the chopped-cosine axial shape: the cosine's
/// zeros lie this many active lengths apart. \[A\]
pub const COSINE_EXTRAPOLATION: f64 = 1.3;

// ── Pressure vessel ─────────────────────────────────────────────────────────

/// RPV inner radius, m (about 4.8 m ID). \[U\] SAND2012-6173 / INPO Table 7.3-2.
pub const RPV_RADIUS_M: f64 = 2.4;
/// RPV inner height, m. \[U\] about 20 m.
pub const RPV_HEIGHT_M: f64 = 20.0;
/// Bottom of active fuel above the vessel bottom, m. \[A\]
pub const BAF_M: f64 = 5.0;
/// Fraction of the vessel's cross-section free for water below the core
/// (control-rod guide tubes, the bottom head's curvature) and above it
/// (separators, dryers). \[A\]
pub const LOWER_PLENUM_FREE: f64 = 0.75;
pub const UPPER_FREE: f64 = 0.85;
/// Cross-section taken in the core region by the shroud, jet pumps and
/// control blades besides the fuel rods and channels, m^2. \[A\]
pub const CORE_OTHER_AREA_M2: f64 = 1.0;
/// Normal water level above the top of active fuel, m. \[U\]
pub const NORMAL_LEVEL_ABOVE_TAF_M: f64 = 4.4;
/// Steam-dome pressure at the scram, Pa absolute. \[P\] about 7 MPa
/// (6.93 MPa gauge rated).
pub const INITIAL_PRESSURE_PA: f64 = 7.03e6;

/// Main steam isolation valves close this long after the scram, s; until
/// then the pressure regulator keeps passing steam to the turbine bypass and
/// condenser, holding the dome at its initial pressure. \[U\] INPO 11-005:
/// MSIVs closed at about 14:47 on the loss of off-site power.
pub const MSIV_CLOSURE_S: f64 = 60.0;
/// Pressure-regulator gain while the bypass is open, kg/s per Pa above the
/// initial pressure. \[A\] (any gain that holds the pressure within a few kPa).
pub const BYPASS_GAIN_KG_S_PA: f64 = 1.0e-3;

/// Relief valves: Unit 1 has electromatic relief valves plus spring safety
/// valves. \[U\] count, set points and throat. While DC power lasts the relief
/// valves lift at the relief set point; after the station blackout only the
/// spring safety valves act, at their higher set point.
pub const RELIEF_VALVES: f64 = 4.0;
pub const RELIEF_OPEN_PA: f64 = 7.37e6 + 1.013e5;
pub const RELIEF_CLOSE_PA: f64 = 7.0e6 + 1.013e5;
pub const SAFETY_OPEN_PA: f64 = 8.3e6;
pub const SAFETY_CLOSE_PA: f64 = 7.9e6;
/// Throat area of one relief valve, m^2. \[A\] about 87 kg/s of steam each at
/// 7.4 MPa through the steam tables' choked-flow solution.
pub const RELIEF_THROAT_M2: f64 = 0.0081;

/// Isolation condenser trains (A and B). \[P\] TEPCO: two.
pub const IC_TRAINS: usize = 2;
/// Auto-start on high RPV pressure, Pa absolute. \[P\] TEPCO: 7.13 MPa gauge
/// for 15 s.
pub const IC_AUTOSTART_PA: f64 = 7.13e6 + 1.013e5;
/// Heat removal of one IC train: `UA (T_sat - T_shell)`, W/K. \[A\]
/// **Placeholder**: about 43 MW per train at 7 MPa against 100 degC shell
/// water. The per-train rating was not found in open sources; SAND2012-6173
/// or the IAEA Technical Volume 1 would replace it.
pub const IC_UA_W_PER_K: f64 = 2.3e5;
/// Shell-side water per train, kg. \[A\] placeholder, about 100 m^3 each
/// (open sources quote about 8 h of inventory for both trains).
pub const IC_SHELL_WATER_KG: f64 = 1.0e5;

/// Fire-engine injection (fresh water, later seawater): shut-off pressure,
/// Pa absolute, and flow against atmospheric back-pressure, kg/s. \[A\] A pump
/// curve `W = W_max sqrt(1 - (p - p_atm)/(p_shut - p_atm))`.
pub const FIRE_ENGINE_SHUTOFF_PA: f64 = 1.0e6;
pub const FIRE_ENGINE_MAX_KG_S: f64 = 4.0;
pub const INJECTION_TEMPERATURE_K: f64 = 293.15;

/// RPV leak to the drywell once the core-exit gas is hot (instrument lines,
/// SRV gaskets; the BSAF Unit 1 analyses needed such a path). \[A\] Opens above
/// [`HOT_LEAK_GAS_K`] with throat [`HOT_LEAK_M2`].
pub const HOT_LEAK_GAS_K: f64 = 1000.0;
pub const HOT_LEAK_M2: f64 = 3.0e-4;
/// Lower-head failure: debris in the dry lower plenum above this, K, opens a
/// breach of this throat, m^2. \[A\] both.
pub const LOWER_HEAD_FAILURE_K: f64 = 1500.0;
pub const BREACH_M2: f64 = 0.01;

/// Time over which debris in a wet lower plenum gives up its stored heat,
/// s. \[A\] (quench of a debris bed takes minutes).
pub const DEBRIS_QUENCH_S: f64 = 120.0;

/// Zircaloy relocation: a node whose cladding reaches this, K, melts and
/// slumps to the lower plenum. \[U\] Zircaloy-4 melts near 2100 K.
pub const ZR_MELT_K: f64 = 2100.0;

// ── Zirconium-steam oxidation ───────────────────────────────────────────────
//
// Parabolic: d(W^2)/dt = K(T), W = mass of Zr oxidised per unit area,
// kg/m^2. Urbanic-Heidrick (1978), MELCOR's default. [U] constants as quoted
// by the literature search from the MELCOR COR reference manual (public SAND
// report); check against it before quoting. Between the two branches the rate
// is interpolated linearly in T.
pub const UH_LOW_A: f64 = 29.6;
pub const UH_LOW_B_K: f64 = 16_820.0;
pub const UH_HIGH_A: f64 = 87.9;
pub const UH_HIGH_B_K: f64 = 16_610.0;
pub const UH_LOW_MAX_K: f64 = 1853.0;
pub const UH_HIGH_MIN_K: f64 = 1873.0;
/// Below this the oxidation is negligible on the accident's time scale. \[A\]
pub const OXIDATION_MIN_K: f64 = 1000.0;
/// Zr + 2 H2O(g) -> ZrO2 + 2 H2: heat released per kg Zr, J/kg.
/// ~~\[U\] about 586 kJ/mol, 6.45 MJ/kg Zr~~ (`6.45e6`; also inconsistent with
/// itself: 586 kJ/mol / 91.224 g/mol is 6.42 MJ/kg). **CORRECTED 2026-10-29
/// (gh:#1038):** OFFBEAT's
/// `corrosion::exotherm::ENERGY_PER_KG_ZIRCONIUM`, reused, derived from the
/// standard formation enthalpies of monoclinic ZrO2 (-1100.6 kJ/mol) and
/// water vapour (-241.826 kJ/mol) at 298.15 K: 616.95 kJ/mol, **6.763 MJ/kg**.
/// \[P\] for the formation enthalpies; the derivation is that module's, not
/// fitted to anything. Not modelled: the drift of the reaction enthalpy with
/// temperature (Kirchhoff), which that module puts at a few per cent over
/// 300-2000 K (not measured here).
pub const OXIDATION_HEAT_J_PER_KG_ZR: f64 =
    outram_park_fork_offbeat::corrosion::exotherm::ENERGY_PER_KG_ZIRCONIUM;
/// Molar masses, kg/mol.
pub const M_ZR: f64 = 0.091_224;
pub const M_H2O: f64 = 0.018_015;
pub const M_H2: f64 = 0.002_016;
pub const M_N2: f64 = 0.028_014;
pub const R_GAS: f64 = 8.314_462;

// ── Containment (Mark I) ────────────────────────────────────────────────────

/// Drywell free volume, m^3. \[U\] about 3410 (KTH slides, simplyinfo).
pub const DRYWELL_VOLUME_M3: f64 = 3410.0;
/// Wetwell (torus) gas space, m^3, and pool water, m^3. \[P\] Japanese
/// government report to the IAEA, June 2011, attachment 4: 2620 and 1750.
pub const WETWELL_GAS_M3: f64 = 2620.0;
pub const POOL_WATER_M3: f64 = 1750.0;
/// Primary containment design pressure, Pa absolute. \[P\] 0.43 MPa gauge
/// (NISA, via WNA).
pub const CONTAINMENT_DESIGN_PA: f64 = 0.43e6 + 1.013e5;
/// Initial containment state: inerted with nitrogen. \[A\] pressure, Pa
/// absolute, and temperatures, K.
pub const INITIAL_CONTAINMENT_PA: f64 = 1.05e5;
pub const INITIAL_DRYWELL_K: f64 = 323.15;
pub const INITIAL_POOL_K: f64 = 303.15;
/// Drywell to wetwell downcomers: submergence head, Pa (about 1.2 m of
/// water), total flow area, m^2, and discharge coefficient. \[A\]
pub const VENT_SUBMERGENCE_PA: f64 = 11_800.0;
pub const DOWNCOMER_AREA_M2: f64 = 20.0;
pub const DISCHARGE_COEFFICIENT: f64 = 0.6;
/// Wetwell-to-drywell vacuum breakers: opening differential, Pa, and area,
/// m^2. \[A\]
pub const VACUUM_BREAKER_PA: f64 = 3400.0;
pub const VACUUM_BREAKER_M2: f64 = 0.5;
/// Wetwell vent line to the stack: rupture-disk burst pressure, Pa absolute,
/// and flow area, m^2. \[U\] burst pressure; \[A\] area.
pub const VENT_RUPTURE_DISK_PA: f64 = 0.549e6;
pub const VENT_AREA_M2: f64 = 0.07;
/// Drywell head-flange leak to the reactor building at high pressure (the
/// leading explanation for Unit 1's hydrogen in the building). \[A\] opens
/// above this pressure, Pa absolute, with this area, m^2.
pub const FLANGE_LEAK_PA: f64 = 0.75e6;
pub const FLANGE_LEAK_M2: f64 = 5.0e-4;
/// Reactor building free volume, m^3. \[A\] placeholder (about 38,000 is
/// quoted for Unit 3).
pub const REACTOR_BUILDING_M3: f64 = 26_000.0;
/// Hydrogen in air: lower flammability limit (upward propagation).
/// \[P\] 4 % (Shapiro diagram).
pub const H2_FLAMMABLE: f64 = 0.04;
/// Hydrogen mole fraction above which a deflagration is strong enough to
/// wreck the building's upper floor. \[A\] 8 %.
pub const H2_EXPLOSION: f64 = 0.08;
/// Atmosphere, Pa.
pub const P_ATM: f64 = 1.013e5;

// ── The inputs as one value ─────────────────────────────────────────────────

/// Where an input's default comes from (the module docs' marks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// A published figure from a named source, not yet checked against the
    /// primary document.
    P,
    /// Unverified (from memory, or an ambiguous excerpt).
    U,
    /// An assumption or placeholder of this model's, with no source.
    A,
}

/// Declares [`Params`] from one list, so its fields, their defaults (the
/// documented constants above), their tags and the order they cross the
/// worker wire in cannot drift apart. A test checks each tag against the
/// constant's doc comment.
macro_rules! params {
    ($($field:ident = $default:ident, $tag:ident,)*) => {
        /// Every scalar input of the model, as one value (gh:#1033): what a
        /// caller changes to run the plant with a corrected or perturbed
        /// input, without recompiling. [`Params::default`] is the constants
        /// above, field for field (`thermal_power_w` is [`THERMAL_POWER_W`],
        /// and so on), so their tags and sources are the defaults'. Left as
        /// constants: the node and train counts, which size arrays
        /// ([`AXIAL_NODES`], [`IC_TRAINS`]), and physical constants (molar
        /// masses, [`R_GAS`], [`P_ATM`]).
        ///
        /// It travels whole in each part's snapshot, so a worker steps with
        /// the plant's inputs and one worker still equals three.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Params {
            $(pub $field: f64,)*
            /// [`RING_FACTORS`].
            pub ring_factors: [f64; 3],
        }

        impl Default for Params {
            fn default() -> Self {
                Self { $($field: $default,)* ring_factors: RING_FACTORS }
            }
        }

        impl Params {
            pub fn write(&self, w: &mut Writer) {
                w.f(&[$(self.$field,)*]);
                w.f(&self.ring_factors);
            }
            pub fn read(r: &mut Reader) -> Self {
                Self { $($field: r.f(),)* ring_factors: [r.f(), r.f(), r.f()] }
            }

            /// Every scalar input by field name, with its tag (the
            /// default's, from its doc comment), in declaration order.
            pub const INPUTS: &[(&str, Tag)] = &[$((stringify!($field), Tag::$tag),)*];

            /// The input named `name` (a field name from [`Params::INPUTS`]).
            pub fn get(&self, name: &str) -> Option<f64> {
                match name {
                    $(stringify!($field) => Some(self.$field),)*
                    _ => None,
                }
            }

            /// Sets the input named `name`; `false` if there is none.
            pub fn set(&mut self, name: &str, value: f64) -> bool {
                match name {
                    $(stringify!($field) => { self.$field = value; true })*
                    _ => false,
                }
            }
        }
    };
}

params! {
    thermal_power_w = THERMAL_POWER_W, P,
    assemblies = ASSEMBLIES, P,
    rods_per_assembly = RODS_PER_ASSEMBLY, U,
    active_length_m = ACTIVE_LENGTH_M, U,
    clad_outer_r_m = CLAD_OUTER_R_M, U,
    clad_wall_m = CLAD_WALL_M, U,
    pellet_r_m = PELLET_R_M, U,
    uo2_density = UO2_DENSITY, A,
    zr_density = ZR_DENSITY, P,
    channel_inner_width_m = CHANNEL_INNER_WIDTH_M, U,
    channel_wall_m = CHANNEL_WALL_M, U,
    channel_length_m = CHANNEL_LENGTH_M, U,
    uo2_cp = UO2_CP, A,
    zr_cp = ZR_CP, A,
    uo2_k = UO2_K, A,
    gap_h = GAP_H, A,
    boiling_h = BOILING_H, A,
    cosine_extrapolation = COSINE_EXTRAPOLATION, A,
    rpv_radius_m = RPV_RADIUS_M, U,
    rpv_height_m = RPV_HEIGHT_M, U,
    baf_m = BAF_M, A,
    lower_plenum_free = LOWER_PLENUM_FREE, A,
    upper_free = UPPER_FREE, A,
    core_other_area_m2 = CORE_OTHER_AREA_M2, A,
    normal_level_above_taf_m = NORMAL_LEVEL_ABOVE_TAF_M, U,
    initial_pressure_pa = INITIAL_PRESSURE_PA, P,
    msiv_closure_s = MSIV_CLOSURE_S, U,
    bypass_gain_kg_s_pa = BYPASS_GAIN_KG_S_PA, A,
    relief_valves = RELIEF_VALVES, U,
    relief_open_pa = RELIEF_OPEN_PA, U,
    relief_close_pa = RELIEF_CLOSE_PA, U,
    safety_open_pa = SAFETY_OPEN_PA, U,
    safety_close_pa = SAFETY_CLOSE_PA, U,
    relief_throat_m2 = RELIEF_THROAT_M2, A,
    ic_autostart_pa = IC_AUTOSTART_PA, P,
    ic_ua_w_per_k = IC_UA_W_PER_K, A,
    ic_shell_water_kg = IC_SHELL_WATER_KG, A,
    fire_engine_shutoff_pa = FIRE_ENGINE_SHUTOFF_PA, A,
    fire_engine_max_kg_s = FIRE_ENGINE_MAX_KG_S, A,
    injection_temperature_k = INJECTION_TEMPERATURE_K, A,
    hot_leak_gas_k = HOT_LEAK_GAS_K, A,
    hot_leak_m2 = HOT_LEAK_M2, A,
    lower_head_failure_k = LOWER_HEAD_FAILURE_K, A,
    breach_m2 = BREACH_M2, A,
    debris_quench_s = DEBRIS_QUENCH_S, A,
    zr_melt_k = ZR_MELT_K, U,
    uh_low_a = UH_LOW_A, U,
    uh_low_b_k = UH_LOW_B_K, U,
    uh_high_a = UH_HIGH_A, U,
    uh_high_b_k = UH_HIGH_B_K, U,
    uh_low_max_k = UH_LOW_MAX_K, U,
    uh_high_min_k = UH_HIGH_MIN_K, U,
    oxidation_min_k = OXIDATION_MIN_K, A,
    oxidation_heat_j_per_kg_zr = OXIDATION_HEAT_J_PER_KG_ZR, P,
    drywell_volume_m3 = DRYWELL_VOLUME_M3, U,
    wetwell_gas_m3 = WETWELL_GAS_M3, P,
    pool_water_m3 = POOL_WATER_M3, P,
    containment_design_pa = CONTAINMENT_DESIGN_PA, P,
    initial_containment_pa = INITIAL_CONTAINMENT_PA, A,
    initial_drywell_k = INITIAL_DRYWELL_K, A,
    initial_pool_k = INITIAL_POOL_K, A,
    vent_submergence_pa = VENT_SUBMERGENCE_PA, A,
    downcomer_area_m2 = DOWNCOMER_AREA_M2, A,
    discharge_coefficient = DISCHARGE_COEFFICIENT, A,
    vacuum_breaker_pa = VACUUM_BREAKER_PA, A,
    vacuum_breaker_m2 = VACUUM_BREAKER_M2, A,
    vent_rupture_disk_pa = VENT_RUPTURE_DISK_PA, A,
    vent_area_m2 = VENT_AREA_M2, A,
    flange_leak_pa = FLANGE_LEAK_PA, A,
    flange_leak_m2 = FLANGE_LEAK_M2, A,
    reactor_building_m3 = REACTOR_BUILDING_M3, A,
    h2_flammable = H2_FLAMMABLE, P,
    h2_explosion = H2_EXPLOSION, A,
}

/// Derived core geometry.
impl Params {
    /// UO2 mass of the whole core, kg (from the pellet geometry).
    pub fn core_uo2_kg(&self) -> f64 {
        self.assemblies * self.rods_per_assembly * std::f64::consts::PI * self.pellet_r_m.r_powi(2) * self.active_length_m * self.uo2_density
    }

    /// Zircaloy mass of the whole core (cladding over the active length plus
    /// channel boxes), kg.
    pub fn core_zr_kg(&self) -> f64 {
        let ri = self.clad_outer_r_m - self.clad_wall_m;
        let clad = self.assemblies * self.rods_per_assembly * std::f64::consts::PI * (self.clad_outer_r_m.r_powi(2) - ri.r_powi(2)) * self.active_length_m * self.zr_density;
        let channel = self.assemblies * 4.0 * (self.channel_inner_width_m + self.channel_wall_m) * self.channel_wall_m * self.channel_length_m * self.zr_density;
        clad + channel
    }

    /// Zircaloy surface exposed to steam over the active length, m^2: the rods'
    /// outer surface and both faces of the channel walls.
    pub fn core_zr_area_m2(&self) -> f64 {
        let rods = self.assemblies * self.rods_per_assembly * 2.0 * std::f64::consts::PI * self.clad_outer_r_m * self.active_length_m;
        let channel = self.assemblies * 2.0 * 4.0 * self.channel_inner_width_m * self.active_length_m;
        rods + channel
    }

    /// Rod surface over the active length, m^2 (the heat-transfer surface).
    pub fn core_rod_area_m2(&self) -> f64 {
        self.assemblies * self.rods_per_assembly * 2.0 * std::f64::consts::PI * self.clad_outer_r_m * self.active_length_m
    }

    /// Coolant flow area inside the channels, m^2, and their hydraulic diameter, m.
    pub fn core_flow_area_m2(&self) -> f64 {
        self.assemblies * (self.channel_inner_width_m.r_powi(2) - self.rods_per_assembly * std::f64::consts::PI * self.clad_outer_r_m.r_powi(2))
    }
    pub fn hydraulic_diameter_m(&self) -> f64 {
        let a = self.channel_inner_width_m.r_powi(2) - self.rods_per_assembly * std::f64::consts::PI * self.clad_outer_r_m.r_powi(2);
        let wet = self.rods_per_assembly * 2.0 * std::f64::consts::PI * self.clad_outer_r_m + 4.0 * self.channel_inner_width_m;
        4.0 * a / wet
    }

    /// Fuel-to-clad conductance per unit rod length, W/(m K): the pellet's
    /// volume-average-to-surface resistance `1/(8 pi k)` plus the gap.
    pub fn fuel_to_clad_w_per_m_k(&self) -> f64 {
        1.0 / (1.0 / (8.0 * std::f64::consts::PI * self.uo2_k) + 1.0 / (2.0 * std::f64::consts::PI * self.pellet_r_m * self.gap_h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each input's tag in [`Params::INPUTS`] is the one its constant's doc
    /// comment gives: the last mark on the nearest marked doc line above
    /// it (a constant with no doc of its own shares the block above). A
    /// harness check that the table and the docs cannot drift apart.
    #[test]
    fn every_input_carries_its_documented_tag() {
        let src: Vec<&str> = include_str!("params.rs").lines().collect();
        for &(name, tag) in Params::INPUTS {
            let konst = format!("pub const {}: f64", name.to_uppercase());
            let at = src.iter().position(|l| l.starts_with(&konst)).unwrap_or_else(|| panic!("{konst}"));
            let documented = src[..at]
                .iter()
                .rev()
                .take_while(|l| l.starts_with("//") || l.starts_with("pub const"))
                .find_map(|l| {
                    let marks: Vec<Tag> = l
                        .match_indices('[')
                        .filter_map(|(i, _)| {
                            // `[U]` in a plain comment, `\[U\]` in a doc.
                            let rest = &l[i + 1..];
                            let closed = rest.get(1..2).is_some_and(|c| c == "]" || c == "\\");
                            match rest.chars().next() {
                                Some('P') if closed => Some(Tag::P),
                                Some('U') if closed => Some(Tag::U),
                                Some('A') if closed => Some(Tag::A),
                                _ => None,
                            }
                        })
                        .collect();
                    marks.last().copied()
                });
            assert_eq!(documented, Some(tag), "{name}");
        }
        // Get and set by name reach the fields.
        let mut p = Params::default();
        assert!(p.set("zr_melt_k", 2000.0) && !p.set("no_such_input", 1.0));
        assert_eq!((p.get("zr_melt_k"), p.zr_melt_k, p.get("no_such_input")), (Some(2000.0), 2000.0, None));
        assert_eq!(Params::INPUTS.len(), 73);
    }

    /// The derived core inventories are the right size for a 400-bundle
    /// BWR/3: about 76 t of UO2 (about 67 t of uranium) and about 30 t of
    /// Zircaloy; an 8x8 channel's hydraulic diameter is about 1.4 cm.
    /// A harness check on the arithmetic, not a validation.
    #[test]
    fn the_derived_core_is_the_right_size() {
        let uo2 = Params::default().core_uo2_kg();
        assert!((70e3..82e3).contains(&uo2), "{uo2}");
        let zr = Params::default().core_zr_kg();
        assert!((25e3..35e3).contains(&zr), "{zr}");
        let dh = Params::default().hydraulic_diameter_m();
        assert!((0.012..0.016).contains(&dh), "{dh}");
    }
}
