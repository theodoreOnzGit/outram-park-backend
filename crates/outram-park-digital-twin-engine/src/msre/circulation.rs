//! # Circulating-fuel kinetics at variable flow (gh:#1332, gh:#1330)
//!
//! The delayed-neutron precursors of the MSRE drift with the fuel. When the
//! fuel pump trips, the drift slows and stops, the precursors that would have
//! decayed outside the core now decay inside it, and the reactor gains back
//! the reactivity it lost to circulation. [`CirculatingKinetics`] carries
//! that through any flow history, in one of two ORNL models
//! ([`CirculationModel`]):
//!
//! - **[`CirculationModel::WellStirredTm1070`]** (the default): ORNL-TM-1070
//!   App. A (A.29), p. 57. The core is a well-stirred tank of transit `τ_c`
//!   and the external loop a pure delay `τ_L` (Table 1: 8.46 s and 16.73 s),
//!   with TM-1070 App. B p. 69's six groups. Every delayed neutron emitted in
//!   the core counts in full; none emitted outside counts. At full flow this
//!   is `teh_o_prke`'s `CirculatingPointKinetics` (MSRE T-C1, gh:#1131)
//!   step for step; the test `the_well_stirred_model_is_teh_o_prkes_at_full_flow`
//!   pins that.
//! - **[`CirculationModel::LineTm1626`]**: ORNL-TM-1626's line model
//!   (Prince, 1966, Eqs. 14 and 24-27, pp. 7-15): plug flow through the
//!   lower plenum, the graphite region, the upper plenum and the external
//!   loop (Table 1 residence times 3.8, 9.4, 3.9, 8.1 s); precursors made in
//!   proportion to the fission density in the salt and their neutrons
//!   weighted by the fast adjoint, both from TM-1626 Fig. 4. Prince treated
//!   the lower plenum as a well-stirred tank; here it is plug flow, as in
//!   this workspace's exact reference (`outram-park-fork-moltres`,
//!   `tests/msre_n4_precursor_drift_tm1626.rs`, gh:#1130), which records
//!   0.2293 % (plug) against 0.2299 % (stirred).
//!
//! ## Variable flow
//!
//! The flow enters as a fraction `f` of the full flow, per step, and the
//! loop is indexed by **fluid displacement**, not time: the loop (and, for
//! the line model, the whole flow path) is a ring of cells, each the volume
//! that passes in one step at full flow, and a cell moves on whenever the
//! accumulated displacement `Σ f` passes one cell. At full flow that is one
//! cell per step, exactly. Below full flow the contents move in whole cells,
//! so the plug-flow position is resolved to one cell (0.01 s of full-flow
//! transit); in between, the precursors decay in place. At `f = 0` the fuel
//! is stationary and the kinetics are the stationary-fuel equations.
//!
//! ## Time stepping
//!
//! - Well-stirred: backward Euler, as `CirculatingPointKinetics`, with the
//!   core removal `f/τ_c`, and the return term `f/τ_c` times the
//!   concentration of the cell leaving the loop, decayed for the time it
//!   actually spent there.
//! - Line: each step moves the ring, then decays every cell exactly and adds
//!   the source over the step, `β_i P_k n' (1 − e^{−λ_i Δt})/(λ_i Λ)`, with
//!   the new neutron density `n'`, so both models are implicit in `n`. In
//!   both, eliminating the precursors leaves one scalar equation for `n'`.
//!
//! ## What is not modelled
//!
//! Axial mixing in the core (TM-1070 p. 14 measured "a considerable amount"
//! on the hydraulic mockup), radial flow and flux variation (Prince assumes
//! separability, TM-1626 p. 12), the change of flux shape after a pump trip,
//! and any flow below what the caller supplies (no natural circulation).
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only. NOT VALIDATED.

use std::collections::VecDeque;

use petir::mathf::RealMath;

use super::design::{BETA, LAMBDA};

/// Which ORNL circulating-fuel model [`CirculatingKinetics`] uses. See the
/// module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CirculationModel {
    /// ORNL-TM-1070 (A.29): well-stirred core, delayed loop, TM-1070 data.
    /// The default: the MSRE T-1 frequency-response V&V was measured with it.
    #[default]
    WellStirredTm1070,
    /// ORNL-TM-1626's importance-weighted line model, TM-1626 data.
    LineTm1626,
}

/// TM-1070 Table 1 core transit time \[s\].
pub const TM1070_CORE_TRANSIT_S: f64 = 8.46;
/// TM-1070 Table 1 external-loop transit time \[s\].
pub const TM1070_LOOP_TRANSIT_S: f64 = 16.73;

/// ORNL-TM-1626 Table 1 (report p. 24): `(β_i, λ_i [1/s])`, the static,
/// spectrum-corrected fractions (10⁴β_i = 2.23, 14.57, 13.07, 26.28, 7.66,
/// 2.80; sum 0.006661). As read from the scan for gh:#1130 and used by
/// `outram-park-fork-moltres/tests/msre_n4_precursor_drift_tm1626.rs`.
pub const TM1626_TABLE1: [(f64, f64); 6] = [
    (2.23e-4, 0.0124),
    (14.57e-4, 0.0305),
    (13.07e-4, 0.1114),
    (26.28e-4, 0.3013),
    (7.66e-4, 1.140),
    (2.80e-4, 3.010),
];

/// ORNL-TM-1626 Table 1 residence times \[s\]: lower plenum, graphite
/// (channelled) region, upper plenum, external loop. Total 25.2 s.
pub const TM1626_RESIDENCE_S: [f64; 4] = [3.8, 9.4, 3.9, 8.1];

/// Region extents along z \[cm\] read from TM-1626 Figs. 4 and 5 (gh:#1130):
/// the bottom of the lower plenum, the top of the graphite, the top of the
/// upper plenum. The graphite starts at z = 0.
pub const TM1626_Z_EXTENTS_CM: [f64; 3] = [-22.0, 168.0, 190.0];

/// ORNL-TM-1626 Fig. 4 (report p. 21, PDF p. 27), "Axial Distributions of
/// Fission Density in Fuel Salt and Fast Group Adjoint Flux (Calculated
/// Curves)": `(z cm, P°φ_s, φ₁†)`, digitised by eye for gh:#1130, the same
/// table as `FIG4` in `outram-park-fork-moltres/tests/msre_n4_precursor_drift_tm1626.rs`
/// (its README records the digitisation and its ±0.005 % effect on the
/// loss). Linear between points.
pub const TM1626_FIG4: [(f64, f64, f64); 26] = [
    (-22.0, 0.015, 0.045),
    (-20.0, 0.015, 0.055),
    (-10.0, 0.035, 0.130),
    (-5.0, 0.070, 0.190),
    (0.0, 0.120, 0.260),
    (10.0, 0.330, 0.370),
    (20.0, 0.500, 0.500),
    (30.0, 0.620, 0.620),
    (40.0, 0.730, 0.730),
    (50.0, 0.830, 0.830),
    (60.0, 0.900, 0.900),
    (70.0, 0.950, 0.950),
    (80.0, 0.985, 0.985),
    (90.0, 0.990, 0.990),
    (100.0, 0.970, 0.970),
    (110.0, 0.925, 0.925),
    (120.0, 0.860, 0.860),
    (130.0, 0.780, 0.780),
    (140.0, 0.680, 0.690),
    (150.0, 0.560, 0.585),
    (160.0, 0.390, 0.440),
    (165.0, 0.250, 0.380),
    (170.0, 0.110, 0.340),
    (180.0, 0.050, 0.170),
    (185.0, 0.025, 0.120),
    (190.0, 0.008, 0.070),
];

/// Errors from [`CirculatingKinetics`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CirculationError {
    /// An input out of range (stated in the message).
    #[error("circulating kinetics: {0}")]
    InvalidInput(String),
}

/// TM-1626 Fig. 4 at `z` \[cm\]: `(fission density, importance)`, linear
/// between points and flat beyond the ends.
pub fn tm1626_shape_at(z: f64) -> (f64, f64) {
    let t = &TM1626_FIG4;
    if z <= t[0].0 {
        return (t[0].1, t[0].2);
    }
    for w in t.windows(2) {
        let (z0, p0, a0) = w[0];
        let (z1, p1, a1) = w[1];
        if z <= z1 {
            let r = (z - z0) / (z1 - z0);
            return (p0 + r * (p1 - p0), a0 + r * (a1 - a0));
        }
    }
    let last = t[t.len() - 1];
    (last.1, last.2)
}

/// TM-1626's flow path at full flow: `(fission density, importance)` at
/// transit time `t` \[s\] from the lower-plenum inlet, plug flow in each
/// region, zero in the external loop.
pub fn tm1626_path_at(t: f64) -> (f64, f64) {
    let [t_lp, t_c, t_up, _] = TM1626_RESIDENCE_S;
    let [z_bottom, z_core_top, z_top] = TM1626_Z_EXTENTS_CM;
    let z = if t < t_lp {
        z_bottom + (0.0 - z_bottom) * t / t_lp
    } else if t < t_lp + t_c {
        z_core_top * (t - t_lp) / t_c
    } else if t < t_lp + t_c + t_up {
        z_core_top + (z_top - z_core_top) * (t - t_lp - t_c) / t_up
    } else {
        return (0.0, 0.0);
    };
    tm1626_shape_at(z)
}

/// Whole number of `dt` steps in `seconds`, or an error.
fn whole_steps(name: &str, seconds: f64, dt: f64) -> Result<usize, CirculationError> {
    let n = (seconds / dt).round();
    if n < 1.0 || ((n * dt - seconds) / seconds).abs() > 1e-9 {
        return Err(CirculationError::InvalidInput(format!(
            "{name} {seconds} s is not a whole number of {dt} s steps"
        )));
    }
    Ok(n as usize)
}

/// The geometry-specific state.
#[derive(Debug, Clone, PartialEq)]
enum Path {
    /// TM-1070: core `C_i`, and the loop as cells, front = next to return,
    /// each `(step index at which it left the core, C_i)`.
    WellStirred {
        core_transit: f64,
        loop_cells: usize,
        core: Vec<f64>,
        loop_: VecDeque<(i64, Vec<f64>)>,
    },
    /// TM-1626: a ring of `production.len()` cells fixed in space; the
    /// contents `cells[i][(head + s) % M]` sit in slot `s`.
    Line {
        production: Vec<f64>,
        importance: Vec<f64>,
        /// `Σ_s φ†_s P_s`, the normalisation of the delayed source.
        weight: f64,
        cells: Vec<Vec<f64>>,
        head: usize,
    },
}

/// Circulating-fuel point kinetics at variable flow. See the module docs.
///
/// Units: the neutron density `n` is relative (the initial steady state is
/// `n = 1`); the delayed source is normalised so that a stationary core's is
/// `β n / Λ`.
#[derive(Debug, Clone, PartialEq)]
pub struct CirculatingKinetics {
    model: CirculationModel,
    betas: Vec<f64>,
    lambdas: Vec<f64>,
    generation_time: f64,
    dt: f64,
    n: f64,
    flow: f64,
    displacement: f64,
    step_index: i64,
    path: Path,
    loss: f64,
}

impl CirculatingKinetics {
    /// The kinetics for `model` at full flow, in the circulating steady
    /// state at `n = 1`, with prompt generation time `generation_time` \[s\]
    /// and fixed step `dt` \[s\]. Errors if `dt` or `Λ` is not positive, or
    /// a transit is not a whole number of steps.
    pub fn new(model: CirculationModel, generation_time: f64, dt: f64) -> Result<Self, CirculationError> {
        if !(dt > 0.0 && dt.is_finite() && generation_time > 0.0 && generation_time.is_finite()) {
            return Err(CirculationError::InvalidInput(format!(
                "dt and generation time must be > 0, got {dt} and {generation_time}"
            )));
        }
        let (betas, lambdas): (Vec<f64>, Vec<f64>) = match model {
            CirculationModel::WellStirredTm1070 => (BETA.to_vec(), LAMBDA.to_vec()),
            CirculationModel::LineTm1626 => TM1626_TABLE1.iter().cloned().unzip(),
        };
        let path = match model {
            CirculationModel::WellStirredTm1070 => {
                let loop_cells = whole_steps("loop transit", TM1070_LOOP_TRANSIT_S, dt)?;
                Path::WellStirred {
                    core_transit: TM1070_CORE_TRANSIT_S,
                    loop_cells,
                    core: vec![0.0; betas.len()],
                    loop_: VecDeque::new(),
                }
            }
            CirculationModel::LineTm1626 => {
                for (name, t) in ["lower plenum", "graphite region", "upper plenum", "external loop"]
                    .iter()
                    .zip(TM1626_RESIDENCE_S.iter())
                {
                    whole_steps(name, *t, dt)?;
                }
                let total: f64 = TM1626_RESIDENCE_S.iter().sum();
                let m = whole_steps("loop", total, dt)?;
                let (production, importance): (Vec<f64>, Vec<f64>) =
                    (0..m).map(|s| tm1626_path_at((s as f64 + 0.5) * dt)).unzip();
                let weight = production.iter().zip(&importance).map(|(p, w)| p * w).sum();
                Path::Line { production, importance, weight, cells: vec![vec![0.0; m]; betas.len()], head: 0 }
            }
        };
        let mut k = Self {
            model,
            betas,
            lambdas,
            generation_time,
            dt,
            n: 1.0,
            flow: 1.0,
            displacement: 0.0,
            step_index: 0,
            path,
            loss: 0.0,
        };
        k.set_circulating_equilibrium(1.0);
        k.loss = k.critical_reactivity();
        Ok(k)
    }

    /// Put the state in the full-flow circulating steady state at neutron
    /// density `n`, the periodic solution of this scheme.
    fn set_circulating_equilibrium(&mut self, n: f64) {
        self.n = n;
        self.flow = 1.0;
        self.displacement = 0.0;
        let (lam, dt) = (self.generation_time, self.dt);
        match &mut self.path {
            Path::WellStirred { core_transit, loop_cells, core, loop_ } => {
                for (i, c) in core.iter_mut().enumerate() {
                    let l = self.lambdas[i];
                    let drift = (1.0 - (-l * TM1070_LOOP_TRANSIT_S).r_exp()) / *core_transit;
                    *c = self.betas[i] * n / (lam * (l + drift));
                }
                let lc = *loop_cells as i64;
                *loop_ = (0..lc).map(|j| (self.step_index + 1 - lc + j, core.clone())).collect();
            }
            Path::Line { production, cells, head, .. } => {
                *head = 0;
                let m = production.len();
                for (i, row) in cells.iter_mut().enumerate() {
                    let l = self.lambdas[i];
                    let e = (-l * dt).r_exp();
                    let g = self.betas[i] * (1.0 - e) / (l * lam) * n;
                    // A parcel advances one slot per step: c' = e c + g P_slot.
                    // Once round the loop from slot 0 is the affine map a x + b.
                    let mut b = 0.0;
                    for s in 1..=m {
                        b = e * b + g * production[s % m];
                    }
                    let a = (-l * dt * m as f64).r_exp();
                    let mut c = b / (1.0 - a);
                    row[0] = c;
                    for s in 1..m {
                        c = e * c + g * production[s];
                        row[s] = c;
                    }
                }
            }
        }
    }

    /// The circulation model in use.
    pub fn model(&self) -> CirculationModel {
        self.model
    }

    /// `β = Σ β_i` of the model's data set.
    pub fn total_delayed_fraction(&self) -> f64 {
        self.betas.iter().sum()
    }

    /// Relative neutron density `n`.
    pub fn neutron_density(&self) -> f64 {
        self.n
    }

    /// The flow fraction of the last step.
    pub fn flow_fraction(&self) -> f64 {
        self.flow
    }

    /// The reactivity that holds the full-flow circulating core critical
    /// (the steady circulation loss), computed at construction from the
    /// scheme's own equilibrium. For the well-stirred model it equals the
    /// analytic `Σ β_i [1 − λ_i/(λ_i + (1 − e^{−λ_i τ_L})/τ_c)]`.
    pub fn circulation_loss(&self) -> f64 {
        self.loss
    }

    /// Each group's delayed-neutron source, normalised so that a
    /// stationary critical core's is `β_i n / Λ`.
    pub fn delayed_sources(&self) -> Vec<f64> {
        match &self.path {
            Path::WellStirred { core, .. } => core.iter().zip(&self.lambdas).map(|(c, l)| l * c).collect(),
            Path::Line { importance, weight, cells, head, .. } => cells
                .iter()
                .zip(&self.lambdas)
                .map(|(row, l)| l * weighted_sum(importance, row, *head) / weight)
                .collect(),
        }
    }

    /// `β − Λ D / n`: the reactivity at which the present delayed source
    /// `D` holds `n` steady (the prompt-jump critical reactivity). In the
    /// circulating steady state it is [`Self::circulation_loss`]; with the
    /// fuel long stationary it tends to zero. An ideal flux servo inserts
    /// the change in it.
    pub fn critical_reactivity(&self) -> f64 {
        let d: f64 = self.delayed_sources().iter().sum();
        self.total_delayed_fraction() - self.generation_time * d / self.n
    }

    /// The cells (in whole cells) the ring moves in a step at `flow`.
    fn shifts(&self, flow: f64) -> usize {
        let mut d = self.displacement + flow;
        let mut k = 0;
        while d >= 1.0 - 1e-9 {
            d = (d - 1.0).max(0.0);
            k += 1;
        }
        k
    }

    /// The scalar equation for the next `n`, `lhs n' = rhs`, for a step at
    /// `flow`, starting from `lhs = lhs_start` and `rhs = rhs_start` (the
    /// reactivity and source terms), with the well-stirred model's
    /// per-group `(known, per_n)` (empty for the line model). Reads the
    /// state; changes nothing.
    fn coefficients(&self, flow: f64, lhs_start: f64, rhs_start: f64) -> (f64, f64, Vec<(f64, f64)>) {
        let (dt, lam) = (self.dt, self.generation_time);
        let (mut lhs, mut rhs) = (lhs_start, rhs_start);
        let mut partial = Vec::new();
        match &self.path {
            Path::WellStirred { core_transit, core, loop_, .. } => {
                let returning = loop_.front();
                for i in 0..core.len() {
                    let l = self.lambdas[i];
                    let a = 1.0 / dt + l + flow / *core_transit;
                    let d = match returning {
                        Some((stamp, c)) => {
                            let elapsed = (self.step_index + 1 - stamp) as f64 * dt;
                            c[i] * (-l * elapsed).r_exp() * flow / *core_transit
                        }
                        None => 0.0,
                    };
                    let known = (core[i] / dt + d) / a;
                    let per_n = self.betas[i] / lam / a;
                    lhs -= l * per_n;
                    rhs += l * known;
                    partial.push((known, per_n));
                }
            }
            Path::Line { importance, weight, cells, head, .. } => {
                let m = importance.len();
                let head = (*head + m - self.shifts(flow) % m) % m;
                for (i, row) in cells.iter().enumerate() {
                    let l = self.lambdas[i];
                    let e = (-l * dt).r_exp();
                    rhs += l * e * weighted_sum(importance, row, head) / *weight;
                    // The source over the step adds Σ_s φ†_s g P_s n' = g W n'.
                    lhs -= self.betas[i] * (1.0 - e) / lam;
                }
            }
        }
        (lhs, rhs, partial)
    }

    /// The reactivity that brings `n` to `n_target` over the next step at
    /// fuel flow `flow`: the exact servo of this scheme (the step's
    /// equation is affine in the reactivity). With `n_target = n` it holds
    /// the power, as ORNL's flux servo did (ORNL-4233 Fig. 24).
    pub fn holding_reactivity(&self, flow: f64, n_target: f64) -> f64 {
        let lam = self.generation_time;
        let beta = self.total_delayed_fraction();
        let (lhs, rhs, _) = self.coefficients(flow, 1.0 / self.dt + beta / lam, self.n / self.dt);
        lam * (lhs - rhs / n_target)
    }

    /// Advance one step at absolute reactivity `rho` (relative to the
    /// stationary-fuel critical state), fuel flow fraction `flow ≥ 0` and
    /// external source `source` (in `n` per second).
    pub fn step(&mut self, rho: f64, flow: f64, source: f64) -> Result<(), CirculationError> {
        if !(flow >= 0.0 && flow.is_finite() && rho.is_finite() && source.is_finite()) {
            return Err(CirculationError::InvalidInput(format!(
                "flow must be finite and >= 0, rho and source finite; got {flow}, {rho}, {source}"
            )));
        }
        let (dt, lam) = (self.dt, self.generation_time);
        let beta = self.total_delayed_fraction();
        let (lhs, rhs, partial) = self.coefficients(flow, 1.0 / dt - (rho - beta) / lam, self.n / dt + source);
        let n_new = solve_n(lhs, rhs)?;
        let shifts = self.shifts(flow);
        let step_index = self.step_index;
        match &mut self.path {
            Path::WellStirred { loop_cells, core, loop_, .. } => {
                for (c, (known, per_n)) in core.iter_mut().zip(partial) {
                    *c = known + per_n * n_new;
                }
                for _ in 0..shifts {
                    loop_.push_back((step_index + 1, core.clone()));
                    if loop_.len() > *loop_cells {
                        loop_.pop_front();
                    }
                }
            }
            Path::Line { production, cells, head, .. } => {
                let m = production.len();
                *head = (*head + m - shifts % m) % m;
                for (i, row) in cells.iter_mut().enumerate() {
                    let l = self.lambdas[i];
                    let e = (-l * dt).r_exp();
                    let g = self.betas[i] * (1.0 - e) / (l * lam) * n_new;
                    for c in row.iter_mut() {
                        *c *= e;
                    }
                    let (wrapped, from_head) = row.split_at_mut(*head);
                    for (c, p) in from_head.iter_mut().chain(wrapped.iter_mut()).zip(production.iter()) {
                        *c += g * p;
                    }
                }
            }
        }
        let mut d = self.displacement + flow;
        for _ in 0..shifts {
            d = (d - 1.0).max(0.0);
        }
        self.displacement = d;
        self.n = n_new;
        self.flow = flow;
        self.step_index += 1;
        Ok(())
    }
}

/// `Σ_s w_s row[(head + s) mod M]`: slot weights against ring contents.
fn weighted_sum(weights: &[f64], row: &[f64], head: usize) -> f64 {
    let (wrapped, from_head) = row.split_at(head);
    from_head.iter().chain(wrapped.iter()).zip(weights.iter()).map(|(c, w)| c * w).sum()
}

/// `n' = rhs / lhs`, or an error for a singular step.
fn solve_n(lhs: f64, rhs: f64) -> Result<f64, CirculationError> {
    if !(lhs.is_finite() && lhs != 0.0 && rhs.is_finite()) {
        return Err(CirculationError::InvalidInput(format!("singular kinetics step: lhs {lhs}, rhs {rhs}")));
    }
    Ok(rhs / lhs)
}

/// The analytic well-stirred circulation loss `Σ β_i [1 − λ_i/(λ_i + (1 −
/// e^{−λ_i τ_L})/τ_c)]` for any group data and transits: the instrument of
/// gh:#1330's data-set and transit ablations.
pub fn well_stirred_loss(groups: &[(f64, f64)], core_transit: f64, loop_transit: f64) -> f64 {
    groups
        .iter()
        .map(|&(b, l)| b * (1.0 - l / (l + (1.0 - (-l * loop_transit).r_exp()) / core_transit)))
        .sum()
}

#[cfg(test)]
mod tests;
