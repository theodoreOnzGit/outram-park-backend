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
                let (energy, nu_total): (Vec<f64>, Vec<f64>) = tab1.pairs.iter().copied().unzip();
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
    /// The evaluation's own **incident-energy** interpolation law, as ENDF
    /// `(NBT, INT)` ranges from the MF=6 LAW=1 / MF=5 LF=1 TAB2.
    ///
    /// # Why this is carried, and what is actually honoured
    ///
    /// Until 2026-09-16 this was **dropped at conversion** — the parser read it
    /// and the transport-side structure threw it away, so no consumer could
    /// even see what the evaluation asked for. It is carried now so the
    /// information is not silently lost.
    ///
    /// **What the samplers do is still unit-base interpolation in every case.**
    /// ENDF File 6 uses the extended codes here: `11..=15` is
    /// *corresponding-point* interpolation (scheme `INT − 10`) and `21..=25` is
    /// *unit-base* (scheme `INT − 20`). Measured across this workspace's 27
    /// neutron tapes on MT=16/17/91 (`acer::energy::mf6`'s
    /// `survey_incident_energy_interpolation_laws`): **17 ranges are INT=22
    /// (unit-base) and 9 are INT=12 (corresponding-point)**.
    ///
    /// So roughly a third of the ranges specify corresponding-point and get
    /// unit-base. That is a real discrepancy against the *evaluation* — but it
    /// is **not** a discrepancy against this crate's reference implementation:
    /// NJOY's ACER preserves the flag into the ACE Law-4 header, OpenMC applies
    /// unit-base regardless, and this port's sampled spectra reproduce OpenMC's
    /// construction to 0.02 % (`outram-mc-libs`'s
    /// `mt91_transfer_vs_openmc.rs`). Diverging from OpenMC here is a
    /// maintainer decision, not something to do silently — hence: carried,
    /// measured, documented, not yet acted on.
    ///
    /// Empty when the source carried no TAB2 interpolation record.
    pub incident_interp: Vec<(u32, u32)>,
}

impl Default for FissionSpectrum {
    /// A representative fast-fission Watt spectrum (U-235 thermal-fission params).
    fn default() -> Self {
        FissionSpectrum::Watt {
            a: 0.988e6,
            b: 2.249e-6,
        }
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
                1.5 * crate::endf::interp::eval_tab1(e_in, &theta.interp, &theta.pairs)
                    .unwrap_or(0.0)
            }
            FissionSpectrum::Evaporation { theta, .. } => {
                2.0 * crate::endf::interp::eval_tab1(e_in, &theta.interp, &theta.pairs)
                    .unwrap_or(0.0)
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
            1 => Some(FissionSpectrum::ContinuousTabular(parse_lf1_tabular(
                &mut cur,
            )?)),
            7 => Some(FissionSpectrum::Maxwell {
                theta: cur.read_tab1()?,
                u,
            }),
            9 => Some(FissionSpectrum::Evaporation {
                theta: cur.read_tab1()?,
                u,
            }),
            11 => {
                let a = cur.read_tab1()?;
                let b = cur.read_tab1()?;
                Some(FissionSpectrum::WattEnergyDependent { a, b, u })
            }
            // LF=5 (general evaporation) and LF=12 (Madland-Nix) are not read
            // here. **NJOY DOES support LF=5** — `groupr.f90:12355`, "law 5.
            // general evaporation spectrum", and `acefc.f90:2251/2477/6889`
            // handle it too. An earlier comment here called it "unsupported
            // upstream", which was wrong and is the kind of error that stops
            // someone porting something. LF=5 is a tabulated `g(x)` with
            // `x = E'/θ(E)` — the same "universal shape, incident-dependent
            // scale" form as MF=6 LAW=6, so it would convert the same way.
            // ~~Not yet ported because no evaluation in `reference-data/endf/`
            // uses it~~ **CORRECTED 2026-09-25.** Seven held tapes DO use LF=5
            // -- U-234, U-235 (VII.0 and VIII.0), U-238 (VII.0, VIII.0,
            // JENDL-3.3) and Pu-239 JENDL-3.3 -- all of them on **MT=455**, the
            // delayed-neutron spectra. The old claim held only for the MTs
            // `mf5_lf_survey` walks (18, 16, 91, 5), which is where it was
            // measured; MT=455 was never in that set, so the survey could not
            // have failed and the "will fail if one is added" was not true of
            // LF=5 either.
            //
            // Nothing is silently degraded by the omission: the delayed
            // *spectrum* is dropped on both routes (`DelayedData` keeps
            // `fraction`, not `spectrum`), and NJOY's ACER linearises those
            // MT=455 LF=5 sections into ACE LAW=4 -- measured as `{LAW4: 6}` in
            // the DNED block of every U-234/235/238 table in
            // `reference-data/ace`. So LF=5 reaches neither route's sampler and
            // porting it would gain nothing until delayed spectra are carried.
            // Verified by scanning all 79 tapes' MF=5 subsection headers, not by
            // re-reading the previous note.
            _ => None,
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

/// One neutron-emitting subsection of an ENDF **MF=6 LAW=1** reaction.
///
/// An (n,2n) reaction is written either as a single subsection of yield 2
/// (U-238, U-235 in ENDF/B-VIII.0) or as two subsections of yield 1 carrying
/// *different* spectra for the first and second emitted neutron (F-19, same
/// library). One branch is one subsection, so both conventions are represented
/// without the consumer having to know which it got.
#[derive(Debug, Clone)]
pub struct ContinuumBranch {
    /// This subsection's outgoing-energy law `f₀(E→E')`, incident and outgoing
    /// grids in **eV**, pdf in eV⁻¹ — the same representation as the MF=5 LF=1
    /// fission spectrum, so it samples through the identical code path.
    pub spectrum: ChiTabular,
    /// This subsection's neutron multiplicity `y(E)` as `(E \[eV\], y)` pairs.
    pub yield_pairs: Vec<(f64, f64)>,
    /// The **angular** half of the law, correlated with the outgoing energy.
    ///
    /// MF=6 LAW=1 is a correlated energy-angle law: the emission cosine depends
    /// on which outgoing energy was drawn, so this is indexed by the same
    /// `(incident table, outgoing row)` pair that `spectrum` was sampled at.
    /// See [`ContinuumAngular`] for what each variant means — in particular, it
    /// distinguishes "the evaluation says isotropic" from "this port cannot
    /// sample this representation yet", which are the same number and very
    /// different facts.
    pub angular: ContinuumAngular,
}

/// The angular half of an MF=6 LAW=1 continuum emission law, in the form a
/// Monte Carlo code samples.
///
/// # The distinction this type exists to make
///
/// Sampling a continuum neutron isotropically can be right or wrong, and the
/// outgoing direction alone cannot tell you which. This enum records *why* a
/// given law is isotropic, so a reader — and an ablation study — can tell a
/// faithful reading of the evaluation from an unported representation. Until
/// bead `op-og56` the coefficients were dropped at parse time and every
/// continuum emission was [`EvaluatedIsotropic`](Self::EvaluatedIsotropic) by
/// construction, with nothing in the data structures to say otherwise.
#[derive(Debug, Clone)]
pub enum ContinuumAngular {
    /// The evaluation itself declares isotropic emission — ENDF `NA = 0` on
    /// every outgoing-energy row of every incident energy. Sampling
    /// `μ = 2ξ − 1` is then **correct**, not a fallback.
    ///
    /// ENDF/B-VIII.0's F-19 MT=91 is the reference example.
    EvaluatedIsotropic,
    /// `LANG = 1`: the evaluation's Legendre coefficients, linearised into a
    /// per-row tabulated cosine CDF by
    /// [`crate::acer::angular::legendre_cosine_law`].
    ///
    /// `tables[i]` corresponds to `spectrum.tables[i]`, and `tables[i].rows[k]`
    /// to that table's outgoing-energy point `k`.
    Legendre(Vec<ContinuumAngularTable>),
    /// `LANG = 2`: Kalbach-Mann, as `(r, a)` per outgoing-energy row.
    ///
    /// `r` is the pre-compound fraction the evaluation tabulates. `a` is the
    /// slope: tabulated too when `NA = 2`, and otherwise computed from the
    /// Kalbach-86 systematics by [`crate::groupr::kinematics::bach`] — which is
    /// a function of the projectile, ejectile and target masses, hence of the
    /// nuclide rather than of the emission law alone.
    ///
    /// ENDF/B-VIII.0's O-16 and Al-27 use this on both MT=16 and MT=91, with
    /// `NA = 1` throughout, so the systematics path is the live one.
    KalbachMann(Vec<ContinuumKalbachTable>),
    /// **MF=6 LAW=7** (lab-frame angle-then-energy): a tabulated cosine CDF per
    /// outgoing-energy row, exactly as [`Legendre`](Self::Legendre) stores one,
    /// but built from the evaluation's own per-cosine spectra rather than from
    /// Legendre coefficients.
    ///
    /// Kept as its own variant rather than folded into `Legendre` for two
    /// reasons. It is a **different representation** — LAW=7 tabulates
    /// `f(mu, E')` directly and the cosine law is obtained by slicing it at
    /// fixed `E'`, not by linearising a series — and it is **laboratory-frame by
    /// construction** (ENDF-102: LAW=7 data is in the lab regardless of `LCT`),
    /// so it must not acquire a CM→lab transform if one is ever added to the
    /// `Legendre` path. A caller inspecting the enum can tell which it has.
    LabTabulated(Vec<ContinuumAngularTable>),
    /// A representation this port retains but does not sample — `LANG = 11…15`
    /// (tabulated cosines), or any `LANG` value ENDF adds later.
    ///
    /// Emission falls back to isotropic. **That fallback is a port gap, not the
    /// evaluation's statement**, and this variant is what makes the difference
    /// visible to a caller instead of leaving it in a source comment.
    ///
    /// `LANG = 2` (Kalbach-Mann) used to land here and no longer does — it is
    /// sampled via [`ContinuumAngular::KalbachMann`]. No evaluation in
    /// `reference-data/endf/` currently reaches this variant.
    Unported(crate::acer::energy::Mf6AngularLaw),
    /// The law was read and then **deliberately switched off**, by
    /// [`ContinuumEmission::with_isotropic_angle`]. The ablation arm of a paired
    /// worth measurement.
    ///
    /// A third distinct reason to emit isotropically, kept distinct on purpose:
    /// an ablated nuclide is self-describing, so a run cannot quietly report an
    /// ablation arm's number as the physical one, and an ablation that failed to
    /// take effect is visible in the data rather than only in a `Δk` that came
    /// back suspiciously small.
    Ablated,
}

/// One incident energy's worth of correlated angular laws, one entry per
/// outgoing-energy row of the matching [`ChiEout`].
#[derive(Debug, Clone)]
pub struct ContinuumAngularTable {
    /// Per-outgoing-energy angular laws, aligned with `ChiEout::e_out`.
    pub rows: Vec<ContinuumAngularRow>,
}

/// One incident energy's worth of Kalbach-Mann parameters, one entry per
/// outgoing-energy row of the matching [`ChiEout`].
#[derive(Debug, Clone)]
pub struct ContinuumKalbachTable {
    /// Per-outgoing-energy `(r, a)` pairs, aligned with `ChiEout::e_out`.
    pub rows: Vec<ContinuumKalbachRow>,
}

/// The Kalbach-Mann emission-cosine law conditional on one `(incident energy,
/// outgoing energy)` pair.
///
/// The density is
///
/// ```text
/// f(mu) = a [cosh(a mu) + r sinh(a mu)] / (2 sinh a),   mu in [-1, 1]
/// ```
///
/// which integrates to 1 over `[-1, 1]` for any `a > 0` and any `r` (the `sinh`
/// term is odd and contributes nothing to the norm). `r` is the **pre-compound
/// fraction**, a number in `[0, 1]`; `a` is the **slope**, which grows with
/// incident energy and makes the distribution forward-peaked.
///
/// Cosines are in the frame the MF=6 section names (`LCT`).
#[derive(Debug, Clone, Copy)]
pub struct ContinuumKalbachRow {
    /// Pre-compound fraction `r`, tabulated by the evaluation. In `[0, 1]`.
    pub r: f64,
    /// Slope `a`. Tabulated when `NA = 2`, otherwise from the Kalbach-86
    /// systematics.
    pub a: f64,
}

impl ContinuumKalbachRow {
    /// An isotropic row — `a → 0` is the isotropic limit of the Kalbach form,
    /// and is represented exactly rather than approached.
    pub fn isotropic() -> Self {
        ContinuumKalbachRow { r: 0.0, a: 0.0 }
    }

    /// Whether this row carries no angular structure.
    ///
    /// True when the slope has collapsed (`a ≈ 0`, the isotropic limit) — note
    /// `r = 0` alone does **not** make the law isotropic, because the `cosh`
    /// term is still peaked at both ends. That asymmetry is why this is a
    /// method and not a field comparison at the call site.
    pub fn is_isotropic(&self) -> bool {
        self.a.abs() < 1.0e-12
    }

    /// The exact mean cosine of this row's density,
    /// `⟨μ⟩ = r · (coth a − 1/a)`.
    ///
    /// The bracket is the Langevin function `L(a)`, which runs from `0` at
    /// `a = 0` to `1` as `a → ∞`. Every `cosh` term integrates to zero against
    /// `μ`, so the mean is carried entirely by `r`.
    ///
    /// This is a closed form, not a quadrature, which makes it a genuine oracle
    /// for [`sample_mu`](Self::sample_mu) — the sampler inverts the CDF and this
    /// integrates the density, so agreement between them tests both.
    pub fn mubar(&self) -> f64 {
        if self.is_isotropic() {
            return 0.0;
        }
        let a = self.a;
        // coth(a) - 1/a, guarded at small a where both terms blow up: the
        // series is a/3 - a^3/45 + ...
        let langevin = if a.abs() < 1.0e-4 {
            a / 3.0
        } else {
            1.0 / a.tanh() - 1.0 / a
        };
        self.r * langevin
    }

    /// Invert this row's cosine CDF at a uniform variate `xi ∈ [0, 1)`.
    ///
    /// # The closed form, and why it is one variate and not two
    ///
    /// The CDF integrates in elementary functions:
    ///
    /// ```text
    /// F(mu) = [sinh(a mu) + sinh(a) + r (cosh(a mu) - cosh(a))] / (2 sinh a)
    /// ```
    ///
    /// and `sinh(a mu) + r cosh(a mu)` collapses to a single hyperbolic sine,
    /// `sqrt(1 - r^2) sinh(a mu + phi)` with `phi = atanh(r)`, so `F` inverts
    /// directly:
    ///
    /// ```text
    /// mu = [ asinh( ((2 xi - 1) sinh a + r cosh a) / sqrt(1 - r^2) ) - phi ] / a
    /// ```
    ///
    /// **One variate matters here.** The usual implementation splits on `r` and
    /// spends two: one to pick the `cosh` or `sinh` branch, one to invert it.
    /// That would consume a different number of draws from the isotropic
    /// fallback, so an ablation of this law would shift the random stream and a
    /// measured `Δk` would mix physics with re-randomisation. The single-variate
    /// inverse keeps the ablation attributable — the property
    /// `outram-mc-libs`' `tests/continuum_angular_ablation_control.rs` asserts.
    ///
    /// Degenerate cases return the isotropic inverse `2ξ − 1`: `a ≈ 0` (the
    /// isotropic limit of the form itself) and `|r| ≥ 1` (which would make
    /// `atanh` diverge; `r` is a probability and should never reach 1, so this
    /// is a guard rather than a path).
    pub fn sample_mu(&self, xi: f64) -> f64 {
        let xi = xi.clamp(0.0, 1.0);
        let (r, a) = (self.r, self.a);
        if self.is_isotropic() || r.abs() >= 1.0 || !a.is_finite() || !r.is_finite() {
            return 2.0 * xi - 1.0;
        }
        let root = (1.0 - r * r).sqrt();
        if root <= 0.0 {
            return 2.0 * xi - 1.0;
        }
        let phi = r.atanh();
        let s = ((2.0 * xi - 1.0) * a.sinh() + r * a.cosh()) / root;
        let mu = (s.asinh() - phi) / a;
        if mu.is_finite() {
            mu.clamp(-1.0, 1.0)
        } else {
            2.0 * xi - 1.0
        }
    }
}

/// The emission cosine law conditional on one `(incident energy, outgoing
/// energy)` pair, as a tabulated CDF ready to invert.
///
/// Cosines are in the frame the MF=6 section names (`LCT`), which is the centre
/// of mass for every actinide MT=91 in ENDF/B-VIII.0 — the transport layer must
/// transform to the laboratory frame along with the energy.
#[derive(Debug, Clone)]
pub struct ContinuumAngularRow {
    /// Ascending cosine grid on `[−1, 1]`. Empty ⇒ this row is isotropic.
    pub cosines: Vec<f64>,
    /// Cumulative distribution on `cosines` (`cdf[0] = 0`, `cdf[last] = 1`).
    pub cdf: Vec<f64>,
    /// The row's mean cosine `⟨μ⟩`, equal to the normalised `a₁ = f₁/f₀`.
    /// Retained because it is the single number a transport-corrected model
    /// needs, and because it is what an ablation control asserts is non-zero.
    pub mubar: f64,
}

impl ContinuumAngularRow {
    /// An isotropic row — no tabulated data, `⟨μ⟩ = 0`.
    pub fn isotropic() -> Self {
        ContinuumAngularRow {
            cosines: Vec::new(),
            cdf: Vec::new(),
            mubar: 0.0,
        }
    }

    /// Whether this row carries no angular structure.
    pub fn is_isotropic(&self) -> bool {
        self.cosines.is_empty()
    }

    /// Invert this row's cosine CDF at a uniform variate `xi ∈ [0, 1)`.
    ///
    /// Returns `μ` in the law's own frame. An isotropic row gives the flat
    /// inverse `2ξ − 1`, so a caller never needs to branch on
    /// [`is_isotropic`](Self::is_isotropic) for correctness.
    pub fn sample_mu(&self, xi: f64) -> f64 {
        let xi = xi.clamp(0.0, 1.0);
        let n = self.cosines.len();
        if n < 2 {
            return 2.0 * xi - 1.0;
        }
        // Locate the CDF bin, then interpolate linearly within it. The stored
        // pdf is lin-lin, so a strictly correct inverse is the quadratic one;
        // linear interpolation of the CDF is used instead because the grid is
        // already bisected to ANGLE_TOL, which bounds the difference well below
        // the tolerance the tabulation itself carries.
        let mut k = 0usize;
        while k + 2 < n && self.cdf[k + 1] <= xi {
            k += 1;
        }
        let (c0, c1) = (self.cdf[k], self.cdf[k + 1]);
        let (m0, m1) = (self.cosines[k], self.cosines[k + 1]);
        if c1 > c0 {
            (m0 + (xi - c0) / (c1 - c0) * (m1 - m0)).clamp(-1.0, 1.0)
        } else {
            m0
        }
    }
}

impl ContinuumAngular {
    /// The angular law for outgoing-energy row `row` of incident table
    /// `table`, or `None` when this representation is not sampled.
    ///
    /// `None` and an isotropic row are deliberately different returns: the
    /// first means "no angular information is available here", the second means
    /// "the evaluation says the emission is flat".
    pub fn row(&self, table: usize, row: usize) -> Option<&ContinuumAngularRow> {
        match self {
            ContinuumAngular::EvaluatedIsotropic
            | ContinuumAngular::Unported(_)
            | ContinuumAngular::Ablated
            | ContinuumAngular::KalbachMann(_) => None,
            ContinuumAngular::Legendre(tables) | ContinuumAngular::LabTabulated(tables) => {
                tables.get(table)?.rows.get(row)
            }
        }
    }

    /// Sample the emission cosine for outgoing-energy row `row` of incident
    /// table `table`, given one uniform variate `xi ∈ [0, 1)`.
    ///
    /// This is the interface transport should use: it dispatches over the
    /// representation so a caller never has to know whether the evaluation
    /// stored Legendre coefficients or Kalbach-Mann parameters.
    ///
    /// Returns `None` when no angular information is available — a law that is
    /// evaluated-isotropic, unported, or ablated — which is deliberately
    /// distinct from returning `0.0`. **Every arm consumes exactly one
    /// variate**, including the `None` case at the call site, so switching
    /// between them does not shift the random stream.
    pub fn sample_mu(&self, table: usize, row: usize, xi: f64) -> Option<f64> {
        match self {
            ContinuumAngular::EvaluatedIsotropic
            | ContinuumAngular::Unported(_)
            | ContinuumAngular::Ablated => None,
            ContinuumAngular::Legendre(tables) | ContinuumAngular::LabTabulated(tables) => {
                Some(tables.get(table)?.rows.get(row)?.sample_mu(xi))
            }
            ContinuumAngular::KalbachMann(tables) => {
                Some(tables.get(table)?.rows.get(row)?.sample_mu(xi))
            }
        }
    }

    /// The exact mean cosine of row `row` of table `table`, or `None` where no
    /// angular law is available.
    ///
    /// Closed form in both representations — `a₁` for Legendre, `r·L(a)` for
    /// Kalbach-Mann — so this is an oracle for
    /// [`sample_mu`](Self::sample_mu) rather than a second estimate of it.
    pub fn mubar(&self, table: usize, row: usize) -> Option<f64> {
        match self {
            ContinuumAngular::EvaluatedIsotropic
            | ContinuumAngular::Unported(_)
            | ContinuumAngular::Ablated => None,
            ContinuumAngular::Legendre(tables) | ContinuumAngular::LabTabulated(tables) => {
                Some(tables.get(table)?.rows.get(row)?.mubar)
            }
            ContinuumAngular::KalbachMann(tables) => {
                Some(tables.get(table)?.rows.get(row)?.mubar())
            }
        }
    }

    /// Whether any row of this law carries angular structure.
    ///
    /// The assertion an ablation control needs: switching off a law that was
    /// already flat produces no difference and reads as "this physics does not
    /// matter".
    pub fn is_anisotropic(&self) -> bool {
        match self {
            ContinuumAngular::EvaluatedIsotropic
            | ContinuumAngular::Unported(_)
            | ContinuumAngular::Ablated => false,
            ContinuumAngular::Legendre(tables) | ContinuumAngular::LabTabulated(tables) => tables
                .iter()
                .any(|t| t.rows.iter().any(|r| !r.is_isotropic())),
            ContinuumAngular::KalbachMann(tables) => tables
                .iter()
                .any(|t| t.rows.iter().any(|r| !r.is_isotropic())),
        }
    }

    /// The largest `|⟨μ⟩|` anywhere in this law; `0.0` when it carries none.
    pub fn peak_mubar(&self) -> f64 {
        match self {
            ContinuumAngular::EvaluatedIsotropic
            | ContinuumAngular::Unported(_)
            | ContinuumAngular::Ablated => 0.0,
            ContinuumAngular::Legendre(tables) | ContinuumAngular::LabTabulated(tables) => tables
                .iter()
                .flat_map(|t| t.rows.iter().map(|r| r.mubar.abs()))
                .fold(0.0, f64::max),
            ContinuumAngular::KalbachMann(tables) => tables
                .iter()
                .flat_map(|t| t.rows.iter().map(|r| r.mubar().abs()))
                .fold(0.0, f64::max),
        }
    }
}

impl ContinuumBranch {
    /// Multiplicity `y` at incident energy `e_in` \[eV\], lin-lin interpolated
    /// and clamped to the end values outside the tabulated range. Returns 1.0 if
    /// the table is empty.
    pub fn yield_at(&self, e_in: f64) -> f64 {
        let p = &self.yield_pairs;
        match p.len() {
            0 => 1.0,
            1 => p[0].1,
            _ => {
                if e_in <= p[0].0 {
                    return p[0].1;
                }
                if e_in >= p[p.len() - 1].0 {
                    return p[p.len() - 1].1;
                }
                for i in 1..p.len() {
                    let (x0, y0) = p[i - 1];
                    let (x1, y1) = p[i];
                    if e_in <= x1 {
                        return if x1 > x0 {
                            y0 + (y1 - y0) * (e_in - x0) / (x1 - x0)
                        } else {
                            y1
                        };
                    }
                }
                p[p.len() - 1].1
            }
        }
    }
}

/// The secondary-neutron emission of an ENDF **MF=6 LAW=1** reaction — the
/// continuum inelastic (MT=91) and (n,xn) energy distributions.
///
/// # Why this exists
///
/// RECONR reconstructs MF=3 cross sections but no secondary-energy law, so
/// `outram-mc-libs` modelled the MT=91 continuum with a **Weisskopf evaporation
/// stand-in**. That is a shape assumption, not the evaluation's own data, and
/// MT=91 carries 10–25 % of the collisions in a bare fast metal sphere (measured
/// on Godiva, 2026-09-13 — see `gh:#192`). This type carries the evaluated law
/// instead, in the same [`ChiTabular`] form the MF=5 LF=1 fission spectrum uses,
/// so the transport layer samples it with the machinery it already has.
///
/// # What it does and does not carry
///
/// The **energy** spectrum `f₀(E→E')` of every leading neutron subsection is
/// carried in full. The **angular** correlation present in MF=6 (Legendre
/// `f₁…f_NA` when LANG=1, Kalbach `r`/`a` when LANG=2) is **not**: emission is
/// isotropic in the frame named by [`cm_frame`](Self::cm_frame), which is the
/// same reduction ACE Law 4 makes (see
/// [`crate::acer::energy::Mf6Neutron`]). Correlated emission is the follow-up,
/// not something this type approximates.
#[derive(Debug, Clone)]
pub struct ContinuumEmission {
    /// One entry per neutron (ZAP=1) LAW=1 subsection, in file order. Never
    /// empty — [`from_endf_mf6`](Self::from_endf_mf6) returns `None` rather than
    /// an emission with no branches.
    pub branches: Vec<ContinuumBranch>,
    /// `true` when the distributions are tabulated in the **centre-of-mass**
    /// frame (ENDF `LCT = 2`), which is the usual case for MT=91 on actinide
    /// evaluations. The transport layer must then transform the sampled `E'` to
    /// the laboratory frame; `false` means it is already laboratory-frame.
    /// `LCT` is a property of the whole MF=6 section, so it is shared by every
    /// branch.
    pub cm_frame: bool,
}

/// A **pre-ENDF-6 uncorrelated** neutron emission law: the outgoing energy from
/// **MF=5** and the emission cosine from **MF=4**, drawn independently.
///
/// # When this is the law
///
/// A modern evaluation writes a continuum or multiplying reaction as MF=6, whose
/// energy and angle are *correlated* — [`ContinuumEmission`]. An older one puts
/// the energy spectrum in MF=5 and the angular distribution in MF=4, with no
/// correlation between them. Both say what a neutron does; they are different
/// representations, not different fidelities of the same one.
///
/// # Why a separate type rather than a conversion
///
/// Every other law in this module converts into [`ChiTabular`] so it can reuse
/// the existing samplers (see [`ContinuumEmission::from_endf_mf6`]'s LAW=6 and
/// LAW=7 paths). This one deliberately does not, for three reasons:
///
/// 1. **Both halves already have exact samplers.** `outram-mc-libs` samples every
///    MF=5 `LF` law — including the analytic LF=7/9/11 — through `sample_chi`,
///    and MF=4 through `sample_mf4_mu_cm`, which implements OpenMC's
///    statistical-neighbour convention for the AND block. Converting would
///    replace two exact paths with one tabulated approximation, which is
///    backwards.
/// 2. **The angular grid is its own.** MF=4's incident-energy grid is unrelated
///    to MF=5's. Forcing the cosine law onto the energy law's grid would either
///    resample it or index it wrongly; keeping the section intact avoids the
///    question.
/// 3. **Uncorrelated is a physical statement worth keeping visible.** Folding it
///    into a correlated structure by repeating one cosine law across every
///    outgoing row would say the same thing while hiding it.
///
/// # Frame
///
/// [`lct`](Self::lct) comes from **MF=4**, which is how NJOY decides it too:
/// `acefc.f90:5825-5869` reads `lct` from MF=4's second CONT record and makes the
/// ACE `TY` negative when `lct >= 2`. MF=5 carries no frame flag — ENDF-102
/// defines its secondary energies as laboratory always.
///
/// Measured across `reference-data/endf/`: **all 11** MF=4/MT=16/17/91 sections
/// are `LCT = 1` (laboratory), so no held evaluation exercises the CM branch.
/// [`from_endf`](Self::from_endf) therefore refuses `LCT >= 2` rather than
/// guessing at a transform it has no case to check against — an honest `None`
/// that leaves the caller's documented fallback, not a silent approximation.
#[derive(Debug, Clone)]
pub struct UncorrelatedEmission {
    /// Outgoing-energy law from MF=5. Despite the type's name this is not
    /// necessarily fission — [`FissionSpectrum`] is simply this crate's
    /// representation of an MF=5 section, and MF=5 is the same format wherever
    /// it appears.
    pub energy: FissionSpectrum,
    /// Emission-cosine law from MF=4, on its own incident-energy grid. Empty
    /// [`energies`](crate::acer::angular::ElasticAngular::energies) means the
    /// evaluation declares the reaction isotropic (`LTT = 0` or `LI = 1`), which
    /// is a statement, not an absence — C-12's MT=91 is exactly this.
    pub angular: crate::acer::angular::ElasticAngular,
    /// Reference frame from MF=4's `LCT`: `1` laboratory, `2` centre of mass.
    /// Always `1` for every section in `reference-data/endf/`; see the type docs.
    pub lct: i32,
    /// Neutron multiplicity for this reaction — `2` for MT=16, `3` for MT=17,
    /// `1` for MT=91. Taken from the MT, as ACER does
    /// (`acefc.f90:5857-5866` sets `n` per MT), because MF=4/MF=5 evaluations
    /// carry no yield record of their own.
    pub yield_n: u32,
}

impl UncorrelatedEmission {
    /// Read the MF=4 + MF=5 emission law for reaction `mt`, or `Ok(None)` when
    /// this representation does not apply or cannot be read honestly.
    ///
    /// # Returns `Ok(None)` when
    ///
    /// - there is no MF=5 section for `mt` — nothing to read;
    /// - MF=5 uses an `LF` this crate has not ported (`parse_mf5_section`
    ///   returns `None`), rather than a partially-read mixture;
    /// - MF=4 declares `LCT >= 2`. No evaluation in `reference-data/endf/` does,
    ///   so a centre-of-mass branch here would be untested code deciding a frame
    ///   transform. Refusing is the failure direction that shows up as a
    ///   stand-in rather than as a wrong answer.
    ///
    /// **This is not a fallback path for MF=6.** Callers should try
    /// [`ContinuumEmission::from_endf_mf6`] first; an evaluation carrying both
    /// is answering the same question twice, and MF=6 is the answer ACER uses.
    pub fn from_endf(
        tape: &crate::endf::tape::Tape,
        mat: i32,
        mt: i32,
    ) -> Result<Option<UncorrelatedEmission>, crate::NjoyError> {
        let Some(energy) = FissionSpectrum::from_endf_mf5_mt(tape, mat, mt)? else {
            return Ok(None);
        };
        // MF=4 may legitimately be absent; ENDF then means isotropic emission.
        let angular = match tape.section(mat, 4, mt) {
            Some(sec) => crate::acer::angular::parse_mf4_angular(sec)?,
            None => crate::acer::angular::ElasticAngular {
                energies: Vec::new(),
                lct: 1,
            },
        };
        if angular.lct >= 2 {
            return Ok(None);
        }
        let yield_n = match mt {
            16 => 2,
            17 => 3,
            37 => 4,
            _ => 1,
        };
        let lct = angular.lct;
        Ok(Some(UncorrelatedEmission {
            energy,
            angular,
            lct,
            yield_n,
        }))
    }

    /// Whether the emission cosine carries any structure at all.
    ///
    /// `false` means the evaluation itself declares isotropic emission (`LTT = 0`
    /// or `LI = 1`), which is a different fact from "this port cannot read it" —
    /// the same distinction [`ContinuumAngular`] draws.
    pub fn is_anisotropic(&self) -> bool {
        !self.angular.is_all_isotropic()
    }
}

/// Convert an ENDF **MF=6 LAW=7** (lab-frame angle-then-energy) emission into
/// the [`ChiTabular`] + per-row cosine-CDF form the transport samplers already
/// consume.
///
/// # The representation, and why it converts exactly
///
/// LAW=7 tabulates the joint density `f(mu, E')` directly: at each incident
/// energy a cosine grid, and at each cosine a `(E', f)` spectrum. What the
/// samplers want instead is the marginal `f(E')` plus the conditional
/// `P(mu | E')`. Both come out of the same table:
///
/// ```text
/// f(mu_j, E')  =  w_j * p_j(E')          w_j = the cosine's retained weight
/// f(E')        =  integral over mu of f(mu, E')      (marginal)
/// P(mu | E')  propto  f(mu, E')  at fixed E'         (conditional)
/// ```
///
/// where `p_j` is cosine `j`'s unit-area table and `w_j` its `Law7MuTable::weight`
/// — the integral the parser used to discard (see that field's documentation).
///
/// The one construction step is a **merged outgoing-energy grid**: the union of
/// every cosine's own `E'` knots at that incident energy. That is exact rather
/// than approximate, and it is the reason this needs no new sampling path —
/// evaluating a piecewise-linear density on a *superset* of its own knots
/// reproduces it identically, so nothing is resampled or smoothed. The
/// integration over `mu` is the trapezoid rule, which is precisely ENDF
/// `INTMU = 2` (lin-lin); a section declaring `INTMU = 1` (histogram) would need
/// the other rule, so that case returns `Ok(None)` and keeps the caller's
/// documented fallback rather than silently applying the wrong one.
///
/// # Frame
///
/// **Laboratory, by construction.** ENDF-102 defines LAW=7 data in the lab frame
/// regardless of the section's `LCT`, so the emission is built with
/// `cm_frame = false` and the transport layer applies no CM→lab transform — it
/// already has the branch for this
/// (`outram_mc_libs::physics::scatter::continuum_inelastic_scatter_evaluated_with`).
/// Putting a lab spectrum through the CM transform is the failure mode this
/// paragraph exists to prevent.
///
/// # Angular variant
///
/// [`ContinuumAngular::LabTabulated`], not `Legendre` — same row type, different
/// provenance and a different frame guarantee. See that variant's docs.
///
/// # Returns
///
/// `Ok(None)` — never a fabricated law — when the section carries no `ZAP=1,
/// LAW=7` neutron subsection, when `INTMU` is not lin-lin, or when a table is
/// too short to integrate. The caller then keeps its existing fallback.
fn lab_angle_energy_emission(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    mt: i32,
) -> Result<Option<ContinuumEmission>, crate::NjoyError> {
    let Some(sec) = tape.section(mat, 6, mt) else {
        return Ok(None);
    };
    let law7 = match crate::acer::energy::parse_mf6_law7_lab_angle_energy(sec) {
        Ok(l) => l,
        Err(crate::NjoyError::NotPorted(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    if law7.incident.len() < 2 {
        return Ok(None);
    }

    const EMEV: f64 = 1.0e6;
    let mut incident = Vec::with_capacity(law7.incident.len());
    let mut tables = Vec::with_capacity(law7.incident.len());
    let mut ang_tables = Vec::with_capacity(law7.incident.len());

    for inc in &law7.incident {
        // Only lin-lin over mu; see the note above on INTMU = 1.
        if inc.mu_interp != 2 || inc.mu.len() < 2 || inc.mu.len() != inc.tables.len() {
            return Ok(None);
        }

        // Merged E' grid [MeV]: the union of every cosine's own knots.
        let mut grid: Vec<f64> = inc
            .tables
            .iter()
            .flat_map(|t| t.e_out_mev.iter().copied())
            .collect();
        grid.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        grid.dedup_by(|a, b| (*a - *b).abs() <= 0.0);
        if grid.len() < 2 {
            return Ok(None);
        }

        // f(mu_j, E'_k) on the merged grid, cosine-major.
        let joint: Vec<Vec<f64>> = inc
            .tables
            .iter()
            .map(|t| {
                grid.iter()
                    .map(|&e| t.weight * lin_interp_zero_outside(&t.e_out_mev, &t.pdf, e))
                    .collect()
            })
            .collect();

        // Marginal f(E') = integral over mu, trapezoid (= INTMU 2).
        let mut pdf = vec![0.0f64; grid.len()];
        for k in 0..grid.len() {
            let mut acc = 0.0;
            for j in 1..inc.mu.len() {
                acc += 0.5 * (joint[j][k] + joint[j - 1][k]) * (inc.mu[j] - inc.mu[j - 1]);
            }
            pdf[k] = acc.max(0.0);
        }

        // Conditional P(mu | E') at each merged grid point.
        let mut rows = Vec::with_capacity(grid.len());
        for k in 0..grid.len() {
            let slice: Vec<f64> = (0..inc.mu.len()).map(|j| joint[j][k]).collect();
            rows.push(cosine_row_from_slice(&inc.mu, &slice));
        }

        // The outgoing-energy CDF, in eV, normalised.
        let e_out: Vec<f64> = grid.iter().map(|&e| e * EMEV).collect();
        let pdf_ev: Vec<f64> = pdf.iter().map(|&p| p / EMEV).collect();
        let mut cdf = vec![0.0f64; e_out.len()];
        for i in 1..e_out.len() {
            cdf[i] = cdf[i - 1] + 0.5 * (pdf_ev[i] + pdf_ev[i - 1]) * (e_out[i] - e_out[i - 1]);
        }
        let total = *cdf.last().unwrap_or(&0.0);
        let (pdf_ev, cdf) = if total > 0.0 {
            (
                pdf_ev.iter().map(|&p| p / total).collect::<Vec<_>>(),
                cdf.iter().map(|&c| c / total).collect::<Vec<_>>(),
            )
        } else {
            // A genuinely empty row -- the reaction threshold, where every
            // spectrum is zero. Keep the grid, leave the density flat-zero, and
            // let the sampler's own bracketing handle it; do NOT invent a shape.
            (pdf_ev, cdf)
        };

        incident.push(inc.e_in_mev * EMEV);
        tables.push(ChiEout {
            e_out,
            pdf: pdf_ev,
            cdf,
            linlin: true,
        });
        ang_tables.push(ContinuumAngularTable { rows });
    }

    let branch = ContinuumBranch {
        spectrum: ChiTabular {
            incident,
            tables,
            incident_interp: collapse_law7_incident_interp(&law7.e_in_interp),
        },
        yield_pairs: law7.yield_pairs,
        angular: ContinuumAngular::LabTabulated(ang_tables),
    };
    Ok(Some(ContinuumEmission {
        branches: vec![branch],
        // LAW=7 is laboratory-frame by definition -- see the frame note above.
        cm_frame: false,
    }))
}

/// `y(x)` from an ascending lin-lin table, **zero outside** its own range.
///
/// Zero rather than clamped-to-endpoint on purpose: outside its own `E'` range a
/// LAW=7 cosine's spectrum contributes nothing to the joint density, and
/// clamping would invent probability where the evaluation put none.
fn lin_interp_zero_outside(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len().min(ys.len());
    if n == 0 || x < xs[0] || x > xs[n - 1] {
        return 0.0;
    }
    let mut hi = 1usize;
    while hi < n && xs[hi] < x {
        hi += 1;
    }
    if hi >= n {
        return ys[n - 1];
    }
    let (x0, x1) = (xs[hi - 1], xs[hi]);
    if x1 <= x0 {
        return ys[hi];
    }
    let f = (x - x0) / (x1 - x0);
    ys[hi - 1] + f * (ys[hi] - ys[hi - 1])
}

/// Build one [`ContinuumAngularRow`] from an unnormalised `f(mu)` slice on a
/// cosine grid: trapezoid CDF, normalised, plus the exact `<mu>` of the same
/// piecewise-linear density.
///
/// `mubar` is computed from the density rather than estimated from samples, so
/// it is an oracle for [`ContinuumAngularRow::sample_mu`] and not a second
/// estimate of it — the same standard the Legendre and Kalbach-Mann rows are
/// held to.
fn cosine_row_from_slice(mu: &[f64], f: &[f64]) -> ContinuumAngularRow {
    let n = mu.len().min(f.len());
    if n < 2 {
        return ContinuumAngularRow::isotropic();
    }
    let mut cdf = vec![0.0f64; n];
    let mut num = 0.0f64; // integral of mu * f dmu
    for i in 1..n {
        let (m0, m1) = (mu[i - 1], mu[i]);
        let (f0, f1) = (f[i - 1].max(0.0), f[i].max(0.0));
        let h = m1 - m0;
        cdf[i] = cdf[i - 1] + 0.5 * (f0 + f1) * h;
        // Exact for a linear f on [m0, m1]: integral of mu*f = h*(m0*(2f0+f1) +
        // m1*(f0+2f1))/6. The trapezoid rule is exact for f but NOT for mu*f,
        // which is quadratic -- the same trap that produced a false +0.60 %
        // "bias" earlier in this port's history, so it is done in closed form.
        num += h * (m0 * (2.0 * f0 + f1) + m1 * (f0 + 2.0 * f1)) / 6.0;
    }
    let total = cdf[n - 1];
    if !(total > 0.0) {
        return ContinuumAngularRow::isotropic();
    }
    for c in &mut cdf {
        *c /= total;
    }
    ContinuumAngularRow {
        cosines: mu.to_vec(),
        cdf,
        mubar: (num / total).clamp(-1.0, 1.0),
    }
}

/// LAW=7's incident-energy interpolation ranges, in the same `(NBT, INT)` form
/// [`ChiTabular::incident_interp`] carries for LAW=1.
///
/// Passed through unchanged — it is recorded so a consumer can see what the
/// evaluation asked for, exactly as the LAW=1 path records it, and with the same
/// caveat that the samplers apply unit-base regardless.
fn collapse_law7_incident_interp(interp: &[(u32, u32)]) -> Vec<(u32, u32)> {
    interp.to_vec()
}

/// Convert an ENDF **MF=6 LAW=6** (`n`-body phase space) emission into the
/// tabulated [`ChiTabular`] form the transport samplers already consume.
///
/// # Why convert rather than add a law
///
/// LAW=6's distribution is a *shape in the scaled variable* `x = E'/E'_max(E)`
/// that does not depend on the incident energy — only the upper limit moves.
/// `crate::acer::energy::parse_mf6_law6_phase_space` already builds that
/// universal `(x, pdf, cdf)` table. Evaluating `E'_max(E)` on an incident grid
/// therefore turns the law into exactly the incident-energy-indexed tabulated
/// form every sampler in this workspace already handles, with **no new sampling
/// path, no kernel change and no second implementation to drift**.
///
/// Phase-space emission is isotropic in the centre of mass by construction, so
/// the angular half is [`ContinuumAngular::EvaluatedIsotropic`] — genuinely
/// isotropic per the evaluation, which is a different fact from "unported", and
/// the enum keeps them apart.
///
/// # `E'_max`
///
/// The formula and its upstream citation live in [`phase_space_chi`], which does
/// the scaling for both this route and the ACE one. Upstream supports **3, 4 or
/// 5 particles only** (`f6psp` errors otherwise); this returns `Ok(None)` for any
/// other `NPSX` rather than inventing a shape, so the caller keeps its
/// documented fallback.
///
/// `Q` is read from MF=3's `QM` for the same MT — the reaction Q-value the
/// formula wants, and the same one upstream's `q` argument carries.
fn phase_space_emission(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    mt: i32,
) -> Result<Option<ContinuumEmission>, crate::NjoyError> {
    use crate::endf::records::SectionCursor;

    let Some(sec) = tape.section(mat, 6, mt) else {
        return Ok(None);
    };
    let ps = match crate::acer::energy::parse_mf6_law6_phase_space(sec) {
        Ok(p) => p,
        // LAW=7 and anything else stay unported -- fall back, do not fail.
        Err(crate::NjoyError::NotPorted(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    if !(3..=5).contains(&ps.npsx) || ps.x_frac.len() < 2 {
        return Ok(None);
    }

    // Q and the threshold from MF=3.
    let Some(mf3) = tape.section(mat, 3, mt) else {
        return Ok(None);
    };
    let mut cur = SectionCursor::new(&mf3.rows);
    let head = cur.read_cont()?;
    let awr = head.c2;
    let qm_row = cur.read_tab1()?;
    let q = qm_row.head.c1; // QM
    let (e_first, e_last) = match (qm_row.pairs.first(), qm_row.pairs.last()) {
        (Some(&(a, _)), Some(&(b, _))) if b > a => (a, b),
        _ => return Ok(None),
    };

    let Some(spectrum) = phase_space_chi(
        &ps.x_frac,
        &ps.pdf,
        &ps.cdf,
        ps.apsx,
        awr,
        q,
        e_first.max(1.0e-5),
        e_last,
    ) else {
        return Ok(None);
    };

    Ok(Some(ContinuumEmission {
        branches: vec![ContinuumBranch {
            spectrum,
            yield_pairs: ps.yield_pairs.clone(),
            angular: ContinuumAngular::EvaluatedIsotropic,
        }],
        cm_frame: ps.lct >= 2,
    }))
}

/// Scale an `n`-body phase-space **shape** in `x = E'/E'_max` onto an incident
/// grid, giving the tabulated [`ChiTabular`] every sampler here consumes.
///
/// Shared by the two routes that meet this law: ENDF MF=6 LAW=6
/// ([`phase_space_emission`], which reads the shape out of the section) and ACE
/// LAW=66 ([`crate::acer::ce_laws::ace_phase_space_chi`], which rebuilds the
/// shape from `NPSX` because the file stores only the two parameters). One
/// implementation, so the routes cannot disagree about `E'_max(E)` -- the part
/// that carries the physics and the part neither file stores.
///
/// # `E'_max`, ported from upstream
///
/// `groupr.f90:12658-12666` (`f6psp`), with `AWP = 1` for an emitted neutron:
///
/// ```text
/// f1     = (APSX - AWP) / APSX
/// f2     = AWR / (AWR + 1)
/// E'_max = f1 * (f2 * E + Q)
/// ```
///
/// `q` is the reaction Q-value \[eV\] and `awr` the target-to-neutron mass
/// ratio. The incident grid is 80 log-spaced points over `[e_lo, e_hi]`: the
/// shape does not vary with incident energy, so the grid only has to resolve
/// `E'_max(E)`, which is linear in `E`. Points below threshold
/// (`E'_max <= 0`) are dropped, and `None` comes back if fewer than two survive
/// -- a one-row table is not a distribution.
pub fn phase_space_chi(
    x_frac: &[f64],
    pdf_in: &[f64],
    cdf_in: &[f64],
    apsx: f64,
    awr: f64,
    q: f64,
    e_lo: f64,
    e_hi: f64,
) -> Option<ChiTabular> {
    if x_frac.len() < 2 || !(apsx > 0.0) || !(e_hi > e_lo) || !(e_lo > 0.0) {
        return None;
    }
    // AWP: the emitted particle is a neutron, so 1 neutron mass.
    const AWP: f64 = 1.0;
    let f1 = (apsx - AWP) / apsx;
    let f2 = awr / (awr + 1.0);
    let e_max_at = |e: f64| f1 * (f2 * e + q);

    const N_IN: usize = 80;
    let mut incident = Vec::with_capacity(N_IN);
    let mut tables = Vec::with_capacity(N_IN);
    for i in 0..N_IN {
        let e = e_lo * (e_hi / e_lo).powf(i as f64 / (N_IN - 1) as f64);
        let emax = e_max_at(e);
        if !(emax > 0.0) {
            continue; // below threshold: no phase space to share
        }
        let e_out: Vec<f64> = x_frac.iter().map(|&x| x * emax).collect();
        // `pdf` is a density in x; rescaling the variable by `emax` divides it.
        let pdf: Vec<f64> = pdf_in.iter().map(|&p| p / emax).collect();
        incident.push(e);
        tables.push(ChiEout {
            e_out,
            pdf,
            cdf: cdf_in.to_vec(),
            linlin: true,
        });
    }
    if incident.len() < 2 {
        return None;
    }
    Some(ChiTabular {
        incident,
        tables,
        // Synthesised grid: the law carries no TAB2 of its own.
        incident_interp: Vec::new(),
    })
}

impl ContinuumEmission {
    /// Read the MF=6 LAW=1 neutron emission of reaction `mt` for material `mat`.
    ///
    /// Returns `Ok(None)` when the material has no MF=6/`mt` section, or when the
    /// section uses a law this port does not read (LAW≠1, or a subsection layout
    /// [`crate::acer::energy::parse_mf6_law1_neutrons`] rejects) — the caller is
    /// expected to keep its previous fallback in that case rather than fail.
    ///
    /// Units are converted here: [`crate::acer::energy::Law4`] is an ACE-side
    /// structure and carries MeV, while every `nuclear_data` consumer works in eV.
    pub fn from_endf_mf6(
        tape: &crate::endf::tape::Tape,
        mat: i32,
        mt: i32,
    ) -> Result<Option<ContinuumEmission>, crate::NjoyError> {
        let Some(sec) = tape.section(mat, 6, mt) else {
            return Ok(None);
        };
        let neutrons = match crate::acer::energy::parse_mf6_law1_neutrons(sec) {
            Ok(n) => n,
            // LAW=6 (phase space) is a different representation, not an
            // unreadable one -- convert it rather than falling back.
            Err(crate::NjoyError::NotPorted(_)) => {
                // LAW=6 (phase space) and LAW=7 (lab angle-energy) are different
                // representations, not unreadable ones -- convert whichever is
                // present. Both return Ok(None) if they are not, which keeps the
                // documented fallback rather than inventing a law.
                if let Some(em) = phase_space_emission(tape, mat, mt)? {
                    return Ok(Some(em));
                }
                return lab_angle_energy_emission(tape, mat, mt);
            }
            Err(e) => return Err(e),
        };

        const EMEV: f64 = 1.0e6;
        // `>= 2`, not `== 2`: ENDF-6 also defines **LCT = 3** (the first two
        // particles in the centre of mass, the rest in the laboratory), and the
        // emitted neutron is one of the first two. NJOY does exactly this
        // collapse — `acefc.f90:7187`, `if (lct.gt.2) lct=2`. ENDF/B-VIII.0's
        // C-12 MF=6/MT=5 carries LCT = 3, so treating it as laboratory would put
        // a CM spectrum through no frame transform at all.
        let cm_frame = neutrons.first().map(|n| n.lct >= 2).unwrap_or(false);
        let mut branches = Vec::with_capacity(neutrons.len());
        for neutron in neutrons {
            let mut incident = Vec::with_capacity(neutron.law4.incident.len());
            let mut tables = Vec::with_capacity(neutron.law4.incident.len());
            for t in &neutron.law4.incident {
                // `ND > 0` means the table's leading entries are **discrete
                // lines**, not a continuum. [`ChiTabular`] is a pure continuum
                // pdf/cdf and cannot represent them: sampled as continuum, a
                // zero-width discrete line is either lost or smeared. Refuse the
                // whole emission rather than return a law that samples wrongly —
                // the caller's documented behaviour on `None` is to keep its own
                // fallback, which is a known approximation rather than a silent
                // one. No evaluation in `reference-data/endf/` currently has
                // `ND > 0` on a neutron subsection, so this is a guard, not a
                // live path.
                if t.nd() != 0 {
                    return Ok(None);
                }
                incident.push(t.e_in_mev * EMEV);
                tables.push(ChiEout {
                    e_out: t.e_out_mev.iter().map(|&x| x * EMEV).collect(),
                    // pdf is a density in the energy variable, so it scales inversely.
                    pdf: t.pdf.iter().map(|&y| y / EMEV).collect(),
                    cdf: t.cdf.clone(),
                    // `lep()`, not the raw `intt`: ACE packs `INTT = LEP + 10·ND`,
                    // so a histogram table carrying discrete lines reads `intt =
                    // 11` and a bare `intt != 1` would call it lin-lin.
                    linlin: t.lep() != 1,
                });
            }
            if incident.is_empty() {
                continue;
            }
            let angular = build_continuum_angular(&neutron);
            branches.push(ContinuumBranch {
                spectrum: ChiTabular {
                    incident,
                    tables,
                    // The evaluation's own incident-energy law, carried rather
                    // than dropped -- see `ChiTabular::incident_interp`.
                    incident_interp: neutron.law4.e_in_interp.clone(),
                },
                yield_pairs: neutron.yield_pairs,
                angular,
            });
        }
        if branches.is_empty() {
            return Ok(None);
        }

        Ok(Some(ContinuumEmission { branches, cm_frame }))
    }

    /// Total neutron multiplicity at incident energy `e_in` \[eV\] — the sum
    /// over branches.
    ///
    /// This is the number the transport layer must emit: 1 for MT=91, 2 for
    /// MT=16 **however the evaluation writes it** (one branch of yield 2, or two
    /// branches of yield 1).
    pub fn total_yield_at(&self, e_in: f64) -> f64 {
        self.branches.iter().map(|b| b.yield_at(e_in)).sum()
    }

    /// This emission with its angular law **switched off** — every branch's
    /// [`ContinuumAngular`] replaced by [`ContinuumAngular::Ablated`], so
    /// emission is isotropic in the frame the law names.
    ///
    /// The ablation arm of a paired worth measurement, and the counterpart of
    /// `outram_mc_libs::material::nuclide::Nuclide::with_isotropic_inelastic_scattering`
    /// one channel over. **The energy law is untouched** — only the angle
    /// changes, which is what makes a measured `Δk` attributable to the angular
    /// physics rather than to a second thing moving at the same time.
    ///
    /// Returning a new value rather than mutating in place is deliberate: both
    /// arms of an ablation must be able to exist at once, in one process, over
    /// the same seeds. An ablation driven by a global flag cannot do that, and
    /// an ablation run in two processes re-randomises everything it did not mean
    /// to change.
    pub fn with_isotropic_angle(mut self) -> Self {
        for b in &mut self.branches {
            b.angular = ContinuumAngular::Ablated;
        }
        self
    }

    /// Whether any branch of this emission carries a samplable angular law.
    ///
    /// The assertion an ablation control needs on the *unablated* arm: switching
    /// off a law that was already flat produces no difference and reads as "this
    /// physics does not matter".
    pub fn is_anisotropic(&self) -> bool {
        self.branches.iter().any(|b| b.angular.is_anisotropic())
    }

    /// The branch an emitted neutron is drawn from, chosen in proportion to the
    /// branches' yields at `e_in`, given a uniform variate `xi` in `[0, 1)`.
    ///
    /// With a single branch this always returns it. With F-19's two equal-yield
    /// (n,2n) branches it picks each half the time, which reproduces the
    /// evaluation's *average* emission spectrum over the two neutrons — it does
    /// not correlate the pair, so a code emitting both neutrons of one event
    /// should take one draw per neutron.
    pub fn branch_for(&self, e_in: f64, xi: f64) -> &ContinuumBranch {
        let total = self.total_yield_at(e_in);
        if self.branches.len() == 1 || !(total > 0.0) {
            return &self.branches[0];
        }
        let mut acc = 0.0;
        let target = xi.clamp(0.0, 1.0) * total;
        for b in &self.branches {
            acc += b.yield_at(e_in);
            if target < acc {
                return b;
            }
        }
        &self.branches[self.branches.len() - 1]
    }
}

/// Turn one MF=6 LAW=1 subsection's retained angular coefficients into the
/// samplable [`ContinuumAngular`] form.
///
/// Legendre (`LANG = 1`) rows are linearised once, at load time, through
/// [`crate::acer::angular::legendre_cosine_law`] — the same routine that
/// converts MF=4, so the continuum and the discrete levels cannot drift apart
/// in how a harmonic law becomes a samplable one. Rows the evaluation declares
/// isotropic (`NA = 0`), and rows whose coefficients all vanish, are stored as
/// [`ContinuumAngularRow::isotropic`] rather than as a degenerate table.
///
/// Anything that is not `LANG = 1` becomes [`ContinuumAngular::Unported`],
/// carrying the law it actually was. That is deliberate: falling back to
/// isotropic silently is what this whole change exists to stop.
///
/// # Cost
///
/// The linearisation is adaptive and runs once per anisotropic row. Measured on
/// ENDF/B-VIII.0 (2026-09-16), U-238 carries 8652 anisotropic MT=91 rows and
/// 2066 MT=16 rows, U-235 5294 and 2459 — so a Godiva-style three-actinide load
/// linearises of order 20 000 rows. This is load-time work, not per-collision
/// work; the sampling path is a CDF inversion.
fn build_continuum_angular(neutron: &crate::acer::energy::Mf6Neutron) -> ContinuumAngular {
    use crate::acer::energy::Mf6AngularLaw;

    if neutron.is_angular_isotropic() {
        return ContinuumAngular::EvaluatedIsotropic;
    }
    if neutron.lang == Mf6AngularLaw::KalbachMann {
        return build_kalbach_angular(neutron);
    }
    if neutron.lang != Mf6AngularLaw::Legendre {
        return ContinuumAngular::Unported(neutron.lang);
    }

    let mut tables = Vec::with_capacity(neutron.angular.len());
    let mut any = false;
    for t in &neutron.angular {
        let mut rows = Vec::with_capacity(t.len());
        for r in 0..t.len() {
            let coeffs = t.legendre_coefficients(r);
            match crate::acer::angular::legendre_cosine_law(&coeffs) {
                Some((cosines, _pdf, cdf)) => {
                    any = true;
                    rows.push(ContinuumAngularRow {
                        cosines,
                        cdf,
                        // `a₁` straight from the tape, not re-integrated off the
                        // linearised grid: it is exact, and comparing the two is
                        // how a linearisation defect would show up.
                        mubar: t.legendre_mubar(r),
                    });
                }
                None => rows.push(ContinuumAngularRow::isotropic()),
            }
        }
        tables.push(ContinuumAngularTable { rows });
    }

    if any {
        ContinuumAngular::Legendre(tables)
    } else {
        // Every row's coefficients vanished, so the law is flat in fact even
        // though `NA > 0` was declared. Reporting it as evaluated-isotropic is
        // the honest reading and costs the sampler nothing.
        ContinuumAngular::EvaluatedIsotropic
    }
}

/// Build the `LANG = 2` (Kalbach-Mann) angular law of one MF=6 LAW=1
/// subsection.
///
/// Each row carries `r` directly. The slope `a` is the row's third number when
/// the evaluation tabulates it (`NA = 2`), and otherwise comes from the
/// **Kalbach-86 systematics**, [`crate::groupr::kinematics::bach`] — a faithful
/// port of NJOY2016's `groupr.f90:8812-8932` that already existed in this crate
/// for the GROUPR path. Reusing it rather than writing a second copy is the
/// point: two implementations of the same systematics would drift, and this one
/// is already tested.
///
/// `bach` is called with `za_projectile = 1` (incident neutron),
/// `za_emitted = 1` (the ZAP=1 subsection is a neutron by construction) and the
/// section's own target `ZA`, at the incident energy of the table and the
/// outgoing energy of the row — both in eV, which is what `bach` takes.
///
/// A row whose `bach` call fails (an unknown dominant isotope for a natural
/// element) falls back to isotropic for that row rather than failing the whole
/// emission: the caller's documented behaviour on a missing law is to keep its
/// own fallback, and losing one row's anisotropy is a smaller error than losing
/// the entire energy law with it.
fn build_kalbach_angular(neutron: &crate::acer::energy::Mf6Neutron) -> ContinuumAngular {
    const EMEV: f64 = 1.0e6;
    /// ENDF `ZA` of a neutron — both the projectile and the emitted particle
    /// here, since this is the `ZAP = 1` subsection.
    const ZA_NEUTRON: i32 = 1;

    let mut tables = Vec::with_capacity(neutron.angular.len());
    let mut any = false;
    for t in &neutron.angular {
        let e_in_ev = t.e_in_mev * EMEV;
        let mut rows = Vec::with_capacity(t.len());
        for k in 0..t.len() {
            let raw = t.row(k);
            if raw.is_empty() {
                rows.push(ContinuumKalbachRow::isotropic());
                continue;
            }
            let r = raw[0];
            // `NA = 2` tabulates the slope; otherwise the systematics supply it.
            let a = if raw.len() >= 2 {
                raw[1]
            } else {
                // The outgoing energy this row belongs to, in eV.
                let e_out_ev = neutron
                    .law4
                    .incident
                    .iter()
                    .find(|d| (d.e_in_mev - t.e_in_mev).abs() <= f64::EPSILON * t.e_in_mev.max(1.0))
                    .and_then(|d| d.e_out_mev.get(k).copied())
                    .unwrap_or(0.0)
                    * EMEV;
                if e_out_ev <= 0.0 || e_in_ev <= 0.0 {
                    rows.push(ContinuumKalbachRow::isotropic());
                    continue;
                }
                match crate::groupr::kinematics::bach(
                    ZA_NEUTRON,
                    ZA_NEUTRON,
                    neutron.za_target,
                    e_in_ev,
                    e_out_ev,
                ) {
                    Ok(a) => a,
                    Err(_) => {
                        rows.push(ContinuumKalbachRow::isotropic());
                        continue;
                    }
                }
            };
            let row = ContinuumKalbachRow { r, a };
            if !row.is_isotropic() {
                any = true;
            }
            rows.push(row);
        }
        tables.push(ContinuumKalbachTable { rows });
    }

    if any {
        ContinuumAngular::KalbachMann(tables)
    } else {
        // Every slope collapsed, so the law is flat in fact. Same reasoning as
        // the Legendre arm: report what it is, not how it was declared.
        ContinuumAngular::EvaluatedIsotropic
    }
}

/// Parse the **LF=1** body (arbitrary tabulated secondary energy distribution):
/// a TAB2 over NE incident energies, each an inner TAB1 g(E→E'). Shared by every
/// partition of [`FissionSpectrum::from_endf_mf5`] that uses LF=1.
fn parse_lf1_tabular(
    cur: &mut crate::endf::records::SectionCursor<'_>,
) -> Result<ChiTabular, crate::NjoyError> {
    let tab2 = cur.read_tab2()?;
    // Carried, not dropped -- see `ChiTabular::incident_interp`.
    let tab2_interp: Vec<(u32, u32)> = tab2.interp.clone();
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
        tables.push(ChiEout {
            e_out,
            pdf,
            cdf,
            linlin,
        });
    }
    Ok(ChiTabular {
        incident,
        tables,
        // MF=5 LF=1's TAB2 interpolation, carried for the same reason as MF=6's.
        incident_interp: tab2_interp,
    })
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
        let chi = FissionSpectrum::Watt {
            a: 0.988e6,
            b: 2.249e-6,
        };
        let expected = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
        assert!(
            (chi.mean_energy(1.0e6) - expected).abs() < 1.0,
            "got {}",
            chi.mean_energy(1.0e6)
        );
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
        assert!(
            (chi.mean_energy(0.0) - 1.0e6).abs() < 1.0,
            "got {}",
            chi.mean_energy(0.0)
        );
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

#[cfg(test)]
mod kalbach_tests {
    use super::ContinuumKalbachRow;

    /// Sample mean of `n` stratified draws through the row's CDF inverse.
    ///
    /// Stratified rather than pseudo-random so the comparison against the
    /// closed form is limited by the quadrature and not by Monte Carlo noise —
    /// this is testing an inverse function, not a random number generator.
    fn sampled_mean(row: &ContinuumKalbachRow, n: usize) -> f64 {
        (0..n)
            .map(|i| row.sample_mu((i as f64 + 0.5) / n as f64))
            .sum::<f64>()
            / n as f64
    }

    /// The single-variate CDF inverse reproduces the density's **closed-form**
    /// mean `⟨μ⟩ = r·(coth a − 1/a)` across the parameter range Kalbach-Mann
    /// actually spans.
    ///
    /// This is a real oracle, not a self-consistency check: `mubar()` integrates
    /// the density analytically while `sample_mu()` inverts its cumulative, so
    /// they share no code. An algebra slip in either shows up here.
    #[test]
    fn the_inverse_reproduces_the_closed_form_mean() {
        const N: usize = 200_000;
        // r spans the pre-compound fraction's full range; a spans from nearly
        // isotropic to strongly forward-peaked, which is what the systematics
        // produce between threshold and 20 MeV.
        for &r in &[0.0, 0.1, 0.35, 0.7, 0.95] {
            for &a in &[0.05, 0.3, 1.0, 3.0, 8.0] {
                let row = ContinuumKalbachRow { r, a };
                let exact = row.mubar();
                let got = sampled_mean(&row, N);
                assert!(
                    (got - exact).abs() < 2.0e-3,
                    "r = {r}, a = {a}: sampled <mu> = {got:+.6} against the closed form \
                     r*(coth a - 1/a) = {exact:+.6}. These share no code -- one integrates the \
                     density, the other inverts its cumulative -- so a disagreement is an \
                     algebra error in one of them."
                );
            }
        }
    }

    /// Every sampled cosine is physical, at the extremes of the parameter range
    /// where the hyperbolic functions overflow most readily.
    #[test]
    fn sampled_cosines_stay_in_range() {
        for &r in &[0.0, 0.5, 0.999] {
            for &a in &[1.0e-9, 1.0e-3, 1.0, 20.0, 200.0] {
                let row = ContinuumKalbachRow { r, a };
                for i in 0..=1000 {
                    let mu = row.sample_mu(i as f64 / 1000.0);
                    assert!(
                        (-1.0..=1.0).contains(&mu) && mu.is_finite(),
                        "r = {r}, a = {a}, xi = {}: mu = {mu} is not a physical cosine",
                        i as f64 / 1000.0
                    );
                }
            }
        }
    }

    /// The endpoints of the inverse land exactly on the support, which is what
    /// says the CDF was inverted rather than approximated.
    #[test]
    fn the_inverse_maps_the_unit_interval_onto_the_support() {
        for &(r, a) in &[(0.0, 1.0), (0.3, 2.0), (0.8, 5.0)] {
            let row = ContinuumKalbachRow { r, a };
            assert!(
                (row.sample_mu(0.0) + 1.0).abs() < 1.0e-9,
                "r = {r}, a = {a}: F^-1(0) = {} , expected -1",
                row.sample_mu(0.0)
            );
            assert!(
                (row.sample_mu(1.0) - 1.0).abs() < 1.0e-9,
                "r = {r}, a = {a}: F^-1(1) = {}, expected +1",
                row.sample_mu(1.0)
            );
        }
    }

    /// `a → 0` is the isotropic limit of the Kalbach form, and `r = 0` is NOT.
    ///
    /// Worth pinning because the two look symmetric in the formula and are not:
    /// with `r = 0` the density is `a cosh(a mu) / (2 sinh a)`, which is peaked
    /// at *both* ends and has zero mean but is not flat. A predicate that
    /// treated `r = 0` as isotropic would silently drop that structure.
    #[test]
    fn only_the_slope_makes_the_law_isotropic() {
        assert!(ContinuumKalbachRow { r: 0.5, a: 0.0 }.is_isotropic());
        assert!(!ContinuumKalbachRow { r: 0.0, a: 3.0 }.is_isotropic());

        // r = 0, a = 3: mean zero, but the distribution is not uniform.
        let peaked = ContinuumKalbachRow { r: 0.0, a: 3.0 };
        assert!(peaked.mubar().abs() < 1.0e-12, "r = 0 must give zero mean");
        let mu_lo = peaked.sample_mu(0.25);
        let mu_hi = peaked.sample_mu(0.75);
        // A uniform law would give -0.5 and +0.5; a cosh-peaked one pushes both
        // quartiles outward.
        assert!(
            mu_lo < -0.5 && mu_hi > 0.5,
            "r = 0, a = 3 quartiles came back at {mu_lo:+.4} / {mu_hi:+.4}; a cosh-peaked \
             density must push them outside the uniform law's -0.5 / +0.5"
        );
    }
}

#[cfg(test)]
mod mf5_lf_survey {
    use crate::endf::records::SectionCursor;
    use crate::endf::tape::Tape;
    use crate::reference_data::reference_data_dir;

    /// **Which MF=5 secondary-energy laws (`LF`) do the held evaluations use?**
    ///
    /// [`super::FissionSpectrum::from_endf_mf5_mt`] ports LF=1/7/9/11 and
    /// returns `None` for anything else, which makes the *whole* MF=5 fall back
    /// to the thermal-Watt stand-in. That is a silent degradation, so this
    /// measures which codes actually appear rather than leaving it to a comment.
    ///
    /// It also **asserts that no held evaluation uses an unported LF**, so a
    /// tape that does fails loudly instead of quietly losing its fission
    /// spectrum.
    ///
    /// Counts its own skips, for the same reason the MF=6 survey does.
    #[test]
    fn survey_mf5_lf_codes() {
        let dir = reference_data_dir("endf");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            println!("no reference-data/endf; skipping");
            return;
        };
        let mut files: Vec<_> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|x| x == "endf")
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("n-"))
            })
            .collect();
        files.sort();

        let mut seen: std::collections::BTreeMap<i32, usize> = Default::default();
        let mut unported: Vec<String> = Vec::new();
        let (mut n_sec, mut n_err) = (0usize, 0usize);
        for f in &files {
            let Ok(tape) = Tape::read_file(f) else {
                continue;
            };
            let Some(&mat) = tape.materials().first() else {
                continue;
            };
            for mt in [18i32, 16, 91, 5] {
                let Some(sec) = tape.section(mat, 5, mt) else {
                    continue;
                };
                n_sec += 1;
                let mut cur = SectionCursor::new(&sec.rows);
                let Ok(head) = cur.read_cont() else {
                    n_err += 1;
                    continue;
                };
                let nk = head.n1.max(0);
                for _ in 0..nk {
                    let Ok(p_tab) = cur.read_tab1() else {
                        n_err += 1;
                        break;
                    };
                    let lf = p_tab.head.l2;
                    *seen.entry(lf).or_insert(0) += 1;
                    if !matches!(lf, 1 | 7 | 9 | 11) {
                        unported.push(format!(
                            "{} MF=5 MT={mt} LF={lf}",
                            f.file_name().unwrap().to_string_lossy()
                        ));
                    }
                    // Only the first subsection's records are walked reliably
                    // without dispatching on LF; stop after one per section.
                    break;
                }
            }
        }

        let name = |lf: i32| match lf {
            1 => "arbitrary tabulated",
            5 => "GENERAL EVAPORATION (unported)",
            7 => "simple Maxwellian fission",
            9 => "evaporation",
            11 => "energy-dependent Watt",
            12 => "Madland-Nix (unported)",
            _ => "?",
        };
        println!(
            "MF=5 LF codes across {} neutron tapes ({n_sec} sections, {n_err} read errors):",
            files.len()
        );
        for (lf, n) in &seen {
            println!("   LF={lf} ({}) -> {n} subsection(s)", name(*lf));
        }
        assert!(
            unported.is_empty(),
            "held evaluations use an MF=5 law this port does not read, so their whole fission \
             spectrum silently falls back to the thermal-Watt stand-in: {unported:?}. NOTE: \
             NJOY DOES support LF=5 (groupr.f90:12355, 'law 5. general evaporation spectrum') \
             -- a comment in this file claiming otherwise was wrong and has been corrected."
        );
    }
}
