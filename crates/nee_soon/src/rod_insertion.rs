//! Apply a smeared control-rod absorber to a multigroup library.
//!
//! # What this does
//!
//! [`crate::htr10_rmc::control_rod`] gives the **atom densities** a smeared rod
//! adds to the side-reflector boring band. This module turns those into a
//! **macroscopic absorption cross section per energy group** and adds it to a
//! zone of an [`MgxsLibrary`], so a deterministic solve can be run at any rod
//! position without a new Monte Carlo run.
//!
//! That is the whole point: a Monte Carlo point costs 1-2 hours, so a rod
//! position sweep dense enough to fit a surrogate to is not reachable through
//! MC. The rod is a pure absorber addition, which is the one axis that *can*
//! be applied to already-condensed constants honestly.
//!
//! # The 1/v approximation, and why it is defensible here
//!
//! B-10's `(n, alpha)` absorption is the textbook `1/v` absorber from thermal
//! energies up to roughly 100 keV. Rather than assume a group-average, this
//! module **integrates** `sigma(E) = sigma_0 sqrt(E_0/E)` over each group
//! against a `1/E` (constant-lethargy) weighting spectrum, which is the right
//! weight for the epithermal range where most of the groups sit:
//!
//! ```text
//! <sigma>_g = sigma_0 sqrt(E_0) * 2 (E_lo^-1/2 - E_hi^-1/2) / ln(E_hi / E_lo)
//! ```
//!
//! **Where this is wrong, stated up front:**
//!
//! - The thermal group's true weight is Maxwellian, not `1/E`. For a `1/v`
//!   absorber the Maxwellian average carries the familiar `sqrt(pi)/2` factor
//!   against the `2200 m/s` value; this module does not apply it, so the
//!   thermal group is **over-estimated by about 13 %**.
//! - B-10 departs from `1/v` above ~100 keV, where this will be wrong — but
//!   its absorption there is negligible against the resonance and thermal
//!   contributions, so the error in `k` is small.
//! - **Self-shielding is absent.** This is the large one, and it compounds the
//!   smearing error already documented in [`crate::htr10_rmc::control_rod`]:
//!   a real B4C annulus is black to thermal neutrons and shields its own
//!   interior, while a smeared dilute absorber does not. Both errors push the
//!   same way, so rod worth from this path is expected to be **over-predicted**.
//!
//! Treat results from this module as an *uncalibrated hierarchical surrogate*
//! in the sense of the workspace model-hierarchy rule: derived from geometry
//! and published densities with nothing tuned, and to be reported with its
//! disagreement rather than corrected into agreement.

use crate::htr10_rmc::control_rod::smeared_composition;
use crate::mgxs::MgxsLibrary;

/// B-10 `(n, alpha)` cross section at 2200 m/s \[barn\].
///
/// The standard thermal reference value. Used with the `1/v` law below; no
/// resonance structure is represented because B-10 has none of consequence.
pub const SIGMA_A_B10_2200: f64 = 3837.0;
/// Energy of a 2200 m/s neutron \[eV\].
pub const E_2200: f64 = 0.0253;
/// Natural abundance of B-10 in boron \[atom fraction\].
pub const B10_ABUNDANCE: f64 = 0.199;
/// B-11 absorption is ~5 mb thermal — four orders below B-10's, and it is
/// carried explicitly rather than silently dropped so a reader can see it was
/// considered.
pub const SIGMA_A_B11_2200: f64 = 0.005;

/// Group-averaged `1/v` cross section over `[e_lo, e_hi]` eV against a `1/E`
/// weighting spectrum \[barn\].
///
/// Returns `sigma_0` evaluated at the group midpoint if the group is
/// degenerate (`e_hi <= e_lo`), rather than dividing by a zero logarithm.
#[must_use]
pub fn one_over_v_group_average(sigma_2200: f64, e_lo: f64, e_hi: f64) -> f64 {
    if e_lo <= 0.0 || e_hi <= e_lo {
        let e = if e_lo > 0.0 { e_lo } else { E_2200 };
        return sigma_2200 * (E_2200 / e).sqrt();
    }
    let num = 2.0 * (e_lo.powf(-0.5) - e_hi.powf(-0.5));
    let den = (e_hi / e_lo).ln();
    sigma_2200 * E_2200.sqrt() * num / den
}

/// Macroscopic absorption added per group by a rod insertion \[cm^-1\].
///
/// `group_edges` must be ascending in eV and have `n_groups + 1` entries —
/// i.e. the ordering [`MgxsLibrary::groups`] stores, **not** the descending
/// order the GeN-Foam bridge wants.
#[must_use]
pub fn rod_absorption_per_group(fraction_inserted: f64, group_edges: &[f64]) -> Vec<f64> {
    let comp = smeared_composition(fraction_inserted);
    let n_b10 = comp.natural_boron * B10_ABUNDANCE;
    let n_b11 = comp.natural_boron * (1.0 - B10_ABUNDANCE);

    group_edges
        .windows(2)
        .map(|w| {
            let (lo, hi) = (w[0], w[1]);
            let s10 = one_over_v_group_average(SIGMA_A_B10_2200, lo, hi);
            let s11 = one_over_v_group_average(SIGMA_A_B11_2200, lo, hi);
            // atoms/(b.cm) * barn = cm^-1
            n_b10 * s10 + n_b11 * s11
        })
        .collect()
}

/// A copy of `lib` with the rod absorber added to zone `zone_idx`.
///
/// Both `absorption` **and** `total` are increased by the same amount, because
/// an absorber removes neutrons from the group: raising absorption alone would
/// leave the balance `Sigma_t = Sigma_a + Sigma_s,row` broken, and
/// [`crate::genfoam_xs`] infers the solver's absorption from exactly that
/// difference — so the rod would have had no effect at all on the eigenvalue.
/// See `op-q6yy` for the measured consequence of getting that wrong.
///
/// Returns `lib` unchanged if `zone_idx` is out of range, so a mis-indexed
/// sweep produces a visibly flat curve rather than a panic mid-map.
#[must_use]
pub fn with_rod_inserted(lib: &MgxsLibrary, zone_idx: usize, fraction_inserted: f64) -> MgxsLibrary {
    let add = rod_absorption_per_group(fraction_inserted, lib.groups.edges());
    let mut out = lib.clone();
    if let Some(z) = out.zones.get_mut(zone_idx) {
        for (g, &d) in add.iter().enumerate() {
            if let Some(a) = z.absorption.get_mut(g) {
                *a += d;
            }
            if let Some(t) = z.total.get_mut(g) {
                *t += d;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `1/v` group average reduces to the reference value on a narrow
    /// group at 2200 m/s, and falls as `1/sqrt(E)` at higher energy.
    ///
    /// # Methodology
    ///
    /// A narrow group bracketing `E_2200 = 0.0253 eV` must return
    /// approximately `sigma_2200` itself, since the weighting cannot move the
    /// average far across a narrow interval. A decade higher, the average must
    /// drop by roughly `sqrt(10)`.
    ///
    /// # Results, measured 2026-09-20
    ///
    /// Narrow group `[0.0252, 0.0254]`: **3837.0 barn**, 0.0 % from the
    /// reference. Group `[0.252, 0.254]`: **1213.5 barn**, a factor
    /// **3.162** below — `sqrt(10) = 3.1623`, so the law is reproduced to
    /// better than 0.1 %.
    #[test]
    fn one_over_v_average_reproduces_the_law() {
        let at_2200 = one_over_v_group_average(SIGMA_A_B10_2200, 0.0252, 0.0254);
        assert!(
            (at_2200 / SIGMA_A_B10_2200 - 1.0).abs() < 1.0e-3,
            "narrow group at 2200 m/s gave {at_2200}"
        );
        let decade_up = one_over_v_group_average(SIGMA_A_B10_2200, 0.252, 0.254);
        let ratio = at_2200 / decade_up;
        assert!(
            (ratio - 10.0_f64.sqrt()).abs() < 0.01,
            "expected a sqrt(10) drop per decade, got {ratio}"
        );
    }

    /// Inserting a rod raises absorption, lowers `k_inf`, and does so
    /// monotonically — and it keeps the cross-section balance intact.
    ///
    /// # Methodology
    ///
    /// A two-zone, two-group library: a fissile zone and an absorber-free
    /// "reflector" zone that the rod is inserted into. The rod fraction is
    /// swept `0.0, 0.25, 0.5, 0.75, 1.0` and `k_inf` recorded. The balance
    /// check compares [`crate::mgxs::ZoneMgxs::inferred_absorption`] against
    /// the tallied absorption before and after insertion: if the rod were
    /// added to `absorption` only, the inferred value would be unchanged and
    /// the solver would never see the rod.
    ///
    /// # Results, measured 2026-09-20
    ///
    /// `k_inf` falls monotonically from **1.500000** (withdrawn) to
    /// **1.315789** (fully inserted) on this synthetic fixture, and the
    /// inferred-minus-tallied absorption gap is unchanged to 1e-15 at every
    /// step — i.e. the balance the bridge depends on is preserved.
    #[test]
    fn rod_insertion_lowers_k_and_preserves_the_balance() {
        use crate::mgxs::{GroupStructure, ZoneMgxs};
        let lib = MgxsLibrary {
            groups: GroupStructure::new(vec![1.0e-5, 1.0, 2.0e7]).expect("edges ascend"),
            zones: vec![
                ZoneMgxs {
                    name: "fuel".into(),
                    flux: vec![1.0, 1.0],
                    total: vec![0.50, 0.50],
                    absorption: vec![0.02, 0.02],
                    nu_fission: vec![0.03, 0.03],
                    kappa_fission: vec![0.0, 0.0],
                    scatter: vec![vec![0.40, 0.08], vec![0.10, 0.38]],
                    chi: vec![0.0, 1.0],
                },
                ZoneMgxs {
                    name: "boring band".into(),
                    flux: vec![1.0, 1.0],
                    total: vec![0.40, 0.40],
                    absorption: vec![0.001, 0.001],
                    nu_fission: vec![0.0, 0.0],
                    kappa_fission: vec![0.0, 0.0],
                    scatter: vec![vec![0.30, 0.099], vec![0.05, 0.349]],
                    chi: vec![0.0, 0.0],
                },
            ],
        };

        let gap_before =
            lib.zones[1].inferred_absorption(0).unwrap() - lib.zones[1].absorption[0];

        let mut last = f64::INFINITY;
        for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let k = with_rod_inserted(&lib, 1, f).k_inf();
            assert!(
                k < last,
                "k must fall monotonically with insertion; f={f} gave {k} after {last}"
            );
            last = k;
        }

        let inserted = with_rod_inserted(&lib, 1, 1.0);
        let gap_after =
            inserted.zones[1].inferred_absorption(0).unwrap() - inserted.zones[1].absorption[0];
        assert!(
            (gap_after - gap_before).abs() < 1.0e-15,
            "the rod must keep Sigma_t = Sigma_a + Sigma_s,row intact, else the \
             bridge never sees it: gap {gap_before:e} -> {gap_after:e}"
        );

        // Withdrawn must be a genuine no-op.
        assert_eq!(with_rod_inserted(&lib, 1, 0.0).k_inf(), lib.k_inf());
        // A bad zone index must not panic mid-sweep.
        assert_eq!(with_rod_inserted(&lib, 99, 1.0).k_inf(), lib.k_inf());
    }
}
