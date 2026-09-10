//! Rigorous MESH distillation and absorption columns, and their solvers.
//!
//! Pure-Rust port of DWSIM's rigorous-column tier (GPL-3.0), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008-2022
//! Daniel Wagner O. de Medeiros et al. Source files:
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumn.vb` and
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumnSolvers/{BubblePoint,
//! BubblePoint2, SumRates, NewtonRaphson, Tomich, ExternalColumnSolver}.vb`.
//!
//! > **⚠️ Untrusted AI-assisted draft — no human V&V.** Verified against
//! > internal consistency and the ported algorithms' own structure, **not**
//! > validated against experimental distillation data or against DWSIM's own
//! > outputs. Not for nuclear facility operation, reactor control,
//! > safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
//! > the official DWSIM.
//!
//! # What a rigorous column is
//!
//! A counter-current cascade of `N` equilibrium stages, numbered **top to
//! bottom**. Stage `j` receives liquid `L_{j-1}` from above, vapour `V_{j+1}`
//! from below, and an optional feed `F_j`; it emits liquid `L_j` plus a side
//! draw `U_j`, and vapour `V_j` plus a side draw `W_j`, both in equilibrium at
//! `(T_j, P_j)`. Stage `0` is the condenser when the column has one; stage
//! `N-1` is the reboiler when it has one.
//!
//! Each stage carries the **MESH** equations (Wang & Henke's naming):
//!
//! - **M** — component **M**aterial balance, `C` per stage:
//!   `L_{j-1} x_{i,j-1} + V_{j+1} y_{i,j+1} + F_j z_{i,j} − (L_j + U_j) x_{i,j} − (V_j + W_j) y_{i,j} = 0`
//! - **E** — phase-**E**quilibrium relation, `C` per stage:
//!   `y_{i,j} − K_{i,j} x_{i,j} = 0` (Murphree-efficiency-corrected where
//!   `η_j < 1`)
//! - **S** — mole-fraction **S**ummations, 2 per stage:
//!   `Σ_i y_{i,j} − 1 = 0`, `Σ_i x_{i,j} − 1 = 0`
//! - **H** — the ent**H**alpy (energy) balance, 1 per stage.
//!
//! That is `N(2C + 3)` simultaneous non-linear equations. The four solvers here
//! differ only in how they attack them.
//!
//! # Modules
//!
//! ## Shortcut design tier
//!
//! - [`shortcut`] — the **Fenske-Underwood-Gilliland** shortcut column
//!   ([`shortcut::ShortcutColumn`], from upstream `ShortcutColumn.vb`). It
//!   produces the stage count, feed stage, and reflux ratio a rigorous solve
//!   needs as inputs — size with shortcut, then refine with the rigorous
//!   solvers below.
//!
//! ## Data model and assembly
//!
//! - [`model`] — [`Stage`], [`ColumnSpec`], [`CondenserType`], [`ColumnType`],
//!   [`InitialEstimates`], [`ColumnSolverInput`] / [`ColumnSolverOutput`],
//!   [`ColumnError`], and the `uom` type aliases.
//! - [`initial_estimates`] — [`RigorousColumn`], the human-facing assembly type,
//!   and the starting-profile generation (temperature ramp, constant-molar-
//!   overflow flows, per-stage PT flash for compositions).
//! - [`profile`] — [`StageProfile`], the named replacement for upstream's
//!   positional `Object()` result array.
//! - [`specs`] — evaluation of the two end-of-column specifications against a
//!   profile.
//!
//! ## Solvers ([`solver::ColumnSolverMethod`], enum dispatch)
//!
//! - [`bubble_point`] — **Wang-Henke** bubble-point (BP), 1966. Tears on
//!   `(T_j, V_j)`; the reliable default for narrow-boiling distillation.
//! - [`bubble_point2`] — **Modified Wang-Henke** (MBP), DWSIM's second
//!   implementation: no Broyden temperature fallback, and a ~`N/100`-times
//!   tighter convergence gate.
//! - [`sum_rates`] — **Burningham-Otto** sum-rates (SR), 1967. Gets the total
//!   liquid flows from the component-flow sums and the temperatures from a
//!   Newton solve of the energy balances; the method for wide-boiling absorbers.
//! - [`newton_raphson`] — **Naphtali-Sandholm** simultaneous correction (SC),
//!   1971. One Newton solve of the whole `N(2C+1)` system, with the two user
//!   specifications replacing the end-stage energy balances.
//!
//! ## Numerics
//!
//! - [`tridiagonal`] — the **Tomich**/Thomas tridiagonal solver every method
//!   leans on.
//! - [`linalg`] — dense LU with partial pivoting, a finite-difference Jacobian,
//!   and Broyden / damped-Newton root finders. No BLAS, no `ndarray-linalg`
//!   (Android-hostile, banned in this workspace).
//! - [`thermo_bridge`] — [`ColumnThermo`], the mapping from DWSIM's property-
//!   package calls onto [`crate::thermo`].
//!
//! # Units
//!
//! `uom` at the assembly boundary ([`Stage`], [`ColumnSpec`] constructors,
//! [`ColumnSolverOutput`] accessors) with named aliases
//! ([`model::StagePressure`], [`model::StageTemperature`],
//! [`model::MolarFlowRate`], [`model::MolarEnthalpy`],
//! [`model::StageHeatDuty`]); documented raw `f64` in SI inside the solvers, per
//! the crate `CLAUDE.md`: \[K\], \[Pa\], \[mol/s\], \[J/mol\], \[W\], \[-\].
//!
//! # Quick start
//!
//! ```no_run
//! use outram_park_fork_dwsim_libs::columns::{
//!     initial_estimates::RigorousColumn,
//!     model::{ColumnSpec, MolarFlowRate, Stage, StagePressure, StageTemperature},
//!     solver::ColumnSolverMethod,
//! };
//! use outram_park_fork_dwsim_libs::thermo::component::reference;
//! use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;
//! use uom::si::catalytic_activity::katal;
//! use uom::si::f64::MolarEnergy;
//! use uom::si::molar_energy::joule_per_mole;
//! use uom::si::pressure::pascal;
//! use uom::si::thermodynamic_temperature::kelvin;
//!
//! let comps = vec![reference::methane(), reference::ethane()];
//! let p = StagePressure::new::<pascal>(101_325.0);
//! let t = StageTemperature::new::<kelvin>(200.0);
//!
//! // Ten stages; a feed on stage 5.
//! let mut stages: Vec<Stage> = (0..10)
//!     .map(|i| Stage::new(format!("stage {i}"), p, t, 2))
//!     .collect();
//! stages[5] = stages[5].clone().with_feed(
//!     MolarFlowRate::new::<katal>(1.0),
//!     vec![0.5, 0.5],
//!     MolarEnergy::new::<joule_per_mole>(0.0),
//! );
//!
//! let column = RigorousColumn::distillation(
//!     comps,
//!     PropertyPackageModel::PengRobinson,
//!     stages,
//!     ColumnSpec::reflux_ratio(2.0),
//!     ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
//! );
//!
//! let input = column.solver_input().unwrap();
//! let output = ColumnSolverMethod::default().solve(&input).unwrap();
//! println!("bottoms = {:?}", output.bottoms_molar_flow());
//! ```
//!
//! # Excluded DWSIM behavior (module-wide)
//!
//! Each sub-module documents its own exclusions with source line ranges. The
//! blanket exclusions, applied everywhere:
//!
//! - **GUI / editor forms**, icons and bitmaps (`EditingForm_Column`,
//!   `GetIconBitmap`, `DisplayEditForm`, `GetChartModel*`).
//! - **XML/JSON serialization and cloning** (`SaveData`, `LoadData`,
//!   `CloneXML`, `CloneJSON`, the `Parameter` wrapper class).
//! - **Property-grid reflection** (`GetPropertyValue`, `SetPropertyValue`,
//!   `GetPropertyUnit`).
//! - **`Inspector` trace paragraphs** — MathML documentation strings emitted
//!   into DWSIM's HTML inspector, plus the `ColumnSolverConvergenceReport`
//!   `StringBuilder` text reports.
//! - **The .NET external-solver plug-in mechanism**
//!   (`RigorousColumnSolvers/ExternalColumnSolver.vb`, whole file;
//!   `IExternalNonLinearSystemSolver` in `NewtonRaphson.vb:732-735`). Runtime
//!   `dyn` dispatch is forbidden here; the solver set is the closed enum
//!   [`solver::ColumnSolverMethod`].
//! - **The flowsheet object graph** — `ConnectFeed` / `ConnectDistillate` /
//!   `Calculate` / `DeCalculate` / `CheckConnPos` and both
//!   `GetSolverInputData` builders' stream-walking halves. Feeds arrive here as
//!   plain per-stage data.
//! - **`Parallel.For` / `TaskHelper.Run`** per-stage property evaluation.
//!   Identical arithmetic, evaluated serially.
//! - **The liquid-liquid extractor mode** (`llextr` / `llextractor`, the
//!   `L1trials` / `x1trials` seeds, and the `"LL"` K-value flavour).
//! - **Reactive distillation** (`DW_CalcEnthalpyOfReaction`, guarded upstream by
//!   `pp.HasReactivePhase`).
//! - **Side operations** — side strippers and side rectifiers
//!   (`StreamInformation.Behavior::SideOpLiquidProduct` / `SideOpVaporProduct`
//!   survive as enum variants, but nothing consumes them). Simple liquid and
//!   vapour **side draws** *are* supported by every solver.
//!
//! [`Stage`]: model::Stage
//! [`ColumnSpec`]: model::ColumnSpec
//! [`CondenserType`]: model::CondenserType
//! [`ColumnType`]: model::ColumnType
//! [`InitialEstimates`]: model::InitialEstimates
//! [`ColumnSolverInput`]: model::ColumnSolverInput
//! [`ColumnSolverOutput`]: model::ColumnSolverOutput
//! [`ColumnError`]: model::ColumnError
//! [`RigorousColumn`]: initial_estimates::RigorousColumn
//! [`StageProfile`]: profile::StageProfile
//! [`ColumnThermo`]: thermo_bridge::ColumnThermo

pub mod bubble_point;
pub mod bubble_point2;
pub mod dynamic;
pub mod initial_estimates;
pub mod linalg;
pub mod model;
pub mod newton_raphson;
pub mod profile;
pub mod shortcut;
pub mod solver;
pub mod specs;
pub mod sum_rates;
pub mod thermo_bridge;
pub mod tridiagonal;

pub use model::{
    ColumnError, ColumnSolverInput, ColumnSolverOutput, ColumnSpec, ColumnType, CondenserType,
    InitialEstimates, MolarEnthalpy, MolarFlowRate, SpecBasis, SpecType, Stage, StageEfficiency,
    StageHeatDuty, StagePressure, StageTemperature,
};
pub use profile::StageProfile;
pub use shortcut::{
    ShortcutColumn, ShortcutColumnError, ShortcutColumnResult, ShortcutCondenserType, ShortcutFeed,
    ShortcutHeatDuty, UnderwoodMode,
};
pub use solver::ColumnSolverMethod;

#[cfg(test)]
mod tests {
    //! # V&V — the ported rigorous-column solvers on a real column
    //!
    //! ## Methodology
    //!
    //! **Test case.** A benzene(1)/toluene(2) distillation column at
    //! atmospheric pressure — the standard textbook near-ideal binary, chosen
    //! because its behaviour is qualitatively unambiguous (benzene is the light
    //! key everywhere, no azeotrope, no wide-boiling pathology) so a *wrong*
    //! answer is visible without a reference data set. Configuration:
    //!
    //! - 8 equilibrium stages, total condenser (stage 0), reboiler (stage 7).
    //! - Uniform 101.325 kPa, unit stage efficiency.
    //! - Saturated-liquid feed of 1 mol/s equimolar benzene/toluene onto
    //!   stage 4.
    //! - Specifications: reflux ratio `R = 2.0` (condenser end), bottoms molar
    //!   flow `B = 0.5 mol/s` (reboiler end). Both are *directly imposable*, so
    //!   the bubble-point solvers run their inner loop once with no outer
    //!   root-find.
    //! - Property package: `Ideal` (Wilson K-values + Vetere/Watson latent
    //!   heat). The `Ideal` package is used rather than Peng-Robinson because at
    //!   1 atm and `Tr ≈ 0.63` the PR liquid `Z`-root is poorly conditioned for
    //!   these aromatics; the enthalpy route then falls back to the same latent
    //!   heat anyway, so `Ideal` is the honest choice and makes the test
    //!   deterministic.
    //!
    //! Pure-component constants (Tc, Pc, Vc, ω, Tb, Cp0 coefficients) are from
    //! **Poling, Prausnitz & O'Connell, *The Properties of Gases and Liquids*,
    //! 5th ed. (2001), Appendix A** — public literature, no licence restriction.
    //! See [`thermo_bridge::tests::benzene`] / [`thermo_bridge::tests::toluene`].
    //!
    //! **Pass criteria** (all four are properties a correct MESH solve must
    //! have; none is a benchmark value):
    //!
    //! 1. **Overall molar balance closes**:
    //!    `Σ F − D − B − Σ U − Σ W − V_0 = 0` to < 1e-6 mol/s.
    //! 2. **Per-stage compositions normalise**: `|Σ_i x_{i,j} − 1| < 1e-6` and
    //!    likewise for `y`, on every stage.
    //! 3. **Temperature profile is monotone increasing top to bottom** — the
    //!    defining qualitative feature of a distillation column, and the thing
    //!    that breaks first when the energy balance is wired up wrongly.
    //! 4. **Specifications are satisfied**: the achieved reflux ratio and
    //!    bottoms rate match the requested values.
    //!
    //! Plus a **separation check** (benzene enriched in the distillate relative
    //! to the feed, depleted in the bottoms), which is not a tolerance test but
    //! a direction test.
    //!
    //! ## Results
    //!
    //! Recorded per test below. All numbers were measured on **2026-08-11** with
    //! `cargo test --release -p outram-park-fork-dwsim-libs`, on this port at
    //! this commit.
    //!
    //! ## Honest scope
    //!
    //! **Verification, not validation.** Nothing here is compared against
    //! measured distillation data, against DWSIM's own output on the same case,
    //! or against a published benchmark column. The K-values use `k_ij = 0` and
    //! a corresponding-states vapour-pressure correlation, which carries a few
    //! kelvin of error at atmospheric pressure (see
    //! [`thermo_bridge::tests::bubble_temperature_of_benzene_is_near_its_boiling_point`]).
    //! Treat every number below as an internal-consistency record, not as a
    //! validated prediction. AI-assisted draft; no human V&V.

    use super::*;
    use crate::columns::bubble_point::WangHenkeSolver;
    use crate::columns::bubble_point2::{run_tridiagonal, ModifiedWangHenkeSolver};
    use crate::columns::initial_estimates::RigorousColumn;
    use crate::columns::newton_raphson::NaphtaliSandholmSolver;
    use crate::columns::sum_rates::SumRatesSolver;
    use crate::columns::thermo_bridge::tests::{benzene, toluene};
    use crate::columns::thermo_bridge::ColumnThermo;
    use crate::thermo::property_package::PropertyPackageModel;
    use uom::si::catalytic_activity::katal;
    use uom::si::f64::MolarEnergy;
    use uom::si::molar_energy::joule_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    const P_ATM: f64 = 101_325.0;
    const N_STAGES: usize = 8;
    const FEED_STAGE: usize = 4;

    /// Build the benzene/toluene test column described in the module header.
    pub(super) fn benzene_toluene_column() -> RigorousColumn {
        let comps = vec![benzene(), toluene()];
        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        // Saturated-liquid feed at its bubble point.
        let feed_z = [0.5, 0.5];
        let t_feed = thermo
            .bubble_temperature(&feed_z, P_ATM, 365.0, FEED_STAGE)
            .map(|(t, _)| t)
            .unwrap_or(365.0);
        let h_feed = thermo.feed_molar_enthalpy(&feed_z, t_feed, P_ATM, 0.0);

        let p = StagePressure::new::<pascal>(P_ATM);
        let mut stages: Vec<Stage> = (0..N_STAGES)
            .map(|i| {
                let t = StageTemperature::new::<kelvin>(355.0 + 4.0 * i as f64);
                Stage::new(format!("stage {i}"), p, t, 2)
            })
            .collect();
        stages[FEED_STAGE] = stages[FEED_STAGE].clone().with_feed(
            MolarFlowRate::new::<katal>(1.0),
            feed_z.to_vec(),
            MolarEnergy::new::<joule_per_mole>(h_feed),
        );

        RigorousColumn::distillation(
            comps,
            PropertyPackageModel::Ideal,
            stages,
            ColumnSpec::reflux_ratio(2.0),
            ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
        )
        .with_distillate_estimate(MolarFlowRate::new::<katal>(0.5))
        .with_reflux_ratio_estimate(2.0)
    }

    /// Assert the four pass criteria from the module header on a solved column.
    fn assert_mesh_consistency(out: &ColumnSolverOutput, input: &ColumnSolverInput, label: &str) {
        // (1) Overall molar balance.
        let residual = out.molar_balance_residual(&input.feed_flows);
        assert!(
            residual.abs() < 1e-6,
            "{label}: overall molar balance residual = {residual} mol/s"
        );

        // (2) Compositions normalise.
        for (j, (x, y)) in out
            .liquid_compositions
            .iter()
            .zip(out.vapor_compositions.iter())
            .enumerate()
        {
            let sx: f64 = x.iter().sum();
            let sy: f64 = y.iter().sum();
            assert!((sx - 1.0).abs() < 1e-6, "{label}: stage {j} Σx = {sx}");
            assert!((sy - 1.0).abs() < 1e-6, "{label}: stage {j} Σy = {sy}");
            assert!(
                x.iter().all(|v| *v >= -1e-12) && y.iter().all(|v| *v >= -1e-12),
                "{label}: stage {j} has a negative mole fraction"
            );
        }

        // (3) Monotone temperature profile, top to bottom.
        for j in 1..out.stage_temperatures.len() {
            assert!(
                out.stage_temperatures[j] >= out.stage_temperatures[j - 1] - 1e-6,
                "{label}: temperature profile not monotone at stage {j}: {:?}",
                out.stage_temperatures
            );
        }
        assert!(
            out.stage_temperatures
                .iter()
                .all(|t| *t > 250.0 && *t < 600.0),
            "{label}: temperatures out of physical range: {:?}",
            out.stage_temperatures
        );

        // Flows are non-negative and finite.
        assert!(
            out.liquid_flows
                .iter()
                .chain(out.vapor_flows.iter())
                .all(|v| v.is_finite() && *v >= 0.0),
            "{label}: negative or non-finite internal flow"
        );
    }

    /// **Methodology.** The full benzene/toluene column of the module header,
    /// solved by [`WangHenkeSolver`]. Checks all four pass criteria plus the
    /// separation direction, and that both specifications were achieved.
    ///
    /// **Results (2026-08-11, `cargo test --release`):**
    /// converged in **15** inner iterations to
    /// `t_error + vf_error = 6.9349e-7`. Temperature profile (K, stage 0 → 7):
    /// `[354.6, 357.6, 361.0, 364.3, 366.9, 370.3, 374.2, 377.7]` — monotone.
    /// Distillate `D = 0.500000000 mol/s`, bottoms `B = 0.500000000 mol/s`,
    /// reflux ratio `L_0/D = 2.000000000`. Overall molar-balance residual
    /// **exactly 0** mol/s. Liquid benzene profile
    /// `x_benzene = [0.8828, 0.7528, 0.6155, 0.4986, 0.4154, 0.3116, 0.2059,
    /// 0.1173]` against a feed value of 0.5. Condenser duty
    /// `Q_0 = +46 849 W`, reboiler duty `Q_7 = −47 298 W` (sign convention:
    /// positive **into** the stage, so this port's condenser duty is positive
    /// and its reboiler duty negative — see [`model::StageHeatDuty`]).
    ///
    /// **Interpretation.** The mass balance, energy balance, and spec handling
    /// are wired together correctly: the column closes exactly, the profile is
    /// monotone, and benzene is strongly enriched overhead (0.883 vs 0.117).
    /// The *degree* of separation is what a Wilson-K, `k_ij = 0` model gives
    /// for this system — it is **not** claimed to match a measured or
    /// DWSIM-computed column.
    #[test]
    fn wang_henke_solves_benzene_toluene_column() {
        let column = benzene_toluene_column();
        let input = column
            .solver_input()
            .expect("estimate generation must succeed");
        let out = WangHenkeSolver::default()
            .solve_column(&input)
            .expect("Wang-Henke must converge on the benzene/toluene column");

        assert_mesh_consistency(&out, &input, "Wang-Henke");

        // (4) Specifications satisfied.
        let d = out
            .distillate_molar_flow(input.condenser_type)
            .get::<katal>();
        let b = out.bottoms_molar_flow().get::<katal>();
        assert!((b - 0.5).abs() < 1e-6, "bottoms spec: B = {b} mol/s");
        assert!((d - 0.5).abs() < 1e-6, "implied distillate: D = {d} mol/s");
        let rr = out.liquid_flows[0] / d;
        assert!((rr - 2.0).abs() < 1e-6, "reflux-ratio spec: R = {rr}");

        // Separation direction: benzene up, toluene down.
        let x_top_benzene = out.liquid_compositions[0][0];
        let x_bot_benzene = out.liquid_compositions[N_STAGES - 1][0];
        assert!(
            x_top_benzene > 0.5,
            "benzene must be enriched overhead, got {x_top_benzene}"
        );
        assert!(
            x_bot_benzene < 0.5,
            "benzene must be depleted in the bottoms, got {x_bot_benzene}"
        );
        assert!(x_top_benzene > x_bot_benzene);
    }

    /// **Methodology.** The same column solved by [`ModifiedWangHenkeSolver`],
    /// whose only substantive difference is a convergence gate `N/100` times
    /// tighter. Same four pass criteria. The two solvers must agree on the
    /// converged profile to well within the looser solver's own tolerance.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** converged in **13**
    /// inner iterations to `8.1377e-6` (against Wang-Henke's 15 iterations to
    /// `6.9349e-7` — the two gates differ, so the iteration counts are not
    /// directly comparable). Stage temperatures agree with the Wang-Henke
    /// result to a maximum of **1.298e-3 K** over the 8 stages; distillate and
    /// bottoms rates agree to `< 1e-9 mol/s`.
    ///
    /// **Interpretation.** The two independently-ported inner loops converge to
    /// the same fixed point, which is the strongest internal cross-check
    /// available without a reference implementation to compare against.
    #[test]
    fn modified_wang_henke_agrees_with_wang_henke() {
        let column = benzene_toluene_column();
        let input = column.solver_input().unwrap();

        let bp = WangHenkeSolver::default().solve_column(&input).unwrap();
        let mbp = ModifiedWangHenkeSolver::default()
            .solve_column(&input)
            .expect("Modified Wang-Henke must converge on the benzene/toluene column");

        assert_mesh_consistency(&mbp, &input, "Modified Wang-Henke");

        for j in 0..N_STAGES {
            assert!(
                (bp.stage_temperatures[j] - mbp.stage_temperatures[j]).abs() < 0.05,
                "stage {j}: BP T = {}, MBP T = {}",
                bp.stage_temperatures[j],
                mbp.stage_temperatures[j]
            );
        }
        let db = bp
            .distillate_molar_flow(input.condenser_type)
            .get::<katal>();
        let dm = mbp
            .distillate_molar_flow(input.condenser_type)
            .get::<katal>();
        assert!((db - dm).abs() < 1e-9, "D: BP = {db}, MBP = {dm}");
    }

    /// **Methodology.** [`run_tridiagonal`] (upstream's `RunTridiagonal`) is a
    /// single non-iterating consistency sweep, not a solve. It must therefore
    /// (a) succeed on the generated starting profile, (b) leave the stage
    /// temperatures **exactly** unchanged — it has no temperature update — and
    /// (c) return normalised vapour compositions.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** returned
    /// `iterations = 1`; all 8 stage temperatures bit-identical to the input
    /// ramp `[365.5 … 371.7 K]`; every `Σ_i y_{i,j} = 1` to < 1e-12.
    #[test]
    fn run_tridiagonal_is_a_single_consistency_sweep() {
        let column = benzene_toluene_column();
        let input = column.solver_input().unwrap();
        let thermo = ColumnThermo::new(input.components.clone(), input.package);

        let pf = run_tridiagonal(&input, &thermo, &input.condenser_spec, &input.reboiler_spec)
            .expect("RunTridiagonal must succeed on a generated starting profile");

        assert_eq!(pf.iterations, 1);
        for j in 0..N_STAGES {
            assert_eq!(
                pf.temperatures[j], input.stage_temperatures[j],
                "RunTridiagonal must not change temperatures (stage {j})"
            );
            let sy: f64 = pf.vapor_compositions[j].iter().sum();
            assert!((sy - 1.0).abs() < 1e-12, "stage {j}: Σy = {sy}");
        }
    }

    /// **Methodology.** Smoke test for [`SumRatesSolver`] on the same column.
    /// Sum-rates is designed for wide-boiling absorbers, not narrow-boiling
    /// distillation, so this test does **not** require convergence — it
    /// requires that the solver runs the full ported path (tridiagonal mass
    /// balance, sum-rates flow update, `dH/dT` Newton step on the temperatures)
    /// and either converges to a consistent profile or reports a typed
    /// [`ColumnError`], never panics or silently returns nonsense. When it does
    /// converge, the four MESH consistency criteria are asserted.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** the solver returned
    /// `ColumnError::InvalidProfile { stage: 5, detail: "converged to an
    /// invalid temperature (T = -1180.2028129624227 K)" }` — the Newton step on
    /// the stage energy balances (`T_j += 0.7 ΔT_j`, upstream's uncapped
    /// update, `SumRates.vb:611`) overshoots into negative absolute temperature
    /// from this starting profile. No panic and no non-finite output; the
    /// guard fires and a typed error is returned.
    ///
    /// The same failure occurs on a 6-stage benzene/toluene **absorber**
    /// (lean-solvent liquid feed on stage 0, vapour feed on stage 5, zero
    /// duties, [`ColumnType::AbsorptionColumn`]), with and without
    /// [`SumRatesSolver::relax_temperature_updates`] enabled — measured
    /// separately on 2026-08-11, `InvalidProfile` at stages 3 and 4
    /// respectively.
    ///
    /// **Status: compiles and runs the full ported algorithm; NOT demonstrated
    /// to converge on any test case.**
    ///
    /// **Interpretation.** The sum-rates port is exercised end-to-end (the
    /// tridiagonal mass balance, the `L_j ← L_j Σ_i l_{i,j}` sum-rates step,
    /// the central-difference `dH/dT`, and the tridiagonal Newton solve for
    /// `ΔT` all execute), but it is **unvalidated** and currently unusable. The
    /// likely cause is that upstream's uncapped `0.7 ΔT` update (its computed
    /// step cap `af = 5/max|ΔT|` is dead code — see
    /// [`crate::columns::sum_rates`]'s "Faithfully ported upstream quirks") is
    /// only stable from DWSIM's own much better initial estimates. Getting it
    /// to converge is proposed as follow-up work.
    #[test]
    fn sum_rates_runs_the_full_algorithm() {
        let column = benzene_toluene_column();
        let input = column.solver_input().unwrap();
        match SumRatesSolver::default().solve_column(&input) {
            Ok(out) => assert_mesh_consistency(&out, &input, "sum-rates"),
            Err(e) => {
                // A typed error is an acceptable outcome for this case; a panic
                // or a non-finite profile is not.
                let msg = format!("{e}");
                assert!(
                    !msg.is_empty(),
                    "sum-rates must report a descriptive error, got {e:?}"
                );
            }
        }
    }

    /// **Methodology.** The same benzene/toluene column solved by
    /// [`NaphtaliSandholmSolver`] with the upstream warm start (one Wang-Henke
    /// iteration) enabled. Exercises the whole simultaneous-correction path —
    /// the scaled variable packing, the `[H, M, E]` residual assembly with the
    /// two specifications replacing the end-stage energy balances, and the
    /// three-attempt Broyden/Newton cascade — and requires convergence, the
    /// four MESH consistency criteria, and agreement with the Wang-Henke
    /// solution (a genuinely independent method).
    ///
    /// **Results (2026-08-11, `cargo test --release`):** the residual vector is
    /// 8 stages × (2·2 + 1) = **40** equations. The solver **converged**, in 6
    /// root-finder iterations, to `Σ f² = 6.2180e-7` (below the `1e-5`
    /// tolerance), giving the temperature profile
    /// `[354.6, 357.6, 361.0, 364.3, 366.9, 370.3, 374.2, 377.7] K` and
    /// `x_benzene = [0.8827, 0.7529, 0.6156, 0.4987, 0.4154, 0.3116, 0.2059,
    /// 0.1173]` — agreeing with the Wang-Henke solution to **< 0.1 K** on every
    /// stage and **< 2e-4** on every liquid mole fraction, with
    /// `D = B = 0.500000000 mol/s` and a molar-balance residual of
    /// `1.11e-16 mol/s`.
    ///
    /// **Status: converges on the benzene/toluene test case**, agreeing with
    /// both bubble-point solvers.
    ///
    /// **Interpretation.** Three independently-ported solvers — Wang-Henke,
    /// Modified Wang-Henke, and Naphtali-Sandholm, which share only the
    /// tridiagonal kernel and the thermo bridge — converge to the same profile.
    /// That is the strongest internal cross-check available without a reference
    /// implementation to compare against. It is still **not** validation
    /// against measured data.
    #[test]
    fn naphtali_sandholm_solves_benzene_toluene_column() {
        let column = benzene_toluene_column();
        let input = column.solver_input().unwrap();
        let bp = WangHenkeSolver::default().solve_column(&input).unwrap();
        let ns = NaphtaliSandholmSolver::with_warm_start()
            .solve_column(&input)
            .expect("Naphtali-Sandholm must converge on the benzene/toluene column");
        assert_mesh_consistency(&ns, &input, "Naphtali-Sandholm");

        for j in 0..N_STAGES {
            assert!(
                (bp.stage_temperatures[j] - ns.stage_temperatures[j]).abs() < 0.1,
                "stage {j}: BP T = {}, NS T = {}",
                bp.stage_temperatures[j],
                ns.stage_temperatures[j]
            );
            assert!(
                (bp.liquid_compositions[j][0] - ns.liquid_compositions[j][0]).abs() < 2e-3,
                "stage {j}: BP x_benzene = {}, NS x_benzene = {}",
                bp.liquid_compositions[j][0],
                ns.liquid_compositions[j][0]
            );
        }
    }

    /// **Methodology.** [`ColumnSolverMethod`] must dispatch to the right
    /// solver: each variant's `name()` and `description()` must match the
    /// wrapped solver's, and the default must be Wang-Henke (upstream's
    /// default, `RigorousColumn.vb:1913`). Also exercises `solve` through the
    /// enum on the benzene/toluene column.
    ///
    /// **Results (2026-08-11):** all four names/descriptions dispatch
    /// correctly; the default variant solves the column with the same result as
    /// calling [`WangHenkeSolver`] directly (distillate rates agree to < 1e-12).
    #[test]
    fn solver_enum_dispatches_correctly() {
        assert_eq!(
            ColumnSolverMethod::default().name(),
            WangHenkeSolver::name()
        );
        assert_eq!(
            ColumnSolverMethod::SumRates(SumRatesSolver::default()).name(),
            SumRatesSolver::name()
        );
        assert_eq!(
            ColumnSolverMethod::ModifiedWangHenke(ModifiedWangHenkeSolver::default()).description(),
            ModifiedWangHenkeSolver::description()
        );
        assert_eq!(
            ColumnSolverMethod::NaphtaliSandholm(NaphtaliSandholmSolver::default()).description(),
            NaphtaliSandholmSolver::description()
        );

        let column = benzene_toluene_column();
        let input = column.solver_input().unwrap();
        let via_enum = ColumnSolverMethod::default().solve(&input).unwrap();
        let direct = WangHenkeSolver::default().solve_column(&input).unwrap();
        let a = via_enum
            .distillate_molar_flow(input.condenser_type)
            .get::<katal>();
        let b = direct
            .distillate_molar_flow(input.condenser_type)
            .get::<katal>();
        assert!((a - b).abs() < 1e-12);
    }

    /// **Methodology.** The estimate generator must produce a well-shaped,
    /// physically plausible starting profile: `validate_shape` passes, the
    /// temperature ramp is monotone, every composition normalises, and the
    /// mixed feed composition is the flow-weighted average of the stage feeds.
    ///
    /// **Results (2026-08-11):** shape validation passes for the 8-stage /
    /// 2-component column; the generated ramp runs
    /// `365.5 K → 371.7 K` monotonically (bubble point of the equimolar feed at
    /// the top, dew point at the bottom); mixed feed `z_m = [0.5, 0.5]` exactly.
    #[test]
    fn initial_estimates_are_well_formed() {
        let column = benzene_toluene_column();
        let zm = column.mixed_feed_composition();
        assert!((zm[0] - 0.5).abs() < 1e-12 && (zm[1] - 0.5).abs() < 1e-12);

        let input = column.solver_input().unwrap();
        input.validate_shape().unwrap();

        for j in 1..N_STAGES {
            assert!(
                input.stage_temperatures[j] >= input.stage_temperatures[j - 1] - 1e-9,
                "generated temperature ramp not monotone at stage {j}: {:?}",
                input.stage_temperatures
            );
        }
        for j in 0..N_STAGES {
            let sx: f64 = input.liquid_compositions[j].iter().sum();
            let sy: f64 = input.vapor_compositions[j].iter().sum();
            assert!((sx - 1.0).abs() < 1e-9, "stage {j}: Σx = {sx}");
            assert!((sy - 1.0).abs() < 1e-9, "stage {j}: Σy = {sy}");
            assert!(input.k_values[j].iter().all(|k| k.is_finite() && *k > 0.0));
        }
    }

    /// **Methodology.** A column whose specification is a *product purity*
    /// (a [`SpecType::ComponentFraction`] on the distillate) cannot be imposed
    /// directly by the bubble-point inner loop, so it must drive the outer
    /// Broyden root-find on the reflux ratio. This test exercises that path —
    /// the branch of `BubblePoint.vb:334-517` — and asserts the achieved
    /// distillate purity matches the request when the solve succeeds.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** requesting a
    /// distillate benzene mole fraction of **0.90** (reachable at 8 stages,
    /// where the `R = 2` base case reaches 0.8828 and `R = 3` reaches 0.9063),
    /// the outer Broyden loop converged to a reflux ratio of **2.655916**,
    /// giving `x_0,benzene = 0.89999959` — the spec met to **4.1e-7**. The
    /// returned best profile took 17 inner Wang-Henke iterations and satisfies
    /// all four MESH consistency criteria.
    ///
    /// Measured separately on the same case: target 0.92 → `R = 4.134647`,
    /// achieved 0.91999322; target 0.95 → `R = 24.765360`, achieved
    /// 0.94999922. A target of 0.60 is **not** reachable on this column (even
    /// `R = 0.5` gives 0.7757) and the outer loop correctly reports
    /// [`ColumnError::NotConverged`] rather than returning a wrong answer.
    ///
    /// **Interpretation.** The two-level iteration (outer spec root-find over
    /// the inner Wang-Henke loop) works end to end.
    #[test]
    fn outer_spec_loop_meets_a_purity_specification() {
        let mut column = benzene_toluene_column();
        column.condenser_spec = ColumnSpec::component_mole_fraction(0, 0.90);
        column.condenser_spec.initial_estimate = Some(2.0);

        let input = column.solver_input().unwrap();
        match WangHenkeSolver::default().solve_column(&input) {
            Ok(out) => {
                assert_mesh_consistency(&out, &input, "outer purity spec");
                let achieved = out.liquid_compositions[0][0];
                assert!(
                    (achieved - 0.90).abs() < 1e-4,
                    "distillate benzene fraction = {achieved}, requested 0.90"
                );
                assert!(
                    (out.condenser_spec.calculated_value - achieved).abs() < 1e-12,
                    "calculated_value must echo the achieved purity"
                );
            }
            Err(e) => panic!("outer spec loop failed: {e}"),
        }
    }
}

#[cfg(test)]
mod column_type_tests {
    //! # V&V — the three non-distillation [`ColumnType`] variants, through the
    //! public constructors (GitHub issue #103, bead `op-0njm`)
    //!
    //! ## Methodology
    //!
    //! Until 2026-09-10 [`RigorousColumn::distillation`] was the only
    //! constructor, so [`ColumnType::AbsorptionColumn`],
    //! [`ColumnType::ReboiledAbsorber`] and [`ColumnType::RefluxedAbsorber`]
    //! were unreachable from the public API even though every solver carries
    //! code paths for them. These tests build one column of each type through
    //! the new constructors ([`RigorousColumn::absorption`],
    //! [`RigorousColumn::reboiled_absorber`],
    //! [`RigorousColumn::refluxed_absorber`]), solve it, and check the same
    //! MESH-consistency properties the distillation tests above use — plus the
    //! **duty convention** each type promises, which is the thing the issue
    //! exists to make explicit.
    //!
    //! All three use the benzene(1)/toluene(2) binary at 101.325 kPa with the
    //! `Ideal` package, for the reasons given in the module header of
    //! [`super::tests`] — the pure-component constants are Poling, Prausnitz &
    //! O'Connell (2001), Appendix A, public literature.
    //!
    //! | Test column | Feeds | Spec(s) | End duties |
    //! |---|---|---|---|
    //! | Reboiled absorber (stripper), 8 stages | 1 mol/s equimolar saturated **liquid** on stage 0 | `B = 0.5 mol/s` | `Q_7` back-calculated; `Q_0` user input (0) |
    //! | Refluxed absorber, 8 stages, total condenser | 1 mol/s equimolar saturated **vapour** on stage 7 | reflux ratio `R = 2` | `Q_0` back-calculated; `Q_7` user input (0) |
    //! | Absorber, 6 stages | 1 mol/s toluene liquid at 320 K on stage 0; 1 mol/s 90/10 benzene/toluene saturated vapour on stage 5 | none | both user input (0) |
    //!
    //! **Pass criteria**, per solved column: overall molar balance closes to
    //! `< 1e-6 mol/s`; every stage's `x` and `y` normalise to `< 1e-6`; all
    //! flows finite and non-negative; the duty convention holds (a
    //! back-calculated end duty is non-zero, a user-input end duty is returned
    //! exactly as given); the imposed spec is met to `< 1e-6`; and the
    //! separation runs in the physically right direction. Where a solver is
    //! *expected* to fail on a variant, the test accepts a typed
    //! [`ColumnError`] and asserts consistency only if it happens to succeed —
    //! the same pattern as [`super::tests::sum_rates_runs_the_full_algorithm`] —
    //! so an improved solver does not break the suite.
    //!
    //! ## Results
    //!
    //! Recorded per test below. All numbers measured **2026-09-10** with
    //! `cargo test --release -p outram-park-fork-dwsim-libs`.
    //!
    //! ## Honest scope
    //!
    //! **Verification, not validation** — internal consistency of the ported
    //! algorithms, no comparison against measured data, DWSIM's own output, or a
    //! published absorber/stripper benchmark. Two solver-side limitations were
    //! found while writing these tests and are **recorded, not fixed** (the
    //! task was API exposure only): see
    //! [`refluxed_absorber_solves_through_the_public_api`] for both. AI-assisted
    //! draft; no human V&V.

    use super::*;
    use crate::columns::bubble_point::WangHenkeSolver;
    use crate::columns::bubble_point2::ModifiedWangHenkeSolver;
    use crate::columns::initial_estimates::RigorousColumn;
    use crate::columns::newton_raphson::NaphtaliSandholmSolver;
    use crate::columns::thermo_bridge::tests::{benzene, toluene};
    use crate::columns::thermo_bridge::ColumnThermo;
    use crate::thermo::property_package::PropertyPackageModel;
    use crate::thermo::saturation::dew_temperature;
    use uom::si::catalytic_activity::katal;
    use uom::si::f64::MolarEnergy;
    use uom::si::molar_energy::joule_per_mole;
    use uom::si::power::watt;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    const P_ATM: f64 = 101_325.0;

    /// `n` bare stages at 101.325 kPa with a linear temperature guess
    /// `t0 + dt * i` \[K\].
    fn bare_stages(n: usize, t0: f64, dt: f64) -> Vec<Stage> {
        let p = StagePressure::new::<pascal>(P_ATM);
        (0..n)
            .map(|i| {
                let t = StageTemperature::new::<kelvin>(t0 + dt * i as f64);
                Stage::new(format!("stage {i}"), p, t, 2)
            })
            .collect()
    }

    /// Molar enthalpy \[J/mol\] of a feed of composition `z` at its own
    /// saturation point: bubble point for `beta = 0`, dew point for `beta = 1`.
    fn saturated_feed_enthalpy(thermo: &ColumnThermo, z: &[f64], beta: f64) -> f64 {
        let comps = thermo.components();
        let t = if beta >= 1.0 {
            dew_temperature(comps, z, P_ATM, PropertyPackageModel::Ideal)
                .expect("dew point")
                .temperature
        } else {
            thermo
                .bubble_temperature(z, P_ATM, 365.0, 0)
                .expect("bubble point")
                .0
        };
        thermo.feed_molar_enthalpy(z, t, P_ATM, beta)
    }

    /// Energy residual of the **bottom** stage, `in − out` \[W\], evaluated at
    /// the returned profile with the enthalpy model the solver used:
    /// `L_{ns-1} h_L,ns-1 + F_ns h_F,ns + Q_ns,into − L_ns h_L,ns − V_ns h_V,ns`.
    ///
    /// Zero (to round-off in the enthalpy model) when the solver enforced that
    /// stage's energy balance. `Q_ns` is read from the *input* (the given
    /// duty), with `Stage::heat_duty`'s documented positive-into sign.
    fn bottom_stage_energy_residual(out: &ColumnSolverOutput, input: &ColumnSolverInput) -> f64 {
        let thermo = ColumnThermo::new(input.components.clone(), input.package);
        let ns = input.number_of_stages - 1;
        let hl = |i: usize| {
            thermo.liquid_molar_enthalpy(
                &out.liquid_compositions[i],
                out.stage_temperatures[i],
                input.stage_pressures[i],
            )
        };
        let hv = |i: usize| {
            thermo.vapor_molar_enthalpy(
                &out.vapor_compositions[i],
                out.stage_temperatures[i],
                input.stage_pressures[i],
            )
        };
        out.liquid_flows[ns - 1] * hl(ns - 1)
            + input.feed_flows[ns] * input.feed_enthalpies[ns]
            + input.stage_heats[ns]
            - out.liquid_flows[ns] * hl(ns)
            - out.vapor_flows[ns] * hv(ns)
    }

    /// The MESH-consistency checks shared by all three variants: overall molar
    /// balance, per-stage normalisation, finite non-negative flows. Unlike
    /// [`super::tests`]' helper this does **not** demand a monotone temperature
    /// profile — an adiabatic absorber legitimately runs hottest where the
    /// absorption heat is released, which need not be the bottom.
    fn assert_balances(out: &ColumnSolverOutput, input: &ColumnSolverInput, label: &str) {
        let residual = out.molar_balance_residual(&input.feed_flows);
        assert!(
            residual.abs() < 1e-6,
            "{label}: overall molar balance residual = {residual} mol/s"
        );
        for (j, (x, y)) in out
            .liquid_compositions
            .iter()
            .zip(out.vapor_compositions.iter())
            .enumerate()
        {
            let sx: f64 = x.iter().sum();
            let sy: f64 = y.iter().sum();
            assert!((sx - 1.0).abs() < 1e-6, "{label}: stage {j} Σx = {sx}");
            assert!((sy - 1.0).abs() < 1e-6, "{label}: stage {j} Σy = {sy}");
            assert!(
                x.iter().all(|v| *v >= -1e-12) && y.iter().all(|v| *v >= -1e-12),
                "{label}: stage {j} has a negative mole fraction"
            );
        }
        assert!(
            out.liquid_flows
                .iter()
                .chain(out.vapor_flows.iter())
                .chain(out.liquid_side_draws.iter())
                .all(|v| v.is_finite() && *v >= 0.0),
            "{label}: negative or non-finite flow"
        );
        assert!(
            out.stage_temperatures
                .iter()
                .all(|t| *t > 250.0 && *t < 600.0),
            "{label}: temperatures out of physical range: {:?}",
            out.stage_temperatures
        );
    }

    fn assert_monotone_increasing(out: &ColumnSolverOutput, label: &str) {
        for j in 1..out.stage_temperatures.len() {
            assert!(
                out.stage_temperatures[j] >= out.stage_temperatures[j - 1] - 1e-6,
                "{label}: temperature profile not monotone at stage {j}: {:?}",
                out.stage_temperatures
            );
        }
    }

    /// The 8-stage benzene/toluene **stripper** of the module table.
    fn stripper() -> RigorousColumn {
        let comps = vec![benzene(), toluene()];
        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        let z = [0.5, 0.5];
        let h_feed = saturated_feed_enthalpy(&thermo, &z, 0.0);
        let mut stages = bare_stages(8, 355.0, 4.0);
        stages[0] = stages[0].clone().with_feed(
            MolarFlowRate::new::<katal>(1.0),
            z.to_vec(),
            MolarEnergy::new::<joule_per_mole>(h_feed),
        );
        RigorousColumn::reboiled_absorber(
            comps,
            PropertyPackageModel::Ideal,
            stages,
            ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
        )
    }

    /// The 8-stage benzene/toluene **refluxed absorber** of the module table.
    fn refluxed_absorber() -> RigorousColumn {
        let comps = vec![benzene(), toluene()];
        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        let z = [0.5, 0.5];
        let h_feed = saturated_feed_enthalpy(&thermo, &z, 1.0);
        let mut stages = bare_stages(8, 355.0, 4.0);
        stages[7] = stages[7].clone().with_feed(
            MolarFlowRate::new::<katal>(1.0),
            z.to_vec(),
            MolarEnergy::new::<joule_per_mole>(h_feed),
        );
        RigorousColumn::refluxed_absorber(
            comps,
            PropertyPackageModel::Ideal,
            stages,
            ColumnSpec::reflux_ratio(2.0),
        )
    }

    /// The 6-stage benzene/toluene **absorber** of the module table.
    fn absorber() -> RigorousColumn {
        let comps = vec![benzene(), toluene()];
        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        // Lean toluene, well subcooled (320 K against a 383.8 K boiling point).
        let z_l = [0.0, 1.0];
        let h_l = thermo.feed_molar_enthalpy(&z_l, 320.0, P_ATM, 0.0);
        // Benzene-rich saturated vapour.
        let z_v = [0.9, 0.1];
        let h_v = saturated_feed_enthalpy(&thermo, &z_v, 1.0);
        let mut stages = bare_stages(6, 358.0, 2.0);
        stages[0] = stages[0].clone().with_feed(
            MolarFlowRate::new::<katal>(1.0),
            z_l.to_vec(),
            MolarEnergy::new::<joule_per_mole>(h_l),
        );
        stages[5] = stages[5].clone().with_feed(
            MolarFlowRate::new::<katal>(1.0),
            z_v.to_vec(),
            MolarEnergy::new::<joule_per_mole>(h_v),
        );
        RigorousColumn::absorption(comps, PropertyPackageModel::Ideal, stages)
    }

    /// **Methodology.** Every constructor must apply its documented duty
    /// convention when it builds the [`ColumnSolverInput`]: an end duty that is
    /// to be back-calculated is zeroed, a user-input end duty is passed through
    /// unchanged, the placeholder/mirror specs carry the stage duty, and the
    /// stage-0 liquid draw is seeded with the distillate estimate only where
    /// there is a liquid distillate. For the refluxed absorber the generated
    /// `L_ns` estimate must also close the overall balance with that distillate
    /// (`L_ns = F − D`), because that solver family reads `B = L_ns` from the
    /// estimate — see the next test. Checked with distinguishable duties
    /// (`−100 W` on stage 0, `+200 W` on the bottom stage) on all four types.
    ///
    /// **Result (2026-09-10):** all four types behave as documented:
    /// distillation zeroes both ends and seeds `LSS_0 = 0.5`; absorption keeps
    /// `[−100, +200]`, mirrors both into `HeatDuty` specs, `LSS_0 = 0`,
    /// `FullReflux`; reboiled absorber keeps `−100`, zeroes the bottom, mirrors
    /// `−100` into the condenser-end spec, `LSS_0 = 0`, `FullReflux`; refluxed
    /// absorber zeroes the top, keeps `+200`, mirrors `+200` into the
    /// reboiler-end spec, `LSS_0 = 0.5`, `TotalCondenser`, and
    /// `L_7,est = 0.5 = F − D_est`.
    #[test]
    fn constructors_apply_the_documented_duty_conventions() {
        let comps = vec![benzene(), toluene()];
        let with_duties = |mut stages: Vec<Stage>| {
            let n = stages.len();
            stages[0] = stages[0]
                .clone()
                .with_heat_duty(StageHeatDuty::new::<watt>(-100.0));
            stages[n - 1] = stages[n - 1]
                .clone()
                .with_heat_duty(StageHeatDuty::new::<watt>(200.0));
            stages
        };

        // Distillation: both ends back-calculated, liquid distillate seeded.
        let c = RigorousColumn::distillation(
            comps.clone(),
            PropertyPackageModel::Ideal,
            with_duties(super::tests::benzene_toluene_column().stages),
            ColumnSpec::reflux_ratio(2.0),
            ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
        );
        let input = c.solver_input().unwrap();
        assert_eq!(input.column_type, ColumnType::DistillationColumn);
        assert_eq!(input.stage_heats[0], 0.0);
        assert_eq!(input.stage_heats[7], 0.0);
        assert!((input.liquid_side_draws[0] - 0.5).abs() < 1e-12);

        // Absorption: both ends user input, mirrored into HeatDuty specs.
        let c = RigorousColumn::absorption(
            comps.clone(),
            PropertyPackageModel::Ideal,
            with_duties(absorber().stages),
        );
        assert_eq!(c.condenser_type, CondenserType::FullReflux);
        assert_eq!(c.condenser_spec.spec_type, SpecType::HeatDuty);
        assert_eq!(c.condenser_spec.value, -100.0);
        assert_eq!(c.reboiler_spec.spec_type, SpecType::HeatDuty);
        assert_eq!(c.reboiler_spec.value, 200.0);
        let input = c.solver_input().unwrap();
        assert_eq!(input.column_type, ColumnType::AbsorptionColumn);
        assert_eq!(input.stage_heats[0], -100.0);
        assert_eq!(input.stage_heats[5], 200.0);
        assert_eq!(input.liquid_side_draws[0], 0.0);

        // Reboiled absorber: bottom back-calculated, top user input.
        let c = RigorousColumn::reboiled_absorber(
            comps.clone(),
            PropertyPackageModel::Ideal,
            with_duties(stripper().stages),
            ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
        );
        assert_eq!(c.condenser_type, CondenserType::FullReflux);
        assert_eq!(c.condenser_spec.spec_type, SpecType::HeatDuty);
        assert_eq!(c.condenser_spec.value, -100.0);
        assert_eq!(c.reboiler_spec.spec_type, SpecType::ProductMolarFlowRate);
        let input = c.solver_input().unwrap();
        assert_eq!(input.column_type, ColumnType::ReboiledAbsorber);
        assert_eq!(input.stage_heats[0], -100.0);
        assert_eq!(input.stage_heats[7], 0.0);
        assert_eq!(input.liquid_side_draws[0], 0.0);

        // Refluxed absorber: top back-calculated, bottom user input, liquid
        // distillate seeded AND the L estimate closes the balance with it.
        let c = RigorousColumn::refluxed_absorber(
            comps,
            PropertyPackageModel::Ideal,
            with_duties(refluxed_absorber().stages),
            ColumnSpec::reflux_ratio(2.0),
        );
        assert_eq!(c.condenser_type, CondenserType::TotalCondenser);
        assert_eq!(c.condenser_spec.spec_type, SpecType::StreamRatio);
        assert_eq!(c.reboiler_spec.spec_type, SpecType::HeatDuty);
        assert_eq!(c.reboiler_spec.value, 200.0);
        let input = c.solver_input().unwrap();
        assert_eq!(input.column_type, ColumnType::RefluxedAbsorber);
        assert_eq!(input.stage_heats[0], 0.0);
        assert_eq!(input.stage_heats[7], 200.0);
        assert!((input.liquid_side_draws[0] - 0.5).abs() < 1e-12);
        assert!(
            (input.liquid_flows[7] - 0.5).abs() < 1e-9,
            "refluxed absorber L_ns estimate must be F − D_est = 0.5, got {}",
            input.liquid_flows[7]
        );
    }

    /// **Methodology.** The stripper of the module table, built with
    /// [`RigorousColumn::reboiled_absorber`] and solved by all three usable
    /// solvers ([`WangHenkeSolver`], [`ModifiedWangHenkeSolver`],
    /// [`NaphtaliSandholmSolver`] with warm start). Each must converge and
    /// satisfy [`assert_balances`], a monotone-increasing temperature profile
    /// (liquid enters cold at the top, reboiler at the bottom), the bottoms
    /// spec `B = 0.5 mol/s` to `< 1e-6`, and the duty convention: `LSS_0 = 0`
    /// (no liquid distillate), `Q_0` returned exactly as given (`0 W`, user
    /// input), `Q_7 ≠ 0` (back-calculated reboiler duty). Separation direction:
    /// benzene, the light key, must be richer in the overhead vapour `y_0` than
    /// in the bottoms `x_7`, and richer than the 0.5 feed. The three solvers
    /// must agree to `< 0.1 K` per stage and `< 1e-6 mol/s` on `D`.
    ///
    /// **Results (2026-09-10, `cargo test --release`):**
    ///
    /// | Solver | Iterations | Final error | `D = V_0` | `B` | `Q_7` (internal sign) |
    /// |---|---|---|---|---|---|
    /// | Wang-Henke | 15 | `3.7190e-7` | 0.500000 | 0.500000 | `−16 185 W` |
    /// | Modified Wang-Henke | 13 | `6.4989e-6` | 0.500000 | 0.500000 | `−16 185 W` |
    /// | Naphtali-Sandholm (warm) | 4 | `2.4576e-7` | 0.500000 | 0.500000 | `−16 187 W` |
    ///
    /// Wang-Henke profile, stage 0 → 7: `T = [364.3, 364.3, 364.4, 364.6,
    /// 364.9, 365.8, 367.6, 371.0] K`, `x_benzene = [0.4995, 0.4985, 0.4960,
    /// 0.4904, 0.4777, 0.4499, 0.3931, 0.2922]`, overhead `y_0,benzene =
    /// 0.7078`; `V = [0.500, 0.500, 0.4998, …, 0.4907]`, `L = [1.000, 0.9998,
    /// …, 0.9907, 0.500]`; `LSS = 0` on every stage; molar-balance residual
    /// exactly `0`. Naphtali-Sandholm without warm start also converges (13
    /// iterations, `3.0509e-6`, `D = 0.500020`); sum-rates fails with
    /// `InvalidProfile` (`T = −60.5 K` at stage 1), as on every other column
    /// in this crate.
    ///
    /// **Interpretation.** The `ReboiledAbsorber` path — `LSS_0 = 0`, `V_0 =
    /// ΣF − B`, stripping-ratio bookkeeping at the top, reboiler duty from the
    /// bottom-stage balance — is exercised end to end through the public
    /// constructor and three independently-ported solvers agree on the answer.
    /// The reboiler duty's sign follows the Wang-Henke internal convention
    /// (positive = removed from the stage; `−16.2 kW` is 16.2 kW *added*), the
    /// same convention the distillation test above records for `Q_7`. The
    /// separation is weak (a stripper with `B/F = 0.5` and no reflux) but in
    /// the right direction. Not compared with any reference stripper.
    #[test]
    fn reboiled_absorber_solves_through_the_public_api() {
        let column = stripper();
        assert_eq!(column.column_type, ColumnType::ReboiledAbsorber);
        let input = column.solver_input().expect("estimate generation");

        let check = |out: &ColumnSolverOutput, label: &str| {
            assert_balances(out, &input, label);
            assert_monotone_increasing(out, label);
            let b = out.bottoms_molar_flow().get::<katal>();
            assert!((b - 0.5).abs() < 1e-6, "{label}: bottoms spec B = {b}");
            let d = out
                .distillate_molar_flow(input.condenser_type)
                .get::<katal>();
            assert!((d - 0.5).abs() < 1e-6, "{label}: overhead vapour D = {d}");
            assert_eq!(
                out.liquid_side_draws[0], 0.0,
                "{label}: no liquid distillate"
            );
            // Duty convention: Q_0 user input (0), Q_7 back-calculated.
            assert_eq!(
                out.stage_heats[0], 0.0,
                "{label}: Q_0 must be the given 0 W"
            );
            assert!(
                out.stage_heats[7].abs() > 1.0e3,
                "{label}: reboiler duty must be back-calculated, got {} W",
                out.stage_heats[7]
            );
            // Separation direction.
            let y_top = out.vapor_compositions[0][0];
            let x_bot = out.liquid_compositions[7][0];
            assert!(
                y_top > 0.5 && x_bot < 0.5 && y_top > x_bot,
                "{label}: y_0 = {y_top}, x_7 = {x_bot}"
            );
        };

        let bp = WangHenkeSolver::default()
            .solve_column(&input)
            .expect("Wang-Henke must converge on the stripper");
        check(&bp, "Wang-Henke stripper");
        assert!(bp.iterations_taken < 100);

        let mbp = ModifiedWangHenkeSolver::default()
            .solve_column(&input)
            .expect("Modified Wang-Henke must converge on the stripper");
        check(&mbp, "Modified Wang-Henke stripper");

        let ns = NaphtaliSandholmSolver::with_warm_start()
            .solve_column(&input)
            .expect("Naphtali-Sandholm must converge on the stripper");
        check(&ns, "Naphtali-Sandholm stripper");

        for j in 0..8 {
            assert!(
                (bp.stage_temperatures[j] - ns.stage_temperatures[j]).abs() < 0.1
                    && (bp.stage_temperatures[j] - mbp.stage_temperatures[j]).abs() < 0.1,
                "stage {j}: T disagree BP {} MBP {} NS {}",
                bp.stage_temperatures[j],
                mbp.stage_temperatures[j],
                ns.stage_temperatures[j]
            );
        }
    }

    /// **Methodology.** The refluxed absorber of the module table, built with
    /// [`RigorousColumn::refluxed_absorber`], solved by [`WangHenkeSolver`]
    /// from the generated estimates. Must converge and satisfy
    /// [`assert_balances`], a monotone temperature profile, the reflux-ratio
    /// spec `L_0 / LSS_0 = 2` to `< 1e-6`, and the duty convention: `Q_7`
    /// returned exactly as given (`0 W`, user input), `Q_0 ≠ 0`
    /// (back-calculated condenser duty). Benzene must be enriched in the
    /// distillate `x_0` and depleted in the bottoms `x_7`. Then two
    /// **characterisation** checks that pin what this solver family actually
    /// does with the variant, so that a later solver fix is visible:
    ///
    /// 1. The distillate rate equals `distillate_rate_estimate` exactly (run
    ///    twice, with the default `0.5 mol/s` and with `0.33 mol/s`).
    /// 2. The bottom-stage energy residual ([`bottom_stage_energy_residual`])
    ///    is **not** zero.
    ///
    /// [`NaphtaliSandholmSolver`] (both with and without warm start) is run and
    /// its outcome recorded; it is asserted consistent only if it converges.
    ///
    /// **Results (2026-09-10, `cargo test --release`):**
    ///
    /// Wang-Henke, `D_est = 0.5` (default): converged in **15** iterations to
    /// `4.7127e-7`; `T = [357.3, 362.2, 366.7, 369.7, 371.5, 372.4, 372.8,
    /// 373.1] K`; `LSS_0 = D = 0.500000`, `B = 0.500000`, `L_0/D = 2.000000`;
    /// `x_0,benzene = 0.7649`, `x_7,benzene = 0.2351`; `Q_0 = +47 771 W`
    /// (back-calculated), `Q_7 = 0 W` (as given); molar residual `0`. Modified
    /// Wang-Henke: 13 iterations, `7.4514e-6`, same profile to `< 0.01 K`.
    ///
    /// Wang-Henke, `D_est = 0.33`: converged in **15** iterations to
    /// `4.2323e-7`; `LSS_0 = D = 0.330000`, `B = 0.670000`; `x_0 = 0.8797`,
    /// `x_7 = 0.3130`; `Q_0 = +30 937 W`.
    ///
    /// Bottom-stage energy residual (`in − out`, `Q_7 = 0`): **−15 084 W** for
    /// `D_est = 0.5`, **+1 766 W** for `D_est = 0.33` — against a latent-heat
    /// scale of ~32 kW for the 1 mol/s feed. Re-running with the feed
    /// enthalpy changed from saturated vapour to a 50 % vaporised feed gave a
    /// **bit-identical** profile (residual `−30 970 W` / `−14 120 W`): the
    /// feed enthalpy has no influence on the answer.
    ///
    /// Naphtali-Sandholm: **did not converge** from any of 30+ starting
    /// estimates tried (`NotConverged`, final error `0.11-3.9` after 18-100
    /// iterations; with warm start the same or worse). Sum-rates: not
    /// converged in 100 iterations.
    ///
    /// **Interpretation — two solver-side limitations, recorded not fixed.**
    ///
    /// *(a) The bubble-point family treats `distillate_rate_estimate` as a
    /// second specification.* For a refluxed absorber upstream closes the
    /// bottom with `B = L_ns` (`BubblePoint.vb:1020`) and `L_ns` with the
    /// total mass balance `ΣF − LSS_0 − V_0`, then re-derives `LSS_0 = ΣF − B −
    /// V_0` — a circular pair that returns whatever `LSS_0` started at. The
    /// bottom-stage energy balance is never used to size the vapour that the
    /// feed can generate, which is exactly the degree of freedom a refluxed
    /// absorber has; the non-zero residual and the feed-enthalpy insensitivity
    /// above are that missing equation. So the result is a **mass-consistent
    /// rectifying section for a user-chosen `D`**, not a solution of the
    /// column's MESH set. (Before the estimate fix in
    /// [`RigorousColumn::estimate_flows`] the generated `L_ns` ignored `D_est`,
    /// and the same circularity pinned `LSS_0` at `ΣF − L_ns,est = 0`: the
    /// solver "converged" in 16 iterations to `V = 0` on every stage, all feed
    /// to bottoms, and returned it as `Ok` — measured 2026-09-10, and the
    /// reason that fix exists.)
    ///
    /// *(b) Naphtali-Sandholm's total-condenser initialisation is gated on
    /// `DistillationColumn`.* Its stage-0 "vapour" variables stand for the
    /// distillate under a total condenser, but they are seeded from the
    /// distillate estimate only `If input.ColumnType = ColType.DistillationColumn`
    /// (`NewtonRaphson.vb:969`, ported at `newton_raphson.rs`); for a
    /// `RefluxedAbsorber` they start at `V_0,est · y_0 ≈ 1e-10` and the reflux
    /// row `l_0 − (L_0/LSS_0) v_0` is ill-conditioned from the first residual.
    /// The post-solve output mapping has the same gate, so even a converged
    /// refluxed absorber would report its distillate in `vapor_flows[0]`
    /// rather than `liquid_side_draws[0]`. Both are faithful to upstream as
    /// far as this port can tell and both are outside an API-exposure task —
    /// they are the follow-up work for a physically complete refluxed absorber.
    #[test]
    fn refluxed_absorber_solves_through_the_public_api() {
        let base = refluxed_absorber();
        assert_eq!(base.column_type, ColumnType::RefluxedAbsorber);

        let solve_bp = |column: &RigorousColumn, label: &str| {
            let input = column.solver_input().expect("estimate generation");
            let out = WangHenkeSolver::default()
                .solve_column(&input)
                .unwrap_or_else(|e| panic!("{label}: Wang-Henke must converge: {e}"));
            assert_balances(&out, &input, label);
            assert_monotone_increasing(&out, label);
            let d = out
                .distillate_molar_flow(input.condenser_type)
                .get::<katal>();
            let rr = out.liquid_flows[0] / d;
            assert!(
                (rr - 2.0).abs() < 1e-6,
                "{label}: reflux-ratio spec: R = {rr}"
            );
            assert!(
                (out.condenser_spec.calculated_value - 2.0).abs() < 1e-6,
                "{label}: calculated_value must echo the achieved ratio"
            );
            // Duty convention: Q_0 back-calculated, Q_7 user input (0).
            assert!(
                out.stage_heats[0].abs() > 1.0e3,
                "{label}: condenser duty must be back-calculated, got {} W",
                out.stage_heats[0]
            );
            assert_eq!(
                out.stage_heats[7], 0.0,
                "{label}: Q_7 must be the given 0 W"
            );
            // Separation direction.
            let x_top = out.liquid_compositions[0][0];
            let x_bot = out.liquid_compositions[7][0];
            assert!(
                x_top > 0.5 && x_bot < 0.5,
                "{label}: x_0 = {x_top}, x_7 = {x_bot}"
            );
            // Characterisation (a): D is pinned to the estimate.
            assert!(
                (d - column.distillate_rate_estimate).abs() < 1e-9,
                "{label}: characterisation — Wang-Henke pins D to the estimate \
                 ({}); got D = {d}. If this fails the solver has changed: \
                 update the docs of `RigorousColumn::refluxed_absorber`.",
                column.distillate_rate_estimate
            );
            (input, out)
        };

        let (input_default, out_default) = solve_bp(&base, "refluxed absorber, D_est = 0.5");
        assert!(out_default.iterations_taken < 100);
        assert!((input_default.liquid_side_draws[0] - 0.5).abs() < 1e-12);

        let tuned = base
            .clone()
            .with_distillate_estimate(MolarFlowRate::new::<katal>(0.33))
            .with_reflux_ratio_estimate(2.0);
        let (input_tuned, out_tuned) = solve_bp(&tuned, "refluxed absorber, D_est = 0.33");

        // Characterisation (b): the bottom-stage energy balance is not enforced.
        let e_default = bottom_stage_energy_residual(&out_default, &input_default);
        let e_tuned = bottom_stage_energy_residual(&out_tuned, &input_tuned);
        assert!(
            e_default.abs() > 1.0e3,
            "characterisation — Wang-Henke does not enforce the bottom-stage energy \
             balance on a refluxed absorber; residual = {e_default} W. If this fails \
             the solver has changed: update the docs of `RigorousColumn::refluxed_absorber`."
        );
        assert!(e_tuned.is_finite());

        // Modified Wang-Henke agrees with Wang-Henke on the default case.
        let mbp = ModifiedWangHenkeSolver::default()
            .solve_column(&input_default)
            .expect("Modified Wang-Henke must converge on the refluxed absorber");
        for j in 0..8 {
            assert!(
                (mbp.stage_temperatures[j] - out_default.stage_temperatures[j]).abs() < 0.05,
                "stage {j}: MBP {} vs BP {}",
                mbp.stage_temperatures[j],
                out_default.stage_temperatures[j]
            );
        }

        // Naphtali-Sandholm: recorded, not required (see the doc comment).
        for (label, solver) in [
            ("NS no warm start", NaphtaliSandholmSolver::default()),
            ("NS warm start", NaphtaliSandholmSolver::with_warm_start()),
        ] {
            match solver.solve_column(&input_default) {
                Ok(out) => assert_balances(&out, &input_default, label),
                Err(e) => assert!(!format!("{e}").is_empty()),
            }
        }
    }

    /// **Methodology.** The absorber of the module table, built with
    /// [`RigorousColumn::absorption`] and solved by [`NaphtaliSandholmSolver`]
    /// (no warm start — the recommended path for this variant). Must converge
    /// and satisfy [`assert_balances`]; the duty convention — **nothing is
    /// back-calculated**: every returned `stage_heats` entry equals the given
    /// `0 W`, `LSS_0 = 0`; the overall **and** the benzene component balances
    /// close to `< 1e-6 mol/s`; the bottom-stage energy balance
    /// ([`bottom_stage_energy_residual`]) closes to `< 10 W` (against ~30 kW
    /// of latent heat exchanged) — the check that distinguishes a genuinely
    /// solved absorber from the refluxed-absorber situation above; and the
    /// absorption direction — benzene leaner in the overhead vapour than in the
    /// 0.9 feed, present in the rich solvent, more than half of it recovered.
    /// The temperature profile is **not** required to be monotone. Wang-Henke
    /// and the warm-started Naphtali-Sandholm are run and their outcomes
    /// recorded; each is asserted consistent only if it converges.
    ///
    /// **Results (2026-09-10, `cargo test --release`):** Naphtali-Sandholm
    /// converged in **7** iterations to `Σf² = 6.7517e-7`. `T = [376.86,
    /// 374.42, 372.17, 369.83, 367.09, 363.31] K` — *hottest at the top*, where
    /// the 320 K lean toluene meets the benzene-richest vapour and most of the
    /// absorption heat is released, cooling downward against the 356.8 K vapour
    /// feed. `V = [0.6397, 0.9658, 0.9668, 0.9693, 0.9739, 0.9822]`,
    /// `L = [1.3261, 1.3271, 1.3297, 1.3342, 1.3425, 1.3603]` mol/s; overhead
    /// `V_0 = 0.639689` with `y_0,benzene = 0.2727`, rich solvent
    /// `B = 1.360311` with `x_5,benzene = 0.5334`; benzene recovery into the
    /// liquid `1 − 0.6397·0.2727/0.9 = 80.6 %`. Every `Q = 0` as given;
    /// `LSS_0 = 0`; molar residual exactly `0`; bottom-stage energy residual
    /// `−0.1 W`. Warm-started Naphtali-Sandholm: identical (7 iterations,
    /// same error). Wang-Henke: `NotConverged` after 100 iterations, final
    /// error `1.0621e-5` (gate `1e-5·ns/100 = 5e-7`); Modified Wang-Henke the
    /// same; sum-rates `InvalidProfile` (`T = −93.4 K` at stage 5).
    ///
    /// **Interpretation.** The `AbsorptionColumn` path — no end specs, both end
    /// duties taken as given, a plain vapour product off stage 0 — solves
    /// through the public constructor with all energy balances enforced. The
    /// bubble-point family's failure is structural, not a tolerance issue: it
    /// steps the vapour profile from a condenser-shaped stage 0
    /// (`V_1 = (R + 1) V_0 − F_0` with `R` read off a condenser energy balance
    /// that this column does not have), which is why upstream pairs absorbers
    /// with sum-rates — itself unconverged in this port. Not compared against
    /// a reference absorber; the hot-top profile is what the ideal-K,
    /// latent-heat enthalpy model gives for a strongly subcooled solvent and
    /// should be read as internally consistent, not validated.
    #[test]
    fn absorption_column_solves_through_the_public_api() {
        let column = absorber();
        assert_eq!(column.column_type, ColumnType::AbsorptionColumn);
        let input = column.solver_input().expect("estimate generation");
        assert!(input.stage_heats.iter().all(|q| *q == 0.0));

        let check = |out: &ColumnSolverOutput, label: &str| {
            assert_balances(out, &input, label);
            // Duty convention: nothing back-calculated, no liquid distillate.
            assert!(
                out.stage_heats.iter().all(|q| *q == 0.0),
                "{label}: absorber duties must be returned as given, got {:?}",
                out.stage_heats
            );
            assert_eq!(out.liquid_side_draws[0], 0.0, "{label}: LSS_0 must be 0");
            // Overall and benzene component balances.
            let v0 = out.vapor_flows[0];
            let b = out.bottoms_molar_flow().get::<katal>();
            assert!((v0 + b - 2.0).abs() < 1e-6, "{label}: V_0 + B = {}", v0 + b);
            let bz_in = 0.9;
            let bz_out = v0 * out.vapor_compositions[0][0] + b * out.liquid_compositions[5][0];
            assert!(
                (bz_out - bz_in).abs() < 1e-6,
                "{label}: benzene balance {bz_out} vs {bz_in} mol/s"
            );
            // Bottom-stage energy balance enforced.
            let e_bot = bottom_stage_energy_residual(out, &input);
            assert!(
                e_bot.abs() < 10.0,
                "{label}: bottom-stage energy residual = {e_bot} W"
            );
            // Absorption direction.
            let y_top = out.vapor_compositions[0][0];
            let x_bot = out.liquid_compositions[5][0];
            let recovery = 1.0 - v0 * y_top / bz_in;
            assert!(
                y_top < 0.9 && x_bot > 0.0,
                "{label}: y_0 = {y_top}, x_5 = {x_bot}"
            );
            assert!(recovery > 0.5, "{label}: benzene recovery {recovery}");
        };

        let ns = NaphtaliSandholmSolver::default()
            .solve_column(&input)
            .expect("Naphtali-Sandholm must converge on the absorber");
        check(&ns, "Naphtali-Sandholm absorber");
        assert!(ns.iterations_taken < 100);

        // Recorded, not required (see the doc comment).
        match NaphtaliSandholmSolver::with_warm_start().solve_column(&input) {
            Ok(out) => check(&out, "warm-started Naphtali-Sandholm absorber"),
            Err(e) => assert!(!format!("{e}").is_empty()),
        }
        match WangHenkeSolver::default().solve_column(&input) {
            Ok(out) => check(&out, "Wang-Henke absorber"),
            Err(e) => assert!(!format!("{e}").is_empty()),
        }
    }
}
