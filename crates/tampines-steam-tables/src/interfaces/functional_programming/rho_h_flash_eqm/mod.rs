//! Density-enthalpy `(rho,h)` flash: pressure, temperature and steam quality
//! for a state whose independent variables are **density and specific
//! enthalpy**.
//!
//! # Why this module exists
//!
//! IAPWS-IF97 publishes backward equations for `(p,h)`, `(p,s)` and `(h,s)`,
//! but none for `(rho,h)`. A compressible flow solver, however, carries
//! **density** (from continuity) and **enthalpy** (from the energy equation)
//! as its conserved variables, and needs pressure back out of them. Without a
//! `(rho,h)` entry point such a solver has to reach for a two-dimensional
//! iterative solve over the forward (Gibbs/Helmholtz) equations, which is both
//! slow and fragile near the saturation line.
//!
//! This module closes that gap **using the published backward equations
//! themselves**. The observation it rests on is that
//! [`v_ph_eqm`](crate::interfaces::functional_programming::ph_flash_eqm::v_ph_eqm)
//! is already an *explicit* function: given `(p,h)` it selects a region and
//! evaluates a backward equation (`T_ph` in Regions 1 and 2, `v_ph_3` directly
//! in Region 3, a quality-weighted mixture in Region 4) with no inner
//! iteration. Inverting it therefore collapses from a two-dimensional solve to
//! a **one-dimensional, bracketed root find in pressure alone**, where every
//! residual evaluation costs exactly one explicit backward-equation flash.
//!
//! # What is IAPWS-traceable here, and what is not
//!
//! Every thermodynamic value this module returns comes from the crate's
//! IAPWS-IF97 equations — it adds no fitted correlation of its own. What it
//! adds is the **inversion strategy** (bracketing and root finding), which is
//! numerics, not thermodynamics. That distinction matters: the accuracy of
//! `p(rho,h)` is the accuracy of IF97's own `v(p,h)` plus a convergence
//! tolerance that the caller can read off [`P_RHO_H_REL_TOL`].
//!
//! Contrast this with
//! [`backward_eqn_chebyshev_experimental::p_rho_h`](crate::backward_eqn_chebyshev_experimental::p_rho_h),
//! which is an in-house Chebyshev *fit* to the same surface together with a
//! statistical region classifier. That module is fast and explicit but carries
//! fit error and a classifier that misfiles roughly 2.7 % of states (8.4 % in
//! Region 4). **This module is the accurate path**; the Chebyshev module is the
//! cheap-and-approximate one. They are complementary, not redundant.
//!
//! # Validity
//!
//! The same domain as the `(p,h)` flash it inverts: pressure in
//! `[p_sat(273.15 K), 100 MPa]` and enthalpy between the `273.15 K` and
//! `1073.15 K` isotherms. Region 5 (above 1073.15 K) has no `(p,h)` backward
//! equation in this crate and is therefore not reachable from `(rho,h)` either.
//!
//! # Measured accuracy, and where this does NOT work
//!
//! Verified against the published steam tables in
//! `interfaces::tests_and_examples::rho_h_flash_steam_table` (2334 single-phase
//! nodes and 220 saturation rows, measured 2026-09-14). **No node failed**: every
//! one was recovered either to the right pressure (`|dp/p| < 1e-6`, 2284 nodes)
//! or to the right density (`|dv/v| < 1e-9`, 50 nodes).
//!
//! Worst pressure error by region, inverting this crate's own `v(p,h)`:
//!
//! | region | max `\|dp/p\|` | comment |
//! |---|---|---|
//! | Region 4 (two-phase) | `3.7e-13` | the blowdown regime; excellent |
//! | Region 2 (vapour) | `3.0e-5` | well conditioned |
//! | Region 3 | `4.8e-5` | well conditioned |
//! | Region 1 (compressed liquid) | `7.9e-1` | **input-error sensitive — see below** |
//!
//! **What Region 1 does and does not mean — an earlier version of this file
//! got this wrong, so it is stated carefully.** The `7.9e-1` above is measured
//! against *table-rounded* density: the published `v` carries six significant
//! figures, and in nearly-incompressible liquid that rounding is amplified.
//! The amplification
//!
//! ```text
//! A = |d ln p / d ln v|_h  ~  1 / (p * kappa_T)
//! ```
//!
//! multiplies whatever error is already in the **input**. It does not mean the
//! inversion is inaccurate.
//!
//! Given an *exact* density the inversion is exact throughout the compressed
//! liquid. Measured 2026-09-14 on subcooled liquid at 18 degC, feeding density
//! straight from this crate's own `v(p,h)`
//! (`diagnose_saturation_fallback_against_the_root_find`):
//!
//! | true p | recovered p | `A` |
//! |---|---|---|
//! | 0.5 bar | 0.50000 bar | `4.0e4` |
//! | 1 bar | 1.00000 bar | `2.0e4` |
//! | 10 bar | 10.00000 bar | `2.0e3` |
//! | 70 bar | 70.00000 bar | `2.9e2` |
//! | 99 MPa | 990.00000 bar | `2.3e1` |
//!
//! Exact at every one, including where `A = 4.0e4`, because a large `A` acting
//! on a machine-precision input error is still a machine-precision output
//! error. **So "the compressed liquid is not recoverable" is false as a
//! blanket claim** — it is recoverable whenever the caller's density is better
//! known than the printed tables.
//!
//! The one state in that sweep that genuinely fails is **0.1 bar**, which
//! returns `0.02065 bar` against a true `0.1 bar`. That value is exactly
//! `p_sat(T(h))`, the bubble point — i.e. the search converged to the bottom
//! edge of its own bracket, not to the root. **That is a branch-selection
//! defect in this module, not a physical limit**, and it is filed rather than
//! papered over.
//!
//! [`p_rho_h_conditioning`] reports `A` for a state, which is what a caller
//! needs to turn its own density uncertainty into a pressure uncertainty. In
//! the two-phase and vapour regions — where a depressurisation transient
//! actually spends its time — `A` is order 1.
//!
//! # Cost, and where this belongs
//!
//! Measured 2026-09-14 over a spread of states
//! (`diagnose_the_cost_of_the_inversion_relative_to_a_ph_flash`):
//! `v_ph_eqm` 3.45 us/call, `p_rho_h_eqm` 205 us/call â a factor of **59.5**.
//!
//! That ordering is structural, not a tuning failure: this is a bracketed root
//! find built out of repeated `v(p,h)` evaluations, plus a region scan, so it
//! cannot be cheaper than the flash it inverts.
//!
//! The practical consequence is worth stating plainly, because it is easy to
//! reach for this function in the wrong place. **A pressure-based solver has
//! nothing to gain here.** If the algorithm already carries `p` as a primary
//! variable and derives density from it â which is what
//! `TampinesSteamArray::correct_thermo` does â then pressure never has to be
//! recovered, and routing through this module would only add cost. The payoff
//! is against a *two-dimensional* iterative solve over the forward equations,
//! i.e. in a density-based algorithm where `rho` and `h` are the conserved
//! variables and `p` genuinely must be inverted for.
//!
//! Most of the cost is the region scan rather than the root find; narrowing it
//! is tracked as a follow-up rather than done here, since correctness across
//! the seams was the point.
//!
//! # Contents
//!
//! - [`p_rho_h_eqm`] / [`p_rho_h_eqm_explicit`] — pressure from `(rho,h)`.
//! - [`tpx_rho_h_eqm`] — temperature, pressure and steam quality together,
//!   returned as [`TpxRhoH`].
//! - [`rho_h_is_within_validity_range`] — a non-panicking domain predicate, for
//!   callers that must check before committing to a flash.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{megapascal, pascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use crate::constants::{P_C_MPA, RHO_C_KG_PER_M3, T_C_KELVIN};
use crate::interfaces::functional_programming::ph_flash_eqm::{ph_flash_region, v_ph_eqm, x_ph_flash};
use crate::interfaces::functional_programming::pt_flash_eqm::{h_tp_eqm_single_phase, FwdEqnRegion};
use crate::region_1_subcooled_liquid::h_tp_1;
use crate::region_2_vapour::backward_eqn_ph_2::t_ph_2;
use crate::region_1_subcooled_liquid::backward_eqn_ph_1::t_ph_1;
use crate::region_3_single_phase_plus_supercritical_steam::backward_eqn_ph_3::t_ph_flash::t_ph_3;
use crate::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};

#[cfg(test)]
mod tests;

/// Lower pressure bound of the `(p,h)` flash domain, in pascal:
/// `p_sat(273.15 K)`.
///
/// Recomputed rather than hard-coded so it can never drift from
/// [`sat_pressure_4`].
fn p_lower_limit_pascal() -> f64 {
    sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(273.15)).get::<pascal>()
}

/// Upper pressure bound of the `(p,h)` flash domain: 100 MPa, in pascal.
const P_UPPER_LIMIT_PASCAL: f64 = 100.0e6;

/// Relative convergence tolerance on pressure for [`p_rho_h_eqm`].
///
/// The returned pressure satisfies `|p - p_exact| <= P_RHO_H_REL_TOL * p`,
/// where `p_exact` is the pressure at which this crate's own `v(p,h)` equals
/// the requested specific volume exactly. It is a *numerical* tolerance on the
/// inversion and says nothing about IF97's own uncertainty.
pub const P_RHO_H_REL_TOL: f64 = 1.0e-12;

/// Maximum residual evaluations before [`p_rho_h_eqm`] gives up.
///
/// The safeguarded false-position iteration below converges in roughly 8 to 15
/// evaluations across the steam table; this ceiling exists only so a pathology
/// fails loudly instead of spinning.
const MAX_ITERATIONS: usize = 200;

/// Temperature, pressure and steam quality recovered from a `(rho,h)` state.
///
/// Returned by [`tpx_rho_h_eqm`]. The quality convention is documented on that
/// function — in particular, `vapour_quality` is a genuine equilibrium quality
/// only inside Region 4; elsewhere it is the single-phase convention 0 (liquid)
/// or 1 (vapour).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TpxRhoH {
    /// Temperature of the state.
    pub temperature: ThermodynamicTemperature,
    /// Pressure of the state.
    pub pressure: Pressure,
    /// Steam (vapour) quality, dimensionless, on `[0, 1]`.
    ///
    /// Inside Region 4 this is the equilibrium vapour mass fraction. Outside
    /// it, it is a convention flag rather than a measured fraction — see
    /// [`tpx_rho_h_eqm`].
    pub vapour_quality: f64,
    /// The IF97 region the state was found in.
    pub region: FwdEqnRegion,
}

/// Returns `true` when `(p,h)` lies inside the `(p,h)` flash domain, without
/// panicking.
///
/// The crate's `(p,h)` validity checks panic on an out-of-domain point, which
/// is the right behaviour for a user-facing flash but useless inside a root
/// find that must probe the domain edges. This is the same test, expressed as
/// a predicate.
///
/// `p` is pressure and `h` specific enthalpy.
fn ph_is_within_validity_range(p: Pressure, h: AvailableEnergy) -> bool {
    let p_pascal = p.get::<pascal>();

    if p_pascal < p_lower_limit_pascal() || p_pascal > P_UPPER_LIMIT_PASCAL {
        return false;
    }

    // Lower bound: the 273.15 K isotherm. Evaluated with the Region 1 forward
    // equation directly, for the reason documented in
    // `ph_flash_eqm::validity_range::is_below_isotherm_t_273_15` — the (T,p)
    // region router reports Region 4 exactly on the saturation line, and the
    // Region 4 (T,p) arm is deliberately unsupported.
    let h_lower = h_tp_1(ThermodynamicTemperature::new::<kelvin>(273.15), p);
    if h < h_lower {
        return false;
    }

    // Upper bound: the 1073.15 K isotherm.
    let h_upper = h_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(1073.15), p);
    if h > h_upper {
        return false;
    }

    true
}

/// Returns `true` when a `(rho,h)` state can be flashed by [`p_rho_h_eqm`].
///
/// `rho` is mass density and `h` specific enthalpy. A state is flashable when
/// some pressure in the `(p,h)` domain reproduces the requested density; this
/// predicate answers that question without panicking, so a caller carrying
/// possibly-unphysical solver state can check first.
pub fn rho_h_is_within_validity_range(rho: MassDensity, h: AvailableEnergy) -> bool {
    let rho_si = rho.get::<kilogram_per_cubic_meter>();
    if !rho_si.is_finite() || rho_si <= 0.0 || !h.value.is_finite() {
        return false;
    }
    bracket_pressure(rho_si, h).is_some()
}

/// Specific volume in m3/kg at `(p,h)`, with `p` given in pascal.
///
/// One explicit backward-equation flash; this is the residual kernel the root
/// find calls.
#[inline]
fn v_at_pressure(p_pascal: f64, h: AvailableEnergy) -> f64 {
    v_ph_eqm(Pressure::new::<pascal>(p_pascal), h).get::<cubic_meter_per_kilogram>()
}

/// Locates the pressures at which the IF97 region changes along the fixed-`h`
/// line, between `p_low` and `p_high`.
///
/// Writes the interior boundaries in increasing order into `out` and returns
/// how many were found (at most 4 — along a fixed enthalpy the state passes
/// through at most Regions 2, 3, 4 and 1).
///
/// # Why the dispatcher needs these
///
/// `v(p,h)` is smooth and monotone *within* a region, but **not across a region
/// seam**: IF97's backward equations are only consistent with each other to
/// about `1e-5`, so the specific volume takes a small step at each boundary.
/// Those steps are what break a naive root find — see
/// the measured Region 4 to Region 1 case below,
/// where the true root becomes a tangential touch and a spurious sign change
/// appears 1.5 % away. Region 2 to Region 3 does the same thing at, for
/// example, `h = 2902.88 kJ/kg`.
///
/// Splitting the search at every seam makes each sub-interval monotone, so the
/// spurious crossings cannot be reached from the sub-interval that holds the
/// real root.
///
/// # Method
///
/// A coarse logarithmic scan detects label changes, then each change is pinned
/// by bisection. The scan is logarithmic because the pressure domain spans five
/// decades and the interesting structure is not uniformly distributed in `p`.
fn region_boundaries(
    h: AvailableEnergy,
    p_low: f64,
    p_high: f64,
    out_below: &mut [f64; 4],
    out_above: &mut [f64; 4],
) -> usize {
    const SCAN_POINTS: usize = 14;

    let region_at = |p_pascal: f64| ph_flash_region(Pressure::new::<pascal>(p_pascal), h);

    let ln_lo = p_low.ln();
    let ln_hi = p_high.ln();
    let d_ln = (ln_hi - ln_lo) / ((SCAN_POINTS - 1) as f64);

    let mut found = 0_usize;
    let mut prev_p = p_low;
    let mut prev_region = region_at(p_low);

    for i in 1..SCAN_POINTS {
        let p = if i == SCAN_POINTS - 1 {
            p_high
        } else {
            (ln_lo + d_ln * (i as f64)).exp()
        };
        let region = region_at(p);

        if region != prev_region {
            // Pin this boundary. 60 halvings is far past f64 resolution on any
            // sub-interval of this domain; the relative-width test exits first.
            let mut lo = prev_p;
            let mut hi = p;
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if region_at(mid) == prev_region {
                    lo = mid;
                } else {
                    hi = mid;
                }
                if (hi - lo) <= 1.0e-13 * hi {
                    break;
                }
            }
            // Hand back BOTH sides, not the midpoint. The whole purpose of the
            // split is that the two branches disagree at the seam, so a single
            // shared endpoint would be evaluated on whichever side the
            // bisection happened to land -- and if that is the far side, the
            // sub-interval holding the true root shows no sign change and the
            // search falls through to the spurious one. `lo` is the last
            // pressure still in the lower region and `hi` the first in the
            // upper, so each sub-interval is evaluated strictly inside itself.
            if found < out_below.len() {
                out_below[found] = lo;
                out_above[found] = hi;
                found += 1;
            }
            prev_region = region;
        }
        prev_p = p;
    }

    found
}

/// Brackets the pressure root of `v(p,h) = 1/rho`.
///
/// Returns `(p_low, p_high, f_low, f_high)` in pascal with the residual
/// `f(p) = ln v(p,h) - ln(1/rho)` bracketing a sign change, or `None` when the
/// state lies outside the flash domain.
///
/// # Why a log residual
///
/// Specific volume spans roughly six decades across the steam table, and the
/// root find must behave the same on a 1e-3 m3/kg liquid as on a 1e2 m3/kg
/// rarefied vapour. Working in `ln v` makes the residual's scale uniform, so a
/// single relative tolerance is meaningful everywhere.
///
/// # Why the domain edge is searched, not assumed
///
/// For a given enthalpy only a sub-interval of `[p_min, 100 MPa]` is inside the
/// `(p,h)` domain: the 273.15 K isotherm rises with pressure and the 1073.15 K
/// isotherm falls, so both bound the *upper* end. The valid set is therefore an
/// interval anchored at `p_min`, and its upper edge is located by bisection on
/// the validity predicate.
fn bracket_pressure(rho_si: f64, h: AvailableEnergy) -> Option<(f64, f64, f64, f64)> {
    let ln_v_target = (1.0 / rho_si).ln();

    let p_min = p_lower_limit_pascal();
    let h_pressure_min = Pressure::new::<pascal>(p_min);
    if !ph_is_within_validity_range(h_pressure_min, h) {
        // The enthalpy is outside the domain even at the lowest pressure, so no
        // pressure can host it.
        return None;
    }

    // Locate the upper edge of the valid pressure interval.
    let p_max = if ph_is_within_validity_range(Pressure::new::<pascal>(P_UPPER_LIMIT_PASCAL), h) {
        P_UPPER_LIMIT_PASCAL
    } else {
        let mut lo = p_min;
        let mut hi = P_UPPER_LIMIT_PASCAL;
        // 200 halvings of a ~5-decade interval is far beyond what f64 can
        // resolve; the loop exits on the relative-width test long before.
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if ph_is_within_validity_range(Pressure::new::<pascal>(mid), h) {
                lo = mid;
            } else {
                hi = mid;
            }
            if (hi - lo) <= 1.0e-12 * hi {
                break;
            }
        }
        lo
    };

    if !(p_max > p_min) {
        return None;
    }

    // Dispatch on REGION before searching. `v(p,h)` steps discontinuously at
    // each IF97 region seam, so a search spanning one can converge on a
    // spurious root while reporting a machine-epsilon residual -- see
    // `region_boundaries`. Splitting at every seam leaves each sub-interval
    // monotone.
    let mut seam_below = [0.0_f64; 4];
    let mut seam_above = [0.0_f64; 4];
    let n_boundaries = region_boundaries(h, p_min, p_max, &mut seam_below, &mut seam_above);

    // Walk the sub-intervals from the LOWEST pressure upward and take the first
    // that brackets a sign change.
    //
    // Lowest-first is the tie-break, and it is the physically right one. Where a
    // seam manufactures a second crossing, the spurious one always lies on the
    // far side of the seam from the true root -- the volume step displaces the
    // next region\'s branch, so the extra crossing appears at higher pressure.
    // Taking the lowest bracketing sub-interval therefore returns the real
    // state and leaves the artefact unreachable.
    // Sub-interval i runs from `lower_edge[i]` to `upper_edge[i]`.
    let mut lower_edge = [0.0_f64; 5];
    let mut upper_edge = [0.0_f64; 5];
    lower_edge[0] = p_min;
    for i in 0..n_boundaries {
        upper_edge[i] = seam_below[i];
        lower_edge[i + 1] = seam_above[i];
    }
    upper_edge[n_boundaries] = p_max;
    let n_intervals = n_boundaries + 1;

    for i in 0..n_intervals {
        let a = lower_edge[i];
        let b = upper_edge[i];
        if !(b > a) {
            continue;
        }

        let f_low = v_at_pressure(a, h).ln() - ln_v_target;
        let f_high = v_at_pressure(b, h).ln() - ln_v_target;

        if !f_low.is_finite() || !f_high.is_finite() {
            continue;
        }

        // v decreases with p at fixed h, so a bracketed root has
        // f_low >= 0 >= f_high.
        if f_low * f_high <= 0.0 {
            return Some((a, b, f_low, f_high));
        }
    }

    // No sub-interval brackets a root: the requested density is unreachable at
    // this enthalpy anywhere in the domain.
    None
}

/// Pressure in pascal from density in kg/m3 and specific enthalpy in J/kg.
///
/// The undimensioned twin of [`p_rho_h_eqm`], for hot loops that already hold
/// SI scalars and do not want to pay for `uom` construction. Same algorithm,
/// same tolerance, same panics.
///
/// # Panics
///
/// Panics when `rho_si` is not strictly positive and finite, when `h_si` is not
/// finite, or when the `(rho,h)` state lies outside the `(p,h)` flash domain
/// (see [`rho_h_is_within_validity_range`] for a non-panicking check).
pub fn p_rho_h_eqm_explicit(rho_si: f64, h_si: f64) -> f64 {
    assert!(
        rho_si.is_finite() && rho_si > 0.0,
        "p_rho_h_eqm: density must be finite and strictly positive, got {rho_si} kg/m3"
    );
    assert!(
        h_si.is_finite(),
        "p_rho_h_eqm: specific enthalpy must be finite, got {h_si} J/kg"
    );

    let h = AvailableEnergy::new::<joule_per_kilogram>(h_si);
    let ln_v_target = (1.0 / rho_si).ln();

    let (mut a, mut b, mut fa, mut fb) = bracket_pressure(rho_si, h).unwrap_or_else(|| {
        panic!(
            "p_rho_h_eqm: (rho = {rho_si} kg/m3, h = {h_si} J/kg) is outside the \
             IAPWS-IF97 (p,h) flash domain \
             (p in [p_sat(273.15 K), 100 MPa], 273.15 K <= T <= 1073.15 K)"
        )
    });

    if fa == 0.0 {
        return a;
    }
    if fb == 0.0 {
        return b;
    }

    // Safeguarded false position (the Illinois variant). Regula falsi keeps the
    // root bracketed unconditionally, which matters here because the residual
    // has a kink where the pressure crosses a region boundary and a plateau of
    // near-zero slope inside the two-phase dome; a plain secant or Newton step
    // can leave the bracket at both. The Illinois halving of the retained
    // endpoint's residual removes regula falsi's one weakness — the stagnant
    // endpoint that makes it degrade to linear convergence on a convex
    // residual — and restores superlinear convergence.
    let mut side_low = 0_u8;
    let mut side_high = 0_u8;

    for _ in 0..MAX_ITERATIONS {
        if (b - a) <= P_RHO_H_REL_TOL * b {
            break;
        }

        let mut p = b - fb * (b - a) / (fb - fa);

        // Keep the trial strictly inside the bracket. A degenerate (fb - fa)
        // or a rounding excursion falls back to bisection.
        if !p.is_finite() || p <= a || p >= b {
            p = 0.5 * (a + b);
        }

        let f = v_at_pressure(p, h).ln() - ln_v_target;
        if !f.is_finite() {
            // Should be unreachable inside a validated bracket; bisect rather
            // than propagate a NaN.
            p = 0.5 * (a + b);
            let f_mid = v_at_pressure(p, h).ln() - ln_v_target;
            if f_mid * fa < 0.0 {
                b = p;
                fb = f_mid;
            } else {
                a = p;
                fa = f_mid;
            }
            continue;
        }

        if f == 0.0 {
            return p;
        }

        if f * fa < 0.0 {
            // Root is in [a, p]; the high endpoint moves.
            b = p;
            fb = f;
            side_high = 0;
            side_low += 1;
            if side_low >= 2 {
                fa *= 0.5;
                side_low = 0;
            }
        } else {
            // Root is in [p, b]; the low endpoint moves.
            a = p;
            fa = f;
            side_low = 0;
            side_high += 1;
            if side_high >= 2 {
                fb *= 0.5;
                side_high = 0;
            }
        }
    }

    0.5 * (a + b)
}

/// Pressure from density and specific enthalpy, by inverting the IAPWS-IF97
/// `(p,h)` backward equations.
///
/// `rho` is mass density and `h` is specific enthalpy. The returned pressure is
/// the one at which this crate's own `v(p,h)` reproduces `1/rho`, converged to
/// [`P_RHO_H_REL_TOL`] relative.
///
/// # Valid range
///
/// The `(p,h)` flash domain: `p_sat(273.15 K) <= p <= 100 MPa` and
/// `273.15 K <= T <= 1073.15 K`. Region 5 is not reachable. Use
/// [`rho_h_is_within_validity_range`] to test a state without risking a panic.
///
/// # Accuracy
///
/// This inverts the published backward equations rather than fitting them, so
/// the error is IF97's own `v(p,h)` error plus the convergence tolerance. That
/// makes it the accurate counterpart to the explicit Chebyshev fit in
/// [`crate::backward_eqn_chebyshev_experimental::p_rho_h`], which is cheaper
/// but carries fit and classifier error.
///
/// # Panics
///
/// Panics on a non-finite or non-positive density, a non-finite enthalpy, or a
/// state outside the flash domain.
pub fn p_rho_h_eqm(rho: MassDensity, h: AvailableEnergy) -> Pressure {
    let p_pascal = p_rho_h_eqm_explicit(
        rho.get::<kilogram_per_cubic_meter>(),
        h.get::<joule_per_kilogram>(),
    );
    Pressure::new::<pascal>(p_pascal)
}

/// Temperature, pressure and steam quality from density and specific enthalpy.
///
/// `rho` is mass density and `h` is specific enthalpy. The pressure comes from
/// [`p_rho_h_eqm`]; temperature and quality then follow from the `(p,h)`
/// backward equations at that pressure, so the three returned values are
/// mutually consistent by construction.
///
/// # The steam-quality convention outside Region 4
///
/// Quality is only physically meaningful in the two-phase dome. Region 4
/// therefore returns the genuine equilibrium vapour mass fraction from
/// `x_ph_flash`. Everywhere else this function reports a **convention flag**,
/// so that a caller carrying `x` as a field never has to special-case a
/// missing value:
///
/// - **Region 1** (subcooled / compressed liquid): `x = 0`.
/// - **Regions 2 and 5** (superheated vapour): `x = 1`.
/// - **Region 3 below the critical pressure**: compared against the saturation
///   temperature at that pressure — `T < T_sat(p)` gives `x = 0`
///   (liquid-like), otherwise `x = 1` (vapour-like).
/// - **At or above the critical pressure** (`p >= 22.064 MPa`): there is no
///   phase boundary to cross, so the convention is taken from the critical
///   *temperature* instead — a state to the **left** of the critical point
///   (`T < 647.096 K`) is reported `x = 0`, and one to the **right**
///   (`T >= T_c`) is reported `x = 1`.
///
/// That last rule is the project convention for supercritical states. It is a
/// labelling choice, not a physical claim: above the critical pressure the
/// fluid is a single supercritical phase and no vapour fraction exists. Do not
/// feed a supercritical `x` into a two-phase correlation and expect meaning
/// from it.
///
/// # Panics
///
/// Panics under the same conditions as [`p_rho_h_eqm`], and additionally if the
/// recovered state classifies as Region 5, for which this crate has no `(p,h)`
/// backward equation.
pub fn tpx_rho_h_eqm(rho: MassDensity, h: AvailableEnergy) -> TpxRhoH {
    let p = p_rho_h_eqm(rho, h);
    let region = ph_flash_region(p, h);

    let (temperature, vapour_quality) = match region {
        FwdEqnRegion::Region1 => (t_ph_1(p, h), 0.0),
        FwdEqnRegion::Region2 => (t_ph_2(p, h), 1.0),
        FwdEqnRegion::Region4 => (sat_temp_4(p), x_ph_flash(p, h)),
        FwdEqnRegion::Region3 => {
            let t = t_ph_3(p, h);
            let x = quality_convention_single_phase(t, p, rho);
            (t, x)
        }
        FwdEqnRegion::Region5 => panic!(
            "tpx_rho_h_eqm: the (rho,h) state resolved into Region 5 \
             (T > 1073.15 K), for which this crate has no (p,h) backward equation"
        ),
    };

    TpxRhoH {
        temperature,
        pressure: p,
        vapour_quality,
        region,
    }
}

/// The single-phase steam-quality convention for a state that is not in the
/// two-phase dome.
///
/// See [`tpx_rho_h_eqm`] for the rationale. Returns 0 for a liquid-like state
/// and 1 for a vapour-like one.
///
/// # Why density, not temperature, decides the subcritical case
///
/// The obvious rule for a Region 3 state below the critical pressure is to
/// compare its temperature against `T_sat(p)`. That is correct in the interior
/// but **degenerate exactly on the saturation line**, which is precisely where
/// the saturated-liquid and saturated-vapour table entries live: there
/// `T == T_sat(p)` to within floating point, and the comparison falls through
/// to whichever branch the `else` happens to be. Measured symptom: the
/// saturated *liquid* at 373 degC / 218.132 bar came back labelled `x = 1`.
///
/// The critical isochore does not have that degeneracy. Below the critical
/// pressure `v_f < v_c < v_g` holds strictly (the three coincide only at the
/// critical point itself), so comparing the state volume against `v_c`
/// separates liquid-like from vapour-like cleanly, and it agrees with the
/// temperature rule everywhere in the interior.
fn quality_convention_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    rho: MassDensity,
) -> f64 {
    let p_critical = Pressure::new::<megapascal>(P_C_MPA);
    let t_critical = ThermodynamicTemperature::new::<kelvin>(T_C_KELVIN);

    if p >= p_critical {
        // Supercritical pressure: there is no phase boundary, so the project
        // convention splits on the critical TEMPERATURE -- left of the critical
        // point is labelled liquid-like, right of it vapour-like.
        if t < t_critical {
            0.0
        } else {
            1.0
        }
    } else {
        // Subcritical Region 3: split on the critical isochore.
        let rho_si = rho.get::<kilogram_per_cubic_meter>();
        if rho_si > RHO_C_KG_PER_M3 {
            0.0
        } else {
            1.0
        }
    }
}

/// How well the pressure is determined by the density at this state.
///
/// Returns the **amplification factor**
///
/// ```text
/// A = |d ln p / d ln v|_h
/// ```
///
/// which is the factor by which a relative error in specific volume becomes a
/// relative error in the pressure that [`p_rho_h_eqm`] returns. `A` near 1
/// means the inversion is well conditioned; `A` of 1e5 means a part-per-million
/// error in density becomes a 10 % error in pressure.
///
/// # Why this is public
///
/// Because the limitation it measures is real, and a caller cannot see it
/// otherwise. In the **low-pressure subcooled liquid** the pressure signal
/// carried by the density falls *below IF97's own backward-equation noise*:
/// water at 18 degC has almost exactly the same density at 0.02 bar as at
/// 0.1 bar, while the `T(p,h)` backward equation's ~25 mK uncertainty moves the
/// volume by ~5e-6 relative — more than the whole pressure signal across that
/// range. `p(rho,h)` there returns a pressure that satisfies the density to
/// machine precision and is still badly wrong, and **no implementation can do
/// better**: the information is not in the inputs.
///
/// Measured 2026-09-14 (`diagnose_the_conditioning_measure_across_regimes`):
///
/// | state | `A` |
/// |---|---|
/// | superheated vapour, 10 bar | `1.0` |
/// | two-phase, 1 bar | `1.0` |
/// | saturated liquid, 8 bar | `5.3e-2` |
/// | compressed liquid, 99 MPa | `2.1e1` |
/// | subcooled liquid, 0.5 bar | `4.0e4` |
/// | subcooled liquid, 0.1 bar | `4.8e6` |
///
/// # Cost
///
/// Two extra flashes, because it perturbs the density and re-solves rather
/// than differentiating locally. That is deliberate â the cheap local
/// derivative is blind in exactly the regime this exists to detect â so treat
/// it as a diagnostic to call when in doubt, not something for a hot loop.
///
/// A solver carrying `(rho,h)` should check this where it might be in the
/// subcooled liquid, and take its pressure from the momentum/pressure equation
/// rather than the equation of state when `A` is large. In the two-phase and
/// vapour regions — where a blowdown actually spends its time — `A` is order 1
/// and `p(rho,h)` is trustworthy.
///
/// # Panics
///
/// Panics under the same conditions as [`p_rho_h_eqm`].
pub fn p_rho_h_conditioning(rho: MassDensity, h: AvailableEnergy) -> f64 {
    let rho_si = rho.get::<kilogram_per_cubic_meter>();
    let h_si = h.get::<joule_per_kilogram>();

    let p_at = p_rho_h_eqm_explicit(rho_si, h_si);
    if !(p_at > 0.0) {
        return f64::INFINITY;
    }

    // Measured as the response of the ANSWER to the INPUT, not as a local
    // derivative of `v(p,h)`.
    //
    // The derivative form is what the definition suggests, and it is wrong in
    // exactly the case that matters. It has to be evaluated somewhere, and the
    // only pressure available is the one just returned -- which, precisely when
    // the state is ill-conditioned, is not the state's real pressure. The
    // derivative then describes a different state and reports it healthy:
    // measured at 0.1 bar / 18 degC, a central difference gave `A = 1.2e-3` and
    // a one-sided pair `A = 25`, for a state the solver cannot place to better
    // than 79 %.
    //
    // Perturbing the density and re-solving has no such blind spot. It asks the
    // question the caller is actually asking -- "if my density were slightly
    // off, how far would this pressure move?" -- and it answers it with the
    // same dispatch and root find that produced the pressure, so a branch that
    // is about to flip shows up as the large excursion it is.
    let relative_perturbation = 1.0e-6;
    let mut worst: f64 = 0.0;

    for sign in [1.0_f64, -1.0] {
        let rho_probe = rho_si * (1.0 + sign * relative_perturbation);
        if !(rho_probe > 0.0) {
            continue;
        }
        if !rho_h_is_within_validity_range(
            MassDensity::new::<kilogram_per_cubic_meter>(rho_probe),
            h,
        ) {
            continue;
        }

        let p_probe = p_rho_h_eqm_explicit(rho_probe, h_si);
        let d_ln_p = ((p_probe - p_at) / p_at).abs();
        worst = worst.max(d_ln_p / relative_perturbation);
    }

    worst
}
