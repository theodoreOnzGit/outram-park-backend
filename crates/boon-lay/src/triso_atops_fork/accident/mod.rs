// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`inventory_processing`, `release_activity`, `coolant_release`)
//                    trisoatops/trisoatops.py (`accident_case`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// Ported under GPL-3.0; see LICENSE.triso-atops and NOTICE.triso-atops.

//! Depressurisation-accident release: how much of the activity a normal
//! operation left sitting in the fuel and the primary circuit escapes when the
//! coolant blows down.
//!
//! The chain upstream's `accident_case` runs, per nuclide and per node:
//!
//! ```text
//!   integrate(D over the transient T(t))            -> diffusion integral
//!     -> release_fraction(kernel) / (graphite)      -> dimensionless RF
//!       -> release_activity(what is left to release) -> atoms
//!         -> x lambda / 3.7e10                       -> curies
//!           -> x coolant_release fraction + lift-off -> released curies
//! ```
//!
//! The first two steps already live in
//! [`diffusion`](super::diffusion) and
//! [`release_models`](super::release_models); this module adds the rest.
//!
//! # Scope limit
//!
//! Like the whole crate this is **research, education and V&V only**, and an
//! accident source term especially must not be presented as authoritative for
//! emergency planning, emergency response or licensing. See the crate docs.

use super::nuclide_model::ElementGroup;
use uom::si::f64::{Pressure, ThermodynamicTemperature, Time};
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// Universal gas constant, J/(mol·K), as upstream spells it in
/// `coolant_release` (`8.31447`).
const R_GAS: f64 = 8.314_47;

/// The six failure fractions an accident run uses.
///
/// Upstream assembles these into a bare six-element array
/// (`trisoatops.py::accident_case`) from `constants[0..3]` plus
/// `constants[12..13]`; naming them here is what stops an index slip from
/// silently reinterpreting the source term.
///
/// All six are dimensionless fractions in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccidentFractions {
    /// `f_hm` — heavy-metal contamination fraction (fuel outside intact particles).
    pub heavy_metal: f64,
    /// `f_sic` — as-manufactured SiC-defective fraction.
    pub sic: f64,
    /// `f_inc` — incremental in-service failure fraction during normal operation.
    pub incremental: f64,
    /// `f_inc_sic` — incremental SiC-only failure fraction during normal operation.
    pub incremental_sic: f64,
    /// `f_inc_acc` — **additional** incremental failure fraction caused by the
    /// accident itself.
    pub incremental_accident: f64,
    /// `f_inc_sic_acc` — additional incremental SiC-only failure from the accident.
    pub incremental_sic_accident: f64,
}

impl AccidentFractions {
    /// Upstream's `np.sum(fractions)` — all six.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.heavy_metal
            + self.sic
            + self.incremental
            + self.incremental_sic
            + self.incremental_accident
            + self.incremental_sic_accident
    }

    /// Upstream's `np.sum(fractions[:4])` — the four normal-operation fractions.
    ///
    /// This is the fraction already accounted for by the normal-operation
    /// result, and it is what the "has anything been released already?"
    /// condition in [`release_activity`] tests against plate-out.
    #[must_use]
    pub fn normal_operation_sum(&self) -> f64 {
        self.heavy_metal + self.sic + self.incremental + self.incremental_sic
    }

    /// Upstream's `np.sum(fractions[4:])` — the two accident-only fractions.
    #[must_use]
    pub fn accident_sum(&self) -> f64 {
        self.incremental_accident + self.incremental_sic_accident
    }

    /// Upstream's `fractions[0] + fractions[2] + fractions[-2]` — the volatile
    /// path: heavy metal, incremental, and the accident incremental.
    ///
    /// Note `fractions[-2]` is `incremental_accident`, not
    /// `incremental_sic_accident`; the negative index is easy to misread, which
    /// is the reason this is a named method.
    #[must_use]
    pub fn volatile_sum(&self) -> f64 {
        self.heavy_metal + self.incremental + self.incremental_accident
    }
}

/// The per-node normal-operation state an accident release draws down.
///
/// Mirrors upstream's `nodal_data[nuclide][channel, radial, axial]` array, one
/// node's worth. Channel 0 is an **atom count**; channels 1-6 are the
/// activities `normal_operation_node` produces, in the same units it produces
/// them (atoms, or atoms/second for the two rates — the curie conversion
/// happens later).
///
/// Naming the channels is not cosmetic: upstream indexes this array by bare
/// integer at eleven sites in `release_activity` alone, and `[5]` versus `[6]`
/// is the difference between plate-out and clean-up.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NormalOperationNode {
    /// Channel 0 — kernel inventory, **atoms**
    /// (upstream: `inventory / lambda * 3.7e10`).
    pub kernel_inventory_atoms: f64,
    /// Channel 1 — TRISO release rate, atoms/s.
    pub release_rate: f64,
    /// Channel 2 — source rate into the coolant, atoms/s.
    pub source_rate: f64,
    /// Channel 3 — activity held up in the matrix graphite, atoms.
    pub graphite_activity: f64,
    /// Channel 4 — circulating activity, atoms.
    pub circulating_activity: f64,
    /// Channel 5 — plated-out activity, atoms.
    pub plate_out_activity: f64,
    /// Channel 6 — activity removed by the clean-up system, atoms. Zero when
    /// no clean-up system is fitted.
    pub clean_up_activity: f64,
}

/// Which material's release fraction is being converted to an activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseMaterial {
    /// Release out of the fuel kernel.
    Kernel,
    /// Release out of the matrix graphite.
    Graphite,
}

/// Distribute a per-radial-ring inventory evenly over the axial nodes.
///
/// Ports `inventory_processing`. A run may supply inventories either already
/// resolved per node (radial × time × axial, passed through untouched) or only
/// per radial ring, in which case each ring's inventory is **divided equally**
/// among `n_axial` nodes.
///
/// # Arguments
/// - `ring_inventory` — inventory for one radial ring, in whatever unit the
///   caller is working in (upstream uses curies here, before the
///   `/ lambda * 3.7e10` conversion to atoms). Must be finite.
/// - `n_axial` — number of axial nodes to spread it over; must be `>= 1`.
///
/// # Returns
/// `n_axial` equal shares, each `ring_inventory / n_axial`.
///
/// # Divergence from upstream
/// Upstream handles `ndim == 3` (pass through) and `ndim == 2` (split) and has
/// **no `else`**, so any other rank falls off the end and returns `None`,
/// which then fails confusingly downstream. Rust's type system removes that
/// case: the pass-through variant is simply not routed here.
///
/// # Panics
/// Panics if `n_axial == 0`.
#[must_use]
pub fn distribute_inventory_axially(ring_inventory: f64, n_axial: usize) -> Vec<f64> {
    assert!(n_axial >= 1, "n_axial must be at least 1");
    vec![ring_inventory / n_axial as f64; n_axial]
}

/// The activity still available for accident release at one node, in atoms.
///
/// Ports `release_activity`. The idea is a mass balance: take the node's total
/// kernel inventory, scale it by the failure fraction appropriate to the
/// nuclide's transport group, then subtract whatever normal operation has
/// already moved elsewhere — graphite hold-up, circulating activity, plate-out
/// and clean-up.
///
/// Upstream evaluates two expressions and selects between them with a
/// per-node mask:
///
/// ```text
///   condition = inventory * sum(fractions[:4]) - plate_out > 0
/// ```
///
/// i.e. *has normal operation already released more than it plated out?* Where
/// that holds, the fuller expression (scaled by the group fraction, and
/// subtracting plate-out as well) is used; elsewhere the accident-only
/// fractions apply and plate-out is not subtracted.
///
/// # Arguments
/// - `group` — the nuclide's transport group, which picks the failure fraction.
/// - `fractions` — the six accident failure fractions.
/// - `node` — that node's normal-operation state.
/// - `release_fraction` — the dimensionless RF from
///   [`release_fraction_transient`](super::release_models::release_fraction_transient).
/// - `clean` — whether a clean-up system is fitted; when `true` the clean-up
///   channel is subtracted too.
/// - `material` — kernel or graphite.
///
/// # Returns
/// Released activity at this node, **atoms**. May be negative if the
/// subtractions exceed the scaled inventory; upstream does not clamp, and
/// neither does this — a negative value is a signal that the normal-operation
/// and accident fraction sets are inconsistent, and hiding it would hide that.
///
/// # UPSTREAM DEFECT reproduced behind a flag: the silver group is `z == 48`
///
/// At `calculation_functions.py:906` the silver branch reads
/// `z == 47 or z == 48` — silver and **cadmium**. Every other silver-group
/// test in the file (lines 208, 722, 747, 775) reads `z == 47 or z == 46`,
/// silver and **palladium**. Both affected nuclides ship in the table
/// (`Pd-107`, `Z = 46`; `Cd-113`, `Z = 48`), so this is reachable: under
/// upstream, Pd-107 falls through to the all-fractions sum instead of
/// receiving `fract = 1`, and Cd-113 wrongly receives the silver treatment.
///
/// Four sites against one make `48` the near-certain typo, so this port treats
/// [`ElementGroup::Silver`] (Ag **and** Pd, as the rest of the model defines
/// it) as the silver branch. Pass `upstream_cadmium_typo = true` to reproduce
/// the stock behaviour for code-to-code comparison.
#[must_use]
pub fn release_activity(
    group: ElementGroup,
    fractions: AccidentFractions,
    node: NormalOperationNode,
    release_fraction: f64,
    clean: bool,
    material: ReleaseMaterial,
    upstream_cadmium_typo: bool,
    z: u32,
) -> f64 {
    let activities = match material {
        ReleaseMaterial::Graphite => node.graphite_activity,
        ReleaseMaterial::Kernel => {
            let is_silver_branch = if upstream_cadmium_typo {
                z == 47 || z == 48
            } else {
                matches!(group, ElementGroup::Silver)
            };
            let fract = if matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen) {
                fractions.volatile_sum()
            } else if is_silver_branch {
                1.0
            } else {
                fractions.total()
            };

            let condition = node.kernel_inventory_atoms * fractions.normal_operation_sum()
                - node.plate_out_activity
                > 0.0;
            let clean_up = if clean { node.clean_up_activity } else { 0.0 };

            if condition {
                node.kernel_inventory_atoms * fract
                    - node.graphite_activity
                    - node.circulating_activity
                    - node.plate_out_activity
                    - clean_up
            } else {
                node.kernel_inventory_atoms * fractions.accident_sum()
                    - node.graphite_activity
                    - node.circulating_activity
                    - clean_up
            }
        }
    };
    release_fraction * activities
}

/// Fraction of the primary coolant vented, over the venting window.
///
/// Ports `coolant_release`. During a depressurisation the coolant leaves while
/// the core is heating: by the ideal gas law at fixed pressure and volume,
/// `n = PV/RT`, so `dn/dt = -(P/R) (dT/dt) / T^2`. Upstream integrates that
/// trapezoidally, normalises by the initial mole count, and reports only the
/// samples where the bed is heating (`dT/dt >= 0`), which is when gas is
/// actually being pushed out.
///
/// # Arguments
/// - `times` — sample times, SI seconds, strictly increasing, at least 2.
/// - `mean_dtdt` — mean `dT/dt` across the core at each sample, K/s (upstream
///   averages over the radial and axial axes). Same length as `times`.
/// - `hot_node_temperature` — temperature of the reference (hottest) node at
///   each sample. Upstream defaults to the innermost ring, centre axial node,
///   and takes these in **degrees Celsius**, converting with `+ 273.15`
///   inline; this port takes a `uom` temperature so the unit cannot be
///   mistaken. Same length as `times`.
/// - `pressure` — system pressure; upstream's default is `101.325` in the
///   **kilopascal** the `R = 8.31447` J/(mol·K) denominator implies.
///
/// # Returns
/// `(fraction, vent_times)` — the released fraction at each venting sample and
/// the times those correspond to. Upstream forces the first element to exactly
/// `1`, which this reproduces.
///
/// # The `pressure` argument cannot change the answer — measured, not assumed
///
/// `frac = |integral / n_0|`, and both the integral (`dn/dt = -(P/R)...`) and
/// the normalisation (`n_0 = P/(R T_0)`) carry the same `P/R` factor, so it
/// cancels exactly. Verified against upstream on 2026-09-21: feeding
/// `P = 1.0`, `101.325`, `202.65` and `5000.0` through
/// `calculation_functions.coolant_release` returns **bit-identical** fractions.
///
/// **This port agrees to within 1 ulp, not bit-exactly.** The cancellation is
/// algebraic, and its exactness depends on operation order: upstream's NumPy
/// expression happens to cancel exactly, while this port's
/// `-p / R * dTdt / T / T` against `p / R / T_0` does not for every `p`
/// (measured: `1.0` kPa moves the second sample by one ulp,
/// `5.07056142185376e-2` against `5.070561421853761e-2`). That is a
/// floating-point artefact of the same algebra, not a physical dependence.
///
/// The parameter is kept because it is upstream's signature and because a
/// future formulation that tracks absolute moles would need it — but a caller
/// tuning it expecting a different release fraction is wasting their time, and
/// this is the only place that says so. [`pressure_does_not_affect_the_fraction`]
/// pins it, since a fixture comparison cannot: upstream's own output does not
/// depend on it either.
///
/// # Two upstream quirks preserved
///
/// 1. **`frac[0] = 1`** is hard-coded, so the first venting sample always
///    reports a fully released coolant regardless of the integral.
/// 2. **The venting samples need not be contiguous.** `np.where(dTdt_avg >= 0)`
///    selects every heating sample, so a transient that cools and re-heats
///    produces a gappy set, and the returned times are that same gappy set.
///
/// # Panics
/// Panics if the three slices differ in length, if fewer than two samples are
/// supplied, or if any temperature is at or below absolute zero.
#[must_use]
pub fn coolant_release(
    times: &[Time],
    mean_dtdt: &[f64],
    hot_node_temperature: &[ThermodynamicTemperature],
    pressure: Pressure,
) -> (Vec<f64>, Vec<Time>) {
    assert!(
        times.len() == mean_dtdt.len() && times.len() == hot_node_temperature.len(),
        "times, mean_dtdt and hot_node_temperature must be the same length; got {}, {}, {}",
        times.len(),
        mean_dtdt.len(),
        hot_node_temperature.len()
    );
    assert!(times.len() >= 2, "need at least two samples");

    let p = pressure.get::<kilopascal>();
    let t_s: Vec<f64> = times.iter().map(|t| t.get::<second>()).collect();

    // dn/dt = -(P/R) (dT/dt) / T^2, with T in kelvin.
    let dndt: Vec<f64> = (0..t_s.len())
        .map(|i| {
            let t_k = hot_node_temperature[i].get::<degree_celsius>() + 273.15;
            assert!(
                t_k > 0.0,
                "temperature must exceed absolute zero; got {t_k} K"
            );
            -p / R_GAS * mean_dtdt[i] / t_k / t_k
        })
        .collect();

    let t0_k = hot_node_temperature[0].get::<degree_celsius>() + 273.15;
    let n_0 = p / R_GAS / t0_k;

    // Trapezoidal cumulative integral, matching upstream's
    // cumsum(0.5 * diff(times, prepend=times[0]) * (dndt + shifted dndt)).
    let mut integral = vec![0.0; t_s.len()];
    let mut running = 0.0;
    for i in 0..t_s.len() {
        let dt = if i == 0 { 0.0 } else { t_s[i] - t_s[i - 1] };
        let prev = if i == 0 { 0.0 } else { dndt[i - 1] };
        running += 0.5 * dt * (dndt[i] + prev);
        integral[i] = running;
    }
    integral[0] = 0.0;

    let mut fraction = Vec::new();
    let mut vent_times = Vec::new();
    for i in 0..t_s.len() {
        if mean_dtdt[i] >= 0.0 {
            fraction.push((integral[i] / n_0).abs());
            vent_times.push(times[i]);
        }
    }
    if let Some(first) = fraction.first_mut() {
        // Upstream: frac[0] = 1.
        *first = 1.0;
    }
    (fraction, vent_times)
}

/// Mean `dT/dt` at each sample, averaged across the core.
///
/// Ports the first three lines of `coolant_release`, which upstream computes
/// inline: a backward difference in time, zero-padded at the first sample, then
/// averaged over the radial and axial axes.
///
/// Separated out because it is the only part of the calculation that needs the
/// full 3-D temperature field; splitting it lets [`coolant_release`] stay a
/// slice-based function like the rest of the port.
///
/// # Arguments
/// - `times` — sample times, SI seconds, strictly increasing.
/// - `node_temperatures` — `[node][time]` temperature history for every node
///   in the core (radial × axial flattened; the average does not care about
///   the layout). Every inner slice must match `times` in length.
///
/// # Returns
/// Mean `dT/dt` in K/s at each sample; the first entry is `0` by construction.
///
/// # Note on upstream's epsilon
/// Upstream divides by `np.diff(times) + np.finfo(float).eps` to avoid a
/// zero-division on repeated timestamps. This port asserts strictly increasing
/// times instead, which is the condition that epsilon was papering over.
///
/// # Panics
/// Panics on ragged input, fewer than two samples, or non-increasing times.
#[must_use]
pub fn mean_temperature_rate(
    times: &[Time],
    node_temperatures: &[Vec<ThermodynamicTemperature>],
) -> Vec<f64> {
    assert!(times.len() >= 2, "need at least two samples");
    assert!(!node_temperatures.is_empty(), "need at least one node");
    let t_s: Vec<f64> = times.iter().map(|t| t.get::<second>()).collect();
    for w in t_s.windows(2) {
        assert!(w[1] > w[0], "times must be strictly increasing");
    }

    let n_t = times.len();
    let mut sum = vec![0.0; n_t];
    for node in node_temperatures {
        assert_eq!(
            node.len(),
            n_t,
            "every node needs one temperature per sample"
        );
        for i in 1..n_t {
            let dt = t_s[i] - t_s[i - 1];
            let d_temp = node[i].get::<degree_celsius>() - node[i - 1].get::<degree_celsius>();
            sum[i] += d_temp / dt;
        }
    }
    let n = node_temperatures.len() as f64;
    sum.iter().map(|s| s / n).collect()
}

/// Total released activity for one nuclide over the accident, in **curies**.
///
/// Ports the per-nuclide body of `trisoatops.py::accident_case`:
///
/// ```text
///   released(t) = frac(t) * (kernel + graphite)          [curies]
///               + circulating + x_liftoff * plate_out    [curies, released at once]
/// ```
///
/// The first term is what diffuses out of the fuel and matrix during the
/// transient, scaled by the fraction of coolant that has actually vented by
/// time `t`. The second is the primary-circuit inventory: circulating activity
/// leaves with the coolant, and a `x_liftoff` share of the plated-out activity
/// is re-entrained by the blowdown. That second term is **not** scaled by
/// `frac`, and is constant in `t`.
///
/// # Arguments
/// - `kernel_release_curies` — already-converted kernel release at each
///   transient sample (i.e. [`release_activity`] on the kernel, times
///   `lambda / 3.7e10`).
/// - `graphite_release_curies` — the same for the graphite path.
/// - `vent_fraction` — the vented-coolant fraction at each sample, from
///   [`coolant_release`]. Must match the two release slices in length.
/// - `circulating_curies`, `plate_out_curies` — the node-summed
///   normal-operation inventories, already in curies.
/// - `x_liftoff` — re-entrained share of plate-out, dimensionless `[0, 1]`.
///
/// # Returns
/// Released activity in curies at each sample.
///
/// # UPSTREAM DEFECT not reproduced: `accident_temp[:, :-rmv, :]`
///
/// `accident_case` truncates the temperature history to the venting window
/// with
///
/// ```python
/// rmv = np.size(times) - np.size(times_short)
/// accident_temp = accident_temp[:, :-rmv, :]
/// ```
///
/// When nothing is truncated — every sample is a venting sample, which is
/// exactly what a monotonic heat-up produces — `rmv` is `0`, and `[:-0]` in
/// Python is `[:0]`, i.e. **the empty slice**. The temperature history is
/// silently discarded and every downstream integral is empty.
///
/// This port cannot reproduce that: the slices are passed in already aligned,
/// and a length mismatch is an assertion rather than an empty result. Recorded
/// here because a reader comparing against a stock TRISO-ATOPS run on a
/// monotonic transient will see upstream produce nothing and should know why.
///
/// # Panics
/// Panics if the three per-sample slices differ in length, or if `x_liftoff`
/// is outside `[0, 1]`.
#[must_use]
pub fn accident_release_curies(
    kernel_release_curies: &[f64],
    graphite_release_curies: &[f64],
    vent_fraction: &[f64],
    circulating_curies: f64,
    plate_out_curies: f64,
    x_liftoff: f64,
) -> Vec<f64> {
    assert!(
        kernel_release_curies.len() == graphite_release_curies.len()
            && kernel_release_curies.len() == vent_fraction.len(),
        "kernel, graphite and vent_fraction must be the same length; got {}, {}, {}",
        kernel_release_curies.len(),
        graphite_release_curies.len(),
        vent_fraction.len()
    );
    assert!(
        (0.0..=1.0).contains(&x_liftoff),
        "x_liftoff must be a fraction in [0, 1]; got {x_liftoff}"
    );
    let circuit = circulating_curies + x_liftoff * plate_out_curies;
    (0..kernel_release_curies.len())
        .map(|i| {
            vent_fraction[i] * (kernel_release_curies[i] + graphite_release_curies[i]) + circuit
        })
        .collect()
}

/// Convert an activity in atoms to curies: `atoms * lambda / 3.7e10`.
///
/// Ports the `* lam / 3.7e10` conversion `accident_case` applies to both
/// release paths. Provided as a named function because upstream writes that
/// literal at four separate sites, and `3.7e10` is easy to mistype.
///
/// # Arguments
/// - `atoms` — activity as an atom count.
/// - `decay_constant` — `lambda`, s⁻¹.
#[must_use]
pub fn atoms_to_curies(atoms: f64, decay_constant: f64) -> f64 {
    atoms * decay_constant / 3.7e10
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::pressure::kilopascal;

    fn fractions() -> AccidentFractions {
        AccidentFractions {
            heavy_metal: 1e-4,
            sic: 1e-4,
            incremental: 2.3e-5,
            incremental_sic: 3.6e-5,
            incremental_accident: 5e-5,
            incremental_sic_accident: 7e-5,
        }
    }

    #[test]
    fn fraction_sums_match_the_upstream_slices() {
        let f = fractions();
        assert!((f.total() - 3.79e-4).abs() < 1e-15);
        assert!((f.normal_operation_sum() - 2.59e-4).abs() < 1e-15);
        assert!((f.accident_sum() - 1.2e-4).abs() < 1e-15);
        // fractions[0] + fractions[2] + fractions[-2]
        assert!((f.volatile_sum() - (1e-4 + 2.3e-5 + 5e-5)).abs() < 1e-15);
    }

    #[test]
    fn inventory_splits_evenly_and_conserves() {
        let split = distribute_inventory_axially(44.5, 5);
        assert_eq!(split.len(), 5);
        assert!((split.iter().sum::<f64>() - 44.5).abs() < 1e-12);
        assert!((split[0] - 8.9).abs() < 1e-12);
    }

    #[test]
    fn graphite_release_reads_the_graphite_channel_only() {
        let node = NormalOperationNode {
            graphite_activity: 1234.0,
            kernel_inventory_atoms: 9e9,
            ..Default::default()
        };
        let got = release_activity(
            ElementGroup::SpecialMetal,
            fractions(),
            node,
            0.5,
            false,
            ReleaseMaterial::Graphite,
            false,
            55,
        );
        assert!((got - 0.5 * 1234.0).abs() < 1e-9);
    }

    /// The cadmium typo must actually change the answer, or documenting it is
    /// worthless. Pd-107 (Z=46) is the discriminating case.
    #[test]
    fn the_cadmium_typo_changes_palladium() {
        let node = NormalOperationNode {
            kernel_inventory_atoms: 1e12,
            ..Default::default()
        };
        let corrected = release_activity(
            ElementGroup::Silver,
            fractions(),
            node,
            1.0,
            false,
            ReleaseMaterial::Kernel,
            false,
            46,
        );
        let stock = release_activity(
            ElementGroup::Silver,
            fractions(),
            node,
            1.0,
            false,
            ReleaseMaterial::Kernel,
            true,
            46,
        );
        // Corrected: Pd takes fract = 1. Stock: it falls through to sum(fractions).
        assert!(
            corrected > stock,
            "Pd-107 must differ: {corrected} vs {stock}"
        );
        assert!((corrected - 1e12).abs() / 1e12 < 1e-9, "fract = 1 path");
    }

    #[test]
    fn mean_rate_is_zero_at_the_first_sample_and_matches_a_linear_ramp() {
        let times: Vec<Time> = [0.0, 10.0, 20.0]
            .iter()
            .map(|t| Time::new::<second>(*t))
            .collect();
        // Two nodes, both ramping at 2 K/s.
        let node: Vec<ThermodynamicTemperature> = [300.0, 320.0, 340.0]
            .iter()
            .map(|t| ThermodynamicTemperature::new::<degree_celsius>(*t))
            .collect();
        let rate = mean_temperature_rate(&times, &[node.clone(), node]);
        assert_eq!(rate[0], 0.0);
        assert!((rate[1] - 2.0).abs() < 1e-12);
        assert!((rate[2] - 2.0).abs() < 1e-12);
    }

    /// Pressure is inert: a 5000x change must not move the fraction.
    ///
    /// Asserted to 1 ulp rather than bit-exactly, and the distinction is real:
    /// upstream returns bit-identical values across `P`, this port does not,
    /// because the `P/R` cancellation is algebraic and its exactness depends on
    /// operation order. See the note on [`coolant_release`]. A tolerance of
    /// `4 * f64::EPSILON` is tight enough that any genuine `P` dependence —
    /// which would scale with `P` — fails immediately.
    ///
    /// This is not a property the code-to-code fixture can test: upstream's
    /// output is equally independent of `P`, so every fixture row would pass
    /// whatever the port did with it. Pinning it here is what stops a future
    /// refactor from quietly making the parameter load-bearing.
    #[test]
    fn pressure_does_not_affect_the_fraction() {
        let times: Vec<Time> = [0.0, 100.0, 200.0, 300.0]
            .iter()
            .map(|t| Time::new::<second>(*t))
            .collect();
        let dtdt = vec![0.0, 1.0, 1.0, 1.0];
        let temps: Vec<ThermodynamicTemperature> = [500.0, 600.0, 700.0, 800.0]
            .iter()
            .map(|t| ThermodynamicTemperature::new::<degree_celsius>(*t))
            .collect();
        let base = coolant_release(&times, &dtdt, &temps, Pressure::new::<kilopascal>(101.325)).0;
        for p in [1.0, 202.65, 5000.0] {
            let other = coolant_release(&times, &dtdt, &temps, Pressure::new::<kilopascal>(p)).0;
            assert_eq!(base.len(), other.len());
            for (i, (b, o)) in base.iter().zip(other.iter()).enumerate() {
                let dev = if *b == 0.0 {
                    o.abs()
                } else {
                    (b - o).abs() / b.abs()
                };
                assert!(
                    dev <= 4.0 * f64::EPSILON,
                    "pressure {p} kPa moved sample {i}: {b} vs {o} (rel {dev:e})"
                );
            }
        }
    }

    #[test]
    fn circuit_inventory_is_not_scaled_by_the_vent_fraction() {
        // Zero diffusive release: everything reported must be the circuit term,
        // constant in time and independent of how much coolant has vented.
        let out = accident_release_curies(
            &[0.0, 0.0, 0.0],
            &[0.0, 0.0, 0.0],
            &[1.0, 0.3, 0.05],
            7.0,
            100.0,
            0.25,
        );
        for v in &out {
            assert!((v - (7.0 + 0.25 * 100.0)).abs() < 1e-12, "got {v}");
        }
    }

    #[test]
    fn diffusive_release_is_scaled_by_the_vent_fraction() {
        let out = accident_release_curies(&[10.0, 10.0], &[2.0, 2.0], &[1.0, 0.5], 0.0, 0.0, 0.0);
        assert!((out[0] - 12.0).abs() < 1e-12);
        assert!((out[1] - 6.0).abs() < 1e-12);
    }

    #[test]
    fn curie_conversion_matches_the_upstream_literal() {
        // 3.7e10 Bq per curie; A = lambda N.
        let lam = 7.302_186_793e-10_f64;
        assert!((atoms_to_curies(1e18, lam) - 1e18 * lam / 3.7e10).abs() < 1e-18);
        assert_eq!(atoms_to_curies(0.0, lam), 0.0);
    }

    #[test]
    fn coolant_release_vents_only_while_heating_and_pins_the_first_sample() {
        let times: Vec<Time> = [0.0, 100.0, 200.0, 300.0]
            .iter()
            .map(|t| Time::new::<second>(*t))
            .collect();
        // Heat, heat, then cool: the last sample must not be a venting sample.
        let dtdt = vec![0.0, 1.0, 0.5, -1.0];
        let temps: Vec<ThermodynamicTemperature> = [500.0, 600.0, 650.0, 600.0]
            .iter()
            .map(|t| ThermodynamicTemperature::new::<degree_celsius>(*t))
            .collect();
        let (frac, vent) =
            coolant_release(&times, &dtdt, &temps, Pressure::new::<kilopascal>(101.325));
        assert_eq!(vent.len(), 3, "three heating samples");
        assert_eq!(frac.len(), 3);
        assert_eq!(frac[0], 1.0, "upstream pins the first sample to 1");
        assert!(frac[1] > 0.0 && frac[1].is_finite());
    }
}
