//! Output data structures for `calcem`'s two secondary-distribution formats.
//!
//! Upstream NJOY writes these directly to a scratch ENDF tape (`nscr`) as
//! MF=6 records — a special `LANG=3` equal-probable-cosine encoding for
//! `iform=0` (`thermr.f90:1927-2266`) and a `LAW=7` continuous angle-energy
//! encoding for `iform=1` (`thermr.f90:2306-2404`). This crate has no MF=6
//! tape writer for either format yet, so these types carry the same *physical
//! content* the Fortran would write — the numbers, already `sigfig`-rounded
//! and clamped exactly as NJOY leaves them — as plain Rust data, for a future
//! MF=6/ACE writer to consume. See the `calcem` module docs for what is and
//! is not covered by "the physics is ported".

/// One outgoing-energy row of the `iform=0` (equally-probable-cosine,
/// `LANG=3`) distribution at a single incident energy — `thermr.f90`
/// `calcem` labels 360-430 (lines 2098-2274).
#[derive(Debug, Clone)]
pub struct EqualProbableRow {
    /// Outgoing neutron energy `E'` \[eV\].
    pub ep_ev: f64,
    /// The (`sigfig`-rounded) cross-section-like ordinate `y(1,i)` \[barn\]
    /// at this `E'` — NOT yet normalized to a probability density; it is
    /// `sigl`'s own `σ(E→E')` output, tabulated as a function of `E'` the way
    /// the ENDF record stores it.
    pub pdf: f64,
    /// `nbin = nl - 1` equally-probable scattering cosines for this `E'`
    /// (ascending, each clamped to `[-1, 1]` — NJOY's overflow/underflow
    /// guard at `thermr.f90:2179-2199`, applied here without the
    /// accompanying warning message).
    pub cosines: Vec<f64>,
}

/// The full `iform=0` secondary distribution at one incident energy —
/// `thermr.f90` `calcem` lines 1955-2225.
#[derive(Debug, Clone)]
pub struct IncidentEnergyRecord {
    /// Incident neutron energy `E` \[eV\] (`esi(ie)`, already `sigfig`-rounded
    /// and rescaled for `temp > break` per [`super::egrid::rescale_iform0`]).
    pub e_in_ev: f64,
    /// `σ_inel(E)` \[barn\] — `xsi(ie)`, the trapezoidal `E'`-integral of the
    /// rows' `pdf` column.
    pub cross_section_b: f64,
    /// Mean scattering cosine `⟨μ⟩(E)`, normalized by `cross_section_b`.
    /// `0.0` when `cross_section_b == 0`. NJOY only performs this
    /// normalization inside its `iprint == 2` print branch
    /// (`thermr.f90:2207-2209`) — elsewhere `ubar(ie)` is left as the raw,
    /// unnormalized integral. This port always normalizes (a packaging
    /// choice, not an algorithm change: nothing downstream in `calcem`
    /// re-reads `ubar`/`p2`/`p3`, so there is no "improved" numeric result to
    /// worry about diverging from — see the module docs).
    pub mubar: f64,
    /// Second Legendre moment `⟨P₂(μ)⟩(E)`, normalized the same way.
    pub p2: f64,
    /// Third Legendre moment `⟨P₃(μ)⟩(E)`, normalized the same way.
    pub p3: f64,
    /// The `j` outgoing-energy rows (already trimmed to the first all-zero
    /// terminator, `thermr.f90:2225-2226`'s `jnz`/`j=jnz+1` logic).
    pub rows: Vec<EqualProbableRow>,
}

/// The complete `iform=0` table over the incident-energy grid actually used
/// (`nne` points of [`super::egrid::EGRID`]).
#[derive(Debug, Clone)]
pub struct Iform0Table {
    pub records: Vec<IncidentEnergyRecord>,
}

/// The continuous outgoing-energy distribution at one scattering cosine `μ`
/// — `iform=1` (`LAW=7`) format, `thermr.f90` `calcem` labels 590-608.
#[derive(Debug, Clone)]
pub struct MuDistribution {
    /// Scattering cosine (dimensionless, `[-1, 1]`).
    pub mu: f64,
    /// `(E' \[eV\], pdf)` pairs from [`super::sigu::sigu`], already
    /// normalized so `∫ pdf dE' = 1` (`thermr.f90:2404`:
    /// `yu(2+2*ib)*2/sum`) and trimmed of the low-probability tail below
    /// `yumin` (`thermr.f90:2379-2384`).
    pub points: Vec<(f64, f64)>,
}

/// The full `iform=1` secondary distribution at one incident energy —
/// `thermr.f90` `calcem` lines 2285-2404.
#[derive(Debug, Clone)]
pub struct IncidentEnergyAngleRecord {
    /// Incident neutron energy `E` \[eV\] (`esi(ie)`, rescaled per
    /// [`super::egrid::rescale_iform1`]).
    pub e_in_ev: f64,
    /// `σ_inel(E)` \[barn\] — `xsi(ie) = sum/2` (`thermr.f90:2367`).
    pub cross_section_b: f64,
    /// Mean scattering cosine `⟨μ⟩(E)` (`thermr.f90:2368-2370`; NJOY does not
    /// gate this normalization on `iprint` in the `iform=1` path, unlike
    /// `iform=0`'s `ubar`/`p2`/`p3`).
    pub mubar: f64,
    /// The adaptively-determined `nmu` scattering cosines and each one's
    /// continuous secondary-energy distribution.
    pub mu_distributions: Vec<MuDistribution>,
}

/// The complete `iform=1` table over the incident-energy grid actually used.
#[derive(Debug, Clone)]
pub struct Iform1Table {
    pub records: Vec<IncidentEnergyAngleRecord>,
}
