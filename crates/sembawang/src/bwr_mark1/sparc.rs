//! Pool scrubbing past the vent exit: SPARC-90's inertial impaction and
//! swarm-rise removal, as MELCOR describes them (gh:#1047; no source used;
//! MELCOR 2.2 Vol. 2, SAND2023-10994 O, RN §2.7.1 Eqs 2-112 and 2-113,
//! Appendices D and E).
//!
//! MELCOR's overall decontamination factor for each particle size is
//!
//! ```text
//! DF_OV,i = DF_EC . DF_II,i . DF_ER,i . DF_SR,i                (2-113)
//! ```
//!
//! `DF_EC`, removal in proportion to the gas volume the steam's condensation
//! takes away, is in `containment.rs`. This module gives the next two:
//!
//! - **`DF_II`, inertial impaction** into the front of the decelerating
//!   globule at the vent exit (Eqs D-8 to D-11), from the Stokes number of
//!   the particle at the vent's exit velocity.
//! - **`DF_SR`, the swarm rise** (Eq 2-112, Appendix E). The injected gas
//!   forms a globule (Eqs D-1 to D-4) that breaks into a swarm of bubbles
//!   over twelve of its diameters. The rise from the vent exit to the pool
//!   surface is marched in `N_RISE` steps; in each, the part of the gas
//!   already in bubbles loses particles to the bubble wall by centrifugal
//!   and Brownian deposition and by settling (Eqs E.12 to E.25), the part
//!   still in the globule loses none.
//!
//! `DF_ER` (deposition during injection) applies only to multihole vents
//! (MELCOR's `MVENT=1`). The downcomers are single large pipes, so it is 1
//! and is not built.
//!
//! **Where this departs from the manual, and why:**
//!
//! - **The bubble is at the pool's temperature and saturated with
//!   vapour.** MELCOR iterates the bubble's heat and mass transfer through
//!   each step (Eqs E.6 to E.11). The gas is taken to be already at the
//!   pool's temperature when it leaves the vent exit region, which is
//!   MELCOR's own assumption for that region. Two things follow. The
//!   evaporation into the bubble is whatever keeps it saturated as it
//!   expands under the falling static head. And the particles do not grow
//!   by condensation (no supersaturation, so Eq E.10 is not needed). That
//!   removes a removal mechanism MELCOR has, so this is a lower bound.
//! - **The exposure time of the bubble surface** is the boundary-layer
//!   value for a stretching surface (Levich): `tau_e = int r^2 V_s ds /
//!   (r^2 V_s^2)`. The manual's "arc length divided by the local surface
//!   velocity" is the flat-plate limit of this. Taken literally on a
//!   bubble it diverges at the top stagnation point, where V_s goes to 0
//!   (the integral of ds/V_s is logarithmic there). On a sphere, Levich's
//!   form gives Boussinesq's Sh = (2/sqrt(pi)) Pe^(1/2) exactly; the test
//!   checks it.
//! - **The surface normal** is the ellipse's own normal: tan beta =
//!   (b/a)^2 tan theta. The manual's Eq E.15 prints (a/b)^2. With a > b
//!   (oblate), that would make the normal *less* vertical than the radius
//!   on the flat top, which is geometrically wrong.
//! - **The evaporation-hindrance floor** of Eq E.14 ("limited to a minimum
//!   value of V_D,i") is read as the diffusion velocity actually in
//!   effect, `xi V_D` of Eq E.24. Read as the unhindered `V_D`, the floor
//!   would undo E.24 wherever it binds.
//! - **The fraction of the gas still in the globule** is its volume
//!   fraction, `(D_g / D_g,o)^3`, from the linear shrinkage of Eq D-4.
//!   The manual gives the shrinkage but not the fraction.
//! - **Settling** is Stokes's law with slip (Eq E.16) throughout. MELCOR
//!   switches to empirical drag above about 70 um (Eqs E.17 to E.19);
//!   the largest section here is 40 um.
//!
//! Gas properties are air's, as MELCOR evaluates every aerosol
//! coefficient (RN §2.4.2.3, constraint c).

use super::aerosol::Medium;
use super::steam::p_sat_pa;
use petir::mathf::RealMath;
use std::f64::consts::PI;
use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_single_phase;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::v_tp_1;
use tampines_steam_tables::surface_tension::water_surf_tension;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::radiant_exposure::joule_per_square_meter;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

const G: f64 = 9.80665;
const R_GAS: f64 = 8.314462618;
/// Steps of the rise (MELCOR's XNRISE, C7150(1), default 10).
pub const N_RISE: usize = 10;
/// Strips of the bubble surface in the quadrature. MELCOR's XNCIRC default
/// is 5; this is a numerical choice, set where the result has converged:
/// at 16 the Boussinesq check (test) is within 2e-4, and the removal rate
/// of a 0.0039 x 0.0027 m bubble within 0.15 % of 256 strips (2026-10-29).
const N_STRIPS: usize = 16;

/// The vent types of MELCOR's globule correlation `v_n = a We^b` (Eq D-1,
/// C7151 defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vent {
    MultipleHoles,
    Downcomer,
    Horizontal,
}

impl Vent {
    fn globule(self) -> (f64, f64) {
        match self {
            Vent::MultipleHoles => (3.45, 0.46),
            Vent::Downcomer => (0.0891, 0.616),
            Vent::Horizontal => (0.857, 0.73),
        }
    }
}

/// The pool where the gas is injected.
#[derive(Clone, Copy, Debug)]
pub struct Pool {
    pub t_k: f64,
    /// Gas pressure at the pool surface, Pa.
    pub p_surface_pa: f64,
    /// Submergence of the vent exit, m.
    pub depth_m: f64,
}

impl Pool {
    /// A pool at `t_k` under gas at `p_surface_pa`, the vent exit under a
    /// static head of `head_pa` of its water.
    pub fn submerged(t_k: f64, p_surface_pa: f64, head_pa: f64) -> Self {
        let mut pool = Pool { t_k, p_surface_pa, depth_m: 0.0 };
        pool.depth_m = head_pa / (pool.rho_l() * G);
        pool
    }
    pub(crate) fn rho_l(&self) -> f64 {
        let t = ThermodynamicTemperature::new::<kelvin>(self.t_k.min(640.0));
        let p = Pressure::new::<pascal>(self.p_surface_pa.max(p_sat_pa(self.t_k.min(640.0)) * 1.0001));
        1.0 / v_tp_1(t, p).get::<cubic_meter_per_kilogram>()
    }
    pub(crate) fn sigma(&self) -> f64 {
        water_surf_tension(ThermodynamicTemperature::new::<kelvin>(self.t_k.min(640.0))).get::<joule_per_square_meter>()
    }
    /// Pressure at height `x` above the vent exit, Pa.
    fn p_at(&self, rho_l: f64, x: f64) -> f64 {
        self.p_surface_pa + rho_l * G * (self.depth_m - x).max(0.0)
    }
    /// MELCOR's `X_o` (Eq D-6): the non-condensable mole fraction of the
    /// gas once it is in equilibrium with the pool at the vent exit.
    pub fn x_o(&self) -> f64 {
        (1.0 - p_sat_pa(self.t_k.min(640.0)) / self.p_at(self.rho_l(), 0.0)).max(0.0)
    }
}

/// The gas entering one vent: its non-condensable flow, mol/s, its total
/// volumetric flow at the vent exit before equilibration, m^3/s, and the
/// vent's diameter, m.
#[derive(Clone, Copy, Debug)]
pub struct Injection {
    pub vent: Vent,
    pub diameter_m: f64,
    pub nc_mol_s: f64,
    pub q_exit_m3_s: f64,
}

/// The bubble of the swarm, as an oblate spheroid of semi-axes `a`
/// (horizontal) and `b` (vertical), m, for a non-condensable mole fraction
/// `x_nc` (Eqs E.1, E.2).
pub fn bubble_axes(x_nc: f64) -> (f64, f64) {
    let d_cm = 0.7 * 10f64.r_powf(-0.2265 + (0.0203 + 0.0313 * x_nc.clamp(0.0, 1.0)).sqrt());
    let ab = if d_cm < 0.15 {
        1.0
    } else if d_cm > 1.3 {
        1.675
    } else {
        0.84107 + 1.13466 * d_cm - 0.3795 * d_cm * d_cm
    };
    // Equal volume to the sphere: a^2 b = r^3.
    let r = 0.5 * d_cm * 1e-2;
    let b = r / ab.r_powf(2.0 / 3.0);
    (ab * b, b)
}

/// The bubble's rise velocity relative to the liquid, m/s (Eq E.3, with
/// `sigma` in N/m and `rho_l` in kg/m^3 converted to the correlation's cgs).
pub fn rise_velocity(d_cm: f64, sigma: f64, rho_l: f64) -> f64 {
    let v_t = 7.876 * (sigma * 1e3 / (rho_l * 1e-3)).r_powf(0.25);
    let v = if d_cm <= 0.5 { v_t } else { 1.40713 * d_cm.r_powf(0.49275) * v_t };
    v * 1e-2
}

/// The swarm's rise velocity, m/s (Eqs E.4, E.5): the mean of the
/// correlation at the vent depth and at the surface, for a total flow
/// `q_l_s` (litre/s, at half depth) through one vent, at most 1.7 m/s.
pub fn swarm_velocity(q_l_s: f64, depth_m: f64) -> f64 {
    let at = |x_cm: f64| ((q_l_s + 5.33) / 3.011e-3).sqrt() * (1.0 - 3.975e-4 * x_cm);
    (0.5 * (at(0.0) + at(depth_m * 100.0))).min(170.0) * 1e-2
}

/// The initial globule's diameter, m (Eqs D-1 to D-3), for a gas flow
/// `q` (m^3/s at the vent depth, in equilibrium with the pool).
pub fn globule_diameter(vent: Vent, d_o: f64, q: f64, sigma: f64, rho_l: f64) -> f64 {
    let (a, b) = vent.globule();
    let v_o = q / (0.25 * PI * d_o * d_o);
    let we = rho_l * d_o * v_o * v_o / sigma;
    let v_n = a * we.r_powf(b);
    (1.5 * v_n * d_o * d_o * (sigma / (rho_l * G)).sqrt()).r_powf(1.0 / 3.0)
}

/// The inertial-impaction factor `DF_II` (Eqs D-8 to D-11) for a particle
/// of diameter `d` and density `rho_p`, gas viscosity `mu`, vent exit
/// velocity `v_e` and vent diameter `d_o`.
pub fn impaction_df(d: f64, rho_p: f64, mu: f64, v_e: f64, d_o: f64) -> f64 {
    let stk = rho_p * v_e * d * d / (9.0 * mu * d_o);
    let alpha = if stk <= 0.65868 {
        1.79182 * 3.3437e-11f64.r_powf(5.9244e-3f64.r_powf(stk))
    } else {
        1.13893 * 1.4173e-6f64.r_powf(4.25973e-3f64.r_powf(stk))
    };
    1.0 / (1.0 - alpha.min(0.99))
}

/// The bubble's surface, sampled at strip midpoints in the oblate angle
/// `eta` (0 at the top): area, local surface speed, curvature radius,
/// cosine of the normal's angle from vertical, and the Levich exposure
/// time.
struct Surface {
    da: Vec<f64>,
    v_s: Vec<f64>,
    r_c: Vec<f64>,
    cos_beta: Vec<f64>,
    tau_e: Vec<f64>,
    volume: f64,
}

impl Surface {
    /// Potential flow past the oblate spheroid rising at `v_r` along its
    /// minor axis (the manual's Eq E.21, which this reproduces in oblate
    /// coordinates: `V_s = V_r K sin(eta) / h`, `h` the metric
    /// `sqrt(a^2 cos^2 + b^2 sin^2)`, and `K = 3b/2` on a sphere).
    fn new(a: f64, b: f64, v_r: f64) -> Self {
        let k = if a / b < 1.0 + 1e-4 {
            1.5 * b
        } else {
            let c = (a * a - b * b).sqrt();
            // c / (cosh^2 xi0 acot(sinh xi0) - sinh xi0), sinh xi0 = b/c.
            c * c * c / (a * a * (c / b).r_atan() - b * c)
        };
        let n = N_STRIPS;
        let d_eta = PI / n as f64;
        let mut s = Surface { da: vec![], v_s: vec![], r_c: vec![], cos_beta: vec![], tau_e: vec![], volume: 4.0 / 3.0 * PI * a * a * b };
        for j in 0..n {
            let eta = (j as f64 + 0.5) * d_eta;
            let (sn, cs) = (eta.r_sin(), eta.r_cos());
            let h = (a * a * cs * cs + b * b * sn * sn).sqrt();
            s.da.push(2.0 * PI * a * sn * h * d_eta);
            s.v_s.push(v_r * k * sn / h);
            s.r_c.push(h * h * h / (a * b));
            s.cos_beta.push(a * cs / h);
            // int_0^eta sin^3 = 2/3 - cos + cos^3/3.
            let g = 2.0 / 3.0 - cs + cs * cs * cs / 3.0;
            s.tau_e.push(g * h * h / (v_r * k * sn.r_powf(4.0)));
        }
        s
    }

    /// `(1/v_b) int V_n dA`, 1/s (Eqs E.13, E.14), for a particle with
    /// settling velocity `v_g` and diffusivity `d_i`, and an evaporative
    /// velocity `v_v` into the bubble.
    fn removal_rate(&self, v_g: f64, d_i: f64, v_v: f64) -> f64 {
        let mut sum = 0.0;
        for j in 0..self.da.len() {
            let v_d = (d_i / (PI * self.tau_e[j])).sqrt();
            let phi = if v_d > 0.0 { v_v / v_d } else { 0.0 };
            // Past phi = 7 the factor is below 3e-22: zero to the sum.
            let xi = if phi < 7.0 { (-phi * phi).r_exp() / (2.0 - (-1.85 * phi).r_exp()) } else { 0.0 };
            let v_d = xi * v_d;
            let v_c = v_g * self.v_s[j] * self.v_s[j] / (self.r_c[j] * G);
            let v_n = (v_c + v_d - v_g * self.cos_beta[j] - v_v).max(v_d);
            sum += v_n * self.da[j];
        }
        sum / self.volume
    }

    fn area(&self) -> f64 {
        self.da.iter().sum()
    }
}

/// One step of the rise: its pressure, the time spent in it, the share of
/// the gas still in the globule, the bubble's surface, and the evaporative
/// velocity into the bubble.
struct RiseStep {
    p: f64,
    dt: f64,
    f_gl: f64,
    surf: Surface,
    v_v: f64,
}

/// The march through the rise (Appendix E), shared by the particles and
/// the iodine vapour: the steps, and the vent's exit velocity. `None` with
/// no gas or no depth.
fn rise(pool: &Pool, inj: &Injection) -> Option<(Vec<RiseStep>, f64)> {
    if inj.nc_mol_s <= 0.0 || pool.depth_m <= 0.0 {
        return None;
    }
    let rho_l = pool.rho_l();
    let sigma = pool.sigma();
    let p_sat = p_sat_pa(pool.t_k.min(640.0));
    // Equilibrium gas flow (non-condensable plus saturated vapour), m^3/s.
    let q_at = |x: f64| {
        let p = pool.p_at(rho_l, x);
        inj.nc_mol_s * R_GAS * pool.t_k / (p - p_sat).max(1.0)
    };
    let d_g0 = globule_diameter(inj.vent, inj.diameter_m, q_at(0.0), sigma, rho_l);
    let v_sw = swarm_velocity(q_at(0.5 * pool.depth_m) * 1e3, pool.depth_m);
    let v_e = inj.q_exit_m3_s / (0.25 * PI * inj.diameter_m * inj.diameter_m);
    let dx = pool.depth_m / N_RISE as f64;
    let dt = dx / v_sw;
    let steps = (0..N_RISE)
        .map(|k| {
            let x = (k as f64 + 0.5) * dx;
            let p = pool.p_at(rho_l, x);
            let x_nc = (1.0 - p_sat / p).max(0.0);
            let (a, b) = bubble_axes(x_nc);
            let d_cm = 2.0 * (a * a * b).r_powf(1.0 / 3.0) * 100.0;
            let surf = Surface::new(a, b, rise_velocity(d_cm, sigma, rho_l));
            // Evaporation that keeps the expanding bubble saturated, per
            // unit of its surface, m/s (the vapour volume added as the
            // static head falls by rho_l g v_sw each second).
            let v_v = surf.volume / surf.area() * p_sat * rho_l * G * v_sw / (p * (p - p_sat).max(1.0));
            let f_gl = (1.0 - x / (12.0 * d_g0)).max(0.0).r_powf(3.0);
            RiseStep { p, dt, f_gl, surf, v_v }
        })
        .collect();
    Some((steps, v_e))
}

/// Pass fractions `1 / (DF_II DF_SR)` for particles of diameters `d` (m)
/// and density `rho_p` injected through one vent into `pool`.
pub fn pass_fractions(pool: &Pool, inj: &Injection, d: &[f64], rho_p: f64) -> Vec<f64> {
    let Some((steps, v_e)) = rise(pool, inj) else {
        return vec![1.0; d.len()];
    };
    let mu = air_viscosity(pool.t_k);
    let mut pass: Vec<f64> = d.iter().map(|&di| 1.0 / impaction_df(di, rho_p, mu, v_e, inj.diameter_m)).collect();
    for st in &steps {
        let mut medium = Medium::air(pool.t_k, st.p, mu);
        medium.rho_p = rho_p;
        for (i, &di) in d.iter().enumerate() {
            let rate = st.surf.removal_rate(medium.settling_velocity(di), medium.diffusivity(di), st.v_v);
            let df_bb = (rate * st.dt).r_exp();
            pass[i] *= st.f_gl + (1.0 - st.f_gl) / df_bb;
        }
    }
    pass
}

/// The pool's side of iodine-vapour scrubbing: I2's diffusivity in the
/// bubble gas and in the water, m^2/s, the pool's partition coefficient
/// (`iodine::partition`), its dissolved iodine, mol of I2 per litre, and
/// the entering gas's I2 concentration at the vent exit, mol per litre of
/// gas.
#[derive(Clone, Copy, Debug)]
pub struct IodineVapour {
    pub d_gas: f64,
    pub d_liquid: f64,
    pub h: f64,
    pub sum_aq: f64,
    pub c_gas: f64,
}

/// I2's diffusivity in water, m^2/s, at `t_k`: 1.2e-9 at 298.15 K \[A\],
/// scaled by Stokes-Einstein (`T / mu`) with water's viscosity from the
/// steam tables. The manual gives no value; with the pool's partition
/// large (CsOH), the gas side controls and this barely matters (test).
pub fn i2_liquid_diffusivity(t_k: f64, p_pa: f64) -> f64 {
    let mu = |t: f64| {
        let t = ThermodynamicTemperature::new::<kelvin>(t.min(640.0));
        let p = Pressure::new::<pascal>(p_pa.max(p_sat_pa(t.get::<kelvin>()) * 1.0001));
        mu_tp_eqm_single_phase(t, p).get::<pascal_second>()
    };
    1.2e-9 * (t_k / 298.15) * (mu(298.15) / mu(t_k))
}

/// The swarm rise's pass fraction for I2 vapour through one vent (MELCOR
/// Appendix F): in each step and on each strip of the bubble, the
/// gas-side penetration velocity `sqrt(D_g / (pi tau_e))` brings I2 to the
/// interface, whose concentration balances it against the liquid side
/// with the partition (Eq F.2); the net velocity is hindered by the
/// evaporation as for particles (Eq F.1) and integrated as Eq E.13. The
/// bubble's I2 concentration is updated after each step (diluted as the
/// bubble expands and scrubbed). No scrubbing where the pool's iodine over
/// the partition is at least the bubble's.
pub fn iodine_pass(pool: &Pool, inj: &Injection, iv: &IodineVapour) -> f64 {
    let Some((steps, _)) = rise(pool, inj) else {
        return 1.0;
    };
    if iv.c_gas <= 0.0 {
        return 1.0;
    }
    let p_sat = p_sat_pa(pool.t_k.min(640.0));
    let p0 = pool.p_at(pool.rho_l(), 0.0);
    let mut pass = 1.0;
    for st in &steps {
        // The bubble's concentration: the entering gas's, diluted by the
        // expansion (non-condensable moles fixed, volume as 1/(p - p_sat))
        // and by what earlier steps removed.
        let c = iv.c_gas * (st.p - p_sat).max(1.0) / (p0 - p_sat).max(1.0) * pass;
        if iv.sum_aq / iv.h >= c {
            continue;
        }
        let s = &st.surf;
        let mut sum = 0.0;
        for j in 0..s.da.len() {
            let vg = (iv.d_gas / (PI * s.tau_e[j])).sqrt();
            let vl = (iv.d_liquid / (PI * s.tau_e[j])).sqrt();
            let c_i = (vg * c + vl * iv.sum_aq) / (vg + vl * iv.h);
            let v_d = vg * (c - c_i) / c;
            if v_d <= 0.0 {
                continue;
            }
            let phi = st.v_v / v_d;
            let xi = if phi < 7.0 { (-phi * phi).r_exp() / (2.0 - (-1.85 * phi).r_exp()) } else { 0.0 };
            sum += xi * v_d * s.da[j];
        }
        let df_bb = (sum / s.volume * st.dt).r_exp();
        pass *= st.f_gl + (1.0 - st.f_gl) / df_bb;
    }
    pass
}

/// Air's viscosity, Pa s (Sutherland, as `containment::air_viscosity`).
fn air_viscosity(t_k: f64) -> f64 {
    1.458e-6 * t_k * t_k.sqrt() / (t_k + 110.4)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces against hand values (2026-10-29):
    /// - E.1 at x_nc = 1 gives 0.7 x 10^(-0.2265 + 0.22716) = 0.7011 cm;
    ///   E.2 gives a/b = 1.4497 there, and the axes keep the sphere's
    ///   volume;
    /// - E.3 at 0.5 cm with water at 300 K (sigma 0.0717 N/m, rho 996.5)
    ///   is 7.876 x 72.0^0.25 = 22.9 cm/s, and continuous across 0.5 cm
    ///   (1.40713 x 0.5^0.49275 = 1.0002);
    /// - E.5 with no flow at 1.2 m: sqrt(5.33/3.011e-3) = 42.07 cm/s at the
    ///   vent, x(1 - 0.0477) at the surface, mean 41.07 cm/s;
    /// - D-10 and D-11 meet at Stk = 0.65868 (alpha = 0.787 from both);
    ///   impaction of a 0.1 um particle is nothing;
    /// - on a sphere, the surface speed is 1.5 V_r sin(theta), and the
    ///   area-mean of sqrt(D / (pi tau_e)) is Boussinesq's
    ///   (2/sqrt(pi)) sqrt(D V_r / d) to 1e-3 (the strip quadrature: at 16
    ///   strips the area is 0.32 % high and the Boussinesq mean 2e-4 low).
    #[test]
    fn the_sparc_pieces_match_hand_values() {
        let (a, b) = bubble_axes(1.0);
        let d_cm = 2.0 * (a * a * b).r_powf(1.0 / 3.0) * 100.0;
        assert!((d_cm - 0.7011).abs() < 1e-3, "{d_cm}");
        assert!((a / b - 1.4497).abs() < 1e-3, "{}", a / b);
        let v = rise_velocity(0.5, 0.0717, 996.5);
        assert!((v - 0.2290).abs() < 5e-4, "{v}");
        assert!((rise_velocity(0.50001, 0.0717, 996.5) / v - 1.0).abs() < 1e-3);
        let vs = swarm_velocity(0.0, 1.2);
        assert!((vs - 0.4107).abs() < 5e-4, "{vs}");
        assert!((swarm_velocity(1e6, 1.2) - 1.7).abs() < 1e-12);
        let mu = 2.0e-5;
        let stk_d = |stk: f64| (stk * 9.0 * mu * 0.5 / (1000.0 * 10.0)).sqrt();
        let lo = impaction_df(stk_d(0.65868) * (1.0 - 1e-9), 1000.0, mu, 10.0, 0.5);
        let hi = impaction_df(stk_d(0.65868) * (1.0 + 1e-9), 1000.0, mu, 10.0, 0.5);
        assert!((lo / hi - 1.0).abs() < 1e-5 && (1.0 - 1.0 / lo - 0.787).abs() < 1e-3, "{lo} {hi}");
        assert!((impaction_df(1e-7, 1000.0, mu, 1.0, 0.5) - 1.0).abs() < 1e-9);
        let r = 0.003;
        let s = Surface::new(r, r, 0.2);
        let j = N_STRIPS / 4;
        let eta = (j as f64 + 0.5) * PI / N_STRIPS as f64;
        assert!((s.v_s[j] / (1.5 * 0.2 * eta.r_sin()) - 1.0).abs() < 1e-12);
        assert!((s.area() / (4.0 * PI * r * r) - 1.0).abs() < 4e-3, "{}", s.area() / (4.0 * PI * r * r));
        let dif = 1e-9;
        let mean: f64 = (0..N_STRIPS).map(|j| (dif / (PI * s.tau_e[j])).sqrt() * s.da[j]).sum::<f64>() / s.area();
        let boussinesq = 2.0 / PI.sqrt() * (dif * 0.2 / (2.0 * r)).sqrt();
        assert!((mean / boussinesq - 1.0).abs() < 1e-3, "{mean} {boussinesq}");
    }

    /// Settling alone (no circulation, so no centrifugal velocity) removes
    /// the projected area's share (to the quadrature: +0.65 % at 16
    /// strips), `v_g pi a^2 / v_b`, because the
    /// floor keeps the top's negative normal velocity from cancelling the
    /// bottom's. At the bubble's real rise velocity (0.27 m/s here) the
    /// centrifugal velocity, which also scales with `v_g`, takes 17.0 times
    /// more (2026-10-29): it, not settling, is the swarm's main removal of
    /// large particles.
    #[test]
    fn settling_onto_an_oblate_bubble_takes_its_projected_area() {
        let (a, b) = bubble_axes(1.0);
        let v_g = 1e-3;
        let want = v_g * PI * a * a / (4.0 / 3.0 * PI * a * a * b);
        let s = Surface::new(a, b, 1e-6);
        let rate = s.removal_rate(v_g, 0.0, 0.0);
        assert!((rate / want - 1.0).abs() < 1e-2, "{rate} {want}");
        let d_cm = 2.0 * (a * a * b).r_powf(1.0 / 3.0) * 100.0;
        let real = Surface::new(a, b, rise_velocity(d_cm, 0.0717, 996.5)).removal_rate(v_g, 0.0, 0.0);
        assert!((real / want - 17.03).abs() < 0.05, "{}", real / want);
    }

    /// The whole factor (2026-10-29), at the plant's downcomers: a 0.515 m
    /// pipe 1.2 m deep in a 375 K pool under 0.3 MPa, 50 mol/s of
    /// non-condensable gas through it. Predicted before the run: the
    /// globule (D_g,o of order 0.3 m) outlasts most of the 1.2 m rise, so
    /// the swarm's factor is weak; small particles pass almost entirely
    /// (pass > 0.8 at 0.1 um), large ones are removed much more (pass <
    /// 0.3 at 20 um), and pass falls with size above about 1 um.
    /// Measured: 0.1 um 0.9796, 1 um 0.9798, 5 um 0.6083, 20 um 0.0434,
    /// inside the prediction (Brownian removal of the smallest makes 0.1 um
    /// pass very slightly less than 1 um, the classic minimum near 0.3-1
    /// um).
    #[test]
    fn the_swarm_scrubs_large_particles_and_passes_small_ones() {
        let pool = Pool { t_k: 375.0, p_surface_pa: 3.0e5, depth_m: 1.2 };
        let inj = Injection { vent: Vent::Downcomer, diameter_m: 0.515, nc_mol_s: 50.0, q_exit_m3_s: 2.0 };
        let d = [0.1e-6, 1e-6, 5e-6, 20e-6];
        let p = pass_fractions(&pool, &inj, &d, 1000.0);
        assert!(p[0] > 0.8 && p[3] < 0.3, "{p:?}");
        let want = [0.9796, 0.9798, 0.6083, 0.0434];
        assert!(p.iter().zip(want).all(|(x, w)| (x - w).abs() < 1e-4), "{p:?}");
        assert!(p[1] > p[2] && p[2] > p[3], "{p:?}");
        assert!(p.iter().all(|&x| x > 0.0 && x <= 1.0));
        // No gas, nothing scrubbed; no depth likewise.
        assert_eq!(pass_fractions(&pool, &Injection { nc_mol_s: 0.0, ..inj }, &d, 1000.0), vec![1.0; 4]);
        // X_o at the vent: 1 - p_sat(375 K)/(3e5 + rho g 1.2), with
        // p_sat(375 K) = 108.0 kPa and rho = 957 kg/m^3: 0.653.
        let xo = pool.x_o();
        assert!((xo - (1.0 - 108_000.0 / (3.0e5 + 957.0 * G * 1.2))).abs() < 2e-3, "{xo}");
    }

    /// Iodine vapour in the swarm (2026-10-29), at the downcomer case
    /// above with D_g = 1e-5 m^2/s and 1e-6 mol/L entering. Predicted: in an
    /// alkaline pool (H = 2e4) the gas side controls and the bubbles lose
    /// nearly all their I2, so the factor is set by the globule, of order
    /// tens; a neutral pool (H = 16) is limited by the liquid side and
    /// scrubs much less; a pool whose iodine over H exceeds the gas's does
    /// not scrub. Measured: alkaline 0.0860 (DF 11.6; complete removal in
    /// the bubbles would be the product of the globule shares, 0.0364:
    /// the evaporation's hindrance takes the rest); neutral 0.773
    /// (1e-5 M dissolved) and 0.394 (none); saturated 1; doubling the
    /// \[A\] liquid diffusivity moves the alkaline case by 1e-4
    /// (0.08598 -> 0.08591). I2's liquid diffusivity at 375 K is
    /// 4.86e-9 m^2/s.
    #[test]
    fn the_swarm_scrubs_iodine_vapour_by_its_partition() {
        let pool = Pool { t_k: 375.0, p_surface_pa: 3.0e5, depth_m: 1.2 };
        let inj = Injection { vent: Vent::Downcomer, diameter_m: 0.515, nc_mol_s: 50.0, q_exit_m3_s: 2.0 };
        let dl = i2_liquid_diffusivity(375.0, 3.0e5);
        assert!((dl / 4.862e-9 - 1.0).abs() < 1e-3, "{dl}");
        let at = |h: f64, sum: f64, dl: f64| iodine_pass(&pool, &inj, &IodineVapour { d_gas: 1e-5, d_liquid: dl, h, sum_aq: sum, c_gas: 1e-6 });
        let near = |x: f64, w: f64| (x - w).abs() < 1e-4;
        assert!(near(at(2e4, 1e-5, dl), 0.08598), "{}", at(2e4, 1e-5, dl));
        assert!(near(at(16.0, 1e-5, dl), 0.77294) && near(at(16.0, 0.0, dl), 0.39395), "{} {}", at(16.0, 1e-5, dl), at(16.0, 0.0, dl));
        assert_eq!(at(16.0, 1e-3, dl), 1.0);
        assert!(near(at(2e4, 1e-5, 2.0 * dl), 0.08591));
        let (steps, _) = rise(&pool, &inj).unwrap();
        let globule: f64 = steps.iter().map(|s| s.f_gl).product();
        assert!(at(2e4, 0.0, dl) > globule && (globule - 0.0364).abs() < 1e-4, "{globule}");
    }
}
