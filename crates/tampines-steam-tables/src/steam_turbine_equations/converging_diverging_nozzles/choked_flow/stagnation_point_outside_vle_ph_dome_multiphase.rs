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

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::megapascal;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use super::basic_multiphase_equations::bubble_point_pressure_from_entropy;
use super::basic_multiphase_equations::dew_point_pressure_from_entropy;

/// Golden-section maximisation of the HEM energy-balance mass flux `g_of_p`
/// over the pressure bracket `[a_pa, b_pa]` (both in Pa, `a_pa <= b_pa`).
///
/// `g_of_p` is unimodal on each single-phase / two-phase stretch of the
/// isentrope (zero at the high-pressure end, a single interior peak at the
/// choke, then falling toward `p_min`), so this is robust and needs no
/// derivative of the noisy sound speed. On a monotone stretch it converges to
/// the appropriate endpoint, which is the intended behaviour when the real peak
/// lies in the neighbouring stretch.
///
/// Reference:
///   Price, C. J., & Robertson, B. L. (2012). Golden Section Search.
///   In Encyclopedia of Engineering Optimization and Heuristics
///   (pp. 1-4). Singapore: Springer Nature Singapore.
#[inline]
fn golden_section_max_g(
    g_of_p: impl Fn(f64) -> MassFlux,
    a_pa: f64,
    b_pa: f64,
) -> (Pressure, MassFlux) {
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;     // 0.618...
    let mut a = a_pa;
    let mut b = b_pa;
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    for _ in 0..100 {
        if (b - a).abs() < 1.0 { break; }      // 1 Pa bracket width
        let gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
        let gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
        if gc > gd {
            b = d;                             // peak is in [a, d]
        } else {
            a = c;                             // peak is in [c, b]
        }
        c = b - gr * (b - a);
        d = a + gr * (b - a);
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
/// Method (energy-balance max-G, HEM — same `G(p)` curve as the in-dome
/// solver, no sound speed involved):
///   along the isentrope `s = s0`,
///     `G(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )`
///
/// * In the single-phase liquid stretch `[p_bubble, p0]`, `rho ~ const` and the
///   enthalpy drop is tiny, so `G` rises monotonically to its liquid-region
///   maximum **at the bubble point**.
/// * Below the bubble point the flow flashes, the HEM sound speed collapses,
///   and `G` develops a (possibly interior) peak in the two-phase region.
///
/// The choke is the global maximum of `G` along the whole isentrope:
///   `G_crit = max( G(p_bubble),  max_{p in [p_min, p_bubble]} G(p) )`
/// * interior two-phase peak wins -> **flashing choke**
/// * bubble-point value wins      -> **bubble-point choke** (strongly subcooled)
///
/// This avoids `mass_flux_ps_eqm_throat` (finite-difference sound speed +
/// bubble-point clamp) entirely; it only needs smooth `h(p,s0)`, `v(p,s0)`.
///
/// # Validation status
///
/// Validated against Zaloudek (1961) HEM critical mass flux curves for
/// genuinely subcooled stagnation states (throat quality x_t = 0.05–1.00,
/// stagnation subcooling ΔH_sub ≥ ~12 kJ/kg). All 20 curves pass within
/// tolerance.
///
/// # Known limitation — near-saturated stagnation (x_t ≈ 0)
///
/// For stagnation states very close to the bubble point (ΔH_sub < ~10 kJ/kg,
/// i.e. throat quality x_t ≈ 0), the HEM equilibrium assumption breaks down:
///
/// * **5–10 psia**: spurious mass-flux artifacts (HEM overpredicts G by 3–7×).
/// * **15–200 psia**: choke pressure is 11–21% below the measured throat.
///
/// This is a fundamental limitation of the Homogeneous **Equilibrium** Model
/// (instantaneous flashing assumed at the bubble point). Reproducing the
/// x ≈ 0 choking line requires a non-equilibrium / relaxation model (HRM).
/// The test `quality_bubble_point_subcooled` covering this regime is
/// `#[ignore]`d accordingly.
#[inline]
pub fn get_critical_pressure_and_mass_flux_subcooled_liquid_ph(
    p0: Pressure,
    h0: AvailableEnergy,
) -> (Pressure, MassFlux) {

    let s0 = s_ph_eqm(p0, h0);
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);

    // mass flux from energy conservation at pressure p (along s = s0)
    let g_of_p = |p_pa: f64| -> MassFlux {
        let p = Pressure::new::<pascal>(p_pa);
        let h = h_ps_eqm(p, s0);
        let ke = h0 - h;                       // kinetic energy per unit mass
        if ke < AvailableEnergy::ZERO {
            return MassFlux::ZERO;             // over-expanded guard
        }
        let rho = v_ps_eqm(p, s0).recip();
        rho * (2.0 * ke).sqrt()                // = rho * u
    };

    // bubble point: where the isentrope first reaches saturation (x = 0).
    // The single-phase liquid stretch above it is monotonic in G, so its
    // maximum is exactly this point — one evaluation, no search needed.
    let p_bubble = bubble_point_pressure_from_entropy(s0);
    let g_bubble = g_of_p(p_bubble.get::<pascal>());

    // two-phase maximum: golden-section over [p_min, p_bubble]. G is unimodal
    // here, so this is robust and needs no derivative of the noisy sound speed.
    //
    // Reference:
    //   Price, C. J., & Robertson, B. L. (2012). Golden Section Search.
    //   In Encyclopedia of Engineering Optimization and Heuristics
    //   (pp. 1-4). Singapore: Springer Nature Singapore.
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;     // 0.618...
    let mut a = p_min.get::<pascal>();
    let mut b = p_bubble.get::<pascal>();
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    for _ in 0..100 {
        if (b - a).abs() < 1.0 { break; }      // 1 Pa bracket width
        let gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
        let gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
        if gc > gd {
            b = d;                             // peak is in [a, d]
        } else {
            a = c;                             // peak is in [c, b]
        }
        c = b - gr * (b - a);
        d = a + gr * (b - a);
    }
    let p_two_phase = Pressure::new::<pascal>(0.5 * (a + b));
    let g_two_phase = g_of_p(p_two_phase.get::<pascal>());

    // global maximum along the isentrope = the choke (critical) condition
    if g_two_phase.get::<kilogram_per_square_meter_second>()
        >= g_bubble.get::<kilogram_per_square_meter_second>()
    {
        (p_two_phase, g_two_phase)             // flashing choke
    } else {
        (p_bubble, g_bubble)                   // bubble-point choke
    }
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
        let ke = h0 - h;                       // kinetic energy per unit mass
        if ke < AvailableEnergy::ZERO {
            return MassFlux::ZERO;             // over-expanded guard
        }
        let rho = v_ps_eqm(p, s0).recip();
        rho * (2.0 * ke).sqrt()                // = rho * u
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
        (p_vapour, g_vapour)                   // vapour sonic choke
    } else {
        (p_two_phase, g_two_phase)             // condensation choke
    }
}
