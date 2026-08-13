//! Marviken full-scale critical-flow tests (CFT) — HEM verification &
//! validation for the 500 mm / (L/D) = 0.3 nozzle, tests 23 and 24.
//!
//! # Reference
//!
//! *The Marviken Full Scale Critical Flow Tests: Summary Report*,
//! **NUREG/CR-2671 (MXC-301)**, Joint Reactor Safety Experiments in the
//! Marviken Power Station, Sweden; published by the U.S. Nuclear Regulatory
//! Commission, Division of Accident Evaluation, May 1982 (manuscript completed
//! December 1979), 288 pp. US federal-government / NTIS publication, openly
//! published, availability statement **UNLIMITED**, security class
//! **UNCLASSIFIED**. Catalogued in this workspace's KOVAN literature archive as
//! `crates/kovan-literature/open/reports/nureg-cr-2671-marviken.json`
//! (slug `nrc1982marviken`, sha256 `aaf3ffed…8c50a7cc`, accessed 2026-08-11);
//! no DOI or source URL is recorded in the report or the catalogue entry.
//! Compliant with `DATA_POLICY.md`: openly published literature data only.
//!
//! **Page numbering.** PDF page = report page + 16 (verified at five points).
//! Every citation below gives the report page first and the PDF page second.
//!
//! # The case, stated in full (so it can be reproduced elsewhere)
//!
//! Everything a higher-fidelity model needs to set up the *identical* case:
//!
//! | Quantity | Value | Source |
//! |---|---|---|
//! | Nozzle bore `D` (= throat = exit) | 500 mm | Table 3:1, p.7 / PDF 23 |
//! | Nozzle length `L` | 166 mm | Table 3:1 |
//! | `L/D` | 0.3 | Table 3:1 |
//! | Inlet | rounded, radius of curvature `R` = 250 mm = nozzle radius, tangent to the bore | §3, p.6 / PDF 22 |
//! | Nozzle shape | **converging only** — rounded inlet then a constant-diameter cylinder; there is *no* diverging section | §3, p.6 / PDF 22 |
//! | Exit geometry | straight (non-flared) ring fastener, used from test 15 onward; the flare→straight change had "no measurable effect on the nozzle mass flow rates" | §3 p.6; §8.5.3 p.66 / PDF 82 |
//! | Vessel | 425 m^3 net volume; velocity at the nozzle inlet is negligible, so vessel state = stagnation state | §3, p.4 / PDF 20 |
//! | Initial steam-dome pressure | 4.96 MPa (both tests) | Table 4:2, p.21 / PDF 37, row 3 |
//! | Initial saturation temperature | 263 degC (both tests) | Table 4:2, row 4 |
//! | Test 23 nominal subcooling | **3 K** (category III) | Table 4:2, row 5; Table 4:1, p.18 / PDF 34 |
//! | Test 24 nominal subcooling | **33 K** (category II) | Table 4:2, row 5; Table 4:1 |
//! | Test 23 minimum vessel fluid temperature | 260 degC | Table 4:2, row 6 |
//! | Test 24 minimum vessel fluid temperature | 230 degC | Table 4:2, row 6 |
//! | Initial vessel level | 19.85 m (t23), 19.88 m (t24) | Table 4:2, row 11 |
//! | Test period | 69 s (t23), 54 s (t24) | Table 4:2, row 16 |
//! | Subcooled → transition → saturated times | t23: `t_T` = 4.9 s, `t_s` = 6.8 s; t24: `t_T` = 21.7 s, `t_s` = 28.5 s | Table 8:1, p.72 / PDF 88 |
//! | Discharge receiver | reactor containment (drywell, 3234 m^3), **not** the atmosphere | §3, pp.4-5 / PDF 20-21 |
//! | Measured peak containment pressure | 236 kPa (t23), 328 kPa (t24) | Table 8:4, p.76 / PDF 92 |
//!
//! **Back pressure.** NUREG/CR-2671 never states an *initial* containment
//! pressure for tests 23/24, and never states a nozzle-exit pressure for them.
//! This file therefore takes 1 atm as an assumed lower bound and says so. The
//! assumption is inconsequential here: the lowest choke pressure returned by the
//! dispatcher over all 69 points is 2.774 MPa (test 24; test 23's lowest is 3.723 MPa) (printed and asserted by
//! [`dispatcher_deviations`]), an order of magnitude above even the *peak*
//! containment pressure of 328 kPa, so the nozzle is choked throughout and the
//! critical mass flux does not depend on the back pressure. A model that
//! resolves the nozzle interior (drift-flux, two-fluid) should use 100 kPa
//! rising to the measured peak, and verify the same insensitivity rather than
//! assuming it.
//!
//! ## Stagnation state as a function of vessel pressure
//!
//! Fig. 8:24 is a *test envelope*: measured nozzle mass flux against measured
//! nozzle-inlet stagnation pressure, traced out as the vessel blows down. It
//! does not carry the inlet temperature, so the stagnation state has to be
//! reconstructed. This file uses the report's own picture of the vessel
//! (§4.3, p.17 / PDF 33) as a **one-parameter-per-test** model:
//!
//! - The vessel water sits at a "zone of relatively constant subcooling", at the
//!   minimum vessel fluid temperature `T_w` of Table 4:2 row 6 — 260 degC for
//!   test 23, 230 degC for test 24.
//! - While `Tsat(p) > T_w` the fluid entering the nozzle is **subcooled liquid
//!   at `(p, T_w)`**.
//! - Once `Tsat(p) <= T_w` the vessel water has flashed and rides the saturation
//!   line, so the fluid entering the nozzle is **saturated liquid at `p`**
//!   (`x = 0`).
//!
//! That second branch is parameter-free, and it is corroborated by the report:
//! Table 8:3 (p.74 / PDF 90) tabulates the "universal stagnation quality"
//! `x = (v - v_l(P)) / (v_v(P) - v_l(P))` at the nozzle inlet and gives
//! `x <= 60e-4` for test 23 and `x <= 29.4e-4` for test 24 across the whole
//! saturated period — i.e. the inlet fluid never departs far from the
//! saturated-liquid line. (A constant-stagnation-enthalpy model, by contrast,
//! reaches `x ~ 0.04` at the low-pressure end and over-predicts flashing; it was
//! tried and rejected — see "Dead ends" below.) The subcooled branch is
//! corroborated too: Table 8:3 gives test 24 `x_min = -17e-4` at 4.78 MPa, which
//! is what `(v_l(230 degC) - v_l(4.78 MPa)) / (v_v - v_l)` evaluates to.
//!
//! [`marviken_nozzle_inlet_stagnation_enthalpy`] implements exactly this and is
//! the single point of truth for the case setup.
//!
//! # Data provenance and digitisation
//!
//! The measured points in [`TEST_23_POINTS`] / [`TEST_24_POINTS`] were read off
//! **Fig. 8:24**, "The test envelope for the nozzle with diameter 500 mm and
//! L/D = 0.3: nozzle mass flux and inlet stagnation pressure", report **p.100 /
//! PDF p.116**, using the `graphreader` web tool. (An earlier revision of this
//! file cited PDF p.113; that was wrong — the offset is +16, not +13.)
//! Fig. 8:24 covers tests 23 and 24 and no others, because Table 3:1 and
//! Table 4:1 both list the 500 mm / `L/D` = 0.3 nozzle as used in exactly those
//! two tests. The x axis is `INLET PRESSURE (KPA)`, ticked 2500…5000.
//!
//! **Digitisation uncertainty — measured, not asserted.** Table 8:3 (p.74 /
//! PDF 90) independently tabulates the mass flux at four envelope endpoints per
//! test. [`digitised_points_agree_with_nureg_table_8_3`] compares the digitised
//! series against all eight and measured (2026-08-11):
//! **RMS 6.3 %, worst 14.8 %.** Seven of the eight agree to within 5.7 %; the
//! 14.8 % outlier is test 24's subcooled endpoint at 3.15 MPa, which sits on the
//! near-vertical part of the curve where a small pressure reading error maps to
//! a large flux error. Reading error is therefore **about +/-6 % over most of
//! the range, degrading to about +/-15 % in the steep 3.0-3.3 MPa transition**.
//!
//! # Error budget (this is where the tolerance comes from)
//!
//! | Source | Magnitude | Basis |
//! |---|---|---|
//! | Experimental scatter within a narrow pressure window | **+/-12.9 % (t23), +/-20.0 % (t24)** | measured by [`marviken_measured_scatter`]; half-spread of all digitised points inside a +/-60 kPa window |
//! | Experimental measurement uncertainty | +/-3 to 10 % (subcooled pitot-static), +/-8 to 15 % (two-phase pitot-static), +/-6 % (mass inventory, up to +/-12 % for 500 mm nozzles during rapid change) | §7, pp.47-48 / PDF 63-64 |
//! | Digitisation reading error | RMS 6.3 %, worst 14.8 % | measured, above |
//! | Numerical discretisation of the model | **< 0.001 %** | measured: the 1000-point isentrope scan in [`hem_maximum_mass_flux`] agrees with a 20000-point scan to better than 1e-5 relative; the dispatcher's golden section closes to 1 Pa |
//!
//! **The dominant source is experimental — the scatter of the measurement
//! itself (+/-13 to 20 %), not the digitisation and emphatically not the
//! numerics.** Adding the experimental scatter and the digitisation RMS in
//! quadrature gives sqrt(20^2 + 6.3^2) = 21 % for test 24 and
//! sqrt(12.9^2 + 6.3^2) = 14 % for test 23. Rounding the worse of the two up to
//! a round number sets the reference band:
//!
//! - **per-point pass criterion: `|G_HEM/G_meas - 1| <= 0.25`**
//! - **aggregate pass criterion: mean `|G_HEM/G_meas - 1| <= 0.15`**
//!
//! These were fixed from the budget above *before* the outcome was read, and
//! were not adjusted afterwards. The Marviken deviations reported below are
//! either comfortably inside them or miss them by a factor of three; nothing
//! sits close enough to the line for the rounding to matter.
//!
//! # Results — measured 2026-08-11, tampines-steam-tables v0.2.5, release mode
//!
//! Model under test: [`get_critical_pressure_and_mass_flux_multiphase_ph`], the
//! crate's public HEM critical-flow dispatcher.
//!
//! | Test | Nominal subcooling | n points | mean abs dev | mean signed dev | max abs dev | Verdict |
//! |---|---|---|---|---|---|---|
//! | 23 | 3 K | 29 | **12.6 %** | **+5.9 %** | **23.1 %** | **VALIDATED** within the budget |
//! | 24 | 33 K | 40 | **48.6 %** | **-48.5 %** | **70.2 %** | **NOT VALIDATED** — misses the budget by ~3.2x |
//!
//! **Test 23 (near-saturated, 3 K) validates.** Both criteria are met, though
//! without much margin: the 23.1 % worst point is 92 % of the 25 % band, and it
//! is one of the three high-pressure outliers at 4.80 / 4.88 / 4.94 MPa that lie
//! inside the +/-12.9 % measured scatter of their own neighbours. The signed
//! mean is only +5.9 %, so there is no strong systematic bias.
//!
//! **Test 24 (33 K subcooled) does not validate, and the failure is
//! systematic, not scattered.** The dispatcher returns an almost
//! pressure-independent 16.85-16.95 Mg/(m^2 s) across the whole 2.83-4.77 MPa
//! sweep, while the measurement climbs from 16.7 to 56.6 Mg/(m^2 s). The
//! deviation therefore tracks the *local* subcooling almost perfectly:
//!
//! | local `Tsat(p) - T_w` | deviation |
//! |---|---|
//! | 0.6 K (p = 2.83 MPa) | +1.3 % |
//! | 5.4 K (p = 3.09 MPa) | -17.4 % |
//! | 11.2 K (p = 3.42 MPa) | -59.5 % |
//! | 21.5 K (p = 4.08 MPa) | -65.5 % |
//! | 31.1 K (p = 4.77 MPa) | -70.2 % |
//!
//! # THE FINDING: this is a solver branch-selection defect, not HEM physics
//!
//! **Do not record "HEM cannot do subcooled Marviken" as a physics result.** It
//! is not one. [`hem_maximum_mass_flux`] evaluates the *textbook* HEM critical
//! flow criterion — maximise `G(p) = rho(p,s_0) * sqrt(2(h_0 - h(p,s_0)))` along
//! the isentrope — using only this crate's public `(p,s)` flash functions and
//! no branch heuristics at all. Measured 2026-08-11 against the *same* case
//! setup and the *same* digitised points:
//!
//! | Test | dispatcher mean abs dev | max-flux criterion mean abs dev | max-flux signed | max-flux max abs |
//! |---|---|---|---|---|
//! | 23 | 12.6 % | **10.1 %** | -10.1 % | 28.2 % |
//! | 24 | **48.6 %** | **9.0 %** | **-1.8 %** | 26.5 % |
//!
//! So plain HEM reproduces Marviken test 24 to a mean 9.0 % with a signed bias
//! of only -1.8 %, well inside the error budget. The 48.6 % deficit belongs
//! entirely to a branch choice inside
//! `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`: for a subcooled
//! stagnation state it computes the two-phase quality at the energy-balance
//! maximum, finds it below 0.03, and therefore returns the **bubble-point sonic
//! kink** value `rho_f * c_2phi` instead of the energy-balance maximum. On the
//! Marviken points that substitutes ~17 Mg/(m^2 s) for the correct ~57
//! Mg/(m^2 s). The escape hatch that would have taken the right branch,
//! `DEEP_SUBCOOLING_RATIO = 5.0`, is not reached: at 33 K subcooling the ratio
//! `v_b / c_2phi` is still under 5, so these points fall inside the
//! "overlap zone" that the solver's own comment block (see
//! `stagnation_point_outside_vle_ph_dome_multiphase.rs`, "KNOWN LIMITATION —
//! the overlap zone") flags as unresolved for want of experimental data.
//!
//! **Marviken is that experimental data, and it comes down on the side of the
//! energy-balance / Bernoulli branch.** `docs/notes.md` already anticipated
//! this: "Whether an HRM is warranted for *real* flashing flow is an open
//! physics question that only experimental data (Marviken) can settle." It is
//! now settled for the subcooled branch, and no HRM is needed for it.
//! Retuning `DEEP_SUBCOOLING_RATIO` is *not* done here — it would be fitting the
//! solver to this dataset, and it risks the Zaloudek near-saturation curves the
//! current threshold protects. It is filed as a bead instead.
//!
//! This is the **second** time in this crate that an apparent "HEM limitation"
//! at the saturated-liquid line has turned out to be a choke-finder branch
//! problem. The first was the near-bubble-point artifact fixed in v0.2.1 by the
//! quality-based routing described above — and that very fix is what produces
//! this one. Marviken's 33 K points exercise exactly that repaired code path.
//! Treat "HEM under-predicts" as a hypothesis to be tested against the bare
//! maximum-flux criterion before it is written down as physics.
//!
//! # Lessons for the higher-fidelity fleet (drift-flux, six-equation two-fluid)
//!
//! Written for the `crates/tampines` effort that reruns these same cases at
//! higher fidelity and compares against this HEM reference.
//!
//! 1. **Run tests 23 and 24, and only those two.** They are the complete set
//!    for the 500 mm / `L/D` = 0.3 nozzle (Table 3:1), which is the shortest and
//!    purest nozzle in the programme — no long discharge pipe to add frictional
//!    pressure loss between the stagnation measurement and the throat. The two
//!    are a matched pair differing in one variable: 3 K vs 33 K nominal
//!    subcooling at the same 4.96 MPa dome pressure and the same hardware.
//! 2. **Use the case setup in the table above verbatim**, and in particular the
//!    two-branch stagnation model. Do not re-derive the inlet temperature; it is
//!    Table 4:2 row 6, 260 degC and 230 degC. (Table 4:2 row 7, "initial
//!    temperature at nozzle inlet", is *not* usable — the scan's OCR truncates
//!    tests 23 and 24 to two digits, `19` and `27`, where every neighbouring
//!    column is three digits. Those values are unreadable, not small.)
//! 3. **Compare mass flux, not mass flow rate.** Fig. 8:24 publishes flux
//!    directly, so the comparison is independent of the throat area and immune
//!    to any error in it. Geometry still matters to *you*: `L/D` = 0.3 is the
//!    whole point.
//! 4. **Tolerance: +/-25 % per point, +/-15 % on the mean, and the dominant
//!    term is experimental scatter (+/-13 to 20 %), not digitisation (6 %) and
//!    not numerics (<0.001 %).** A higher-fidelity model cannot demonstrate
//!    better than about +/-15 % agreement on this dataset no matter how good it
//!    is; do not read a 10 % result as better than a 15 % one.
//! 5. **Nothing here was numerically difficult.** No timestep, no convergence
//!    loop, no initialisation care: these are steady algebraic critical-flow
//!    evaluations at ~70 stagnation states, and the whole file runs in about a
//!    second. The one numerical sensitivity worth knowing is that `G(p)` has a
//!    **kink**, not a smooth maximum, at the bubble point — a golden-section or
//!    Newton search will land on the wrong side of it. [`hem_maximum_mass_flux`]
//!    deliberately uses a uniform scan plus a local refinement for that reason.
//! 6. **Falsifiable expectation for drift-flux / two-fluid.** The HEM reference
//!    established here is *not* biased low in the subcooled regime once the
//!    correct branch is taken (signed mean -1.8 % on test 24). So a
//!    non-equilibrium model should **not** show a large improvement on the
//!    subcooled branch — if it does, suspect its stagnation setup rather than
//!    its physics. Where the equilibrium assumption should visibly bite is the
//!    other end: the **saturated / low-quality branch**, and the **3.0-3.3 MPa
//!    flashing-inception transition** of test 24, where the measured flux climbs
//!    from 17.2 to 37.5 Mg/(m^2 s) between 2.984 and 3.288 MPa (+119 % over
//!    304 kPa) and the HEM maximum-flux criterion over-predicts by
//!    up to +26.5 % (2.90-3.09 MPa) and then under-predicts by up to -18.2 %
//!    (3.12-3.32 MPa) — i.e. it puts the knee at the wrong pressure. Thermal
//!    non-equilibrium delays flashing to below `p_sat`, so a drift-flux or
//!    two-fluid model with interfacial heat transfer should move that knee
//!    *down* in pressure and sharpen it. That is the concrete, falsifiable
//!    prediction to test. Test 23's saturated branch (nearly all of its 29
//!    points) is the second place to look: HEM's signed mean there is +5.9 %,
//!    and `L/D` = 0.3 is exactly the regime where NUREG/CR-2671 §9.2 (p.113 /
//!    PDF 129) reports that "the mass fluxes … with lengths less than 500 mm
//!    increase sharply", a length effect no equilibrium model can express.
//! 7. **Dead ends, one line each.**
//!    - *Constant stagnation enthalpy down the blowdown* — rejected: it reaches
//!      `x ~ 0.04` at the low-pressure end against Table 8:3's `x <= 0.006`, so
//!      it over-flashes the inlet; the saturation-clamped isothermal model above
//!      matches the tabulated quality.
//!    - *Routing through
//!      [`calculate_velocity_mass_flowrate_and_state_in_cd_nozzle`]* (what the
//!      previous revision of this file did) — rejected: that entry point takes
//!      its choke from `get_critical_pressure_ratio_pure_vapour`, an
//!      ideal-gas-like critical pressure ratio for a *pure vapour*, which has no
//!      meaning for a subcooled-water stagnation state. Call the multiphase
//!      dispatcher directly.
//!    - *A hard-coded 33 K subcooling for both tests* (also the previous
//!      revision) — right for test 24, wrong by 30 K for test 23, whose nominal
//!      subcooling is 3 K. The two tests differ by a factor of ~2.3 in measured
//!      flux at the same pressure precisely because of that.
//!    - *Re-tuning `DEEP_SUBCOOLING_RATIO` so test 24 passes* — deliberately not
//!      done: it would be fitting the model to its own validation data, and it
//!      would put the Zaloudek near-saturation curves at risk.

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::millimeter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::{atmosphere, kilopascal, megapascal, pascal};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::ConstZero;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::interfaces::functional_programming::ps_flash_eqm::{h_ps_eqm, v_ps_eqm};
use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;
#[allow(unused_imports)]
use crate::steam_turbine_equations::converging_diverging_nozzles::calculate_velocity_mass_flowrate_and_state_in_cd_nozzle;
use crate::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;

// ─────────────────────────── case constants ────────────────────────────────

/// Nozzle bore of the Marviken test-23/24 nozzle, in millimetres.
///
/// This is the constant-diameter cylindrical test section, i.e. simultaneously
/// the throat and the exit diameter — the nozzle is converging only, with a
/// rounded inlet of radius 250 mm and no diverging section.
/// NUREG/CR-2671 Table 3:1, report p.7 / PDF p.23 (tolerance +/-4 mm).
const NOZZLE_BORE_MM: f64 = 500.0;

/// Length of the Marviken test-23/24 nozzle, in millimetres.
/// NUREG/CR-2671 Table 3:1, report p.7 / PDF p.23. Gives `L/D` = 0.3.
const NOZZLE_LENGTH_MM: f64 = 166.0;

/// Minimum vessel fluid temperature for Marviken test 23, in degrees Celsius —
/// the nozzle-inlet water temperature used to build the stagnation state.
/// NUREG/CR-2671 Table 4:2 row 6, report p.21 / PDF p.37. Corresponds to the
/// 3 K nominal subcooling of Table 4:2 row 5 relative to the 263 degC steam-dome
/// saturation temperature at 4.96 MPa. Category III test.
const TEST_23_WATER_TEMPERATURE_DEGC: f64 = 260.0;

/// Minimum vessel fluid temperature for Marviken test 24, in degrees Celsius.
/// NUREG/CR-2671 Table 4:2 row 6, report p.21 / PDF p.37. Corresponds to the
/// 33 K nominal subcooling of Table 4:2 row 5. Category II test.
const TEST_24_WATER_TEMPERATURE_DEGC: f64 = 230.0;

/// Assumed back pressure, in atmospheres. NUREG/CR-2671 does not state an
/// initial containment pressure for tests 23/24; 1 atm is a lower bound and the
/// choked result is insensitive to it (see the module doc).
const ASSUMED_BACK_PRESSURE_ATM: f64 = 1.0;

/// Per-point acceptance band on `|G_model / G_measured - 1|`, from the error
/// budget in the module doc (experimental scatter +/-13 to 20 % in quadrature
/// with 6.3 % digitisation RMS).
const PER_POINT_TOLERANCE: f64 = 0.25;

/// Acceptance band on the mean of `|G_model / G_measured - 1|` over a test.
const MEAN_TOLERANCE: f64 = 0.15;

// ────────────────────────────── measured data ──────────────────────────────

/// Marviken **test 23** envelope, digitised from NUREG/CR-2671 Fig. 8:24
/// (report p.100 / PDF p.116) with `graphreader`.
///
/// Each entry is `(nozzle inlet stagnation pressure in kPa, measured nozzle
/// mass flux in kg/(m^2 s))`. Reading uncertainty about +/-6 % (RMS 6.3 %
/// against the Table 8:3 endpoints); see the module doc.
///
/// Test 23 is the near-saturated member of the pair: 3 K nominal subcooling,
/// subcooled flow only for the first 4.9 s, so all but the top few points are
/// saturated-liquid stagnation states.
const TEST_23_POINTS: [(f64, f64); 29] = [
    (3724.711, 19501.04),
    (3778.902, 19501.04),
    (3829.48, 19501.04),
    (3887.283, 19209.979),
    (3959.538, 19501.04),
    (4024.566, 19646.57),
    (4075.145, 19792.1),
    (4125.723, 19792.1),
    (4190.751, 19792.1),
    (4248.555, 20228.69),
    (4313.584, 20374.22),
    (4385.838, 20956.341),
    (4443.642, 21101.871),
    (4494.22, 21247.401),
    (4537.572, 21392.931),
    (4580.925, 22120.582),
    (4631.503, 22848.233),
    (4667.63, 23284.823),
    (4696.532, 23721.414),
    (4747.11, 24158.004),
    (4797.688, 28669.439),
    (4812.139, 24594.595),
    (4841.04, 25322.245),
    (4877.168, 31725.572),
    (4891.618, 29397.089),
    (4898.844, 27214.137),
    (4913.295, 25467.775),
    (4942.197, 32744.283),
    (4974.711, 33035.343),
];

/// Marviken **test 24** envelope, digitised from NUREG/CR-2671 Fig. 8:24
/// (report p.100 / PDF p.116) with `graphreader`.
///
/// Each entry is `(nozzle inlet stagnation pressure in kPa, measured nozzle
/// mass flux in kg/(m^2 s))`. Reading uncertainty about +/-6 %, degrading to
/// about +/-15 % across the steep 3.0-3.3 MPa transition; see the module doc.
///
/// Test 24 is the subcooled member of the pair: 33 K nominal subcooling,
/// subcooled flow for the first 21.7 s, so the great majority of these points
/// are subcooled-liquid stagnation states.
const TEST_24_POINTS: [(f64, f64); 40] = [
    (2828.757, 16735.967),
    (2868.497, 17318.087),
    (2904.624, 16881.497),
    (2947.977, 16735.967),
    (2984.104, 17172.557),
    (3027.457, 22266.112),
    (3049.133, 20083.16),
    (3063.584, 21247.401),
    (3085.26, 20519.751),
    (3121.387, 30124.74),
    (3150.289, 26049.896),
    (3164.74, 27214.137),
    (3193.642, 34636.175),
    (3215.318, 33180.873),
    (3273.121, 35509.356),
    (3287.572, 37546.778),
    (3316.474, 39147.609),
    (3352.601, 40020.79),
    (3417.63, 41767.152),
    (3453.757, 40020.79),
    (3576.59, 43513.514),
    (3612.717, 44386.694),
    (3634.393, 43513.514),
    (3706.647, 44386.694),
    (3778.902, 45405.405),
    (3822.254, 44823.285),
    (3901.734, 45405.405),
    (3916.185, 46424.116),
    (3959.538, 46133.056),
    (4060.694, 47442.827),
    (4075.145, 48898.129),
    (4161.85, 51808.732),
    (4255.78, 50935.551),
    (4306.358, 51808.732),
    (4356.936, 52827.443),
    (4421.965, 53700.624),
    (4515.896, 53409.563),
    (4580.925, 54573.805),
    (4703.757, 54137.214),
    (4772.399, 56611.227),
];

/// Envelope endpoints tabulated in NUREG/CR-2671 **Table 8:3**, "Summary of
/// inlet stagnation conditions", report p.74 / PDF p.90, for the
/// 500 mm / `L/D` = 0.3 nozzle.
///
/// `(test label, stagnation pressure in MPa, mass flux in Mg/(m^2 s), endpoint
/// name)`. These are printed numbers in the report, independent of Fig. 8:24,
/// and are used by [`digitised_points_agree_with_nureg_table_8_3`] to measure
/// the digitisation error rather than assume it.
const NUREG_TABLE_8_3_ENDPOINTS: [(&str, f64, f64, &str); 8] = [
    ("23", 4.94, 34.13, "subcooled, x_min"),
    ("23", 4.94, 32.20, "subcooled, x_max"),
    ("23", 4.90, 26.59, "saturated, x = 0"),
    ("23", 3.71, 18.75, "saturated, x_max"),
    ("24", 4.78, 59.35, "subcooled, x_min"),
    ("24", 3.15, 30.57, "subcooled, x_max"),
    ("24", 3.05, 19.00, "saturated, x = 0"),
    ("24", 2.80, 16.71, "saturated, x_max"),
];

// ────────────────────────────── case helpers ───────────────────────────────

/// Nozzle-inlet stagnation specific enthalpy for a Marviken blowdown point.
///
/// This is the single point of truth for the Marviken case setup — the
/// two-branch model justified in the module doc:
///
/// - `Tsat(p) > t_water` — the vessel water is still subcooled, so the
///   stagnation state is subcooled liquid at `(p, t_water)` (IAPWS-IF97
///   Region 1).
/// - `Tsat(p) <= t_water` — the vessel water has flashed and rides the
///   saturation line, so the stagnation state is saturated liquid at `p`
///   (quality `x = 0`).
///
/// # Parameters
/// - `p` — nozzle-inlet stagnation pressure. Valid over the Marviken envelope,
///   2.8 to 5.0 MPa; the branch logic itself is valid anywhere `Tsat` is
///   defined (611.657 Pa to 22.064 MPa).
/// - `t_water` — the vessel's minimum fluid temperature, i.e. the temperature of
///   the subcooled zone feeding the discharge pipe. 260 degC for Marviken test
///   23 and 230 degC for test 24, both from NUREG/CR-2671 Table 4:2 row 6.
///
/// # Returns
/// Stagnation specific enthalpy `h_0` in J/kg (`uom` `AvailableEnergy`), ready
/// to hand to a `(p, h)` critical-flow solver together with `p`.
fn marviken_nozzle_inlet_stagnation_enthalpy(
    p: Pressure,
    t_water: ThermodynamicTemperature,
) -> AvailableEnergy {
    let ref_vol = TampinesSteamTableCV::get_ref_vol();
    let t_sat = TampinesSteamTableCV::try_get_tsat(p)
        .expect("Marviken stagnation pressures are inside the IAPWS-IF97 saturation line");

    if t_sat > t_water {
        // subcooled liquid at the constant vessel water temperature
        TampinesSteamTableCV::new_from_tp_quality_0(t_water, p, ref_vol).get_specific_enthalpy()
    } else {
        // vessel water has flashed: saturated liquid on the saturation line
        TampinesSteamTableCV::new_from_tp_quality_0(t_sat, p, ref_vol).get_specific_enthalpy()
    }
}

/// Bare Homogeneous-Equilibrium-Model critical mass flux, by direct
/// maximisation of the energy-balance flux along the isentrope.
///
/// This is the textbook HEM choking criterion and nothing else:
///
/// `G(p) = rho(p, s_0) * sqrt(2 * (h_0 - h(p, s_0)))`,  `G_crit = max G(p)`
/// over `p` in `[p_back, p_0]`, with `s_0 = s(p_0, h_0)`.
///
/// It is built only from this crate's public `(p,h)` and `(p,s)` flash
/// functions, with **no region routing, no branch heuristics and no
/// discriminators**. Its purpose is to separate "what the HEM model says" from
/// "what this crate's dispatcher returns", which the Marviken results show are
/// not the same thing in the subcooled regime — see the module doc.
///
/// `G(p)` has a *kink*, not a smooth stationary point, at the bubble point, so
/// this deliberately uses a uniform scan followed by a local refinement rather
/// than a golden-section or Newton search, either of which can land on the wrong
/// side of the kink. Measured resolution sensitivity: `n_steps = 1000` agrees
/// with `n_steps = 20000` to better than 1e-5 relative.
///
/// # Parameters
/// - `p0` — stagnation pressure.
/// - `h0` — stagnation specific enthalpy (J/kg).
/// - `p_back` — receiver pressure; the scan's lower bound.
/// - `n_steps` — number of uniform scan intervals. 1000 is ample here.
///
/// # Returns
/// The critical (maximum) mass flux in kg/(m^2 s) as a `uom` `MassFlux`.
fn hem_maximum_mass_flux(
    p0: Pressure,
    h0: AvailableEnergy,
    p_back: Pressure,
    n_steps: usize,
) -> MassFlux {
    let s0 = s_ph_eqm(p0, h0);

    let g_of_p = |p_pa: f64| -> f64 {
        let p = Pressure::new::<pascal>(p_pa);
        let kinetic_energy = h0 - h_ps_eqm(p, s0);
        if kinetic_energy < AvailableEnergy::ZERO {
            return 0.0; // over-expanded guard
        }
        let rho = v_ps_eqm(p, s0).recip();
        (rho * (2.0 * kinetic_energy).sqrt()).get::<kilogram_per_square_meter_second>()
    };

    let lower = p_back.get::<pascal>();
    let upper = p0.get::<pascal>();
    let mut best_g = 0.0_f64;
    let mut best_p = upper;
    for i in 0..=n_steps {
        let p_pa = lower + (upper - lower) * (i as f64) / (n_steps as f64);
        let g = g_of_p(p_pa);
        if g > best_g {
            best_g = g;
            best_p = p_pa;
        }
    }

    // local refinement over one coarse interval either side of the peak
    let step = (upper - lower) / (n_steps as f64);
    let refine_lo = (best_p - step).max(lower);
    let refine_hi = (best_p + step).min(upper);
    for i in 0..=200 {
        let p_pa = refine_lo + (refine_hi - refine_lo) * (i as f64) / 200.0;
        let g = g_of_p(p_pa);
        if g > best_g {
            best_g = g;
        }
    }

    MassFlux::new::<kilogram_per_square_meter_second>(best_g)
}

/// Summary statistics of a set of signed relative deviations.
///
/// Returns `(mean absolute deviation, mean signed deviation, maximum absolute
/// deviation)`, all as plain fractions (0.25 = 25 %). Panics on an empty slice,
/// which would mean the test data failed to load.
fn deviation_stats(deviations: &[f64]) -> (f64, f64, f64) {
    assert!(
        !deviations.is_empty(),
        "no deviations collected — the Marviken point set is empty"
    );
    let n = deviations.len() as f64;
    let mean_abs = deviations.iter().map(|d| d.abs()).sum::<f64>() / n;
    let mean_signed = deviations.iter().sum::<f64>() / n;
    let max_abs = deviations.iter().fold(0.0_f64, |acc, d| acc.max(d.abs()));
    (mean_abs, mean_signed, max_abs)
}

/// Runs the crate's HEM dispatcher over one Marviken point set and returns the
/// signed relative deviations `G_HEM / G_measured - 1`, printing a per-point
/// table as it goes.
fn dispatcher_deviations(label: &str, points: &[(f64, f64)], t_water_degc: f64) -> Vec<f64> {
    let t_water = ThermodynamicTemperature::new::<degree_celsius>(t_water_degc);
    println!("\n--- Marviken test {label}: HEM dispatcher, T_water = {t_water_degc} degC ---");
    println!("   p [kPa]   dTsub [K]    h0 [kJ/kg]     G_HEM      G_meas    dev [%]");

    let mut deviations = Vec::with_capacity(points.len());
    let mut lowest_choke_pressure = Pressure::new::<megapascal>(1.0e3);
    for (p_kpa, g_measured) in points.iter() {
        let p = Pressure::new::<kilopascal>(*p_kpa);
        let h0 = marviken_nozzle_inlet_stagnation_enthalpy(p, t_water);
        let (p_crit, g_hem) = get_critical_pressure_and_mass_flux_multiphase_ph(p, h0);
        if p_crit < lowest_choke_pressure {
            lowest_choke_pressure = p_crit;
        }

        let g_hem_si = g_hem.get::<kilogram_per_square_meter_second>();
        let deviation = g_hem_si / g_measured - 1.0;
        deviations.push(deviation);

        let t_sat = TampinesSteamTableCV::try_get_tsat(p).unwrap();
        let local_subcooling = (t_sat.get::<degree_celsius>() - t_water_degc).max(0.0);
        println!(
            "  {:8.1}  {:8.2}   {:9.2}   {:9.1} {:11.1}   {:+7.1}",
            p_kpa,
            local_subcooling,
            h0.get::<kilojoule_per_kilogram>(),
            g_hem_si,
            g_measured,
            100.0 * deviation
        );
    }

    // The nozzle is choked at every point by a wide margin, which is why the
    // assumed 1 atm back pressure does not enter the result. Assert it rather
    // than assume it: the peak containment pressure was 328 kPa (Table 8:4).
    println!(
        "  lowest choke pressure over the sweep: {:.3} MPa (peak containment pressure \
         was 0.328 MPa)",
        lowest_choke_pressure.get::<megapascal>()
    );
    assert!(
        lowest_choke_pressure > Pressure::new::<megapascal>(1.0),
        "Marviken test {label}: choke pressure fell to {:.3} MPa, close enough to the \
         containment pressure that the back-pressure assumption stops being immaterial",
        lowest_choke_pressure.get::<megapascal>()
    );

    deviations
}

// ──────────────────────────────── the tests ────────────────────────────────

/// **Data-provenance gate** — checks the digitised Fig. 8:24 points against the
/// independently printed endpoint values of NUREG/CR-2671 Table 8:3.
///
/// # Methodology
///
/// Table 8:3 (report p.74 / PDF p.90) tabulates the nozzle mass flux at four
/// envelope endpoints per test — the subcooled `x_min` and `x_max` points and
/// the saturated `x = 0` and `x_max` points — for the same 500 mm / `L/D` = 0.3
/// nozzle plotted in Fig. 8:24. Those numbers are typeset in the report, so they
/// carry no graph-reading error. For each of the eight, this test picks the
/// digitised point nearest in pressure and forms the relative flux difference.
/// The result *is* this file's digitisation-uncertainty estimate; nothing else
/// in the file assumes a reading error, it is measured here.
///
/// Pass criterion: RMS deviation `<= 8 %`, worst-case deviation `<= 16 %`. The
/// looser worst-case allowance exists because one endpoint (test 24 subcooled
/// `x_max`, 3.15 MPa) lies on the near-vertical flashing-transition part of the
/// curve, where a pressure reading error of a few tens of kPa maps to a large
/// flux error.
///
/// # Results — measured 2026-08-11, v0.2.5, release mode
///
/// **RMS 6.3 %, worst 14.8 %.** Per endpoint: test 23 gave -4.1 %, +1.7 %,
/// +2.3 %, +4.0 %; test 24 gave -4.6 %, **-14.8 %** (the 3.15 MPa transition
/// point), +5.7 %, +0.2 %. Seven of eight are within 5.7 %. Interpretation: the
/// digitisation is faithful to about +/-6 % over most of the envelope and to
/// about +/-15 % through the 3.0-3.3 MPa transition, and it is *not* the
/// dominant term in the error budget — experimental scatter is.
#[test]
fn digitised_points_agree_with_nureg_table_8_3() {
    let mut sum_squares = 0.0;
    let mut worst = 0.0_f64;

    println!("\n--- digitised Fig. 8:24 vs NUREG/CR-2671 Table 8:3 endpoints ---");
    for (test, p_ref_mpa, g_ref_mg, endpoint) in NUREG_TABLE_8_3_ENDPOINTS.iter() {
        let points: &[(f64, f64)] = if *test == "23" {
            &TEST_23_POINTS
        } else {
            &TEST_24_POINTS
        };

        let nearest = points
            .iter()
            .copied()
            .reduce(|a, b| {
                if (b.0 / 1000.0 - p_ref_mpa).abs() < (a.0 / 1000.0 - p_ref_mpa).abs() {
                    b
                } else {
                    a
                }
            })
            .expect("Marviken point set is non-empty");

        let g_ref = g_ref_mg * 1000.0;
        let deviation = nearest.1 / g_ref - 1.0;
        sum_squares += deviation * deviation;
        worst = worst.max(deviation.abs());

        println!(
            "  test {test} {endpoint:18}  ref {:.2} MPa / {:8.0}  vs digitised {:.3} MPa / {:8.0}  dev {:+6.1} %",
            p_ref_mpa,
            g_ref,
            nearest.0 / 1000.0,
            nearest.1,
            100.0 * deviation
        );
    }

    let rms = (sum_squares / NUREG_TABLE_8_3_ENDPOINTS.len() as f64).sqrt();
    println!(
        "  digitisation RMS = {:.1} %, worst = {:.1} %",
        100.0 * rms,
        100.0 * worst
    );

    assert!(
        rms <= 0.08,
        "digitisation RMS error {:.1} % exceeds the 8 % provenance gate",
        100.0 * rms
    );
    assert!(
        worst <= 0.16,
        "worst digitisation error {:.1} % exceeds the 16 % provenance gate",
        100.0 * worst
    );
}

/// **Measured experimental scatter** in the digitised Fig. 8:24 envelopes — the
/// dominant term in this file's error budget.
///
/// # Methodology
///
/// For each digitised point, gather every other point whose stagnation pressure
/// lies within +/-60 kPa (roughly one plot tick width) and form the half-spread
/// `(G_max - G_min) / (G_max + G_min)` of that neighbourhood, requiring at least
/// three members. The largest such half-spread over the series is reported. This
/// captures the combined effect of genuine run-to-run and time-to-time
/// variability, the superposition of subcooled and saturated branches in a
/// single envelope figure, and graph-reading error — that is, everything that
/// makes a single-valued `G(p)` model unable to hit every point.
///
/// Pass criterion: the measured scatter must be non-trivial (`>= 5 %`, i.e. the
/// budget's dominant term is real) and must not exceed the `PER_POINT_TOLERANCE`
/// of 25 % that it justifies. This is a bookkeeping gate on the error budget,
/// not a physics test.
///
/// # Results — measured 2026-08-11, v0.2.5, release mode
///
/// **Test 23: +/-12.9 % worst half-spread, near p = 4942 kPa.
/// Test 24: +/-20.0 % worst half-spread, near p = 3064 kPa.** Both maxima fall
/// where expected — test 23's at the top of its envelope where the subcooled and
/// saturated branches overlap, test 24's inside the 3.0-3.3 MPa flashing
/// transition. Interpretation: no model, at any fidelity, can agree with this
/// dataset to better than roughly +/-13 to 20 % per point.
#[test]
fn marviken_measured_scatter() {
    println!("\n--- measured scatter of the digitised envelopes (+/-60 kPa windows) ---");
    for (label, points) in [("23", &TEST_23_POINTS[..]), ("24", &TEST_24_POINTS[..])] {
        let mut worst_half_spread = 0.0_f64;
        let mut worst_pressure = 0.0;

        for centre in points.iter() {
            let window: Vec<f64> = points
                .iter()
                .filter(|q| (q.0 - centre.0).abs() <= 60.0)
                .map(|q| q.1)
                .collect();
            if window.len() < 3 {
                continue;
            }
            let lo = window.iter().cloned().fold(f64::MAX, f64::min);
            let hi = window.iter().cloned().fold(0.0_f64, f64::max);
            let half_spread = (hi - lo) / (hi + lo);
            if half_spread > worst_half_spread {
                worst_half_spread = half_spread;
                worst_pressure = centre.0;
            }
        }

        println!(
            "  test {label}: worst half-spread +/-{:.1} % near p = {:.0} kPa",
            100.0 * worst_half_spread,
            worst_pressure
        );

        assert!(
            worst_half_spread >= 0.05,
            "test {label} scatter {:.1} % is implausibly small — check the digitised data loaded",
            100.0 * worst_half_spread
        );
        assert!(
            worst_half_spread <= PER_POINT_TOLERANCE,
            "test {label} scatter {:.1} % exceeds the {:.0} % per-point tolerance it is meant to justify",
            100.0 * worst_half_spread,
            100.0 * PER_POINT_TOLERANCE
        );
    }
}

/// **VALIDATION** of the HEM critical-flow dispatcher against Marviken
/// **test 23** (near-saturated blowdown, 3 K nominal subcooling).
///
/// # Methodology
///
/// - **Model under test:** [`get_critical_pressure_and_mass_flux_multiphase_ph`],
///   this crate's public Homogeneous-Equilibrium-Model critical-flow dispatcher,
///   called once per digitised blowdown point with the stagnation state
///   `(p, h_0)` built by [`marviken_nozzle_inlet_stagnation_enthalpy`].
/// - **Reference:** the 29 points of [`TEST_23_POINTS`], digitised from
///   NUREG/CR-2671 Fig. 8:24, "The test envelope for the nozzle with diameter
///   500 mm and L/D = 0.3: nozzle mass flux and inlet stagnation pressure",
///   report p.100 / PDF p.116.
/// - **Inputs:** 500 mm bore / 166 mm long converging nozzle, `L/D` = 0.3,
///   rounded 250 mm-radius inlet (Table 3:1, p.7 / PDF 23); initial steam-dome
///   pressure 4.96 MPa and saturation temperature 263 degC; vessel water at
///   `T_w` = 260 degC, i.e. 3 K nominal subcooling (Table 4:2 rows 3-6, p.21 /
///   PDF 37); assumed back pressure 1 atm (immaterial — every choke pressure is
///   above 2.7 MPa). Properties from IAPWS-IF97 as implemented in this crate.
/// - **Comparison quantity:** mass flux, so the result is independent of the
///   throat area.
/// - **Pass criterion:** mean `|G_HEM/G_meas - 1| <= 15 %` and per-point
///   `|G_HEM/G_meas - 1| <= 25 %`, from the error budget in the module doc,
///   whose dominant term is the +/-12.9 % experimental scatter measured by
///   [`marviken_measured_scatter`].
///
/// # Results — measured 2026-08-11, tampines-steam-tables v0.2.5, release mode
///
/// **Mean absolute deviation 12.6 %; mean signed deviation +5.9 %; maximum
/// absolute deviation 23.1 %; 29 of 29 points inside the 25 % band. PASS.**
///
/// Interpretation: for a near-saturated stagnation state the HEM dispatcher
/// reproduces the Marviken 500 mm / `L/D` = 0.3 envelope within the
/// experimental uncertainty of the data, with a small positive bias (+5.9 %) and
/// no systematic trend against pressure. The worst three points (-19.9 %,
/// -22.4 %, -23.1 % at 4.877, 4.942 and 4.975 MPa) are the top-of-envelope
/// outliers that also lie +/-12.9 % from their own immediate neighbours, so the
/// residual is dominated by the measurement rather than by the model. The margin
/// is real but not large — the worst point uses 92 % of the allowed band — so
/// this is a pass, not a demonstration of accuracy well beyond the data.
///
/// **Caveat, per `RESPONSIBLE_USE.md`:** this is an AI-assisted result and has
/// not had human V&V sign-off. See the crate README "Bookkeeping status".
#[test]
fn validate_against_marviken_test_23() {
    let deviations = dispatcher_deviations("23", &TEST_23_POINTS, TEST_23_WATER_TEMPERATURE_DEGC);
    let (mean_abs, mean_signed, max_abs) = deviation_stats(&deviations);

    println!(
        "\n  test 23 SUMMARY: mean|dev| = {:.1} %, mean dev = {:+.1} %, max|dev| = {:.1} % (n = {})",
        100.0 * mean_abs,
        100.0 * mean_signed,
        100.0 * max_abs,
        deviations.len()
    );

    assert!(
        mean_abs <= MEAN_TOLERANCE,
        "Marviken test 23: mean absolute deviation {:.1} % exceeds the {:.0} % budget",
        100.0 * mean_abs,
        100.0 * MEAN_TOLERANCE
    );
    assert!(
        max_abs <= PER_POINT_TOLERANCE,
        "Marviken test 23: worst point deviates {:.1} %, beyond the {:.0} % budget",
        100.0 * max_abs,
        100.0 * PER_POINT_TOLERANCE
    );
}

/// **CHARACTERISATION — NOT A VALIDATION.** The HEM critical-flow dispatcher
/// against Marviken **test 24** (33 K subcooled blowdown), which it
/// **fails** by a factor of about three on the error budget.
///
/// Read the verdict before the numbers: **Marviken validation of the HEM
/// dispatcher is NOT achieved for subcooled stagnation states.** This test
/// exists to pin down *how far* it misses and *where*, so the miss cannot
/// silently change; the assertions below bracket the measured deviation
/// envelope, they do not express agreement. Nothing in this crate may be
/// described as "Marviken-validated" on the strength of it.
///
/// # Methodology
///
/// Identical to [`validate_against_marviken_test_23`] except for the case
/// parameter: 40 points from [`TEST_24_POINTS`] (same Fig. 8:24, same nozzle,
/// same 4.96 MPa dome pressure), with the vessel water at `T_w` = 230 degC, i.e.
/// **33 K** nominal subcooling (NUREG/CR-2671 Table 4:2 rows 5-6, p.21 /
/// PDF 37 — the pairing of test 24 with 33 K, and test 23 with 3 K, is the
/// single most important fact to get right here). The same 15 % / 25 % budget
/// applies and is *not* met.
///
/// # Results — measured 2026-08-11, tampines-steam-tables v0.2.5, release mode
///
/// **Mean absolute deviation 48.6 %; mean signed deviation -48.5 %; maximum
/// absolute deviation 70.2 %; 31 of 40 points outside the 25 % band. FAIL
/// against the validation budget — this is a characterisation.**
///
/// The failure is systematic, not scattered: the dispatcher returns an almost
/// pressure-independent 16.85 to 16.95 Mg/(m^2 s) over the whole 2.83-4.77 MPa
/// sweep while the measurement rises from 16.7 to 56.6 Mg/(m^2 s). The
/// deviation tracks the *local* subcooling `Tsat(p) - T_w` monotonically:
/// +1.3 % at 0.6 K, -17.4 % at 5.4 K, -59.5 % at 11.2 K, -65.5 % at 21.5 K,
/// -70.2 % at 31.1 K. Binned by pressure (13 points each), the lowest-pressure
/// third averages -18.9 % and the highest-pressure third -67.1 %.
///
/// # Interpretation — a solver branch defect, not an HEM limitation
///
/// The obvious reading, "HEM's equilibrium assumption under-predicts subcooled
/// critical flow through a short nozzle", is **wrong here**, and this crate has
/// made that exact mistake once before (the near-bubble-point artifact
/// misdiagnosed as needing an HRM, actually fixed numerically in v0.2.1).
/// [`characterise_hem_maximum_flux_criterion`] evaluates the bare HEM
/// maximum-mass-flux criterion on this identical case and reaches a mean
/// absolute deviation of 9.0 % with a signed bias of -1.8 % — inside the budget.
/// The whole 48.6 % deficit comes from
/// `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` taking its
/// bubble-point sonic-kink branch (`rho_f * c_2phi`) rather than the
/// energy-balance maximum, because the two-phase quality at the energy maximum
/// is below 0.03 and the `DEEP_SUBCOOLING_RATIO = 5.0` escape is not reached at
/// 33 K subcooling. These points sit squarely in the "overlap zone" that the
/// solver's own comment block flags as unresolved for want of experimental
/// data. Marviken is that data. Fixing it is a separate change, filed as a bead,
/// and deliberately not done here — retuning the threshold against this dataset
/// would be fitting the model to its own validation case.
///
/// **Caveat, per `RESPONSIBLE_USE.md`:** AI-assisted result, no human V&V
/// sign-off.
#[test]
fn validate_against_marviken_test_24() {
    println!(
        "\n  *** Marviken test 24 is a CHARACTERISATION of a known HEM-dispatcher \
         deficit, not a validation. See the doc comment. ***"
    );

    let deviations = dispatcher_deviations("24", &TEST_24_POINTS, TEST_24_WATER_TEMPERATURE_DEGC);
    let (mean_abs, mean_signed, max_abs) = deviation_stats(&deviations);

    let outside_budget = deviations
        .iter()
        .filter(|d| d.abs() > PER_POINT_TOLERANCE)
        .count();

    // pressure-binned trend: lowest third vs highest third of the sweep
    let third = deviations.len() / 3;
    let low_pressure_mean = deviations[..third].iter().sum::<f64>() / third as f64;
    let high_pressure_mean =
        deviations[deviations.len() - third..].iter().sum::<f64>() / third as f64;

    println!(
        "\n  test 24 SUMMARY: mean|dev| = {:.1} %, mean dev = {:+.1} %, max|dev| = {:.1} % \
         (n = {}, {} outside the {:.0} % band)",
        100.0 * mean_abs,
        100.0 * mean_signed,
        100.0 * max_abs,
        deviations.len(),
        outside_budget,
        100.0 * PER_POINT_TOLERANCE
    );
    println!(
        "  trend: lowest-pressure third mean dev {:+.1} %, highest-pressure third {:+.1} %",
        100.0 * low_pressure_mean,
        100.0 * high_pressure_mean
    );

    // The validation budget is NOT met. Record that fact as an assertion so it
    // cannot quietly become a pass without this doc comment being revisited.
    assert!(
        mean_abs > MEAN_TOLERANCE,
        "Marviken test 24 now meets the {:.0} % validation budget (mean|dev| = {:.1} %). \
         That is good news, but this test and the crate README are written as a \
         characterisation of a failure — update both before relaxing this assertion.",
        100.0 * MEAN_TOLERANCE,
        100.0 * mean_abs
    );

    // Bracket the measured deviation envelope so any change in the deficit shows up.
    assert!(
        (0.43..=0.55).contains(&mean_abs),
        "Marviken test 24 mean absolute deviation {:.1} % has moved outside the \
         characterised 43-55 % envelope",
        100.0 * mean_abs
    );
    assert!(
        (-0.55..=-0.42).contains(&mean_signed),
        "Marviken test 24 mean signed deviation {:+.1} % has moved outside the \
         characterised -55 to -42 % envelope",
        100.0 * mean_signed
    );
    assert!(
        (0.65..=0.76).contains(&max_abs),
        "Marviken test 24 worst deviation {:.1} % has moved outside the characterised \
         65-76 % envelope",
        100.0 * max_abs
    );

    // The deficit is systematic and one-signed: HEM as dispatched under-predicts.
    assert!(
        mean_signed < 0.0 && low_pressure_mean > high_pressure_mean,
        "Marviken test 24 no longer shows the characterised monotone under-prediction \
         with increasing subcooling (low-p third {:+.1} %, high-p third {:+.1} %)",
        100.0 * low_pressure_mean,
        100.0 * high_pressure_mean
    );
}

/// **CHARACTERISATION** — the bare HEM maximum-mass-flux criterion against both
/// Marviken tests, isolating the model from this crate's dispatcher.
///
/// # Methodology
///
/// - **Model under test:** [`hem_maximum_mass_flux`] — `G_crit = max_p rho(p,s_0)
///   * sqrt(2(h_0 - h(p,s_0)))` along the isentrope from the stagnation state,
///   built from public `(p,s)` flash functions with no region routing and no
///   branch discriminators. This is HEM as textbooks state it.
/// - **Reference, inputs and comparison quantity:** identical to
///   [`validate_against_marviken_test_23`] and
///   [`validate_against_marviken_test_24`] — same digitised points, same
///   stagnation model, same nozzle, same tests.
/// - **Purpose:** answer the question "is the test-24 deficit HEM physics or
///   this crate's solver?" It is a characterisation, not a validation gate:
///   `hem_maximum_mass_flux` is a test-local reference implementation, not part
///   of the crate's public API.
/// - **Pass criterion:** mean `|dev| <= 15 %` for both tests (the aggregate
///   budget). The per-point 25 % band is deliberately *not* asserted, because a
///   handful of points in the steep flashing transition exceed it — see below.
///
/// # Results — measured 2026-08-11, tampines-steam-tables v0.2.5, release mode
///
/// | Test | mean abs dev | mean signed dev | max abs dev |
/// |---|---|---|---|
/// | 23 | **10.1 %** | -10.1 % | 28.2 % |
/// | 24 | **9.0 %** | **-1.8 %** | 26.5 % |
///
/// Interpretation, and the main scientific result of this file: **plain HEM
/// reproduces both Marviken envelopes to a mean of about 9-10 %, inside the
/// experimental uncertainty of the data.** In particular the 33 K subcooled test
/// 24 comes out essentially unbiased (-1.8 %), against the dispatcher's -48.5 %.
/// The equilibrium assumption is therefore *not* what fails on subcooled
/// Marviken flow; a branch choice in
/// `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` is.
///
/// Where HEM does visibly struggle is the flashing-inception knee of test 24: it
/// over-predicts by up to +26.5 % at 2.90-3.09 MPa and then under-predicts by up
/// to -18.2 % at 3.12-3.32 MPa, i.e. it places the knee at too high a pressure
/// and makes it too gradual. That is the equilibrium assumption biting, and it is
/// the concrete target for a non-equilibrium drift-flux or two-fluid model.
/// Test 23's saturated branch carries a uniform -10.1 % bias, consistent with
/// NUREG/CR-2671 §9.2 (p.113 / PDF 129) reporting that measured fluxes "increase
/// sharply" for nozzles shorter than 500 mm — a length effect no equilibrium
/// model contains.
///
/// **Caveat, per `RESPONSIBLE_USE.md`:** AI-assisted result, no human V&V
/// sign-off.
#[test]
fn characterise_hem_maximum_flux_criterion() {
    let p_back = Pressure::new::<atmosphere>(ASSUMED_BACK_PRESSURE_ATM);

    for (label, points, t_water_degc) in [
        ("23", &TEST_23_POINTS[..], TEST_23_WATER_TEMPERATURE_DEGC),
        ("24", &TEST_24_POINTS[..], TEST_24_WATER_TEMPERATURE_DEGC),
    ] {
        let t_water = ThermodynamicTemperature::new::<degree_celsius>(t_water_degc);
        println!(
            "\n--- Marviken test {label}: bare HEM maximum-flux criterion, \
             T_water = {t_water_degc} degC ---"
        );
        println!("   p [kPa]     G_HEMmax      G_meas    dev [%]");

        let mut deviations = Vec::with_capacity(points.len());
        for (p_kpa, g_measured) in points.iter() {
            let p = Pressure::new::<kilopascal>(*p_kpa);
            let h0 = marviken_nozzle_inlet_stagnation_enthalpy(p, t_water);
            let g_hem = hem_maximum_mass_flux(p, h0, p_back, 1000)
                .get::<kilogram_per_square_meter_second>();
            let deviation = g_hem / g_measured - 1.0;
            deviations.push(deviation);
            println!(
                "  {:8.1}   {:10.1} {:11.1}   {:+7.1}",
                p_kpa,
                g_hem,
                g_measured,
                100.0 * deviation
            );
        }

        let (mean_abs, mean_signed, max_abs) = deviation_stats(&deviations);
        println!(
            "\n  test {label} bare-HEM SUMMARY: mean|dev| = {:.1} %, mean dev = {:+.1} %, \
             max|dev| = {:.1} %",
            100.0 * mean_abs,
            100.0 * mean_signed,
            100.0 * max_abs
        );

        assert!(
            mean_abs <= MEAN_TOLERANCE,
            "bare HEM maximum-flux criterion, Marviken test {label}: mean absolute \
             deviation {:.1} % exceeds the {:.0} % budget",
            100.0 * mean_abs,
            100.0 * MEAN_TOLERANCE
        );
    }
}

/// Documents the Marviken test-23/24 nozzle geometry as machine-checked
/// constants, and confirms the `L/D` = 0.3 of NUREG/CR-2671 Table 3:1.
///
/// # Methodology and results — measured 2026-08-11
///
/// The nozzle is a rounded-inlet, constant-bore, **converging-only** passage:
/// bore `D` = 500 mm (= throat = exit), length `L` = 166 mm, inlet radius of
/// curvature 250 mm (Table 3:1, report p.7 / PDF p.23; tolerance +/-4 mm).
/// `L/D` = 166/500 = **0.332**, which the report rounds to 0.3. This test
/// asserts that ratio to +/-0.04 and reports the throat area (0.19635 m^2) that
/// a mass-flow — as opposed to mass-flux — comparison would need. Fig. 8:24
/// publishes mass flux, so the tests above never use the area; it is recorded
/// here for the higher-fidelity models that will. The test also checks that this
/// crate's IAPWS-IF97 `Tsat(4.96 MPa)` reproduces the report's 263 degC
/// steam-dome saturation temperature (Table 4:2 row 4) to within 1 K.
#[test]
fn marviken_nozzle_geometry() {
    let bore = Length::new::<millimeter>(NOZZLE_BORE_MM);
    let length = Length::new::<millimeter>(NOZZLE_LENGTH_MM);
    let length_over_diameter = (length / bore).get::<uom::si::ratio::ratio>();
    let throat_area: Area = std::f64::consts::PI * bore * bore / 4.0;

    println!(
        "\n  Marviken 500 mm nozzle: D = {:.0} mm, L = {:.0} mm, L/D = {:.3}, A_throat = {:.5} m^2",
        NOZZLE_BORE_MM,
        NOZZLE_LENGTH_MM,
        length_over_diameter,
        throat_area.get::<uom::si::area::square_meter>()
    );

    assert!(
        (length_over_diameter - 0.3).abs() <= 0.04,
        "L/D = {length_over_diameter:.3} does not match the 0.3 of NUREG/CR-2671 Table 3:1"
    );

    let dome_pressure = Pressure::new::<megapascal>(4.96);
    let t_sat = TampinesSteamTableCV::try_get_tsat(dome_pressure)
        .expect("4.96 MPa is a valid saturation pressure");
    println!(
        "  Tsat(4.96 MPa) = {:.1} degC (NUREG/CR-2671 Table 4:2 row 4 gives 263 degC)",
        t_sat.get::<degree_celsius>()
    );
    assert!(
        (t_sat.get::<degree_celsius>() - 263.0).abs() <= 1.0,
        "IAPWS-IF97 Tsat(4.96 MPa) = {:.1} degC disagrees with the report's 263 degC",
        t_sat.get::<degree_celsius>()
    );
}
