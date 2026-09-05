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

use std::sync::OnceLock;

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
/// Bernoulli maximum. Set above the maximum reached by any Zaloudek subcooled
/// reference point (3.30) so this only adds deep-subcooling behaviour and never
/// perturbs the validated near-bubble (sonic) regime. See README v0.2.1.
///
/// Public so the Moody deep-subcooling verification test can classify points with
/// the exact same threshold the solver uses.
pub const DEEP_SUBCOOLING_RATIO: f64 = 5.0;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::mass_flux_ps_eqm_throat;
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
/// Method: two-regime choke finder along the isentrope `s = s0`,
///     `G_energy(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )`
///
/// * **Genuinely subcooled** (throat quality > ~0.03): the choke is a smooth
///   interior two-phase point where the energy-balance maximum of `G_energy`
///   coincides with the sonic point (`dG/dp = 0 ⇔ v = c`). Located by
///   golden-section over `[p_min, p_bubble]`.
/// * **Near-saturated** (throat quality ≲ 0.03, i.e. throat on the saturated-
///   liquid line): the energy maximum is *not* the choke — it either overshoots
///   `rho_f·v ≫ rho_f·c` at the bubble point or walks off to a deeper stationary
///   point the flow never reaches at `M = 1`. Here the choke is the bubble-point
///   kink itself; the mass flux `rho_f·c_2φ` is read from a precomputed sonic map
///   along the saturated-liquid line (see [`saturation_line_sonic_mass_flux`]).
///
/// The regime is selected by the two-phase quality at the energy-max choke,
/// which is the only quantity that cleanly separates the two cases (stagnation
/// subcooling and pressure both overlap between them).
///
/// # Validation status
///
/// Validated against Zaloudek (1961) HEM critical mass flux curves for all
/// throat qualities x_t = 0.0–1.00 (the saturated-liquid-line curve x_t ≈ 0
/// included). All curves pass within tolerance.
///
/// # Note — the x ≈ 0 curve is numerical, not a physics limit
///
/// The near-saturation correction exists because the energy-balance objective
/// is blind to the discontinuity in the HEM sound speed at the bubble point, and
/// the pointwise sonic function is unreliable in the thin band just below it —
/// *not* because HEM cannot reproduce the x ≈ 0 line. The Zaloudek reference is
/// itself HEM, and `mass_flux_ps_eqm_throat` evaluated at the throat reproduces
/// it to ±0.04 in log10 G at every point.
#[inline]
pub fn get_critical_pressure_and_mass_flux_subcooled_liquid_ph(
    p0: Pressure,
    h0: AvailableEnergy,
) -> (Pressure, MassFlux) {
    let s0 = s_ph_eqm(p0, h0);
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);

    // Energy-balance mass flux at pressure p along s = s0: G_e(p) = ρ·√(2(h0−h))
    // (= ρ·v, the flux if all available enthalpy becomes kinetic energy).
    let g_energy_of_p = |p_pa: f64| -> f64 {
        let p = Pressure::new::<pascal>(p_pa);
        let h = h_ps_eqm(p, s0);
        let ke = h0 - h; // kinetic energy per unit mass
        if ke < AvailableEnergy::ZERO {
            return 0.0; // over-expanded guard
        }
        let rho = v_ps_eqm(p, s0).recip();
        (rho * (2.0 * ke).sqrt()).get::<kilogram_per_square_meter_second>()
    };
    let p_bubble = bubble_point_pressure_from_entropy(s0);

    // Energy-balance choke candidate: golden-section maximise G_energy over
    // [p_min, p_bubble] with the shared [`golden_section_max_g`]. G is unimodal
    // here, so this is robust and needs no derivative of the noisy sound speed.
    // This used to be an inline copy of that loop (bead `op-uyi3`); it is a call
    // now so the two cannot drift.
    let g_bubble = g_energy_of_p(p_bubble.get::<pascal>());
    let (p_two_phase, g_two_phase_typed) = golden_section_max_g(
        |p_pa| MassFlux::new::<kilogram_per_square_meter_second>(g_energy_of_p(p_pa)),
        p_min.get::<pascal>(),
        p_bubble.get::<pascal>(),
    );
    let g_two_phase = g_two_phase_typed.get::<kilogram_per_square_meter_second>();

    let (p_energy, g_energy) = if g_two_phase >= g_bubble {
        (p_two_phase, g_two_phase) // flashing choke
    } else {
        (p_bubble, g_bubble) // bubble-point choke
    };

    // ── Near-saturation (x ≈ 0) correction ──────────────────────────────────
    //
    // The energy-balance objective G(p) = ρ·√(2(h0−h)) is blind to the
    // discontinuity in the HEM sound speed at the bubble point (c drops from
    // ~1500 m/s liquid to ~0.5 m/s two-phase). Maximising it equals the choke
    // only at a smooth interior stationary point (dG/dp = 0 ⇔ v = c). When the
    // throat is essentially on the saturated-liquid line, the energy max either
    // overshoots ρ_f·v ≫ ρ_f·c at the bubble point, or walks off to a deeper
    // stationary point the flow never reaches at M = 1. We detect this by the
    // two-phase quality at the energy-max choke: below 0.03 the throat is
    // effectively saturated liquid → take the bubble-point sonic kink, mass flux
    // ρ_f·c_2φ from the saturated-liquid-line sonic map.
    //
    // ⚠ KNOWN LIMITATION — the overlap zone.
    // The quality discriminator alone handles every Zaloudek subcooled curve but
    // cannot, by itself, recognise *deeply* subcooled stagnation (Moody's low-
    // enthalpy isobar points), where the two-phase quality at the choke is also
    // ≈ 0 yet the true choke is the energy-balance / Bernoulli maximum, not the
    // sonic kink. No clean local candidate (quality, stagnation subcooling,
    // v_b/c_2φ, pressure) fully separates the two regimes inside their overlap:
    // e.g. Zaloudek 10 psia (v_b/c_2φ ≈ 3.3, wants the sonic value 748 kg/m²s) and
    // Moody p/p_ref=8 / h≈4.47 (v_b/c_2φ ≈ 3.1, wants the Bernoulli value
    // ≈ 57 700 kg/m²s) sit on top of each other in every local parameter but want
    // opposite branches — HEM equilibrium is known to under-predict subcooled
    // critical flow, and Moody's deep branch is effectively non-equilibrium.
    // The deep-subcooling escape below resolves the *unambiguous* deep region
    // (v_b/c_2φ above the whole Zaloudek range); states inside the overlap defer to
    // the precise Zaloudek (sonic) treatment. See README v0.2.1 ("Subcooled choked
    // flow has two regimes").
    // ── Deep-subcooling escape ───────────────────────────────────────────────
    //
    // Make the solver usable far into the subcooled region (e.g. for transient
    // FHR coupling) without disturbing the validated near-bubble behaviour. The
    // ratio of the Bernoulli velocity at the bubble point
    // v_b = √(2(h0 − h_f(p_b))) to the two-phase sound speed there c_2φ measures
    // how deeply subcooled the stagnation is. Every Zaloudek subcooled reference
    // point has v_b/c_2φ ≤ 3.30; the Moody deep-subcooling points have
    // v_b/c_2φ ≫ 6 (up to ~1600). Above DEEP_SUBCOOLING_RATIO the state is
    // unambiguously past Zaloudek's range, so the choke is the energy-balance /
    // Bernoulli maximum (which recovers the incompressible √(2·ρ_f·Δp) discharge
    // for the deepest points). At or below the threshold we defer to the validated
    // near-bubble logic — Zaloudek's near-saturation data is far more precise there
    // than Moody's, and no local discriminator separates the two regimes inside
    // the overlap (see README v0.2.1 and the comment below). This branch therefore
    // only *adds* correct deep-subcooling behaviour; it cannot change any
    // near-bubble (Zaloudek) result.
    let g_sonic_bubble = saturation_line_sonic_mass_flux(p_bubble);
    let t_bubble = sat_temp_4(p_bubble);
    let rho_f = v_tp_eqm_two_phase(t_bubble, p_bubble, 0.0).recip();
    let h_f_bubble = h_tp_eqm_two_phase(t_bubble, p_bubble, 0.0);
    let dh_bubble = (h0 - h_f_bubble).max(AvailableEnergy::ZERO);
    let v_bubble = (2.0 * dh_bubble).sqrt();
    let subcooling_ratio = (v_bubble * rho_f / g_sonic_bubble).get::<uom::si::ratio::ratio>();
    if subcooling_ratio > DEEP_SUBCOOLING_RATIO {
        return (
            p_energy,
            MassFlux::new::<kilogram_per_square_meter_second>(g_energy),
        );
    }

    let x_at_energy = two_phase_quality(p_energy, s0);
    if x_at_energy < 0.03 {
        (p_bubble, g_sonic_bubble)
    } else {
        (
            p_energy,
            MassFlux::new::<kilogram_per_square_meter_second>(g_energy),
        )
    }
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

/// Precomputed map of the HEM two-phase sonic mass flux `ρ_f·c_2φ` along the
/// saturated-liquid line, used to correct near-saturation choked flow.
///
/// Returns a `(bubble_pressure_Pa, mass_flux_kg_m2_s)` table sorted by
/// pressure, log-spaced from ~3 psia to ~3000 psia (the near-saturation
/// envelope plus margin). Each entry is `mass_flux_ps_eqm_throat` evaluated at
/// the saturated-liquid state — i.e. the downstream (two-phase) sound speed at
/// the bubble-point kink. Built once (this is pure HEM/IAPWS, no external data)
/// and cached.
fn saturation_line_sonic_map() -> &'static Vec<(f64, f64)> {
    static MAP: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
    MAP.get_or_init(|| {
        let p_lo = 20_000.0_f64; // ~3 psia
        let p_hi = 21_000_000.0_f64; // ~3000 psia
        let n = 13;
        (0..n)
            .map(|i| {
                let f = i as f64 / (n - 1) as f64;
                let p_pa = p_lo * (p_hi / p_lo).powf(f);
                let p = Pressure::new::<pascal>(p_pa);
                let s_f = s_tp_eqm_two_phase(sat_temp_4(p), p, 0.0);
                let g = mass_flux_ps_eqm_throat(p, s_f).get::<kilogram_per_square_meter_second>();
                (p_pa, g)
            })
            .collect()
    })
}

/// Saturated-liquid-line sonic mass flux at `p_bubble`, by log–log linear
/// interpolation of [`saturation_line_sonic_map`] (clamped at the table ends).
fn saturation_line_sonic_mass_flux(p_bubble: Pressure) -> MassFlux {
    let map = saturation_line_sonic_map();
    let n = map.len();
    let lp = p_bubble.get::<pascal>().ln();

    if lp <= map[0].0.ln() {
        return MassFlux::new::<kilogram_per_square_meter_second>(map[0].1);
    }
    if lp >= map[n - 1].0.ln() {
        return MassFlux::new::<kilogram_per_square_meter_second>(map[n - 1].1);
    }
    for w in map.windows(2) {
        let (p0, g0) = w[0];
        let (p1, g1) = w[1];
        let (lp0, lp1) = (p0.ln(), p1.ln());
        if lp >= lp0 && lp <= lp1 {
            let t = (lp - lp0) / (lp1 - lp0);
            let lg = g0.ln() * (1.0 - t) + g1.ln() * t;
            return MassFlux::new::<kilogram_per_square_meter_second>(lg.exp());
        }
    }
    MassFlux::new::<kilogram_per_square_meter_second>(map[n - 1].1)
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
        rho * (2.0 * ke).sqrt() // = rho * u
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
