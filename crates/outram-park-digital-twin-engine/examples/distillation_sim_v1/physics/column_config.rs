//! Default plant configuration — an 8-stage benzene/toluene distillation
//! column, saturated-liquid feed at stage 4, total condenser, reflux-ratio +
//! reboiler-duty specs.
//!
//! # Why this exact case
//!
//! This is **not a new, unverified configuration**. It is the same case
//! `outram-park-fork-dwsim-libs`'s own [`DynamicColumn`] V&V test
//! (`columns::dynamic::tests::dynamic_column_relaxes_to_steady_mesh_solution`,
//! kopi-beans `op-6rhz`, status Done) already validated: an RK4-integrated
//! transient relaxes to the independently-computed steady MESH solution to
//! 4e-4 K stage temperature, 9.3e-6 liquid mole fraction, and 1e-3 mol/s on
//! the product split. Reusing it here means this simulator's default plant is
//! backed by that measured agreement, not a fresh, uncompared setup.
//!
//! # Reboiler duty
//!
//! The dynamic column takes reboiler duty as a **direct input**
//! ([`DynamicColumnOperating::reboiler_duty_watts`]), not something it solves
//! for. [`steady_reboiler_duty_watts`] gets a physically consistent value the
//! same way the validated test does: solve the column **steady** with a
//! reflux-ratio spec and a bottoms-flow spec, then read back the duty that
//! solve implies. That duty is then handed to the *dynamic* column alongside
//! the *same* reflux ratio, so the dynamic column's steady state (once it
//! relaxes) is the profile the steady solver already found — which is
//! exactly what the V&V test checks.
//!
//! The steady solver reports the reboiler stage heat with the **opposite**
//! of the physical sign (documented on the steady solver itself: a negative
//! reboiler duty though the reboiler physically adds heat), so
//! [`steady_reboiler_duty_watts`] negates it — the dynamic column's
//! `reboiler_duty_watts` is the physical heat *input*.

use outram_park_fork_dwsim_libs::columns::dynamic::{DynamicColumnOperating, TrayHydraulics};
use outram_park_fork_dwsim_libs::columns::initial_estimates::RigorousColumn;
use outram_park_fork_dwsim_libs::columns::solver::ColumnSolverMethod;
use outram_park_fork_dwsim_libs::columns::thermo_bridge::ColumnThermo;
use outram_park_fork_dwsim_libs::columns::{
    ColumnSolverInput, ColumnSpec, MolarFlowRate, Stage, StagePressure, StageTemperature,
};
use outram_park_fork_dwsim_libs::thermo::component::reference::{benzene, toluene};
use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;
use uom::si::catalytic_activity::katal;
use uom::si::f64::MolarEnergy;
use uom::si::molar_energy::joule_per_mole;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Column pressure — atmospheric \[Pa\], uniform on every stage.
pub const PRESSURE_PA: f64 = 101_325.0;
/// Number of equilibrium stages (condenser + 6 interior trays + reboiler).
pub const N_STAGES: usize = 8;
/// Feed stage index (0 = condenser/top, `N_STAGES - 1` = reboiler/bottom).
pub const FEED_STAGE: usize = 4;
/// Feed molar flow \[mol/s\] — an equimolar (50/50) saturated liquid.
pub const FEED_FLOW_MOL_S: f64 = 1.0;
/// Feed liquid mole fraction of the light key (benzene); the heavy key
/// (toluene) is `1.0 - this`.
pub const FEED_Z_BENZENE: f64 = 0.5;
/// Default reflux ratio `R = L_0 / D` \[-\] — the value both the steady
/// reference solve and the dynamic column start at.
pub const DEFAULT_REFLUX_RATIO: f64 = 2.0;
/// Default bottoms product rate \[mol/s\] used only to find a physically
/// consistent reboiler duty at startup (see [`steady_reboiler_duty_watts`]);
/// the dynamic column itself is driven by duty, not by this flow.
pub const DEFAULT_BOTTOMS_MOL_S: f64 = 0.5;
/// Tray hydraulic time constant \[s\] (first-order holdup-to-flow lag).
pub const TRAY_TAU_S: f64 = 30.0;
/// Constant condenser-drum molar holdup \[mol\] (perfect level control).
pub const DRUM_HOLDUP_MOL: f64 = 50.0;
/// Constant reboiler-sump molar holdup \[mol\] (perfect level control).
pub const SUMP_HOLDUP_MOL: f64 = 50.0;

/// Build the benzene/toluene [`ColumnSolverInput`] this whole example is
/// built on — the shared config both the steady reference solve and the
/// dynamic column consume.
///
/// `reboiler_duty_watts` only matters for the steady solve's initial
/// temperature-profile estimate quality, not for correctness; the dynamic
/// column reads its own duty from [`DynamicColumnOperating`], not from here.
pub fn column_input(reboiler_duty_watts: f64) -> ColumnSolverInput {
    let comps = vec![benzene(), toluene()];
    let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
    let feed_z = [FEED_Z_BENZENE, 1.0 - FEED_Z_BENZENE];
    let t_feed = thermo
        .bubble_temperature(&feed_z, PRESSURE_PA, 365.0, FEED_STAGE)
        .map(|(t, _)| t)
        .unwrap_or(365.0);
    let h_feed = thermo.feed_molar_enthalpy(&feed_z, t_feed, PRESSURE_PA, 0.0);

    let p = StagePressure::new::<pascal>(PRESSURE_PA);
    let mut stages: Vec<Stage> = (0..N_STAGES)
        .map(|i| {
            let t = StageTemperature::new::<kelvin>(355.0 + 4.0 * i as f64);
            Stage::new(format!("stage {i}"), p, t, 2)
        })
        .collect();
    stages[FEED_STAGE] = stages[FEED_STAGE].clone().with_feed(
        MolarFlowRate::new::<katal>(FEED_FLOW_MOL_S),
        feed_z.to_vec(),
        MolarEnergy::new::<joule_per_mole>(h_feed),
    );

    RigorousColumn::distillation(
        comps,
        PropertyPackageModel::Ideal,
        stages,
        ColumnSpec::reflux_ratio(DEFAULT_REFLUX_RATIO),
        ColumnSpec::heat_duty(uom::si::f64::Power::new::<watt>(reboiler_duty_watts)),
    )
    .with_distillate_estimate(MolarFlowRate::new::<katal>(
        FEED_FLOW_MOL_S - DEFAULT_BOTTOMS_MOL_S,
    ))
    .with_reflux_ratio_estimate(DEFAULT_REFLUX_RATIO)
    .solver_input()
    .expect("column_input: estimate generation must succeed for the validated benzene/toluene case")
}

/// Solve the column **steady** (reflux-ratio + bottoms-flow specs) and return
/// the reboiler duty \[W\] that solve implies, sign-corrected to the physical
/// convention (positive = heat added). This is the duty the dynamic column
/// is then built with, exactly as `op-6rhz`'s own V&V test does it.
///
/// # Panics
///
/// If the steady solve does not converge. This is the same case `op-6rhz`
/// validated converging, so a failure here means the shared column config or
/// the solver has regressed, not that this call site is doing anything
/// unusual — panicking loudly at startup is preferable to silently falling
/// back to a duty nobody has checked.
pub fn steady_reboiler_duty_watts() -> f64 {
    let mut input = column_input(0.0);
    input.reboiler_spec =
        ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(DEFAULT_BOTTOMS_MOL_S));
    let steady = ColumnSolverMethod::default().solve(&input).expect(
        "steady_reboiler_duty_watts: the reference benzene/toluene column must converge \
                 (same case validated by op-6rhz)",
    );
    -steady.reboiler_duty().get::<watt>()
}

/// The dynamic operating point a freshly launched plant starts at: the
/// default reflux ratio, the steady-consistent reboiler duty, and the
/// hydraulic/inventory constants above.
pub fn default_operating(reboiler_duty_watts: f64) -> DynamicColumnOperating {
    DynamicColumnOperating {
        reflux_ratio: DEFAULT_REFLUX_RATIO,
        reboiler_duty_watts,
        hydraulics: TrayHydraulics::HoldupTimeConstant {
            tau_seconds: TRAY_TAU_S,
        },
        drum_holdup_moles: DRUM_HOLDUP_MOL,
        sump_holdup_moles: SUMP_HOLDUP_MOL,
    }
}
