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
/// RPV wall thickness, m, for its heat capacity (`Mechanisms::rpv_wall`).
/// \[A\] about 6 in, typical of a BWR/3 shell; not in a held source.
pub const RPV_WALL_M: f64 = 0.16;
/// The dome internals' envelope facing the RPV wall, m^2, for their
/// radiation to it (`Mechanisms::internals_radiation`). \[A\] a 4 m tall
/// cylinder at the vessel's radius with its top: `2 pi r 4 + pi r^2`.
pub const INTERNALS_ENVELOPE_M2: f64 = 78.4;
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

/// Main steam lines (gh:#1037, `msl.rs`): number, inner radius, wall
/// thickness, m. \[A\] all three: Unit 1's line size is not in a held
/// source. The radius is sized for its rated steam flow, about 667 kg/s
/// over two lines at about 45 m/s and 36.5 kg/m^3 (inner diameter about
/// 0.51 m). The wall is the ASME thickness `p D / (2 S)` at 8.6 MPa design
/// pressure and S about 120 MPa (18 mm), plus allowances.
pub const MSL_LINES: f64 = 2.0;
pub const MSL_INNER_RADIUS_M: f64 = 0.254;
pub const MSL_WALL_M: f64 = 0.025;
/// Insulation conductance to the drywell, W/(m^2 K). \[A\] about 0.05 W/(m
/// K) through 0.1 m of insulation is 0.5; 1 is used.
pub const MSL_INSULATION_H: f64 = 1.0;
/// Larson-Miller coefficients of the line's steel: MELCOR's default for
/// A-508, "class 2 carbon steel" (Vol. 1, SAND2017-0455 O, CF Table 1;
/// stress in Pa, T in K, time in s): a1 K, b1 K, C. The coefficients are
/// \[P\]; the choice of A-508 for Unit 1's line is an assumption (its grade
/// is not in a held source).
pub const MSL_LM_A1: f64 = -5335.0;
pub const MSL_LM_B1: f64 = 62291.3;
pub const MSL_LM_C: f64 = 16.44;

/// The steam dome's internals as one heat structure (gh:#1037,
/// `internals.rs`): steel mass, kg, wetted area, m^2, and gas-side heat
/// transfer coefficient, W/(m^2 K). \[A\] all three: Unit 1's internals are
/// not in a held source. The mass is the dryer, separators and shroud head
/// scaled from public BWR/4 dryer masses (about 45 t) to a 460 MWe plant;
/// the area is that steel at 3 mm (dryer vanes) to 10 mm (separators),
/// both faces; h is low-velocity forced convection.
pub const INTERNALS_KG: f64 = 5.0e4;
pub const INTERNALS_AREA_M2: f64 = 2500.0;
pub const INTERNALS_H: f64 = 10.0;
/// The internals' natural-convection coefficient to the saturated dome when
/// no gas flows past them, W/(m^2 K). \[A\] an order of magnitude for
/// natural convection in steam (time constant about 2000 s with the mass
/// and area above).
pub const INTERNALS_LOSS_H: f64 = 5.0;

/// Unit 1's cesium inventory, kg: the BSAF Phase II benchmark specification's
/// value (OECD/NEA No. 7525, 2021, p.20-21: "154.0 kg" Cs and 11.98 kg I,
/// "specified by the benchmark co-ordinator"). \[P\]
pub const CS_INVENTORY_KG: f64 = 154.0;
/// Unit 1's iodine inventory, kg (the same source: "11.98 kg"). \[P\]
pub const I_INVENTORY_KG: f64 = 11.98;
/// Unit 1's tellurium inventory, kg. \[A\] derived, no published value
/// held: the ENDF/B-VIII summation's Te/Cs mass ratio at the scram
/// (`fp_decay`, 9.508/95.008 kg) times the specification's 154.0 kg Cs.
/// The summation's own Cs is low (95 kg) because MELCOR's default 584-day
/// history is shorter than Unit 1's fuel residence; the ratio is the
/// part used.
pub const TE_INVENTORY_KG: f64 = 154.0 * 9.508261 / 95.00829;
/// Deposition areas not otherwise in the model: the drywell floor, m^2,
/// and the torus pool's free surface, m^2. \[A\] both: not in a held source.
/// The floor is about a third of a 3410 m^3 light-bulb drywell's footprint;
/// the pool surface is a torus of about 15 m major radius filled about 40 %
/// (the pool's 1750 of 4370 m^3), so about 8 m wide.
pub const DW_FLOOR_M2: f64 = 200.0;
pub const WW_POOL_SURFACE_M2: f64 = 750.0;

/// CORSOR-M release of the volatile fission products (Xe, Cs, I), MELCOR's
/// default row for those classes (Vol. 2, SAND2023-10994 O, RN §2.3.1,
/// C7102): `k0`, per minute, and `Q`, kcal/mol. \[P\]
pub const CORSOR_K0_PER_MIN: f64 = 2.0e5;
pub const CORSOR_Q_KCAL_PER_MOL: f64 = 63.8;
/// CORSOR-M's row for La and UO2 (same source): `k0`, per minute, `Q`,
/// kcal/mol. \[P\]
pub const CORSOR_UO2_K0_PER_MIN: f64 = 1.46e7;
pub const CORSOR_UO2_Q_KCAL_PER_MOL: f64 = 143.1;

/// RPV leak to the drywell once the core-exit gas is hot (instrument lines,
/// SRV gaskets; the BSAF Unit 1 analyses needed such a path). \[A\] Opens above
/// [`HOT_LEAK_GAS_K`] with throat [`HOT_LEAK_M2`].
pub const HOT_LEAK_GAS_K: f64 = 1000.0;
pub const HOT_LEAK_M2: f64 = 3.0e-4;
/// Lower-head failure in the `without_lower_head_wall` ablation: debris in
/// the dry lower plenum above this, K (a debris temperature, not MELCOR's
/// wall-node TPFAIL; see `HEAD_FAILURE_WALL_K`), opens a breach of this
/// throat, m^2. \[A\] both. MELCOR's default opening is 0.1 m diameter,
/// 7.9e-3 m^2 (MELCOR 2.2 Vol. 2, COR §6.3).
pub const LOWER_HEAD_FAILURE_K: f64 = 1500.0;
pub const BREACH_M2: f64 = 0.01;

// ── Lower-head wall (gh:#1034) ───────────────────────────────────────────────
//
// The head heats from the debris, and it fails on the WALL, not on the debris
// temperature. This follows the public description of MELCOR's COR lower-head
// model (MELCOR 2.2 Vol. 2, SAND2023-10994 O, §6.1 and §6.3; Vol. 1,
// SAND2017-0455 O, COR_LHF and COR_VP). No MELCOR source is used.

/// Lower-head wall thickness, m. \[A\] MELCOR's generic BWR default DZLH
/// (Vol. 1, COR_VP table), which is for a 3.34 m vessel; Unit 1's vessel is
/// 2.4 m, and its own value needs a plant source (plan L4).
pub const LOWER_HEAD_THICKNESS_M: f64 = 0.22;
/// Debris-to-wall heat transfer coefficient, W/(m^2 K). \[A\] MELCOR's
/// default HDBLH (Vol. 1, COR_LHF), which its manual calls an
/// order-of-magnitude value for sensitivity studies.
pub const DEBRIS_TO_HEAD_W_M2_K: f64 = 1000.0;
/// Wall thickness of a lower-head penetration (instrument or control-rod
/// drive housing), m: its steel per unit heated area. \[A\] Unit 1's
/// housings are not in a held source. The debris heats it with
/// `debris_to_head_w_m2_k` (MELCOR's HDBPN default is the same 1000
/// W/(m^2 K)), and it fails at `head_failure_wall_k` (MELCOR's TPFAIL
/// applies to penetrations and the inner wall node alike).
pub const PENETRATION_WALL_M: f64 = 0.01;
/// Inner-wall temperature that fails the head, K. \[A\] MELCOR's default
/// TPFAIL (Vol. 1, COR_LHF), described as roughly where steel turns plastic.
pub const HEAD_FAILURE_WALL_K: f64 = 1273.15;
/// Larson-Miller creep rupture of the head, SA533B1 (NUREG/CR-5582, as MELCOR
/// Vol. 2, COR §6.3, Eqs 6-23 to 6-25 and C1601 give it): time to rupture
/// `10^(P/T - C)` s with `P = P0 - slope log10(sigma [Pa])`; plastic strain
/// grows by `fail_strain dt / t_R` and the head fails at `fail_strain`.
/// \[P\] V2's slope, 4725 (Vol. 1's control-function table prints 4730).
pub const LM_P0: f64 = 4.812e4;
pub const LM_SLOPE: f64 = 4.725e3;
pub const LM_C: f64 = 7.042;
pub const CREEP_FAIL_STRAIN: f64 = 0.18;
/// Porosity of the debris bed in the lower head, which sets how high it
/// stands and so the wall area it touches. \[A\]
pub const DEBRIS_POROSITY: f64 = 0.4;
/// Particulate debris diameter, m, for its oxidation area `6 V / D`
/// (MELCOR COR §3.2.1, Eq 3-28). \[A\] MELCOR has no default (COR_EDR
/// DHYPD is required input); 1 cm, the order of a pellet.
pub const DEBRIS_PARTICLE_M: f64 = 0.01;
/// Height of the lower plenum's bottom axial level, m: the penetrations'
/// height over which MELCOR scales the debris contact (`dz_d / dz_1`, COR
/// §6.1). \[A\] no Unit 1 nodalization held; half a metre, the order of a
/// lower-plenum cell.
pub const LP_BOTTOM_LEVEL_M: f64 = 0.5;
/// Diameter of the lower head's failure opening, m, through which a failed
/// vessel's water drains (`failed_vessel_drain`). \[A\] MELCOR has no
/// default (COR_PEN DFLPN, and the lower-plenum flow path, are user input);
/// 0.1 m, a few failed penetrations.
pub const BREACH_DIAMETER_M: f64 = 0.1;

/// Time over which debris in a wet lower plenum gives up its stored heat,
/// s. \[A\] (quench of a debris bed takes minutes).
pub const DEBRIS_QUENCH_S: f64 = 120.0;
/// Falling-debris quench heat-transfer coefficient, W/(m^2 K). \[P\]
/// MELCOR's default (Vol. 1, COR_LP HDBH2O, 100), used over the particle
/// area (`debris_quench`).
pub const QUENCH_H: f64 = 100.0;
/// Time constant of the quench decay factor, s. \[P\] MELCOR's radial
/// spreading time for solid debris (Vol. 2, COR §2.3.8 and §3.2.5: "an ad
/// hoc default value of 360 s", C1020(1)). The coefficient table lists
/// §3.2.5 against C1020(2) = 60 s; the text is followed (stated).
pub const QUENCH_TAU_S: f64 = 360.0;

/// Zircaloy relocation in the `without_rod_collapse` ablation: a node whose
/// cladding reaches this, K, slumps whole to the lower plenum. \[U\]
/// Zircaloy melts at 2098 K (MELCOR 2.2 Vol. 2, MP Table 7, citing MATPRO),
/// but MELCOR does not relocate anything at it (`CLAD_BREACH_K`).
pub const ZR_MELT_K: f64 = 2100.0;

// ── Rod collapse (gh:#1035) ──────────────────────────────────────────────────
//
// After the public description of MELCOR's COR model (MELCOR 2.2 Vol. 2,
// §3.1.3 and §3.2.1, Eq. 3-29; Vol. 1, C1131 and C1132 and COR_ROD). No
// MELCOR source is used.

/// Cladding temperature at which molten Zr breaches its oxide shell, K.
/// \[A\] MELCOR's default for cladding (C1131(2)).
pub const CLAD_BREACH_K: f64 = 2400.0;
/// Unoxidised metal below which a rod counts as oxidised, m. \[A\]
/// MELCOR's default (DRZRMN).
pub const MIN_METAL_M: f64 = 1.0e-4;
/// An oxidised rod collapses at this fuel temperature, K, and any rod at
/// the second. \[A\] MELCOR's defaults TRDFAI (2800 K in 1.8.4, lowered on
/// Phebus evidence for irradiated fuel) and TRDMAX (C1132(1..2)).
pub const ROD_COLLAPSE_K: f64 = 2500.0;
pub const ROD_COLLAPSE_MAX_K: f64 = 3100.0;
/// Time-at-temperature damage of an oxidised rod: lifetime
/// `1/L = A exp(B T)`, failure when `sum dt/L` reaches 1. \[P\] MELCOR's
/// 50th-percentile fit to VERCORS and the SOARCA uncertainty elicitation,
/// A in 1/s, B in 1/K (Vol. 2, Eq. 3-29).
pub const DAMAGE_A: f64 = 2.16e-11;
pub const DAMAGE_B: f64 = 7.0e-3;

// ── In-core radiation and the shroud (gh:#1037, plan item F6) ─────────────────
//
// After the public description of MELCOR's COR radiation model (MELCOR 2.2
// Vol. 2, §2.1; Vol. 1, COR_RF). No MELCOR source is used.

/// Fuel-assembly pitch, m: sets the core's equivalent radius, and so the ring
/// boundaries' areas. \[U\] GE BWR/3-4 lattice, 6 in.
pub const ASSEMBLY_PITCH_M: f64 = 0.1524;
/// Radial (cell-to-cell, and outer ring to shroud) and axial radiative
/// exchange factors. \[A\] MELCOR's 2.0 defaults FCELR and FCELA (Vol. 1,
/// COR_RF; 0.25 in its 1.86 scheme).
pub const RADIAL_EXCHANGE: f64 = 0.1;
pub const AXIAL_EXCHANGE: f64 = 0.1;
/// Core shroud wall thickness, m. \[A\] MELCOR's manual describes the BWR
/// shroud as "about 5 cm" (Vol. 2, COR §2.2.12); Unit 1's own value needs L4.
pub const SHROUD_THICKNESS_M: f64 = 0.05;
/// ZrO2 density, kg/m^3, for the oxide thickness that sets the cladding's
/// emissivity. \[P\] MELCOR's MP default (Vol. 2, MP Table 6.1).
pub const ZRO2_DENSITY: f64 = 5600.0;

// ── Two-phase mixture level (gh:#1036, plan item F5) ────────────────────────
//
// After the public description of MELCOR's CVH pool model (MELCOR 2.2 Vol. 2,
// CVH/FL §5.1.3, Eqs 5-26 to 5-30; C4407). No MELCOR source is used.

/// Bubble rise velocity in the pool, m/s, and the largest mean pool void.
/// \[A\] MELCOR's defaults C4407(1) and C4407(11); its manual says other
/// assumptions change the answer by about a factor of 2, and calls 0.4
/// roughly the upper limit of bubbly flow.
pub const BUBBLE_RISE_M_S: f64 = 0.3;
pub const MAX_POOL_VOID: f64 = 0.4;

// ── Decay heat (gh:#1041, plan item F10) ─────────────────────────────────────
//
// The ANS-5.1-1979 form as MELCOR's DCH package applies it (MELCOR 2.2 Vol. 2,
// DCH §3.2; Vol. 1, DCH_FPW, DCH_OPT, DCH_NFA, 3201, 3205). See `decay.rs`.

/// Shares of the rated power from U-235 thermal and Pu-239 thermal fission
/// (U-238 fast is the rest). \[A\] MELCOR's BWR defaults, 2.316e9,
/// 1.1114e9 and 1.506e8 W of 3578 MW, from a Sandia-ORIGEN equilibrium cycle
/// (Vol. 1, DCH_FPW); Unit 1's own cycle needs a plant source (plan L4).
pub const FISSION_SHARE_U235: f64 = 2.316 / 3.5780;
pub const FISSION_SHARE_PU239: f64 = 1.1114 / 3.5780;
/// Irradiation time at constant power, s. \[A\] MELCOR's default, 584 days
/// (2 years at 80 % capacity; Vol. 1, DCH_OPT).
pub const OPERATING_TIME_S: f64 = 5.05e7;
/// Fissions per initial fissile atom, for the capture correction. \[A\]
/// MELCOR's default from Sandia-ORIGEN (Vol. 1, DCH_NFA).
pub const FISSIONS_PER_FISSILE_ATOM: f64 = 0.713;
/// Energy per fission, MeV, fission plus decay. \[A\] MELCOR's defaults
/// from Sandia-ORIGEN (Vol. 1, 3201).
pub const Q_U235_MEV: f64 = 199.0;
pub const Q_PU239_MEV: f64 = 210.2;
pub const Q_U238_MEV: f64 = 199.3;
/// U-239 atoms made per fission at shutdown. \[A\] MELCOR's default, fitted
/// so the standard's equation matches ORIGEN (Vol. 1, 3205(1)).
pub const U239_PER_FISSION: f64 = 0.526;
/// Mean decay energy of U-239 and Np-239, MeV, and their decay constants,
/// 1/s. \[P\] the ANS-5.1-1979 standard's values (page 5), as MELCOR's
/// defaults give them (Vol. 1, 3205(2..5)).
pub const E_U239_MEV: f64 = 0.474;
pub const E_NP239_MEV: f64 = 0.419;
pub const U239_DECAY_PER_S: f64 = 4.91e-4;
pub const NP239_DECAY_PER_S: f64 = 3.41e-6;

// ── Zirconium-steam oxidation ───────────────────────────────────────────────
//
// Parabolic: d(W^2)/dt = K(T), W = mass of Zr oxidised per unit area,
// kg/m^2. Urbanic-Heidrick (1978), MELCOR's default. ~~[U] constants as quoted
// by the literature search from the MELCOR COR reference manual (public SAND
// report); check against it before quoting.~~ **CHECKED 2026-10-29** against
// the MELCOR 2.2 Reference Manual (SAND2023-10994 O, a Sandia report held
// privately, not public as was assumed): COR package §2.5.4, Table 2-6 gives
// exactly these six numbers, and its Eq. 2-174 is this interpolation, linear
// in T between the two branches. [P]. ~~Not adopted from it: its lower limit
// Tmin = 1100 K for steam (see OXIDATION_MIN_K)~~ (adopted 2026-10-29) ~~and its gaseous-diffusion
// limit on the rate (Eq. 2-168; this model limits only by steam supply)~~
// (the gaseous-diffusion limit is in since 2026-10-29: H2O_H2_DIFFUSIVITY_A,
// `core::diffusion_limited_rate`).
pub const UH_LOW_A: f64 = 29.6;
pub const UH_LOW_B_K: f64 = 16_820.0;
pub const UH_HIGH_A: f64 = 87.9;
pub const UH_HIGH_B_K: f64 = 16_610.0;
pub const UH_LOW_MAX_K: f64 = 1853.0;
pub const UH_HIGH_MIN_K: f64 = 1873.0;
/// No oxidation below this temperature, K. ~~Below this the oxidation is
/// negligible on the accident's time scale. \[A\] 1000 K~~ **CORRECTED
/// 2026-10-29**: MELCOR's TOXMIN for steam, 1100 K (Vol. 1, SAND2017-0455
/// O, C1004(1); Vol. 2, COR §2.5.4), which is also the lower end of the
/// Urbanic-Heidrick low branch's stated range (1100-1853 K, Table 2-6), so
/// the rate is no longer extrapolated below its fit. \[P\]
pub const OXIDATION_MIN_K: f64 = 1100.0;
/// Binary diffusion coefficient of steam in hydrogen, `D = a T^b / p`
/// (m^2/s, T in K, p in Pa): MELCOR 2.2 Vol. 2 (SAND2023-10994 O), NCG/H2O
/// package §6.1, Eq 6-2, which takes it from the MARCH 2 code's reference.
/// At 1000 K and 1 atm it gives 7.2e-4 m^2/s. \[P\]
pub const H2O_H2_DIFFUSIVITY_A: f64 = 6.60639e-4;
pub const H2O_H2_DIFFUSIVITY_B: f64 = 1.68;
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

/// Drywell structures (steel shell, concrete, the pedestal) as one lumped
/// heat sink: area, m^2, convective coefficient, W/(m^2 K), and heat
/// capacity, J/K. \[A\] all three placeholders, moved here from
/// `containment.rs` 2026-10-29 unchanged; the real heat-sink inventory is in
/// SAND2012-6173 (not held).
/// Since 2026-10-29 the coefficient is used only by the
/// `without_dw_wall_hs` ablation: the default computes it (MELCOR's
/// heat-structure surface).
pub const DW_WALL_AREA_M2: f64 = 4000.0;
pub const DW_WALL_H: f64 = 20.0;
pub const DW_WALL_C: f64 = 5.0e8;
/// Height of the drywell structures, m: the characteristic length of their
/// natural convection (`Mechanisms::dw_wall_hs`). \[A\] about the height
/// of a light-bulb drywell's cylinder; in the turbulent regime that applies
/// here it cancels.
pub const DW_WALL_HEIGHT_M: f64 = 20.0;
/// The torus shell above the pool (`Mechanisms::ww_shell`): area, m^2,
/// heat capacity, J/K, and height, m. \[A\] all three, derived from the
/// same assumed torus as `WW_POOL_SURFACE_M2` (15 m major radius, 4.15 m
/// minor, 40 % full by volume, so 55 % of its 2458 m^2 above the water), a
/// 15 mm carbon-steel shell (7850 kg/m^3, 500 J/(kg K)), and the gas
/// space's height; the height cancels in the turbulent regime.
pub const WW_SHELL_AREA_M2: f64 = 1353.0;
pub const WW_SHELL_C: f64 = 7.96e7;
pub const WW_SHELL_HEIGHT_M: f64 = 5.0;

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
/// Number of downcomers sharing that area. \[A\] the Mark I layout of 96
/// (not found for Unit 1 in the held sources; each is then 0.515 m across).
/// Sets each downcomer's flow and diameter in the swarm-rise scrubbing
/// (`sparc`).
pub const DOWNCOMER_COUNT: f64 = 96.0;
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
    rpv_wall_m = RPV_WALL_M, A,
    internals_envelope_m2 = INTERNALS_ENVELOPE_M2, A,
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
    msl_lines = MSL_LINES, A,
    msl_inner_radius_m = MSL_INNER_RADIUS_M, A,
    msl_wall_m = MSL_WALL_M, A,
    msl_insulation_h = MSL_INSULATION_H, A,
    msl_lm_a1 = MSL_LM_A1, P,
    msl_lm_b1 = MSL_LM_B1, P,
    msl_lm_c = MSL_LM_C, P,
    internals_kg = INTERNALS_KG, A,
    internals_area_m2 = INTERNALS_AREA_M2, A,
    internals_h = INTERNALS_H, A,
    internals_loss_h = INTERNALS_LOSS_H, A,
    lower_head_failure_k = LOWER_HEAD_FAILURE_K, A,
    breach_m2 = BREACH_M2, A,
    debris_quench_s = DEBRIS_QUENCH_S, A,
    quench_h = QUENCH_H, P,
    quench_tau_s = QUENCH_TAU_S, P,
    lower_head_thickness_m = LOWER_HEAD_THICKNESS_M, A,
    debris_to_head_w_m2_k = DEBRIS_TO_HEAD_W_M2_K, A,
    penetration_wall_m = PENETRATION_WALL_M, A,
    corsor_k0_per_min = CORSOR_K0_PER_MIN, P,
    corsor_q_kcal_per_mol = CORSOR_Q_KCAL_PER_MOL, P,
    corsor_uo2_k0_per_min = CORSOR_UO2_K0_PER_MIN, P,
    corsor_uo2_q_kcal_per_mol = CORSOR_UO2_Q_KCAL_PER_MOL, P,
    cs_inventory_kg = CS_INVENTORY_KG, P,
    i_inventory_kg = I_INVENTORY_KG, P,
    te_inventory_kg = TE_INVENTORY_KG, A,
    dw_floor_m2 = DW_FLOOR_M2, A,
    ww_pool_surface_m2 = WW_POOL_SURFACE_M2, A,
    head_failure_wall_k = HEAD_FAILURE_WALL_K, A,
    lm_p0 = LM_P0, P,
    lm_slope = LM_SLOPE, P,
    lm_c = LM_C, P,
    creep_fail_strain = CREEP_FAIL_STRAIN, P,
    debris_porosity = DEBRIS_POROSITY, A,
    debris_particle_m = DEBRIS_PARTICLE_M, A,
    lp_bottom_level_m = LP_BOTTOM_LEVEL_M, A,
    breach_diameter_m = BREACH_DIAMETER_M, A,
    zr_melt_k = ZR_MELT_K, U,
    clad_breach_k = CLAD_BREACH_K, A,
    min_metal_m = MIN_METAL_M, A,
    rod_collapse_k = ROD_COLLAPSE_K, A,
    rod_collapse_max_k = ROD_COLLAPSE_MAX_K, A,
    damage_a = DAMAGE_A, P,
    damage_b = DAMAGE_B, P,
    assembly_pitch_m = ASSEMBLY_PITCH_M, U,
    radial_exchange = RADIAL_EXCHANGE, A,
    axial_exchange = AXIAL_EXCHANGE, A,
    shroud_thickness_m = SHROUD_THICKNESS_M, A,
    zro2_density = ZRO2_DENSITY, P,
    bubble_rise_m_s = BUBBLE_RISE_M_S, A,
    max_pool_void = MAX_POOL_VOID, A,
    fission_share_u235 = FISSION_SHARE_U235, A,
    fission_share_pu239 = FISSION_SHARE_PU239, A,
    operating_time_s = OPERATING_TIME_S, A,
    fissions_per_fissile_atom = FISSIONS_PER_FISSILE_ATOM, A,
    q_u235_mev = Q_U235_MEV, A,
    q_pu239_mev = Q_PU239_MEV, A,
    q_u238_mev = Q_U238_MEV, A,
    u239_per_fission = U239_PER_FISSION, A,
    e_u239_mev = E_U239_MEV, P,
    e_np239_mev = E_NP239_MEV, P,
    u239_decay_per_s = U239_DECAY_PER_S, P,
    np239_decay_per_s = NP239_DECAY_PER_S, P,
    uh_low_a = UH_LOW_A, P,
    uh_low_b_k = UH_LOW_B_K, P,
    uh_high_a = UH_HIGH_A, P,
    uh_high_b_k = UH_HIGH_B_K, P,
    uh_low_max_k = UH_LOW_MAX_K, P,
    uh_high_min_k = UH_HIGH_MIN_K, P,
    oxidation_min_k = OXIDATION_MIN_K, P,
    h2o_h2_diffusivity_a = H2O_H2_DIFFUSIVITY_A, P,
    h2o_h2_diffusivity_b = H2O_H2_DIFFUSIVITY_B, P,
    oxidation_heat_j_per_kg_zr = OXIDATION_HEAT_J_PER_KG_ZR, P,
    drywell_volume_m3 = DRYWELL_VOLUME_M3, U,
    dw_wall_area_m2 = DW_WALL_AREA_M2, A,
    dw_wall_h = DW_WALL_H, A,
    dw_wall_c = DW_WALL_C, A,
    dw_wall_height_m = DW_WALL_HEIGHT_M, A,
    ww_shell_area_m2 = WW_SHELL_AREA_M2, A,
    ww_shell_c = WW_SHELL_C, A,
    ww_shell_height_m = WW_SHELL_HEIGHT_M, A,
    wetwell_gas_m3 = WETWELL_GAS_M3, P,
    pool_water_m3 = POOL_WATER_M3, P,
    containment_design_pa = CONTAINMENT_DESIGN_PA, P,
    initial_containment_pa = INITIAL_CONTAINMENT_PA, A,
    initial_drywell_k = INITIAL_DRYWELL_K, A,
    initial_pool_k = INITIAL_POOL_K, A,
    vent_submergence_pa = VENT_SUBMERGENCE_PA, A,
    downcomer_area_m2 = DOWNCOMER_AREA_M2, A,
    downcomer_count = DOWNCOMER_COUNT, A,
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
        assert_eq!(Params::INPUTS.len(), 144);
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
