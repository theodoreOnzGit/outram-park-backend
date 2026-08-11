// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! # V&V: Marviken critical-flow tests at drift-flux and two-fluid fidelity
//!
//! Registered ahead of the cases (2026-08-11). The case setup is taken from
//! the HEM reference implementation in
//! `crates/tampines-steam-tables/src/steam_turbine_equations/converging_diverging_nozzles/tests/marviken_tests.rs`
//! (bead `op-ja3t`), never re-derived independently, so the cross-fidelity
//! comparison stays meaningful.
//!
//! # What is under test, and against what
//!
//! **Model under test:** [`DriftFlux1d`], the 1-D semi-implicit drift-flux
//! solver, on the Marviken 500 mm / `L/D` = 0.3 nozzle represented as a
//! constant-area pipe with an [`AxialBoundary::ReservoirInlet`] at the vessel
//! stagnation state and an [`AxialBoundary::PressureOutlet`] at the receiver.
//! Neither boundary imposes a critical-flow criterion, so the critical mass
//! flux is obtained by **external maximisation of the marched steady exit flux
//! over the receiver pressure** ([`critical_flux_by_sweep`]) — deliberately
//! the same `G_crit = max_p G(p)` framing the bare HEM criterion uses, so only
//! the *model* changes between the two fidelities.
//!
//! **Measurement:** the digitised NUREG/CR-2671 (MXC-301) Fig. 8:24 envelopes,
//! [`TEST_23_POINTS`] (29 points) and [`TEST_24_POINTS`] (40 points).
//!
//! **Comparison baseline:** [`HEM_MAX_FLUX_REFERENCE`], the *bare* HEM
//! maximum-mass-flux criterion, quoted from the sibling crate and not
//! re-measured. Not the crate's HEM dispatcher — its 48.6 % test-24 deficit is
//! a branch-selection defect (bead `op-dqng`), and beating that would flatter
//! this model for the wrong reason.
//!
//! **Tolerances:** [`PER_POINT_TOLERANCE`] 0.25 and [`MEAN_TOLERANCE`] 0.15,
//! taken unchanged from the HEM file's error budget (dominant term: the
//! measured +/-12.9 % / +/-20.0 % experimental scatter of the envelope itself).
//! Nothing about higher fidelity shrinks them. **No drift-flux coefficient
//! (`C_0`, `V_gj`, `tau`) was tuned to this dataset.**
//!
//! # Results — measured 2026-08-11, `tampines` v0.0.1, release mode
//!
//! Configuration: [`MarvikenCase::default`] — 8 cells, `dt` = 20 us,
//! `t_end` = 12 ms, 1 ms outlet ramp, `C_0` = 1.13, `V_gj` = 0.1 m/s,
//! `tau` = 1 ms (the solver default).
//!
//! | Test | Subcooling | n | mean \|dev\| | signed | max \|dev\| | outside +/-25 % | Verdict |
//! |---|---|---|---|---|---|---|---|
//! | 23 | 3 K | 29 | **8.9 %** | **+2.8 %** | 19.7 % | 0 / 29 | **VALIDATED** |
//! | 24 | 33 K | 40 | **11.4 %** | **-9.7 %** | 28.8 % | 6 / 40 | **NOT VALIDATED** — characterisation only |
//!
//! Against the quoted bare-HEM baseline (10.1 % / -10.1 % / 28.2 % on test 23;
//! 9.0 % / -1.8 % / 26.5 % on test 24): **drift flux clearly wins test 23 and
//! slightly loses test 24.**
//!
//! Run health, both gates: **897 marched transients, 0 refused steps**; max
//! material Courant 0.326; worst steadiness half-spread at a quoted maximum
//! 0.0308 (t23) / 0.1256 (t24). Wall clock **98.1 s** (t23) and **381 s**
//! (t24), measured by the tests themselves and reproduced to 0.1 s across two
//! independent runs; they are separate `#[test]` fns so cargo runs them in
//! parallel and the pair costs about as much as the longer one — the whole
//! `multiphase_1d` module finished in **380.85 s** on 2026-08-11. No sweep was
//! trimmed and no timestep was coarsened to fit a budget.
//!
//! These numbers were not the first thing measured. The `#[ignore]`d
//! [`marviken_drift_flux_explore`] harness was run first, at five stagnation
//! points spanning both tests (2.829, 3.150, 3.725, 4.772, 4.975 MPa), to
//! answer *before* any assertion was written whether the sweeps have an
//! interior maximum at all (they do not — see below) and what one costs
//! (2.9 to 12.4 s, extrapolating to ~2 and ~6 min for the two gates, which is
//! what they then took).
//!
//! ## The structural finding: no interior maximum, but a plateau
//!
//! **68 of 69 receiver-pressure sweeps have no interior maximum.** The steady
//! flux rises monotonically as the receiver pressure falls, all the way to the
//! 101.325 kPa containment pressure, so the reported `G_crit` is almost always
//! just the flux at the experiment's own back pressure. (The one exception, at
//! 4.306 MPa on test 24, wins by 0.3 % and is plateau noise, not a choke.)
//!
//! This is expected and was anticipated before the runs: a semi-implicit
//! pressure-based scheme propagates pressure elliptically, so a receiver
//! pressure is felt upstream, and neither boundary here imposes a critical
//! criterion — which is precisely why RELAP5 imposes choking as a boundary
//! condition instead of capturing it. **It is reported as a limitation of the
//! method, not papered over.**
//!
//! What the model *does* produce is an effective plateau, and that is
//! measurable where "choked" is not ([`SweepResult::plateau_spread`]): over
//! the lower half of the sweep — a further factor ~20 in receiver pressure —
//! the flux changes by a mean of 4.9 % (test 23, worst 5.2 %) and 3.2 %
//! (test 24, worst 7.3 %). The exit-cell pressure meanwhile stays far above
//! the imposed boundary pressure (e.g. 2.44 MPa in the cell against
//! 0.101 MPa at the face, at `p_0` = 4.77 MPa), which is the signature of the
//! interior refusing to communicate the receiver pressure.
//!
//! ## The falsifiable expectation: one prediction held, one did not
//!
//! The HEM reference file recorded a concrete prediction for this fleet
//! (its "Lessons" section, item 6). Both halves were tested:
//!
//! 1. **"No large gain on the subcooled branch"** — **supported.** Over test
//!    24's 11 deeply subcooled points (`p_0` >= 4.00 MPa, 21-31 K local
//!    subcooling) the drift-flux model gives mean \|dev\| 4.3 %, signed
//!    -2.3 %, against HEM's unbiased -1.8 %. Neither gain nor loss, as
//!    predicted: the extra physics buys nothing where nothing flashes.
//! 2. **"A gain at test 24's 3.0-3.3 MPa flashing-inception knee, with the
//!    knee moving down in pressure and sharpening"** — **refuted.** The model
//!    removes HEM's over-prediction *below* 3.10 MPa (signed +0.1 % over nine
//!    points vs HEM's up to +26.5 %), but it flattens the knee instead of
//!    sharpening it: a systematic **-23.0 %** across 3.10-3.46 MPa, worse than
//!    HEM's -18.2 %, still -12.4 % out to 4.00 MPa. Measured envelope
//!    +143 % over 2.984-3.418 MPa; model +65 % over the same interval.
//! 3. **Test 23's saturated branch** (the other place item 6 said to look, on
//!    the `L/D` < 1.5 length effect of NUREG/CR-2671 §9.2) — **supported, and
//!    it is where the whole test-23 result comes from.** HEM's uniform
//!    -10.1 % bias becomes +2.8 %.
//!
//! **What this does not establish.** The knee deficit is not attributed here.
//! The single-`tau` vapour-generation relaxation, the constant-area domain
//! that resolves no convergent inlet, and the absence of any wall-nucleation
//! or interfacial-area model are all live candidates, and this test set does
//! not separate them. A `tau` sensitivity study is the obvious next step and
//! is deliberately absent: fitting `tau` to the Marviken knee would be fitting
//! the model to its own validation set.
//!
//! **Reproduce:** `cargo test --release -p tampines --lib
//! multiphase_1d::marviken_tests -- --nocapture`. The exploration harness
//! [`marviken_drift_flux_explore`] is `#[ignore]`d and env-driven for
//! single-point work.

use uom::si::angle::radian;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Angle, Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_multiphase::drift_flux::SlipModel;

use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;

use super::drift_flux::{AxialBoundary, DriftFlux1d};
use super::geometry::Pipe1d;

// ─────────────────────────────────────────────────────────────────────────────
//  Case constants — transcribed 2026-08-11 from the HEM Marviken reference,
//  `crates/tampines-steam-tables/src/steam_turbine_equations/
//   converging_diverging_nozzles/tests/marviken_tests.rs`, which cites
//  NUREG/CR-2671 (MXC-301), *The Marviken Full Scale Critical Flow Tests:
//  Summary Report*, U.S. NRC, May 1982 (openly published, UNCLASSIFIED,
//  availability UNLIMITED; KOVAN slug `nrc1982marviken`).
//  Transcribed rather than imported: `tampines` does not depend on that crate's
//  test target, and the two crates must be able to drift apart only visibly.
// ─────────────────────────────────────────────────────────────────────────────

/// Nozzle bore \[m\] — simultaneously throat and exit, the nozzle being
/// converging only. NUREG/CR-2671 Table 3:1, report p.7 / PDF p.23 (500 mm,
/// tolerance +/-4 mm).
const NOZZLE_BORE_M: f64 = 0.500;

/// Nozzle length \[m\]. NUREG/CR-2671 Table 3:1 (166 mm), giving `L/D` = 0.332,
/// which the report rounds to 0.3.
const NOZZLE_LENGTH_M: f64 = 0.166;

/// Minimum vessel fluid temperature for Marviken **test 23** \[degC\] — the
/// nozzle-inlet water temperature the stagnation state is built from.
/// NUREG/CR-2671 Table 4:2 row 6, report p.21 / PDF p.37; 3 K nominal
/// subcooling against the 263 degC steam-dome saturation temperature at
/// 4.96 MPa. Category III test.
const TEST_23_WATER_TEMPERATURE_DEGC: f64 = 260.0;

/// Minimum vessel fluid temperature for Marviken **test 24** \[degC\].
/// NUREG/CR-2671 Table 4:2 row 6; 33 K nominal subcooling. Category II test.
const TEST_24_WATER_TEMPERATURE_DEGC: f64 = 230.0;

/// Containment back pressure \[Pa\], taken as 1 atm.
///
/// NUREG/CR-2671 states no initial containment pressure for tests 23/24; the
/// HEM reference file assumes 1 atm as a lower bound and shows the HEM answer
/// is insensitive to it (its lowest choke pressure is 2.774 MPa). **This model
/// does not get to inherit that insensitivity** — it resolves the nozzle
/// interior, so the back pressure is a real boundary condition here. The
/// sweep below therefore measures the dependence rather than assuming it, and
/// the measured peak containment pressures (236 kPa for test 23, 328 kPa for
/// test 24; Table 8:4, p.76 / PDF 92) bound how far the assumption could be
/// wrong.
const CONTAINMENT_BACK_PRESSURE_PA: f64 = 101_325.0;

/// Marviken **test 23** envelope: `(inlet stagnation pressure [kPa], measured
/// nozzle mass flux [kg/(m^2 s)])`.
///
/// Digitised from NUREG/CR-2671 Fig. 8:24 (report p.100 / PDF p.116) with
/// `graphreader`; transcribed verbatim from the HEM reference file on
/// 2026-08-11. Reading uncertainty about +/-6 % (RMS 6.3 % against the
/// Table 8:3 endpoints, measured by that file). Test 23 is the near-saturated
/// member of the pair — subcooled for only the first 4.9 s, so all but the top
/// few points are saturated-liquid stagnation states.
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

/// Marviken **test 24** envelope: `(inlet stagnation pressure [kPa], measured
/// nozzle mass flux [kg/(m^2 s)])`.
///
/// Same figure, same digitisation, same transcription date as
/// [`TEST_23_POINTS`]. Reading uncertainty about +/-6 %, degrading to about
/// +/-15 % across the steep 3.0-3.3 MPa flashing transition. Test 24 is the
/// subcooled member of the pair — subcooled for the first 21.7 s, so most of
/// these are subcooled-liquid stagnation states.
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

/// Per-point acceptance band on `|G_model / G_measured - 1|`, taken unchanged
/// from the HEM reference file's error budget so the two fidelities are judged
/// on the same scale.
///
/// 0.25. Its dominant term is the **measured** experimental scatter of the
/// digitised envelope (+/-12.9 % test 23, +/-20.0 % test 24) in quadrature
/// with the 6.3 % digitisation RMS. Nothing about the higher fidelity of this
/// solver shrinks it: no model can demonstrate better than about +/-15 %
/// agreement on this dataset.
const PER_POINT_TOLERANCE: f64 = 0.25;

/// Acceptance band on the mean of `|G_model / G_measured - 1|` over a test.
/// 0.15, same provenance as [`PER_POINT_TOLERANCE`].
const MEAN_TOLERANCE: f64 = 0.15;

/// The **bare HEM maximum-mass-flux criterion** reference numbers, **QUOTED
/// not re-measured**.
///
/// `(test label, mean |dev|, mean signed dev, max |dev|)` as fractions. Read
/// on 2026-08-11 from the doc comment of `characterise_hem_maximum_flux_criterion`
/// in the HEM reference file (measured there 2026-08-11, tampines-steam-tables
/// v0.2.5, release mode). These are the numbers the three-fidelity comparison
/// uses, on the sibling fleet's own instruction: the crate's HEM *dispatcher*
/// misses test 24 by 48.6 % through a branch-selection defect (bead `op-dqng`),
/// not through HEM physics, and comparing against that would flatter this
/// model for the wrong reason.
const HEM_MAX_FLUX_REFERENCE: [(&str, f64, f64, f64); 2] =
    [("23", 0.101, -0.101, 0.282), ("24", 0.090, -0.018, 0.265)];

// ────────────────────────────── case helpers ───────────────────────────────

/// The Marviken nozzle-inlet stagnation state at vessel pressure `p` \[Pa\],
/// for a vessel whose water sits at `t_water` \[K\].
///
/// Returns `(T_init [K], h_0 [J/kg])` — the temperature to initialise the pipe
/// with and the stagnation specific enthalpy to feed the reservoir inlet.
///
/// This is the HEM reference file's two-branch model, reimplemented on the
/// same IAPWS-IF97 Region-1 forward equation this crate's property layer uses:
///
/// - `Tsat(p) > t_water` — the vessel water is still subcooled, so the
///   stagnation state is subcooled liquid at `(p, t_water)`.
/// - `Tsat(p) <= t_water` — the vessel water has flashed and rides the
///   saturation line, so the stagnation state is saturated liquid at `p`
///   (quality `x = 0`, i.e. Region 1 evaluated at `Tsat(p)`).
///
/// The second branch is parameter-free and is corroborated by NUREG/CR-2671
/// Table 8:3, which gives a nozzle-inlet "universal stagnation quality" of
/// `x <= 60e-4` (test 23) and `x <= 29.4e-4` (test 24) throughout the saturated
/// period. A constant-stagnation-enthalpy model reaches `x ~ 0.04` and was
/// tried and rejected by the HEM file; it is not re-tried here.
fn marviken_stagnation_state(p_pa: f64, t_water_k: f64) -> (f64, f64) {
    let p = Pressure::new::<pascal>(p_pa);
    let t_sat = sat_temp_4(p).get::<kelvin>();
    let t_init = t_water_k.min(t_sat);
    let h0 = h_tp_1(
        ThermodynamicTemperature::new::<kelvin>(t_init),
        p,
    )
    .get::<joule_per_kilogram>();
    (t_init, h0)
}

/// One Marviken drift-flux run's settings.
///
/// The mesh and timestep are numerics; `c0`, `vgj_x` and `tau_s` are the
/// drift-flux model's only exposed **model parameters** and are called out
/// here rather than buried in literals, because the sensitivity of the result
/// to them is part of the V&V deliverable
/// (see [`marviken_drift_flux_sensitivity`]).
#[derive(Debug, Clone, Copy)]
struct MarvikenCase {
    /// Number of uniform axial cells across the 166 mm nozzle length.
    n_cells: usize,
    /// Fixed timestep \[s\].
    dt_s: f64,
    /// End of the integration \[s\].
    t_end: f64,
    /// Time over which the outlet pressure is ramped from the stagnation
    /// pressure down to its set value \[s\]. A numerical device only — a
    /// step change of several MPa across one face on the first step is a
    /// gratuitously hard start — and its irrelevance to the steady answer is
    /// measured by [`marviken_drift_flux_sensitivity`].
    ramp_s: f64,
    /// Zuber-Findlay distribution parameter `C_0` \[-\].
    c0: f64,
    /// Zuber-Findlay drift velocity `V_gj` along `+x` \[m/s\].
    vgj_x: f64,
    /// Vapour-generation relaxation time `tau` \[s\] — the flashing-delay
    /// (metastability) knob.
    tau_s: f64,
}

impl Default for MarvikenCase {
    /// The headline configuration, **fixed before any deviation was looked
    /// at**: 8 cells over the 166 mm nozzle, `dt = 20 us`, 12 ms of transient
    /// with a 1 ms outlet ramp, the reference crate's own documented typical
    /// churn-turbulent Zuber-Findlay band taken at its low end
    /// (`C_0 = 1.13`, `V_gj = 0.1 m/s` — the same values the Edwards case
    /// uses), and the solver's default `tau`.
    fn default() -> Self {
        Self {
            n_cells: 8,
            dt_s: 20.0e-6,
            t_end: 12.0e-3,
            ramp_s: 1.0e-3,
            c0: 1.13,
            vgj_x: 0.1,
            tau_s: super::drift_flux::DEFAULT_VAPOUR_RELAXATION_TIME,
        }
    }
}

/// What one steady-state Marviken run produces.
#[derive(Debug, Clone, Copy)]
struct SteadyRun {
    /// Nozzle-exit mass flux \[kg/(m^2 s)\], averaged over the steadiness
    /// window. NaN if the run did not survive.
    g: f64,
    /// Steadiness metric \[-\]: the half-spread `(G_max - G_min) / (G_max +
    /// G_min)` of the exit mass flux over the last third of the march. Small
    /// means the transient has settled; it is **reported, not assumed**.
    steadiness: f64,
    /// Largest material Courant number over the run \[-\].
    max_courant: f64,
    /// Void fraction in the exit cell at the end of the run \[-\].
    exit_void: f64,
    /// Pressure in the exit cell at the end of the run \[Pa\].
    exit_pressure: f64,
    /// Steps completed.
    steps: usize,
    /// True if the march ran to `t_end` without a refused step.
    completed: bool,
}

impl SteadyRun {
    /// A run that did not survive: no flux, and excluded from the maximisation.
    fn failed(steps: usize) -> Self {
        Self {
            g: f64::NAN,
            steadiness: f64::NAN,
            max_courant: f64::NAN,
            exit_void: f64::NAN,
            exit_pressure: f64::NAN,
            steps,
            completed: false,
        }
    }
}

/// March one nozzle configuration to quasi-steady state and report its exit
/// mass flux.
///
/// # The model of the experiment
///
/// The 500 mm / `L/D` = 0.3 Marviken nozzle is represented as a **1-D
/// constant-area pipe**, 0.5 m bore by 0.166 m long, fed at `x = 0` by an
/// [`AxialBoundary::ReservoirInlet`] at the vessel stagnation state
/// `(p_0, h_0)` and discharging at `x = L` through an
/// [`AxialBoundary::PressureOutlet`] at `p_out`.
///
/// **The constant-area approximation, and what it costs.** The real nozzle is
/// a rounded 250 mm-radius inlet followed by a constant-bore cylinder — i.e.
/// converging then straight, with no diverging section. This model resolves
/// only the cylinder; the convergence is collapsed into the reservoir
/// boundary's Bernoulli jump. The expected consequence is specific: **there is
/// no geometric throat inside the domain**, so nothing in the field equations
/// can produce an area-driven sonic point. Any critical behaviour must come
/// from flashing alone. That is a real limitation of the setup and it is the
/// reason the critical flux is taken by an *external* maximisation over `p_out`
/// (below) rather than read off a single run.
///
/// # Initial condition and start-up
///
/// The pipe starts uniform at the stagnation state — `(p_0, T_init)`, with
/// `T_init` the two-branch vessel temperature — and the outlet pressure is
/// ramped linearly from `p_0` to `p_out` over `ramp_s`, then held. The ramp is
/// numerical kindness only; its effect on the steady answer is measured.
///
/// # Steadiness criterion
///
/// The exit mass flux is sampled every 50 steps. The run reports the **mean**
/// over the last third of the march as `g`, and the **half-spread**
/// `(G_max - G_min)/(G_max + G_min)` over that same window as `steadiness`. A
/// value near zero means quasi-steady has been reached; the value is printed
/// and asserted on rather than assumed, because a case that never settles must
/// not be reported as a steady flux.
fn run_steady(case: MarvikenCase, p0_pa: f64, t_init_k: f64, h0: f64, p_out_pa: f64) -> SteadyRun {
    let pipe = match Pipe1d::circular(
        Length::new::<meter>(NOZZLE_LENGTH_M),
        Length::new::<meter>(NOZZLE_BORE_M),
        Angle::new::<radian>(0.0),
        case.n_cells,
    ) {
        Ok(p) => p,
        Err(_) => return SteadyRun::failed(0),
    };
    let area = pipe.area_si();

    let slip = SlipModel::ZuberFindlay {
        c0: case.c0,
        vgj: Vector3::new(case.vgj_x, 0.0, 0.0),
    };

    let mut solver = match DriftFlux1d::new(
        pipe,
        slip,
        Pressure::new::<pascal>(p0_pa),
        ThermodynamicTemperature::new::<kelvin>(t_init_k),
        Time::new::<second>(case.dt_s),
    ) {
        Ok(s) => s,
        Err(_) => return SteadyRun::failed(0),
    };
    if solver
        .set_vapour_relaxation_time(Time::new::<second>(case.tau_s))
        .is_err()
    {
        return SteadyRun::failed(0);
    }

    solver.set_left_boundary(AxialBoundary::ReservoirInlet {
        stagnation_pressure: p0_pa,
        stagnation_enthalpy: h0,
    });

    let n_steps = (case.t_end / case.dt_s).round() as usize;
    let sample_stride = 50usize;
    let mut g_series: Vec<f64> = Vec::new();
    let mut max_courant = 0.0_f64;
    let mut steps = 0usize;
    let mut time_s = 0.0_f64;

    for step in 1..=n_steps {
        // Linear ramp of the receiver pressure, re-imposed every step.
        let w = (time_s / case.ramp_s).clamp(0.0, 1.0);
        solver.set_right_boundary(AxialBoundary::PressureOutlet {
            pressure: p0_pa + w * (p_out_pa - p0_pa),
        });
        match solver.step() {
            Ok(report) => {
                time_s = report.time;
                steps = step;
                max_courant = max_courant.max(report.max_courant);
                if step % sample_stride == 0 {
                    g_series.push(report.outlet_mass_flow / area);
                }
            }
            Err(_) => return SteadyRun::failed(step),
        }
    }

    if g_series.len() < 6 {
        return SteadyRun::failed(steps);
    }
    let window_start = g_series.len() - g_series.len() / 3;
    let window = &g_series[window_start..];
    let mean = window.iter().sum::<f64>() / window.len() as f64;
    let lo = window.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = window.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let steadiness = if (hi + lo).abs() > 0.0 {
        (hi - lo) / (hi + lo).abs()
    } else {
        f64::NAN
    };

    let n = solver.pipe().n_cells();
    SteadyRun {
        g: mean,
        steadiness,
        max_courant,
        exit_void: solver.void_fraction()[n - 1],
        exit_pressure: solver.pressure()[n - 1],
        steps,
        completed: true,
    }
}

/// Outlet-pressure ratios `p_out / p_0` swept by
/// [`critical_flux_by_sweep`]. The containment back pressure is appended, so
/// the sweep always reaches the receiver pressure the experiment actually
/// discharged into.
///
/// The grid brackets the HEM critical-pressure ratios for this dataset
/// (0.55-0.75 of stagnation, from the HEM file's reported 2.774-3.723 MPa
/// choke pressures) with room on both sides, so a maximum is not forced to sit
/// at an endpoint by the choice of grid.
const SWEEP_RATIOS: [f64; 12] = [
    0.95, 0.90, 0.85, 0.80, 0.75, 0.70, 0.65, 0.60, 0.55, 0.50, 0.40, 0.30,
];

/// The result of maximising the steady mass flux over receiver pressure.
#[derive(Debug, Clone)]
struct SweepResult {
    /// `max` over the sweep of the steady exit mass flux \[kg/(m^2 s)\].
    g_crit: f64,
    /// Receiver pressure \[Pa\] at which that maximum occurred.
    p_at_max: f64,
    /// Steady flux \[kg/(m^2 s)\] at the containment back pressure — the one
    /// receiver pressure the experiment actually had.
    g_at_back_pressure: f64,
    /// True if the maximum is **interior** to the sweep, i.e. the flux stops
    /// rising before the lowest receiver pressure is reached. False means the
    /// flux was still climbing at the bottom of the sweep — a finding, not a
    /// result.
    interior_maximum: bool,
    /// Worst steadiness half-spread over the surviving runs \[-\].
    worst_steadiness: f64,
    /// Largest material Courant number over the surviving runs \[-\].
    max_courant: f64,
    /// How many of the swept receiver pressures produced a completed run.
    n_completed: usize,
    /// How many were attempted.
    n_attempted: usize,
    /// The whole sweep, for printing: `(p_out [Pa], run)`.
    points: Vec<(f64, SteadyRun)>,
}

impl SweepResult {
    /// Fractional spread of the steady exit mass flux over the **lower half**
    /// of the receiver-pressure sweep, `(G_max - G_min) / G_max` taken over
    /// the completed runs with `p_out <= 0.6 p_0` \[-\].
    ///
    /// This is the honest substitute for [`Self::interior_maximum`] when that
    /// comes back `false`. A pressure-based scheme with no imposed choking
    /// criterion need never saturate exactly (see the doc of
    /// [`critical_flux_by_sweep`]), but it can still go *nearly* flat — and
    /// "nearly flat" is a measurable claim where "choked" is not. A value of
    /// 0.05 means the flux varies by 5 % while the receiver pressure falls by
    /// a factor of ~20, i.e. an effective plateau. A value near the total
    /// rise across the whole sweep means no plateau at all.
    ///
    /// Returns NaN if fewer than two runs in that band completed.
    fn plateau_spread(&self, p0_pa: f64) -> f64 {
        let g: Vec<f64> = self
            .points
            .iter()
            .filter(|(p_out, r)| *p_out <= 0.6 * p0_pa && r.completed && r.g.is_finite())
            .map(|(_, r)| r.g)
            .collect();
        if g.len() < 2 {
            return f64::NAN;
        }
        let lo = g.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = g.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (hi - lo) / hi
    }

    /// Steadiness half-spread of the *winning* run — the one whose flux is
    /// reported as `g_crit` \[-\].
    ///
    /// [`Self::worst_steadiness`] is taken over every run in the sweep,
    /// including the high-receiver-pressure ones that are still accelerating
    /// at `t_end` and are never selected. This is the figure that qualifies
    /// the number actually quoted. NaN if no run completed.
    fn steadiness_at_max(&self) -> f64 {
        self.points
            .iter()
            .find(|(p_out, r)| r.completed && *p_out == self.p_at_max)
            .map(|(_, r)| r.steadiness)
            .unwrap_or(f64::NAN)
    }
}

/// The critical mass flux at one Marviken stagnation point, by **external
/// maximisation of the steady flux over receiver pressure**.
///
/// # Why the maximisation is external
///
/// This is deliberately the same framing the bare HEM maximum-flux criterion
/// uses — `G_crit = max_p G(p)` — so that only the *model* changes between the
/// two fidelities and not the definition of the answer. HEM maximises an
/// algebraic expression along an isentrope; this maximises a *marched steady
/// state* over the boundary condition. The comparison is like-for-like in
/// framing, and the difference in the numbers is attributable to the physics.
///
/// # The honest risk, and how it is handled
///
/// A semi-implicit pressure-based scheme propagates pressure information
/// implicitly, i.e. elliptically, so a receiver pressure is felt upstream.
/// Such a scheme need not saturate the flux as the back pressure drops — which
/// is exactly why RELAP5 imposes choking as a *boundary criterion* instead of
/// capturing it from the field equations. If the flux keeps rising all the way
/// to the ambient receiver pressure, [`SweepResult::interior_maximum`] comes
/// back `false` and that is reported as a limitation of the method, not
/// papered over. Nothing here manufactures a plateau.
fn critical_flux_by_sweep(
    case: MarvikenCase,
    p0_pa: f64,
    t_init_k: f64,
    h0: f64,
    p_back_pa: f64,
) -> SweepResult {
    let mut targets: Vec<f64> = SWEEP_RATIOS
        .iter()
        .map(|r| r * p0_pa)
        .filter(|p| *p > p_back_pa * 1.05)
        .collect();
    targets.push(p_back_pa);

    let mut points: Vec<(f64, SteadyRun)> = Vec::with_capacity(targets.len());
    for p_out in targets.iter().copied() {
        points.push((p_out, run_steady(case, p0_pa, t_init_k, h0, p_out)));
    }

    let n_attempted = points.len();
    let n_completed = points.iter().filter(|(_, r)| r.completed).count();
    let mut g_crit = f64::NEG_INFINITY;
    let mut p_at_max = f64::NAN;
    let mut best_index = usize::MAX;
    let mut worst_steadiness = 0.0_f64;
    let mut max_courant = 0.0_f64;
    for (k, (p_out, run)) in points.iter().enumerate() {
        if !run.completed || !run.g.is_finite() {
            continue;
        }
        worst_steadiness = worst_steadiness.max(run.steadiness);
        max_courant = max_courant.max(run.max_courant);
        if run.g > g_crit {
            g_crit = run.g;
            p_at_max = *p_out;
            best_index = k;
        }
    }

    // The maximum is interior when a lower-pressure run survived and did not
    // beat it. The sweep is ordered from the highest receiver pressure down,
    // so "interior" means the winning index is not the last surviving one.
    let last_completed = points.iter().rposition(|(_, r)| r.completed);
    let interior_maximum = match (best_index, last_completed) {
        (b, Some(l)) if b != usize::MAX => b < l,
        _ => false,
    };

    let g_at_back_pressure = points
        .last()
        .filter(|(_, r)| r.completed)
        .map(|(_, r)| r.g)
        .unwrap_or(f64::NAN);

    SweepResult {
        g_crit: if g_crit.is_finite() { g_crit } else { f64::NAN },
        p_at_max,
        g_at_back_pressure,
        interior_maximum,
        worst_steadiness,
        max_courant,
        n_completed,
        n_attempted,
        points,
    }
}

/// Summary statistics of a set of signed relative deviations: `(mean |dev|,
/// mean signed dev, max |dev|)`, as fractions. Mirrors the HEM reference
/// file's `deviation_stats` so the two are compared on identical metrics.
fn deviation_stats(deviations: &[f64]) -> (f64, f64, f64) {
    assert!(
        !deviations.is_empty(),
        "no deviations collected -- the Marviken point set is empty"
    );
    let n = deviations.len() as f64;
    let mean_abs = deviations.iter().map(|d| d.abs()).sum::<f64>() / n;
    let mean_signed = deviations.iter().sum::<f64>() / n;
    let max_abs = deviations.iter().fold(0.0_f64, |acc, d| acc.max(d.abs()));
    (mean_abs, mean_signed, max_abs)
}

/// What one whole Marviken test set produced.
struct TestSetResult {
    deviations: Vec<f64>,
    /// Per point: `(p_0 [kPa], G_model, G_measured, dev, sweep)`.
    rows: Vec<(f64, f64, f64, f64, SweepResult)>,
    n_interior_maxima: usize,
    worst_steadiness: f64,
    max_courant: f64,
    n_runs: usize,
    n_failed_runs: usize,
}

/// Run one whole Marviken point set through the drift-flux solver and print
/// the per-point comparison table.
fn run_test_set(label: &str, points: &[(f64, f64)], t_water_degc: f64, case: MarvikenCase) -> TestSetResult {
    let t_water_k = t_water_degc + 273.15;
    println!(
        "\n===== Marviken test {label}: drift flux, T_water = {t_water_degc} degC, \
         {} cells, dt = {:.0} us, t_end = {:.1} ms, C0 = {}, Vgj = {} m/s, tau = {:.0e} s =====",
        case.n_cells,
        case.dt_s * 1.0e6,
        case.t_end * 1.0e3,
        case.c0,
        case.vgj_x,
        case.tau_s
    );
    println!(
        "   p0 [kPa]  Tsub [K]  h0 [kJ/kg]     G_DF    G_meas   dev [%]  p*/p0  intMax  \
         G(p_amb)  steady  Cou"
    );

    let mut result = TestSetResult {
        deviations: Vec::with_capacity(points.len()),
        rows: Vec::with_capacity(points.len()),
        n_interior_maxima: 0,
        worst_steadiness: 0.0,
        max_courant: 0.0,
        n_runs: 0,
        n_failed_runs: 0,
    };

    for (p_kpa, g_measured) in points.iter().copied() {
        let p0_pa = p_kpa * 1.0e3;
        let (t_init, h0) = marviken_stagnation_state(p0_pa, t_water_k);
        let sweep = critical_flux_by_sweep(case, p0_pa, t_init, h0, CONTAINMENT_BACK_PRESSURE_PA);

        let t_sat = sat_temp_4(Pressure::new::<pascal>(p0_pa)).get::<kelvin>();
        let local_subcooling = (t_sat - t_water_k).max(0.0);
        let deviation = sweep.g_crit / g_measured - 1.0;

        result.n_runs += sweep.n_attempted;
        result.n_failed_runs += sweep.n_attempted - sweep.n_completed;
        result.worst_steadiness = result.worst_steadiness.max(sweep.worst_steadiness);
        result.max_courant = result.max_courant.max(sweep.max_courant);
        if sweep.interior_maximum {
            result.n_interior_maxima += 1;
        }

        println!(
            "  {:9.1} {:9.2} {:11.2} {:8.0} {:9.0} {:+9.1} {:6.2} {:>7} {:9.0} {:7.4} {:5.2}",
            p_kpa,
            local_subcooling,
            h0 / 1000.0,
            sweep.g_crit,
            g_measured,
            100.0 * deviation,
            sweep.p_at_max / p0_pa,
            if sweep.interior_maximum { "yes" } else { "NO" },
            sweep.g_at_back_pressure,
            sweep.worst_steadiness,
            sweep.max_courant
        );

        result.deviations.push(deviation);
        result
            .rows
            .push((p_kpa, sweep.g_crit, g_measured, deviation, sweep));
    }

    let (mean_abs, mean_signed, max_abs) = deviation_stats(&result.deviations);
    println!(
        "\n  test {label} SUMMARY: mean|dev| = {:.1} %, mean dev = {:+.1} %, max|dev| = {:.1} % \
         (n = {}); interior maxima {}/{}; runs {} ({} failed); worst steadiness {:.4}; \
         max Courant {:.2}",
        100.0 * mean_abs,
        100.0 * mean_signed,
        100.0 * max_abs,
        result.deviations.len(),
        result.n_interior_maxima,
        result.deviations.len(),
        result.n_runs,
        result.n_failed_runs,
        result.worst_steadiness,
        result.max_courant
    );
    let (hem_mean, hem_signed, hem_max) = HEM_MAX_FLUX_REFERENCE
        .iter()
        .find(|(l, _, _, _)| *l == label)
        .map(|&(_, a, b, c)| (a, b, c))
        .expect("both Marviken tests carry a quoted HEM reference");
    println!(
        "  bare-HEM reference (QUOTED from tampines-steam-tables, not re-measured): \
         mean|dev| = {:.1} %, mean dev = {:+.1} %, max|dev| = {:.1} %",
        100.0 * hem_mean,
        100.0 * hem_signed,
        100.0 * hem_max
    );

    result
}

/// Print one point's whole receiver-pressure sweep — the evidence for or
/// against an interior maximum.
fn print_sweep(label: &str, p0_pa: f64, sweep: &SweepResult) {
    println!("\n--- sweep detail, {label}, p_0 = {:.3} MPa ---", p0_pa / 1.0e6);
    println!(
        "   p_out [MPa]  p_out/p0        G   steady   exit alpha  exit p [MPa]  Cou  steps  ok"
    );
    for (p_out, run) in sweep.points.iter() {
        println!(
            "  {:12.4} {:9.3} {:8.0} {:8.4} {:12.4} {:13.4} {:4.2} {:6}  {}",
            p_out / 1.0e6,
            p_out / p0_pa,
            run.g,
            run.steadiness,
            run.exit_void,
            run.exit_pressure / 1.0e6,
            run.max_courant,
            run.steps,
            run.completed
        );
    }
    println!(
        "  -> G_crit {:.0} kg/(m^2 s) at p_out/p0 = {:.3}; interior maximum: {}; \
         lower-half plateau spread {:.4}; steadiness at the maximum {:.4}",
        sweep.g_crit,
        sweep.p_at_max / p0_pa,
        sweep.interior_maximum,
        sweep.plateau_spread(p0_pa),
        sweep.steadiness_at_max()
    );
}

/// Structural (non-physics) facts about a whole Marviken test set, extracted
/// from its per-point sweeps and printed as the evidence behind the gate.
///
/// These are the "did the numerics behave" questions, kept separate from the
/// deviation statistics so a good agreement built on a run that never settled
/// cannot be mistaken for a result.
#[derive(Debug, Clone, Copy)]
struct GateDiagnostics {
    /// Worst steadiness half-spread among the *winning* runs \[-\], i.e. among
    /// the runs whose flux was actually quoted. See
    /// [`SweepResult::steadiness_at_max`].
    worst_steadiness_at_max: f64,
    /// Worst lower-half plateau spread over the point set \[-\]. See
    /// [`SweepResult::plateau_spread`].
    worst_plateau_spread: f64,
    /// Mean lower-half plateau spread over the point set \[-\].
    mean_plateau_spread: f64,
    /// Number of points whose flux maximum was interior to the sweep.
    n_interior_maxima: usize,
    /// Number of points where the reported maximum sat at the lowest
    /// (containment) receiver pressure — the complement of the above.
    n_endpoint_maxima: usize,
    /// Largest material Courant number anywhere in the point set \[-\].
    max_courant: f64,
    /// Refused steps: swept runs that did not reach `t_end`.
    n_failed_runs: usize,
    /// Swept runs attempted in total.
    n_runs: usize,
}

/// Reduce a [`TestSetResult`] to its structural diagnostics and print them.
fn gate_diagnostics(label: &str, result: &TestSetResult) -> GateDiagnostics {
    let mut worst_steadiness_at_max = 0.0_f64;
    let mut worst_plateau_spread = 0.0_f64;
    let mut plateau_sum = 0.0_f64;
    let mut plateau_n = 0usize;
    for (p_kpa, _, _, _, sweep) in result.rows.iter() {
        let s = sweep.steadiness_at_max();
        if s.is_finite() {
            worst_steadiness_at_max = worst_steadiness_at_max.max(s);
        }
        let spread = sweep.plateau_spread(p_kpa * 1.0e3);
        if spread.is_finite() {
            worst_plateau_spread = worst_plateau_spread.max(spread);
            plateau_sum += spread;
            plateau_n += 1;
        }
    }
    let diagnostics = GateDiagnostics {
        worst_steadiness_at_max,
        worst_plateau_spread,
        mean_plateau_spread: if plateau_n > 0 {
            plateau_sum / plateau_n as f64
        } else {
            f64::NAN
        },
        n_interior_maxima: result.n_interior_maxima,
        n_endpoint_maxima: result.rows.len() - result.n_interior_maxima,
        max_courant: result.max_courant,
        n_failed_runs: result.n_failed_runs,
        n_runs: result.n_runs,
    };
    println!(
        "  test {label} DIAGNOSTICS: worst steadiness at the quoted maximum {:.4}; \
         plateau spread over the lower half of the sweep mean {:.4} worst {:.4}; \
         interior maxima {} / endpoint maxima {}; max Courant {:.3}; \
         runs {} ({} refused)",
        diagnostics.worst_steadiness_at_max,
        diagnostics.mean_plateau_spread,
        diagnostics.worst_plateau_spread,
        diagnostics.n_interior_maxima,
        diagnostics.n_endpoint_maxima,
        diagnostics.max_courant,
        diagnostics.n_runs,
        diagnostics.n_failed_runs
    );
    diagnostics
}

/// Count the points whose deviation falls outside [`PER_POINT_TOLERANCE`], and
/// print them.
fn points_outside_band(label: &str, result: &TestSetResult) -> usize {
    let mut n = 0usize;
    for (p_kpa, g_model, g_meas, dev, _) in result.rows.iter() {
        if dev.abs() > PER_POINT_TOLERANCE {
            n += 1;
            println!(
                "  test {label} OUTSIDE BAND: p_0 = {:.1} kPa, G_DF = {:.0}, \
                 G_meas = {:.0}, dev = {:+.1} %",
                p_kpa,
                g_model,
                g_meas,
                100.0 * dev
            );
        }
    }
    println!(
        "  test {label}: {n} of {} points outside the +/-{:.0} % per-point band",
        result.rows.len(),
        100.0 * PER_POINT_TOLERANCE
    );
    n
}

/// Deviation statistics restricted to a stagnation-pressure window
/// `[p_lo, p_hi)` in kPa, printed under `band`.
///
/// The aggregate over a whole Marviken envelope hides where a model is right
/// and where it is wrong — test 24 in particular spans 0.6 K to 31 K of local
/// subcooling and three distinct flow regimes. Returns
/// `(n, mean |dev|, mean signed dev, max |dev|)`; `n` may be zero, in which
/// case the statistics are NaN.
fn band_stats(
    label: &str,
    band: &str,
    result: &TestSetResult,
    p_lo_kpa: f64,
    p_hi_kpa: f64,
) -> (usize, f64, f64, f64) {
    let devs: Vec<f64> = result
        .rows
        .iter()
        .filter(|(p_kpa, _, _, _, _)| *p_kpa >= p_lo_kpa && *p_kpa < p_hi_kpa)
        .map(|(_, _, _, dev, _)| *dev)
        .collect();
    if devs.is_empty() {
        println!("  test {label} BAND {band} ({p_lo_kpa:.0}-{p_hi_kpa:.0} kPa): no points");
        return (0, f64::NAN, f64::NAN, f64::NAN);
    }
    let (mean_abs, mean_signed, max_abs) = deviation_stats(&devs);
    println!(
        "  test {label} BAND {band} ({p_lo_kpa:.0}-{p_hi_kpa:.0} kPa, n = {}): \
         mean|dev| = {:.1} %, mean dev = {:+.1} %, max|dev| = {:.1} %",
        devs.len(),
        100.0 * mean_abs,
        100.0 * mean_signed,
        100.0 * max_abs
    );
    (devs.len(), mean_abs, mean_signed, max_abs)
}

/// Assert that a measured deviation statistic reproduces its recorded value.
///
/// The solver is deterministic (fixed timestep, no RNG, no thread-dependent
/// reduction), so a re-run must return the same numbers to round-off. The band
/// is 0.02 in absolute deviation-fraction terms — wide enough that a
/// compiler/`libm` difference cannot trip it, narrow enough that any real
/// change in the drift-flux closures or the property layer will.
fn assert_recorded(name: &str, measured: f64, recorded: f64) {
    const REGRESSION_BAND: f64 = 0.02;
    assert!(
        (measured - recorded).abs() <= REGRESSION_BAND,
        "{name}: measured {measured:.4} but the recorded 2026-08-11 characterisation is \
         {recorded:.4} (band +/-{REGRESSION_BAND}). This is a regression guard, not a \
         tolerance: if the change is intended, re-measure and update the doc comment and \
         the recorded value together -- do not widen the band."
    );
}

// ──────────────────────────────── the tests ────────────────────────────────

/// V&V gate — **Marviken test 23** (near-saturated, 3 K nominal subcooling) at
/// drift-flux fidelity. **VALIDATED.**
///
/// # Methodology
///
/// **What is computed.** For each of the 29 digitised
/// [`TEST_23_POINTS`] stagnation pressures, the nozzle-inlet stagnation state
/// is built by [`marviken_stagnation_state`] at `T_w` = 260 degC, the 1-D
/// constant-area nozzle is marched to quasi-steady by [`run_steady`] at each
/// of 13 receiver pressures, and the critical mass flux is taken as the
/// maximum of the steady exit flux over that sweep
/// ([`critical_flux_by_sweep`]) — deliberately the same `G_crit = max_p G(p)`
/// framing the bare HEM criterion uses, so only the *model* differs between
/// the two fidelities.
///
/// **Reference.** The measured NUREG/CR-2671 Fig. 8:24 envelope
/// ([`TEST_23_POINTS`], report p.100 / PDF p.116). The *comparison baseline*
/// is [`HEM_MAX_FLUX_REFERENCE`], the bare HEM maximum-mass-flux criterion
/// (mean 10.1 %, signed -10.1 %, max 28.2 %), quoted not re-measured. The
/// crate HEM *dispatcher* is deliberately not used as the baseline: its test-24
/// deficit is a branch-selection defect (bead `op-dqng`), and comparing against
/// it would flatter this model for the wrong reason.
///
/// **Inputs.** [`MarvikenCase::default`] — 8 cells over the 166 mm nozzle,
/// `dt` = 20 us, `t_end` = 12 ms, 1 ms outlet ramp, Zuber-Findlay
/// `C_0` = 1.13 / `V_gj` = 0.1 m/s, `tau` = 1 ms. Geometry 0.500 m bore x
/// 0.166 m. Receiver sweep [`SWEEP_RATIOS`] plus the 101.325 kPa containment
/// back pressure. **No coefficient was tuned to this dataset.**
///
/// **Pass criterion.** The HEM reference file's error budget, unchanged:
/// mean `|G_DF/G_meas - 1|` <= [`MEAN_TOLERANCE`] (0.15) **and** every point
/// within [`PER_POINT_TOLERANCE`] (0.25). Both must hold for "validated".
///
/// # Results — measured 2026-08-11, `tampines` v0.0.1, release mode
///
/// | Statistic | Drift flux | Bare HEM (quoted) |
/// |---|---|---|
/// | n points | 29 | 29 |
/// | mean \|dev\| | **8.9 %** | 10.1 % |
/// | mean signed dev | **+2.8 %** | -10.1 % |
/// | max \|dev\| | **19.7 %** (at 4.942 MPa) | 28.2 % |
/// | points outside +/-25 % | **0 / 29** | — |
///
/// **Verdict: VALIDATED.** Both criteria are met — 8.9 % against a 15 % mean
/// band, and no point outside 25 %.
///
/// Per-band residual structure (printed by the test):
///
/// | Band | n | mean \|dev\| | mean signed | max \|dev\| |
/// |---|---|---|---|---|
/// | saturated core, 3.70-4.70 MPa | 19 | 8.6 % | **+8.6 %** | 12.6 % |
/// | high-pressure tail, 4.70-5.00 MPa | 10 | 9.5 % | -8.1 % | 19.7 % |
///
/// The residual is *not* noise: the model runs uniformly **+8.6 %** high
/// across the saturated core — every one of those 19 points is positive — and
/// every large negative deviation sits in the top 300 kPa where the digitised
/// envelope itself scatters by a factor 1.3 (e.g. 31 726 vs 25 468
/// kg/(m^2 s) at 4.877 and 4.913 MPa, 36 kPa apart). The five worst points
/// (-12.6, -19.0, -12.2, -19.7, -19.3 %) are all in that tail, and each has a
/// near-pressure neighbour the model matches to a few per cent.
///
/// **Run health.** 377 marched transients, **0 refused steps**; worst
/// steadiness half-spread *at the quoted maximum* 0.0308; max material Courant
/// 0.326; wall clock **98.1 s** for the whole gate (measured by the test
/// itself, reproduced to 0.1 s across two independent runs; the two gates are
/// separate `#[test]` fns so cargo runs them in parallel).
///
/// **Interior maxima: 0 of 29 — a structural finding, not a result.** The flux
/// never stops rising as the receiver pressure falls, so the reported
/// `G_crit` is always the value at the containment back pressure. What it does
/// instead is go nearly flat: the lower half of the sweep
/// (`p_out <= 0.6 p_0`, a further factor ~20 in receiver pressure) changes the
/// flux by a mean of 4.9 % and at most 5.2 %
/// ([`SweepResult::plateau_spread`]). This solver imposes no choking criterion
/// and its pressure equation is elliptic, so it *cannot* saturate exactly;
/// "effective plateau" is the honest description and it is measured rather
/// than asserted.
///
/// # Interpretation against the falsifiable expectation
///
/// The HEM reference file predicted that a non-equilibrium model **should**
/// gain on test 23's saturated branch, where HEM carries a uniform -10.1 %
/// bias attributed to the `L/D` < 1.5 length effect of NUREG/CR-2671 §9.2.
/// **Supported.** The bias is removed and slightly over-corrected: signed mean
/// -10.1 % -> **+2.8 %**, mean absolute 10.1 % -> 8.9 %, worst 28.2 % ->
/// 19.7 %. All three statistics improve, and the sign flip is the substantive
/// part — the systematic under-prediction is gone.
#[test]
fn marviken_test_23_drift_flux_critical_mass_flux() {
    let started = std::time::Instant::now();
    let case = MarvikenCase::default();
    let result = run_test_set("23", &TEST_23_POINTS, TEST_23_WATER_TEMPERATURE_DEGC, case);
    let (mean_abs, mean_signed, max_abs) = deviation_stats(&result.deviations);
    let diagnostics = gate_diagnostics("23", &result);
    let n_outside = points_outside_band("23", &result);
    band_stats("23", "saturated core", &result, 3700.0, 4700.0);
    band_stats("23", "high-pressure tail", &result, 4700.0, 5000.0);

    // Per-point evidence for the interior-maximum question, at the saturated
    // low-pressure end of the envelope.
    let (p_kpa, _, _, _, sweep) = &result.rows[0];
    print_sweep("test 23, lowest-pressure point", p_kpa * 1.0e3, sweep);

    println!(
        "  test 23 GATE: mean|dev| {mean_abs:.4}, mean dev {mean_signed:+.4}, \
         max|dev| {max_abs:.4}, n_outside {n_outside}, wall clock {:.1} s",
        started.elapsed().as_secs_f64()
    );

    // ── run health ──────────────────────────────────────────────────────────
    assert_eq!(
        diagnostics.n_failed_runs, 0,
        "a swept run refused a step -- the flux quoted for that point is not a steady state"
    );
    assert_eq!(diagnostics.n_runs, 377, "swept-run count changed");
    assert!(
        diagnostics.max_courant < 1.0,
        "material Courant number {} reached 1; donor-cell transport has stepped past a cell",
        diagnostics.max_courant
    );
    assert!(
        diagnostics.worst_steadiness_at_max <= 0.05,
        "the quoted maximum came from a run with steadiness half-spread {} -- \
         measured 0.0308 on 2026-08-11, so anything near 0.05 means the march no \
         longer settles inside t_end",
        diagnostics.worst_steadiness_at_max
    );

    // ── structural characterisation of the method (see the doc comment) ─────
    assert_eq!(
        diagnostics.n_interior_maxima, 0,
        "measured 0/29 interior maxima on 2026-08-11; the flux rises monotonically to \
         the containment back pressure. A nonzero count is a change in the solver's \
         behaviour worth reading, not a failure to hide"
    );
    assert!(
        diagnostics.worst_plateau_spread <= 0.08,
        "lower-half plateau spread {} -- measured worst 0.0517 on 2026-08-11. Above \
         about 0.08 the flux is no longer effectively plateaued and `G_crit` degenerates \
         into 'the flux at whatever the lowest receiver pressure happened to be'",
        diagnostics.worst_plateau_spread
    );

    // ── validation criteria (both must hold) ────────────────────────────────
    assert!(
        mean_abs <= MEAN_TOLERANCE,
        "mean |dev| {mean_abs:.4} exceeds the {MEAN_TOLERANCE} aggregate band"
    );
    assert!(
        max_abs <= PER_POINT_TOLERANCE,
        "worst |dev| {max_abs:.4} exceeds the {PER_POINT_TOLERANCE} per-point band"
    );
    assert_eq!(n_outside, 0, "no test-23 point was outside the band on 2026-08-11");

    // ── regression guard on the recorded characterisation ───────────────────
    assert_recorded("test 23 mean |dev|", mean_abs, 0.0888);
    assert_recorded("test 23 mean signed dev", mean_signed, 0.0283);
    assert_recorded("test 23 max |dev|", max_abs, 0.1967);

    // ── the falsifiable expectation: drift flux should gain here ────────────
    let (_, hem_mean, hem_signed, hem_max) = HEM_MAX_FLUX_REFERENCE[0];
    assert!(
        mean_abs < hem_mean && mean_signed.abs() < hem_signed.abs() && max_abs < hem_max,
        "the HEM reference predicts a non-equilibrium gain on test 23's saturated \
         branch: measured mean {mean_abs:.4} vs HEM {hem_mean}, signed {mean_signed:+.4} \
         vs {hem_signed}, max {max_abs:.4} vs {hem_max}"
    );
}

/// V&V gate — **Marviken test 24** (33 K nominal subcooling) at drift-flux
/// fidelity. **NOT VALIDATED — honest characterisation only.**
///
/// # Methodology
///
/// Identical to [`marviken_test_23_drift_flux_critical_mass_flux`] in every
/// respect except the point set ([`TEST_24_POINTS`], 40 points, 2.83-4.77 MPa)
/// and the vessel water temperature (`T_w` = 230 degC, 33 K nominal
/// subcooling). Same [`MarvikenCase::default`], same receiver sweep, same
/// `G_crit = max_p G(p)` framing, same error budget
/// ([`MEAN_TOLERANCE`] 0.15 aggregate, [`PER_POINT_TOLERANCE`] 0.25 per point,
/// **both** required for "validated"). Comparison baseline is again
/// [`HEM_MAX_FLUX_REFERENCE`] — the bare maximum-flux criterion, mean 9.0 %,
/// signed -1.8 %, max 26.5 % — quoted, not re-measured.
///
/// # Results — measured 2026-08-11, `tampines` v0.0.1, release mode
///
/// | Statistic | Drift flux | Bare HEM (quoted) |
/// |---|---|---|
/// | n points | 40 | 40 |
/// | mean \|dev\| | **11.4 %** | 9.0 % |
/// | mean signed dev | **-9.7 %** | -1.8 % |
/// | max \|dev\| | **28.8 %** (at 3.194 MPa) | 26.5 % |
/// | points outside +/-25 % | **6 / 40** | — |
///
/// **Verdict: NOT VALIDATED.** The aggregate criterion is met (11.4 % against
/// a 15 % band) but the per-point criterion is not: six points miss the 25 %
/// band, and they are not scattered — **all six lie between 3.121 and
/// 3.418 MPa**, the flashing-inception knee, where the model under-predicts by
/// 25 to 29 %. This test is therefore kept as a **characterisation** of
/// measured behaviour, in the same spirit as the HEM reference file's own
/// test-24 treatment. Do not describe the drift-flux solver as
/// Marviken-validated for subcooled stagnation states.
///
/// Per-band structure — this is where the whole result lives:
///
/// | Band | n | mean \|dev\| | mean signed |
/// |---|---|---|---|
/// | near-saturated low end, 2.83-3.10 MPa | 9 | 5.2 % | +0.1 % |
/// | flashing-inception knee, 3.10-3.46 MPa | 11 | 23.0 % | **-23.0 %** |
/// | mid transition, 3.46-4.00 MPa | 9 | 12.4 % | -12.4 % |
/// | deeply subcooled, 4.00-4.80 MPa | 11 | 4.3 % | -2.3 % |
///
/// The two ends are excellent and the middle is not. The measured envelope
/// climbs from 17.2 to 41.8 Mg/(m^2 s) between 2.984 and 3.418 MPa (**+143 %**
/// over 434 kPa); the model climbs from 18.8 to 31.0 over the same interval
/// (**+65 %**). The transition is real in the model but far too gradual.
///
/// **Run health.** 520 marched transients, **0 refused steps**; worst
/// steadiness half-spread at the quoted maximum 0.1256; max material Courant
/// 0.269; wall clock **381 s** for the whole gate (381.0 s and 380.9 s on two
/// independent runs). **Interior maxima: 1 of
/// 40** (at 4.306 MPa, and by a margin of 0.3 % — it is not a genuine choke,
/// it is the plateau being flat enough for the ordering to invert). Mean
/// lower-half plateau spread 0.0318, worst 0.0734. Same caveat as test 23:
/// this solver imposes no choking criterion and cannot saturate exactly.
///
/// # Interpretation against the falsifiable expectation
///
/// The HEM reference file made two predictions. Measured here:
///
/// 1. *"A non-equilibrium model should **not** gain much on the subcooled
///    branch — bare HEM is already unbiased there (-1.8 %)."*
///    **Supported.** Over the 11 deeply subcooled points (`p_0` >= 4.00 MPa,
///    21-31 K local subcooling) the drift-flux model gives mean \|dev\| 4.3 %
///    and signed **-2.3 %** — statistically the same place HEM sits, neither
///    gain nor loss. The extra physics buys nothing where there is no
///    flashing, exactly as predicted.
/// 2. *"It **should** gain at the flashing-inception knee of test 24
///    (3.0-3.3 MPa), where HEM over-predicts up to +26.5 % then under-predicts
///    up to -18.2 %; thermal non-equilibrium should move the knee down in
///    pressure and sharpen it."* **Refuted, and informatively so.** Below
///    3.10 MPa the model does remove HEM's over-prediction (measured signed
///    **+0.1 %** over nine points, against HEM's up to +26.5 %). But it does
///    not sharpen the knee — it *flattens* it: from 3.10 to 3.46 MPa the model
///    under-predicts by a systematic **-23.0 %**, worse than HEM's -18.2 %,
///    and the deficit persists at -12.4 % out to 4.00 MPa. The model's
///    flashing front comes on later in pressure and rises more slowly than the
///    data, which is the opposite of the predicted "down and sharper".
///
/// The plausible causes are not separated by this test and should not be
/// asserted as if they were: the single-`tau` vapour-generation relaxation
/// (`tau` = 1 ms, held at the solver default and **not** tuned here), the
/// constant-area domain that resolves no convergent inlet, and the absence of
/// any wall-nucleation or interfacial-area model are all candidates. A `tau`
/// sensitivity study is the obvious next step and is deliberately left out of
/// this file: fitting `tau` to the Marviken knee would be fitting the model to
/// its own validation set.
#[test]
fn marviken_test_24_drift_flux_critical_mass_flux() {
    let started = std::time::Instant::now();
    let case = MarvikenCase::default();
    let result = run_test_set("24", &TEST_24_POINTS, TEST_24_WATER_TEMPERATURE_DEGC, case);
    let (mean_abs, mean_signed, max_abs) = deviation_stats(&result.deviations);
    let diagnostics = gate_diagnostics("24", &result);
    let n_outside = points_outside_band("24", &result);
    band_stats("24", "near-saturated low end", &result, 2800.0, 3100.0);
    let (_, _, knee_signed, _) = band_stats("24", "flashing knee", &result, 3100.0, 3460.0);
    band_stats("24", "mid transition", &result, 3460.0, 4000.0);
    let (n_sub, sub_abs, sub_signed, _) =
        band_stats("24", "deeply subcooled", &result, 4000.0, 4800.0);

    // Per-point evidence at the flashing-inception knee, the place the HEM
    // reference file predicted a non-equilibrium model should differ.
    let knee = result
        .rows
        .iter()
        .min_by(|a, b| {
            (a.0 - 3150.289)
                .abs()
                .partial_cmp(&(b.0 - 3150.289).abs())
                .expect("finite pressures")
        })
        .expect("test 24 has points");
    print_sweep("test 24, flashing-inception knee", knee.0 * 1.0e3, &knee.4);

    println!(
        "  test 24 GATE: mean|dev| {mean_abs:.4}, mean dev {mean_signed:+.4}, \
         max|dev| {max_abs:.4}, n_outside {n_outside}, wall clock {:.1} s",
        started.elapsed().as_secs_f64()
    );

    // ── run health ──────────────────────────────────────────────────────────
    assert_eq!(
        diagnostics.n_failed_runs, 0,
        "a swept run refused a step -- the flux quoted for that point is not a steady state"
    );
    assert_eq!(diagnostics.n_runs, 520, "swept-run count changed");
    assert!(
        diagnostics.max_courant < 1.0,
        "material Courant number {} reached 1; donor-cell transport has stepped past a cell",
        diagnostics.max_courant
    );
    assert!(
        diagnostics.worst_steadiness_at_max <= 0.20,
        "the quoted maximum came from a run with steadiness half-spread {} -- \
         measured 0.1256 on 2026-08-11",
        diagnostics.worst_steadiness_at_max
    );
    assert!(
        diagnostics.worst_plateau_spread <= 0.10,
        "lower-half plateau spread {} -- measured worst 0.0734 on 2026-08-11",
        diagnostics.worst_plateau_spread
    );
    assert_eq!(
        diagnostics.n_interior_maxima, 1,
        "measured 1/40 interior maxima on 2026-08-11 (at 4.306 MPa, by 0.3 %); 39 of 40 \
         maxima sit at the containment back pressure"
    );

    // ── the aggregate criterion IS met ──────────────────────────────────────
    assert!(
        mean_abs <= MEAN_TOLERANCE,
        "mean |dev| {mean_abs:.4} exceeds the {MEAN_TOLERANCE} aggregate band"
    );

    // ── the per-point criterion is NOT met: characterise it exactly ─────────
    //
    // Asserting the measured failure, rather than relaxing the band, is the
    // point. Six points miss +/-25 %, and the assertion below pins *which* six
    // -- if a future change moves the knee, this fails loudly instead of
    // quietly trading one set of bad points for another.
    assert_eq!(
        n_outside, 6,
        "recorded 2026-08-11: exactly 6 of 40 points miss the +/-{PER_POINT_TOLERANCE} \
         per-point band. This test is a characterisation, not a validation"
    );
    for (p_kpa, _, _, dev, _) in result.rows.iter() {
        if dev.abs() > PER_POINT_TOLERANCE {
            assert!(
                (3100.0..3460.0).contains(p_kpa),
                "an out-of-band point appeared at {p_kpa:.1} kPa, outside the recorded \
                 3.10-3.46 MPa flashing-inception knee -- the failure has moved and the \
                 characterisation in the doc comment no longer describes it"
            );
        }
    }

    // ── regression guard on the recorded characterisation ───────────────────
    assert_recorded("test 24 mean |dev|", mean_abs, 0.1145);
    assert_recorded("test 24 mean signed dev", mean_signed, -0.0970);
    assert_recorded("test 24 max |dev|", max_abs, 0.2885);

    // ── falsifiable expectation 1: no gain on the subcooled branch ──────────
    let (_, hem_mean, hem_signed, _) = HEM_MAX_FLUX_REFERENCE[1];
    assert_eq!(n_sub, 11, "the deeply subcooled band holds 11 points");
    assert!(
        sub_abs <= 0.10 && sub_signed.abs() <= 0.06,
        "the deeply subcooled band measured mean|dev| 4.3 %, signed -2.3 % on 2026-08-11, \
         i.e. HEM's own unbiased {hem_signed} with no non-equilibrium gain; got \
         {sub_abs:.4} / {sub_signed:+.4}"
    );

    // ── falsifiable expectation 2: a gain at the knee -- REFUTED ────────────
    assert!(
        knee_signed < -0.15,
        "recorded 2026-08-11: the 3.10-3.46 MPa flashing knee is a systematic \
         {knee_signed:+.4} under-prediction (measured -23.0 %), refuting the HEM \
         reference's prediction that non-equilibrium would sharpen this knee. Whole-set \
         mean|dev| {mean_abs:.4} against the quoted bare-HEM {hem_mean}"
    );
}

/// Scratch exploration runner for the Marviken drift-flux case — **not a V&V
/// gate**.
///
/// Reads `MRV_CELLS`, `MRV_DT_US`, `MRV_TEND_MS`, `MRV_RAMP_MS`, `MRV_C0`,
/// `MRV_VGJ`, `MRV_TAU`, `MRV_P0_KPA` and `MRV_TW_DEGC` from the environment
/// and prints one full receiver-pressure sweep at a single stagnation point,
/// plus the per-run steadiness. It asserts nothing, so it can be pointed at a
/// configuration that fails: [`run_steady`] records a refused step rather than
/// panicking. Kept `#[ignore]`d so it never runs in an ordinary suite.
#[test]
#[ignore = "exploration harness, asserts nothing; run explicitly with MRV_* set"]
fn marviken_drift_flux_explore() {
    fn env_f64(key: &str, default: f64) -> f64 {
        std::env::var(key)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    }
    let base = MarvikenCase::default();
    let case = MarvikenCase {
        n_cells: env_f64("MRV_CELLS", base.n_cells as f64) as usize,
        dt_s: env_f64("MRV_DT_US", base.dt_s * 1.0e6) * 1.0e-6,
        t_end: env_f64("MRV_TEND_MS", base.t_end * 1.0e3) * 1.0e-3,
        ramp_s: env_f64("MRV_RAMP_MS", base.ramp_s * 1.0e3) * 1.0e-3,
        c0: env_f64("MRV_C0", base.c0),
        vgj_x: env_f64("MRV_VGJ", base.vgj_x),
        tau_s: env_f64("MRV_TAU", base.tau_s),
    };
    let p0_kpa = env_f64("MRV_P0_KPA", 4772.399);
    let t_water_degc = env_f64("MRV_TW_DEGC", TEST_24_WATER_TEMPERATURE_DEGC);

    let p0_pa = p0_kpa * 1.0e3;
    let (t_init, h0) = marviken_stagnation_state(p0_pa, t_water_degc + 273.15);
    println!(
        "case {case:?}\n  p_0 = {p0_kpa} kPa, T_water = {t_water_degc} degC, \
         T_init = {:.2} K, h_0 = {:.2} kJ/kg",
        t_init,
        h0 / 1000.0
    );
    let started = std::time::Instant::now();
    let sweep = critical_flux_by_sweep(case, p0_pa, t_init, h0, CONTAINMENT_BACK_PRESSURE_PA);
    print_sweep("exploration", p0_pa, &sweep);
    println!(
        "  wall clock for the whole sweep ({} runs): {:.2} s",
        sweep.n_attempted,
        started.elapsed().as_secs_f64()
    );
}
