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

// ── Molten core-concrete interaction (`Mechanisms::mcci`) ─────────────────
/// The concrete: MELCOR's standard CORCON basaltic aggregate concrete (CAV
/// UG Tables 2.1, 2.2): density, kg/m^3, ablation temperature, K, and the
/// mass fractions of water (evaporable 3.86 % plus chemically bound 2.00
/// %) and of CO2 (1.50 %). That Unit 1's basemat is of this type is \[A\]
/// (not in a held source; limestone/common sand, with far more CO2, is
/// the alternative); the values are MELCOR's defaults, \[P\].
pub const CONCRETE_DENSITY: f64 = 2340.0;
pub const CONCRETE_ABLATION_K: f64 = 1450.0;
pub const CONCRETE_H2O_FRACTION: f64 = 0.0586;
pub const CONCRETE_CO2_FRACTION: f64 = 0.0150;
/// Concrete's heat capacity, J/(kg K), and its ablation enthalpy, J/kg:
/// heating from 303 K to the ablation temperature (1.147 MJ/kg) plus
/// evaporating its free water (0.0386 x 2.257 MJ/kg = 0.087), releasing its
/// bound water (Ca(OH)2 -> CaO + H2O, 109 kJ/mol: 0.121), its CO2 (CaCO3 ->
/// CaO + CO2, 178 kJ/mol: 0.061) and melting (0.4 MJ/kg). \[A\] the heat
/// capacity and the heat of fusion, typical values; the reaction heats are
/// standard thermochemistry. Derived, not fitted: 1.82 MJ/kg.
pub const CONCRETE_CP: f64 = 1000.0;
pub const CONCRETE_ABLATION_J_PER_KG: f64 = 1.147e6 + 0.087e6 + 0.121e6 + 0.061e6 + 0.4e6;
/// The cavity: the debris's contact area with the floor, m^2 (the
/// pedestal, about 5.2 m across), and the heat-transfer coefficient from
/// the debris's interior to its bottom and to its top surface, W/(m^2 K).
/// \[A\] both. MELCOR's CORCON-Mod3 computes the coefficient from its gas-
/// film and bubbling correlations (CAV RM §3.3, not held); 1000 is their
/// order for an oxidic pool stirred by concrete gas.
pub const CAVITY_AREA_M2: f64 = 21.2;
pub const CAVITY_H: f64 = 1000.0;
/// Emissivity of the debris surface and of the drywell surroundings it
/// radiates to: MELCOR's CAV defaults EMISS.OX and EMISS.SUR (0.9; CAV UG,
/// the 2.0 defaults). \[P\]
pub const CAVITY_EMISSIVITY: f64 = 0.9;
/// Heat of Zr + 2 CO2 -> ZrO2 + 2 CO, J per kg of Zr: -(-1100.6 + 2 x
/// (-110.53) - 2 x (-393.51)) kJ/mol over 0.091224 kg/mol (NIST standard
/// enthalpies of formation at 298 K). \[P\]
pub const ZR_CO2_HEAT_J_PER_KG: f64 = (1100.6 + 2.0 * 110.53 - 2.0 * 393.51) * 1000.0 / 0.091224;
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

// ── Turbine-driven pumps: RCIC and HPCI (gh:#1109, `turbine_pump`) ────────
//
// Units 2 and 3 only: Unit 1's sequence never runs them (its HPCI was lost
// with DC at the tsunami). Marks as elsewhere; an [S] is a search excerpt
// of a named public document whose primary was not opened (treat as [U],
// the tag it carries here).

/// RCIC's rated (governed) flow, kg/s: 95 m^3/h at the suction's 998.2
/// kg/m^3 (IF97, 293.15 K). Unit 2's; Unit 3's is in `Params::unit3`. [S]
/// TEPCO, press material of 6 Aug 2014 (www.tepco.co.jp, betu14_e
/// 140806e0113), "the rated flow rate (95m3/h)" (page not seen; its 30 m^3/h
/// is TEPCO's fit to the measured pressure, a calibration, not used).
/// \[U\]
pub const RCIC_RATED_KG_S: f64 = 95.0 * 998.2 / 3600.0;
/// HPCI's rated flow, kg/s: 965 t/h. [S] KTH, EMUG 2012 slides (Unit 3) and
/// PSI, EMUG 2013 (Unit 2), "HPCI nominal flow rate: 965 t/h" (page not
/// seen; both MELCOR users' inputs, so a design value at second hand).
/// \[U\]
pub const HPCI_RATED_KG_S: f64 = 965_000.0 / 3600.0;
/// The pump's and the turbine's efficiencies (`turbine_pump`). \[A\] both:
/// not in a held source; a Terry turbine at part load is inefficient, so
/// the turbine's is set low. They fix the turbine's steam per kilogram
/// pumped (about 0.032 at 7 MPa against a 0.15 MPa exhaust).
/// \[A\]
pub const PUMP_EFFICIENCY: f64 = 0.7;
/// \[A\]
pub const TURBINE_EFFICIENCY: f64 = 0.5;
/// The vessel pressure below which a turbine-driven pump stops (its
/// low-steam-pressure isolation), Pa absolute. \[A\] not in a held source.
pub const TURBINE_MIN_PA: f64 = 0.5e6;
/// The low level L2, m above the top of active fuel: 120 in. USNRC HRTD ch.
/// 7.1, Table 7.1-2, p. 7.1-20 (12 Mar 12:35): "Unit 3 HPCI auto start at
/// 120 inches > TAF"; HPCI starts on L2 ([S] KTH EMUG 2012: "Start at low
/// level of water (L2) automatically"). Used as the bottom of the operators'
/// level band for both pumps \[P\]
pub const LEVEL_L2_ABOVE_TAF_M: f64 = 120.0 * 0.0254;
/// The high level L8, m above the top of active fuel, where an injecting
/// pump trips while DC lasts. \[A\] the number is not in a held source
/// (HRTD records the trips, Table 7.1-2 p. 7.1-18, not the set point):
/// 0.6 m above the model's normal level (4.4 m, Unit 1's, borrowed).
pub const LEVEL_L8_ABOVE_TAF_M: f64 = 5.0;
/// The steam line's nozzle, m above the top of active fuel, where a
/// rising swollen level starts to carry water into a turbine's steam
/// supply, and the band over which the steam fraction falls to zero, m
/// (`turbine_pump::steam_fraction`). \[A\] both: above L8, as L8 is set to
/// protect the steam lines; not in a held source, and not adjusted to any
/// measured level.
pub const MSL_NOZZLE_ABOVE_TAF_M: f64 = 6.0;
/// \[A\]
pub const CARRYOVER_BAND_M: f64 = 0.5;

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
/// The BWR core plate (`Mechanisms::core_plate`, MELCOR's PLATEB): its
/// thickness and the spacing of the beams that carry it, m. \[A\] both: a
/// few centimetres of stainless plate on beams about two fuel-cell
/// pitches apart (Unit 1's core plate is not in a held source).
pub const CORE_PLATE_M: f64 = 0.05;
pub const CORE_PLATE_BEAM_M: f64 = 0.3;
/// The lower plenum water's emissivity, for the radiation from the lowest
/// uncovered core cell down to it (`Mechanisms::bottom_radiation`): the
/// common infrared value (Incropera's tables give 0.95-0.96), \[A\].
pub const WATER_EMISSIVITY: f64 = 0.95;
/// That radiation's exchange factor over the cell's area: MELCOR's `FLPUP`
/// (COR UG, COR_RF default 0.25), \[P\].
pub const FLPUP: f64 = 0.25;
/// The control blades (`Mechanisms::control_blades`, MELCOR's NS
/// component): Unit 1's cruciform control rods, \[U\] 97 for its 400
/// assemblies (one per four).
pub const CONTROL_RODS: f64 = 97.0;
/// Stainless steel per blade over the active height, kg, \[A\] from the
/// common GE design: four wings 0.124 m by 3.66 m, a 1.4 mm sheath on both
/// faces (41 kg) and 18 B4C tubes per wing, 4.8 mm across with 0.6 mm walls
/// (17 kg), rounded up for the tie rod: 60 kg.
pub const BLADE_STEEL_KG: f64 = 60.0;
/// Each blade's oxidising surface, m^2: the sheath's outer faces, 4 x 2 x
/// 0.124 x 3.66. \[A\]
pub const BLADE_AREA_M2: f64 = 3.6;
/// MELCOR's stainless-steel oxidation in steam (COR RM Table 2-9):
/// parabolic, `K = A exp(-B/T)` (kg/m^2)^2/s above `SS_OX_MIN_K`. \[P\]
pub const SS_OX_A: f64 = 2.42e9;
/// The rate's activation temperature, K. \[P\]
pub const SS_OX_B: f64 = 42400.0;
/// Below this the steel does not oxidise, K. \[P\]
pub const SS_OX_MIN_K: f64 = 1100.0;
/// The control blades fail (melt and drain) at the steel's melting
/// temperature, MELCOR's default criterion and MP value, K. \[P\]
pub const SS_MELT_K: f64 = 1700.0;
/// The steel's heat of fusion, J/kg: MELCOR's default for stainless steel
/// (MP RM §8, "Estimated"; 2.692e5 for 304). \[P\]
pub const SS_FUSION_J_PER_KG: f64 = 2.68e5;
/// The cavity debris's phases (`Mechanisms::cavity_phases`): its thermal
/// conductivity when solid, W/(m K), \[A\] UO2's at high temperature
/// (about 2-3).
pub const DEBRIS_K: f64 = 2.5;
/// The fuel component's melting point, K: MELCOR's UO2 default (MP RM §7,
/// from MATPRO). \[P\]
pub const FUEL_MELT_K: f64 = 3113.0;
/// The fuel component's heat of fusion, J/kg: MELCOR's UO2 default (MP RM
/// §8). \[P\]
pub const FUEL_FUSION_J_PER_KG: f64 = 2.74e5;
/// The concrete's oxides as the low-melting component, taken as SiO2: its
/// heat of fusion, J/mol, \[A\] (JANAF's about 9.6 kJ/mol), its melting
/// point being the concrete's ablation temperature.
pub const CONCRETE_OXIDE_FUSION_J_PER_MOL: f64 = 9.6e3;
/// The concrete oxides' molar mass, kg/mol, as SiO2. \[A\]
pub const CONCRETE_OXIDE_M: f64 = 0.060_08;
/// MELCOR's BUR ignition limits for a volume without igniters (BUR RM
/// §2.2, record BUR_IGN defaults): hydrogen mole fraction, \[P\].
pub const BUR_XH2IGN: f64 = 0.10;
/// CO's ignition mole fraction (XCOIGN default). \[P\]
pub const BUR_XCOIGN: f64 = 0.167;
/// The least oxygen mole fraction for ignition (XO2IG default). \[P\]
pub const BUR_XO2IG: f64 = 0.05;
/// The most steam and CO2 for ignition (XMSCIG default), inerting above.
/// \[P\]
pub const BUR_XMSCIG: f64 = 0.55;
/// MELCOR's BUR combustion completeness (BUR RM §2.3, record BUR_COM
/// defaults): the hydrogen mole fraction in the completeness's Le
/// Chatelier formula (Eq 2-7), XH2CC. \[P\]
pub const BUR_XH2CC: f64 = 0.08;
/// CO's mole fraction in the completeness's Le Chatelier formula, XCOCC.
/// \[P\]
pub const BUR_XCOCC: f64 = 0.148;
/// The HECTR 1.5 completeness correlation's slope (Eq 2-9,
/// sensitivity coefficients 2202). \[P\]
pub const BUR_CC_SLOPE: f64 = 23.4116;
/// The correlation's threshold Ymax, below which nothing burns (Eq 2-8).
/// \[P\]
pub const BUR_CC_Y0: f64 = 0.037_46;
/// Hydrogen's heat of combustion to steam (the lower heating value),
/// J/mol. \[P\] (NIST: 241.8 kJ/mol.)
pub const H2_COMBUSTION_J_PER_MOL: f64 = 241.8e3;
/// CO's heat of combustion to CO2, J/mol. \[P\] (NIST: 283.0 kJ/mol.)
pub const CO_COMBUSTION_J_PER_MOL: f64 = 283.0e3;
/// The ejected debris's surface tension, N/m, for its breakup in the
/// pedestal's water (`Mechanisms::fdi_lpme`, FDI RM Eq 2-8). \[A\] liquid
/// UO2's, about 0.45 N/m at its melting point (MATPRO).
pub const FDI_SURFACE_TENSION: f64 = 0.45;
/// The ejection velocity above which MELCOR models a high-pressure melt
/// ejection instead, m/s (FDI RM §2.3, sensitivity coefficient 4602).
/// \[P\]
pub const FDI_HPME_VELOCITY: f64 = 10.0;
/// UO2 carried by candling Zr, as a fraction of the molten Zr's mass
/// (`Mechanisms::uo2_candling`): MELCOR's COR_CMT default FUOZR with
/// MTUOZR = 1 (COR RM §3.1.4, Eq 3-14). \[P\]
pub const CANDLING_UO2_FRACTION: f64 = 0.2;
/// The lower plenum's debris is a convecting oxide pool above this, K
/// (`Mechanisms::molten_pool`): the UO2-ZrO2 eutectic of MELCOR's Table
/// 2-12 (COR RM §2.8.2). \[P\]
pub const POOL_MELT_K: f64 = 2800.0;
/// MELCOR's ACOPO correlation for an oxide pool's radial boundary, `Nu =
/// A Ra_i^n` (COR RM Table 2-4, j = 1, C1280): the coefficient. \[P\]
pub const POOL_NU_A: f64 = 0.3;
/// The ACOPO correlation's exponent. \[P\]
pub const POOL_NU_N: f64 = 0.22;
/// The oxidic melt's conductivity, W/(m K). \[A\] corium literature,
/// about 3.
pub const POOL_K: f64 = 3.0;
/// The oxidic melt's viscosity, Pa s. \[A\] corium literature, 4.4e-3.
pub const POOL_MU: f64 = 4.4e-3;
/// The oxidic melt's thermal expansion, 1/K. \[A\] corium literature,
/// 1.05e-4.
pub const POOL_BETA: f64 = 1.05e-4;
/// The oxidic melt's specific heat, J/(kg K). \[A\] corium literature,
/// about 500.
pub const POOL_CP: f64 = 500.0;
/// The oxidic melt's density, kg/m^3. \[A\] corium literature, about
/// 8000.
pub const POOL_RHO: f64 = 8000.0;
/// B4C per control blade over the active height, kg (`Mechanisms::
/// b4c_oxidation`): \[A\] the blade geometry of `BLADE_STEEL_KG`, 72 tubes of
/// 3.6 mm bore over 3.66 m, packed at 70 % of B4C's 2520 kg/m^3: 4.7 kg.
pub const BLADE_B4C_KG: f64 = 4.7;
/// MELCOR's simple B4C model (COR RM §2.5.9, MARCON 2.1B, Eq 2-208): the
/// fraction of the initial B4C reacting per second is `A exp(-B/T)`,
/// `A = 9.973e6 / 60` 1/s. \[P\]
pub const B4C_RATE_A: f64 = 9.973e6 / 60.0;
/// Its activation temperature, K. \[P\]
pub const B4C_RATE_B: f64 = 22647.2;
/// The most of the B4C that may react, MELCOR's default (C1005(2)): the
/// blade's eutectic relocates it first. \[P\]
pub const B4C_MAX_FRACTION: f64 = 0.02;
/// The steel's iron, chromium and nickel mass fractions, MELCOR's MP_SS
/// default (Vol. 1, MP UG): 0.74, 0.18, 0.08, no carbon. \[P\]
pub const SS_X_FE: f64 = 0.74;
/// Chromium's mass fraction (MP_SS default). \[P\]
pub const SS_X_CR: f64 = 0.18;
/// Nickel's mass fraction (MP_SS default). \[P\]
pub const SS_X_NI: f64 = 0.08;
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
/// The molar gas constant, J/(mol K): since gh:#1335 (2026-11-11) a
/// re-export of the generic constant in `outram_park_fork_offbeat::melcor::params`,
/// the same `f64` as before (8.314 462).
pub use outram_park_fork_offbeat::melcor::params::R_GAS;

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
/// The drywell structures' outer heat path (`Mechanisms::dw_wall_outer`):
/// the gap between the steel shell and the concrete around it, m, and the
/// emissivity of its two faces; the concrete's thickness, m, and
/// conductivity, W/(m K); and the coefficient from its outer face to the
/// reactor building's air, W/(m^2 K). \[A\] all five: a Mark I shell
/// stands a few centimetres off its concrete; oxidised steel and concrete
/// near 0.8; 1.5 m of biological shield; ordinary concrete's
/// conductivity; natural convection to still air.
pub const DW_GAP_M: f64 = 0.05;
pub const DW_GAP_EMISSIVITY: f64 = 0.8;
pub const DW_CONCRETE_M: f64 = 1.5;
pub const CONCRETE_K: f64 = 1.4;
pub const RB_WALL_H: f64 = 5.0;
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
/// the plant's inputs and one worker still equals three. The fields, their
/// defaults, their tags ([`Params::INPUTS`]) and the order they cross the
/// worker wire in are written out below in one order; a test checks each tag
/// against the constant's doc comment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub thermal_power_w: f64,
    pub assemblies: f64,
    pub rods_per_assembly: f64,
    pub active_length_m: f64,
    pub clad_outer_r_m: f64,
    pub clad_wall_m: f64,
    pub pellet_r_m: f64,
    pub uo2_density: f64,
    pub zr_density: f64,
    pub channel_inner_width_m: f64,
    pub channel_wall_m: f64,
    pub channel_length_m: f64,
    pub uo2_cp: f64,
    pub zr_cp: f64,
    pub uo2_k: f64,
    pub gap_h: f64,
    pub boiling_h: f64,
    pub cosine_extrapolation: f64,
    pub rpv_radius_m: f64,
    pub rpv_height_m: f64,
    pub rpv_wall_m: f64,
    pub internals_envelope_m2: f64,
    pub concrete_density: f64,
    pub concrete_ablation_k: f64,
    pub concrete_h2o_fraction: f64,
    pub concrete_co2_fraction: f64,
    pub concrete_cp: f64,
    pub concrete_ablation_j_per_kg: f64,
    pub cavity_area_m2: f64,
    pub cavity_h: f64,
    pub cavity_emissivity: f64,
    pub zr_co2_heat_j_per_kg: f64,
    pub baf_m: f64,
    pub lower_plenum_free: f64,
    pub upper_free: f64,
    pub core_other_area_m2: f64,
    pub normal_level_above_taf_m: f64,
    pub initial_pressure_pa: f64,
    pub msiv_closure_s: f64,
    pub bypass_gain_kg_s_pa: f64,
    pub relief_valves: f64,
    pub relief_open_pa: f64,
    pub relief_close_pa: f64,
    pub safety_open_pa: f64,
    pub safety_close_pa: f64,
    pub relief_throat_m2: f64,
    pub ic_autostart_pa: f64,
    pub ic_ua_w_per_k: f64,
    pub ic_shell_water_kg: f64,
    pub fire_engine_shutoff_pa: f64,
    pub fire_engine_max_kg_s: f64,
    pub injection_temperature_k: f64,
    pub hot_leak_gas_k: f64,
    pub hot_leak_m2: f64,
    pub msl_lines: f64,
    pub msl_inner_radius_m: f64,
    pub msl_wall_m: f64,
    pub msl_insulation_h: f64,
    pub msl_lm_a1: f64,
    pub msl_lm_b1: f64,
    pub msl_lm_c: f64,
    pub internals_kg: f64,
    pub internals_area_m2: f64,
    pub internals_h: f64,
    pub internals_loss_h: f64,
    pub lower_head_failure_k: f64,
    pub breach_m2: f64,
    pub debris_quench_s: f64,
    pub quench_h: f64,
    pub quench_tau_s: f64,
    pub lower_head_thickness_m: f64,
    pub debris_to_head_w_m2_k: f64,
    pub core_plate_m: f64,
    pub core_plate_beam_m: f64,
    pub water_emissivity: f64,
    pub flpup: f64,
    pub control_rods: f64,
    pub blade_steel_kg: f64,
    pub blade_area_m2: f64,
    pub ss_ox_a: f64,
    pub ss_ox_b: f64,
    pub ss_ox_min_k: f64,
    pub ss_melt_k: f64,
    pub ss_fusion_j_per_kg: f64,
    pub debris_k: f64,
    pub fuel_melt_k: f64,
    pub fuel_fusion_j_per_kg: f64,
    pub concrete_oxide_fusion_j_per_mol: f64,
    pub concrete_oxide_m: f64,
    pub bur_xh2ign: f64,
    pub bur_xcoign: f64,
    pub bur_xo2ig: f64,
    pub bur_xmscig: f64,
    pub bur_xh2cc: f64,
    pub bur_xcocc: f64,
    pub bur_cc_slope: f64,
    pub bur_cc_y0: f64,
    pub h2_combustion_j_per_mol: f64,
    pub co_combustion_j_per_mol: f64,
    pub fdi_surface_tension: f64,
    pub fdi_hpme_velocity: f64,
    pub candling_uo2_fraction: f64,
    pub pool_melt_k: f64,
    pub pool_nu_a: f64,
    pub pool_nu_n: f64,
    pub pool_k: f64,
    pub pool_mu: f64,
    pub pool_beta: f64,
    pub pool_cp: f64,
    pub pool_rho: f64,
    pub blade_b4c_kg: f64,
    pub b4c_rate_a: f64,
    pub b4c_rate_b: f64,
    pub b4c_max_fraction: f64,
    pub ss_x_fe: f64,
    pub ss_x_cr: f64,
    pub ss_x_ni: f64,
    pub penetration_wall_m: f64,
    pub corsor_k0_per_min: f64,
    pub corsor_q_kcal_per_mol: f64,
    pub corsor_uo2_k0_per_min: f64,
    pub corsor_uo2_q_kcal_per_mol: f64,
    pub cs_inventory_kg: f64,
    pub i_inventory_kg: f64,
    pub te_inventory_kg: f64,
    pub dw_floor_m2: f64,
    pub ww_pool_surface_m2: f64,
    pub head_failure_wall_k: f64,
    pub lm_p0: f64,
    pub lm_slope: f64,
    pub lm_c: f64,
    pub creep_fail_strain: f64,
    pub debris_porosity: f64,
    pub debris_particle_m: f64,
    pub lp_bottom_level_m: f64,
    pub breach_diameter_m: f64,
    pub zr_melt_k: f64,
    pub clad_breach_k: f64,
    pub min_metal_m: f64,
    pub rod_collapse_k: f64,
    pub rod_collapse_max_k: f64,
    pub damage_a: f64,
    pub damage_b: f64,
    pub assembly_pitch_m: f64,
    pub radial_exchange: f64,
    pub axial_exchange: f64,
    pub shroud_thickness_m: f64,
    pub zro2_density: f64,
    pub bubble_rise_m_s: f64,
    pub max_pool_void: f64,
    pub fission_share_u235: f64,
    pub fission_share_pu239: f64,
    pub operating_time_s: f64,
    pub fissions_per_fissile_atom: f64,
    pub q_u235_mev: f64,
    pub q_pu239_mev: f64,
    pub q_u238_mev: f64,
    pub u239_per_fission: f64,
    pub e_u239_mev: f64,
    pub e_np239_mev: f64,
    pub u239_decay_per_s: f64,
    pub np239_decay_per_s: f64,
    pub uh_low_a: f64,
    pub uh_low_b_k: f64,
    pub uh_high_a: f64,
    pub uh_high_b_k: f64,
    pub uh_low_max_k: f64,
    pub uh_high_min_k: f64,
    pub oxidation_min_k: f64,
    pub h2o_h2_diffusivity_a: f64,
    pub h2o_h2_diffusivity_b: f64,
    pub oxidation_heat_j_per_kg_zr: f64,
    pub drywell_volume_m3: f64,
    pub dw_wall_area_m2: f64,
    pub dw_wall_h: f64,
    pub dw_wall_c: f64,
    pub dw_wall_height_m: f64,
    pub dw_gap_m: f64,
    pub dw_gap_emissivity: f64,
    pub dw_concrete_m: f64,
    pub concrete_k: f64,
    pub rb_wall_h: f64,
    pub ww_shell_area_m2: f64,
    pub ww_shell_c: f64,
    pub ww_shell_height_m: f64,
    pub wetwell_gas_m3: f64,
    pub pool_water_m3: f64,
    pub containment_design_pa: f64,
    pub initial_containment_pa: f64,
    pub initial_drywell_k: f64,
    pub initial_pool_k: f64,
    pub vent_submergence_pa: f64,
    pub downcomer_area_m2: f64,
    pub downcomer_count: f64,
    pub discharge_coefficient: f64,
    pub vacuum_breaker_pa: f64,
    pub vacuum_breaker_m2: f64,
    pub vent_rupture_disk_pa: f64,
    pub vent_area_m2: f64,
    pub flange_leak_pa: f64,
    pub flange_leak_m2: f64,
    pub reactor_building_m3: f64,
    pub h2_flammable: f64,
    pub h2_explosion: f64,
    pub rcic_rated_kg_s: f64,
    pub hpci_rated_kg_s: f64,
    pub pump_efficiency: f64,
    pub turbine_efficiency: f64,
    pub turbine_min_pa: f64,
    pub level_l2_above_taf_m: f64,
    pub level_l8_above_taf_m: f64,
    pub msl_nozzle_above_taf_m: f64,
    pub carryover_band_m: f64,
    /// [`RING_FACTORS`].
    pub ring_factors: [f64; 3],
}

impl Default for Params {
    fn default() -> Self {
        Self {
            thermal_power_w: THERMAL_POWER_W,
            assemblies: ASSEMBLIES,
            rods_per_assembly: RODS_PER_ASSEMBLY,
            active_length_m: ACTIVE_LENGTH_M,
            clad_outer_r_m: CLAD_OUTER_R_M,
            clad_wall_m: CLAD_WALL_M,
            pellet_r_m: PELLET_R_M,
            uo2_density: UO2_DENSITY,
            zr_density: ZR_DENSITY,
            channel_inner_width_m: CHANNEL_INNER_WIDTH_M,
            channel_wall_m: CHANNEL_WALL_M,
            channel_length_m: CHANNEL_LENGTH_M,
            uo2_cp: UO2_CP,
            zr_cp: ZR_CP,
            uo2_k: UO2_K,
            gap_h: GAP_H,
            boiling_h: BOILING_H,
            cosine_extrapolation: COSINE_EXTRAPOLATION,
            rpv_radius_m: RPV_RADIUS_M,
            rpv_height_m: RPV_HEIGHT_M,
            rpv_wall_m: RPV_WALL_M,
            internals_envelope_m2: INTERNALS_ENVELOPE_M2,
            concrete_density: CONCRETE_DENSITY,
            concrete_ablation_k: CONCRETE_ABLATION_K,
            concrete_h2o_fraction: CONCRETE_H2O_FRACTION,
            concrete_co2_fraction: CONCRETE_CO2_FRACTION,
            concrete_cp: CONCRETE_CP,
            concrete_ablation_j_per_kg: CONCRETE_ABLATION_J_PER_KG,
            cavity_area_m2: CAVITY_AREA_M2,
            cavity_h: CAVITY_H,
            cavity_emissivity: CAVITY_EMISSIVITY,
            zr_co2_heat_j_per_kg: ZR_CO2_HEAT_J_PER_KG,
            baf_m: BAF_M,
            lower_plenum_free: LOWER_PLENUM_FREE,
            upper_free: UPPER_FREE,
            core_other_area_m2: CORE_OTHER_AREA_M2,
            normal_level_above_taf_m: NORMAL_LEVEL_ABOVE_TAF_M,
            initial_pressure_pa: INITIAL_PRESSURE_PA,
            msiv_closure_s: MSIV_CLOSURE_S,
            bypass_gain_kg_s_pa: BYPASS_GAIN_KG_S_PA,
            relief_valves: RELIEF_VALVES,
            relief_open_pa: RELIEF_OPEN_PA,
            relief_close_pa: RELIEF_CLOSE_PA,
            safety_open_pa: SAFETY_OPEN_PA,
            safety_close_pa: SAFETY_CLOSE_PA,
            relief_throat_m2: RELIEF_THROAT_M2,
            ic_autostart_pa: IC_AUTOSTART_PA,
            ic_ua_w_per_k: IC_UA_W_PER_K,
            ic_shell_water_kg: IC_SHELL_WATER_KG,
            fire_engine_shutoff_pa: FIRE_ENGINE_SHUTOFF_PA,
            fire_engine_max_kg_s: FIRE_ENGINE_MAX_KG_S,
            injection_temperature_k: INJECTION_TEMPERATURE_K,
            hot_leak_gas_k: HOT_LEAK_GAS_K,
            hot_leak_m2: HOT_LEAK_M2,
            msl_lines: MSL_LINES,
            msl_inner_radius_m: MSL_INNER_RADIUS_M,
            msl_wall_m: MSL_WALL_M,
            msl_insulation_h: MSL_INSULATION_H,
            msl_lm_a1: MSL_LM_A1,
            msl_lm_b1: MSL_LM_B1,
            msl_lm_c: MSL_LM_C,
            internals_kg: INTERNALS_KG,
            internals_area_m2: INTERNALS_AREA_M2,
            internals_h: INTERNALS_H,
            internals_loss_h: INTERNALS_LOSS_H,
            lower_head_failure_k: LOWER_HEAD_FAILURE_K,
            breach_m2: BREACH_M2,
            debris_quench_s: DEBRIS_QUENCH_S,
            quench_h: QUENCH_H,
            quench_tau_s: QUENCH_TAU_S,
            lower_head_thickness_m: LOWER_HEAD_THICKNESS_M,
            debris_to_head_w_m2_k: DEBRIS_TO_HEAD_W_M2_K,
            core_plate_m: CORE_PLATE_M,
            core_plate_beam_m: CORE_PLATE_BEAM_M,
            water_emissivity: WATER_EMISSIVITY,
            flpup: FLPUP,
            control_rods: CONTROL_RODS,
            blade_steel_kg: BLADE_STEEL_KG,
            blade_area_m2: BLADE_AREA_M2,
            ss_ox_a: SS_OX_A,
            ss_ox_b: SS_OX_B,
            ss_ox_min_k: SS_OX_MIN_K,
            ss_melt_k: SS_MELT_K,
            ss_fusion_j_per_kg: SS_FUSION_J_PER_KG,
            debris_k: DEBRIS_K,
            fuel_melt_k: FUEL_MELT_K,
            fuel_fusion_j_per_kg: FUEL_FUSION_J_PER_KG,
            concrete_oxide_fusion_j_per_mol: CONCRETE_OXIDE_FUSION_J_PER_MOL,
            concrete_oxide_m: CONCRETE_OXIDE_M,
            bur_xh2ign: BUR_XH2IGN,
            bur_xcoign: BUR_XCOIGN,
            bur_xo2ig: BUR_XO2IG,
            bur_xmscig: BUR_XMSCIG,
            bur_xh2cc: BUR_XH2CC,
            bur_xcocc: BUR_XCOCC,
            bur_cc_slope: BUR_CC_SLOPE,
            bur_cc_y0: BUR_CC_Y0,
            h2_combustion_j_per_mol: H2_COMBUSTION_J_PER_MOL,
            co_combustion_j_per_mol: CO_COMBUSTION_J_PER_MOL,
            fdi_surface_tension: FDI_SURFACE_TENSION,
            fdi_hpme_velocity: FDI_HPME_VELOCITY,
            candling_uo2_fraction: CANDLING_UO2_FRACTION,
            pool_melt_k: POOL_MELT_K,
            pool_nu_a: POOL_NU_A,
            pool_nu_n: POOL_NU_N,
            pool_k: POOL_K,
            pool_mu: POOL_MU,
            pool_beta: POOL_BETA,
            pool_cp: POOL_CP,
            pool_rho: POOL_RHO,
            blade_b4c_kg: BLADE_B4C_KG,
            b4c_rate_a: B4C_RATE_A,
            b4c_rate_b: B4C_RATE_B,
            b4c_max_fraction: B4C_MAX_FRACTION,
            ss_x_fe: SS_X_FE,
            ss_x_cr: SS_X_CR,
            ss_x_ni: SS_X_NI,
            penetration_wall_m: PENETRATION_WALL_M,
            corsor_k0_per_min: CORSOR_K0_PER_MIN,
            corsor_q_kcal_per_mol: CORSOR_Q_KCAL_PER_MOL,
            corsor_uo2_k0_per_min: CORSOR_UO2_K0_PER_MIN,
            corsor_uo2_q_kcal_per_mol: CORSOR_UO2_Q_KCAL_PER_MOL,
            cs_inventory_kg: CS_INVENTORY_KG,
            i_inventory_kg: I_INVENTORY_KG,
            te_inventory_kg: TE_INVENTORY_KG,
            dw_floor_m2: DW_FLOOR_M2,
            ww_pool_surface_m2: WW_POOL_SURFACE_M2,
            head_failure_wall_k: HEAD_FAILURE_WALL_K,
            lm_p0: LM_P0,
            lm_slope: LM_SLOPE,
            lm_c: LM_C,
            creep_fail_strain: CREEP_FAIL_STRAIN,
            debris_porosity: DEBRIS_POROSITY,
            debris_particle_m: DEBRIS_PARTICLE_M,
            lp_bottom_level_m: LP_BOTTOM_LEVEL_M,
            breach_diameter_m: BREACH_DIAMETER_M,
            zr_melt_k: ZR_MELT_K,
            clad_breach_k: CLAD_BREACH_K,
            min_metal_m: MIN_METAL_M,
            rod_collapse_k: ROD_COLLAPSE_K,
            rod_collapse_max_k: ROD_COLLAPSE_MAX_K,
            damage_a: DAMAGE_A,
            damage_b: DAMAGE_B,
            assembly_pitch_m: ASSEMBLY_PITCH_M,
            radial_exchange: RADIAL_EXCHANGE,
            axial_exchange: AXIAL_EXCHANGE,
            shroud_thickness_m: SHROUD_THICKNESS_M,
            zro2_density: ZRO2_DENSITY,
            bubble_rise_m_s: BUBBLE_RISE_M_S,
            max_pool_void: MAX_POOL_VOID,
            fission_share_u235: FISSION_SHARE_U235,
            fission_share_pu239: FISSION_SHARE_PU239,
            operating_time_s: OPERATING_TIME_S,
            fissions_per_fissile_atom: FISSIONS_PER_FISSILE_ATOM,
            q_u235_mev: Q_U235_MEV,
            q_pu239_mev: Q_PU239_MEV,
            q_u238_mev: Q_U238_MEV,
            u239_per_fission: U239_PER_FISSION,
            e_u239_mev: E_U239_MEV,
            e_np239_mev: E_NP239_MEV,
            u239_decay_per_s: U239_DECAY_PER_S,
            np239_decay_per_s: NP239_DECAY_PER_S,
            uh_low_a: UH_LOW_A,
            uh_low_b_k: UH_LOW_B_K,
            uh_high_a: UH_HIGH_A,
            uh_high_b_k: UH_HIGH_B_K,
            uh_low_max_k: UH_LOW_MAX_K,
            uh_high_min_k: UH_HIGH_MIN_K,
            oxidation_min_k: OXIDATION_MIN_K,
            h2o_h2_diffusivity_a: H2O_H2_DIFFUSIVITY_A,
            h2o_h2_diffusivity_b: H2O_H2_DIFFUSIVITY_B,
            oxidation_heat_j_per_kg_zr: OXIDATION_HEAT_J_PER_KG_ZR,
            drywell_volume_m3: DRYWELL_VOLUME_M3,
            dw_wall_area_m2: DW_WALL_AREA_M2,
            dw_wall_h: DW_WALL_H,
            dw_wall_c: DW_WALL_C,
            dw_wall_height_m: DW_WALL_HEIGHT_M,
            dw_gap_m: DW_GAP_M,
            dw_gap_emissivity: DW_GAP_EMISSIVITY,
            dw_concrete_m: DW_CONCRETE_M,
            concrete_k: CONCRETE_K,
            rb_wall_h: RB_WALL_H,
            ww_shell_area_m2: WW_SHELL_AREA_M2,
            ww_shell_c: WW_SHELL_C,
            ww_shell_height_m: WW_SHELL_HEIGHT_M,
            wetwell_gas_m3: WETWELL_GAS_M3,
            pool_water_m3: POOL_WATER_M3,
            containment_design_pa: CONTAINMENT_DESIGN_PA,
            initial_containment_pa: INITIAL_CONTAINMENT_PA,
            initial_drywell_k: INITIAL_DRYWELL_K,
            initial_pool_k: INITIAL_POOL_K,
            vent_submergence_pa: VENT_SUBMERGENCE_PA,
            downcomer_area_m2: DOWNCOMER_AREA_M2,
            downcomer_count: DOWNCOMER_COUNT,
            discharge_coefficient: DISCHARGE_COEFFICIENT,
            vacuum_breaker_pa: VACUUM_BREAKER_PA,
            vacuum_breaker_m2: VACUUM_BREAKER_M2,
            vent_rupture_disk_pa: VENT_RUPTURE_DISK_PA,
            vent_area_m2: VENT_AREA_M2,
            flange_leak_pa: FLANGE_LEAK_PA,
            flange_leak_m2: FLANGE_LEAK_M2,
            reactor_building_m3: REACTOR_BUILDING_M3,
            h2_flammable: H2_FLAMMABLE,
            h2_explosion: H2_EXPLOSION,
            rcic_rated_kg_s: RCIC_RATED_KG_S,
            hpci_rated_kg_s: HPCI_RATED_KG_S,
            pump_efficiency: PUMP_EFFICIENCY,
            turbine_efficiency: TURBINE_EFFICIENCY,
            turbine_min_pa: TURBINE_MIN_PA,
            level_l2_above_taf_m: LEVEL_L2_ABOVE_TAF_M,
            level_l8_above_taf_m: LEVEL_L8_ABOVE_TAF_M,
            msl_nozzle_above_taf_m: MSL_NOZZLE_ABOVE_TAF_M,
            carryover_band_m: CARRYOVER_BAND_M,
            ring_factors: RING_FACTORS,
        }
    }
}

impl Params {
    pub fn write(&self, w: &mut Writer) {
        w.f(&[
            self.thermal_power_w,
            self.assemblies,
            self.rods_per_assembly,
            self.active_length_m,
            self.clad_outer_r_m,
            self.clad_wall_m,
            self.pellet_r_m,
            self.uo2_density,
            self.zr_density,
            self.channel_inner_width_m,
            self.channel_wall_m,
            self.channel_length_m,
            self.uo2_cp,
            self.zr_cp,
            self.uo2_k,
            self.gap_h,
            self.boiling_h,
            self.cosine_extrapolation,
            self.rpv_radius_m,
            self.rpv_height_m,
            self.rpv_wall_m,
            self.internals_envelope_m2,
            self.concrete_density,
            self.concrete_ablation_k,
            self.concrete_h2o_fraction,
            self.concrete_co2_fraction,
            self.concrete_cp,
            self.concrete_ablation_j_per_kg,
            self.cavity_area_m2,
            self.cavity_h,
            self.cavity_emissivity,
            self.zr_co2_heat_j_per_kg,
            self.baf_m,
            self.lower_plenum_free,
            self.upper_free,
            self.core_other_area_m2,
            self.normal_level_above_taf_m,
            self.initial_pressure_pa,
            self.msiv_closure_s,
            self.bypass_gain_kg_s_pa,
            self.relief_valves,
            self.relief_open_pa,
            self.relief_close_pa,
            self.safety_open_pa,
            self.safety_close_pa,
            self.relief_throat_m2,
            self.ic_autostart_pa,
            self.ic_ua_w_per_k,
            self.ic_shell_water_kg,
            self.fire_engine_shutoff_pa,
            self.fire_engine_max_kg_s,
            self.injection_temperature_k,
            self.hot_leak_gas_k,
            self.hot_leak_m2,
            self.msl_lines,
            self.msl_inner_radius_m,
            self.msl_wall_m,
            self.msl_insulation_h,
            self.msl_lm_a1,
            self.msl_lm_b1,
            self.msl_lm_c,
            self.internals_kg,
            self.internals_area_m2,
            self.internals_h,
            self.internals_loss_h,
            self.lower_head_failure_k,
            self.breach_m2,
            self.debris_quench_s,
            self.quench_h,
            self.quench_tau_s,
            self.lower_head_thickness_m,
            self.debris_to_head_w_m2_k,
            self.core_plate_m,
            self.core_plate_beam_m,
            self.water_emissivity,
            self.flpup,
            self.control_rods,
            self.blade_steel_kg,
            self.blade_area_m2,
            self.ss_ox_a,
            self.ss_ox_b,
            self.ss_ox_min_k,
            self.ss_melt_k,
            self.ss_fusion_j_per_kg,
            self.debris_k,
            self.fuel_melt_k,
            self.fuel_fusion_j_per_kg,
            self.concrete_oxide_fusion_j_per_mol,
            self.concrete_oxide_m,
            self.bur_xh2ign,
            self.bur_xcoign,
            self.bur_xo2ig,
            self.bur_xmscig,
            self.bur_xh2cc,
            self.bur_xcocc,
            self.bur_cc_slope,
            self.bur_cc_y0,
            self.h2_combustion_j_per_mol,
            self.co_combustion_j_per_mol,
            self.fdi_surface_tension,
            self.fdi_hpme_velocity,
            self.candling_uo2_fraction,
            self.pool_melt_k,
            self.pool_nu_a,
            self.pool_nu_n,
            self.pool_k,
            self.pool_mu,
            self.pool_beta,
            self.pool_cp,
            self.pool_rho,
            self.blade_b4c_kg,
            self.b4c_rate_a,
            self.b4c_rate_b,
            self.b4c_max_fraction,
            self.ss_x_fe,
            self.ss_x_cr,
            self.ss_x_ni,
            self.penetration_wall_m,
            self.corsor_k0_per_min,
            self.corsor_q_kcal_per_mol,
            self.corsor_uo2_k0_per_min,
            self.corsor_uo2_q_kcal_per_mol,
            self.cs_inventory_kg,
            self.i_inventory_kg,
            self.te_inventory_kg,
            self.dw_floor_m2,
            self.ww_pool_surface_m2,
            self.head_failure_wall_k,
            self.lm_p0,
            self.lm_slope,
            self.lm_c,
            self.creep_fail_strain,
            self.debris_porosity,
            self.debris_particle_m,
            self.lp_bottom_level_m,
            self.breach_diameter_m,
            self.zr_melt_k,
            self.clad_breach_k,
            self.min_metal_m,
            self.rod_collapse_k,
            self.rod_collapse_max_k,
            self.damage_a,
            self.damage_b,
            self.assembly_pitch_m,
            self.radial_exchange,
            self.axial_exchange,
            self.shroud_thickness_m,
            self.zro2_density,
            self.bubble_rise_m_s,
            self.max_pool_void,
            self.fission_share_u235,
            self.fission_share_pu239,
            self.operating_time_s,
            self.fissions_per_fissile_atom,
            self.q_u235_mev,
            self.q_pu239_mev,
            self.q_u238_mev,
            self.u239_per_fission,
            self.e_u239_mev,
            self.e_np239_mev,
            self.u239_decay_per_s,
            self.np239_decay_per_s,
            self.uh_low_a,
            self.uh_low_b_k,
            self.uh_high_a,
            self.uh_high_b_k,
            self.uh_low_max_k,
            self.uh_high_min_k,
            self.oxidation_min_k,
            self.h2o_h2_diffusivity_a,
            self.h2o_h2_diffusivity_b,
            self.oxidation_heat_j_per_kg_zr,
            self.drywell_volume_m3,
            self.dw_wall_area_m2,
            self.dw_wall_h,
            self.dw_wall_c,
            self.dw_wall_height_m,
            self.dw_gap_m,
            self.dw_gap_emissivity,
            self.dw_concrete_m,
            self.concrete_k,
            self.rb_wall_h,
            self.ww_shell_area_m2,
            self.ww_shell_c,
            self.ww_shell_height_m,
            self.wetwell_gas_m3,
            self.pool_water_m3,
            self.containment_design_pa,
            self.initial_containment_pa,
            self.initial_drywell_k,
            self.initial_pool_k,
            self.vent_submergence_pa,
            self.downcomer_area_m2,
            self.downcomer_count,
            self.discharge_coefficient,
            self.vacuum_breaker_pa,
            self.vacuum_breaker_m2,
            self.vent_rupture_disk_pa,
            self.vent_area_m2,
            self.flange_leak_pa,
            self.flange_leak_m2,
            self.reactor_building_m3,
            self.h2_flammable,
            self.h2_explosion,
            self.rcic_rated_kg_s,
            self.hpci_rated_kg_s,
            self.pump_efficiency,
            self.turbine_efficiency,
            self.turbine_min_pa,
            self.level_l2_above_taf_m,
            self.level_l8_above_taf_m,
            self.msl_nozzle_above_taf_m,
            self.carryover_band_m,
        ]);
        w.f(&self.ring_factors);
    }

    pub fn read(r: &mut Reader) -> Self {
        Self {
            thermal_power_w: r.f(),
            assemblies: r.f(),
            rods_per_assembly: r.f(),
            active_length_m: r.f(),
            clad_outer_r_m: r.f(),
            clad_wall_m: r.f(),
            pellet_r_m: r.f(),
            uo2_density: r.f(),
            zr_density: r.f(),
            channel_inner_width_m: r.f(),
            channel_wall_m: r.f(),
            channel_length_m: r.f(),
            uo2_cp: r.f(),
            zr_cp: r.f(),
            uo2_k: r.f(),
            gap_h: r.f(),
            boiling_h: r.f(),
            cosine_extrapolation: r.f(),
            rpv_radius_m: r.f(),
            rpv_height_m: r.f(),
            rpv_wall_m: r.f(),
            internals_envelope_m2: r.f(),
            concrete_density: r.f(),
            concrete_ablation_k: r.f(),
            concrete_h2o_fraction: r.f(),
            concrete_co2_fraction: r.f(),
            concrete_cp: r.f(),
            concrete_ablation_j_per_kg: r.f(),
            cavity_area_m2: r.f(),
            cavity_h: r.f(),
            cavity_emissivity: r.f(),
            zr_co2_heat_j_per_kg: r.f(),
            baf_m: r.f(),
            lower_plenum_free: r.f(),
            upper_free: r.f(),
            core_other_area_m2: r.f(),
            normal_level_above_taf_m: r.f(),
            initial_pressure_pa: r.f(),
            msiv_closure_s: r.f(),
            bypass_gain_kg_s_pa: r.f(),
            relief_valves: r.f(),
            relief_open_pa: r.f(),
            relief_close_pa: r.f(),
            safety_open_pa: r.f(),
            safety_close_pa: r.f(),
            relief_throat_m2: r.f(),
            ic_autostart_pa: r.f(),
            ic_ua_w_per_k: r.f(),
            ic_shell_water_kg: r.f(),
            fire_engine_shutoff_pa: r.f(),
            fire_engine_max_kg_s: r.f(),
            injection_temperature_k: r.f(),
            hot_leak_gas_k: r.f(),
            hot_leak_m2: r.f(),
            msl_lines: r.f(),
            msl_inner_radius_m: r.f(),
            msl_wall_m: r.f(),
            msl_insulation_h: r.f(),
            msl_lm_a1: r.f(),
            msl_lm_b1: r.f(),
            msl_lm_c: r.f(),
            internals_kg: r.f(),
            internals_area_m2: r.f(),
            internals_h: r.f(),
            internals_loss_h: r.f(),
            lower_head_failure_k: r.f(),
            breach_m2: r.f(),
            debris_quench_s: r.f(),
            quench_h: r.f(),
            quench_tau_s: r.f(),
            lower_head_thickness_m: r.f(),
            debris_to_head_w_m2_k: r.f(),
            core_plate_m: r.f(),
            core_plate_beam_m: r.f(),
            water_emissivity: r.f(),
            flpup: r.f(),
            control_rods: r.f(),
            blade_steel_kg: r.f(),
            blade_area_m2: r.f(),
            ss_ox_a: r.f(),
            ss_ox_b: r.f(),
            ss_ox_min_k: r.f(),
            ss_melt_k: r.f(),
            ss_fusion_j_per_kg: r.f(),
            debris_k: r.f(),
            fuel_melt_k: r.f(),
            fuel_fusion_j_per_kg: r.f(),
            concrete_oxide_fusion_j_per_mol: r.f(),
            concrete_oxide_m: r.f(),
            bur_xh2ign: r.f(),
            bur_xcoign: r.f(),
            bur_xo2ig: r.f(),
            bur_xmscig: r.f(),
            bur_xh2cc: r.f(),
            bur_xcocc: r.f(),
            bur_cc_slope: r.f(),
            bur_cc_y0: r.f(),
            h2_combustion_j_per_mol: r.f(),
            co_combustion_j_per_mol: r.f(),
            fdi_surface_tension: r.f(),
            fdi_hpme_velocity: r.f(),
            candling_uo2_fraction: r.f(),
            pool_melt_k: r.f(),
            pool_nu_a: r.f(),
            pool_nu_n: r.f(),
            pool_k: r.f(),
            pool_mu: r.f(),
            pool_beta: r.f(),
            pool_cp: r.f(),
            pool_rho: r.f(),
            blade_b4c_kg: r.f(),
            b4c_rate_a: r.f(),
            b4c_rate_b: r.f(),
            b4c_max_fraction: r.f(),
            ss_x_fe: r.f(),
            ss_x_cr: r.f(),
            ss_x_ni: r.f(),
            penetration_wall_m: r.f(),
            corsor_k0_per_min: r.f(),
            corsor_q_kcal_per_mol: r.f(),
            corsor_uo2_k0_per_min: r.f(),
            corsor_uo2_q_kcal_per_mol: r.f(),
            cs_inventory_kg: r.f(),
            i_inventory_kg: r.f(),
            te_inventory_kg: r.f(),
            dw_floor_m2: r.f(),
            ww_pool_surface_m2: r.f(),
            head_failure_wall_k: r.f(),
            lm_p0: r.f(),
            lm_slope: r.f(),
            lm_c: r.f(),
            creep_fail_strain: r.f(),
            debris_porosity: r.f(),
            debris_particle_m: r.f(),
            lp_bottom_level_m: r.f(),
            breach_diameter_m: r.f(),
            zr_melt_k: r.f(),
            clad_breach_k: r.f(),
            min_metal_m: r.f(),
            rod_collapse_k: r.f(),
            rod_collapse_max_k: r.f(),
            damage_a: r.f(),
            damage_b: r.f(),
            assembly_pitch_m: r.f(),
            radial_exchange: r.f(),
            axial_exchange: r.f(),
            shroud_thickness_m: r.f(),
            zro2_density: r.f(),
            bubble_rise_m_s: r.f(),
            max_pool_void: r.f(),
            fission_share_u235: r.f(),
            fission_share_pu239: r.f(),
            operating_time_s: r.f(),
            fissions_per_fissile_atom: r.f(),
            q_u235_mev: r.f(),
            q_pu239_mev: r.f(),
            q_u238_mev: r.f(),
            u239_per_fission: r.f(),
            e_u239_mev: r.f(),
            e_np239_mev: r.f(),
            u239_decay_per_s: r.f(),
            np239_decay_per_s: r.f(),
            uh_low_a: r.f(),
            uh_low_b_k: r.f(),
            uh_high_a: r.f(),
            uh_high_b_k: r.f(),
            uh_low_max_k: r.f(),
            uh_high_min_k: r.f(),
            oxidation_min_k: r.f(),
            h2o_h2_diffusivity_a: r.f(),
            h2o_h2_diffusivity_b: r.f(),
            oxidation_heat_j_per_kg_zr: r.f(),
            drywell_volume_m3: r.f(),
            dw_wall_area_m2: r.f(),
            dw_wall_h: r.f(),
            dw_wall_c: r.f(),
            dw_wall_height_m: r.f(),
            dw_gap_m: r.f(),
            dw_gap_emissivity: r.f(),
            dw_concrete_m: r.f(),
            concrete_k: r.f(),
            rb_wall_h: r.f(),
            ww_shell_area_m2: r.f(),
            ww_shell_c: r.f(),
            ww_shell_height_m: r.f(),
            wetwell_gas_m3: r.f(),
            pool_water_m3: r.f(),
            containment_design_pa: r.f(),
            initial_containment_pa: r.f(),
            initial_drywell_k: r.f(),
            initial_pool_k: r.f(),
            vent_submergence_pa: r.f(),
            downcomer_area_m2: r.f(),
            downcomer_count: r.f(),
            discharge_coefficient: r.f(),
            vacuum_breaker_pa: r.f(),
            vacuum_breaker_m2: r.f(),
            vent_rupture_disk_pa: r.f(),
            vent_area_m2: r.f(),
            flange_leak_pa: r.f(),
            flange_leak_m2: r.f(),
            reactor_building_m3: r.f(),
            h2_flammable: r.f(),
            h2_explosion: r.f(),
            rcic_rated_kg_s: r.f(),
            hpci_rated_kg_s: r.f(),
            pump_efficiency: r.f(),
            turbine_efficiency: r.f(),
            turbine_min_pa: r.f(),
            level_l2_above_taf_m: r.f(),
            level_l8_above_taf_m: r.f(),
            msl_nozzle_above_taf_m: r.f(),
            carryover_band_m: r.f(),
            ring_factors: [r.f(), r.f(), r.f()],
        }
    }

    /// Every scalar input by field name, with its tag (the
    /// default's, from its doc comment), in declaration order.
    pub const INPUTS: &[(&str, Tag)] = &[
        ("thermal_power_w", Tag::P),
        ("assemblies", Tag::P),
        ("rods_per_assembly", Tag::U),
        ("active_length_m", Tag::U),
        ("clad_outer_r_m", Tag::U),
        ("clad_wall_m", Tag::U),
        ("pellet_r_m", Tag::U),
        ("uo2_density", Tag::A),
        ("zr_density", Tag::P),
        ("channel_inner_width_m", Tag::U),
        ("channel_wall_m", Tag::U),
        ("channel_length_m", Tag::U),
        ("uo2_cp", Tag::A),
        ("zr_cp", Tag::A),
        ("uo2_k", Tag::A),
        ("gap_h", Tag::A),
        ("boiling_h", Tag::A),
        ("cosine_extrapolation", Tag::A),
        ("rpv_radius_m", Tag::U),
        ("rpv_height_m", Tag::U),
        ("rpv_wall_m", Tag::A),
        ("internals_envelope_m2", Tag::A),
        ("concrete_density", Tag::P),
        ("concrete_ablation_k", Tag::P),
        ("concrete_h2o_fraction", Tag::P),
        ("concrete_co2_fraction", Tag::P),
        ("concrete_cp", Tag::A),
        ("concrete_ablation_j_per_kg", Tag::A),
        ("cavity_area_m2", Tag::A),
        ("cavity_h", Tag::A),
        ("cavity_emissivity", Tag::P),
        ("zr_co2_heat_j_per_kg", Tag::P),
        ("baf_m", Tag::A),
        ("lower_plenum_free", Tag::A),
        ("upper_free", Tag::A),
        ("core_other_area_m2", Tag::A),
        ("normal_level_above_taf_m", Tag::U),
        ("initial_pressure_pa", Tag::P),
        ("msiv_closure_s", Tag::U),
        ("bypass_gain_kg_s_pa", Tag::A),
        ("relief_valves", Tag::U),
        ("relief_open_pa", Tag::U),
        ("relief_close_pa", Tag::U),
        ("safety_open_pa", Tag::U),
        ("safety_close_pa", Tag::U),
        ("relief_throat_m2", Tag::A),
        ("ic_autostart_pa", Tag::P),
        ("ic_ua_w_per_k", Tag::A),
        ("ic_shell_water_kg", Tag::A),
        ("fire_engine_shutoff_pa", Tag::A),
        ("fire_engine_max_kg_s", Tag::A),
        ("injection_temperature_k", Tag::A),
        ("hot_leak_gas_k", Tag::A),
        ("hot_leak_m2", Tag::A),
        ("msl_lines", Tag::A),
        ("msl_inner_radius_m", Tag::A),
        ("msl_wall_m", Tag::A),
        ("msl_insulation_h", Tag::A),
        ("msl_lm_a1", Tag::P),
        ("msl_lm_b1", Tag::P),
        ("msl_lm_c", Tag::P),
        ("internals_kg", Tag::A),
        ("internals_area_m2", Tag::A),
        ("internals_h", Tag::A),
        ("internals_loss_h", Tag::A),
        ("lower_head_failure_k", Tag::A),
        ("breach_m2", Tag::A),
        ("debris_quench_s", Tag::A),
        ("quench_h", Tag::P),
        ("quench_tau_s", Tag::P),
        ("lower_head_thickness_m", Tag::A),
        ("debris_to_head_w_m2_k", Tag::A),
        ("core_plate_m", Tag::A),
        ("core_plate_beam_m", Tag::A),
        ("water_emissivity", Tag::A),
        ("flpup", Tag::P),
        ("control_rods", Tag::U),
        ("blade_steel_kg", Tag::A),
        ("blade_area_m2", Tag::A),
        ("ss_ox_a", Tag::P),
        ("ss_ox_b", Tag::P),
        ("ss_ox_min_k", Tag::P),
        ("ss_melt_k", Tag::P),
        ("ss_fusion_j_per_kg", Tag::P),
        ("debris_k", Tag::A),
        ("fuel_melt_k", Tag::P),
        ("fuel_fusion_j_per_kg", Tag::P),
        ("concrete_oxide_fusion_j_per_mol", Tag::A),
        ("concrete_oxide_m", Tag::A),
        ("bur_xh2ign", Tag::P),
        ("bur_xcoign", Tag::P),
        ("bur_xo2ig", Tag::P),
        ("bur_xmscig", Tag::P),
        ("bur_xh2cc", Tag::P),
        ("bur_xcocc", Tag::P),
        ("bur_cc_slope", Tag::P),
        ("bur_cc_y0", Tag::P),
        ("h2_combustion_j_per_mol", Tag::P),
        ("co_combustion_j_per_mol", Tag::P),
        ("fdi_surface_tension", Tag::A),
        ("fdi_hpme_velocity", Tag::P),
        ("candling_uo2_fraction", Tag::P),
        ("pool_melt_k", Tag::P),
        ("pool_nu_a", Tag::P),
        ("pool_nu_n", Tag::P),
        ("pool_k", Tag::A),
        ("pool_mu", Tag::A),
        ("pool_beta", Tag::A),
        ("pool_cp", Tag::A),
        ("pool_rho", Tag::A),
        ("blade_b4c_kg", Tag::A),
        ("b4c_rate_a", Tag::P),
        ("b4c_rate_b", Tag::P),
        ("b4c_max_fraction", Tag::P),
        ("ss_x_fe", Tag::P),
        ("ss_x_cr", Tag::P),
        ("ss_x_ni", Tag::P),
        ("penetration_wall_m", Tag::A),
        ("corsor_k0_per_min", Tag::P),
        ("corsor_q_kcal_per_mol", Tag::P),
        ("corsor_uo2_k0_per_min", Tag::P),
        ("corsor_uo2_q_kcal_per_mol", Tag::P),
        ("cs_inventory_kg", Tag::P),
        ("i_inventory_kg", Tag::P),
        ("te_inventory_kg", Tag::A),
        ("dw_floor_m2", Tag::A),
        ("ww_pool_surface_m2", Tag::A),
        ("head_failure_wall_k", Tag::A),
        ("lm_p0", Tag::P),
        ("lm_slope", Tag::P),
        ("lm_c", Tag::P),
        ("creep_fail_strain", Tag::P),
        ("debris_porosity", Tag::A),
        ("debris_particle_m", Tag::A),
        ("lp_bottom_level_m", Tag::A),
        ("breach_diameter_m", Tag::A),
        ("zr_melt_k", Tag::U),
        ("clad_breach_k", Tag::A),
        ("min_metal_m", Tag::A),
        ("rod_collapse_k", Tag::A),
        ("rod_collapse_max_k", Tag::A),
        ("damage_a", Tag::P),
        ("damage_b", Tag::P),
        ("assembly_pitch_m", Tag::U),
        ("radial_exchange", Tag::A),
        ("axial_exchange", Tag::A),
        ("shroud_thickness_m", Tag::A),
        ("zro2_density", Tag::P),
        ("bubble_rise_m_s", Tag::A),
        ("max_pool_void", Tag::A),
        ("fission_share_u235", Tag::A),
        ("fission_share_pu239", Tag::A),
        ("operating_time_s", Tag::A),
        ("fissions_per_fissile_atom", Tag::A),
        ("q_u235_mev", Tag::A),
        ("q_pu239_mev", Tag::A),
        ("q_u238_mev", Tag::A),
        ("u239_per_fission", Tag::A),
        ("e_u239_mev", Tag::P),
        ("e_np239_mev", Tag::P),
        ("u239_decay_per_s", Tag::P),
        ("np239_decay_per_s", Tag::P),
        ("uh_low_a", Tag::P),
        ("uh_low_b_k", Tag::P),
        ("uh_high_a", Tag::P),
        ("uh_high_b_k", Tag::P),
        ("uh_low_max_k", Tag::P),
        ("uh_high_min_k", Tag::P),
        ("oxidation_min_k", Tag::P),
        ("h2o_h2_diffusivity_a", Tag::P),
        ("h2o_h2_diffusivity_b", Tag::P),
        ("oxidation_heat_j_per_kg_zr", Tag::P),
        ("drywell_volume_m3", Tag::U),
        ("dw_wall_area_m2", Tag::A),
        ("dw_wall_h", Tag::A),
        ("dw_wall_c", Tag::A),
        ("dw_wall_height_m", Tag::A),
        ("dw_gap_m", Tag::A),
        ("dw_gap_emissivity", Tag::A),
        ("dw_concrete_m", Tag::A),
        ("concrete_k", Tag::A),
        ("rb_wall_h", Tag::A),
        ("ww_shell_area_m2", Tag::A),
        ("ww_shell_c", Tag::A),
        ("ww_shell_height_m", Tag::A),
        ("wetwell_gas_m3", Tag::P),
        ("pool_water_m3", Tag::P),
        ("containment_design_pa", Tag::P),
        ("initial_containment_pa", Tag::A),
        ("initial_drywell_k", Tag::A),
        ("initial_pool_k", Tag::A),
        ("vent_submergence_pa", Tag::A),
        ("downcomer_area_m2", Tag::A),
        ("downcomer_count", Tag::A),
        ("discharge_coefficient", Tag::A),
        ("vacuum_breaker_pa", Tag::A),
        ("vacuum_breaker_m2", Tag::A),
        ("vent_rupture_disk_pa", Tag::A),
        ("vent_area_m2", Tag::A),
        ("flange_leak_pa", Tag::A),
        ("flange_leak_m2", Tag::A),
        ("reactor_building_m3", Tag::A),
        ("h2_flammable", Tag::P),
        ("h2_explosion", Tag::A),
        ("rcic_rated_kg_s", Tag::U),
        ("hpci_rated_kg_s", Tag::U),
        ("pump_efficiency", Tag::A),
        ("turbine_efficiency", Tag::A),
        ("turbine_min_pa", Tag::A),
        ("level_l2_above_taf_m", Tag::P),
        ("level_l8_above_taf_m", Tag::A),
        ("msl_nozzle_above_taf_m", Tag::A),
        ("carryover_band_m", Tag::A),
    ];

    /// The input named `name` (a field name from [`Params::INPUTS`]).
    pub fn get(&self, name: &str) -> Option<f64> {
        match name {
            "thermal_power_w" => Some(self.thermal_power_w),
            "assemblies" => Some(self.assemblies),
            "rods_per_assembly" => Some(self.rods_per_assembly),
            "active_length_m" => Some(self.active_length_m),
            "clad_outer_r_m" => Some(self.clad_outer_r_m),
            "clad_wall_m" => Some(self.clad_wall_m),
            "pellet_r_m" => Some(self.pellet_r_m),
            "uo2_density" => Some(self.uo2_density),
            "zr_density" => Some(self.zr_density),
            "channel_inner_width_m" => Some(self.channel_inner_width_m),
            "channel_wall_m" => Some(self.channel_wall_m),
            "channel_length_m" => Some(self.channel_length_m),
            "uo2_cp" => Some(self.uo2_cp),
            "zr_cp" => Some(self.zr_cp),
            "uo2_k" => Some(self.uo2_k),
            "gap_h" => Some(self.gap_h),
            "boiling_h" => Some(self.boiling_h),
            "cosine_extrapolation" => Some(self.cosine_extrapolation),
            "rpv_radius_m" => Some(self.rpv_radius_m),
            "rpv_height_m" => Some(self.rpv_height_m),
            "rpv_wall_m" => Some(self.rpv_wall_m),
            "internals_envelope_m2" => Some(self.internals_envelope_m2),
            "concrete_density" => Some(self.concrete_density),
            "concrete_ablation_k" => Some(self.concrete_ablation_k),
            "concrete_h2o_fraction" => Some(self.concrete_h2o_fraction),
            "concrete_co2_fraction" => Some(self.concrete_co2_fraction),
            "concrete_cp" => Some(self.concrete_cp),
            "concrete_ablation_j_per_kg" => Some(self.concrete_ablation_j_per_kg),
            "cavity_area_m2" => Some(self.cavity_area_m2),
            "cavity_h" => Some(self.cavity_h),
            "cavity_emissivity" => Some(self.cavity_emissivity),
            "zr_co2_heat_j_per_kg" => Some(self.zr_co2_heat_j_per_kg),
            "baf_m" => Some(self.baf_m),
            "lower_plenum_free" => Some(self.lower_plenum_free),
            "upper_free" => Some(self.upper_free),
            "core_other_area_m2" => Some(self.core_other_area_m2),
            "normal_level_above_taf_m" => Some(self.normal_level_above_taf_m),
            "initial_pressure_pa" => Some(self.initial_pressure_pa),
            "msiv_closure_s" => Some(self.msiv_closure_s),
            "bypass_gain_kg_s_pa" => Some(self.bypass_gain_kg_s_pa),
            "relief_valves" => Some(self.relief_valves),
            "relief_open_pa" => Some(self.relief_open_pa),
            "relief_close_pa" => Some(self.relief_close_pa),
            "safety_open_pa" => Some(self.safety_open_pa),
            "safety_close_pa" => Some(self.safety_close_pa),
            "relief_throat_m2" => Some(self.relief_throat_m2),
            "ic_autostart_pa" => Some(self.ic_autostart_pa),
            "ic_ua_w_per_k" => Some(self.ic_ua_w_per_k),
            "ic_shell_water_kg" => Some(self.ic_shell_water_kg),
            "fire_engine_shutoff_pa" => Some(self.fire_engine_shutoff_pa),
            "fire_engine_max_kg_s" => Some(self.fire_engine_max_kg_s),
            "injection_temperature_k" => Some(self.injection_temperature_k),
            "hot_leak_gas_k" => Some(self.hot_leak_gas_k),
            "hot_leak_m2" => Some(self.hot_leak_m2),
            "msl_lines" => Some(self.msl_lines),
            "msl_inner_radius_m" => Some(self.msl_inner_radius_m),
            "msl_wall_m" => Some(self.msl_wall_m),
            "msl_insulation_h" => Some(self.msl_insulation_h),
            "msl_lm_a1" => Some(self.msl_lm_a1),
            "msl_lm_b1" => Some(self.msl_lm_b1),
            "msl_lm_c" => Some(self.msl_lm_c),
            "internals_kg" => Some(self.internals_kg),
            "internals_area_m2" => Some(self.internals_area_m2),
            "internals_h" => Some(self.internals_h),
            "internals_loss_h" => Some(self.internals_loss_h),
            "lower_head_failure_k" => Some(self.lower_head_failure_k),
            "breach_m2" => Some(self.breach_m2),
            "debris_quench_s" => Some(self.debris_quench_s),
            "quench_h" => Some(self.quench_h),
            "quench_tau_s" => Some(self.quench_tau_s),
            "lower_head_thickness_m" => Some(self.lower_head_thickness_m),
            "debris_to_head_w_m2_k" => Some(self.debris_to_head_w_m2_k),
            "core_plate_m" => Some(self.core_plate_m),
            "core_plate_beam_m" => Some(self.core_plate_beam_m),
            "water_emissivity" => Some(self.water_emissivity),
            "flpup" => Some(self.flpup),
            "control_rods" => Some(self.control_rods),
            "blade_steel_kg" => Some(self.blade_steel_kg),
            "blade_area_m2" => Some(self.blade_area_m2),
            "ss_ox_a" => Some(self.ss_ox_a),
            "ss_ox_b" => Some(self.ss_ox_b),
            "ss_ox_min_k" => Some(self.ss_ox_min_k),
            "ss_melt_k" => Some(self.ss_melt_k),
            "ss_fusion_j_per_kg" => Some(self.ss_fusion_j_per_kg),
            "debris_k" => Some(self.debris_k),
            "fuel_melt_k" => Some(self.fuel_melt_k),
            "fuel_fusion_j_per_kg" => Some(self.fuel_fusion_j_per_kg),
            "concrete_oxide_fusion_j_per_mol" => Some(self.concrete_oxide_fusion_j_per_mol),
            "concrete_oxide_m" => Some(self.concrete_oxide_m),
            "bur_xh2ign" => Some(self.bur_xh2ign),
            "bur_xcoign" => Some(self.bur_xcoign),
            "bur_xo2ig" => Some(self.bur_xo2ig),
            "bur_xmscig" => Some(self.bur_xmscig),
            "bur_xh2cc" => Some(self.bur_xh2cc),
            "bur_xcocc" => Some(self.bur_xcocc),
            "bur_cc_slope" => Some(self.bur_cc_slope),
            "bur_cc_y0" => Some(self.bur_cc_y0),
            "h2_combustion_j_per_mol" => Some(self.h2_combustion_j_per_mol),
            "co_combustion_j_per_mol" => Some(self.co_combustion_j_per_mol),
            "fdi_surface_tension" => Some(self.fdi_surface_tension),
            "fdi_hpme_velocity" => Some(self.fdi_hpme_velocity),
            "candling_uo2_fraction" => Some(self.candling_uo2_fraction),
            "pool_melt_k" => Some(self.pool_melt_k),
            "pool_nu_a" => Some(self.pool_nu_a),
            "pool_nu_n" => Some(self.pool_nu_n),
            "pool_k" => Some(self.pool_k),
            "pool_mu" => Some(self.pool_mu),
            "pool_beta" => Some(self.pool_beta),
            "pool_cp" => Some(self.pool_cp),
            "pool_rho" => Some(self.pool_rho),
            "blade_b4c_kg" => Some(self.blade_b4c_kg),
            "b4c_rate_a" => Some(self.b4c_rate_a),
            "b4c_rate_b" => Some(self.b4c_rate_b),
            "b4c_max_fraction" => Some(self.b4c_max_fraction),
            "ss_x_fe" => Some(self.ss_x_fe),
            "ss_x_cr" => Some(self.ss_x_cr),
            "ss_x_ni" => Some(self.ss_x_ni),
            "penetration_wall_m" => Some(self.penetration_wall_m),
            "corsor_k0_per_min" => Some(self.corsor_k0_per_min),
            "corsor_q_kcal_per_mol" => Some(self.corsor_q_kcal_per_mol),
            "corsor_uo2_k0_per_min" => Some(self.corsor_uo2_k0_per_min),
            "corsor_uo2_q_kcal_per_mol" => Some(self.corsor_uo2_q_kcal_per_mol),
            "cs_inventory_kg" => Some(self.cs_inventory_kg),
            "i_inventory_kg" => Some(self.i_inventory_kg),
            "te_inventory_kg" => Some(self.te_inventory_kg),
            "dw_floor_m2" => Some(self.dw_floor_m2),
            "ww_pool_surface_m2" => Some(self.ww_pool_surface_m2),
            "head_failure_wall_k" => Some(self.head_failure_wall_k),
            "lm_p0" => Some(self.lm_p0),
            "lm_slope" => Some(self.lm_slope),
            "lm_c" => Some(self.lm_c),
            "creep_fail_strain" => Some(self.creep_fail_strain),
            "debris_porosity" => Some(self.debris_porosity),
            "debris_particle_m" => Some(self.debris_particle_m),
            "lp_bottom_level_m" => Some(self.lp_bottom_level_m),
            "breach_diameter_m" => Some(self.breach_diameter_m),
            "zr_melt_k" => Some(self.zr_melt_k),
            "clad_breach_k" => Some(self.clad_breach_k),
            "min_metal_m" => Some(self.min_metal_m),
            "rod_collapse_k" => Some(self.rod_collapse_k),
            "rod_collapse_max_k" => Some(self.rod_collapse_max_k),
            "damage_a" => Some(self.damage_a),
            "damage_b" => Some(self.damage_b),
            "assembly_pitch_m" => Some(self.assembly_pitch_m),
            "radial_exchange" => Some(self.radial_exchange),
            "axial_exchange" => Some(self.axial_exchange),
            "shroud_thickness_m" => Some(self.shroud_thickness_m),
            "zro2_density" => Some(self.zro2_density),
            "bubble_rise_m_s" => Some(self.bubble_rise_m_s),
            "max_pool_void" => Some(self.max_pool_void),
            "fission_share_u235" => Some(self.fission_share_u235),
            "fission_share_pu239" => Some(self.fission_share_pu239),
            "operating_time_s" => Some(self.operating_time_s),
            "fissions_per_fissile_atom" => Some(self.fissions_per_fissile_atom),
            "q_u235_mev" => Some(self.q_u235_mev),
            "q_pu239_mev" => Some(self.q_pu239_mev),
            "q_u238_mev" => Some(self.q_u238_mev),
            "u239_per_fission" => Some(self.u239_per_fission),
            "e_u239_mev" => Some(self.e_u239_mev),
            "e_np239_mev" => Some(self.e_np239_mev),
            "u239_decay_per_s" => Some(self.u239_decay_per_s),
            "np239_decay_per_s" => Some(self.np239_decay_per_s),
            "uh_low_a" => Some(self.uh_low_a),
            "uh_low_b_k" => Some(self.uh_low_b_k),
            "uh_high_a" => Some(self.uh_high_a),
            "uh_high_b_k" => Some(self.uh_high_b_k),
            "uh_low_max_k" => Some(self.uh_low_max_k),
            "uh_high_min_k" => Some(self.uh_high_min_k),
            "oxidation_min_k" => Some(self.oxidation_min_k),
            "h2o_h2_diffusivity_a" => Some(self.h2o_h2_diffusivity_a),
            "h2o_h2_diffusivity_b" => Some(self.h2o_h2_diffusivity_b),
            "oxidation_heat_j_per_kg_zr" => Some(self.oxidation_heat_j_per_kg_zr),
            "drywell_volume_m3" => Some(self.drywell_volume_m3),
            "dw_wall_area_m2" => Some(self.dw_wall_area_m2),
            "dw_wall_h" => Some(self.dw_wall_h),
            "dw_wall_c" => Some(self.dw_wall_c),
            "dw_wall_height_m" => Some(self.dw_wall_height_m),
            "dw_gap_m" => Some(self.dw_gap_m),
            "dw_gap_emissivity" => Some(self.dw_gap_emissivity),
            "dw_concrete_m" => Some(self.dw_concrete_m),
            "concrete_k" => Some(self.concrete_k),
            "rb_wall_h" => Some(self.rb_wall_h),
            "ww_shell_area_m2" => Some(self.ww_shell_area_m2),
            "ww_shell_c" => Some(self.ww_shell_c),
            "ww_shell_height_m" => Some(self.ww_shell_height_m),
            "wetwell_gas_m3" => Some(self.wetwell_gas_m3),
            "pool_water_m3" => Some(self.pool_water_m3),
            "containment_design_pa" => Some(self.containment_design_pa),
            "initial_containment_pa" => Some(self.initial_containment_pa),
            "initial_drywell_k" => Some(self.initial_drywell_k),
            "initial_pool_k" => Some(self.initial_pool_k),
            "vent_submergence_pa" => Some(self.vent_submergence_pa),
            "downcomer_area_m2" => Some(self.downcomer_area_m2),
            "downcomer_count" => Some(self.downcomer_count),
            "discharge_coefficient" => Some(self.discharge_coefficient),
            "vacuum_breaker_pa" => Some(self.vacuum_breaker_pa),
            "vacuum_breaker_m2" => Some(self.vacuum_breaker_m2),
            "vent_rupture_disk_pa" => Some(self.vent_rupture_disk_pa),
            "vent_area_m2" => Some(self.vent_area_m2),
            "flange_leak_pa" => Some(self.flange_leak_pa),
            "flange_leak_m2" => Some(self.flange_leak_m2),
            "reactor_building_m3" => Some(self.reactor_building_m3),
            "h2_flammable" => Some(self.h2_flammable),
            "h2_explosion" => Some(self.h2_explosion),
            "rcic_rated_kg_s" => Some(self.rcic_rated_kg_s),
            "hpci_rated_kg_s" => Some(self.hpci_rated_kg_s),
            "pump_efficiency" => Some(self.pump_efficiency),
            "turbine_efficiency" => Some(self.turbine_efficiency),
            "turbine_min_pa" => Some(self.turbine_min_pa),
            "level_l2_above_taf_m" => Some(self.level_l2_above_taf_m),
            "level_l8_above_taf_m" => Some(self.level_l8_above_taf_m),
            "msl_nozzle_above_taf_m" => Some(self.msl_nozzle_above_taf_m),
            "carryover_band_m" => Some(self.carryover_band_m),
            _ => None,
        }
    }

    /// Sets the input named `name`; `false` if there is none.
    pub fn set(&mut self, name: &str, value: f64) -> bool {
        match name {
            "thermal_power_w" => {
                self.thermal_power_w = value;
                true
            }
            "assemblies" => {
                self.assemblies = value;
                true
            }
            "rods_per_assembly" => {
                self.rods_per_assembly = value;
                true
            }
            "active_length_m" => {
                self.active_length_m = value;
                true
            }
            "clad_outer_r_m" => {
                self.clad_outer_r_m = value;
                true
            }
            "clad_wall_m" => {
                self.clad_wall_m = value;
                true
            }
            "pellet_r_m" => {
                self.pellet_r_m = value;
                true
            }
            "uo2_density" => {
                self.uo2_density = value;
                true
            }
            "zr_density" => {
                self.zr_density = value;
                true
            }
            "channel_inner_width_m" => {
                self.channel_inner_width_m = value;
                true
            }
            "channel_wall_m" => {
                self.channel_wall_m = value;
                true
            }
            "channel_length_m" => {
                self.channel_length_m = value;
                true
            }
            "uo2_cp" => {
                self.uo2_cp = value;
                true
            }
            "zr_cp" => {
                self.zr_cp = value;
                true
            }
            "uo2_k" => {
                self.uo2_k = value;
                true
            }
            "gap_h" => {
                self.gap_h = value;
                true
            }
            "boiling_h" => {
                self.boiling_h = value;
                true
            }
            "cosine_extrapolation" => {
                self.cosine_extrapolation = value;
                true
            }
            "rpv_radius_m" => {
                self.rpv_radius_m = value;
                true
            }
            "rpv_height_m" => {
                self.rpv_height_m = value;
                true
            }
            "rpv_wall_m" => {
                self.rpv_wall_m = value;
                true
            }
            "internals_envelope_m2" => {
                self.internals_envelope_m2 = value;
                true
            }
            "concrete_density" => {
                self.concrete_density = value;
                true
            }
            "concrete_ablation_k" => {
                self.concrete_ablation_k = value;
                true
            }
            "concrete_h2o_fraction" => {
                self.concrete_h2o_fraction = value;
                true
            }
            "concrete_co2_fraction" => {
                self.concrete_co2_fraction = value;
                true
            }
            "concrete_cp" => {
                self.concrete_cp = value;
                true
            }
            "concrete_ablation_j_per_kg" => {
                self.concrete_ablation_j_per_kg = value;
                true
            }
            "cavity_area_m2" => {
                self.cavity_area_m2 = value;
                true
            }
            "cavity_h" => {
                self.cavity_h = value;
                true
            }
            "cavity_emissivity" => {
                self.cavity_emissivity = value;
                true
            }
            "zr_co2_heat_j_per_kg" => {
                self.zr_co2_heat_j_per_kg = value;
                true
            }
            "baf_m" => {
                self.baf_m = value;
                true
            }
            "lower_plenum_free" => {
                self.lower_plenum_free = value;
                true
            }
            "upper_free" => {
                self.upper_free = value;
                true
            }
            "core_other_area_m2" => {
                self.core_other_area_m2 = value;
                true
            }
            "normal_level_above_taf_m" => {
                self.normal_level_above_taf_m = value;
                true
            }
            "initial_pressure_pa" => {
                self.initial_pressure_pa = value;
                true
            }
            "msiv_closure_s" => {
                self.msiv_closure_s = value;
                true
            }
            "bypass_gain_kg_s_pa" => {
                self.bypass_gain_kg_s_pa = value;
                true
            }
            "relief_valves" => {
                self.relief_valves = value;
                true
            }
            "relief_open_pa" => {
                self.relief_open_pa = value;
                true
            }
            "relief_close_pa" => {
                self.relief_close_pa = value;
                true
            }
            "safety_open_pa" => {
                self.safety_open_pa = value;
                true
            }
            "safety_close_pa" => {
                self.safety_close_pa = value;
                true
            }
            "relief_throat_m2" => {
                self.relief_throat_m2 = value;
                true
            }
            "ic_autostart_pa" => {
                self.ic_autostart_pa = value;
                true
            }
            "ic_ua_w_per_k" => {
                self.ic_ua_w_per_k = value;
                true
            }
            "ic_shell_water_kg" => {
                self.ic_shell_water_kg = value;
                true
            }
            "fire_engine_shutoff_pa" => {
                self.fire_engine_shutoff_pa = value;
                true
            }
            "fire_engine_max_kg_s" => {
                self.fire_engine_max_kg_s = value;
                true
            }
            "injection_temperature_k" => {
                self.injection_temperature_k = value;
                true
            }
            "hot_leak_gas_k" => {
                self.hot_leak_gas_k = value;
                true
            }
            "hot_leak_m2" => {
                self.hot_leak_m2 = value;
                true
            }
            "msl_lines" => {
                self.msl_lines = value;
                true
            }
            "msl_inner_radius_m" => {
                self.msl_inner_radius_m = value;
                true
            }
            "msl_wall_m" => {
                self.msl_wall_m = value;
                true
            }
            "msl_insulation_h" => {
                self.msl_insulation_h = value;
                true
            }
            "msl_lm_a1" => {
                self.msl_lm_a1 = value;
                true
            }
            "msl_lm_b1" => {
                self.msl_lm_b1 = value;
                true
            }
            "msl_lm_c" => {
                self.msl_lm_c = value;
                true
            }
            "internals_kg" => {
                self.internals_kg = value;
                true
            }
            "internals_area_m2" => {
                self.internals_area_m2 = value;
                true
            }
            "internals_h" => {
                self.internals_h = value;
                true
            }
            "internals_loss_h" => {
                self.internals_loss_h = value;
                true
            }
            "lower_head_failure_k" => {
                self.lower_head_failure_k = value;
                true
            }
            "breach_m2" => {
                self.breach_m2 = value;
                true
            }
            "debris_quench_s" => {
                self.debris_quench_s = value;
                true
            }
            "quench_h" => {
                self.quench_h = value;
                true
            }
            "quench_tau_s" => {
                self.quench_tau_s = value;
                true
            }
            "lower_head_thickness_m" => {
                self.lower_head_thickness_m = value;
                true
            }
            "debris_to_head_w_m2_k" => {
                self.debris_to_head_w_m2_k = value;
                true
            }
            "core_plate_m" => {
                self.core_plate_m = value;
                true
            }
            "core_plate_beam_m" => {
                self.core_plate_beam_m = value;
                true
            }
            "water_emissivity" => {
                self.water_emissivity = value;
                true
            }
            "flpup" => {
                self.flpup = value;
                true
            }
            "control_rods" => {
                self.control_rods = value;
                true
            }
            "blade_steel_kg" => {
                self.blade_steel_kg = value;
                true
            }
            "blade_area_m2" => {
                self.blade_area_m2 = value;
                true
            }
            "ss_ox_a" => {
                self.ss_ox_a = value;
                true
            }
            "ss_ox_b" => {
                self.ss_ox_b = value;
                true
            }
            "ss_ox_min_k" => {
                self.ss_ox_min_k = value;
                true
            }
            "ss_melt_k" => {
                self.ss_melt_k = value;
                true
            }
            "ss_fusion_j_per_kg" => {
                self.ss_fusion_j_per_kg = value;
                true
            }
            "debris_k" => {
                self.debris_k = value;
                true
            }
            "fuel_melt_k" => {
                self.fuel_melt_k = value;
                true
            }
            "fuel_fusion_j_per_kg" => {
                self.fuel_fusion_j_per_kg = value;
                true
            }
            "concrete_oxide_fusion_j_per_mol" => {
                self.concrete_oxide_fusion_j_per_mol = value;
                true
            }
            "concrete_oxide_m" => {
                self.concrete_oxide_m = value;
                true
            }
            "bur_xh2ign" => {
                self.bur_xh2ign = value;
                true
            }
            "bur_xcoign" => {
                self.bur_xcoign = value;
                true
            }
            "bur_xo2ig" => {
                self.bur_xo2ig = value;
                true
            }
            "bur_xmscig" => {
                self.bur_xmscig = value;
                true
            }
            "bur_xh2cc" => {
                self.bur_xh2cc = value;
                true
            }
            "bur_xcocc" => {
                self.bur_xcocc = value;
                true
            }
            "bur_cc_slope" => {
                self.bur_cc_slope = value;
                true
            }
            "bur_cc_y0" => {
                self.bur_cc_y0 = value;
                true
            }
            "h2_combustion_j_per_mol" => {
                self.h2_combustion_j_per_mol = value;
                true
            }
            "co_combustion_j_per_mol" => {
                self.co_combustion_j_per_mol = value;
                true
            }
            "fdi_surface_tension" => {
                self.fdi_surface_tension = value;
                true
            }
            "fdi_hpme_velocity" => {
                self.fdi_hpme_velocity = value;
                true
            }
            "candling_uo2_fraction" => {
                self.candling_uo2_fraction = value;
                true
            }
            "pool_melt_k" => {
                self.pool_melt_k = value;
                true
            }
            "pool_nu_a" => {
                self.pool_nu_a = value;
                true
            }
            "pool_nu_n" => {
                self.pool_nu_n = value;
                true
            }
            "pool_k" => {
                self.pool_k = value;
                true
            }
            "pool_mu" => {
                self.pool_mu = value;
                true
            }
            "pool_beta" => {
                self.pool_beta = value;
                true
            }
            "pool_cp" => {
                self.pool_cp = value;
                true
            }
            "pool_rho" => {
                self.pool_rho = value;
                true
            }
            "blade_b4c_kg" => {
                self.blade_b4c_kg = value;
                true
            }
            "b4c_rate_a" => {
                self.b4c_rate_a = value;
                true
            }
            "b4c_rate_b" => {
                self.b4c_rate_b = value;
                true
            }
            "b4c_max_fraction" => {
                self.b4c_max_fraction = value;
                true
            }
            "ss_x_fe" => {
                self.ss_x_fe = value;
                true
            }
            "ss_x_cr" => {
                self.ss_x_cr = value;
                true
            }
            "ss_x_ni" => {
                self.ss_x_ni = value;
                true
            }
            "penetration_wall_m" => {
                self.penetration_wall_m = value;
                true
            }
            "corsor_k0_per_min" => {
                self.corsor_k0_per_min = value;
                true
            }
            "corsor_q_kcal_per_mol" => {
                self.corsor_q_kcal_per_mol = value;
                true
            }
            "corsor_uo2_k0_per_min" => {
                self.corsor_uo2_k0_per_min = value;
                true
            }
            "corsor_uo2_q_kcal_per_mol" => {
                self.corsor_uo2_q_kcal_per_mol = value;
                true
            }
            "cs_inventory_kg" => {
                self.cs_inventory_kg = value;
                true
            }
            "i_inventory_kg" => {
                self.i_inventory_kg = value;
                true
            }
            "te_inventory_kg" => {
                self.te_inventory_kg = value;
                true
            }
            "dw_floor_m2" => {
                self.dw_floor_m2 = value;
                true
            }
            "ww_pool_surface_m2" => {
                self.ww_pool_surface_m2 = value;
                true
            }
            "head_failure_wall_k" => {
                self.head_failure_wall_k = value;
                true
            }
            "lm_p0" => {
                self.lm_p0 = value;
                true
            }
            "lm_slope" => {
                self.lm_slope = value;
                true
            }
            "lm_c" => {
                self.lm_c = value;
                true
            }
            "creep_fail_strain" => {
                self.creep_fail_strain = value;
                true
            }
            "debris_porosity" => {
                self.debris_porosity = value;
                true
            }
            "debris_particle_m" => {
                self.debris_particle_m = value;
                true
            }
            "lp_bottom_level_m" => {
                self.lp_bottom_level_m = value;
                true
            }
            "breach_diameter_m" => {
                self.breach_diameter_m = value;
                true
            }
            "zr_melt_k" => {
                self.zr_melt_k = value;
                true
            }
            "clad_breach_k" => {
                self.clad_breach_k = value;
                true
            }
            "min_metal_m" => {
                self.min_metal_m = value;
                true
            }
            "rod_collapse_k" => {
                self.rod_collapse_k = value;
                true
            }
            "rod_collapse_max_k" => {
                self.rod_collapse_max_k = value;
                true
            }
            "damage_a" => {
                self.damage_a = value;
                true
            }
            "damage_b" => {
                self.damage_b = value;
                true
            }
            "assembly_pitch_m" => {
                self.assembly_pitch_m = value;
                true
            }
            "radial_exchange" => {
                self.radial_exchange = value;
                true
            }
            "axial_exchange" => {
                self.axial_exchange = value;
                true
            }
            "shroud_thickness_m" => {
                self.shroud_thickness_m = value;
                true
            }
            "zro2_density" => {
                self.zro2_density = value;
                true
            }
            "bubble_rise_m_s" => {
                self.bubble_rise_m_s = value;
                true
            }
            "max_pool_void" => {
                self.max_pool_void = value;
                true
            }
            "fission_share_u235" => {
                self.fission_share_u235 = value;
                true
            }
            "fission_share_pu239" => {
                self.fission_share_pu239 = value;
                true
            }
            "operating_time_s" => {
                self.operating_time_s = value;
                true
            }
            "fissions_per_fissile_atom" => {
                self.fissions_per_fissile_atom = value;
                true
            }
            "q_u235_mev" => {
                self.q_u235_mev = value;
                true
            }
            "q_pu239_mev" => {
                self.q_pu239_mev = value;
                true
            }
            "q_u238_mev" => {
                self.q_u238_mev = value;
                true
            }
            "u239_per_fission" => {
                self.u239_per_fission = value;
                true
            }
            "e_u239_mev" => {
                self.e_u239_mev = value;
                true
            }
            "e_np239_mev" => {
                self.e_np239_mev = value;
                true
            }
            "u239_decay_per_s" => {
                self.u239_decay_per_s = value;
                true
            }
            "np239_decay_per_s" => {
                self.np239_decay_per_s = value;
                true
            }
            "uh_low_a" => {
                self.uh_low_a = value;
                true
            }
            "uh_low_b_k" => {
                self.uh_low_b_k = value;
                true
            }
            "uh_high_a" => {
                self.uh_high_a = value;
                true
            }
            "uh_high_b_k" => {
                self.uh_high_b_k = value;
                true
            }
            "uh_low_max_k" => {
                self.uh_low_max_k = value;
                true
            }
            "uh_high_min_k" => {
                self.uh_high_min_k = value;
                true
            }
            "oxidation_min_k" => {
                self.oxidation_min_k = value;
                true
            }
            "h2o_h2_diffusivity_a" => {
                self.h2o_h2_diffusivity_a = value;
                true
            }
            "h2o_h2_diffusivity_b" => {
                self.h2o_h2_diffusivity_b = value;
                true
            }
            "oxidation_heat_j_per_kg_zr" => {
                self.oxidation_heat_j_per_kg_zr = value;
                true
            }
            "drywell_volume_m3" => {
                self.drywell_volume_m3 = value;
                true
            }
            "dw_wall_area_m2" => {
                self.dw_wall_area_m2 = value;
                true
            }
            "dw_wall_h" => {
                self.dw_wall_h = value;
                true
            }
            "dw_wall_c" => {
                self.dw_wall_c = value;
                true
            }
            "dw_wall_height_m" => {
                self.dw_wall_height_m = value;
                true
            }
            "dw_gap_m" => {
                self.dw_gap_m = value;
                true
            }
            "dw_gap_emissivity" => {
                self.dw_gap_emissivity = value;
                true
            }
            "dw_concrete_m" => {
                self.dw_concrete_m = value;
                true
            }
            "concrete_k" => {
                self.concrete_k = value;
                true
            }
            "rb_wall_h" => {
                self.rb_wall_h = value;
                true
            }
            "ww_shell_area_m2" => {
                self.ww_shell_area_m2 = value;
                true
            }
            "ww_shell_c" => {
                self.ww_shell_c = value;
                true
            }
            "ww_shell_height_m" => {
                self.ww_shell_height_m = value;
                true
            }
            "wetwell_gas_m3" => {
                self.wetwell_gas_m3 = value;
                true
            }
            "pool_water_m3" => {
                self.pool_water_m3 = value;
                true
            }
            "containment_design_pa" => {
                self.containment_design_pa = value;
                true
            }
            "initial_containment_pa" => {
                self.initial_containment_pa = value;
                true
            }
            "initial_drywell_k" => {
                self.initial_drywell_k = value;
                true
            }
            "initial_pool_k" => {
                self.initial_pool_k = value;
                true
            }
            "vent_submergence_pa" => {
                self.vent_submergence_pa = value;
                true
            }
            "downcomer_area_m2" => {
                self.downcomer_area_m2 = value;
                true
            }
            "downcomer_count" => {
                self.downcomer_count = value;
                true
            }
            "discharge_coefficient" => {
                self.discharge_coefficient = value;
                true
            }
            "vacuum_breaker_pa" => {
                self.vacuum_breaker_pa = value;
                true
            }
            "vacuum_breaker_m2" => {
                self.vacuum_breaker_m2 = value;
                true
            }
            "vent_rupture_disk_pa" => {
                self.vent_rupture_disk_pa = value;
                true
            }
            "vent_area_m2" => {
                self.vent_area_m2 = value;
                true
            }
            "flange_leak_pa" => {
                self.flange_leak_pa = value;
                true
            }
            "flange_leak_m2" => {
                self.flange_leak_m2 = value;
                true
            }
            "reactor_building_m3" => {
                self.reactor_building_m3 = value;
                true
            }
            "h2_flammable" => {
                self.h2_flammable = value;
                true
            }
            "h2_explosion" => {
                self.h2_explosion = value;
                true
            }
            "rcic_rated_kg_s" => {
                self.rcic_rated_kg_s = value;
                true
            }
            "hpci_rated_kg_s" => {
                self.hpci_rated_kg_s = value;
                true
            }
            "pump_efficiency" => {
                self.pump_efficiency = value;
                true
            }
            "turbine_efficiency" => {
                self.turbine_efficiency = value;
                true
            }
            "turbine_min_pa" => {
                self.turbine_min_pa = value;
                true
            }
            "level_l2_above_taf_m" => {
                self.level_l2_above_taf_m = value;
                true
            }
            "level_l8_above_taf_m" => {
                self.level_l8_above_taf_m = value;
                true
            }
            "msl_nozzle_above_taf_m" => {
                self.msl_nozzle_above_taf_m = value;
                true
            }
            "carryover_band_m" => {
                self.carryover_band_m = value;
                true
            }
            _ => false,
        }
    }
}

// ── Units 2 and 3 (gh:#1101) ────────────────────────────────────────────────
//
// Every value below comes from
// `verification_and_validation/bwr_mark1/units23_open_data.md`, whose source
// keys (KAN, ICANPS, NISA, …) the comments use. Marks, as in that file:
// [S] a search excerpt of a named public document, the primary not opened
// (treat as [U]); [D] derived here from [S] values, the derivation shown;
// [A] assumed: a field not set here is Unit 1's default, borrowed.

/// Unit 3's rupture-disk set pressure as ANS-A gives it, Pa absolute: the
/// rejected side of the conflict that [`Params::unit3`] resolves to
/// 0.528 MPa abs (see there). Kept so that a caller can run it, and so the
/// conflict stays visible; the two are never averaged. \[S\] ANS Special
/// Committee on Fukushima, Appendix A (2012): "0.549 MPa".
pub const UNIT3_RUPTURE_DISK_ANS_PA: f64 = 0.549e6;

/// Units 2 and 3.
impl Params {
    /// Fukushima Daiichi Unit 2 (GE BWR/4, Mark I), for the fields an open
    /// source gives (gh:#1101). Every field not listed below is Unit 1's
    /// default, borrowed \[A\]; among them, plainly scale-dependent and
    /// **not** scaled (no sourced ratio fits them): the relief-valve count
    /// and throat, the main steam lines, the dome internals, the vessel's
    /// water levels. The isolation-condenser fields mean nothing for
    /// Units 2 and 3, which had RCIC and HPCI instead; ~~no RCIC or HPCI model
    /// exists in this crate, so no Unit 2 or 3 accident sequence can be run.~~
    /// **CORRECTED 2026-11-02 (gh:#1109):** [`super::turbine_pump`] models
    /// them (the `rcic_*`, `hpci_*`, `pump_*`, `turbine_*`, `level_*` and
    /// carry-over inputs) and [`super::units23`] holds the two sequences, so
    /// `Plant::for_unit` runs them; the plant never runs the isolation
    /// condensers for these units.
    ///
    /// | field | value | mark and source |
    /// |---|---|---|
    /// | `thermal_power_w` | 2381 MWt | \[S\] ICANPS |
    /// | `assemblies`, `control_rods` | 548, 137 | \[S\] TEPCO-OL, KTH12 |
    /// | `rpv_radius_m` | 2.8 m (5.6 m ID) | \[S\] TEPCO-OL, KTH12 |
    /// | `rpv_height_m` | 22 m | \[S\] SI2 (Unit 3's sources say 21 m, see [`Params::unit3`]) |
    /// | `cs_`, `i_`, `te_inventory_kg` | Unit 1's × 2381/1380 | \[D\] the power ratio (ICANPS); the same irradiation history \[A\] |
    /// | `drywell_volume_m3` | 4240 m³ | \[S\] KAN, JNST25 |
    /// | `wetwell_gas_m3`, `pool_water_m3` | 3160, 2980 m³ | \[S\] KAN, JNST25, MDPI23 |
    /// | `containment_design_pa` | 0.384 MPa g | \[S\] NISA (ICANPS, INPO, NAS agree) |
    /// | `vent_rupture_disk_pa` | 0.528 MPa abs | \[S\] INPO (62 psig), AESJ4 |
    /// | `downcomer_count` | 96 | \[S\] SI2 |
    /// | `downcomer_area_m2` | 27.71 m² | \[D\] Unit 1's 20 m² \[A\] × the main-vent area ratio (2.06/1.75)², SI1, SI2 |
    /// | `dw_floor_m2`, `dw_wall_area_m2`, `dw_wall_c` | Unit 1's × 1.1563 | \[D\] Unit 1's \[A\] × the drywell-volume ratio (4240/3410)^(2/3), KAN |
    /// | `ww_pool_surface_m2` | 906.3 m² | \[D\] the network's volume-fit torus (R 16.75 m \[S\], volumes \[S\]) at the initial level |
    /// | `ww_shell_area_m2`, `ww_shell_c` | 1492.9 m², 8.789e7 J/K | \[D\] the sourced geometric torus (R 16.75, r 4.45 m, SI2, MDPI23, TEPCO16) above the pool's level; Unit 1's 15 mm steel \[A\] |
    /// | `reactor_building_m3` | 38 000 m³ | \[A\] **borrowed from Unit 3** (KTH12), not Unit 1: same plant size; Unit 2's is not found |
    pub fn unit2() -> Self {
        let power_ratio = 2381.0e6 / THERMAL_POWER_W;
        // Geometric similarity of the drywell: an area scales as V^(2/3).
        let dw_area_ratio = (4240.0 / DRYWELL_VOLUME_M3).r_powf(2.0 / 3.0);
        // The sourced geometric torus (R 16.75 m, r 4.45 m) and the pool's
        // level on it, filled to the pool's share of the free volume,
        // 2980 / (2980 + 3160) = 0.4853, as a circular segment: the level
        // sits 0.023027 r below the tube's centre, so the wetted arc's
        // half-angle from the bottom is acos(0.023027) and the shell above
        // the water is 1 - acos(0.023027) / pi = 0.50733 of 4 pi^2 R r =
        // 2942.6 m^2. Recomputed by `params::tests::units_2_and_3_derive_from_their_sources`.
        let shell_total = 4.0 * std::f64::consts::PI * std::f64::consts::PI * 16.75 * 4.45;
        let ww_shell_area = shell_total * UNITS23_SHELL_ABOVE_POOL;
        Self {
            thermal_power_w: 2381.0e6,
            assemblies: 548.0,
            control_rods: 137.0,
            rpv_radius_m: 2.8,
            rpv_height_m: 22.0,
            cs_inventory_kg: CS_INVENTORY_KG * power_ratio,
            i_inventory_kg: I_INVENTORY_KG * power_ratio,
            te_inventory_kg: TE_INVENTORY_KG * power_ratio,
            drywell_volume_m3: 4240.0,
            wetwell_gas_m3: 3160.0,
            pool_water_m3: 2980.0,
            containment_design_pa: 0.384e6 + P_ATM,
            vent_rupture_disk_pa: 0.528e6,
            downcomer_count: 96.0,
            downcomer_area_m2: DOWNCOMER_AREA_M2 * (2.06_f64 / 1.75).r_powi(2),
            dw_floor_m2: DW_FLOOR_M2 * dw_area_ratio,
            dw_wall_area_m2: DW_WALL_AREA_M2 * dw_area_ratio,
            dw_wall_c: DW_WALL_C * dw_area_ratio,
            ww_pool_surface_m2: UNITS23_WW_POOL_SURFACE_M2,
            ww_shell_area_m2: ww_shell_area,
            // Unit 1's 15 mm of carbon steel (7850 kg/m^3, 500 J/(kg K)) [A].
            ww_shell_c: ww_shell_area * 0.015 * 7850.0 * 500.0,
            reactor_building_m3: 38_000.0,
            ..Self::default()
        }
    }

    /// Fukushima Daiichi Unit 3: [`Params::unit2`]'s values (KAN's
    /// 2381 MWt column, SI3 and the NISA tables give Unit 3 the same
    /// containment; SI3 calls its pool "an estimate based on unit 2", so
    /// for Unit 3 the pool is effectively \[A\] from Unit 2), except:
    ///
    /// | field | value | mark and source |
    /// |---|---|---|
    /// | `rpv_height_m` | 21 m | \[S\] IRID, KTH12 (two sources for Unit 3 itself); SI3's 22 m is recorded, not used |
    /// | `reactor_building_m3` | 38 000 m³ | \[S\] KTH12 (asterisked in its table) |
    /// | `vent_rupture_disk_pa` | 0.528 MPa abs | \[S\] **conflict**, see below |
    /// | `rcic_rated_kg_s` | 97 t/h (26.94 kg/s) | \[S\] KTH12, "RCIC nominal flow rate : 97 t/h" (gh:#1109); Unit 2's 95 m³/h is TEPCO's |
    ///
    /// **The rupture disk: 0.528 against 0.549 MPa abs.** 0.528 is used.
    /// AESJ4 states the rule that the disk is set "according to the maximum
    /// working pressure of the PCV", and NISA gives Units 2 and 3 a maximum
    /// working pressure of 0.427 MPa g, which is 0.528 MPa abs; INPO gives
    /// the same 62 psig for the Unit 2 disk. 0.549 (ANS-A,
    /// [`UNIT3_RUPTURE_DISK_ANS_PA`]) is Unit 1's code value, and the
    /// TEPCO slide that pairs 0.549 with a 0.528 maximum working pressure
    /// looks like Unit 1's; so 0.549 for Unit 3 may be a carry-over. Not
    /// settled: IAEA TV1 and TEPCO's June 2012 report would settle it.
    pub fn unit3() -> Self {
        // RCIC 97 t/h [S] KTH EMUG 2012 (Unit 3's own slide, page not seen).
        Self { rpv_height_m: 21.0, rcic_rated_kg_s: 97_000.0 / 3600.0, ..Self::unit2() }
    }
}

/// The torus shell's share above the pool for Units 2 and 3 (see
/// [`Params::unit2`]): `1 - acos(δ)/π` with `δ` the level's depth below the
/// tube centre over `r` for a circular segment holding 0.4853 of the
/// circle, `δ` = 0.023027 (solved by bisection in
/// `params::tests::units_2_and_3_derive_from_their_sources`; the hand
/// estimate written before the run, 0.50741, **MISSED**: it took the share
/// from the IF97 pool volume, 2978/6138, not 2980/6140). \[D\]
pub const UNITS23_SHELL_ABOVE_POOL: f64 = 0.507_33;
/// The torus pool's free surface for Units 2 and 3, m^2: the area of the
/// network's volume-fit torus (R 16.75 m \[S\], its tube radius 4.309 m
/// from the sourced 3160 + 2980 m³) at the initial level, read from the
/// assembled network by `tests/units_2_and_3_network.rs` (906.26 m²; the
/// hand estimate before the run was 907). The data file's 936 m² is the
/// sourced geometric tube (r = 4.45 m), which holds 6.7 % more than the
/// free volume. \[D\]
pub const UNITS23_WW_POOL_SURFACE_M2: f64 = 906.3;

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
        assert_eq!(Params::INPUTS.len(), 213);
    }

    /// gh:#1101: the Unit 2 and 3 values derived from sourced ones
    /// (`Params::unit2`'s table) recompute from their sources, and Unit 3
    /// differs from Unit 2 only where its doc says. Predictions written
    /// 2026-11-01 before the first run, by hand: the shell's share above the
    /// pool 0.50741 (δ = 0.02328); the downcomers 27.71 m²; the drywell
    /// area ratio 1.1563; the inventories ×1.7254. **Result (2026-11-01):**
    /// δ = 0.023027, the share 0.507330: the first prediction **MISSED**
    /// (the hand arithmetic took the pool's share as 2978/6138, the IF97
    /// volume, not the stated 2980/6140); the constant was corrected to
    /// the computed 0.50733 (the shell 1492.9 m², 8.789e7 J/K). The rest
    /// held.
    #[test]
    fn units_2_and_3_derive_from_their_sources() {
        let (u1, u2, u3) = (Params::default(), Params::unit2(), Params::unit3());
        // The circular segment holding the pool's share of the free volume.
        let share = 2980.0 / (2980.0 + 3160.0);
        let frac = |d: f64| {
            let a = d.r_acos();
            (a - a.r_sin() * a.r_cos()) / std::f64::consts::PI
        };
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if frac(mid) > share { lo = mid } else { hi = mid }
        }
        let delta = 0.5 * (lo + hi);
        let above = 1.0 - delta.r_acos() / std::f64::consts::PI;
        println!("units 2/3: delta {delta:.6}, shell above the pool {above:.6}, shell {:.1} m2, C {:.4e} J/K", u2.ww_shell_area_m2, u2.ww_shell_c);
        assert!((above - UNITS23_SHELL_ABOVE_POOL).abs() < 1e-5, "{above}");
        // The downcomers from the main-vent area ratio; the drywell's areas
        // from its volume ratio; the inventories from the power ratio.
        assert!((u2.downcomer_area_m2 - 27.713).abs() < 1e-3, "{}", u2.downcomer_area_m2);
        let r = u2.dw_floor_m2 / u1.dw_floor_m2;
        assert!((r - 1.15632).abs() < 1e-4 && r == u2.dw_wall_area_m2 / u1.dw_wall_area_m2, "{r}");
        assert!((u2.cs_inventory_kg / u1.cs_inventory_kg - 2381.0 / 1380.0).abs() < 1e-14);
        // Design pressure 0.485 MPa abs (NISA's own conversion), the disk
        // above it as on Unit 1.
        assert!((u2.containment_design_pa - 0.485e6).abs() < 0.001e6 && u2.vent_rupture_disk_pa > u2.containment_design_pa);
        // Unit 3 is Unit 2 but for the vessel's height; the conflicting
        // disk value is kept, not averaged in.
        assert_eq!(Params { rpv_height_m: 22.0, rcic_rated_kg_s: u2.rcic_rated_kg_s, ..u3 }, u2);
        assert!((u3.rcic_rated_kg_s - 26.944).abs() < 1e-3 && (u2.rcic_rated_kg_s - 26.341).abs() < 1e-3);
        assert!(u3.rpv_height_m == 21.0 && u3.vent_rupture_disk_pa == 0.528e6 && UNIT3_RUPTURE_DISK_ANS_PA == 0.549e6);
        // Unit 1 is untouched.
        assert_eq!(u1.thermal_power_w, THERMAL_POWER_W);
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
