//! Closures read off the CFD, and their interpolation to points the CFD was
//! not run at (step (b) of gh:#1178).
//!
//! **Read off, not fitted.** A [`Closures`] comes from one CFD point's
//! fields alone: the mean Nusselt numbers (`cfd::closures_from_profiles`)
//! and the apparent friction `B = (2 Δp / ρ U²) Re`, which is `(f L/D + K)
//! Re` and makes TUAS's `SimpleReynoldsPower(0, B, −1)` loss reproduce that
//! Δp at that Re. No 1-D output is consulted, so nothing here is a
//! calibration (that is rung 3, gh:#1180).
//!
//! **Interpolation:** [`ClosureTable`] is multilinear in
//! (ln Re_hot, ln Re_cold, ln k_wall, ln L) on (ln Nu_hot, ln Nu_cold,
//! ln B_hot, ln B_cold), over a full-factorial table of training points.
//! Log coordinates because developing laminar Nu and f·Re are close to
//! power laws in the Graetz number (V&V record, "Closures"). It refuses
//! held-out points and points outside the table.
use petir::mathf::RealMath;
use uom::si::f64::Ratio;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

use super::design::Split;
use super::{fixed, DoublePipeInput, DoublePipeOutput};

/// The closures the 1-D model takes at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Closures {
    /// Tube-side mean Nusselt number (1-D convention).
    pub nu_hot: Ratio,
    /// Annulus-side mean Nusselt number (1-D convention).
    pub nu_cold: Ratio,
    /// Tube `B = (f L/D + K) Re`.
    pub loss_hot: Ratio,
    /// Annulus `B`.
    pub loss_cold: Ratio,
}

impl Closures {
    /// Read off one CFD point.
    #[must_use]
    pub fn from_cfd(x: &DoublePipeInput, y: &DoublePipeOutput) -> Self {
        let [re_h, re_c, _, _] = x.to_si();
        let (qh, qc) = x.flow_rates();
        let (ah, ac) = fixed::areas();
        let b = |dp: f64, q: f64, a: f64, re: f64| {
            let u = q / a;
            2.0 * dp / (fixed::RHO * u * u) * re
        };
        Self {
            nu_hot: y.nu_hot,
            nu_cold: y.nu_cold,
            loss_hot: Ratio::new::<ratio>(b(y.dp_hot.get::<pascal>(), qh, ah, re_h)),
            loss_cold: Ratio::new::<ratio>(b(y.dp_cold.get::<pascal>(), qc, ac, re_c)),
        }
    }

    fn ln(&self) -> [f64; 4] {
        [self.nu_hot, self.nu_cold, self.loss_hot, self.loss_cold].map(|r| r.get::<ratio>().r_ln())
    }

    fn from_ln(v: [f64; 4]) -> Self {
        let e = |x: f64| Ratio::new::<ratio>(x.r_exp());
        Self { nu_hot: e(v[0]), nu_cold: e(v[1]), loss_hot: e(v[2]), loss_cold: e(v[3]) }
    }
}

/// Why a table could not be built or queried.
#[derive(Debug, Clone, PartialEq)]
pub enum ClosureError {
    /// A held-out point was offered to [`ClosureTable::build`].
    HeldOutOffered,
    /// The training points are not a full factorial on their own levels.
    NotFactorial(String),
    /// The query lies outside the table on an axis (axis index 0–3).
    OutOfRange(usize),
}

impl std::fmt::Display for ClosureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClosureError::HeldOutOffered => write!(f, "a held-out point may not build a closure"),
            ClosureError::NotFactorial(m) => write!(f, "training points are not a full factorial: {m}"),
            ClosureError::OutOfRange(a) => write!(f, "query outside the closure table on axis {a}"),
        }
    }
}

impl std::error::Error for ClosureError {}

/// A full-factorial table of [`Closures`] over the four inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureTable {
    /// Levels per axis, ln-transformed, increasing.
    levels: [Vec<f64>; 4],
    /// ln closures at every node, row-major over the axes in order.
    values: Vec<[f64; 4]>,
}

fn ln_x(x: &DoublePipeInput) -> [f64; 4] {
    x.to_si().map(<f64 as petir::mathf::RealMath>::r_ln)
}

impl ClosureTable {
    /// Build from training points.
    ///
    /// # Errors
    /// [`ClosureError::HeldOutOffered`] if any point is held out;
    /// [`ClosureError::NotFactorial`] unless every combination of the
    /// points' own levels appears exactly once.
    pub fn build(points: &[(Split, DoublePipeInput, Closures)]) -> Result<Self, ClosureError> {
        if points.iter().any(|(s, _, _)| *s == Split::HeldOut) {
            return Err(ClosureError::HeldOutOffered);
        }
        let mut levels: [Vec<f64>; 4] = Default::default();
        for (_, x, _) in points {
            for (a, v) in ln_x(x).into_iter().enumerate() {
                if !levels[a].iter().any(|l| (l - v).abs() < 1e-12) {
                    levels[a].push(v);
                }
            }
        }
        for l in &mut levels {
            l.sort_by(f64::total_cmp);
        }
        let n: usize = levels.iter().map(Vec::len).product();
        if n != points.len() {
            return Err(ClosureError::NotFactorial(format!("{} points for {} level combinations", points.len(), n)));
        }
        let mut values = vec![[f64::NAN; 4]; n];
        for (_, x, c) in points {
            let idx = Self::flat(&levels, &Self::node(&levels, &ln_x(x)));
            if values[idx][0].is_finite() {
                return Err(ClosureError::NotFactorial(format!("duplicate node {x:?}")));
            }
            values[idx] = c.ln();
        }
        Ok(Self { levels, values })
    }

    fn node(levels: &[Vec<f64>; 4], v: &[f64; 4]) -> [usize; 4] {
        let mut out = [0; 4];
        for a in 0..4 {
            out[a] = levels[a].iter().position(|l| (l - v[a]).abs() < 1e-12).unwrap_or(0);
        }
        out
    }

    fn flat(levels: &[Vec<f64>; 4], i: &[usize; 4]) -> usize {
        let mut f = 0;
        for a in 0..4 {
            f = f * levels[a].len() + i[a];
        }
        f
    }

    /// The table's levels in SI (Re_hot, Re_cold, k_wall, L).
    #[must_use]
    pub fn levels_si(&self) -> [Vec<f64>; 4] {
        self.levels.clone().map(|l| l.into_iter().map(<f64 as petir::mathf::RealMath>::r_exp).collect())
    }

    /// Interpolate at `x` (multilinear in the logs).
    ///
    /// # Errors
    /// [`ClosureError::OutOfRange`] outside the table (no extrapolation).
    pub fn at(&self, x: &DoublePipeInput) -> Result<Closures, ClosureError> {
        let v = ln_x(x);
        let mut lo = [0usize; 4];
        let mut w = [0.0f64; 4];
        for a in 0..4 {
            let l = &self.levels[a];
            let tol = 1e-12;
            if v[a] < l[0] - tol || v[a] > l[l.len() - 1] + tol {
                return Err(ClosureError::OutOfRange(a));
            }
            if l.len() == 1 {
                continue;
            }
            let mut i = 0;
            while i + 2 < l.len() && v[a] > l[i + 1] {
                i += 1;
            }
            lo[a] = i;
            w[a] = ((v[a] - l[i]) / (l[i + 1] - l[i])).clamp(0.0, 1.0);
        }
        let mut acc = [0.0f64; 4];
        for corner in 0..16usize {
            let mut idx = [0usize; 4];
            let mut weight = 1.0;
            for a in 0..4 {
                let up = (corner >> a) & 1 == 1;
                if self.levels[a].len() == 1 {
                    if up {
                        weight = 0.0;
                    }
                    continue;
                }
                idx[a] = lo[a] + usize::from(up);
                weight *= if up { w[a] } else { 1.0 - w[a] };
            }
            if weight == 0.0 {
                continue;
            }
            let val = self.values[Self::flat(&self.levels, &idx)];
            for k in 0..4 {
                acc[k] += weight * val[k];
            }
        }
        Ok(Closures::from_ln(acc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multifidelity::double_pipe::design::{design_grid, Split};

    /// A synthetic closure that is exactly log-multilinear (a product of
    /// powers): the table must reproduce it to round-off everywhere inside,
    /// nodes and held-out points alike. This is an analytic check of the
    /// interpolator, not of any physics.
    fn synthetic(x: &DoublePipeInput) -> Closures {
        let [rh, rc, k, l] = x.to_si();
        let r = |v: f64| Ratio::new::<ratio>(v);
        Closures { nu_hot: r(2.0 * rh.powf(0.3) * l.powf(-0.2)), nu_cold: r(3.0 * rc.powf(0.25) * k.powf(0.01)), loss_hot: r(64.0 * rh.powf(0.05)), loss_cold: r(95.0 * (rc * l).powf(0.02)) }
    }

    #[test]
    fn the_table_is_exact_on_a_log_multilinear_closure_and_refuses_held_out_points() {
        let g = design_grid();
        let train: Vec<_> = g.iter().filter(|p| p.split == Split::Training).map(|p| (p.split, p.input, synthetic(&p.input))).collect();
        let t = ClosureTable::build(&train).unwrap();
        assert_eq!(t.levels_si()[0].len(), 3);
        assert_eq!(t.levels_si()[3].len(), 2);
        for p in &g {
            let c = t.at(&p.input).unwrap();
            let e = synthetic(&p.input);
            for (a, b) in [(c.nu_hot, e.nu_hot), (c.nu_cold, e.nu_cold), (c.loss_hot, e.loss_hot), (c.loss_cold, e.loss_cold)] {
                assert!((a.get::<ratio>() / b.get::<ratio>() - 1.0).abs() < 1e-12, "{} {:?} {:?}", p.id, a, b);
            }
        }
        // A held-out point may not build the table.
        let held = g.iter().find(|p| p.split == Split::HeldOut).unwrap();
        let mut bad = train.clone();
        bad.push((Split::HeldOut, held.input, synthetic(&held.input)));
        assert_eq!(ClosureTable::build(&bad), Err(ClosureError::HeldOutOffered));
        // Not a factorial.
        assert!(matches!(ClosureTable::build(&train[1..]), Err(ClosureError::NotFactorial(_))));
        let mut dup = train.clone();
        dup[1] = dup[0];
        assert!(matches!(ClosureTable::build(&dup), Err(ClosureError::NotFactorial(_))));
        // No extrapolation.
        assert_eq!(t.at(&DoublePipeInput::si(5.0, 15.0, 10.0, 0.2)), Err(ClosureError::OutOfRange(0)));
        assert_eq!(t.at(&DoublePipeInput::si(30.0, 15.0, 10.0, 0.5)), Err(ClosureError::OutOfRange(3)));
        for e in [ClosureError::HeldOutOffered, ClosureError::NotFactorial("x".into()), ClosureError::OutOfRange(1)] {
            assert!(!e.to_string().is_empty());
        }
    }

    #[test]
    fn closures_from_cfd_reproduce_the_pressure_drop_through_tuas_law() {
        // B = (2 Δp / ρ U²) Re, and TUAS's Bejan relation Δp = μ² Be / (ρ D²)
        // with Be = ½ (B / Re) Re² must return Δp.
        use uom::si::f64::{Length, Power, Pressure, ThermalConductance, ThermodynamicTemperature};
        let x = DoublePipeInput::si(30.0, 15.0, 10.0, 0.2);
        let y = DoublePipeOutput {
            ua: ThermalConductance::new::<uom::si::thermal_conductance::watt_per_kelvin>(1.0),
            q: Power::new::<uom::si::power::watt>(1.0),
            effectiveness: Ratio::new::<ratio>(0.5),
            t_hot_out: ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(330.0),
            t_cold_out: ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(320.0),
            dp_hot: Pressure::new::<pascal>(0.25),
            dp_cold: Pressure::new::<pascal>(0.125),
            nu_hot: Ratio::new::<ratio>(5.0),
            nu_cold: Ratio::new::<ratio>(6.0),
        };
        let c = Closures::from_cfd(&x, &y);
        let mu = fixed::NU * fixed::RHO;
        for (b, re, d, dp) in [(c.loss_hot, 30.0, fixed::d_hot(), 0.25), (c.loss_cold, 15.0, fixed::d_cold(), 0.125)] {
            let be = 0.5 * b.get::<ratio>() / re * re * re;
            let back = mu * mu * be / (fixed::RHO * d * d);
            assert!((back / dp - 1.0).abs() < 1e-12, "{back} vs {dp}");
        }
        let _ = Length::new::<uom::si::length::meter>(0.0);
    }
}
