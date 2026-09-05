//! The stage-profile record every rigorous-column solver produces internally.
//!
//! Upstream returns this as a loosely-typed
//! `New Object() {Tj, Vj, Lj, VSSj, LSSj, yc, xc, K, Q, ic, t_error}` array
//! (`BubblePoint.vb:1855`, `BubblePoint2.vb`, `SumRates.vb:805`,
//! `NewtonRaphson.vb`), unpacked by index at every call site — `result(0)` is
//! the temperature vector, `result(5)` the vapour compositions, and so on. That
//! is exactly the kind of positional API the workspace "human interface layer"
//! rule forbids, so this port names the fields.
//!
//! Ported from DWSIM (GPL-3.0), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008-2022
//! Daniel Wagner O. de Medeiros et al.
//!
//! # Units
//!
//! `temperatures` \[K\], `vapor_flows` / `liquid_flows` / `vapor_side_draws` /
//! `liquid_side_draws` \[mol/s\], `vapor_compositions` /
//! `liquid_compositions` / `k_values` \[-\], `heats` \[W\].

use crate::columns::model::{ColumnSolverOutput, ColumnSpec};

/// A full stage-by-stage column profile at one point in a solve.
///
/// Every `Vec` is indexed by stage with length `n_stages`; nested vectors are
/// `[stage][component]`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StageProfile {
    /// Stage temperatures \[K\] (upstream `Tj`).
    pub temperatures: Vec<f64>,
    /// Stage vapour molar flows \[mol/s\] (upstream `Vj`).
    pub vapor_flows: Vec<f64>,
    /// Stage liquid molar flows \[mol/s\] (upstream `Lj`).
    pub liquid_flows: Vec<f64>,
    /// Vapour side draws \[mol/s\] (upstream `VSSj`).
    pub vapor_side_draws: Vec<f64>,
    /// Liquid side draws \[mol/s\] (upstream `LSSj`); index 0 doubles as the
    /// distillate rate for a total/partial condenser.
    pub liquid_side_draws: Vec<f64>,
    /// Vapour mole fractions \[-\] (upstream `yc`).
    pub vapor_compositions: Vec<Vec<f64>>,
    /// Liquid mole fractions \[-\] (upstream `xc`).
    pub liquid_compositions: Vec<Vec<f64>>,
    /// K-values \[-\] (upstream `K`).
    pub k_values: Vec<Vec<f64>>,
    /// Stage heat duties \[W\] (upstream `Q`), positive into the stage.
    pub heats: Vec<f64>,
    /// Inner iterations taken (upstream `ic`).
    pub iterations: usize,
    /// Final error-function value (upstream `t_error`).
    pub error: f64,
}

impl StageProfile {
    /// Allocate a zeroed profile for `n_stages` stages and `n_components`
    /// components.
    #[must_use]
    pub fn zeros(n_stages: usize, n_components: usize) -> Self {
        Self {
            temperatures: vec![0.0; n_stages],
            vapor_flows: vec![0.0; n_stages],
            liquid_flows: vec![0.0; n_stages],
            vapor_side_draws: vec![0.0; n_stages],
            liquid_side_draws: vec![0.0; n_stages],
            vapor_compositions: vec![vec![0.0; n_components]; n_stages],
            liquid_compositions: vec![vec![0.0; n_components]; n_stages],
            k_values: vec![vec![1.0; n_components]; n_stages],
            heats: vec![0.0; n_stages],
            iterations: 0,
            error: f64::INFINITY,
        }
    }

    /// Number of stages in this profile.
    #[must_use]
    pub fn n_stages(&self) -> usize {
        self.temperatures.len()
    }

    /// Convert to the public [`ColumnSolverOutput`], attaching the two specs
    /// with their achieved values already written in.
    #[must_use]
    pub fn into_output(
        self,
        condenser_spec: ColumnSpec,
        reboiler_spec: ColumnSpec,
    ) -> ColumnSolverOutput {
        ColumnSolverOutput {
            iterations_taken: self.iterations,
            final_error: self.error,
            stage_temperatures: self.temperatures,
            stage_heats: self.heats,
            vapor_flows: self.vapor_flows,
            vapor_compositions: self.vapor_compositions,
            liquid_flows: self.liquid_flows,
            liquid_compositions: self.liquid_compositions,
            vapor_side_draws: self.vapor_side_draws,
            liquid_side_draws: self.liquid_side_draws,
            k_values: self.k_values,
            condenser_spec,
            reboiler_spec,
        }
    }

    /// `true` if every temperature, flow and composition is finite and the
    /// flows are non-negative.
    ///
    /// Ports the `Not Tj.IsValid Or Not Vj.IsValid Or Not Lj.IsValid` guard of
    /// `BubblePoint.vb:1705` and the per-stage mass-balance guard of `:1791-1820`.
    #[must_use]
    pub fn is_physical(&self) -> bool {
        let finite = |v: &Vec<f64>| v.iter().all(|x| x.is_finite());
        finite(&self.temperatures)
            && finite(&self.vapor_flows)
            && finite(&self.liquid_flows)
            && self.temperatures.iter().all(|t| *t > 0.0)
            && self.vapor_flows.iter().all(|v| *v >= 0.0)
            && self.liquid_flows.iter().all(|l| *l >= 0.0)
            && self.liquid_side_draws.iter().all(|l| *l >= 0.0)
            && self
                .vapor_compositions
                .iter()
                .chain(self.liquid_compositions.iter())
                .all(|row| (row.iter().sum::<f64>() - 1.0).abs() < 1e-3)
    }
}
