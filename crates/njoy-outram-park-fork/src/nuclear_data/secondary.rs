//! Secondary fission data WMP does not carry: ν̄(E) and the fission spectrum χ(E).
//!
//! Windowed multipole ([`crate::wmp`]) gives cross-section *magnitudes* but no
//! information about the neutrons a fission *emits*. A criticality (Keff)
//! calculation needs two more pieces, both tiny enough to embed in-crate:
//!
//! - **ν̄(E)** — average neutrons per fission vs incident energy (ENDF MF=1/452).
//! - **χ(E')** — the fission-neutron energy spectrum (Watt form, or MF=5 later).
//!
//! Sourced properly from the ACER port (4b for the NU block, 4d for χ; see
//! `docs/porting-plan.md` §8) or hardcoded from ENDF as a stopgap.

/// Average neutron yield per fission, ν̄(E).
///
/// Stored as a lin-lin table in incident energy \[eV\]; `nu_total` is prompt +
/// delayed (delayed matters for delayed-critical benchmarks; a prompt bare-sphere
/// Keff uses the total directly).
#[derive(Debug, Clone, Default)]
pub struct NuBar {
    /// Incident-energy grid \[eV\], ascending.
    pub energy: Vec<f64>,
    /// Total ν̄ aligned with `energy`.
    pub nu_total: Vec<f64>,
}

impl NuBar {
    /// Parse total ν̄(E) from ENDF **MF=1/MT=452** for material `mat`.
    ///
    /// Handles both ENDF representations:
    /// - **LNU=2** — ν̄(E) tabulated (TAB1); stored directly as the lin-lin table.
    /// - **LNU=1** — ν̄(E) = Σ Cₖ Eᵏ polynomial (LIST of coefficients); sampled
    ///   onto a 60-point log grid over `[1e-3, 2e7]` eV so it fits the same table.
    ///
    /// Returns `Ok(None)` when the material has no MF=1/MT=452 section (i.e. a
    /// non-fissile nuclide) or an unrecognised `LNU`. Used by the fast-MGXS bake to
    /// fold ν̄ into `nu_fission` (see [`super::Mgxs::collapse_from_reconr`]).
    pub fn from_endf(
        tape: &crate::endf::tape::Tape,
        mat: i32,
    ) -> Result<Option<NuBar>, crate::NjoyError> {
        use crate::endf::records::SectionCursor;

        let sec = match tape.section(mat, 1, 452) {
            Some(s) => s,
            None => return Ok(None),
        };
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?; // HEAD: L2 = LNU
        match head.l2 {
            2 => {
                let tab1 = cur.read_tab1()?;
                let (energy, nu_total): (Vec<f64>, Vec<f64>) =
                    tab1.pairs.iter().copied().unzip();
                Ok(Some(NuBar { energy, nu_total }))
            }
            1 => {
                let list = cur.read_list()?;
                let coeffs = list.data; // [C1 (const), C2, …] ascending powers
                let (e_lo, e_hi, n) = (1.0e-3_f64, 2.0e7_f64, 60usize);
                let mut energy = Vec::with_capacity(n + 1);
                let mut nu_total = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let e = e_lo * (e_hi / e_lo).powf(i as f64 / n as f64);
                    // Horner over reversed coefficients: C1 + C2·E + C3·E² + …
                    let nu = coeffs.iter().rev().fold(0.0, |acc, &c| acc * e + c);
                    energy.push(e);
                    nu_total.push(nu);
                }
                Ok(Some(NuBar { energy, nu_total }))
            }
            _ => Ok(None),
        }
    }

    /// Interpolate ν̄ at incident energy `e` \[eV\] (lin-lin, clamped at the ends).
    pub fn at(&self, e: f64) -> f64 {
        match self.energy.first() {
            None => 0.0,
            Some(&e0) if e <= e0 => self.nu_total[0],
            Some(_) => {
                let &en = self.energy.last().unwrap();
                if e >= en {
                    return *self.nu_total.last().unwrap();
                }
                let hi = self.energy.iter().position(|&x| x >= e).unwrap();
                let (x0, x1) = (self.energy[hi - 1], self.energy[hi]);
                let (y0, y1) = (self.nu_total[hi - 1], self.nu_total[hi]);
                y0 + (y1 - y0) * (e - x0) / (x1 - x0)
            }
        }
    }
}

/// Fission-neutron energy spectrum χ(E').
///
/// A `Watt` form covers the bare-sphere first cut; `Tabulated` allows a faithful
/// MF=5 spectrum later. Sampling is done by the transport layer; this type only
/// *describes* the distribution.
#[derive(Debug, Clone)]
pub enum FissionSpectrum {
    /// Watt spectrum `χ(E') ∝ e^{−E'/a} · sinh(√(b·E'))`, `a` \[eV\], `b` \[eV⁻¹\].
    Watt { a: f64, b: f64 },
    /// Tabulated χ: outgoing energy grid \[eV\] and aligned pdf.
    Tabulated { e_out: Vec<f64>, pdf: Vec<f64> },
}

impl Default for FissionSpectrum {
    /// A representative fast-fission Watt spectrum (U-235 thermal-fission params).
    fn default() -> Self {
        FissionSpectrum::Watt { a: 0.988e6, b: 2.249e-6 }
    }
}

impl FissionSpectrum {
    /// Unnormalized spectral weight χ(E) at outgoing energy `e` \[eV\].
    ///
    /// This is the **group-collapse weighting function** for the fast-range MGXS
    /// path ([`super::Mgxs::collapse_watt`]): a fission-spectrum-weighted average
    /// σ_g = ∫σ(E)χ(E)dE / ∫χ(E)dE. Absolute normalization is irrelevant — only
    /// the *shape* matters for a weighted average — so the Watt form is returned
    /// without its leading constant. `Tabulated` interpolates lin-lin, clamped to
    /// zero outside the tabulated support.
    pub fn weight(&self, e: f64) -> f64 {
        match self {
            FissionSpectrum::Watt { a, b } => {
                if e <= 0.0 {
                    return 0.0;
                }
                (-e / a).exp() * (b * e).sqrt().sinh()
            }
            FissionSpectrum::Tabulated { e_out, pdf } => {
                match e_out.first() {
                    None => 0.0,
                    Some(&e0) if e < e0 => 0.0,
                    Some(_) => {
                        let &en = e_out.last().unwrap();
                        if e > en {
                            return 0.0;
                        }
                        let hi = e_out.iter().position(|&x| x >= e).unwrap();
                        if hi == 0 {
                            return pdf[0];
                        }
                        let (x0, x1) = (e_out[hi - 1], e_out[hi]);
                        let (y0, y1) = (pdf[hi - 1], pdf[hi]);
                        y0 + (y1 - y0) * (e - x0) / (x1 - x0)
                    }
                }
            }
        }
    }
}
