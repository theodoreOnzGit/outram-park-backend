//! Semi-analytical solution of the HEM water double rarefaction ("expansion
//! tube"), plan 03 §4.2 (gh #896).
//!
//! # The problem
//!
//! Subcooled liquid water at `(p0, T0)` moves apart: `u = −u0` left of the
//! centre, `+u0` right of it. The exact HEM solution is two symmetric
//! rarefactions along the isentrope `s0 = s(p0, T0)`, joined by a quiescent
//! centre state `(p*, u = 0)`.
//!
//! Across a left-facing rarefaction the Riemann invariant
//! `u + ∫ dp/(ρc)` is constant, so the velocity reached at pressure `p` is
//!
//! ```text
//! u(p) = −u0 + ∫_p^{p0} dp' / (ρ(p', s0) · c(p', s0))
//! ```
//!
//! with `c` the homogeneous-**equilibrium** sound speed. Symmetry gives
//! `u(p*) = 0`. Because `c` drops discontinuously where the isentrope enters
//! the dome (at the bubble pressure `p_b`), the left wave splits into a liquid
//! fan, a constant saturated-liquid plateau at `(p_b, u_b = u(p_b))`, and a
//! two-phase fan, before the centre state.
//!
//! # Method
//!
//! A bisection for `p_b` on the `(p,s)` flash's own Region 1 / Region 4 test, two `petir::integration::qags`
//! quadratures (liquid and two-phase, split at the jump), and one bracketed
//! Brent root for `p*`. Every property is this crate's IAPWS-IF97 `(p,s)` /
//! `(p,h)` flash; the equilibrium speed is `w_ph_eqm_hem` (Kieffer 1977 eq. 28
//! in the dome). So the reference is semi-analytical and shares the
//! thermodynamics, not the numerics, of the solver it checks.
//!
//! # References
//!
//! The Riemann-invariant construction is standard (Toro 2009, ch. 4, which
//! this workspace's Sod tests already cite). The candidate references for
//! real-EOS / phase-change wave curves listed in plan 03 §4.2 (Menikoff &
//! Plohr 1989; Müller & Voss 2006; Saurel, Petitpas & Abgrall 2008) are
//! **not verified**: none is in the kovan corpus and none was read for this
//! work, so none is cited as support here.

use petir::integration::qags;
use petir::roots::{BracketingMethod, BracketingSolver};
use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    ph_flash_region, w_ph_eqm_hem,
};
use tampines_steam_tables::interfaces::functional_programming::ps_flash_eqm::{
    h_ps_eqm, ps_flash_region, v_ps_eqm,
};
use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::{
    s_tp_eqm_single_phase, FwdEqnRegion,
};
use uom::si::f64::{Pressure, SpecificHeatCapacity, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

/// The semi-analytical answer.
#[derive(Debug, Clone, Copy)]
pub struct DoubleRarefaction {
    /// Initial pressure `p0`, Pa.
    pub p0: f64,
    /// Initial temperature `T0`, K.
    pub t0: f64,
    /// Initial half-separation speed `u0`, m/s.
    pub u0: f64,
    /// Isentrope entropy `s0`, J/(kg·K).
    pub s0: f64,
    /// Bubble pressure where the isentrope meets the saturated-liquid line, Pa.
    pub p_bubble: f64,
    /// Velocity of the left saturated-liquid plateau, `u(p_b)`, m/s.
    pub u_bubble: f64,
    /// Liquid sound speed at `p0` and at `p_b` (liquid side), m/s.
    pub c_liquid: (f64, f64),
    /// Equilibrium speed just inside the dome at `p_b`, m/s.
    pub c_dome_at_bubble: f64,
    /// Centre pressure `p*`, Pa.
    pub p_star: f64,
    /// Equilibrium speed at `p*`, m/s.
    pub c_star: f64,
    /// Density at `p*`, kg/m³.
    pub rho_star: f64,
}

/// `ρ(p, s0)` and the equilibrium speed `c(p, s0)`.
pub fn rho_c(p: f64, s0: f64) -> (f64, f64) {
    let pq = Pressure::new::<pascal>(p);
    let s = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(s0);
    let rho = v_ps_eqm(pq, s).get::<cubic_meter_per_kilogram>().recip();
    let c = w_ph_eqm_hem(pq, h_ps_eqm(pq, s)).get::<meter_per_second>();
    (rho, c)
}

fn inv_rho_c(p: f64, s0: f64) -> f64 {
    let (rho, c) = rho_c(p, s0);
    1.0 / (rho * c)
}

/// `∫_a^b dp/(ρc)` along `s0`, to `1e-7` relative: tighter requests stop on
/// round-off (`qags` reports `RoundOff` at `1e-10`), because the integrand comes
/// from IF97 backward `(p,s)` correlations, consistent to about `1e-6`.
pub fn riemann_integral(a: f64, b: f64, s0: f64) -> f64 {
    qags(|p| inv_rho_c(p, s0), a, b, 0.0, 1e-7, 1000)
        .expect("qags on the isentrope")
        .value
}

/// Solve the problem for `(p0, T0, u0)`; `p0` in Pa, `T0` in K, `u0` in m/s.
pub fn solve(p0: f64, t0: f64, u0: f64) -> DoubleRarefaction {
    let s0 = s_tp_eqm_single_phase(
        ThermodynamicTemperature::new::<kelvin>(t0),
        Pressure::new::<pascal>(p0),
    )
    .get::<joule_per_kilogram_kelvin>();
    // Bubble pressure: where the flashes move from Region 1 to Region 4 along
    // s0 (either the (p,s) or the (p,h) classification saying "dome"). Bisection on the flash's own region test, so the
    // liquid and dome branches below switch exactly where the closure does
    // (a root of s_f(T_sat(p)) = s0 differs from it by the IF97 backward
    // consistency error, enough to put a "liquid" probe inside the dome).
    let s0q = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(s0);
    // Classified by the (p,h) flash at h(p, s0), because that is what places
    // the equilibrium sound speed (`w_ph_eqm_hem`) and the solver's closure.
    let in_dome = |p: f64| {
        let pq = Pressure::new::<pascal>(p);
        ps_flash_region(pq, s0q) == FwdEqnRegion::Region4
            || ph_flash_region(pq, h_ps_eqm(pq, s0q)) == FwdEqnRegion::Region4
    };
    let (mut lo, mut hi) = (2.0e3, p0);
    assert!(
        in_dome(lo) && !in_dome(hi),
        "isentrope must cross the bubble line in [2 kPa, p0]"
    );
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if in_dome(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let p_bubble = hi;
    let u_liquid = riemann_integral(p_bubble, p0, s0);
    let u_bubble = -u0 + u_liquid;
    assert!(
        u_bubble < 0.0,
        "u0 = {u0} is too small to reach the dome (liquid branch gives {u_liquid})"
    );
    // p* < p_b with u(p*) = 0.
    let f = |p: f64| riemann_integral(p, p_bubble, s0) + u_bubble;
    let p_star = BracketingSolver::new(BracketingMethod::Brent, f, 1.0e3, lo)
        .expect("p* bracket")
        .solve(0.0, 1e-10, 200)
        .expect("p*");
    let (rho_star, c_star) = rho_c(p_star, s0);
    let c0 = rho_c(p0, s0).1;
    let c_lb = rho_c(p_bubble, s0).1;
    let c_db = rho_c(lo, s0).1;
    DoubleRarefaction {
        p0,
        t0,
        u0,
        s0,
        p_bubble,
        u_bubble,
        c_liquid: (c0, c_lb),
        c_dome_at_bubble: c_db,
        p_star,
        c_star,
        rho_star,
    }
}

/// Exact pressure and velocity at `ξ = x/t` (`x` from the centre), for the
/// LEFT half; the right half is the mirror image (`p(−ξ) = p(ξ)`,
/// `u(−ξ) = −u(ξ)`). The fans are inverted by bisection on
/// `λ(p) = u(p) − c(p) = ξ`; `λ` is checked monotone by the caller.
pub fn sample_left(sol: &DoubleRarefaction, xi: f64) -> (f64, f64) {
    let lam = |p: f64| {
        let u = if p >= sol.p_bubble {
            -sol.u0 + riemann_integral(p, sol.p0, sol.s0)
        } else {
            sol.u_bubble + riemann_integral(p, sol.p_bubble, sol.s0)
        };
        (u - rho_c(p, sol.s0).1, u)
    };
    let head = -sol.u0 - sol.c_liquid.0;
    let liquid_tail = sol.u_bubble - sol.c_liquid.1;
    let dome_head = sol.u_bubble - sol.c_dome_at_bubble;
    let centre = -sol.c_star;
    let invert = |lo: f64, hi: f64| {
        let (mut lo, mut hi) = (lo, hi); // pressures, lam decreasing in p
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if lam(mid).0 > xi {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let p = 0.5 * (lo + hi);
        (p, lam(p).1)
    };
    if xi <= head {
        (sol.p0, -sol.u0)
    } else if xi <= liquid_tail {
        invert(sol.p_bubble * (1.0 + 1e-12), sol.p0)
    } else if xi <= dome_head {
        (sol.p_bubble, sol.u_bubble)
    } else if xi <= centre {
        invert(sol.p_star, sol.p_bubble * (1.0 - 1e-12))
    } else {
        (sol.p_star, 0.0)
    }
}
