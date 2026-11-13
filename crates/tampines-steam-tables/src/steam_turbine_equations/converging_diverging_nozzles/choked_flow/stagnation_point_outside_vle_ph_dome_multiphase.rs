//! Critical-flow solvers for stagnation states that lie OUTSIDE the p-h VLE
//! dome (single phase at the inlet).
//!
//! Two single-phase buckets sit either side of the dome and are handled by
//! mirror-image solvers here:
//!
//! * **Subcooled-liquid / liquid-like** (left of the dome, `s0 < s_crit`). On
//!   isentropic depressurisation such a state stays single-phase liquid down to
//!   the **bubble point**, then flashes into the dome. See
//!   [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`].
//! * **Superheated-vapour / supercritical** (right of / above the dome,
//!   `s0 > s_crit`). On isentropic depressurisation such a state stays
//!   single-phase vapour down to the **dew point**, then condensation begins as
//!   it enters the dome. See
//!   [`get_critical_pressure_and_mass_flux_superheated_vapour_ph`].
//!
//! Both solvers use the same smooth energy-balance max-G HEM criterion as the
//! in-dome solver — `G(p) = rho(p,s0) * sqrt(2*(h0 - h(p,s0)))` maximised along
//! the isentrope — so no sound speed (and no finite-difference
//! `mass_flux_ps_eqm_throat`) is ever evaluated.

use petir::uom_math::QuantityMath;
use petir::mathf::RealMath;


use uom::ConstZero;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::megapascal;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::interfaces::functional_programming::pt_flash_eqm::s_tp_eqm_two_phase;
use crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_two_phase;
use crate::interfaces::functional_programming::pt_flash_eqm::v_tp_eqm_two_phase;

/// Threshold on `v_b/c_2φ` (Bernoulli velocity at the bubble point over the
/// two-phase sound speed) above which a subcooled stagnation state is treated as
/// unambiguously deeply subcooled and the choke is taken as the energy-balance /
/// Bernoulli maximum. ~~The solver branches on it~~ **CORRECTED 2026-11-02
/// (gh:#1111): the solver no longer uses it.** The branch is now the
/// global-maximum rule in
/// [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`]. The Moody
/// tests still use this constant to choose which points to assert (their
/// `[DEEP]` bucket), so it stays as a test classification only. Set above the maximum reached by any Zaloudek subcooled
/// reference point (3.30) so this only adds deep-subcooling behaviour and never
/// perturbs the validated near-bubble (sonic) regime. See README v0.2.1.
///
/// Public so the Moody deep-subcooling verification test can classify points
/// ~~with the exact same threshold the solver uses~~ (since 2026-11-02 the
/// solver does not use it; it is the test's bucket definition only).
pub const DEEP_SUBCOOLING_RATIO: f64 = 5.0;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use crate::region_4_vap_liq_equilibrium::sat_temp_4;
use super::basic_multiphase_equations::bubble_point_pressure_from_entropy;
use super::basic_multiphase_equations::dew_point_pressure_from_entropy;

/// Golden-section maximisation of the HEM energy-balance mass flux `g_of_p`
/// over the pressure bracket `[a_pa, b_pa]` (both in Pa, `a_pa <= b_pa`).
///
/// **This is the crate's only golden-section search.** Every HEM choked-flow
/// path that needs one calls this function: the in-dome solver
/// [`super::get_critical_pressure_and_mass_flux_ph_vle_dome`], the subcooled
/// solver [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`], the
/// superheated-vapour solver
/// [`get_critical_pressure_and_mass_flux_superheated_vapour_ph`], and the
/// near-critical `dome_crossing_interior_choke` refinement in the parent module.
/// Do not write a fourth copy — bead `op-uyi3` records that three inline
/// rewrites of this loop had already accumulated in this crate and had begun to
/// drift from each other in incidental detail.
///
/// # Method
///
/// `g_of_p` is unimodal on each single-phase / two-phase stretch of the
/// isentrope (zero at the high-pressure end, a single interior peak at the
/// choke, then falling toward `p_min`), so this is robust and needs no
/// derivative of the noisy sound speed. On a monotone stretch it converges to
/// the appropriate endpoint, which is the intended behaviour when the real peak
/// lies in the neighbouring stretch.
///
/// **Unimodality is a precondition the search cannot check.** On a bracket with
/// two basins it returns one of them, silently. The `dome_crossing_interior_choke`
/// caller is the example to copy: it coarse-scans 1500 points first and only
/// hands this function a bracket already known to contain the right peak.
///
/// # One objective evaluation per iteration
///
/// The contraction constant `gr = (sqrt(5) - 1)/2` is chosen precisely so that
/// one interior probe of the contracted bracket **coincides** with a probe of
/// the previous bracket. That probe is therefore carried across the iteration
/// rather than recomputed, so each iteration after the first costs a **single**
/// call to `g_of_p`. This matters here because `g_of_p` is an IAPWS-IF97 `(p,s)`
/// flash, not an arithmetic expression.
///
/// A search that performs `k` iterations calls `g_of_p` exactly `k + 3` times:
/// two initial probes, one per iteration, and one final evaluation at the
/// returned midpoint.
///
/// # Measured (2026-08-13, release)
///
/// On the Gaussian regression objective of
/// `tests::golden_section_shared_search` over `[1.0e4, 1.0e7]` Pa with the 1 Pa
/// stopping rule, this form uses **37** objective evaluations against the **69**
/// used by the recompute-both form it replaced — a **1.86x** reduction — and
/// locates the same maximum to the last bit,
/// `p* = 3000000.2020218512 Pa` in both forms (analytic `3.0e6` Pa; the
/// `0.2020218512 Pa` error is inside the 0.5 Pa a 1 Pa stopping bracket allows).
///
/// # Numerical note — reuse is NOT bit-identical, and the difference is visible
///
/// The carried-over probe keeps the abscissa it was evaluated at, whereas
/// recomputing `a + gr*(b - a)` on the contracted bracket gives a value that
/// differs in the last bits. The naive expectation — that a `1e-16` relative
/// perturbation stays `1e-16` — is **wrong here, and was measured to be wrong**:
/// a last-bit difference in a probe position can change which side of a
/// comparison wins on a nearly-flat objective, or which iteration first trips
/// the `< 1 Pa` test, and the located midpoint then shifts by up to about the
/// stopping-bracket width itself.
///
/// Measured 2026-08-13 (release) by
/// `tests::golden_section_shared_search::probe_reuse_moves_the_choke_far_below_the_stopping_rule`,
/// which runs both forms over 158 real IF97 searches spanning all three bracket
/// shapes this crate uses (in-dome `[p_min, p0]`, subcooled `[p_min, p_bubble]`,
/// superheated `[p_dew, p0]` and `[p_min, p_dew]`):
///
/// | Quantity | Worst over 158 searches |
/// |---|---|
/// | `abs(dp_crit)` | **5.990479e-1 Pa** (stopping rule: 1 Pa) |
/// | `abs(dp_crit)/p` | **4.604111e-8** |
/// | `abs(dG_crit)/G` | **3.429708e-13** |
///
/// Across the full Zaloudek suite the largest observed move was **3 Pa** on a
/// 10.356 MPa choke, i.e. `2.9e-7` relative — about four orders of magnitude
/// inside Zaloudek's own `0.005` relative choke-pressure tolerance — and the
/// mass flux was unchanged to every printed digit at every one of those points.
/// The Marviken and Moody outputs were byte-identical. **The right way to read
/// this is that the choke pressure was only ever defined to ~1 Pa by the
/// stopping rule**, so two forms of the same search agreeing to a few pascals is
/// agreement, not drift — but it is a real difference, not a no-op, and
/// tightening the stopping rule in future would shrink it rather than leave it
/// alone.
///
/// # Arguments
///
/// * `g_of_p` — the objective, taking a pressure in **pascals as a bare `f64`**
///   and returning a `uom`-typed [`MassFlux`]. The bare `f64` is deliberate: the
///   bracket arithmetic is scale-free and would gain nothing from being typed,
///   while the objective's *value* stays dimensioned so a caller cannot confuse
///   a mass flux with a pressure.
/// * `a_pa`, `b_pa` — the bracket ends in pascals, with `a_pa <= b_pa`. A
///   bracket already narrower than 1 Pa returns its midpoint after the two
///   initial probes.
///
/// # Returns
///
/// `(p*, G(p*))` — the located choke pressure and the mass flux there, both
/// `uom`-typed. `p*` is the midpoint of the final bracket, so for a unimodal
/// objective it is within 0.5 Pa of the true maximum.
///
/// # Reference
///
/// Price, C. J., & Robertson, B. L. (2012). Golden Section Search.
/// In Encyclopedia of Engineering Optimization and Heuristics
/// (pp. 1-4). Singapore: Springer Nature Singapore.
///
/// A batched, backend-dispatched generalisation of this same algorithm — same
/// contraction constant, same citation — lives at
/// `outram_foam_basic_lib::math::minimise::golden_section_batch` and names this
/// function as its provenance. That crate is **not** a dependency of
/// `tampines-steam-tables` and deliberately so; see bead `op-uyi3`.
#[inline]
pub(crate) fn golden_section_max_g(
    g_of_p: impl Fn(f64) -> MassFlux,
    a_pa: f64,
    b_pa: f64,
) -> (Pressure, MassFlux) {
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0; // 0.618...
    let mut a = a_pa;
    let mut b = b_pa;
    // c is the left interior probe, d the right one: with 1 - gr == gr^2,
    // c = a + gr^2*(b - a) and d = a + gr*(b - a).
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    let mut gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
    let mut gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
    for _ in 0..100 {
        if (b - a).abs() < 1.0 {
            break;
        } // 1 Pa bracket width
        if gc > gd {
            // peak is in [a, d]. The new right probe of [a, d] is exactly the
            // old left probe c, so carry it over and evaluate only the new c.
            b = d;
            d = c;
            gd = gc;
            c = b - gr * (b - a);
            gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
        } else {
            // peak is in [c, b]. The new left probe of [c, b] is exactly the
            // old right probe d, so carry it over and evaluate only the new d.
            a = c;
            c = d;
            gc = gd;
            d = a + gr * (b - a);
            gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
        }
    }
    let p_star = Pressure::new::<pascal>(0.5 * (a + b));
    (p_star, g_of_p(p_star.get::<pascal>()))
}

/// Critical pressure & mass flux for a subcooled-liquid / liquid-like
/// stagnation state (OUTSIDE the dome, left side).
///
/// Precondition: `(p0, h0)` is single-phase on the liquid side — subcooled
/// liquid (`p0 < p_c`, `T0 < T_sat(p0)`) or liquid-like (`p0 >= p_c`,
/// `T0 < T_c`). The caller's dispatcher is responsible for routing here.
///
/// # Method: the maximum of `G` along the isentrope (gh:#1111, 2026-11-02)
///
/// Along the isentrope `s = s0`, with the stagnation enthalpy `h0`,
///     `G(p) = ρ(p, s0) · √(2 (h0 − h(p, s0)))`,
/// and the choke is its **global maximum over the throat pressure** — the
/// maximum-discharge principle that defines the HEM reference curves this
/// solver is validated against (Moody 1975, "maximum discharge rate";
/// Zaloudek's HEM throats). Along the isentrope `dG/dp|_s = (u² − c²)/(u c²)`,
/// so an interior maximum is a sonic point `u = c`.
///
/// - **Liquid stretch `[p_bubble, p0]`.** `u` is far below the liquid sound
///   speed (≈ 1500 m/s), so `G` rises monotonically; it is not searched.
/// - **The bubble point.** The HEM sound speed drops there from `c_liq` to
///   `c_2φ(x = 0⁺)`, so `G` has a kink. When the kink is the maximum
///   (typically `u_b > c_2φ`, deep subcooling), `p* = p_bubble` and
///   `G* = ρ_f u_b`, `u_b = √(2 (h0 − h_f))` — the Bernoulli discharge.
///   Otherwise the maximum is an interior two-phase sonic point.
/// - **`G*` is continuous in `(p0, h0)`**, being the maximum of a family of
///   continuous functions; only `p*` may jump between two near-equal maxima.
///   Measured 2026-11-02: the worst change of `G*` per 1 J/kg of `h0` over
///   ±300 J/kg is 3.6e-6 (Marviken 4075 kPa), 7.8e-6 and 1.1e-5 (Moody 0.50),
///   2.7e-7 (300 K, 2.31 bar).
///
/// The search is a 160-point geometric scan below `p_bubble` (relative offset
/// 1e-9 to `1 − p_min/p_bubble`) for the largest `G`, including the kink, then
/// [`golden_section_max_g`] inside the two neighbours of the best grid point.
/// The two-phase `G` uses the forward saturation properties at
/// `x = (s0 − s_f)/(s_g − s_f)` (clamped at 0), never a backward equation.
///
/// **A first-local-maximum rule was considered and rejected** (V&V §11): it
/// takes the kink where `G` has a higher interior maximum, and so misses
/// Zaloudek x_t = 0.05 at 1000 psia by −41 %; the references are the global
/// maximum.
///
/// **What this replaced, and why.** Two thresholds used to select the
/// branch:
/// - an `x < 0.03` switch (quality at the global energy maximum) to a
///   bubble-point mass flux `ρ_f c_2φ` read from a sonic map;
/// - a `DEEP_SUBCOOLING_RATIO = 5` escape (`u_b/c_2φ`) back to the energy
///   maximum.
///
/// Both were knife edges: on Marviken test 23 a 0.01 % change in `h0` crossed
/// `x = 0.03` and moved G by 20 %; a Moody 0.50 point sat at a ratio of 5.03.
/// They existed because the energy objective subtracted from the given `h0`
/// while taking `s0` at the backward-equation `T0`, putting up to ≈ 100 J/kg
/// of error into a kinetic energy of 0.1–200 J/kg near saturation, which made
/// the energy maximum overshoot or wander. The `ρ_f c_2φ` branch was also not
/// energy-consistent when `u_b ≠ c_2φ`: it is the flux at a velocity the
/// stagnation enthalpy does not supply. Before/after numbers:
/// `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`
/// §11–§12.
///
/// # Consistent stagnation state
///
/// For a Region 1 stagnation `T0` solves the **forward** equation
/// `h_1(T0, p0) = h0`
/// ([`super::basic_multiphase_equations::t_ph_region_1_forward`]), and
/// `s0 = s_1(T0, p0)`. The given `h0` is kept. `Iapws97Hem`'s unchoked branch
/// uses the same `T0`, so the two meet at `p*`. Region 3 (liquid-like)
/// stagnations keep `s_ph_eqm`. Saturated-liquid `ρ_f`, `h_f` at the bubble
/// point come from the forward equations at `T_sat(p_bubble)`.
///
/// # Validation status
///
/// Zaloudek (1961) HEM subcooled curves, Moody (1975) deep-subcooled points
/// and Marviken tests 23/24: see V&V §12 for the numbers after this change,
/// including the recorded misses (Moody 0.50 / h = 2.2549, Marviken 23's
/// worst point).
#[inline]
pub fn get_critical_pressure_and_mass_flux_subcooled_liquid_ph(
    p0: Pressure,
    h0: AvailableEnergy,
) -> (Pressure, MassFlux) {
    let s0 = match super::basic_multiphase_equations::t_ph_region_1_forward(p0, h0) {
        Some(t0) => crate::region_1_subcooled_liquid::s_tp_1(t0, p0),
        None => s_ph_eqm(p0, h0),
    };
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);
    let p_bubble = bubble_point_pressure_from_entropy(s0);
    let p_b = p_bubble.get::<pascal>();

    // G at the bubble point from the forward saturated-liquid properties.
    let t_bubble = sat_temp_4(p_bubble);
    let rho_f = v_tp_eqm_two_phase(t_bubble, p_bubble, 0.0).recip();
    let h_f = h_tp_eqm_two_phase(t_bubble, p_bubble, 0.0);
    let ke_b = (h0 - h_f).max(AvailableEnergy::ZERO);
    let g_kink = (rho_f * (2.0 * ke_b).r_sqrt()).get::<kilogram_per_square_meter_second>();

    // G inside the dome (p < p_bubble) along s = s0, from the forward
    // saturation properties at the quality x = (s0 − s_f)/(s_g − s_f) (clamped
    // at 0, so a probe within the bubble-point bisection tolerance is not
    // classified as liquid and sent through a backward equation).
    let g_of_p = |p_pa: f64| -> f64 {
        let p = Pressure::new::<pascal>(p_pa);
        let x = two_phase_quality(p, s0).clamp(0.0, 1.0);
        let t_sat = sat_temp_4(p);
        let ke = h0 - h_tp_eqm_two_phase(t_sat, p, x);
        if ke < AvailableEnergy::ZERO {
            return 0.0; // over-expanded guard
        }
        let rho = v_tp_eqm_two_phase(t_sat, p, x).recip();
        (rho * (2.0 * ke).r_sqrt()).get::<kilogram_per_square_meter_second>()
    };

    // Geometric scan below p_bubble.
    let p_lo = p_min.get::<pascal>();
    if p_b <= p_lo * (1.0 + 1e-9) {
        return (p_bubble, MassFlux::new::<kilogram_per_square_meter_second>(g_kink));
    }
    let delta_max = 1.0 - p_lo / p_b;
    let delta_min = 1e-9_f64.min(0.5 * delta_max);
    const N_SCAN: usize = 160;
    let mut grid: Vec<(f64, f64)> = Vec::with_capacity(N_SCAN + 1);
    grid.push((p_b, g_kink));
    for k in 0..N_SCAN {
        let f = k as f64 / (N_SCAN - 1) as f64;
        let delta = delta_min * (delta_max / delta_min).r_powf(f);
        let p_k = p_b * (1.0 - delta);
        grid.push((p_k, g_of_p(p_k)));
    }
    // The global maximum on the grid (the maximum-discharge principle).
    let mut i_best = 0;
    for i in 1..grid.len() {
        if grid[i].1 > grid[i_best].1 {
            i_best = i;
        }
    }
    if i_best == 0 {
        // The bubble point is the maximum: the kink binds.
        return (p_bubble, MassFlux::new::<kilogram_per_square_meter_second>(g_kink));
    }
    if i_best + 1 < grid.len() {
        // Interior maximum, bracketed by its grid neighbours.
        return golden_section_max_g(
            |p_pa| MassFlux::new::<kilogram_per_square_meter_second>(g_of_p(p_pa)),
            grid[i_best + 1].0,
            grid[i_best - 1].0,
        );
    }
    let p_prev = grid[i_best].0;
    let g_prev = grid[i_best].1;
    // G rose all the way to p_min: the steam-table floor is the choke.
    (Pressure::new::<pascal>(p_prev), MassFlux::new::<kilogram_per_square_meter_second>(g_prev))
}

/// Two-phase quality of the isentrope `s = s0` at pressure `p`,
/// `x = (s0 − s_f) / (s_g − s_f)`. Outside the dome it falls below 0 (subcooled
/// liquid) or above 1 (superheated); at or above the critical pressure there is
/// no saturation line and it is reported as fully vapour (1.0) so the caller
/// treats the state as a genuine, non-near-saturation choke.
fn two_phase_quality(p: Pressure, s0: SpecificHeatCapacity) -> f64 {
    if p >= crate::constants::p_crit_water() {
        return 1.0;
    }
    let tsat = sat_temp_4(p);
    let s_f = s_tp_eqm_two_phase(tsat, p, 0.0);
    let s_g = s_tp_eqm_two_phase(tsat, p, 1.0);
    ((s0 - s_f) / (s_g - s_f)).get::<uom::si::ratio::ratio>()
}

/// Critical pressure & mass flux for a superheated-vapour / supercritical
/// (vapour-like) stagnation state (OUTSIDE the dome, right side / above the
/// dome).
///
/// Precondition: `(p0, h0)` is single-phase on the vapour side — superheated
/// vapour (`p0 < p_c`, `T0 > T_sat(p0)`) or supercritical vapour-like
/// (`s0 > s_crit`). The caller's dispatcher is responsible for routing here.
/// The distinguishing test is `s0 > s_crit`: such an isentrope can only re-enter
/// the dome across the **dew** line (x = 1), never the bubble line.
///
/// Method (energy-balance max-G, HEM — the same `G(p)` curve as the in-dome and
/// subcooled solvers, no sound speed involved): along the isentrope `s = s0`,
///   `G(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )`.
///
/// This is the mirror image of
/// [`get_critical_pressure_and_mass_flux_subcooled_liquid_ph`], with the **dew**
/// point playing the role the bubble point plays on the liquid side. There are
/// two candidate stretches and the choke is the global maximum of `G` over both:
///
/// * **Single-phase vapour stretch `[p_dew, p0]`.** Unlike the liquid side
///   (where `G` is monotone up to the bubble point), `G` here has an *interior*
///   peak — the ordinary perfect-gas-like **vapour sonic choke** — because the
///   vapour expands and `rho` falls steeply. This stretch therefore needs its
///   own golden-section maximisation, not a single endpoint evaluation.
/// * **Two-phase stretch `[p_min, p_dew]`.** Below the dew point the flow is a
///   condensing mist; `G` develops a second (possibly higher) peak there.
///
///   `G_crit = max( max_{[p_dew, p0]} G,  max_{[p_min, p_dew]} G )`
/// * vapour-stretch peak wins -> **vapour sonic choke** (strongly superheated)
/// * two-phase peak wins      -> **condensation choke** (near-saturated vapour)
///
/// For strongly superheated / supercritical inlets the dew point sits far below
/// the choke and the vapour-stretch peak dominates, recovering the classical
/// single-phase steam-nozzle result; near the saturated-vapour line the
/// two-phase peak can take over.
///
/// # Known limitation — near-saturated stagnation (x_t ≈ 1)
///
/// Mirroring the bubble-point limitation on the liquid side, for stagnation
/// states very close to the dew point the HEM equilibrium assumption breaks
/// down (droplet condensation lags the local pressure drop). Reproducing the
/// x ≈ 1 choking line faithfully requires a non-equilibrium / relaxation model
/// (HRM). Interior superheat is well represented.
#[inline]
pub fn get_critical_pressure_and_mass_flux_superheated_vapour_ph(
    p0: Pressure,
    h0: AvailableEnergy,
) -> (Pressure, MassFlux) {
    let s0 = s_ph_eqm(p0, h0);
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);

    // mass flux from energy conservation at pressure p (along s = s0)
    let g_of_p = |p_pa: f64| -> MassFlux {
        let p = Pressure::new::<pascal>(p_pa);
        let h = h_ps_eqm(p, s0);
        let ke = h0 - h; // kinetic energy per unit mass
        if ke < AvailableEnergy::ZERO {
            return MassFlux::ZERO; // over-expanded guard
        }
        let rho = v_ps_eqm(p, s0).recip();
        rho * (2.0 * ke).r_sqrt() // = rho * u
    };

    // dew point: where the isentrope first reaches saturation (x = 1) on
    // depressurisation. Clamped to p_min if the isentrope never re-enters the
    // dome (s0 above the triple-point saturated-vapour entropy).
    let p_dew = dew_point_pressure_from_entropy(s0);
    let p_dew_pa = p_dew.get::<pascal>();
    let p_min_pa = p_min.get::<pascal>();
    let p0_pa = p0.get::<pascal>();

    // single-phase vapour peak (vapour sonic choke). Interior maximum, so it
    // needs a full golden-section search rather than an endpoint evaluation.
    let (p_vapour, g_vapour) = golden_section_max_g(&g_of_p, p_dew_pa, p0_pa);

    // two-phase (condensing) peak below the dew point. Skip if the isentrope
    // never re-enters the dome (p_dew clamped to p_min).
    let (p_two_phase, g_two_phase) = if p_dew_pa - p_min_pa > 1.0 {
        golden_section_max_g(&g_of_p, p_min_pa, p_dew_pa)
    } else {
        (p_dew, g_of_p(p_dew_pa))
    };

    // global maximum along the isentrope = the choke (critical) condition
    if g_vapour.get::<kilogram_per_square_meter_second>()
        >= g_two_phase.get::<kilogram_per_square_meter_second>()
    {
        (p_vapour, g_vapour) // vapour sonic choke
    } else {
        (p_two_phase, g_two_phase) // condensation choke
    }
}
