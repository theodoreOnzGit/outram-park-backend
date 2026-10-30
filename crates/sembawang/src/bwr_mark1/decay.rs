//! Whole-core decay heat after the scram (gh:#1041, plan item F10).
//!
//! The form is that of the ANS-5.1-1979 standard, as MELCOR's DCH package
//! applies it (MELCOR 2.2 Vol. 2, SAND2023-10994 O, DCH §3.2, Eqs 3-1 to 3-5
//! and Table 3.1; Vol. 1, SAND2017-0455 O, DCH_FPW, DCH_OPT, DCH_NFA, 3201,
//! 3205). No MELCOR source is used:
//!
//! ```text
//! P(t) = G(t) sum_i P_FP,i(t) + (sum_i P_i / Q_i) [F_U239(t, T) + F_Np239(t, T)]
//! ```
//!
//! - `P_FP,i` is the fission-product decay heat of fissioning nuclide `i`
//!   (U-235 thermal, Pu-239 thermal, U-238 fast) after `T` seconds at its
//!   share `P_i` of the rated power. It comes from `teh-o-prke`'s 23-group
//!   fit, the 1978 draft ANS standard (reused, not re-entered), integrated
//!   analytically over the irradiation. MELCOR interpolates the 1979
//!   standard's tables instead; the two rest on the same England et al.
//!   evaluation, and the difference is not measured here.
//! - `G(t)` is the neutron-capture correction. Below 1e4 s it is the
//!   standard's formula (Eq 3-5). Above, it is the standard's Table 10,
//!   `G_max(t)`, which is an **upper bound**, i.e. conservative, as MELCOR
//!   uses it. The step at 1e4 s is the standard's own.
//! - `F_U239`, `F_Np239` are the decay heat of U-239 and Np-239 (Eqs 3-3,
//!   3-4).
//!
//! Each part is a [`Mechanisms`](super::Mechanisms) switch, default on. The
//! ablation of the first is the model before gh:#1041: U-235 only, at
//! equilibrium (infinite irradiation).

use super::params::Params;
use petir::mathf::RealMath;
use teh_o_prke::decay_heat::{DecayHeat, FissioningNuclide};
use uom::si::energy::megaelectronvolt;
use uom::si::f64::{Energy, Power, Time};
use uom::si::power::watt;
use uom::si::time::second;

/// The ANS-5.1-1979 neutron-capture upper bound `G_max(t)`, its Table 10, as
/// MELCOR 2.2 Vol. 2 DCH Table 3.1 reproduces it: (time after shutdown s,
/// factor), from 1e4 s. \[P\]
const G_MAX: [(f64, f64); 31] = [
    (1.0e4, 1.064),
    (1.5e4, 1.074),
    (2.0e4, 1.081),
    (4.0e4, 1.098),
    (6.0e4, 1.111),
    (8.0e4, 1.119),
    (1.0e5, 1.124),
    (1.5e5, 1.130),
    (2.0e5, 1.131),
    (4.0e5, 1.126),
    (6.0e5, 1.124),
    (8.0e5, 1.123),
    (1.0e6, 1.124),
    (1.5e6, 1.125),
    (2.0e6, 1.127),
    (4.0e6, 1.134),
    (6.0e6, 1.146),
    (8.0e6, 1.162),
    (1.0e7, 1.181),
    (1.5e7, 1.233),
    (2.0e7, 1.284),
    (4.0e7, 1.444),
    (6.0e7, 1.535),
    (8.0e7, 1.586),
    (1.0e8, 1.598),
    (1.5e8, 1.498),
    (2.0e8, 1.343),
    (4.0e8, 1.065),
    (6.0e8, 1.021),
    (8.0e8, 1.012),
    (1.0e9, 1.007),
];

/// Which parts of the decay heat are on (from `Mechanisms`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecayParts {
    /// U-235, Pu-239 and U-238 fission products after a finite irradiation;
    /// off is U-235 alone at equilibrium.
    pub mixed_finite: bool,
    /// The capture correction `G(t)`.
    pub capture: bool,
    /// U-239 and Np-239.
    pub actinides: bool,
}

/// The core's decay heat source, advanced with the plant.
#[derive(Clone, Debug)]
pub struct PlantDecay {
    fp: Vec<DecayHeat>,
    /// Fission rate at shutdown, fissions/s: `sum_i P_i / Q_i`.
    fission_rate: f64,
    t_s: f64,
    parts: DecayParts,
    prm: Params,
}

const JOULE_PER_MEV: f64 = 1.602_176_634e-13;

impl PlantDecay {
    /// The decay source just after the scram from rated power.
    pub fn new(prm: &Params, parts: DecayParts) -> Self {
        let p0 = prm.thermal_power_w;
        let (fp, fission_rate) = if parts.mixed_finite {
            let shares = [
                (FissioningNuclide::U235Thermal, prm.fission_share_u235, prm.q_u235_mev),
                (FissioningNuclide::Pu239Thermal, prm.fission_share_pu239, prm.q_pu239_mev),
                (FissioningNuclide::U238Fast, 1.0 - prm.fission_share_u235 - prm.fission_share_pu239, prm.q_u238_mev),
            ];
            let mut fp = Vec::new();
            let mut rate = 0.0;
            for (nuclide, share, q) in shares {
                let mut d = DecayHeat::new(nuclide).with_energy_per_fission(Energy::new::<megaelectronvolt>(q));
                // The irradiation, integrated analytically in one step.
                d.advance_timestep(Power::new::<watt>(share * p0), Time::new::<second>(prm.operating_time_s));
                fp.push(d);
                rate += share * p0 / (q * JOULE_PER_MEV);
            }
            (fp, rate)
        } else {
            let d = DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, Power::new::<watt>(p0));
            (vec![d], p0 / (prm.q_u235_mev * JOULE_PER_MEV))
        };
        Self { fp, fission_rate, t_s: 0.0, parts, prm: *prm }
    }

    /// Advance by `dt_s` seconds with no fission.
    pub fn advance(&mut self, dt_s: f64) {
        for d in &mut self.fp {
            d.advance_timestep(Power::new::<watt>(0.0), Time::new::<second>(dt_s));
        }
        self.t_s += dt_s;
    }

    /// Time since the scram, s.
    pub fn t_s(&self) -> f64 {
        self.t_s
    }

    /// The fission products' part of the decay power now, W: the power
    /// less U-239 and Np-239.
    pub fn fp_power_w(&self) -> f64 {
        let fp: f64 = self.fp.iter().map(|d| d.total_decay_heat_power().get::<watt>()).sum();
        let g = if self.parts.capture { self.capture_factor() } else { 1.0 };
        g * fp
    }

    /// Decay power now, W.
    pub fn power_w(&self) -> f64 {
        let fp: f64 = self.fp.iter().map(|d| d.total_decay_heat_power().get::<watt>()).sum();
        let g = if self.parts.capture { self.capture_factor() } else { 1.0 };
        let act = if self.parts.actinides { self.actinide_power_w() } else { 0.0 };
        g * fp + act
    }

    /// `G(t)`: the standard's formula below 1e4 s (MELCOR Eq 3-5), then
    /// `G_max` interpolated logarithmically in time, as MELCOR does.
    pub fn capture_factor(&self) -> f64 {
        let t = self.t_s;
        if t < 1.0e4 {
            let p = &self.prm;
            return 1.0 + (3.24e-6 + 5.23e-10 * t) * p.operating_time_s.r_powf(0.4) * p.fissions_per_fissile_atom;
        }
        let i = G_MAX.iter().rposition(|(ti, _)| *ti <= t).unwrap_or(0).min(G_MAX.len() - 2);
        let ((t0, g0), (t1, g1)) = (G_MAX[i], G_MAX[i + 1]);
        let w = ((t / t0).r_ln() / (t1 / t0).r_ln()).clamp(0.0, 1.0);
        g0 + w * (g1 - g0)
    }

    /// U-239 and Np-239 decay heat, W (MELCOR Eqs 3-2 to 3-4).
    pub fn actinide_power_w(&self) -> f64 {
        let p = &self.prm;
        let (t, op, l1, l2) = (self.t_s, p.operating_time_s, p.u239_decay_per_s, p.np239_decay_per_s);
        let u239 = p.e_u239_mev * p.u239_per_fission * (1.0 - (-l1 * op).r_exp()) * (-l1 * t).r_exp();
        let np239 = p.e_np239_mev
            * p.u239_per_fission
            * (l1 * (1.0 - (-l2 * op).r_exp()) * (-l2 * t).r_exp() / (l1 - l2)
                - l2 * (1.0 - (-l1 * op).r_exp()) * (-l1 * t).r_exp() / (l1 - l2));
        self.fission_rate * (u239 + np239) * JOULE_PER_MEV
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: DecayParts = DecayParts { mixed_finite: true, capture: true, actinides: true };
    const OLD: DecayParts = DecayParts { mixed_finite: false, capture: false, actinides: false };

    fn at(parts: DecayParts, t: f64) -> PlantDecay {
        let mut d = PlantDecay::new(&Params::default(), parts);
        let mut done = 0.0;
        while done < t {
            let dt = (t - done).min(10.0);
            d.advance(dt);
            done += dt;
        }
        d
    }

    /// The parts against the standard's own numbers (gh:#1041):
    /// - `G(t)` is 1 plus about 2.8e-3 at shutdown (`3.24e-6 T^0.4 psi`
    ///   with T 5.05e7 s, psi 0.713), and `G_max(1e4 s)` is 1.064, the
    ///   table's first value: the standard's own step.
    /// - Np-239's decay heat at 5 h, as a fraction of rated power, is
    ///   `E_Np R lambda1 (1 - e^-l2 T) e^-l2 t/(l1 - l2)` per fission over
    ///   the mean Q (the U-239 term has decayed away): by hand, 0.419 x
    ///   0.526 x 1.007 x 1 x 0.940 / 202 = 1.03e-3.
    /// - The total against the old U-235-equilibrium source, as the
    ///   prediction (+5 to +10 % over 3-25 h) said before any plant run.
    #[test]
    fn decay_heat_follows_the_ans_1979_form() {
        let d0 = at(ALL, 0.0);
        let g0 = d0.capture_factor() - 1.0;
        assert!((g0 - 2.8e-3).abs() < 1.0e-4, "{g0}");
        let d = at(ALL, 1.0e4);
        assert!((d.capture_factor() - 1.064).abs() < 1e-12);
        let p = Params::default();
        let d5 = at(ALL, 5.0 * 3600.0);
        let frac = d5.actinide_power_w() / p.thermal_power_w;
        assert!((frac - 1.03e-3).abs() < 0.02e-3, "{frac}");
        for h in [1.0, 3.0, 5.0, 10.0, 25.0] {
            let (new, old) = (at(ALL, h * 3600.0).power_w(), at(OLD, h * 3600.0).power_w());
            println!("t = {h} h: new/old = {:.4}", new / old);
            assert!(new > old, "{h} h");
        }
    }
}
