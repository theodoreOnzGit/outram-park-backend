// Ported from NJOY2016 `src/acefc.f90` (`fmn`, :9155-9176, and the `lf.eq.12`
// branch of `acelf5`, :7037-7126), git commit
// ac5adf5f33d893e42f2eed7fb286b0d51c7580da.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. Modified, non-LANL
// version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The Madland-Nix fission spectrum — MF=5 `LF = 12`.
//!
//! # What was blocking this, and what unblocked it
//!
//! `acefc.f90:7037-7126` (the branch) was never the hard part: it is an
//! adaptive linearisation of an analytic shape onto an ACE LAW=4 tabulation.
//! The obstacle was `fmn` (`acefc.f90:9155`), which evaluates the shape in
//! terms of the **exponential integral** `E_1` and the **lower incomplete
//! gamma** `gami` — neither of which existed in this workspace, and both of
//! which NJOY takes from SLATEC.
//!
//! They now exist in `petir`, ported from GSL and verified against GSL
//! compiled and run (`E_1`: 96 % bit-identical over 480 points;
//! `gami(3/2, x)`: worst 1.9e-15 relative over 220 points). See GitHub #201.
//!
//! # `gami` is the LOWER incomplete gamma
//!
//! Worth stating because it is a sign trap: SLATEC's `gami(a, x)` is
//! `∫₀ˣ t^(a-1) e^(-t) dt`, the **lower** one, whereas GSL's
//! `gsl_sf_gamma_inc` is the **upper**. [`petir::gamma_inc_lower`] is the
//! lower one, matching SLATEC.

use petir::{expint_e1, gamma_inc_lower};

use crate::NjoyError;

/// `fmn(e2, efl, efh, tm)` — the Madland-Nix spectrum
/// (`acefc.f90:9155-9176`), transcribed:
///
/// ```text
///   u1 = (sqrt(e2)-sqrt(efl))**2/tm
///   u2 = (sqrt(e2)+sqrt(efl))**2/tm
///   g1 = (u2**1.5*e1(u2) - u1**1.5*e1(u1) + gami(1.5,u2) - gami(1.5,u1))
///        / (3*sqrt(efl*tm))
///   ... same for efh -> g2 ...
///   fmn = (g1+g2)/2
/// ```
///
/// `e2` is the outgoing energy, `efl`/`efh` the average fission-fragment
/// kinetic energies per nucleon for the light and heavy fragments, and `tm`
/// the nuclear temperature — all in the same units (eV here).
///
/// # Errors
/// [`NjoyError::NotPorted`] if either special function refuses — see
/// [`petir::gamma_inc_p`]'s documented branch coverage. The message names the
/// argument so the refusal is traceable.
pub fn fmn(e2: f64, efl: f64, efh: f64, tm: f64) -> Result<f64, NjoyError> {
    let g = |ef: f64| -> Result<f64, NjoyError> {
        let s_e2 = e2.sqrt();
        let s_ef = ef.sqrt();
        let u1 = (s_e2 - s_ef).powi(2) / tm;
        let u2 = (s_e2 + s_ef).powi(2) / tm;
        // E_1(0) diverges; upstream reaches u1 = 0 only at e2 == ef, where the
        // u1 terms cancel to zero in the limit. Take that limit rather than
        // propagating a refusal for a point the spectrum is finite at.
        let (e1_u1, gam_u1) = if u1 == 0.0 {
            (0.0, 0.0)
        } else {
            let e1 = expint_e1(u1)
                .map_err(|_| NjoyError::NotPorted("madland_nix: E_1 refused at u1"))?
                .0;
            let gm = gamma_inc_lower(1.5, u1)
                .map_err(|_| NjoyError::NotPorted("madland_nix: gami(1.5, u1) refused"))?
                .0;
            (u1.powf(1.5) * e1, gm)
        };
        let e1_u2 = expint_e1(u2)
            .map_err(|_| NjoyError::NotPorted("madland_nix: E_1 refused at u2"))?
            .0;
        let gam_u2 = gamma_inc_lower(1.5, u2)
            .map_err(|_| NjoyError::NotPorted("madland_nix: gami(1.5, u2) refused"))?
            .0;
        Ok((u2.powf(1.5) * e1_u2 - e1_u1 + gam_u2 - gam_u1) / (3.0 * (ef * tm).sqrt()))
    };
    Ok((g(efl)? + g(efh)?) / 2.0)
}

/// One incident energy's linearised Madland-Nix spectrum: the `(E', f(E'))`
/// pairs that become an ACE LAW=4 tabulation.
#[derive(Debug, Clone)]
pub struct LinearisedSpectrum {
    /// Outgoing energies \[eV\], ascending.
    pub e_out_ev: Vec<f64>,
    /// The normalised spectrum at those energies, `∫ f dE' = 1`.
    pub pdf: Vec<f64>,
}

/// Adaptively linearise `fmn` onto an `E'` grid — `acefc.f90:7057-7102`.
///
/// Upstream primes a three-point stack with `emin`, the incident energy, and
/// `emax`, then bisects: at each step it compares the midpoint of the top
/// interval against `fmn` there and, if they differ by more than
/// `tol*|f| + tmin`, pushes the midpoint and continues; otherwise it retires
/// the top point into the output. The result is renormalised to unit integral
/// (`renorm`, `:7105`).
///
/// # Parameters
/// `efl`/`efh`/`tm` are the Madland-Nix parameters at this incident energy,
/// `e_in_ev` the incident energy (which upstream seeds the stack with), and
/// `emin`/`emax` the bounds of the outgoing grid — all in eV.
///
/// # Errors
/// [`NjoyError::NotPorted`] if [`fmn`] refuses; [`NjoyError::EndfParse`] for a
/// degenerate grid (`emin >= emax`) or if the spectrum integrates to zero.
pub fn linearise(
    e_in_ev: f64,
    efl: f64,
    efh: f64,
    tm: f64,
    emin: f64,
    emax: f64,
    tol: f64,
) -> Result<LinearisedSpectrum, NjoyError> {
    /// `tmin` (`acefc.f90`): the absolute floor on the convergence test, so a
    /// spectrum that is locally near zero does not bisect forever.
    const TMIN: f64 = 1.0e-9;
    /// `ismax` -- upstream's stack bound.
    const ISMAX: usize = 20;
    /// `jsmax` -- upstream's output bound; it CLAMPS rather than failing
    /// (`if (js.gt.jsmax) js=jsmax`, :7096), which silently overwrites the last
    /// point. Reproduced, since a spectrum that long is already pathological.
    const JSMAX: usize = 2000;

    if !(emin < emax) {
        return Err(NjoyError::EndfParse(
            "madland_nix::linearise: need emin < emax".into(),
        ));
    }

    // Prime the stack exactly as upstream does (:7064-7070): top of stack is
    // index 0 here, so the vector is in reverse order relative to Fortran's.
    let mut xs: Vec<f64> = vec![emax, e_in_ev.clamp(emin, emax), emin];
    let mut ys: Vec<f64> = Vec::with_capacity(3);
    for &x in &xs {
        ys.push(fmn(x, efl, efh, tm)?);
    }

    let mut out_x: Vec<f64> = Vec::new();
    let mut out_y: Vec<f64> = Vec::new();
    let mut renorm = 0.0f64;

    while let (Some(&xtop), Some(&ytop)) = (xs.last(), ys.last()) {
        let n = xs.len();
        let mut dy = 0.0f64;
        let mut test = 1.0f64;
        let mut xm = 0.0f64;
        let mut yt = 0.0f64;
        if n > 1 && n < ISMAX {
            xm = (xs[n - 2] + xtop) / 2.0;
            let ym = (ys[n - 2] + ytop) / 2.0;
            yt = fmn(xm, efl, efh, tm)?;
            test = tol * libm_abs(yt) + TMIN;
            dy = libm_abs(yt - ym);
        }
        if dy > test {
            // Not converged: insert the midpoint below the top.
            xs.insert(n - 1, xm);
            ys.insert(n - 1, yt);
        } else {
            // Converged: retire the top point.
            if out_x.len() < JSMAX {
                out_x.push(xtop);
                out_y.push(ytop);
                if out_x.len() > 1 {
                    let k = out_x.len() - 1;
                    renorm += (out_x[k] - out_x[k - 1]) * (out_y[k] + out_y[k - 1]) / 2.0;
                }
            }
            xs.pop();
            ys.pop();
        }
    }

    if !(renorm > 0.0) {
        return Err(NjoyError::EndfParse(
            "madland_nix::linearise: spectrum integrates to zero".into(),
        ));
    }
    let scale = 1.0 / renorm;
    for y in &mut out_y {
        *y *= scale;
    }
    Ok(LinearisedSpectrum {
        e_out_ev: out_x,
        pdf: out_y,
    })
}

#[inline]
fn libm_abs(x: f64) -> f64 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spectrum must be positive and finite across a realistic range, and
    /// must have the shape a fission spectrum has: rise to a single peak,
    /// then decay.
    ///
    /// `efl = efh = 0.5 MeV`, `tm = 1.0 MeV` are representative Madland-Nix
    /// parameters (in eV here).
    #[test]
    fn the_spectrum_is_positive_and_single_peaked() {
        let (efl, efh, tm) = (5.0e5, 5.0e5, 1.0e6);
        let vals: Vec<(f64, f64)> = (1..=60)
            .map(|k| {
                let e2 = f64::from(k) * 2.5e5;
                (e2, fmn(e2, efl, efh, tm).expect("fmn"))
            })
            .collect();
        for &(e2, v) in &vals {
            assert!(v.is_finite() && v > 0.0, "fmn({e2}) = {v}");
        }
        // Locate the maximum, then require monotone behaviour on each side of
        // it -- a fission spectrum rises to one peak and decays, and a kernel
        // with a sign error or a bad branch would wobble.
        let imax = vals
            .iter()
            .enumerate()
            .max_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert!(imax > 0 && imax < vals.len() - 1, "peak at the edge: index {imax}");
        for w in vals[..=imax].windows(2) {
            assert!(w[1].1 >= w[0].1, "not rising before the peak at {}", w[1].0);
        }
        for w in vals[imax..].windows(2) {
            assert!(w[1].1 <= w[0].1, "not decaying after the peak at {}", w[1].0);
        }
        println!("  peak at E' = {:.3e} eV", vals[imax].0);
    }

    /// The linearised spectrum must be normalised, ascending and dense enough
    /// to represent the shape — the three properties an ACE LAW=4 tabulation
    /// has to have for a sampler to be correct.
    #[test]
    fn the_linearised_spectrum_is_normalised_and_ascending() {
        let (efl, efh, tm) = (5.0e5, 5.0e5, 1.0e6);
        let s = linearise(2.0e6, efl, efh, tm, 1.0e3, 2.0e7, 0.01).expect("linearise");
        assert!(s.e_out_ev.len() > 20, "only {} points", s.e_out_ev.len());
        assert_eq!(s.e_out_ev.len(), s.pdf.len());
        for w in s.e_out_ev.windows(2) {
            assert!(w[1] > w[0], "grid not ascending: {} then {}", w[0], w[1]);
        }
        let integral: f64 = s
            .e_out_ev
            .windows(2)
            .zip(s.pdf.windows(2))
            .map(|(x, y)| (x[1] - x[0]) * (y[1] + y[0]) / 2.0)
            .sum();
        assert!(
            (integral - 1.0).abs() < 1.0e-12,
            "not normalised: integral = {integral}"
        );
        println!("  {} points, integral = {integral:.15}", s.e_out_ev.len());
    }

    /// `e2 = efl` drives `u1` to exactly zero, where `E_1` diverges. The limit
    /// is finite and the implementation must take it rather than refuse.
    #[test]
    fn the_u1_equals_zero_limit_is_finite() {
        let v = fmn(5.0e5, 5.0e5, 5.0e5, 1.0e6).expect("fmn at e2 == efl");
        assert!(v.is_finite() && v > 0.0, "{v}");
    }
}
