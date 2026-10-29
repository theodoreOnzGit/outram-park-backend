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
/// Zr + 2 H2O -> ZrO2 + 2 H2: heat released per kg Zr, J/kg. \[U\] about
/// 586 kJ/mol, 6.45 MJ/kg Zr.
pub const OXIDATION_HEAT_J_PER_KG_ZR: f64 = 6.45e6;
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
/// Hydrogen in air: lower flammability limit (upward propagation) and a mole
/// fraction above which a deflagration is strong enough to wreck the
/// building's upper floor. \[P\] 4 % (Shapiro diagram); \[A\] 8 %.
pub const H2_FLAMMABLE: f64 = 0.04;
pub const H2_EXPLOSION: f64 = 0.08;
/// Atmosphere, Pa.
pub const P_ATM: f64 = 1.013e5;

// ── Derived core geometry ───────────────────────────────────────────────────

/// UO2 mass of the whole core, kg (from the pellet geometry).
pub fn core_uo2_kg() -> f64 {
    ASSEMBLIES * RODS_PER_ASSEMBLY * std::f64::consts::PI * PELLET_R_M.r_powi(2) * ACTIVE_LENGTH_M * UO2_DENSITY
}

/// Zircaloy mass of the whole core (cladding over the active length plus
/// channel boxes), kg.
pub fn core_zr_kg() -> f64 {
    let ri = CLAD_OUTER_R_M - CLAD_WALL_M;
    let clad = ASSEMBLIES * RODS_PER_ASSEMBLY * std::f64::consts::PI * (CLAD_OUTER_R_M.r_powi(2) - ri.r_powi(2)) * ACTIVE_LENGTH_M * ZR_DENSITY;
    let channel = ASSEMBLIES * 4.0 * (CHANNEL_INNER_WIDTH_M + CHANNEL_WALL_M) * CHANNEL_WALL_M * CHANNEL_LENGTH_M * ZR_DENSITY;
    clad + channel
}

/// Zircaloy surface exposed to steam over the active length, m^2: the rods'
/// outer surface and both faces of the channel walls.
pub fn core_zr_area_m2() -> f64 {
    let rods = ASSEMBLIES * RODS_PER_ASSEMBLY * 2.0 * std::f64::consts::PI * CLAD_OUTER_R_M * ACTIVE_LENGTH_M;
    let channel = ASSEMBLIES * 2.0 * 4.0 * CHANNEL_INNER_WIDTH_M * ACTIVE_LENGTH_M;
    rods + channel
}

/// Rod surface over the active length, m^2 (the heat-transfer surface).
pub fn core_rod_area_m2() -> f64 {
    ASSEMBLIES * RODS_PER_ASSEMBLY * 2.0 * std::f64::consts::PI * CLAD_OUTER_R_M * ACTIVE_LENGTH_M
}

/// Coolant flow area inside the channels, m^2, and their hydraulic diameter, m.
pub fn core_flow_area_m2() -> f64 {
    ASSEMBLIES * (CHANNEL_INNER_WIDTH_M.r_powi(2) - RODS_PER_ASSEMBLY * std::f64::consts::PI * CLAD_OUTER_R_M.r_powi(2))
}
pub fn hydraulic_diameter_m() -> f64 {
    let a = CHANNEL_INNER_WIDTH_M.r_powi(2) - RODS_PER_ASSEMBLY * std::f64::consts::PI * CLAD_OUTER_R_M.r_powi(2);
    let wet = RODS_PER_ASSEMBLY * 2.0 * std::f64::consts::PI * CLAD_OUTER_R_M + 4.0 * CHANNEL_INNER_WIDTH_M;
    4.0 * a / wet
}

/// Fuel-to-clad conductance per unit rod length, W/(m K): the pellet's
/// volume-average-to-surface resistance `1/(8 pi k)` plus the gap.
pub fn fuel_to_clad_w_per_m_k() -> f64 {
    1.0 / (1.0 / (8.0 * std::f64::consts::PI * UO2_K) + 1.0 / (2.0 * std::f64::consts::PI * PELLET_R_M * GAP_H))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derived core inventories are the right size for a 400-bundle
    /// BWR/3: about 76 t of UO2 (about 67 t of uranium) and about 30 t of
    /// Zircaloy; an 8x8 channel's hydraulic diameter is about 1.4 cm.
    /// A harness check on the arithmetic, not a validation.
    #[test]
    fn the_derived_core_is_the_right_size() {
        let uo2 = core_uo2_kg();
        assert!((70e3..82e3).contains(&uo2), "{uo2}");
        let zr = core_zr_kg();
        assert!((25e3..35e3).contains(&zr), "{zr}");
        let dh = hydraulic_diameter_m();
        assert!((0.012..0.016).contains(&dh), "{dh}");
    }
}
