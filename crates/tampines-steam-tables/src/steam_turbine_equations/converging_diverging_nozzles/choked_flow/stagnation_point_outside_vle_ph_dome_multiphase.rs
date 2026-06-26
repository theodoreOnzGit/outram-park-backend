//! Critical-flow solvers for stagnation states that lie OUTSIDE the p-h VLE
//! dome (single phase at the inlet).
//!
//! This module currently covers the **subcooled-liquid / liquid-like** bucket
//! (left of the dome). On isentropic depressurisation such a state stays
//! single-phase liquid down to the bubble point, then flashes into the dome.
//! The superheated-vapour / supercritical buckets (right of the dome) will be
//! added here later.

use std::sync::OnceLock;

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::megapascal;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::interfaces::functional_programming::pt_flash_eqm::s_tp_eqm_two_phase;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::mass_flux_ps_eqm_throat;
use crate::region_4_vap_liq_equilibrium::sat_temp_4;
use super::basic_multiphase_equations::bubble_point_pressure_from_entropy;

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
        let ke = h0 - h;                       // kinetic energy per unit mass
        if ke < AvailableEnergy::ZERO {
            return 0.0;                        // over-expanded guard
        }
        let rho = v_ps_eqm(p, s0).recip();
        (rho * (2.0 * ke).sqrt()).get::<kilogram_per_square_meter_second>()
    };
    let p_bubble = bubble_point_pressure_from_entropy(s0);

    // Energy-balance choke candidate: golden-section maximise G_energy over
    // [p_min, p_bubble]. G is unimodal here, so this is robust and needs no
    // derivative of the noisy sound speed.
    //
    // Reference:
    //   Price, C. J., & Robertson, B. L. (2012). Golden Section Search.
    //   In Encyclopedia of Engineering Optimization and Heuristics
    //   (pp. 1-4). Singapore: Springer Nature Singapore.
    let g_bubble = g_energy_of_p(p_bubble.get::<pascal>());
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0;     // 0.618...
    let mut a = p_min.get::<pascal>();
    let mut b = p_bubble.get::<pascal>();
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    for _ in 0..100 {
        if (b - a).abs() < 1.0 { break; }      // 1 Pa bracket width
        if g_energy_of_p(c) > g_energy_of_p(d) {
            b = d;                             // peak is in [a, d]
        } else {
            a = c;                             // peak is in [c, b]
        }
        c = b - gr * (b - a);
        d = a + gr * (b - a);
    }
    let p_two_phase = Pressure::new::<pascal>(0.5 * (a + b));
    let g_two_phase = g_energy_of_p(p_two_phase.get::<pascal>());

    let (p_energy, g_energy) = if g_two_phase >= g_bubble {
        (p_two_phase, g_two_phase)             // flashing choke
    } else {
        (p_bubble, g_bubble)                   // bubble-point choke
    };

    // ── Near-saturation (x ≈ 0) correction ──────────────────────────────────
    //
    // The energy-balance objective G(p) = ρ·√(2(h0−h)) is blind to the
    // discontinuity in the HEM sound speed at the bubble point (c drops from
    // ~1500 m/s liquid to ~0.5 m/s two-phase). Maximising it equals the choke
    // only at a smooth interior stationary point (dG/dp = 0 ⇔ v = c). When the
    // throat is essentially on the saturated-liquid line, the energy max either
    // overshoots ρ_f·v ≫ ρ_f·c at the bubble point, or walks off to a deeper
    // stationary point the flow never reaches at M = 1 (choke pressure 11–21 %
    // below the throat). The pointwise HEM sonic function is itself unreliable in
    // the thin band just below the bubble point, so we do not root-find v = c
    // there. (The Zaloudek x≈0 reference is itself HEM, so this is numerical, not
    // a physics limitation.)
    //
    // Discriminator: the two-phase quality at the energy-max choke. A genuine
    // interior choke lands at the real throat quality (≥ ~0.044 for every
    // validated subcooled curve); the near-saturation artifact lands at x ≲ 0.018
    // (or on the bubble point, x = 0). Below 0.03 the throat is effectively
    // saturated liquid: take the choke at the bubble point with the mass flux
    // interpolated from the saturated-liquid-line sonic map (ρ_f·c_2φ), which is
    // robust where the energy max is not.
    let x_at_energy = two_phase_quality(p_energy, s0);
    if x_at_energy < 0.03 {
        (p_bubble, saturation_line_sonic_mass_flux(p_bubble))
    } else {
        (p_energy, MassFlux::new::<kilogram_per_square_meter_second>(g_energy))
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
        let p_lo = 20_000.0_f64;       // ~3 psia
        let p_hi = 21_000_000.0_f64;   // ~3000 psia
        let n = 13;
        (0..n)
            .map(|i| {
                let f = i as f64 / (n - 1) as f64;
                let p_pa = p_lo * (p_hi / p_lo).powf(f);
                let p = Pressure::new::<pascal>(p_pa);
                let s_f = s_tp_eqm_two_phase(sat_temp_4(p), p, 0.0);
                let g = mass_flux_ps_eqm_throat(p, s_f)
                    .get::<kilogram_per_square_meter_second>();
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
