//! Windowed-multipole evaluation with analytic Doppler broadening:
//! [`WindowedMultipole::evaluate`], the Faddeeva function, the Weideman
//! rational approximation, and the broadened curve-fit polynomial factors.
//!
//! Split out of `wmp.rs` (see the module doc there for provenance and status).

use super::types::{Cf64, WindowedMultipole, WmpXs};
use super::{CH_ABSORPTION, CH_FISSION, CH_SCATTER, K_BOLTZMANN, SQRT_PI};
use std::sync::OnceLock;

impl WindowedMultipole {
    /// Evaluate σ_s / σ_a / σ_f at incident energy `e_ev` \[eV\] and temperature
    /// `temp_k` \[K\] via the windowed-multipole sum with analytic Doppler
    /// broadening. Faithful re-implementation of OpenMC `WindowedMultipole::evaluate`.
    ///
    /// `temp_k == 0` uses the 0 K asymptotic pole form (no Faddeeva call).
    ///
    /// Outside `[e_min, e_max]` the multipole form is invalid; the caller must
    /// fall back to a pointwise high-energy tail (see `docs/architecture.md`).
    pub fn evaluate(&self, e_ev: f64, temp_k: f64) -> WmpXs {
        let sqrt_e = e_ev.sqrt();
        let inv_e = 1.0 / e_ev;
        let sqrt_kt = (K_BOLTZMANN * temp_k).sqrt();

        // Locate the window containing this energy (clamped to the valid range).
        let raw = ((sqrt_e - self.e_min.sqrt()) * self.inv_spacing) as isize;
        let i_window = raw.clamp(0, self.windows.len() as isize - 1) as usize;
        let window = self.windows[i_window];

        let mut sig_s = 0.0;
        let mut sig_a = 0.0;
        let mut sig_f = 0.0;
        let n_coeff = self.fit_order + 1;

        // -- Curve-fit background -------------------------------------------------
        if sqrt_kt > 0.0 && window.broaden_poly {
            let dopp = self.awr.sqrt() / sqrt_kt;
            let factors = broaden_wmp_polynomials(e_ev, dopp, n_coeff);
            for i in 0..n_coeff {
                let cf = self.curvefit[i_window][i];
                sig_s += cf[CH_SCATTER] * factors[i];
                sig_a += cf[CH_ABSORPTION] * factors[i];
                if self.fissionable {
                    sig_f += cf[CH_FISSION] * factors[i];
                }
            }
        } else {
            // Evaluate the polynomial a/E + b/√E + c + d·√E + … directly.
            let mut term = inv_e;
            for i in 0..n_coeff {
                let cf = self.curvefit[i_window][i];
                sig_s += cf[CH_SCATTER] * term;
                sig_a += cf[CH_ABSORPTION] * term;
                if self.fissionable {
                    sig_f += cf[CH_FISSION] * term;
                }
                term *= sqrt_e;
            }
        }

        // -- Pole contributions in this window -----------------------------------
        if !window.is_empty() {
            if sqrt_kt == 0.0 {
                // 0 K asymptotic form: ψχ = −i / (pole − √E).
                for i in window.start..=window.end {
                    let d = self.poles[i].sub(Cf64::new(sqrt_e, 0.0));
                    let c_temp = Cf64::new(0.0, -1.0).div(d).scale(inv_e);
                    let r = &self.residues[i];
                    sig_s += r[CH_SCATTER].mul(c_temp).re;
                    sig_a += r[CH_ABSORPTION].mul(c_temp).re;
                    if self.fissionable {
                        sig_f += r[CH_FISSION].mul(c_temp).re;
                    }
                }
            } else {
                // Temperature-dependent Faddeeva form.
                let dopp = self.awr.sqrt() / sqrt_kt;
                for i in window.start..=window.end {
                    let z = Cf64::new(sqrt_e, 0.0).sub(self.poles[i]).scale(dopp);
                    let w = faddeeva(z).scale(dopp * inv_e * SQRT_PI);
                    let r = &self.residues[i];
                    sig_s += r[CH_SCATTER].mul(w).re;
                    sig_a += r[CH_ABSORPTION].mul(w).re;
                    if self.fissionable {
                        sig_f += r[CH_FISSION].mul(w).re;
                    }
                }
            }
        }

        WmpXs {
            scatter: sig_s,
            absorption: sig_a,
            fission: sig_f,
        }
    }
}

/// Doppler-broaden the windowed-multipole curve-fit polynomial.
///
/// Returns the `n` leading factors `f_k` so the broadened background is
/// `Σ_k curvefit_k · f_k`. Faithful port of OpenMC `broaden_wmp_polynomials`
/// (Josey et al., *J. Comput. Phys.* 2016, Eq. 16). `dopp = √(awr / kT)`.
pub(super) fn broaden_wmp_polynomials(e: f64, dopp: f64, n: usize) -> Vec<f64> {
    let sqrt_e = e.sqrt();
    let beta = sqrt_e * dopp;
    let half_inv_dopp2 = 0.5 / (dopp * dopp);
    let quarter_inv_dopp4 = half_inv_dopp2 * half_inv_dopp2;

    // erf(6) is 1 and β/√π·e^{−β²} ≈ 0 to machine precision — skip the specials.
    let (erf_beta, exp_m_beta2) = if beta > 6.0 {
        (1.0, 0.0)
    } else {
        (erf(beta), (-beta * beta).exp())
    };

    let mut f = vec![0.0; n];
    f[0] = erf_beta / e;
    if n > 1 {
        f[1] = 1.0 / sqrt_e;
    }
    if n > 2 {
        f[2] = f[0] * (half_inv_dopp2 + e) + exp_m_beta2 / (beta * SQRT_PI);
    }
    if n > 3 {
        f[3] = f[1] * (e + 3.0 * half_inv_dopp2);
    }
    // Recursive broadening of higher-order components (Eq. 16).
    for i in 1..n.saturating_sub(3) {
        let ip1 = (i + 1) as f64;
        f[i + 3] = -f[i - 1] * (ip1 - 1.0) * ip1 * quarter_inv_dopp4
            + f[i + 1] * (e + (1.0 + 2.0 * ip1) * half_inv_dopp2);
    }
    f
}

/// Faddeeva function in the **pole-representation (integral) form** used by the
/// multipole sum — matches OpenMC's `faddeeva(z)` wrapper, not the raw `w(z)`.
///
/// For the multipole equations we need
/// `w_int(z) = (i/π) ∫ e^{−t²}/(z−t) dt` (Hwang 1987, Eq. 63), which relates to
/// the standard Faddeeva function `w(z) = e^{−z²}·erfc(−i z)` by
/// `w_int(z) = w(z)` for `Im z > 0` and `w_int(z) = −conj(w(conj z))` otherwise.
/// The underlying `w` is evaluated with a pure-Rust Weideman rational
/// approximation ([`w_standard`]); **no FFI** (workspace rule).
pub fn faddeeva(z: Cf64) -> Cf64 {
    if z.im > 0.0 {
        w_standard(z)
    } else {
        w_standard(z.conj()).conj().neg()
    }
}

/// Standard Faddeeva function `w(z) = e^{−z²}·erfc(−i z)`, accurate in the closed
/// **upper half-plane** (`Im z ≥ 0`) — which is all [`faddeeva`] ever needs.
///
/// Weideman's rational approximation (J. A. C. Weideman, *SIAM J. Numer. Anal.*
/// 31 (1994) 1497–1518): `w(z) ≈ 2·p(Z)/(L−i z)² + (1/√π)/(L−i z)` with
/// `Z = (L+i z)/(L−i z)` and `p` a polynomial whose coefficients are the real
/// FFT of a fixed generating sequence (see [`weideman_coeffs`]).
pub(super) fn w_standard(z: Cf64) -> Cf64 {
    let coeffs = weideman_coeffs();
    let l = weideman_l();
    let iz = z.mul_i();
    let l_re = Cf64::new(l, 0.0);
    let denom = l_re.sub(iz); // L − i z
    let big_z = l_re.add(iz).div(denom); // (L + i z)/(L − i z)

    // Horner over the coefficients (coeffs[0] is the highest-degree term).
    let mut p = Cf64::new(coeffs[0], 0.0);
    for &c in &coeffs[1..] {
        p = p.mul(big_z).add(Cf64::new(c, 0.0));
    }

    let denom2 = denom.mul(denom);
    p.scale(2.0)
        .div(denom2)
        .add(Cf64::new(1.0 / SQRT_PI, 0.0).div(denom))
}

/// Number of terms in the Weideman rational approximation. 48 gives ≳ 1e-12
/// accuracy across the upper half-plane, ample for cross-section evaluation.
///
/// `pub(crate)` so the WGSL GPU port ([`crate::gpu_wmp`]) uses the identical
/// term count and coefficient table as the trusted CPU reference.
pub(crate) const WEIDEMAN_N: usize = 48;

/// Weideman's optimal scaling parameter `L = (N/√2)^{1/2}`.
///
/// `pub(crate)` so the GPU port uploads the identical `L` the CPU uses.
pub(crate) fn weideman_l() -> f64 {
    (WEIDEMAN_N as f64 / std::f64::consts::SQRT_2).sqrt()
}

/// The `N` Weideman coefficients, computed once and cached.
///
/// They are `real(fft(fftshift(f)))` of the generating sequence
/// `f_k = e^{−t_k²}·(L² + t_k²)`, `t_k = L·tan(θ_k/2)`, reordered so index 0 is
/// the highest-degree polynomial term. Computed via a direct DFT (one-off,
/// `O(N²)`) so the derivation stays visible rather than hidden in magic numbers.
///
/// `pub(crate)` so the GPU port ([`crate::gpu_wmp`]) uploads this exact
/// (host-computed, `f64`) table to the GPU as `f32`, guaranteeing the WGSL
/// Faddeeva uses the same polynomial as the CPU reference.
pub(crate) fn weideman_coeffs() -> &'static [f64] {
    static COEFFS: OnceLock<Vec<f64>> = OnceLock::new();
    COEFFS.get_or_init(|| {
        let n = WEIDEMAN_N;
        let l = weideman_l();
        let m = 2 * n;
        let m2 = 2 * m; // sequence length = 4N
        let pi = std::f64::consts::PI;

        // Generating sequence f (length m2): f[0] = 0; for idx = 1..m2, k = idx − m.
        let mut f = vec![0.0f64; m2];
        for (idx, slot) in f.iter_mut().enumerate().skip(1) {
            let k = idx as f64 - m as f64;
            let theta = k * pi / m as f64;
            let t = l * (theta / 2.0).tan();
            *slot = (-t * t).exp() * (l * l + t * t);
        }

        // fftshift: rotate by half the length.
        let mut fs = vec![0.0f64; m2];
        for (i, slot) in fs.iter_mut().enumerate() {
            *slot = f[(i + m) % m2];
        }

        // Real part of the DFT at bins 1..=N, then reverse (flipud) so the
        // highest-degree coefficient is first.
        let mut coeffs = vec![0.0f64; n];
        for j in 1..=n {
            let mut re = 0.0;
            for (i, &v) in fs.iter().enumerate() {
                re += v * (-2.0 * pi * i as f64 * j as f64 / m2 as f64).cos();
            }
            coeffs[n - j] = re / m2 as f64;
        }
        coeffs
    })
}

/// Error function `erf(x)` for real `x`, pure-Rust via the Faddeeva relation
/// `erfc(x) = e^{−x²}·Re w(i x)` for `x ≥ 0` (no libm / FFI).
pub(super) fn erf(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let ax = x.abs();
    let erfc = (-ax * ax).exp() * w_standard(Cf64::new(0.0, ax)).re;
    x.signum() * (1.0 - erfc)
}
