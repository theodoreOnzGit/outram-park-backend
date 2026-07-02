//! Windowed Multipole (WMP) cross sections — analytic on-the-fly Doppler broadening.
//!
//! # Provenance — read first (this is NOT NJOY / NOT LANL)
//!
//! Windowed Multipole is **independent of NJOY2016 and of LANL**. It is the work
//! of the **MIT Computational Reactor Physics Group (CRPG)**. Nothing in this
//! module derives from the NJOY BSD/LANL sources, and its attribution must be
//! kept separate from the crate's NJOY `LICENSE.njoy` / `NOTICE` files.
//!
//! - **Data library:** <https://github.com/mit-crpg/WMP_Library> — © MIT CRPG,
//!   distributed under the **MIT License** (GPL-compatible, so it may coexist
//!   with this crate's GPL-3.0 licensing).
//! - **Method:** the multipole representation of R-matrix cross sections
//!   (R. N. Hwang, *Nucl. Sci. Eng.* 1987) with the windowing/curve-fit scheme
//!   of **C. Josey, P. Romano, B. Forget, K. Smith** (*J. Comput. Phys.* 2016;
//!   *Ann. Nucl. Energy* 2015/2016). Credit MIT CRPG and these authors in any
//!   derived work; do **not** imply LANL endorsement.
//!
//! **Before importing any WMP *data* into this crate, add a separate
//! `LICENSE-WMP` (the upstream MIT text) and a NOTICE entry crediting MIT CRPG.**
//! (The *algorithm* below is an independent re-implementation; the reference is
//! OpenMC's MIT-licensed `src/wmp.cpp`, at `/home/teddy0/Documents/research/openmc/`.)
//!
//! # Role in OUTRAM PARK
//!
//! Per the workspace architecture (`docs/architecture.md`), **all nuclear-data
//! representation lives in this crate**; `openmc-libs` pulls cross sections from
//! here rather than owning any. WMP is the *low-fidelity, in-crate* data format:
//! compact enough to embed (KB–MB/nuclide) and — crucially — Doppler-broadenable
//! analytically, so it serves both OUTRAM PARK priorities:
//!
//! 1. **U-238 (n,γ) Doppler** — σ(E, T) is a closed-form evaluation of the pole
//!    sum via the Faddeeva function [`faddeeva`]; no per-temperature pointwise
//!    library. Compared against njoy's own BROADR kernel and an OpenMC `.h5`.
//! 2. **Bare-sphere Keff** — the cross-section magnitudes; ν̄/χ come from
//!    [`crate::nuclear_data::secondary`].
//!
//! # Status
//!
//! The evaluator ([`WindowedMultipole::evaluate`]) and the analytic Doppler
//! kernel ([`faddeeva`]) are **implemented** and unit-tested; they are faithful
//! re-implementations of OpenMC `WindowedMultipole::evaluate` / `faddeeva`.
//! [`WindowedMultipole::load_h5`] reads real `WMP_Library` HDF5 files (behind the
//! `wmp-hdf5` feature) — see `tests/wmp_u238.rs`. Still scaffold: the embedded
//! zero-dependency shipping path [`WindowedMultipole::from_blob`] — no WMP blob
//! ships in-crate yet.

use crate::NjoyError;
use std::sync::OnceLock;

/// Boltzmann constant in \[eV/K\] (so `kT` in eV = `K_BOLTZMANN * T[K]`).
const K_BOLTZMANN: f64 = 8.617_333_262e-5;

/// √π — appears in every Doppler-broadened pole term.
const SQRT_PI: f64 = 1.772_453_850_905_516;

// Residue / curve-fit channel order (matches OpenMC `wmp.h`: RS, RA, RF).
const CH_SCATTER: usize = 0;
const CH_ABSORPTION: usize = 1;
const CH_FISSION: usize = 2;

/// Minimal complex number for the multipole sum. (The WMP inner loop needs only
/// a handful of operations; a full complex-number dependency is overkill.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cf64 {
    pub re: f64,
    pub im: f64,
}

impl Cf64 {
    /// Construct `re + i·im`.
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    /// Complex addition.
    pub fn add(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re + o.re, self.im + o.im)
    }
    /// Complex subtraction `self − o`.
    pub fn sub(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re - o.re, self.im - o.im)
    }
    /// Complex multiplication.
    pub fn mul(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    /// Complex division `self / o`.
    pub fn div(self, o: Cf64) -> Cf64 {
        let d = o.re * o.re + o.im * o.im;
        Cf64::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    /// Scale by a real factor.
    pub fn scale(self, s: f64) -> Cf64 {
        Cf64::new(self.re * s, self.im * s)
    }
    /// Complex conjugate `re − i·im`.
    pub fn conj(self) -> Cf64 {
        Cf64::new(self.re, -self.im)
    }
    /// Negation `−self`.
    pub fn neg(self) -> Cf64 {
        Cf64::new(-self.re, -self.im)
    }
    /// Multiply by the imaginary unit: `i·z = −im + i·re`.
    pub fn mul_i(self) -> Cf64 {
        Cf64::new(-self.im, self.re)
    }
}

/// The three base cross sections a windowed-multipole evaluation yields, in barn.
///
/// The multipole residues carry **scattering**, **absorption**, and **fission**
/// directly (this is the library's channel layout). Total is their sensible sum
/// `scatter + absorption`; radiative capture is `absorption − fission`. ν̄ is
/// intentionally absent — WMP carries no secondary data. Combine with
/// [`crate::nuclear_data::secondary::NuBar`] to form the full
/// [`crate::nuclear_data::MicroXs`] the transport kernel consumes.
#[derive(Debug, Clone, Copy, Default)]
pub struct WmpXs {
    /// Elastic scattering σ_s(E, T) \[barn\].
    pub scatter: f64,
    /// Absorption (capture + fission) σ_a(E, T) \[barn\].
    pub absorption: f64,
    /// Fission σ_f(E, T) \[barn\] (0 for non-fissile nuclides).
    pub fission: f64,
}

impl WmpXs {
    /// Total microscopic cross section σ_t = σ_s + σ_a \[barn\].
    pub fn total(&self) -> f64 {
        self.scatter + self.absorption
    }
    /// Radiative capture σ_γ = σ_a − σ_f \[barn\] — the U-238 Doppler target.
    pub fn capture(&self) -> f64 {
        (self.absorption - self.fission).max(0.0)
    }
}

/// Base reaction channels carried by the WMP residues, in library order.
#[derive(Debug, Clone, Copy)]
pub enum WmpReaction {
    Scatter = 0,
    Absorption = 1,
    Fission = 2,
}

/// One energy window in `√E` space: the inclusive range of pole indices that
/// contribute inside it, plus whether its curve-fit background is Doppler-broadened.
#[derive(Debug, Clone, Copy)]
pub struct WmpWindow {
    /// First pole index contributing to this window (inclusive).
    pub start: usize,
    /// Last pole index contributing to this window (inclusive).
    pub end: usize,
    /// Whether the curve-fit polynomial is Doppler-broadened (vs. evaluated raw).
    pub broaden_poly: bool,
}

impl WmpWindow {
    /// A window with no poles (`end < start`) contributes only its curve-fit.
    fn is_empty(&self) -> bool {
        self.end < self.start
    }
}

/// Windowed-multipole representation of one nuclide's resonance-range cross
/// sections. Field layout mirrors the MIT `WMP_Library` HDF5.
///
/// The evaluation splits the `√E` axis into equal-spacing **windows**; within a
/// window the cross section is a smooth **curve-fit** polynomial background plus
/// the Faddeeva contribution of the poles assigned to that window. Temperature
/// enters only through the Faddeeva argument, giving analytic Doppler broadening.
#[derive(Debug, Clone, Default)]
pub struct WindowedMultipole {
    /// Nuclide name, e.g. `"U238"`.
    pub name: String,
    /// Atomic weight ratio (target mass / neutron mass) — sets the Doppler width.
    pub awr: f64,
    /// Lower / upper energy bounds of the multipole representation \[eV\].
    pub e_min: f64,
    pub e_max: f64,
    /// Whether this nuclide carries fission residues (the third channel).
    pub fissionable: bool,
    /// Poles (complex, in `√E` space), one per resonance-like term.
    pub poles: Vec<Cf64>,
    /// Residues per pole per base reaction, order `[scatter, absorption, fission]`.
    pub residues: Vec<[Cf64; 3]>,
    /// Curve-fit coefficients `[window][poly_order][channel]`, channel order
    /// `[scatter, absorption, fission]`.
    pub curvefit: Vec<Vec<[f64; 3]>>,
    /// The energy windows (pole ranges + broaden flag), ascending in `√E`.
    pub windows: Vec<WmpWindow>,
    /// Inverse window spacing in `√E`.
    pub inv_spacing: f64,
    /// Curve-fit polynomial order (number of coefficients = `fit_order + 1`).
    pub fit_order: usize,
}

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

        WmpXs { scatter: sig_s, absorption: sig_a, fission: sig_f }
    }

    /// Load a nuclide from an MIT CRPG `WMP_Library` HDF5 file.
    ///
    /// Reads the single nuclide group (e.g. `/U238/`) and its datasets — `data`
    /// (complex poles + residues), `curvefit`, `windows` (1-based → 0-based),
    /// `broaden_poly`, and the `E_min`/`E_max`/`spacing`/`sqrtAWR` scalars — per
    /// the WMP format spec (`WMP_Library/wmp_format.md`). Uses `hdf5-pure`
    /// (pure-Rust, no system `libhdf5`). For the *embedded* offline path (no HDF5
    /// dependency in the shipped build), prefer [`Self::from_blob`].
    ///
    /// Requires the `wmp-hdf5` feature. **License:** this reads MIT CRPG data —
    /// ship `LICENSE-WMP` + a NOTICE credit before embedding any of it.
    ///
    /// # Errors
    /// [`NjoyError::Io`] if the file cannot be read; [`NjoyError::WmpData`] if a
    /// dataset is missing or has an unexpected shape.
    #[cfg(feature = "wmp-hdf5")]
    pub fn load_h5<P: AsRef<std::path::Path>>(path: P) -> Result<Self, NjoyError> {
        use hdf5_pure::File;
        let err = |m: String| NjoyError::WmpData(m);

        let bytes = std::fs::read(path)?;
        let file = File::from_bytes(bytes).map_err(|e| err(format!("open: {e}")))?;

        // The one top-level group is the nuclide (e.g. "U238").
        let name = file
            .root()
            .groups()
            .map_err(|e| err(format!("list groups: {e}")))?
            .into_iter()
            .next()
            .ok_or_else(|| err("no nuclide group in WMP file".into()))?;

        let scalar = |ds: &str| -> Result<f64, NjoyError> {
            let v = file
                .dataset(&format!("/{name}/{ds}"))
                .and_then(|d| d.read_f64())
                .map_err(|e| err(format!("{ds}: {e}")))?;
            v.first().copied().ok_or_else(|| err(format!("{ds} empty")))
        };

        let e_min = scalar("E_min")?;
        let e_max = scalar("E_max")?;
        let spacing = scalar("spacing")?;
        let sqrt_awr = scalar("sqrtAWR")?;

        // `data`: (n_poles, n_cols) complex; col 0 = pole, cols 1.. = residues
        // (scatter, absorption, [fission]). Read raw and decode f64 (r,i) pairs.
        let data_ds = file
            .dataset(&format!("/{name}/data"))
            .map_err(|e| err(format!("data: {e}")))?;
        let shape = data_ds.shape().map_err(|e| err(format!("data shape: {e}")))?;
        if shape.len() != 2 {
            return Err(err(format!("data rank {} != 2", shape.len())));
        }
        let (n_poles, n_cols) = (shape[0] as usize, shape[1] as usize);
        let fissionable = n_cols >= 4;
        let raw = data_ds.read_raw().map_err(|e| err(format!("data raw: {e}")))?;
        if raw.len() != n_poles * n_cols * 16 {
            return Err(err(format!(
                "data bytes {} != {}",
                raw.len(),
                n_poles * n_cols * 16
            )));
        }
        let cx = |elem: usize| -> Cf64 {
            let b = elem * 16;
            let re = f64::from_le_bytes(raw[b..b + 8].try_into().unwrap());
            let im = f64::from_le_bytes(raw[b + 8..b + 16].try_into().unwrap());
            Cf64::new(re, im)
        };
        let mut poles = Vec::with_capacity(n_poles);
        let mut residues = Vec::with_capacity(n_poles);
        for p in 0..n_poles {
            let base = p * n_cols;
            poles.push(cx(base));
            let scatter = cx(base + 1);
            let absorption = cx(base + 2);
            let fission = if fissionable { cx(base + 3) } else { Cf64::new(0.0, 0.0) };
            residues.push([scatter, absorption, fission]);
        }

        // `curvefit`: (n_windows, fit_order+1, 3) f64, row-major.
        let cf_ds = file
            .dataset(&format!("/{name}/curvefit"))
            .map_err(|e| err(format!("curvefit: {e}")))?;
        let cf_shape = cf_ds.shape().map_err(|e| err(format!("curvefit shape: {e}")))?;
        if cf_shape.len() != 3 || cf_shape[2] != 3 {
            return Err(err(format!("curvefit shape {cf_shape:?} unexpected")));
        }
        let n_windows = cf_shape[0] as usize;
        let n_coeff = cf_shape[1] as usize;
        let fit_order = n_coeff - 1;
        let cf_flat = cf_ds.read_f64().map_err(|e| err(format!("curvefit read: {e}")))?;
        let mut curvefit = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let mut coeffs = Vec::with_capacity(n_coeff);
            for c in 0..n_coeff {
                let o = (w * n_coeff + c) * 3;
                coeffs.push([cf_flat[o], cf_flat[o + 1], cf_flat[o + 2]]);
            }
            curvefit.push(coeffs);
        }

        // `windows`: (n_windows, 2) i32, 1-based inclusive pole indices.
        let win_flat = file
            .dataset(&format!("/{name}/windows"))
            .and_then(|d| d.read_i32())
            .map_err(|e| err(format!("windows: {e}")))?;
        // `broaden_poly`: (n_windows,) i8 (1 = broaden the curve-fit).
        let broaden = file
            .dataset(&format!("/{name}/broaden_poly"))
            .and_then(|d| d.read_i8())
            .map_err(|e| err(format!("broaden_poly: {e}")))?;
        if win_flat.len() != 2 * n_windows || broaden.len() != n_windows {
            return Err(err("windows / broaden_poly length mismatch".into()));
        }
        let mut windows = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let (s1, e1) = (win_flat[2 * w], win_flat[2 * w + 1]);
            // Map to 0-based; represent an empty window (e1 < s1) as end < start.
            let (start, end) = if e1 < s1 {
                (1usize, 0usize)
            } else {
                ((s1 - 1) as usize, (e1 - 1) as usize)
            };
            windows.push(WmpWindow { start, end, broaden_poly: broaden[w] != 0 });
        }

        Ok(WindowedMultipole {
            name,
            awr: sqrt_awr * sqrt_awr,
            e_min,
            e_max,
            fissionable,
            poles,
            residues,
            curvefit,
            windows,
            inv_spacing: 1.0 / spacing,
            fit_order,
        })
    }

    /// Load a nuclide from a `WMP_Library` HDF5 file — requires the `wmp-hdf5`
    /// feature (adds the pure-Rust `hdf5-pure` reader). Without it, use the
    /// embedded-blob path [`Self::from_blob`].
    ///
    /// # Errors
    /// Returns [`NjoyError::NotPorted`] when built without `wmp-hdf5`.
    #[cfg(not(feature = "wmp-hdf5"))]
    pub fn load_h5<P: AsRef<std::path::Path>>(_path: P) -> Result<Self, NjoyError> {
        Err(NjoyError::NotPorted(
            "WMP HDF5 import needs the `wmp-hdf5` feature (hdf5-pure)",
        ))
    }

    /// Decode a nuclide from a compact embedded blob (the in-crate delivery path).
    ///
    /// A maintainer bakes the MIT `WMP_Library` HDF5 into a zstd blob offline;
    /// this crate `include_bytes!`s it and decodes here — no HDF5 dependency in
    /// the shipped build. See `docs/architecture.md` for the blob schema.
    ///
    /// # Errors
    /// Always returns [`NjoyError::NotPorted`].
    pub fn from_blob(_bytes: &[u8]) -> Result<Self, NjoyError> {
        Err(NjoyError::NotPorted("WMP embedded-blob decode"))
    }
}

/// Doppler-broaden the windowed-multipole curve-fit polynomial.
///
/// Returns the `n` leading factors `f_k` so the broadened background is
/// `Σ_k curvefit_k · f_k`. Faithful port of OpenMC `broaden_wmp_polynomials`
/// (Josey et al., *J. Comput. Phys.* 2016, Eq. 16). `dopp = √(awr / kT)`.
fn broaden_wmp_polynomials(e: f64, dopp: f64, n: usize) -> Vec<f64> {
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
fn w_standard(z: Cf64) -> Cf64 {
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
const WEIDEMAN_N: usize = 48;

/// Weideman's optimal scaling parameter `L = (N/√2)^{1/2}`.
fn weideman_l() -> f64 {
    (WEIDEMAN_N as f64 / std::f64::consts::SQRT_2).sqrt()
}

/// The `N` Weideman coefficients, computed once and cached.
///
/// They are `real(fft(fftshift(f)))` of the generating sequence
/// `f_k = e^{−t_k²}·(L² + t_k²)`, `t_k = L·tan(θ_k/2)`, reordered so index 0 is
/// the highest-degree polynomial term. Computed via a direct DFT (one-off,
/// `O(N²)`) so the derivation stays visible rather than hidden in magic numbers.
fn weideman_coeffs() -> &'static [f64] {
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
fn erf(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let ax = x.abs();
    let erfc = (-ax * ax).exp() * w_standard(Cf64::new(0.0, ax)).re;
    x.signum() * (1.0 - erfc)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-6;

    #[test]
    fn faddeeva_matches_reference_values() {
        // w(0) = 1.
        let w0 = w_standard(Cf64::new(0.0, 0.0));
        assert!((w0.re - 1.0).abs() < TOL && w0.im.abs() < TOL, "w(0) = {w0:?}");

        // For real x, Re w(x) = e^{−x²} exactly.
        for &x in &[0.5, 1.0, 2.0, 3.0] {
            let w = w_standard(Cf64::new(x, 0.0));
            assert!((w.re - (-x * x).exp()).abs() < TOL, "Re w({x}) = {}", w.re);
        }

        // w(i) = 0.4275835761558070… (real).
        let wi = w_standard(Cf64::new(0.0, 1.0));
        assert!((wi.re - 0.427_583_576_155_807).abs() < TOL, "w(i) = {wi:?}");
        assert!(wi.im.abs() < TOL);

        // w(1+i) = 0.3047442052569126 + 0.2082189382028316 i  (scipy.wofz).
        let w = w_standard(Cf64::new(1.0, 1.0));
        assert!((w.re - 0.304_744_205_256_912_6).abs() < TOL, "Re = {}", w.re);
        assert!((w.im - 0.208_218_938_202_831_6).abs() < TOL, "Im = {}", w.im);
    }

    #[test]
    fn faddeeva_integral_form_reflection() {
        // For Im z > 0 the integral form equals the standard w.
        let z = Cf64::new(0.7, 0.3);
        assert_eq!(faddeeva(z), w_standard(z));
        // For Im z ≤ 0 it is −conj(w(conj z)).
        let z = Cf64::new(0.7, -0.3);
        let expected = w_standard(z.conj()).conj().neg();
        assert_eq!(faddeeva(z), expected);
    }

    #[test]
    fn erf_matches_reference_values() {
        assert!((erf(0.0)).abs() < TOL);
        assert!((erf(0.5) - 0.520_499_877_813_046_5).abs() < TOL);
        assert!((erf(1.0) - 0.842_700_792_949_714_9).abs() < TOL);
        assert!((erf(2.0) - 0.995_322_265_018_952_7).abs() < TOL);
        assert!((erf(-1.0) + 0.842_700_792_949_714_9).abs() < TOL);
    }

    /// A one-pole, one-window synthetic table with an independent 0 K oracle.
    fn single_pole_table() -> WindowedMultipole {
        WindowedMultipole {
            name: "TEST1".into(),
            awr: 235.0,
            e_min: 1.0,
            e_max: 100.0,
            fissionable: false,
            poles: vec![Cf64::new(5.0, 0.05)],
            // scatter, absorption, fission residues.
            residues: vec![[Cf64::new(0.3, 0.1), Cf64::new(0.2, 0.02), Cf64::new(0.0, 0.0)]],
            curvefit: vec![vec![[0.0, 0.0, 0.0]]], // one window, order 0, all zero
            windows: vec![WmpWindow { start: 0, end: 0, broaden_poly: false }],
            inv_spacing: 1.0,
            fit_order: 0,
        }
    }

    #[test]
    fn zero_kelvin_pole_matches_hand_computation() {
        let wmp = single_pole_table();
        let e = 25.0_f64;
        let sqrt_e = e.sqrt();
        let inv_e = 1.0 / e;
        let pole = wmp.poles[0];

        // ψχ = −i / (pole − √E) computed independently with plain f64 arithmetic.
        let dr = pole.re - sqrt_e;
        let di = pole.im;
        let mag2 = dr * dr + di * di;
        // −i / (dr + i·di) = (−di − i·dr) / mag2 · … actually −i·conj / mag2:
        // −i·(dr − i·di) = −di − i·dr ; divide by mag2, then × inv_e.
        let psi_re = (-di / mag2) * inv_e;
        let psi_im = (-dr / mag2) * inv_e;
        // σ_a = Re(residue_a · ψχ).
        let ra = wmp.residues[0][CH_ABSORPTION];
        let expected_a = ra.re * psi_re - ra.im * psi_im;

        let xs = wmp.evaluate(e, 0.0);
        assert!((xs.absorption - expected_a).abs() < 1e-12, "{} vs {}", xs.absorption, expected_a);
        assert_eq!(xs.fission, 0.0);
    }

    #[test]
    fn doppler_broadening_lowers_and_widens_the_peak() {
        // Put a narrow resonance at √E = 5 (E = 25 eV) and check that heating
        // lowers the on-resonance absorption and raises it in the wing.
        let mut wmp = single_pole_table();
        // Physical poles sit in the lower half-plane (Im < 0) so absorption is
        // positive on resonance; narrow width → stronger Doppler effect.
        wmp.poles = vec![Cf64::new(5.0, -0.01)];
        wmp.residues = vec![[Cf64::new(0.0, 0.0), Cf64::new(1.0, 0.0), Cf64::new(0.0, 0.0)]];

        let e_peak = 25.0; // √E = 5, on resonance
        let e_wing = 30.0;

        let cold_peak = wmp.evaluate(e_peak, 300.0).absorption;
        let hot_peak = wmp.evaluate(e_peak, 2500.0).absorption;
        let cold_wing = wmp.evaluate(e_wing, 300.0).absorption;
        let hot_wing = wmp.evaluate(e_wing, 2500.0).absorption;

        assert!(hot_peak < cold_peak, "peak should drop: {hot_peak} !< {cold_peak}");
        assert!(hot_wing > cold_wing, "wing should rise: {hot_wing} !> {cold_wing}");
        // All evaluations finite.
        for v in [cold_peak, hot_peak, cold_wing, hot_wing] {
            assert!(v.is_finite());
        }
    }
}
