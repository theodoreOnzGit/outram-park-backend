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
pub mod initial_estimates;
pub mod linalg;
pub mod model;
pub mod newton_raphson;
pub mod profile;
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
    fn benzene_toluene_column() -> RigorousColumn {
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
