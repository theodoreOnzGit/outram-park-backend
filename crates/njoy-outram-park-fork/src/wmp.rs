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
//! The CORE data blob (`src/data/wmp_core.wmpl`) is now embedded; its MIT CRPG
//! provenance is credited in `LICENSE-WMP` (upstream MIT text) and `NOTICE`, kept
//! separate from the NJOY attribution. (The *algorithm* below is an independent
//! re-implementation; the reference is OpenMC's MIT-licensed `src/wmp.cpp`, at
//! `/home/teddy0/Documents/research/openmc/`.)
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
//! [`WindowedMultipole::load_h5`] reads real `WMP_Library` HDF5 files
//! (pure-Rust `hdf5-pure`, always available) — see `tests/wmp_u238.rs`. The embedded, zero-dependency
//! shipping path — [`WindowedMultipole::to_blob`] (offline bake) and
//! [`WindowedMultipole::from_blob`] (runtime decode) of the pure-Rust **WMPB v1**
//! format — is **implemented and round-trip tested**. The curated **CORE**
//! 125-nuclide set is baked into `src/data/wmp_core.wmpl` and always embedded,
//! exposed offline via [`WmpLibrary::core`] (no feature gate — it ships in every
//! build); re-bake it with the `bake_wmp` example. See
//! `docs/wmp-nuclide-manifest.md`.

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
    /// **License:** this reads MIT CRPG data —
    /// ship `LICENSE-WMP` + a NOTICE credit before embedding any of it.
    ///
    /// # Errors
    /// [`NjoyError::Io`] if the file cannot be read; [`NjoyError::WmpData`] if a
    /// dataset is missing or has an unexpected shape.
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
        // Last dim = number of channels: 2 (scatter, absorption) for a
        // non-fissionable nuclide, 3 (+ fission) for a fissionable one.
        let cf_shape = cf_ds.shape().map_err(|e| err(format!("curvefit shape: {e}")))?;
        if cf_shape.len() != 3 || (cf_shape[2] != 2 && cf_shape[2] != 3) {
            return Err(err(format!("curvefit shape {cf_shape:?} unexpected")));
        }
        let n_windows = cf_shape[0] as usize;
        let n_coeff = cf_shape[1] as usize;
        let n_ch = cf_shape[2] as usize;
        let fit_order = n_coeff - 1;
        let cf_flat = cf_ds.read_f64().map_err(|e| err(format!("curvefit read: {e}")))?;
        let mut curvefit = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let mut coeffs = Vec::with_capacity(n_coeff);
            for c in 0..n_coeff {
                let o = (w * n_coeff + c) * n_ch;
                let fission = if n_ch >= 3 { cf_flat[o + 2] } else { 0.0 };
                coeffs.push([cf_flat[o], cf_flat[o + 1], fission]);
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

    /// Serialize this nuclide into the compact **WMPB v1** embedded blob — the
    /// exact inverse of [`Self::from_blob`]. This is the offline *bake* step: a
    /// maintainer loads MIT `WMP_Library` HDF5 with [`Self::load_h5`] and writes
    /// the result here; the shipped crate then `include_bytes!`s the blob and
    /// decodes it with [`Self::from_blob`], so **no HDF5 dependency lives in the
    /// runtime build**.
    ///
    /// # Format — WMPB v1 (little-endian)
    /// A plaintext header + index, then a single **deflate** stream of the
    /// pole / residue / curve-fit doubles:
    ///
    /// | Bytes | Field |
    /// |---|---|
    /// | `0..4` | magic `b"WMPB"` |
    /// | `4` | version (`1`) |
    /// | `5` | flags (bit 0 = fissionable) |
    /// | `6..8` | reserved (`0`) |
    /// | `8..40` | `awr`, `e_min`, `e_max`, `inv_spacing` (4×`f64`) |
    /// | `40..56` | `fit_order`, `n_poles`, `n_windows`, `name_len` (4×`u32`) |
    /// | `56..` | name (UTF-8), then `n_windows`×(`start` u32, `end` u32, `broaden` u8) |
    /// | rest | deflate(byte-plane-shuffled doubles) |
    ///
    /// The doubles are grouped by column (all pole real parts, then all
    /// imaginary parts, then each residue/curve-fit channel) and **byte-plane
    /// shuffled** (byte 0 of every value, then byte 1, …) before deflate, so the
    /// low-entropy exponent/sign planes cluster. IEEE mantissa bits are
    /// near-incompressible, so this only buys ~1.15–1.17×, but it is free to
    /// apply. Codec is pure-Rust [`miniz_oxide`] — no C toolchain either way.
    pub fn to_blob(&self) -> Vec<u8> {
        let n_poles = self.poles.len();
        let n_windows = self.windows.len();
        let n_coeff = self.fit_order + 1;

        // Column-grouped doubles — homogeneous streams give deflate more to chew
        // on. Order here MUST match the slicing in `from_blob`.
        let mut vals: Vec<f64> = Vec::with_capacity(n_poles * 8 + n_windows * n_coeff * 3);
        vals.extend(self.poles.iter().map(|p| p.re));
        vals.extend(self.poles.iter().map(|p| p.im));
        for ch in 0..3 {
            vals.extend(self.residues.iter().map(|r| r[ch].re));
            vals.extend(self.residues.iter().map(|r| r[ch].im));
        }
        for ch in 0..3 {
            for win in &self.curvefit {
                for coeff in win {
                    vals.push(coeff[ch]);
                }
            }
        }
        let compressed = miniz_oxide::deflate::compress_to_vec(&shuffle_doubles(&vals), 10);

        let name = self.name.as_bytes();
        let mut out =
            Vec::with_capacity(WMPB_HEADER_LEN + name.len() + n_windows * 9 + compressed.len());
        out.extend_from_slice(&WMPB_MAGIC);
        out.push(WMPB_VERSION);
        out.push(if self.fissionable { WMPB_FLAG_FISSIONABLE } else { 0 });
        out.extend_from_slice(&[0u8, 0u8]); // reserved
        out.extend_from_slice(&self.awr.to_le_bytes());
        out.extend_from_slice(&self.e_min.to_le_bytes());
        out.extend_from_slice(&self.e_max.to_le_bytes());
        out.extend_from_slice(&self.inv_spacing.to_le_bytes());
        out.extend_from_slice(&(self.fit_order as u32).to_le_bytes());
        out.extend_from_slice(&(n_poles as u32).to_le_bytes());
        out.extend_from_slice(&(n_windows as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        // Window table. An empty window is `end < start`; its natural encoding
        // (start = 1, end = 0) already fits u32, so no sentinel is needed.
        for w in &self.windows {
            out.extend_from_slice(&(w.start as u32).to_le_bytes());
            out.extend_from_slice(&(w.end as u32).to_le_bytes());
            out.push(w.broaden_poly as u8);
        }
        out.extend_from_slice(&compressed);
        out
    }

    /// Decode a nuclide from a compact embedded **WMPB v1** blob — the in-crate,
    /// zero-dependency delivery path (no HDF5). Inverse of [`Self::to_blob`],
    /// which documents the byte format.
    ///
    /// Input is treated as untrusted: every length is bounds-checked and the
    /// deflate stream is inflated with a hard cap equal to the exact expected
    /// double-payload size (derived from the header counts), so a malformed blob
    /// fails cleanly rather than runaway-allocating (same discipline as the
    /// crate's 12 GB unit-test cap).
    ///
    /// # Errors
    /// [`NjoyError::WmpData`] on bad magic/version, a truncated header/table,
    /// invalid UTF-8 in the name, a deflate failure, or a size mismatch.
    pub fn from_blob(bytes: &[u8]) -> Result<Self, NjoyError> {
        let err = |m: String| NjoyError::WmpData(m);
        if bytes.len() < WMPB_HEADER_LEN {
            return Err(err("blob shorter than WMPB header".into()));
        }
        if bytes[0..4] != WMPB_MAGIC {
            return Err(err("bad WMPB magic".into()));
        }
        if bytes[4] != WMPB_VERSION {
            return Err(err(format!("unsupported WMPB version {}", bytes[4])));
        }
        let fissionable = bytes[5] & WMPB_FLAG_FISSIONABLE != 0;
        let rd_f64 = |o: usize| f64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
        let rd_u32 = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
        let awr = rd_f64(8);
        let e_min = rd_f64(16);
        let e_max = rd_f64(24);
        let inv_spacing = rd_f64(32);
        let fit_order = rd_u32(40);
        let n_poles = rd_u32(44);
        let n_windows = rd_u32(48);
        let name_len = rd_u32(52);

        // -- Name (UTF-8) --------------------------------------------------------
        let name_end = WMPB_HEADER_LEN
            .checked_add(name_len)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| err("truncated name".into()))?;
        let name = std::str::from_utf8(&bytes[WMPB_HEADER_LEN..name_end])
            .map_err(|e| err(format!("name utf8: {e}")))?
            .to_string();

        // -- Window table: 9 bytes each (u32 start, u32 end, u8 broaden) ---------
        let win_bytes = n_windows.checked_mul(9).ok_or_else(|| err("window overflow".into()))?;
        let win_end = name_end
            .checked_add(win_bytes)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| err("truncated window table".into()))?;
        let mut windows = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let o = name_end + w * 9;
            let start = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
            let end = u32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()) as usize;
            windows.push(WmpWindow { start, end, broaden_poly: bytes[o + 8] != 0 });
        }

        // -- Doubles: know the exact expected count → bound the inflate ----------
        let n_coeff = fit_order.checked_add(1).ok_or_else(|| err("fit_order overflow".into()))?;
        let n_cf = n_windows.checked_mul(n_coeff).ok_or_else(|| err("curvefit overflow".into()))?;
        let n_vals = n_poles
            .checked_mul(8)
            .and_then(|a| n_cf.checked_mul(3).and_then(|b| a.checked_add(b)))
            .ok_or_else(|| err("doubles count overflow".into()))?;
        let expected = n_vals.checked_mul(8).ok_or_else(|| err("doubles size overflow".into()))?;

        let planes =
            miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[win_end..], expected)
                .map_err(|e| err(format!("deflate: {e:?}")))?;
        if planes.len() != expected {
            return Err(err(format!("inflated {} bytes, expected {expected}", planes.len())));
        }
        let vals = unshuffle_doubles(&planes);

        // Slice the columns back out in the order `to_blob` wrote them.
        let mut idx = 0usize;
        let mut take = |n: usize| -> Vec<f64> {
            let s = vals[idx..idx + n].to_vec();
            idx += n;
            s
        };
        let pole_re = take(n_poles);
        let pole_im = take(n_poles);
        let res: Vec<Vec<f64>> = (0..6).map(|_| take(n_poles)).collect();
        let cf: Vec<Vec<f64>> = (0..3).map(|_| take(n_cf)).collect();

        let poles = (0..n_poles).map(|i| Cf64::new(pole_re[i], pole_im[i])).collect();
        let residues = (0..n_poles)
            .map(|i| {
                [
                    Cf64::new(res[0][i], res[1][i]),
                    Cf64::new(res[2][i], res[3][i]),
                    Cf64::new(res[4][i], res[5][i]),
                ]
            })
            .collect();
        let mut curvefit = Vec::with_capacity(n_windows);
        for w in 0..n_windows {
            let mut coeffs = Vec::with_capacity(n_coeff);
            for c in 0..n_coeff {
                let o = w * n_coeff + c;
                coeffs.push([cf[0][o], cf[1][o], cf[2][o]]);
            }
            curvefit.push(coeffs);
        }

        Ok(WindowedMultipole {
            name,
            awr,
            e_min,
            e_max,
            fissionable,
            poles,
            residues,
            curvefit,
            windows,
            inv_spacing,
            fit_order,
        })
    }
}

/// Magic prefix identifying a WMPB (Windowed-Multipole Blob) byte stream.
const WMPB_MAGIC: [u8; 4] = *b"WMPB";
/// WMPB format version encoded/decoded by [`WindowedMultipole::to_blob`] / `from_blob`.
const WMPB_VERSION: u8 = 1;
/// Header flag bit: this nuclide carries fission residues (the third channel).
const WMPB_FLAG_FISSIONABLE: u8 = 0x01;
/// Fixed-size WMPB header before the variable-length name: magic(4) + version(1) +
/// flags(1) + reserved(2) + 4×`f64`(32) + 4×`u32`(16) = 56 bytes.
const WMPB_HEADER_LEN: usize = 56;

/// Byte-plane shuffle of an `f64` stream for the WMPB blob: emit byte 0 of every
/// value, then byte 1 of every value, and so on. Clusters the low-entropy
/// exponent/sign bytes so `deflate` finds more redundancy. Inverse of
/// [`unshuffle_doubles`].
fn shuffle_doubles(vals: &[f64]) -> Vec<u8> {
    let n = vals.len();
    let mut out = vec![0u8; n * 8];
    for (i, v) in vals.iter().enumerate() {
        let b = v.to_le_bytes();
        for k in 0..8 {
            out[k * n + i] = b[k];
        }
    }
    out
}

/// Reassemble the `f64` stream that a [`shuffle_doubles`] byte-plane layout
/// encodes. `planes.len()` must be a multiple of 8 (guaranteed by the caller,
/// which sizes the inflate to `n_vals * 8`).
fn unshuffle_doubles(planes: &[u8]) -> Vec<f64> {
    let n = planes.len() / 8;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut b = [0u8; 8];
        for k in 0..8 {
            b[k] = planes[k * n + i];
        }
        out.push(f64::from_le_bytes(b));
    }
    out
}

/// Magic prefix for a WMPL (Windowed-Multipole Library) container.
const WMPL_MAGIC: [u8; 4] = *b"WMPL";
/// WMPL container version handled by [`WmpLibrary`].
const WMPL_VERSION: u8 = 1;
/// Fixed WMPL header before the index: magic(4) + version(1) + reserved(3) +
/// `n_nuclides`(u32, 4) = 12 bytes.
const WMPL_HEADER_LEN: usize = 12;

/// One directory entry: a nuclide's name and the byte range of its WMPB blob
/// inside the owned container image.
#[derive(Debug, Clone)]
struct WmpEntry {
    name: String,
    offset: usize,
    len: usize,
}

/// An embeddable bundle of many nuclides' [`WindowedMultipole`] blobs behind a
/// single byte image — the shipping container for the in-crate CORE data set.
///
/// One [`WindowedMultipole::to_blob`] per nuclide is concatenated behind a small
/// name→range index, so the entire CORE set is a single `include_bytes!` and any
/// nuclide is decoded on demand by [`Self::get`]. The container adds **no second
/// compression pass** — each entry is an already-deflated WMPB blob, so packing
/// is just indexing + concatenation.
///
/// # Format — WMPL v1 (little-endian)
/// | Bytes | Field |
/// |---|---|
/// | `0..4` | magic `b"WMPL"` |
/// | `4` | version (`1`) |
/// | `5..8` | reserved (`0`) |
/// | `8..12` | `n_nuclides` (`u32`) |
/// | per nuclide (index) | `name_len` (u32), name (UTF-8), `blob_len` (u32) |
/// | after the index | the `n_nuclides` WMPB blobs, concatenated in index order |
///
/// Blob offsets are **implicit** — the running sum of preceding `blob_len`s — so
/// the index can never disagree with the payload.
#[derive(Debug, Clone)]
pub struct WmpLibrary {
    /// The whole WMPL container image, owned (typically copied once at startup
    /// from an `include_bytes!` static).
    bytes: Vec<u8>,
    /// name → byte range into `bytes`, in the container's stored order.
    index: Vec<WmpEntry>,
}

impl WmpLibrary {
    /// Pack a set of nuclides into a WMPL v1 container image — the offline *bake*
    /// step for the embedded CORE set. Each nuclide is serialized with
    /// [`WindowedMultipole::to_blob`]; the returned bytes are what a maintainer
    /// commits and the crate `include_bytes!`s. Input order is preserved.
    pub fn pack(nuclides: &[WindowedMultipole]) -> Vec<u8> {
        let blobs: Vec<Vec<u8>> = nuclides.iter().map(|n| n.to_blob()).collect();

        let index_len: usize = nuclides.iter().map(|n| 4 + n.name.len() + 4).sum();
        let payload_len: usize = blobs.iter().map(|b| b.len()).sum();
        let mut out = Vec::with_capacity(WMPL_HEADER_LEN + index_len + payload_len);

        out.extend_from_slice(&WMPL_MAGIC);
        out.push(WMPL_VERSION);
        out.extend_from_slice(&[0u8, 0u8, 0u8]); // reserved
        out.extend_from_slice(&(nuclides.len() as u32).to_le_bytes());
        for (n, b) in nuclides.iter().zip(&blobs) {
            let name = n.name.as_bytes();
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        }
        for b in &blobs {
            out.extend_from_slice(b);
        }
        out
    }

    /// Parse a WMPL v1 container image and take ownership of its bytes. The index
    /// is validated eagerly (magic, version, every name and byte range); the
    /// per-nuclide WMPB blobs are **not** decoded until [`Self::get`].
    ///
    /// Input is treated as untrusted: all lengths are bounds-checked so a
    /// malformed image fails cleanly rather than over-allocating.
    ///
    /// # Errors
    /// [`NjoyError::WmpData`] on bad magic/version, a truncated header/index, an
    /// out-of-range blob, or invalid UTF-8 in a name.
    pub fn from_blob(bytes: &[u8]) -> Result<Self, NjoyError> {
        let err = |m: String| NjoyError::WmpData(m);
        if bytes.len() < WMPL_HEADER_LEN {
            return Err(err("blob shorter than WMPL header".into()));
        }
        if bytes[0..4] != WMPL_MAGIC {
            return Err(err("bad WMPL magic".into()));
        }
        if bytes[4] != WMPL_VERSION {
            return Err(err(format!("unsupported WMPL version {}", bytes[4])));
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;

        let read_u32 = |o: usize| -> Result<usize, NjoyError> {
            bytes
                .get(o..o + 4)
                .map(|s| u32::from_le_bytes(s.try_into().unwrap()) as usize)
                .ok_or_else(|| err("truncated index".into()))
        };

        // Pass 1: read (name, blob_len) pairs; `cur` ends at the payload start.
        let mut cur = WMPL_HEADER_LEN;
        let mut parsed: Vec<(String, usize)> = Vec::with_capacity(n);
        for _ in 0..n {
            let name_len = read_u32(cur)?;
            cur += 4;
            let name_end = cur
                .checked_add(name_len)
                .filter(|&e| e <= bytes.len())
                .ok_or_else(|| err("truncated name in index".into()))?;
            let name = std::str::from_utf8(&bytes[cur..name_end])
                .map_err(|e| err(format!("index name utf8: {e}")))?
                .to_string();
            cur = name_end;
            let blob_len = read_u32(cur)?;
            cur += 4;
            parsed.push((name, blob_len));
        }

        // Pass 2: assign implicit offsets into the payload region.
        let mut offset = cur;
        let mut index = Vec::with_capacity(n);
        for (name, len) in parsed {
            let end = offset
                .checked_add(len)
                .filter(|&e| e <= bytes.len())
                .ok_or_else(|| err(format!("blob for {name} out of range")))?;
            index.push(WmpEntry { name, offset, len });
            offset = end;
        }

        Ok(Self { bytes: bytes.to_vec(), index })
    }

    /// The embedded **CORE** nuclide set — 125 reactor-grade + LFTR nuclides
    /// (ENDF/B-VII.1 windowed multipole, MIT CRPG; see `docs/wmp-nuclide-manifest.md`),
    /// baked into the crate so every build resolves cross sections offline
    /// with no HDF5 and no downloads.
    ///
    /// The ~4.7 MB blob is parsed once and shared; repeated calls return the same
    /// cached library. Look up a nuclide with [`Self::get`] (e.g. `.get("U238")`).
    ///
    /// The blob is **always embedded** — there is no feature to disable it, so this
    /// method is available in every build. **License:** the returned data is
    /// MIT CRPG (`LICENSE-WMP` + `NOTICE`), distinct from the NJOY attribution.
    pub fn core() -> &'static WmpLibrary {
        static CORE: OnceLock<WmpLibrary> = OnceLock::new();
        CORE.get_or_init(|| {
            WmpLibrary::from_blob(include_bytes!("data/wmp_core.wmpl"))
                .expect("embedded CORE WMPL blob is valid")
        })
    }

    /// Number of nuclides in the container.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether the container holds no nuclides.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// The nuclide names present, in stored order (e.g. `["U235", "U238", …]`).
    pub fn names(&self) -> Vec<&str> {
        self.index.iter().map(|e| e.name.as_str()).collect()
    }

    /// Whether a nuclide with this name is present.
    pub fn contains(&self, name: &str) -> bool {
        self.index.iter().any(|e| e.name == name)
    }

    /// Decode one nuclide by name. The WMPB blob is inflated on demand and a
    /// fresh [`WindowedMultipole`] returned each call, so a caller that reuses a
    /// nuclide across many evaluations should keep the decoded value.
    ///
    /// # Errors
    /// [`NjoyError::WmpData`] if `name` is absent or its blob fails to decode.
    pub fn get(&self, name: &str) -> Result<WindowedMultipole, NjoyError> {
        let entry = self
            .index
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| NjoyError::WmpData(format!("nuclide {name} not in WMPL container")))?;
        WindowedMultipole::from_blob(&self.bytes[entry.offset..entry.offset + entry.len])
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

    /// Exact (bit-for-bit) equality — the WMPB round trip is lossless, so no
    /// tolerance is warranted. Assumes no NaN fields (none in the fixtures).
    fn assert_wmp_eq(a: &WindowedMultipole, b: &WindowedMultipole) {
        assert_eq!(a.name, b.name);
        for (x, y) in [
            (a.awr, b.awr),
            (a.e_min, b.e_min),
            (a.e_max, b.e_max),
            (a.inv_spacing, b.inv_spacing),
        ] {
            assert_eq!(x.to_bits(), y.to_bits());
        }
        assert_eq!(a.fit_order, b.fit_order);
        assert_eq!(a.fissionable, b.fissionable);
        assert_eq!(a.poles, b.poles);
        assert_eq!(a.residues, b.residues);
        assert_eq!(a.curvefit, b.curvefit);
        assert_eq!(a.windows.len(), b.windows.len());
        for (x, y) in a.windows.iter().zip(&b.windows) {
            assert_eq!((x.start, x.end, x.broaden_poly), (y.start, y.end, y.broaden_poly));
        }
    }

    #[test]
    fn wmpb_blob_round_trips_single_pole() {
        let wmp = single_pole_table();
        let back = WindowedMultipole::from_blob(&wmp.to_blob()).unwrap();
        assert_wmp_eq(&wmp, &back);
    }

    #[test]
    fn wmpb_blob_round_trips_fissionable_multiwindow() {
        // Three windows (incl. an empty one), fissionable, order-1 curve fit.
        let wmp = WindowedMultipole {
            name: "U235".into(),
            awr: 233.024_8,
            e_min: 1e-5,
            e_max: 2250.0,
            fissionable: true,
            poles: vec![
                Cf64::new(1.5, -0.02),
                Cf64::new(12.7, -0.3),
                Cf64::new(30.1, 0.4),
            ],
            residues: vec![
                [Cf64::new(0.1, 0.2), Cf64::new(0.3, -0.4), Cf64::new(0.5, 0.6)],
                [Cf64::new(-1.1, 0.9), Cf64::new(2.2, -0.8), Cf64::new(3.3, 0.7)],
                [Cf64::new(0.01, -0.02), Cf64::new(0.03, 0.04), Cf64::new(0.05, -0.06)],
            ],
            curvefit: vec![
                vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
                vec![[7.0, 8.0, 9.0], [10.0, 11.0, 12.0]],
                vec![[13.0, 14.0, 15.0], [16.0, 17.0, 18.0]],
            ],
            windows: vec![
                WmpWindow { start: 0, end: 1, broaden_poly: true },
                WmpWindow { start: 1, end: 0, broaden_poly: false }, // empty window
                WmpWindow { start: 2, end: 2, broaden_poly: true },
            ],
            inv_spacing: 0.5,
            fit_order: 1,
        };
        let back = WindowedMultipole::from_blob(&wmp.to_blob()).unwrap();
        assert_wmp_eq(&wmp, &back);

        // The decoded table must evaluate bit-identically to the original.
        for &e in &[2.0, 150.0, 900.0] {
            let a = wmp.evaluate(e, 300.0);
            let b = back.evaluate(e, 300.0);
            assert_eq!((a.scatter, a.absorption, a.fission), (b.scatter, b.absorption, b.fission));
        }
    }

    #[test]
    fn from_blob_rejects_corruption() {
        let good = single_pole_table().to_blob();
        // Bad magic.
        let mut m = good.clone();
        m[0] = b'X';
        assert!(WindowedMultipole::from_blob(&m).is_err());
        // Bad version.
        let mut v = good.clone();
        v[4] = 99;
        assert!(WindowedMultipole::from_blob(&v).is_err());
        // Truncated below the header.
        assert!(WindowedMultipole::from_blob(&good[..10]).is_err());
    }

    #[test]
    fn wmpl_container_round_trips() {
        let a = single_pole_table(); // name "TEST1"
        let mut b = single_pole_table();
        b.name = "TEST2".into();
        b.poles = vec![Cf64::new(7.0, -0.2)];
        b.residues = vec![[Cf64::new(0.5, 0.0), Cf64::new(0.9, 0.1), Cf64::new(0.0, 0.0)]];

        let image = WmpLibrary::pack(&[a.clone(), b.clone()]);
        let lib = WmpLibrary::from_blob(&image).unwrap();

        assert_eq!(lib.len(), 2);
        assert!(!lib.is_empty());
        assert_eq!(lib.names(), vec!["TEST1", "TEST2"]);
        assert!(lib.contains("TEST2") && !lib.contains("U999"));

        assert_wmp_eq(&a, &lib.get("TEST1").unwrap());
        assert_wmp_eq(&b, &lib.get("TEST2").unwrap());
        assert!(lib.get("MISSING").is_err());
    }

    #[test]
    fn wmpl_from_blob_rejects_corruption() {
        let image = WmpLibrary::pack(&[single_pole_table()]);
        let mut bad = image.clone();
        bad[0] = b'Z'; // clobber magic
        assert!(WmpLibrary::from_blob(&bad).is_err());
        // Truncated below the header.
        assert!(WmpLibrary::from_blob(&image[..8]).is_err());
    }

    #[test]
    fn embedded_core_library_loads_and_evaluates() {
        let lib = WmpLibrary::core();
        assert_eq!(lib.len(), 125, "CORE nuclide count");
        assert!(lib.contains("U238") && lib.contains("H1"));

        // Fissionable actinide: the 6.673 eV capture resonance is large at 300 K
        // (the U-238 Doppler target — see tests/wmp_u238.rs for the full gate).
        let u238 = lib.get("U238").unwrap();
        assert!(u238.fissionable);
        let on_res = u238.evaluate(6.673, 300.0).absorption;
        assert!(on_res > 100.0, "U-238 6.673 eV absorption = {on_res} b");

        // Non-fissionable nuclide decodes with an all-zero fission channel.
        let h1 = lib.get("H1").unwrap();
        assert!(!h1.fissionable);
        assert_eq!(h1.evaluate(1.0, 300.0).fission, 0.0);
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
