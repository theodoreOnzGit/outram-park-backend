// SPDX-License-Identifier: GPL-3.0

//! **Mesh-based weight windows** — splitting above the window, Russian
//! roulette below it, and MAGIC generation from a flux tally. GitHub #258.
//!
//! Ported from `src/weight_windows.cpp` / `include/openmc/weight_windows.h`
//! at OpenMC `afa7a14`.
//!
//! # What a weight window is for, and why it is not physics
//!
//! A weight window is an **estimator**, not a model. It splits a particle that
//! is more important than its weight suggests and rouletted one that is less,
//! preserving the expected score while moving sampling effort towards the
//! tally. Nothing about the transported physics changes, which is why
//! [`crate::physics::variance_reduction`] and this module are opt-in while a
//! physics term supplied by the data is not (root `CLAUDE.md`).
//!
//! That framing carries an obligation: **an unbiased-in-principle scheme is
//! still a bug until measured.** A window with the wrong bounds, or a split
//! that mis-divides the weight, produces a plausible answer with a plausible
//! uncertainty and nothing raises an error. Every claim in this module is
//! either an exact algebraic invariant asserted in a test, or a measured
//! comparison against the analog arm.
//!
//! # Where the two checkpoints sit
//!
//! Upstream applies windows at two points, `weight_window_checkpoint_surface`
//! and `weight_window_checkpoint_collision`. Both are honoured here:
//! [`WeightWindows::look_up`] is called after a surface crossing and after a
//! collision, and [`apply`] plays the game.

use crate::geometry::position::Position;
use crate::physics::variance_reduction::russian_roulette;
use crate::tally::mesh::RegularMesh;

/// `WEIGHT_WINDOW_REL_TOL` (`include/openmc/constants.h:80`).
///
/// The relative dead band on the window comparisons. Without it, a weight that
/// sits within rounding of a bound takes a branch decided by the last bit of
/// the bound — and weight-window arithmetic produces exactly that case (a
/// roulette survivor assigned `weight * max_split`, later compared against an
/// upper bound that is an exact multiple of the same lower bound).
pub const WEIGHT_WINDOW_REL_TOL: f64 = 1.0e-9;

/// `DEFAULT_WEIGHT_CUTOFF` (`include/openmc/weight_windows.h:26`).
pub const DEFAULT_WEIGHT_CUTOFF: f64 = 1.0e-38;

/// One window: the bounds and the game parameters at a single
/// (energy, mesh cell). `WeightWindow` (`weight_windows.h:47`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightWindow {
    /// Below this the particle is rouletted. **A non-positive lower bound
    /// means "no window here"** — generators mark an unresolved cell with
    /// `-1`, and a zero lower bound conventionally turns the game off in a
    /// cell (the MCNP `wwinp` convention). That is why [`Self::is_valid`]
    /// exists rather than an `Option`: the sentinel is what the data format
    /// carries.
    pub lower_weight: f64,
    /// Above this the particle is split.
    pub upper_weight: f64,
    /// Cap on how far above the lower bound a particle may sit before the
    /// window itself is scaled up to meet it.
    pub max_lb_ratio: f64,
    /// Weight a roulette survivor is promoted to.
    pub survival_weight: f64,
    /// Absolute weight below which the particle is killed outright.
    pub weight_cutoff: f64,
    /// Cap on the number of copies one split may produce.
    ///
    /// **Integral, matching upstream's `int max_split_` (`weight_windows.h:200`),
    /// and that is load-bearing rather than cosmetic.** `apply` divides the
    /// parent's weight by `n_split` and emits `round(n_split)` particles. Those
    /// two agree only while `n_split` is exactly an integer. Upstream gets that
    /// for free because `max_split` is an `int`, so `min(ceil(..), max_split)`
    /// cannot be fractional. An earlier draft of this port widened the field to
    /// `f64`, which admitted `max_split = 2.5` -> `round(2.5) = 3` copies each
    /// of weight `w/2.5`, i.e. `1.2 w` total — weight created from nothing, in
    /// the one routine whose entire justification is that it is weight-neutral.
    pub max_split: u32,
}

impl Default for WeightWindow {
    fn default() -> Self {
        Self {
            lower_weight: -1.0,
            upper_weight: 1.0,
            max_lb_ratio: 1.0,
            survival_weight: 0.5,
            weight_cutoff: DEFAULT_WEIGHT_CUTOFF,
            max_split: 10,
        }
    }
}

impl WeightWindow {
    /// Whether a window exists here at all (`weight_windows.h:59`).
    pub fn is_valid(&self) -> bool {
        self.lower_weight > 0.0
    }

    /// Scale the window by a constant factor (`weight_windows.h:63`).
    ///
    /// Note that `weight_cutoff` and `max_lb_ratio` are **not** scaled, which
    /// is upstream's behaviour and not an oversight: the cutoff is an absolute
    /// floor and the ratio is dimensionless.
    pub fn scale(&mut self, factor: f64) {
        self.lower_weight *= factor;
        self.upper_weight *= factor;
        self.survival_weight *= factor;
    }
}

/// What playing the window game did to a particle.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowOutcome {
    /// No window here, or the particle is inside it. Weight unchanged.
    Unchanged,
    /// The particle was killed — by the absolute cutoff or by losing a
    /// roulette.
    Killed,
    /// Rouletted and survived, promoted to this weight.
    Survived { weight: f64 },
    /// Split into `copies` particles (including the original), each carrying
    /// `weight`. `copies - 1` new particles must be banked.
    Split { copies: usize, weight: f64 },
}

/// Per-particle state the window game needs to carry.
///
/// Upstream keeps these on `Particle` (`wgt_born`, `wgt_ww_born`, `ww_factor`,
/// `n_split`). They are grouped here so the game is a pure function of
/// (window, state, weight) and can be tested without a transport loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
    /// The weight this particle was born with.
    pub weight_born: f64,
    /// The midpoint of the first window the particle saw, or `-1.0` if it has
    /// not seen one yet. Upstream's `wgt_ww_born`.
    pub ww_born: f64,
    /// Once set, the factor by which the window is scaled up to meet a
    /// particle sitting far above it. Upstream's `ww_factor`; `0.0` means
    /// unset, which is upstream's sentinel.
    pub ww_factor: f64,
    /// Splits accumulated by this history, against `max_history_splits`.
    pub n_split: f64,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            weight_born: 1.0,
            ww_born: -1.0,
            ww_factor: 0.0,
            n_split: 0.0,
        }
    }
}

/// `settings::max_history_splits` (`src/settings.cpp:123`).
pub const MAX_HISTORY_SPLITS: f64 = 10_000_000.0;

/// Play the weight-window game — `apply_weight_window`
/// (`src/weight_windows.cpp`).
///
/// Returns what happened; the caller applies it. `state` is updated in place,
/// which is how `ww_born`, `ww_factor` and `n_split` persist across the
/// checkpoints of one history.
pub fn apply(
    mut window: WeightWindow,
    state: &mut WindowState,
    weight: f64,
    seed: &mut u64,
) -> WindowOutcome {
    if !window.is_valid() || weight <= 0.0 {
        return WindowOutcome::Unchanged;
    }

    // First window this particle has seen: remember its midpoint, so the
    // windows it meets later are read RELATIVE to where it started. Without
    // this a particle born deep in a low-importance region would be rouletted
    // immediately on bounds meant for a source-region particle.
    if state.ww_born == -1.0 {
        state.ww_born = 0.5 * (window.lower_weight + window.upper_weight);
    }
    window.scale(state.weight_born / state.ww_born);

    // Absolute floor, checked before anything else.
    if weight < window.weight_cutoff {
        return WindowOutcome::Killed;
    }

    // A particle far above the window: rather than splitting it many times,
    // scale the window up to meet it once and remember the factor.
    if state.ww_factor == 0.0
        && window.max_lb_ratio > 1.0
        && weight > window.lower_weight * window.max_lb_ratio
    {
        state.ww_factor = weight / (window.lower_weight * window.max_lb_ratio);
    }
    if state.ww_factor > 1.0 {
        window.scale(state.ww_factor);
    }

    if weight > window.upper_weight * (1.0 + WEIGHT_WINDOW_REL_TOL) {
        if state.n_split >= MAX_HISTORY_SPLITS {
            return WindowOutcome::Unchanged;
        }
        // Dividing by the SAME dead-banded bound used in the branch condition
        // is what keeps the split count stable when the ratio sits within
        // rounding of an integer. Upstream comments this at length because it
        // was a real defect; reproduced rather than simplified.
        // Upstream rejects `max_split <= 1` when parsing (`weight_windows.cpp:103`).
        // A public Rust field cannot be validated on assignment, so the floor of
        // 1 is enforced here, at the point of use. One is deliberately the floor
        // and not two: upstream's `max_split_ = 1` means "never split" (it gives
        // `n_split = 1`, leaving the weight untouched), and clamping to 2 would
        // silently start splitting where upstream does not. Only 0 is excluded,
        // because it would divide the weight by zero.
        let cap = window.max_split.max(1) as f64;
        let n_split = (weight / ((1.0 + WEIGHT_WINDOW_REL_TOL) * window.upper_weight))
            .ceil()
            .max(2.0)
            .min(cap);
        state.n_split += n_split;
        let copies = n_split.round() as usize;
        return WindowOutcome::Split {
            copies,
            weight: weight / n_split,
        };
    }

    if weight < window.lower_weight * (1.0 - WEIGHT_WINDOW_REL_TOL) {
        let weight_survive = (weight * window.max_split as f64).min(window.survival_weight);
        let w = russian_roulette(weight, weight_survive, seed);
        return if w == 0.0 {
            WindowOutcome::Killed
        } else {
            WindowOutcome::Survived { weight: w }
        };
    }

    WindowOutcome::Unchanged
}

/// A set of weight windows on a regular mesh, optionally resolved in energy —
/// `WeightWindows` (`src/weight_windows.cpp`).
///
/// Bounds are stored `[energy_bin][mesh_bin]` flattened row-major, matching
/// upstream's `lower_ww_(e, m)`.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightWindows {
    /// The spatial mesh the windows live on.
    pub mesh: RegularMesh,
    /// Ascending energy bounds \[eV\], length `n_energy + 1`. A single bin
    /// covering everything is `[e_min, e_max]`.
    pub energy_bounds: Vec<f64>,
    /// `lower[e * n_mesh + m]`. A non-positive entry means "no window".
    pub lower: Vec<f64>,
    /// `upper[e * n_mesh + m]`.
    pub upper: Vec<f64>,
    /// `survival_weight = lower * survival_ratio`.
    pub survival_ratio: f64,
    /// Carried into every [`WeightWindow`] produced.
    pub max_lb_ratio: f64,
    /// Carried into every [`WeightWindow`] produced. Integral — see
    /// [`WeightWindow::max_split`] for why that matters.
    pub max_split: u32,
    /// Carried into every [`WeightWindow`] produced.
    pub weight_cutoff: f64,
}

impl WeightWindows {
    /// Build from bounds already known.
    ///
    /// # Errors
    ///
    /// Mismatched or wrongly-sized bound arrays, fewer than two energy bounds,
    /// unsorted energy bounds, or an `upper` below its `lower` anywhere a
    /// window is valid — the last is the one that silently produces nonsense,
    /// because a particle is then both "above the window" and "below" it and
    /// the split branch wins by ordering alone.
    pub fn new(
        mesh: RegularMesh,
        energy_bounds: Vec<f64>,
        lower: Vec<f64>,
        upper: Vec<f64>,
    ) -> Result<Self, String> {
        if energy_bounds.len() < 2 {
            return Err(format!(
                "weight windows need at least two energy bounds, got {}",
                energy_bounds.len()
            ));
        }
        if !energy_bounds.windows(2).all(|w| w[1] > w[0]) {
            return Err("energy bounds must be strictly ascending".into());
        }
        let n_e = energy_bounds.len() - 1;
        let n_m = mesh.n_bins();
        let want = n_e * n_m;
        if lower.len() != want || upper.len() != want {
            return Err(format!(
                "weight window bounds must be {n_e} energy x {n_m} mesh = {want}; \
                 got lower {} upper {}",
                lower.len(),
                upper.len()
            ));
        }
        for i in 0..want {
            if lower[i] > 0.0 && !(upper[i] > lower[i]) {
                return Err(format!(
                    "window {i}: upper bound {} is not above lower bound {}. A particle \
                     would then be simultaneously above and below the window, and the \
                     split branch would win on ordering alone.",
                    upper[i], lower[i]
                ));
            }
        }
        Ok(Self {
            mesh,
            energy_bounds,
            lower,
            upper,
            survival_ratio: 3.0,
            max_lb_ratio: 1.0,
            max_split: 10,
            weight_cutoff: DEFAULT_WEIGHT_CUTOFF,
        })
    }

    /// Number of energy bins.
    pub fn n_energy(&self) -> usize {
        self.energy_bounds.len() - 1
    }

    /// The window at `(r, e)`, or `None` outside the mesh or the energy range —
    /// `WeightWindows::get_weight_window` (`:277`).
    pub fn look_up(&self, r: Position, e: f64) -> Option<WeightWindow> {
        if e < self.energy_bounds[0] || e > *self.energy_bounds.last().unwrap() {
            return None;
        }
        let m = self.mesh.get_bin(r)?;
        // `lower_bound_index`: the bin whose lower edge is the last one at or
        // below `e`, clamped to the top bin so `e == e_max` lands inside.
        let eb = self
            .energy_bounds
            .partition_point(|&b| b <= e)
            .saturating_sub(1)
            .min(self.n_energy() - 1);
        let i = eb * self.mesh.n_bins() + m;
        Some(WeightWindow {
            lower_weight: self.lower[i],
            upper_weight: self.upper[i],
            survival_weight: self.lower[i] * self.survival_ratio,
            max_lb_ratio: self.max_lb_ratio,
            max_split: self.max_split,
            weight_cutoff: self.weight_cutoff,
        })
    }

    /// **MAGIC** — regenerate the bounds from a forward flux tally
    /// (`WeightWindows::update_weights`, `:409`, the
    /// `WeightWindowUpdateMethod::MAGIC` branch at `:599`).
    ///
    /// `sum` and `sum_sq` are the tally's accumulated first and second moments
    /// over `n` realisations, laid out `[energy][mesh]`; `volumes` is the mesh
    /// cell volume per spatial bin. `threshold` discards a cell whose relative
    /// error exceeds it, `ratio` sets `upper = ratio * lower`.
    ///
    /// The bounds come out **proportional to the forward flux**, normalised so
    /// the largest cell in each energy group has `lower = 1/2`. Cells with no
    /// score, or too noisy a score, are marked `-1` — *not* given a guessed
    /// value. A fabricated window in an unsampled cell is worse than none: it
    /// biases nothing but rouletted away the very particles that would have
    /// resolved it.
    ///
    /// # Errors
    ///
    /// Array sizes that do not match the mesh and energy structure, or `n < 2`
    /// (the relative error needs at least two realisations).
    pub fn update_magic(
        &mut self,
        sum: &[f64],
        sum_sq: &[f64],
        volumes: &[f64],
        n: usize,
        threshold: f64,
        ratio: f64,
    ) -> Result<(), String> {
        let n_m = self.mesh.n_bins();
        let n_e = self.n_energy();
        if sum.len() != n_e * n_m || sum_sq.len() != n_e * n_m {
            return Err(format!(
                "MAGIC needs {n_e} x {n_m} moments; got sum {} sum_sq {}",
                sum.len(),
                sum_sq.len()
            ));
        }
        if volumes.len() != n_m {
            return Err(format!("MAGIC needs {n_m} mesh volumes, got {}", volumes.len()));
        }
        if n < 2 {
            return Err(format!(
                "the relative error needs at least 2 realisations, got {n}"
            ));
        }
        if !(ratio > 1.0) {
            return Err(format!(
                "the upper/lower ratio must exceed 1, got {ratio}; an upper bound at or \
                 below the lower bound makes every particle simultaneously splittable and \
                 rouletted"
            ));
        }
        let nf = n as f64;
        let mut rel_err = vec![f64::INFINITY; n_e * n_m];
        let mut mean = vec![0.0_f64; n_e * n_m];
        for e in 0..n_e {
            for m in 0..n_m {
                let i = e * n_m + m;
                mean[i] = sum[i] / nf;
                if sum[i] > 0.0 {
                    let var = (sum_sq[i] / nf - mean[i] * mean[i]) / (nf - 1.0);
                    rel_err[i] = var.max(0.0).sqrt() / mean[i];
                }
                // Flux density, not integrated score.
                if volumes[m] > 0.0 {
                    mean[i] /= volumes[m];
                }
            }
        }
        // MAGIC normalises per energy group, so a group with little flux still
        // gets usable windows. (CADIS normalises across all groups, which is
        // the other branch upstream and is not ported here.)
        for e in 0..n_e {
            let group_max = (0..n_m)
                .map(|m| mean[e * n_m + m])
                .fold(0.0_f64, f64::max);
            if group_max > 0.0 {
                let f = 1.0 / (2.0 * group_max);
                for m in 0..n_m {
                    mean[e * n_m + m] *= f;
                }
            }
        }
        for e in 0..n_e {
            for m in 0..n_m {
                let i = e * n_m + m;
                self.lower[i] = if sum[i] <= 0.0 || rel_err[i] > threshold {
                    -1.0
                } else {
                    mean[i]
                };
                self.upper[i] = ratio * self.lower[i];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh() -> RegularMesh {
        RegularMesh {
            lower_left: [0.0, 0.0, 0.0],
            upper_right: [4.0, 1.0, 1.0],
            dimension: [4, 1, 1],
        }
    }

    /// A state for which the birth normalisation is the identity, so a test
    /// can reason about the RAW bounds.
    ///
    /// This helper exists because the first version of
    /// `splitting_conserves_total_weight` did **not** use it and failed: with
    /// `WindowState::default()` a particle of weight 1.5 against a `[0.25, 1]`
    /// window is *inside* the window, because the born normalisation scales
    /// the window to `[0.4, 1.6]`. The port was right and the test was wrong.
    /// `the_birth_normalisation_rescales_the_window` now pins that behaviour
    /// on its own rather than leaving it as a trap in every other test.
    fn neutral_state(lower: f64, upper: f64) -> WindowState {
        let mid = 0.5 * (lower + upper);
        WindowState {
            weight_born: mid,
            ww_born: mid,
            ..WindowState::default()
        }
    }

    fn flat_windows(lower: f64, upper: f64) -> WeightWindows {
        WeightWindows::new(
            mesh(),
            vec![0.0, 2.0e7],
            vec![lower; 4],
            vec![upper; 4],
        )
        .unwrap()
    }

    /// A particle inside the window is untouched, and the RNG is not consumed.
    #[test]
    fn a_particle_inside_the_window_is_left_alone() {
        let ww = flat_windows(0.25, 1.0);
        let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();
        let mut st = neutral_state(0.25, 1.0);
        let mut seed = 42;
        let before = seed;
        assert_eq!(apply(w, &mut st, 0.5, &mut seed), WindowOutcome::Unchanged);
        assert_eq!(seed, before, "the window game must not draw when it does nothing");
    }

    /// A cell with no window plays no game, whatever the weight.
    #[test]
    fn an_invalid_window_plays_no_game() {
        let ww = flat_windows(-1.0, 1.0);
        let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();
        assert!(!w.is_valid());
        let mut st = WindowState::default();
        let mut seed = 42;
        for weight in [1.0e-30, 0.5, 1.0e6] {
            assert_eq!(apply(w, &mut st, weight, &mut seed), WindowOutcome::Unchanged);
        }
    }

    /// **The conservation invariant.** A split divides the weight exactly:
    /// `copies * weight_each == weight_before`, to the last bit where the
    /// arithmetic allows it.
    #[test]
    fn splitting_conserves_total_weight() {
        let ww = flat_windows(0.25, 1.0);
        let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();
        let mut seed = 7;
        for weight in [1.5, 2.0, 3.7, 5.0, 9.0, 40.0] {
            let mut st = neutral_state(0.25, 1.0);
            match apply(w, &mut st, weight, &mut seed) {
                WindowOutcome::Split { copies, weight: each } => {
                    let total = copies as f64 * each;
                    assert!(
                        (total - weight).abs() < 1e-12 * weight,
                        "weight {weight} split into {copies} x {each} = {total}"
                    );
                    assert!(copies >= 2, "a split must produce at least 2");
                    assert!(
                        copies as u32 <= w.max_split.max(1),
                        "{copies} copies exceeds max_split {}",
                        w.max_split
                    );
                }
                other => panic!("weight {weight} is above the window but gave {other:?}"),
            }
        }
    }

    /// **The conservation invariant, against every `max_split` a caller can
    /// set** — the regression pin for a defect this port introduced and
    /// upstream cannot have.
    ///
    /// `apply` emits `round(n_split)` copies each carrying `weight / n_split`,
    /// so the two agree only while `n_split` is exactly integral. Upstream gets
    /// that from `int max_split_` (`weight_windows.h:200`). While this port
    /// carried `max_split: f64`, `max_split = 2.5` gave `round(2.5) = 3` copies
    /// of `w/2.5` each — `1.2 w`, weight created by the one routine whose whole
    /// justification is being weight-neutral. The field is now `u32`.
    ///
    /// `max_split = 0` is included because a public field admits it and it is
    /// the value that would divide by zero. It is floored to 1 at the point of
    /// use, matching upstream's `max_split_ = 1` "never split" behaviour rather
    /// than inventing a split upstream would not make.
    #[test]
    fn splitting_conserves_weight_for_every_max_split() {
        let base = flat_windows(0.25, 1.0);
        for cap in 0u32..=12 {
            let mut ww = base.clone();
            ww.max_split = cap;
            let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();
            let mut seed = 20_260_922;
            for weight in [1.5, 2.0, 3.7, 5.0, 9.0, 40.0, 1.0e3] {
                let mut st = neutral_state(0.25, 1.0);
                match apply(w, &mut st, weight, &mut seed) {
                    WindowOutcome::Split { copies, weight: each } => {
                        let total = copies as f64 * each;
                        assert!(
                            (total - weight).abs() <= 1e-12 * weight,
                            "max_split {cap}: weight {weight} split into \
                             {copies} x {each} = {total}, which is not {weight}"
                        );
                        assert!(each.is_finite() && each > 0.0, "max_split {cap}: each = {each}");
                        assert!(
                            copies as u32 <= cap.max(1),
                            "max_split {cap}: {copies} copies exceeds the cap"
                        );
                    }
                    WindowOutcome::Unchanged => {
                        // `cap <= 1` is upstream's "never split"; nothing to check
                        // beyond the fact that no weight moved.
                        assert!(cap <= 1, "max_split {cap} refused to split weight {weight}");
                    }
                    other => panic!("max_split {cap}, weight {weight} gave {other:?}"),
                }
            }
        }
    }

    /// **The unbiasedness invariant for roulette**, measured rather than
    /// asserted: the mean weight of the survivors, times the survival rate,
    /// must equal the weight going in.
    ///
    /// # Result, 2026-09-22
    ///
    /// Printed by the test at each starting weight; the gate is 4 sigma on the
    /// expected weight.
    #[test]
    fn roulette_below_the_window_preserves_expected_weight() {
        let ww = flat_windows(0.25, 1.0);
        let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();
        const N: usize = 200_000;
        let mut worst = 0.0_f64;
        for start in [0.01, 0.05, 0.1, 0.2] {
            let mut seed = 0x0258_u64;
            let mut total = 0.0;
            let mut survivors = 0usize;
            for _ in 0..N {
                let mut st = neutral_state(0.25, 1.0);
                match apply(w, &mut st, start, &mut seed) {
                    WindowOutcome::Killed => {}
                    WindowOutcome::Survived { weight } => {
                        total += weight;
                        survivors += 1;
                    }
                    other => panic!("weight {start} is below the window but gave {other:?}"),
                }
            }
            let mean = total / N as f64;
            // Var of the per-trial weight is p(1-p)w_s^2 with p = w/w_s.
            let w_s = (start * w.max_split as f64).min(w.survival_weight);
            let p = start / w_s;
            let sigma = (p * (1.0 - p) / N as f64).sqrt() * w_s;
            let dev = (mean - start).abs() / sigma;
            println!(
                "start {start:.2}: survivors {survivors}, mean weight {mean:.6} vs \
                 {start:.6} ({dev:.2} sigma); promoted to {w_s:.3}"
            );
            worst = worst.max(dev);
            assert!(dev < 4.0, "expected weight not preserved: {dev:.2} sigma");
        }
        println!("worst deviation across all starting weights: {worst:.2} sigma");
    }

    /// **The birth normalisation, pinned on its own.**
    ///
    /// The first window a particle meets is remembered as `ww_born` (its
    /// midpoint), and every window it meets afterwards is scaled by
    /// `weight_born / ww_born`. So the raw bounds in the array are **not** the
    /// bounds a given particle is judged against, and a test that forgets this
    /// will disagree with a correct implementation.
    ///
    /// # Result, 2026-09-22
    ///
    /// A particle of birth weight 1.0 meeting a `[0.25, 1.0]` window has
    /// `ww_born = 0.625` and is judged against `[0.4, 1.6]`. Weight 1.5 is
    /// therefore **inside** that window and is left alone, while weight 1.7 is
    /// above it and splits. That is what caught the bad test.
    #[test]
    fn the_birth_normalisation_rescales_the_window() {
        let ww = flat_windows(0.25, 1.0);
        let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();

        let mut st = WindowState::default();
        assert_eq!(st.ww_born, -1.0, "unset before the first window is seen");
        let mut seed = 3;
        assert_eq!(
            apply(w, &mut st, 1.5, &mut seed),
            WindowOutcome::Unchanged,
            "1.5 sits inside the BORN-NORMALISED window [0.4, 1.6]"
        );
        assert_eq!(st.ww_born, 0.625, "midpoint of the first window seen");

        // The same particle, now above the normalised upper bound.
        let mut st = WindowState::default();
        match apply(w, &mut st, 1.7, &mut seed) {
            WindowOutcome::Split { copies, weight } => {
                assert_eq!(copies, 2);
                assert!((copies as f64 * weight - 1.7).abs() < 1e-12);
            }
            other => panic!("1.7 is above [0.4, 1.6] but gave {other:?}"),
        }

        // And against the RAW bounds, 1.5 does split — which is exactly the
        // difference the normalisation makes.
        let mut st = neutral_state(0.25, 1.0);
        assert!(matches!(
            apply(w, &mut st, 1.5, &mut seed),
            WindowOutcome::Split { .. }
        ));
    }

    /// The absolute cutoff kills before any game is played.
    #[test]
    fn the_absolute_cutoff_kills_outright() {
        let mut ww = flat_windows(0.25, 1.0);
        ww.weight_cutoff = 1.0e-10;
        let w = ww.look_up(Position::new(0.5, 0.5, 0.5), 1.0e6).unwrap();
        let mut st = neutral_state(0.25, 1.0);
        let mut seed = 1;
        assert_eq!(apply(w, &mut st, 1.0e-20, &mut seed), WindowOutcome::Killed);
    }

    /// Lookups respect the mesh bounds and the energy range.
    #[test]
    fn look_up_respects_the_mesh_and_the_energy_range() {
        let ww = WeightWindows::new(
            mesh(),
            vec![1.0e3, 1.0e5, 2.0e7],
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        )
        .unwrap();
        assert_eq!(ww.n_energy(), 2);
        // Cell 2, low energy group.
        let w = ww.look_up(Position::new(2.5, 0.5, 0.5), 1.0e4).unwrap();
        assert_eq!(w.lower_weight, 0.3);
        // Cell 2, high energy group.
        let w = ww.look_up(Position::new(2.5, 0.5, 0.5), 1.0e6).unwrap();
        assert_eq!(w.lower_weight, 0.7);
        // Top of the range must land INSIDE the top bin, not off the end.
        let w = ww.look_up(Position::new(2.5, 0.5, 0.5), 2.0e7).unwrap();
        assert_eq!(w.lower_weight, 0.7);
        // Outside the mesh, and outside the energy range.
        assert!(ww.look_up(Position::new(9.0, 0.5, 0.5), 1.0e6).is_none());
        assert!(ww.look_up(Position::new(2.5, 0.5, 0.5), 1.0).is_none());
        assert!(ww.look_up(Position::new(2.5, 0.5, 0.5), 3.0e7).is_none());
    }

    /// Malformed window sets are refused at construction, including the one
    /// that would otherwise go unnoticed: `upper <= lower`.
    #[test]
    fn malformed_window_sets_are_refused() {
        assert!(WeightWindows::new(mesh(), vec![1.0], vec![], vec![]).is_err());
        assert!(WeightWindows::new(mesh(), vec![2.0, 1.0], vec![0.1; 4], vec![1.0; 4]).is_err());
        assert!(WeightWindows::new(mesh(), vec![0.0, 1.0], vec![0.1; 3], vec![1.0; 4]).is_err());
        let err =
            WeightWindows::new(mesh(), vec![0.0, 1.0], vec![0.5; 4], vec![0.5; 4]).unwrap_err();
        assert!(err.contains("not above lower bound"), "{err}");
    }

    /// **MAGIC on a known flux shape.** A flux falling by 10x per mesh cell
    /// must give windows falling by 10x per cell, with the brightest cell at
    /// `lower = 1/2`.
    #[test]
    fn magic_bounds_track_the_flux_shape() {
        let mut ww = flat_windows(1.0, 5.0);
        // Exponentially attenuating flux, one realisation-mean per cell, and a
        // sum_sq consistent with a small relative error.
        let n = 100usize;
        let flux = [1.0e3, 1.0e2, 1.0e1, 1.0e0];
        let sum: Vec<f64> = flux.iter().map(|f| f * n as f64).collect();
        // Choose sum_sq so the relative error is ~1 %: var = (0.01 mean)^2.
        let sum_sq: Vec<f64> = flux
            .iter()
            .map(|f| {
                let var = (0.01 * f) * (0.01 * f);
                (var * (n as f64 - 1.0) + f * f * n as f64) * 1.0
            })
            .collect();
        let volumes = vec![1.0_f64; 4];
        ww.update_magic(&sum, &sum_sq, &volumes, n, 0.1, 5.0).unwrap();

        println!("MAGIC lower bounds: {:?}", ww.lower);
        assert!(
            (ww.lower[0] - 0.5).abs() < 1e-12,
            "the brightest cell must normalise to 1/2, got {}",
            ww.lower[0]
        );
        for i in 0..3 {
            let r = ww.lower[i] / ww.lower[i + 1];
            assert!(
                (r - 10.0).abs() < 1e-9,
                "cell {i}/{}: ratio {r}, expected the flux ratio 10",
                i + 1
            );
        }
        for i in 0..4 {
            assert!((ww.upper[i] - 5.0 * ww.lower[i]).abs() < 1e-12);
        }
    }

    /// **A cell MAGIC cannot resolve is marked invalid, not guessed.**
    ///
    /// This is the behaviour that matters most in a shielding problem, because
    /// the deep cells are exactly the ones with no score yet. A fabricated
    /// window there rouletted away the very particles that would have resolved
    /// it, and nothing would have raised an error.
    #[test]
    fn magic_refuses_to_invent_a_window_it_cannot_resolve() {
        let mut ww = flat_windows(1.0, 5.0);
        let n = 100usize;
        // Cell 0 well sampled; cell 1 far too noisy; cells 2 and 3 never scored.
        let sum = vec![1.0e3 * n as f64, 1.0 * n as f64, 0.0, 0.0];
        let sum_sq = vec![
            (0.01e3 * 0.01e3) * (n as f64 - 1.0) + 1.0e6 * n as f64,
            // relative error ~ 5, far over the 0.1 threshold
            (5.0 * 5.0) * (n as f64 - 1.0) + 1.0 * n as f64,
            0.0,
            0.0,
        ];
        ww.update_magic(&sum, &sum_sq, &vec![1.0; 4], n, 0.1, 5.0)
            .unwrap();
        println!("MAGIC lower bounds with unresolved cells: {:?}", ww.lower);
        assert!(ww.lower[0] > 0.0, "the resolved cell must carry a window");
        for i in 1..4 {
            assert_eq!(ww.lower[i], -1.0, "cell {i} must be marked invalid");
            let w = ww
                .look_up(Position::new(i as f64 + 0.5, 0.5, 0.5), 1.0e6)
                .unwrap();
            assert!(!w.is_valid(), "cell {i} must play no game");
        }
    }

    /// MAGIC rejects inputs it cannot compute from, rather than producing
    /// windows built on nothing.
    #[test]
    fn magic_refuses_degenerate_inputs() {
        let mut ww = flat_windows(1.0, 5.0);
        let v = vec![1.0; 4];
        assert!(ww.update_magic(&[1.0; 3], &[1.0; 4], &v, 100, 0.1, 5.0).is_err());
        assert!(ww.update_magic(&[1.0; 4], &[1.0; 4], &[1.0; 3], 100, 0.1, 5.0).is_err());
        assert!(ww.update_magic(&[1.0; 4], &[1.0; 4], &v, 1, 0.1, 5.0).is_err());
        let err = ww
            .update_magic(&[1.0; 4], &[1.0; 4], &v, 100, 0.1, 1.0)
            .unwrap_err();
        assert!(err.contains("must exceed 1"), "{err}");
    }
}
