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
    /// **Energy-dependent** tabulated χ(E→E') — the ENDF MF=5 / MT=18 LF=1
    /// "arbitrary tabulated secondary energy distribution". The faithful fission
    /// birth spectrum: the outgoing-neutron energy distribution depends on the
    /// incident energy of the neutron that caused the fission. See [`ChiTabular`].
    ContinuousTabular(ChiTabular),
}

/// One incident energy's outgoing-neutron energy distribution g(E→E') — the inner
/// table of an ENDF MF=5 LF=1 fission spectrum.
///
/// `e_out`/`pdf`/`cdf` are the ascending outgoing-energy grid \[eV\], its
/// probability density \[eV⁻¹\], and the cumulative distribution (`cdf[0]=0`,
/// `cdf[last]=1`), all aligned and index-for-index consistent (`cdf` is the
/// integral of `pdf`). `linlin` selects interpolation *between* grid points:
/// `true` = linear-linear (ENDF INT=2), `false` = histogram (INT=1). Mirrors
/// OpenMC's `CTTable` (`include/openmc/distribution_energy.h`).
#[derive(Debug, Clone)]
pub struct ChiEout {
    /// Outgoing-energy grid \[eV\], ascending.
    pub e_out: Vec<f64>,
    /// Probability density g(E') \[eV⁻¹\], aligned with `e_out`, ∫ = 1.
    pub pdf: Vec<f64>,
    /// Cumulative distribution, aligned with `e_out` (`cdf[0]=0`, `cdf[last]=1`).
    pub cdf: Vec<f64>,
    /// `true` ⇒ lin-lin between grid points (ENDF INT=2); `false` ⇒ histogram.
    pub linlin: bool,
}

/// Energy-dependent tabulated fission spectrum χ(E→E') — the ENDF MF=5 / MT=18
/// LF=1 secondary-energy law, the faithful replacement for the fixed thermal-Watt
/// stand-in.
///
/// `incident` is the ascending incident-neutron energy grid \[eV\]; `tables[i]` is
/// the outgoing-energy distribution [`ChiEout`] at `incident[i]`. Sampling (done by
/// the transport layer, which owns the RNG) locates the incident-energy bin,
/// statistically picks the lower or upper table, inverts its CDF, then scales the
/// result between the neighbouring tables' \[E₁, E_K\] envelopes — a port of
/// OpenMC's `ContinuousTabular::sample` (`src/distribution_energy.cpp`).
#[derive(Debug, Clone)]
pub struct ChiTabular {
    /// Incident-neutron energy grid \[eV\], ascending.
    pub incident: Vec<f64>,
    /// Outgoing-energy distribution at each incident energy (`tables.len() ==
    /// incident.len()`).
    pub tables: Vec<ChiEout>,
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
            FissionSpectrum::Tabulated { e_out, pdf } => interp_zero(e_out, pdf, e),
            FissionSpectrum::ContinuousTabular(chi) => {
                // Group-collapse weighting function: use the outgoing spectrum at
                // the lowest incident energy (≈ the thermal-fission χ shape) as the
                // representative weight. This path is only exercised if a HIGH-tier
                // χ is ever fed to the LOW-tier MGXS collapse; the embedded bake
                // uses the Watt arm.
                match chi.tables.first() {
                    None => 0.0,
                    Some(t) => interp_zero(&t.e_out, &t.pdf, e),
                }
            }
        }
    }

    /// Parse the energy-dependent prompt fission-neutron spectrum χ(E→E') from
    /// ENDF **MF=5 / MT=18** for material `mat`.
    ///
    /// Handles the **LF=1** ("arbitrary tabulated secondary energy distribution")
    /// representation with a single subsection (NK=1, p(E)≡1) — how the
    /// ENDF/B-VII.1 uranium fission spectra are stored: a TAB2 over NE incident
    /// energies, each carrying a TAB1 g(E→E') outgoing-energy density. Returns the
    /// [`FissionSpectrum::ContinuousTabular`] form. A per-incident CDF is built by
    /// integrating each density (lin-lin trapezoids or histogram bins) and
    /// renormalising so `cdf[last] = 1`, matching what ACER precomputes for OpenMC.
    ///
    /// Returns `Ok(None)` (caller falls back to the Watt stand-in) when the
    /// material has no MF=5/MT=18 section, `NK ≠ 1`, or `LF ≠ 1` — i.e. any
    /// representation this port does not yet reconstruct (the LF=5/7/9/11
    /// general-evaporation / energy-dependent Maxwell / Watt laws). ENDF/B-VII.1
    /// U-234/235/238 are all LF=1, NK=1.
    pub fn from_endf_mf5(
        tape: &crate::endf::tape::Tape,
        mat: i32,
    ) -> Result<Option<FissionSpectrum>, crate::NjoyError> {
        use crate::endf::records::SectionCursor;

        let sec = match tape.section(mat, 5, 18) {
            Some(s) => s,
            None => return Ok(None),
        };
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?; // HEAD: ZA, AWR, 0, 0, NK, 0
        if head.n1 != 1 {
            return Ok(None); // NK≠1 (multi-partition χ) not yet supported → Watt
        }

        // Subsection: p_k(E) fraction (TAB1; L2 = LF), then the LF=1 body.
        let p_tab = cur.read_tab1()?;
        if p_tab.head.l2 != 1 {
            return Ok(None); // LF=5/7/9/11 not yet ported → Watt fallback
        }

        // TAB2 over NE incident energies (head.n2 = NE), each an inner TAB1.
        let tab2 = cur.read_tab2()?;
        let ne = tab2.head.n2.max(0) as usize;
        let mut incident = Vec::with_capacity(ne);
        let mut tables = Vec::with_capacity(ne);
        for _ in 0..ne {
            let g = cur.read_tab1()?; // header C2 = incident energy E_i; pairs (E', g)
            let e_in = g.head.c2;
            // INT law from the first (only) region: 1 = histogram, else lin-lin.
            let linlin = g.interp.first().map(|&(_, law)| law != 1).unwrap_or(true);
            let e_out: Vec<f64> = g.pairs.iter().map(|&(x, _)| x).collect();
            let mut pdf: Vec<f64> = g.pairs.iter().map(|&(_, y)| y).collect();
            let cdf = build_cdf(&e_out, &mut pdf, linlin);
            incident.push(e_in);
            tables.push(ChiEout { e_out, pdf, cdf, linlin });
        }

        Ok(Some(FissionSpectrum::ContinuousTabular(ChiTabular { incident, tables })))
    }
}

/// Lin-lin interpolate a tabulated density `y` on an ascending `x` grid \[eV\] at
/// `e`, clamped to zero outside `[x[0], x[last]]`. Shared by the [`FissionSpectrum`]
/// `Tabulated` / `ContinuousTabular` group-collapse weights.
fn interp_zero(x: &[f64], y: &[f64], e: f64) -> f64 {
    match x.first() {
        None => 0.0,
        Some(&x0) if e < x0 => 0.0,
        Some(_) => {
            let &xn = x.last().unwrap();
            if e > xn {
                return 0.0;
            }
            let hi = x.iter().position(|&v| v >= e).unwrap();
            if hi == 0 {
                return y[0];
            }
            let (a, b) = (x[hi - 1], x[hi]);
            let (ya, yb) = (y[hi - 1], y[hi]);
            ya + (yb - ya) * (e - a) / (b - a)
        }
    }
}

/// Build a normalized cumulative distribution from a tabulated (`e_out`, `pdf`)
/// pair by integrating with the given interpolation (lin-lin trapezoids or
/// histogram bins), then rescale **both** `pdf` (in place) and the returned `cdf`
/// so the total integral is 1.
///
/// ENDF MF=5 g(E→E') is defined normalized, but the tabulated points carry
/// rounding; renormalising keeps `pdf` and `cdf` mutually consistent — a
/// requirement of the CDF-inversion sampler (`ContinuousTabular::sample`), whose
/// lin-lin inverse mixes `p` and `c` in the same expression.
fn build_cdf(e_out: &[f64], pdf: &mut [f64], linlin: bool) -> Vec<f64> {
    let n = e_out.len();
    let mut cdf = vec![0.0; n];
    for k in 1..n {
        let dx = e_out[k] - e_out[k - 1];
        let area = if linlin {
            0.5 * (pdf[k] + pdf[k - 1]) * dx // trapezoid
        } else {
            pdf[k - 1] * dx // histogram (left value)
        };
        cdf[k] = cdf[k - 1] + area;
    }
    let total = cdf.last().copied().unwrap_or(0.0);
    if total > 0.0 {
        for c in cdf.iter_mut() {
            *c /= total;
        }
        for p in pdf.iter_mut() {
            *p /= total;
        }
    }
    cdf
}
