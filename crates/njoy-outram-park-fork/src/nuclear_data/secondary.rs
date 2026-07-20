//! Secondary fission data WMP does not carry: ν̄(E) and the fission spectrum χ(E).
//!
//! Windowed multipole ([`crate::wmp`]) gives cross-section *magnitudes* but no
//! information about the neutrons a fission *emits*. A criticality (Keff)
//! calculation needs two more pieces, both tiny enough to embed in-crate:
//!
//! - **ν̄(E)** — average neutrons per fission vs incident energy (ENDF MF=1/452,
//!   [`NuBar::from_endf`]).
//! - **χ(E→E')** — the fission-neutron energy spectrum (ENDF MF=5/MT=18,
//!   [`FissionSpectrum::from_endf_mf5`]): every LF code with a real sampling
//!   algorithm is ported (LF=1/7/9/11, plus NK>1 mixtures); the fixed thermal-Watt
//!   form remains the default for nuclides/tiers with no parsed MF=5.
//!
//! Parsed directly off the ENDF tape and consumed by `outram-mc-libs::Nuclide` — not
//! routed through the ACER ACE-file writer (`src/ace/energy.rs` has its own,
//! narrower MF=5 LF=1 parser for that separate path; see `docs/porting-plan.md` §8).

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
/// A `Watt` form covers the bare-sphere first cut. The remaining variants are the
/// ENDF **MF=5** secondary-energy laws (LF code), each named after its ENDF LF /
/// ACE law number; sampling is done by the transport layer (which owns the RNG),
/// this type only *describes* the distribution. See [`FissionSpectrum::from_endf_mf5`]
/// for which LF codes are ported.
#[derive(Debug, Clone)]
pub enum FissionSpectrum {
    /// Watt spectrum `χ(E') ∝ e^{−E'/a} · sinh(√(b·E'))`, `a` \[eV\], `b` \[eV⁻¹\].
    /// Fixed (energy-independent) parameters — the pre-MF=5 stopgap, still the
    /// LOW-tier default (no embedded MF=5).
    Watt { a: f64, b: f64 },
    /// Tabulated χ: outgoing energy grid \[eV\] and aligned pdf. Static (no
    /// incident-energy dependence) — distinct from [`Self::ContinuousTabular`],
    /// which is the real energy-dependent MF=5 LF=1 law.
    Tabulated { e_out: Vec<f64>, pdf: Vec<f64> },
    /// **LF=1** — "arbitrary tabulated secondary energy distribution": the real
    /// energy-dependent χ(E→E'). The faithful fission birth spectrum: the
    /// outgoing-neutron energy distribution depends on the incident energy of the
    /// neutron that caused the fission. See [`ChiTabular`].
    ContinuousTabular(ChiTabular),
    /// **LF=7** — simple Maxwellian fission spectrum
    /// `f(E→E') = √E' · e^{−E'/θ(E)} / I`, restricted to `E' ≤ E − U`. `theta` is
    /// the incident-energy-dependent nuclear temperature θ(E) \[eV\] (ENDF TAB1);
    /// `u` \[eV\] is the restriction energy. Mirrors OpenMC `MaxwellEnergy`
    /// (`include/openmc/distribution_energy.h`).
    Maxwell {
        /// θ(E) \[eV\] vs incident energy \[eV\] (ENDF TAB1, general interpolation).
        theta: crate::endf::Tab1,
        /// Restriction energy U \[eV\]: `0 ≤ E' ≤ E − U`.
        u: f64,
    },
    /// **LF=9** — evaporation spectrum `f(E→E') = E' · e^{−E'/θ(E)} / I`,
    /// restricted to `E' ≤ E − U`. Mirrors OpenMC `Evaporation`.
    Evaporation {
        /// θ(E) \[eV\] vs incident energy \[eV\] (ENDF TAB1, general interpolation).
        theta: crate::endf::Tab1,
        /// Restriction energy U \[eV\].
        u: f64,
    },
    /// **LF=11** — energy-dependent Watt spectrum
    /// `f(E→E') = e^{−E'/a(E)} · sinh(√(b(E)·E')) / I`, restricted to `E' ≤ E − U`.
    /// Unlike [`Self::Watt`], `a`/`b` are themselves tabulated functions of the
    /// *incident* energy. Mirrors OpenMC `WattEnergy`.
    WattEnergyDependent {
        /// a(E) \[eV\] vs incident energy \[eV\] (ENDF TAB1).
        a: crate::endf::Tab1,
        /// b(E) \[eV⁻¹\] vs incident energy \[eV\] (ENDF TAB1).
        b: crate::endf::Tab1,
        /// Restriction energy U \[eV\].
        u: f64,
    },
    /// **NK > 1** — a mixture of `NK` partial distributions, each active with
    /// probability `p_k(E)` (an ENDF TAB1 fraction vs *incident* energy,
    /// Σₖ p_k(E) = 1). Sampling picks a partition by `p_k(E_in)`, then samples
    /// that partition's own law recursively. Rare for incident-neutron fission
    /// (U-234/235/238 are all NK=1); ported for completeness.
    Mixture(Vec<(crate::endf::Tab1, FissionSpectrum)>),
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
            FissionSpectrum::Maxwell { theta, .. } => {
                // Representative shape at the lowest tabulated incident energy.
                let t = theta.pairs.first().map(|&(_, y)| y).unwrap_or(1.0);
                if e <= 0.0 || t <= 0.0 {
                    0.0
                } else {
                    e.sqrt() * (-e / t).exp()
                }
            }
            FissionSpectrum::Evaporation { theta, .. } => {
                let t = theta.pairs.first().map(|&(_, y)| y).unwrap_or(1.0);
                if e <= 0.0 || t <= 0.0 {
                    0.0
                } else {
                    e * (-e / t).exp()
                }
            }
            FissionSpectrum::WattEnergyDependent { a, b, .. } => {
                let a0 = a.pairs.first().map(|&(_, y)| y).unwrap_or(1.0);
                let b0 = b.pairs.first().map(|&(_, y)| y).unwrap_or(1.0);
                if e <= 0.0 {
                    0.0
                } else {
                    (-e / a0).exp() * (b0 * e).sqrt().sinh()
                }
            }
            FissionSpectrum::Mixture(parts) => parts
                .iter()
                .map(|(p_k, law)| {
                    let frac = p_k.pairs.first().map(|&(_, y)| y).unwrap_or(0.0);
                    frac * law.weight(e)
                })
                .sum(),
        }
    }

    /// Mean outgoing fission-neutron energy `⟨E'⟩` \[eV\] at incident energy
    /// `e_in` \[eV\] — the first moment of χ(E_in→E'), used by [`crate::heatr`]'s
    /// fission heating term (`H = E + Q − ν̄·⟨E'⟩`) rather than a full sample.
    ///
    /// Closed-form for the analytic laws (`Watt`: `1.5a+0.25a²b`; `Maxwell`:
    /// `1.5θ`; `Evaporation`: `2θ`), evaluating any energy-dependent parameter at
    /// `e_in`. The restriction energy `u` (LF=7/9/11) is ignored here — it only
    /// truncates the sampled distribution's tail, a second-order correction for a
    /// *mean*-energy estimate. `ContinuousTabular` and `Tabulated` integrate the
    /// tabulated density directly (trapezoidal); `ContinuousTabular` uses the
    /// nearer bracketing incident-energy table rather than the full envelope
    /// interpolation [`crate::heatr`] needs.
    pub fn mean_energy(&self, e_in: f64) -> f64 {
        match self {
            FissionSpectrum::Watt { a, b } => 1.5 * a + 0.25 * a * a * b,
            FissionSpectrum::Tabulated { e_out, pdf } => mean_of_pdf(e_out, pdf),
            FissionSpectrum::ContinuousTabular(chi) => {
                if chi.tables.is_empty() {
                    return 0.0;
                }
                let n = chi.incident.len();
                let i = match chi.incident.iter().position(|&e| e >= e_in) {
                    Some(0) => 0,
                    Some(k) => {
                        // Nearer of the bracketing pair.
                        let (e0, e1) = (chi.incident[k - 1], chi.incident[k]);
                        if (e_in - e0).abs() <= (e1 - e_in).abs() {
                            k - 1
                        } else {
                            k
                        }
                    }
                    None => n - 1,
                };
                mean_of_pdf(&chi.tables[i].e_out, &chi.tables[i].pdf)
            }
            FissionSpectrum::Maxwell { theta, .. } => {
                1.5 * crate::endf::interp::eval_tab1(e_in, &theta.interp, &theta.pairs).unwrap_or(0.0)
            }
            FissionSpectrum::Evaporation { theta, .. } => {
                2.0 * crate::endf::interp::eval_tab1(e_in, &theta.interp, &theta.pairs).unwrap_or(0.0)
            }
            FissionSpectrum::WattEnergyDependent { a, b, .. } => {
                let av = crate::endf::interp::eval_tab1(e_in, &a.interp, &a.pairs).unwrap_or(0.0);
                let bv = crate::endf::interp::eval_tab1(e_in, &b.interp, &b.pairs).unwrap_or(0.0);
                1.5 * av + 0.25 * av * av * bv
            }
            FissionSpectrum::Mixture(parts) => {
                let weights: Vec<f64> = parts
                    .iter()
                    .map(|(p_k, _)| {
                        crate::endf::interp::eval_tab1(e_in, &p_k.interp, &p_k.pairs).unwrap_or(0.0)
                    })
                    .collect();
                let total: f64 = weights.iter().sum();
                if !(total > 0.0) {
                    return 0.0;
                }
                weights
                    .iter()
                    .zip(parts.iter())
                    .map(|(&w, (_, law))| w * law.mean_energy(e_in))
                    .sum::<f64>()
                    / total
            }
        }
    }

    /// Parse the prompt fission-neutron energy spectrum χ(E→E') from ENDF
    /// **MF=5 / MT=18** for material `mat`.
    ///
    /// Loops over all `NK` partial distributions (per-partition weighted by their
    /// own `p_k(E)` fraction vs *incident* energy). For `NK = 1` (the common case
    /// — ENDF/B-VII.1 U-234/235/238 are all NK=1) the single partition's law is
    /// returned directly; for `NK > 1` the partitions are wrapped in
    /// [`FissionSpectrum::Mixture`].
    ///
    /// Each partition's law is selected by its ENDF **LF** code:
    /// - **LF=1** — arbitrary tabulated: a TAB2 over NE incident energies, each an
    ///   inner TAB1 g(E→E') outgoing-energy density → [`FissionSpectrum::ContinuousTabular`].
    ///   A per-incident CDF is built by integrating each density (lin-lin
    ///   trapezoids or histogram bins) and renormalising so `cdf[last] = 1`,
    ///   matching what ACER precomputes for OpenMC.
    /// - **LF=7** — Maxwellian: one TAB1 θ(E) → [`FissionSpectrum::Maxwell`].
    /// - **LF=9** — evaporation: one TAB1 θ(E) → [`FissionSpectrum::Evaporation`].
    /// - **LF=11** — energy-dependent Watt: two TAB1s a(E), b(E) →
    ///   [`FissionSpectrum::WattEnergyDependent`].
    /// - **LF=5** (general evaporation) and **LF=12** (Madland-Nix) are *not*
    ///   ported: LF=5 has no sampling algorithm even in canonical OpenMC (its
    ///   Python `GeneralEvaporation.to_hdf5` raises `NotImplementedError` —
    ///   genuinely unsupported upstream, not a gap in this port) and LF=12 is
    ///   vanishingly rare for incident-neutron fission. Either aborts the whole
    ///   parse (`Ok(None)`, caller falls back to the Watt stand-in) — matching the
    ///   ENDF requirement that Σₖ p_k(E) = 1: a partially-parsed mixture would be
    ///   physically wrong, not just incomplete.
    ///
    /// Returns `Ok(None)` when the material has no MF=5/MT=18 section, or any
    /// partition uses an unported LF.
    pub fn from_endf_mf5(
        tape: &crate::endf::tape::Tape,
        mat: i32,
    ) -> Result<Option<FissionSpectrum>, crate::NjoyError> {
        Self::from_endf_mf5_mt(tape, mat, 18)
    }

    /// Parse the secondary-neutron energy spectrum from ENDF **MF=5** for an
    /// arbitrary reaction `mt` (not just fission MT=18) — the same LF=1/7/9/11 +
    /// NK-mixture machinery as [`from_endf_mf5`](Self::from_endf_mf5), used by
    /// HEATR H5 to read the MF=5 emission spectra of (n,2n)/(n,3n)/continuum
    /// reactions in older evaluations that store them there rather than in MF=6.
    /// Returns `Ok(None)` if the material has no MF=5/`mt` section or it uses an
    /// unported LF.
    pub fn from_endf_mf5_mt(
        tape: &crate::endf::tape::Tape,
        mat: i32,
        mt: i32,
    ) -> Result<Option<FissionSpectrum>, crate::NjoyError> {
        match tape.section(mat, 5, mt) {
            Some(sec) => parse_mf5_section(&sec.rows),
            None => Ok(None),
        }
    }
}

/// Row-level MF=5/MT=18 parser — the body of [`FissionSpectrum::from_endf_mf5`],
/// factored out so it can be exercised directly against hand-built `[f64; 6]` rows
/// in unit tests without constructing a full [`crate::endf::tape::Tape`].
fn parse_mf5_section(rows: &[[f64; 6]]) -> Result<Option<FissionSpectrum>, crate::NjoyError> {
    use crate::endf::records::SectionCursor;

    let mut cur = SectionCursor::new(rows);
    let head = cur.read_cont()?; // HEAD: ZA, AWR, 0, 0, NK, 0
    let nk = head.n1.max(0) as usize;
    if nk == 0 {
        return Ok(None);
    }

    let mut partitions = Vec::with_capacity(nk);
    for _ in 0..nk {
        // Subsection header: p_k(E) fraction (TAB1; C1 = U, L2 = LF).
        let p_tab = cur.read_tab1()?;
        let u = p_tab.head.c1;
        let law = match p_tab.head.l2 {
            1 => Some(FissionSpectrum::ContinuousTabular(parse_lf1_tabular(&mut cur)?)),
            7 => Some(FissionSpectrum::Maxwell { theta: cur.read_tab1()?, u }),
            9 => Some(FissionSpectrum::Evaporation { theta: cur.read_tab1()?, u }),
            11 => {
                let a = cur.read_tab1()?;
                let b = cur.read_tab1()?;
                Some(FissionSpectrum::WattEnergyDependent { a, b, u })
            }
            _ => None, // LF=5 (unsupported upstream), LF=12 (Madland-Nix), other
        };
        match law {
            Some(l) => partitions.push((p_tab, l)),
            None => return Ok(None), // Σp_k=1 requires every partition ported
        }
    }

    if nk == 1 {
        Ok(Some(partitions.into_iter().next().unwrap().1))
    } else {
        Ok(Some(FissionSpectrum::Mixture(partitions)))
    }
}

/// Parse the **LF=1** body (arbitrary tabulated secondary energy distribution):
/// a TAB2 over NE incident energies, each an inner TAB1 g(E→E'). Shared by every
/// partition of [`FissionSpectrum::from_endf_mf5`] that uses LF=1.
fn parse_lf1_tabular(
    cur: &mut crate::endf::records::SectionCursor<'_>,
) -> Result<ChiTabular, crate::NjoyError> {
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
    Ok(ChiTabular { incident, tables })
}

/// Mean of a tabulated (possibly unnormalized) density: `∫x·y(x)dx / ∫y(x)dx`,
/// by trapezoidal quadrature over the given grid. Returns `0.0` for an empty or
/// zero-integral table. Shared by [`FissionSpectrum::mean_energy`]'s tabulated
/// arms.
fn mean_of_pdf(x: &[f64], y: &[f64]) -> f64 {
    if x.len() < 2 {
        return x.first().copied().unwrap_or(0.0);
    }
    let mut norm = 0.0;
    let mut first_moment = 0.0;
    for k in 1..x.len() {
        let dx = x[k] - x[k - 1];
        norm += 0.5 * (y[k] + y[k - 1]) * dx;
        first_moment += 0.5 * (x[k] * y[k] + x[k - 1] * y[k - 1]) * dx;
    }
    if norm > 0.0 {
        first_moment / norm
    } else {
        0.0
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The Watt mean matches the closed form `1.5a + 0.25a²b`, independent of
    /// `e_in` (fixed-parameter Watt has no incident-energy dependence).
    #[test]
    fn watt_mean_energy_matches_closed_form() {
        let chi = FissionSpectrum::Watt { a: 0.988e6, b: 2.249e-6 };
        let expected = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        assert!((chi.mean_energy(1.0e6) - expected).abs() < 1.0, "got {}", chi.mean_energy(1.0e6));
        // Energy-independent: same at a very different e_in.
        assert!((chi.mean_energy(1.0e5) - expected).abs() < 1.0);
    }

    /// A uniform tabulated χ on `[0, 2 MeV]` has an exact mean of 1 MeV — the
    /// same synthetic table style used for the outram-mc-libs `ContinuousTabular`
    /// sampler tests, so both layers agree on what "uniform" integrates to.
    #[test]
    fn tabulated_mean_of_uniform_density() {
        let chi = FissionSpectrum::Tabulated {
            e_out: vec![0.0, 1.0e6, 2.0e6],
            pdf: vec![5.0e-7, 5.0e-7, 5.0e-7],
        };
        assert!((chi.mean_energy(0.0) - 1.0e6).abs() < 1.0, "got {}", chi.mean_energy(0.0));
    }

    /// One NR=1/NP=2 lin-lin TAB1 row group: header + 1 interp row + 1 xy row.
    /// `(nbt, int)` is the sole interpolation region; `(x0,y0)`/`(x1,y1)` the two
    /// tabulated points. `c1`/`l2` are the header's C1 (often `U`) and L2 (often
    /// `LF`) fields — the two the MF=5 parser reads out of each TAB1 subsection.
    #[allow(clippy::too_many_arguments)]
    fn tab1_rows(
        c1: f64,
        l2: i32,
        nbt: u32,
        int: u32,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> Vec<[f64; 6]> {
        vec![
            [c1, 0.0, 0.0, l2 as f64, 1.0, 2.0], // header: NR=1, NP=2
            [nbt as f64, int as f64, 0.0, 0.0, 0.0, 0.0],
            [x0, y0, x1, y1, 0.0, 0.0],
        ]
    }

    /// LF=7 (Maxwell): NK=1, one p_k(E)≡1 TAB1 (LF=7) + one θ(E) TAB1.
    #[test]
    fn parses_lf7_maxwell() {
        let mut rows = vec![[92235.0, 233.0, 0.0, 0.0, 1.0, 0.0]]; // HEAD: NK=1
        rows.extend(tab1_rows(0.0, 7, 2, 2, 1.0e-5, 1.0, 2.0e7, 1.0)); // p_k(E)=1
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 1.0e6, 2.0e7, 1.5e6)); // θ(E)

        let chi = parse_mf5_section(&rows).unwrap().unwrap();
        match chi {
            FissionSpectrum::Maxwell { theta, u } => {
                assert_eq!(u, 0.0);
                assert_eq!(theta.pairs, vec![(1.0e-5, 1.0e6), (2.0e7, 1.5e6)]);
            }
            other => panic!("expected Maxwell, got {other:?}"),
        }
    }

    /// LF=9 (evaporation): same shape as LF=7, distinguished by the p_k TAB1's L2.
    #[test]
    fn parses_lf9_evaporation() {
        let mut rows = vec![[92235.0, 233.0, 0.0, 0.0, 1.0, 0.0]];
        rows.extend(tab1_rows(1.0e3, 9, 2, 2, 1.0e-5, 1.0, 2.0e7, 1.0)); // U = 1 keV
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 8.0e5, 2.0e7, 1.2e6));

        let chi = parse_mf5_section(&rows).unwrap().unwrap();
        match chi {
            FissionSpectrum::Evaporation { theta, u } => {
                assert_eq!(u, 1.0e3);
                assert_eq!(theta.pairs, vec![(1.0e-5, 8.0e5), (2.0e7, 1.2e6)]);
            }
            other => panic!("expected Evaporation, got {other:?}"),
        }
    }

    /// LF=11 (energy-dependent Watt): p_k TAB1 (LF=11) + a(E) TAB1 + b(E) TAB1, in
    /// that order — verifies the cursor consumes exactly the right row counts so
    /// the second TAB1 (`b`) isn't misaligned by the first (`a`).
    #[test]
    fn parses_lf11_watt_energy_dependent() {
        let mut rows = vec![[92238.0, 236.0, 0.0, 0.0, 1.0, 0.0]];
        rows.extend(tab1_rows(0.0, 11, 2, 2, 1.0e-5, 1.0, 2.0e7, 1.0)); // p_k
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 0.965e6, 2.0e7, 1.0e6)); // a(E)
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 2.29e-6, 2.0e7, 2.5e-6)); // b(E)

        let chi = parse_mf5_section(&rows).unwrap().unwrap();
        match chi {
            FissionSpectrum::WattEnergyDependent { a, b, u } => {
                assert_eq!(u, 0.0);
                assert_eq!(a.pairs, vec![(1.0e-5, 0.965e6), (2.0e7, 1.0e6)]);
                assert_eq!(b.pairs, vec![(1.0e-5, 2.29e-6), (2.0e7, 2.5e-6)]);
            }
            other => panic!("expected WattEnergyDependent, got {other:?}"),
        }
    }

    /// NK=2 (mixture): two Maxwell partitions with different θ(E) — verifies the
    /// per-partition cursor advance repeats cleanly across partition boundaries
    /// and the result is wrapped in [`FissionSpectrum::Mixture`] (not collapsed,
    /// since NK≠1).
    #[test]
    fn parses_nk2_mixture() {
        let mut rows = vec![[94239.0, 237.0, 0.0, 0.0, 2.0, 0.0]]; // HEAD: NK=2
        rows.extend(tab1_rows(0.0, 7, 2, 2, 1.0e-5, 0.5, 2.0e7, 0.5)); // p_1=0.5
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 1.0e6, 2.0e7, 1.0e6)); // θ_1
        rows.extend(tab1_rows(0.0, 9, 2, 2, 1.0e-5, 0.5, 2.0e7, 0.5)); // p_2=0.5
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 2.0e6, 2.0e7, 2.0e6)); // θ_2

        let chi = parse_mf5_section(&rows).unwrap().unwrap();
        match chi {
            FissionSpectrum::Mixture(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(parts[0].1, FissionSpectrum::Maxwell { .. }));
                assert!(matches!(parts[1].1, FissionSpectrum::Evaporation { .. }));
            }
            other => panic!("expected Mixture, got {other:?}"),
        }
    }

    /// LF=5 (general evaporation) has no sampling algorithm even in canonical
    /// OpenMC — the whole parse aborts to `None` (caller falls back to Watt),
    /// exactly like an absent section.
    #[test]
    fn lf5_general_evaporation_falls_back_to_none() {
        let mut rows = vec![[92235.0, 233.0, 0.0, 0.0, 1.0, 0.0]];
        rows.extend(tab1_rows(0.0, 5, 2, 2, 1.0e-5, 1.0, 2.0e7, 1.0));
        rows.extend(tab1_rows(0.0, 0, 2, 2, 1.0e-5, 1.0e6, 2.0e7, 1.5e6)); // θ(E)
        rows.extend(tab1_rows(0.0, 0, 2, 2, 0.0, 0.0, 1.0, 1.0)); // g(x)

        assert!(parse_mf5_section(&rows).unwrap().is_none());
    }
}
