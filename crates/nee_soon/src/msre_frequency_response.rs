//! **MSRE power-to-reactivity frequency response (MSRE T-1, gh:#1132).**
//!
//! > Research, education and V&V only. AI draft (Claude Opus 5.5 in Claude
//! > Code), unreviewed. Not for facility operation.
//!
//! A linear model of the whole MSRE heat-transport chain, solved at each
//! frequency as one complex linear system (`petir::depletion::solve_complex`).
//! The pieces, with page numbers from the ORNL-TM-1070 scan (Ball and
//! Kerlin, 1965):
//!
//! - **Kinetics:** circulating-fuel point kinetics, six groups. The
//!   precursors leave the core in a well-stirred core of transit time τ_c
//!   and return after the loop transit τ_L, decayed. This is the equation set
//!   of `teh_o_prke::zero_power_prke::circulating` (T-C1, gh:#1131),
//!   linearised; it is TM-1070 eq. (A.29)/(A.30) in reactivity form.
//!   - λ_i and β'_i from App. B p. 69; Λ, τ_c and τ_L from Table 1 "present
//!     study".
//!   - The steady-state reactivity ρ₀ is the circulation loss (A.31).
//! - **Core:** single region, two fuel lumps and a graphite lump, eqs.
//!   (A.1)-(A.3) p. 49, with App. B's "1965 extrema determination"
//!   coefficients (pp. 67-68). Reactivity follows the first fuel lump and the
//!   graphite (importances 1, 0, 1).
//!   - The delayed-gamma share of the power, 0.0564, arrives through a
//!     188 s first-order lag (Table 1).
//! - **Heat exchanger and radiator:** two lumps per fluid plus the tube
//!   (and, in the heat exchanger, the shell), eqs. (A.32)-(A.37) pp. 58-60,
//!   with App. B p. 69's 1965 coefficients at 10 MW.
//!   - The radiator air side scales with power as ORNL scaled it (p. 60):
//!     W_air ∝ P, h_air ∝ W_air^0.6, so n_air ∝ P^-0.4, τ_tube→air ∝ P^-0.6
//!     and t*_air ∝ P^-1. The air inlet temperature does not change.
//! - **Piping:** pure transport delays (plug flow), App. B p. 70's 1965 lags:
//!   core → HX 5.77 s, HX → core 8.67 s, HX → radiator 4.71 s,
//!   radiator → HX 8.24 s.
//!   - TM-1070 p. 16 calls plug flow "probably superior" (Re ≈ 240 000 and
//!     120 000) to the reduced model's first-order lags. Its complete model
//!     used pure delays.
//!
//! **The fuel and graphite temperature coefficients are inputs.** For T-1
//! they come from this workspace's own Monte Carlo model (N-3, gh:#1129),
//! not from TM-1070's −4.84 and −3.7 × 10⁻⁵ /°F. Those are kept as
//! [`ORNL_1965_FUEL_PER_F`] and [`ORNL_1965_GRAPHITE_PER_F`] for the
//! code-to-code check against ORNL's own predicted curve.
//!
//! Everything inside is in ORNL's units (MW, °F, s); the public API is `uom`.

use petir::depletion::cram::solve_complex;
use petir::depletion::Complex64;
use petir::mathf::RealMath;
use uom::si::angular_velocity::radian_per_second;
use uom::si::f64::{AngularVelocity, Power, TemperatureCoefficient, Time};
use uom::si::power::megawatt;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::time::second;

/// TM-1070 Table 1 "present study" fuel coefficient, −4.84 × 10⁻⁵ /°F.
pub const ORNL_1965_FUEL_PER_F: f64 = -4.84e-5;
/// TM-1070 Table 1 "present study" graphite coefficient, −3.7 × 10⁻⁵ /°F.
pub const ORNL_1965_GRAPHITE_PER_F: f64 = -3.7e-5;

/// TM-1647 (Kerlin, Ball and Steffy, 1966) Fig. 29: the **measured**
/// natural periods of oscillation, `(power MW, period min)`.
///
/// **Digitised 2026-11-09, a processing step.** The scan's PDF page 46 was
/// rendered at 300 dpi in grey. Both log axes were calibrated on the
/// gridlines:
/// - x: 0.02 MW at px 428 and 10 MW at px 2086; the 0.1 and 1 MW lines fall
///   within 2 px of where the calibration puts them;
/// - y: 1 min at px 2085 and 50 min at px 1020; the 2, 5, 10 and 20 min
///   lines fall within 5 px.
///
/// Each value is the centroid of the dot's dense core (pixels whose 13 × 13
/// neighbourhood is ≥ 95 % dark). The dot radius, ~13 px, makes each reading
/// good to about ±2 % in both power and period. The two highest-power dots
/// touch, and were separated with a 10 px search radius.
pub const MEASURED_PERIODS_TM1647: [(f64, f64); 7] = [
    (0.076, 29.91),
    (0.467, 11.01),
    (1.004, 7.16),
    (2.448, 4.33),
    (4.848, 2.41),
    (6.735, 1.85),
    (7.306, 1.71),
];

/// Decay constants λ_i \[1/s\], TM-1070 App. B p. 69.
pub const LAMBDA: [f64; 6] = [3.01, 1.14, 0.301, 0.111, 0.0305, 0.0124];
/// Delayed-neutron fractions β'_i (total, "1965 frequency-response"), App. B
/// p. 69. They sum to 0.006661, Table 1's 0.00666.
pub const BETA: [f64; 6] = [0.00028, 0.000766, 0.002628, 0.001307, 0.001457, 0.000223];

/// A coefficient from ORNL's per-°F to `uom`.
pub fn per_f(alpha_per_f: f64) -> TemperatureCoefficient {
    TemperatureCoefficient::new::<per_kelvin>(alpha_per_f * 1.8)
}

/// One counterflow heat-exchange section: two lumps per fluid, a tube and
/// an optional shell (TM-1070 eqs. A.32-A.37). Fluid 1 is inside the tube.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HxSection {
    /// n₁ = hA/(W c_p) on fluid 1.
    pub n1: f64,
    /// n₂ on fluid 2.
    pub n2: f64,
    /// n_s, fluid 2 to the shell (0: no shell).
    pub ns: f64,
    /// Fluid 1 transit time \[s\].
    pub t1: f64,
    /// Fluid 2 transit time \[s\].
    pub t2: f64,
    /// Tube time constant to fluid 1 \[s\].
    pub tau_t1: f64,
    /// Tube time constant to fluid 2 \[s\].
    pub tau_t2: f64,
    /// Shell time constant \[s\] (unused when `ns == 0`).
    pub tau_s: f64,
}

/// The 1965 heat exchanger (fluid 1 coolant, fluid 2 fuel), App. B p. 69.
pub const HX_1965: HxSection =
    HxSection { n1: 1.10, n2: 1.366, ns: 0.1363, t1: 2.01, t2: 2.29, tau_t1: 0.569, tau_t2: 0.304, tau_s: 1.14 };
/// The 1965 radiator at 10 MW (fluid 1 coolant, fluid 2 air), App. B p. 69.
pub const RADIATOR_1965_10MW: HxSection =
    HxSection { n1: 0.8803, n2: 0.2591, ns: 0.0, t1: 6.52, t2: 0.01, tau_t1: 2.35, tau_t2: 19.7, tau_s: 1.0 };

impl HxSection {
    /// The radiator at `p_mw`, its air side scaled from 10 MW as TM-1070
    /// p. 60 does: W_air ∝ P and h_air ∝ W_air^0.6.
    pub fn radiator_at(p_mw: f64) -> Self {
        let w = p_mw / 10.0;
        let r = RADIATOR_1965_10MW;
        HxSection { n2: r.n2 * w.r_powf(-0.4), t2: r.t2 / w, tau_t2: r.tau_t2 * w.r_powf(-0.6), ..r }
    }
}

/// The MSRE loop model at one power level.
#[derive(Debug, Clone, PartialEq)]
pub struct MsreLoopModel {
    /// Fuel temperature coefficient \[1/°F\].
    pub alpha_fuel: f64,
    /// Graphite temperature coefficient \[1/°F\].
    pub alpha_graphite: f64,
    /// Steady power \[MW\].
    pub power_mw: f64,
    /// Prompt generation time Λ \[s\] (Table 1: 0.00024).
    pub generation_time: f64,
    /// Core transit τ_c \[s\] (Table 1: 8.46).
    pub core_transit: f64,
    /// External loop transit τ_L \[s\] (Table 1: 16.73).
    pub loop_transit: f64,
    /// Heat exchanger.
    pub hx: HxSection,
    /// Radiator at this power.
    pub radiator: HxSection,
    /// Piping delays \[s\]: core→HX, HX→core, HX→radiator, radiator→HX.
    pub delays: [f64; 4],
    /// Whether the external loops respond (false: core alone, inlet held).
    pub loops: bool,
}

// Single-region core coefficients, App. B pp. 67-68 ("1965 extrema").
// Made `pub` 2026-11-10 (gh:#1330) so the engine's time-domain MSRE model
// (`outram_park_digital_twin_engine::msre`) reads these values, not a copy.
/// Fuel-lump residence time τ_F \[s\] (each of the two lumps), App. B pp. 67-68.
pub const TAU_F: f64 = 4.23;
/// Fraction of the fission power deposited in each fuel lump, K_F.
pub const K_F: f64 = 0.47;
/// Fraction of the fission power deposited in each graphite half, K_G
/// (the graphite lump takes 2 K_G).
pub const K_G: f64 = 0.03;
/// Heat capacity of each fuel lump, MC_F \[MW·s/°F\].
pub const MC_F: f64 = 0.750;
/// Heat capacity of the graphite lump, MC_G \[MW·s/°F\].
pub const MC_G: f64 = 3.58;
/// Graphite-to-fuel conductance, hA \[MW/°F\].
pub const HA: f64 = 0.020;
/// Delayed-gamma share of the power (Table 1).
pub const GAMMA_SHARE: f64 = 0.0564;
/// Delayed-gamma first-order lag \[s\] (Table 1).
pub const GAMMA_LAG: f64 = 188.0;

impl MsreLoopModel {
    /// TM-1070's 1965 parameters at `power`, with the given coefficients.
    pub fn tm1070(power: Power, fuel: TemperatureCoefficient, graphite: TemperatureCoefficient) -> Self {
        let p = power.get::<megawatt>();
        MsreLoopModel {
            alpha_fuel: fuel.get::<per_kelvin>() / 1.8,
            alpha_graphite: graphite.get::<per_kelvin>() / 1.8,
            power_mw: p,
            generation_time: 0.00024,
            core_transit: 8.46,
            loop_transit: 16.73,
            hx: HX_1965,
            radiator: HxSection::radiator_at(p),
            delays: [5.77, 8.67, 4.71, 8.24],
            loops: true,
        }
    }

    /// The steady reactivity of the circulating core, eq. (A.31) in
    /// reactivity form: β − Σ λ_i β_i / (λ_i + (1 − e^{−λ_i τ_L})/τ_c).
    pub fn circulation_loss(&self) -> f64 {
        let beta: f64 = BETA.iter().sum();
        beta - (0..6)
            .map(|i| LAMBDA[i] * BETA[i] / (LAMBDA[i] + (1.0 - (-LAMBDA[i] * self.loop_transit).r_exp()) / self.core_transit))
            .sum::<f64>()
    }

    /// δP/P₀ per unit δρ (δk/k) of external reactivity at `omega` \[rad/s\].
    pub fn response(&self, omega: f64) -> Complex64 {
        let n = 26;
        let s = Complex64::new(0.0, omega);
        let one = Complex64::from_real(1.0);
        let mut m = vec![vec![Complex64::from_real(0.0); n]; n];
        let mut b = vec![Complex64::from_real(0.0); n];
        let (lam, tc, tl) = (self.generation_time, self.core_transit, self.loop_transit);
        let beta: f64 = BETA.iter().sum();
        let rho0 = self.circulation_loss();
        // Unknowns.
        const P: usize = 0; // δP/P0
        const Q: usize = 1; // 1..=6 normalised precursors
        const PD: usize = 7; // delayed-gamma power, normalised
        const F1: usize = 8;
        const F2: usize = 9;
        const G: usize = 10;
        const HX_FUEL_IN: usize = 11;
        const HX: usize = 12; // 12..=17: T1, T1out, TT, T2, T2out, Ts
        const CORE_IN: usize = 18;
        const RAD_IN: usize = 19;
        const RAD: usize = 20; // 20..=24: T1, T1out, TT, T2, T2out
        const HX_COOL_IN: usize = 25;
        let p0 = self.power_mw;
        // Kinetics: s p − ((ρ0 − β)/Λ) p − Σ λ q − (α_F F1 + α_G G)/Λ = δρ/Λ.
        m[P][P] = s.sub(Complex64::from_real((rho0 - beta) / lam));
        for i in 0..6 {
            m[P][Q + i] = Complex64::from_real(-LAMBDA[i]);
        }
        m[P][F1] = Complex64::from_real(-self.alpha_fuel / lam);
        m[P][G] = Complex64::from_real(-self.alpha_graphite / lam);
        b[P] = Complex64::from_real(1.0 / lam);
        for i in 0..6 {
            // s q − β p/Λ + (λ + 1/τc) q − q e^{−(s+λ)τL}/τc = 0
            let ret = exp_neg(s, tl).mul(Complex64::from_real((-LAMBDA[i] * tl).r_exp() / tc));
            m[Q + i][Q + i] = s.sub(Complex64::from_real(-(LAMBDA[i] + 1.0 / tc))).sub(ret);
            m[Q + i][P] = Complex64::from_real(-BETA[i] / lam);
        }
        // Delayed gamma: (1 + τ s) pd = p.
        m[PD][PD] = one.sub(Complex64::from_real(-GAMMA_LAG).mul(s));
        m[PD][P] = Complex64::from_real(-1.0);
        // Heating power perturbation [MW] = P0((1−γ) p + γ pd).
        let heat = |row: &mut Vec<Complex64>, frac: f64, cap: f64| {
            row[P] = Complex64::from_real(-p0 * (1.0 - GAMMA_SHARE) * frac / cap);
            row[PD] = Complex64::from_real(-p0 * GAMMA_SHARE * frac / cap);
        };
        let kg_share = 0.5; // K_G1 / (K_G1 + K_G2)
        // (A.1): s F1 = (Tin − F1)/τ + K P/MC + share·hA/MC (G − F1)
        m[F1][F1] = s.sub(Complex64::from_real(-(1.0 / TAU_F + kg_share * HA / MC_F)));
        m[F1][CORE_IN] = Complex64::from_real(-1.0 / TAU_F);
        m[F1][G] = Complex64::from_real(-kg_share * HA / MC_F);
        heat(&mut m[F1], K_F, MC_F);
        // (A.2): s F2 = (F1 − F2)/τ + K P/MC + share·hA/MC (G − F1)
        m[F2][F2] = s.sub(Complex64::from_real(-1.0 / TAU_F));
        m[F2][F1] = Complex64::from_real(-1.0 / TAU_F + kg_share * HA / MC_F);
        m[F2][G] = Complex64::from_real(-kg_share * HA / MC_F);
        heat(&mut m[F2], K_F, MC_F);
        // (A.3): s G = hA/MC_G (F1 − G) + 2 K_G P / MC_G
        m[G][G] = s.sub(Complex64::from_real(-HA / MC_G));
        m[G][F1] = Complex64::from_real(-HA / MC_G);
        heat(&mut m[G], 2.0 * K_G, MC_G);
        if self.loops {
            delay(&mut m, HX_FUEL_IN, F2, s, self.delays[0]);
            hx_rows(&mut m, HX, &self.hx, s, HX_COOL_IN, Some(HX_FUEL_IN));
            delay(&mut m, CORE_IN, HX + 4, s, self.delays[1]);
            delay(&mut m, RAD_IN, HX + 1, s, self.delays[2]);
            hx_rows(&mut m, RAD, &self.radiator, s, RAD_IN, None);
            delay(&mut m, HX_COOL_IN, RAD + 1, s, self.delays[3]);
        } else {
            // Core alone: inlet held; every loop unknown pinned to zero.
            for k in HX_FUEL_IN..n {
                m[k][k] = one;
            }
        }
        let x = solve_complex(m, b).expect("MSRE loop system is non-singular");
        x[P]
    }

    /// The natural period of oscillation: 2π/ω at the peak of |δP/δρ| on a
    /// logarithmic grid of `points` over 10⁻⁴ to 1 rad/s, refined by golden
    /// section to 10⁻⁶ relative. `None` when the peak sits at an edge of the
    /// grid (no resonance inside it).
    pub fn natural_period(&self, points: usize) -> Option<Time> {
        let (lo, hi) = (-4.0_f64, 0.0_f64);
        let w_at = |k: usize| 10f64.r_powf(lo + (hi - lo) * k as f64 / (points - 1) as f64);
        let amp = |w: f64| self.response(w).norm_sqr();
        let (mut best, mut best_k) = (f64::MIN, 0);
        for k in 0..points {
            let a = amp(w_at(k));
            if a > best {
                best = a;
                best_k = k;
            }
        }
        if best_k == 0 || best_k == points - 1 {
            return None;
        }
        let (mut a, mut b) = (w_at(best_k - 1).r_ln(), w_at(best_k + 1).r_ln());
        let g = 0.5 * (5f64.sqrt() - 1.0);
        while (b - a) > 1e-6 {
            let (c, d) = (b - g * (b - a), a + g * (b - a));
            if amp(c.r_exp()) > amp(d.r_exp()) {
                b = d;
            } else {
                a = c;
            }
        }
        let w = (0.5 * (a + b)).r_exp();
        Some(Time::new::<second>(2.0 * core::f64::consts::PI / w))
    }

    /// `|δP/P₀ / δρ|` and phase \[deg\] at `omega`.
    pub fn amplitude_phase(&self, omega: AngularVelocity) -> (f64, f64) {
        let r = self.response(omega.get::<radian_per_second>());
        (r.norm_sqr().sqrt(), r.im.r_atan2(r.re).to_degrees())
    }
}

fn exp_neg(s: Complex64, tau: f64) -> Complex64 {
    // e^{−sτ} for s = jω: cos(ωτ) − j sin(ωτ) (s is purely imaginary here).
    let ph = s.im * tau;
    Complex64::new(ph.r_cos() * (-s.re * tau).r_exp(), -ph.r_sin() * (-s.re * tau).r_exp())
}

/// Row `out`: out = e^{−sτ} input.
fn delay(m: &mut [Vec<Complex64>], out: usize, input: usize, s: Complex64, tau: f64) {
    m[out][out] = Complex64::from_real(1.0);
    let e = exp_neg(s, tau);
    m[out][input] = Complex64::new(-e.re, -e.im);
}

/// Rows `base..` for one section, eqs. (A.32)-(A.37): T1, T1out, TT, T2,
/// T2out, and Ts when `ns > 0`. `in1` is fluid 1's inlet; `in2` is fluid 2's,
/// or `None` for a held inlet (the radiator's air).
fn hx_rows(m: &mut [Vec<Complex64>], base: usize, h: &HxSection, s: Complex64, in1: usize, in2: Option<usize>) {
    let r = |v: f64| Complex64::from_real(v);
    let (t1, t1o, tt, t2, t2o, ts) = (base, base + 1, base + 2, base + 3, base + 4, base + 5);
    // (A.32) s T1 = 2/t1 (T1in − T1) + n1/t1 (TT − T1)
    m[t1][t1] = s.sub(r(-(2.0 + h.n1) / h.t1));
    m[t1][in1] = r(-2.0 / h.t1);
    m[t1][tt] = r(-h.n1 / h.t1);
    // (A.33) s T1out = 2/t1 (T1 − T1out) + n1/t1 (TT − T1)
    m[t1o][t1o] = s.sub(r(-2.0 / h.t1));
    m[t1o][t1] = r(-2.0 / h.t1 + h.n1 / h.t1);
    m[t1o][tt] = r(-h.n1 / h.t1);
    // (A.34) s TT = (T1 − TT)/τT1 + (T2 − TT)/τT2
    m[tt][tt] = s.sub(r(-(1.0 / h.tau_t1 + 1.0 / h.tau_t2)));
    m[tt][t1] = r(-1.0 / h.tau_t1);
    m[tt][t2] = r(-1.0 / h.tau_t2);
    // (A.35) s T2 = 2/t2 (T2in − T2) + n2/t2 (TT − T2) + ns/t2 (Ts − T2)
    m[t2][t2] = s.sub(r(-(2.0 + h.n2 + h.ns) / h.t2));
    if let Some(i2) = in2 {
        m[t2][i2] = r(-2.0 / h.t2);
    }
    m[t2][tt] = r(-h.n2 / h.t2);
    // (A.36) s T2out = 2/t2 (T2 − T2out) + n2/t2 (TT − T2) + ns/t2 (Ts − T2)
    m[t2o][t2o] = s.sub(r(-2.0 / h.t2));
    m[t2o][t2] = r(-2.0 / h.t2 + (h.n2 + h.ns) / h.t2);
    m[t2o][tt] = r(-h.n2 / h.t2);
    if h.ns > 0.0 {
        m[t2][ts] = r(-h.ns / h.t2);
        m[t2o][ts] = r(-h.ns / h.t2);
        // (A.37) s Ts = (T2 − Ts)/τs
        m[ts][ts] = s.sub(r(-1.0 / h.tau_s));
        m[ts][t2] = r(-1.0 / h.tau_s);
    } else {
        m[ts][ts] = r(1.0); // no shell: pinned to zero
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornl(p_mw: f64) -> MsreLoopModel {
        MsreLoopModel::tm1070(
            Power::new::<megawatt>(p_mw),
            per_f(ORNL_1965_FUEL_PER_F),
            per_f(ORNL_1965_GRAPHITE_PER_F),
        )
    }

    /// With the coefficients zeroed, the response is the zero-power
    /// circulating transfer function in closed form,
    /// `1/(sΛ + β − ρ₀ − Σ λβ_i/(s + λ_i + (1 − e^{−(s+λ_i)τ_L})/τ_c))`
    /// (TM-1070 eq. A.30 in reactivity form), to 1e-10 relative at five
    /// frequencies.
    ///
    /// **A finding, not a criterion:** with TM-1070 Table 1's transits
    /// (τ_c 8.46 s, τ_L 16.73 s), the well-stirred circulation loss ρ₀ is
    /// **0.2467 % δk/k** (2026-11-09), +16 % against the measured 0.212 %
    /// (TM-1647 Table 1). A first version of this test asserted agreement
    /// "within a few percent", a guess that was never checked, and failed;
    /// the guess was removed, not loosened. N-4 (gh:#1130) records the same
    /// lumped model at 0.2301 % with TM-1626's transits. Here ρ₀ only sets
    /// the operating point of the kinetics, and the loss is printed.
    #[test]
    fn zero_power_limit_is_the_closed_form_circulating_transfer_function() {
        let mut m = ornl(1.0);
        m.alpha_fuel = 0.0;
        m.alpha_graphite = 0.0;
        let beta: f64 = BETA.iter().sum();
        let rho0 = m.circulation_loss();
        println!("rho0 = {:.5} % dk/k (beta {beta:.5}; measured 0.212 %)", 100.0 * rho0);
        assert!(rho0 > 0.0 && rho0 < beta);
        for w in [1e-3, 1e-2, 0.1, 1.0, 10.0] {
            let s = Complex64::new(0.0, w);
            let mut den = Complex64::new(beta - rho0, w * m.generation_time);
            for i in 0..6 {
                let e = exp_neg(Complex64::new(0.0, w), m.loop_transit).mul(Complex64::from_real((-LAMBDA[i] * m.loop_transit).r_exp()));
                let d = s.sub(Complex64::from_real(-(LAMBDA[i] + 1.0 / m.core_transit))).sub(e.mul(Complex64::from_real(1.0 / m.core_transit)));
                den = den.sub(Complex64::from_real(LAMBDA[i] * BETA[i]).div(d));
            }
            let exact = Complex64::from_real(1.0).div(den);
            let got = m.response(w);
            let rel = (got.sub(exact).norm_sqr() / exact.norm_sqr()).sqrt();
            assert!(rel < 1e-10, "w {w}: rel {rel:e}");
        }
    }

    /// One heat-exchange section solved as lumps reproduces TM-1070's closed
    /// form (A.38) for T1,out/T1,in, with A, B, F as defined on pp. 61-62,
    /// at four frequencies, to 1e-10 relative.
    #[test]
    fn a_section_reproduces_tm1070_closed_form_a38() {
        let h = HX_1965;
        for w in [1e-3, 0.03, 0.3, 3.0] {
            let s = Complex64::new(0.0, w);
            let r = |v: f64| Complex64::from_real(v);
            // Lumps: unknowns 0..6 = T1, T1out, TT, T2, T2out, Ts; T1in = 1 at index 6.
            let mut m = vec![vec![r(0.0); 7]; 7];
            hx_rows(&mut m, 0, &h, s, 6, None);
            m[6][6] = r(1.0);
            let mut b = vec![r(0.0); 7];
            b[6] = r(1.0);
            let x = solve_complex(m, b).expect("solve");
            // Closed form (A.38) with fluid 2's inlet held.
            let ns_term = r(h.ns).div(r(1.0).sub(r(-h.tau_s).mul(s)));
            let a = r(h.n2).div(r(h.t2).mul(s).sub(r(-(h.n2 + 2.0 + h.ns))).sub(ns_term));
            let bb = r(1.0).div(r(h.tau_t1).mul(s).sub(r(-(1.0 + h.tau_t1 / h.tau_t2))).sub(r(h.tau_t1 / h.tau_t2).mul(a)));
            let f = r(2.0).div(r(h.t1).mul(s).sub(r(-(h.n1 + 2.0))).sub(r(h.n1).mul(bb)));
            let exact = r(2.0 - h.n1).sub(r(-h.n1).mul(bb)).mul(f).div(r(h.t1).mul(s).sub(r(-2.0)));
            let rel = (x[1].sub(exact).norm_sqr() / exact.norm_sqr()).sqrt();
            assert!(rel < 1e-10, "w {w}: rel {rel:e}");
        }
    }

    /// **ORNL's own coefficients** (TM-1070 Table 1: fuel −4.84, graphite
    /// −3.7 × 10⁻⁵ /°F) in this model, at each measured power of TM-1647
    /// Fig. 29. This is not T-1's result (T-1 takes N-3's coefficients); it
    /// shows what the model structure gives with ORNL's inputs, beside the
    /// measurement. Each period must be within a factor of 1.5 of the
    /// measured one, a structural check set before running; it says nothing
    /// finer.
    ///
    /// **Result (2026-11-09):** see the printed table, which is recorded in
    /// `verification_and_validation/msre_t1_frequency_response/README.md`.
    #[test]
    fn ornl_coefficients_against_the_measured_periods() {
        println!("   P [MW]  measured [min]  model, ORNL coefficients [min]  ratio");
        for (p, t_meas) in MEASURED_PERIODS_TM1647 {
            let t = ornl(p).natural_period(600).expect("resonance").get::<second>() / 60.0;
            println!("  {p:7.3}  {t_meas:14.2}  {t:30.2}  {:5.3}", t / t_meas);
            assert!(t / t_meas < 1.5 && t_meas / t < 1.5, "{p} MW: {t} vs {t_meas}");
        }
    }

    /// The loops close: at 1 MW the response peaks inside the grid, and the
    /// natural period falls as power rises (TM-1647's qualitative finding).
    #[test]
    fn the_natural_period_falls_with_power() {
        let periods: Vec<f64> = [0.075, 1.0, 7.5]
            .iter()
            .map(|&p| ornl(p).natural_period(400).expect("a resonance").get::<second>() / 60.0)
            .collect();
        println!("ORNL-1965 coefficients: natural periods {periods:?} min at 0.075, 1, 7.5 MW");
        assert!(periods[0] > periods[1] && periods[1] > periods[2]);
    }
}
