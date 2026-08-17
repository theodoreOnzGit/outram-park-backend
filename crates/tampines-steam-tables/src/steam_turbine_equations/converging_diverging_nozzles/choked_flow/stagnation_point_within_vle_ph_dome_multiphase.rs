// Critical-flow solvers for the case where the STAGNATION state lies
// inside the p-h VLE dome (two-phase, at or below the critical point).
//
// These exploit the simplification that an isentrope starting inside the
// dome stays inside it on depressurisation (the dome only widens as p
// falls), so no region switching or flashing event needs handling here.

use uom::ConstZero;
use uom::si::f64::*;
use uom::si::pressure::megapascal;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use super::stagnation_point_outside_vle_ph_dome_multiphase::golden_section_max_g;

/// Critical pressure & mass flux for a stagnation state that sits
/// INSIDE the p-h VLE dome (two-phase, at or below the critical point).
///
/// Precondition: (p0, h0) is two-phase — i.e. ph_flash_region(p0,h0) == Region4.
/// Once inside the dome, isentropic depressurisation stays inside it
/// (the dome only widens as p falls), so there is no flashing event and
/// no region switching to handle here.
///
/// Method (Moody / max-flux form of the HEM choking criterion):
///   along the isentrope s = s0,
///     G(p) = rho(p,s0) * sqrt( 2 * (h0 - h(p,s0)) )
///   G(p0) = 0, rises to a single interior maximum at the choke point,
///   then falls as rho -> 0. The choke is argmax_p G(p).
///
/// This avoids mass_flux_ps_eqm_throat (finite-difference sound speed +
/// bubble-point clamp) entirely; it only needs smooth h(p,s0), v(p,s0).
///
/// Consistent with the validated inverse map: max-G <=> Mach 1 <=>
/// h0 = h_t + 0.5 * u_t^2  (get_stagnation_conditions_from_throat_ps).
///
/// # Validation status
///
/// Validated against Zaloudek (1961) HEM critical mass flux curves for
/// two-phase stagnation states (throat quality x_t = 0.0–1.00, all 21
/// quality curves). All in-dome points pass within tolerance (worst error
/// ~0.86% pressure at 100 psia for x_t = 0.05, near the bubble-point edge
/// of the dome).
#[inline]
pub fn get_critical_pressure_and_mass_flux_ph_vle_dome(
    p0: Pressure,
    h0: AvailableEnergy,
) -> (Pressure, MassFlux) {
    // isentrope to march down
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

    // Maximise G over [p_min, p0] with the crate's shared golden-section search,
    // [`golden_section_max_g`]. G is unimodal here (zero at p0, single interior
    // peak at the choke, falling toward p_min), so this is robust and needs no
    // derivative of the noisy sound speed.
    //
    // This used to be an inline copy of that loop (bead `op-uyi3`). The copy's
    // own comment claimed the golden ratio's defining property — "one probe is
    // reused after each bracket reduction, costing one G-evaluation per
    // iteration" — while the code below it evaluated both probes every
    // iteration. The shared function actually implements the reuse, so the
    // claim is now true rather than aspirational.
    golden_section_max_g(g_of_p, p_min.get::<pascal>(), p0.get::<pascal>())
}
