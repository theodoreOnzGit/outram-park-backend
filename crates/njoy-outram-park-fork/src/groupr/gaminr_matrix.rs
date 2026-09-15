// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GAMINR photon-production **matrices** — the `gpanel` matrix loop + the
//! `dspla` matrix branch, driven by a photon feed function.
//!
//! # What this module computes
//!
//! A neutron-to-photon **group-to-group production matrix**
//!
//! ```text
//! matrix[ig_neutron][ig_photon]
//!   = ( integral_{group ig_neutron} sigma(E) Y(E) g_{ig_photon}(E) phi(E) dE )
//!     / ( integral_{group ig_neutron} phi(E) dE )                       [barn]
//! ```
//!
//! where, over one incident-neutron group `[E_lo, E_hi)`:
//!
//! - `sigma(E)` is the base reaction cross section \[barn\] (e.g. radiative
//!   capture) that produces the photons,
//! - `Y(E)` is the photon multiplicity/yield \[photons per reaction\]
//!   (ENDF File 12 multiplicity or the File 13 production-cross-section shape),
//! - `g_{j}(E)` is the fraction of produced photons whose energy lands in photon
//!   sink group `j` (the tabulated ENDF File 15 continuous photon energy
//!   distribution, normalised to `sum_j g_j = 1` at each incident energy),
//! - `phi(E)` is the weighting flux \[shape-only\].
//!
//! The row sum `sum_j matrix[ig][j]` is the flux-weighted **total** photon
//! production cross section `<sigma * Y>_ig` \[barn\] — the yield-conservation
//! property the tests check.
//!
//! # Fortran routine -> Rust item map (`gaminr.f90`, commit ac5adf5)
//!
//! | Fortran (`gaminr.f90`) | Rust item | Notes |
//! |---|---|---|
//! | `gpanel` matrix loop, lines 874-1011 | [`photon_production_matrix`] / [`integrate_production_panel`] | panel march accumulating `ans(il,iz,igt) += rr*ff(il,ig)` (line 987) and the group flux `ans(il,iz,1)` (line 950) |
//! | `dspla` matrix branch, lines 1083-1128 (`mfd == 26`) | the `production[j] / group_flux` division in [`photon_production_matrix`] | `result(il) = ans(il,1,ig2)/ans(il,1,1)` (line 1098) |
//! | `gtsig`, lines 1133-1160 | reuse [`crate::groupr::panel::PointwiseXs`] | `sigma(E)` feeder |
//! | `gtflx`, lines 825-872 | reuse [`crate::groupr::panel::GroupFlux`] | `phi(E)` feeder |
//! | `gtff` mt=516 pair production, lines 1489-1506 | [`PhotonFeed::PairProduction`] | 2 annihilation photons into the group holding `epair` (`ig2pp`) |
//! | GENDF matrix record layout, `groupr.f90:882-946` | [`photon_matrix_gendf_section`] | `mfh = 26`, `ig2lo > 0`, `ng2 > 2` |
//!
//! # Faithfulness of the quadrature
//!
//! `gpanel` (`gaminr.f90:874-1011`) marches sub-panels bounded by the union of
//! the cross-section, flux, and feed-function break points, integrates the
//! reaction rate `rr = sigma*flux` with a Lobatto rule under a linear-`rr`
//! assumption (lines 966-993), and deposits `rr*ff(il,ig)` into each secondary
//! group. Because a Lobatto rule is exact for a linear integrand, each sub-panel
//! reduces to a trapezoid — [`integrate_production_panel`] marches the same union
//! grid and applies the same per-panel trapezoids, reusing the vector engine's
//! [`PointwiseXs`]/[`GroupFlux`] feeders. This mirrors the approach already taken
//! for the neutron vector path in [`crate::groupr::panel`].
//!
//! # Honest scope — what is and is not ported
//!
//! **Ported and tested here (the achievable, self-contained core):**
//!
//! - the [`PhotonFeed::Continuous`] File-15 continuous-spectrum + File-12/13
//!   yield feed (a normalised tabulated photon energy distribution scaled by a
//!   pointwise yield), and
//! - the [`PhotonFeed::PairProduction`] annihilation-photon feed
//!   (`gtff` mt=516, `gaminr.f90:1489-1506`; the residual-energy word
//!   `ff(1,2)=e-2*epair` at line 1504 is a heating quantity, **not** a photon
//!   group, and is deliberately *not* modelled here — see the variant docs),
//! - the `gpanel`/`dspla` matrix reduction ([`photon_production_matrix`]), and
//! - the GENDF `mfh=26` matrix record builder ([`photon_matrix_gendf_section`]),
//!   reusing the general [`GendfSection`]/[`GendfGroupRecord`] substrate.
//!
//! **Not ported (documented gaps, no fabricated values):**
//!
//! - **Discrete File-12 photon lines** (many discrete secondary energies): the
//!   continuous-distribution path is ported; a dense set of discrete lines would
//!   need the ENDF File-12 `LP`/`LF` control flow and is left as a follow-up.
//! - **The photon-*interaction* feed functions** in `gtff` (`gaminr.f90:1162-1514`)
//!   for coherent (mt=502, lines 1259-1334) and incoherent (mt=504, Klein-Nishina
//!   + form factor, lines 1338-1486) photon scattering: those build a
//!   photon->photon matrix from a form-factor tape and iterative Klein-Nishina
//!   kinematics. They are not required for the neutron->photon production matrix
//!   and are left unported.
//! - **The live ENDF/PENDF photon-tape control flow** (`gaminm` driver,
//!   `gaminr.f90:97-460`) that reads File 12/13/15 records off a tape: this module
//!   takes already-parsed feeders ([`PointwiseXs`], tabulated spectra) as input,
//!   the same design choice the neutron [`crate::groupr::panel`] port made.
//!
//! This is an **untrusted AI-drafted port** pending human review, per the crate
//! `CLAUDE.md`. The physics of the production feed (yield times normalised
//! spectrum, deposited into photon groups) is the standard ENDF File-12/13/15
//! definition; only the matrix-reduction quadrature is line-traceable to
//! `gaminr.f90`'s `gpanel`/`dspla`.

use std::sync::Arc;

use crate::groupr::gendf::{GendfGroupRecord, GendfSection};
use crate::groupr::panel::{GroupFlux, PointwiseXs};
use crate::NjoyError;

/// Electron rest energy `epair` \[eV\] — NJOY's `phys.f90:49`
/// `epair = amasse*amu*clight^2/ev`, which reduces to `amasse * 931.49410242e6`
/// (`amasse = 5.48579909065e-4` amu). Numerically ~510999 eV (0.511 MeV).
///
/// Two of these (`2*epair`, the pair-production threshold energy) are carried
/// off as annihilation photons; [`PhotonFeed::PairProduction`] deposits them.
pub const EPAIR_EV: f64 = 5.48579909065e-4 * 931.49410242e6;

// ---------------------------------------------------------------------------
// Photon production spectrum (ENDF File 15 continuous distribution)
// ---------------------------------------------------------------------------

/// A tabulated **continuous photon energy distribution** `g(E_gamma)` — the
/// ENDF File-15 secondary photon spectrum, normalised so that
/// `integral g(E_gamma) dE_gamma = 1`.
///
/// Stores ascending `(E_gamma \[eV\], p)` pairs (lin-lin, zero outside the
/// tabulated range — the `terpa` convention). The raw pairs need **not** be
/// pre-normalised: [`PhotonProductionSpectrum::group_fractions`] divides by the
/// full integral so the returned per-photon-group fractions sum to 1 (for a
/// photon group structure that spans the whole spectrum).
///
/// Units: `E_gamma` in eV; `p` is a spectral density \[1/eV\] (only ratios
/// matter after normalisation).
#[derive(Debug, Clone)]
pub struct PhotonProductionSpectrum {
    /// Ascending `(E_gamma \[eV\], p \[1/eV\])` table, shared read-only.
    points: Arc<Vec<(f64, f64)>>,
}

impl PhotonProductionSpectrum {
    /// Build a spectrum from ascending `(E_gamma \[eV\], p)` pairs.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if fewer than two points, if the energies are
    /// not strictly ascending, if any probability is negative, or if the total
    /// integral is not positive (a spectrum must carry probability mass).
    pub fn new(points: Arc<Vec<(f64, f64)>>) -> Result<Self, NjoyError> {
        if points.len() < 2 {
            return Err(NjoyError::EndfParse(
                "photon spectrum needs >= 2 tabulated points".into(),
            ));
        }
        for w in points.windows(2) {
            if !(w[1].0 > w[0].0) {
                return Err(NjoyError::EndfParse(format!(
                    "photon spectrum energies must strictly ascend: {} !< {}",
                    w[0].0, w[1].0
                )));
            }
        }
        if points.iter().any(|&(_, p)| p < 0.0) {
            return Err(NjoyError::EndfParse(
                "photon spectrum probabilities must be non-negative".into(),
            ));
        }
        let total = integrate_tab(&points, points[0].0, points[points.len() - 1].0);
        if !(total > 0.0) {
            return Err(NjoyError::EndfParse(
                "photon spectrum integrates to <= 0".into(),
            ));
        }
        Ok(PhotonProductionSpectrum { points })
    }

    /// The un-normalised integral `integral g(E_gamma) dE_gamma` over the full
    /// tabulated range \[same units as `p * E`\].
    pub fn integral(&self) -> f64 {
        integrate_tab(
            &self.points,
            self.points[0].0,
            self.points[self.points.len() - 1].0,
        )
    }

    /// The fraction of the (normalised) photon spectrum falling in each photon
    /// group defined by `photon_bounds` (ascending group edges \[eV\], length =
    /// groups + 1).
    ///
    /// Returns `photon_bounds.len() - 1` fractions, `frac[j] =
    /// integral_{group j} g / integral_all g`. For a photon structure that spans
    /// the whole spectrum these sum to 1 (the normalisation property); any
    /// spectrum mass outside the photon range is dropped (and the sum falls
    /// below 1), matching NJOY, which cannot place photons outside `egg`.
    pub fn group_fractions(&self, photon_bounds: &[f64]) -> Vec<f64> {
        let total = self.integral();
        if photon_bounds.len() < 2 || !(total > 0.0) {
            return Vec::new();
        }
        photon_bounds
            .windows(2)
            .map(|w| integrate_tab(&self.points, w[0], w[1]) / total)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Photon feed function
// ---------------------------------------------------------------------------

/// A photon-production **feed function**: how one reaction distributes its
/// produced photons across the photon sink groups, per unit incident energy.
///
/// The enum (no trait objects, per the crate design rules) closes over the
/// feed variants this port covers. Each variant supplies two things to the
/// matrix reduction:
///
/// - a **yield** `Y(E)` \[photons/reaction\] versus incident neutron energy
///   ([`PhotonFeed::yield_xs`]), and
/// - a fixed **photon-group distribution** `g_j` (summing to 1) that splits each
///   produced photon across the photon groups
///   ([`PhotonFeed::photon_group_fractions`]).
///
/// The product `Y(E) * g_j`, integrated with `sigma(E) phi(E)` over an incident
/// group, is the group-to-group production matrix element.
#[derive(Debug, Clone)]
pub enum PhotonFeed {
    /// ENDF **File-15 continuous** photon energy distribution scaled by a
    /// **File-12/13** yield.
    ///
    /// - `spectrum` — the normalised continuous photon energy distribution
    ///   `g(E_gamma)` (incident-energy-independent shape; the common File-15
    ///   case where one representative distribution is tabulated).
    /// - `yield_xs` — the photon multiplicity `Y(E_in)` \[photons/reaction\]
    ///   versus incident neutron energy, as a [`PointwiseXs`] (use
    ///   [`PointwiseXs::Constant`] for a fixed multiplicity, or
    ///   [`PointwiseXs::LinLin`] for a tabulated File-12 multiplicity).
    Continuous {
        /// Normalised continuous photon energy distribution `g(E_gamma)`.
        spectrum: PhotonProductionSpectrum,
        /// Photon yield `Y(E_in)` \[photons/reaction\] vs incident energy.
        yield_xs: PointwiseXs,
    },
    /// **Pair-production annihilation photons** — `gtff` mt=516,
    /// `gaminr.f90:1489-1506`.
    ///
    /// NJOY deposits `ff(1,1) = 2` photons (line 1503) into the photon group
    /// `iglo = ig2pp` (line 1501) — the group whose lower edge is the highest
    /// boundary below `epair` (`gaminr.f90:238-240`). Those are the two 0.511 MeV
    /// annihilation photons. The residual-energy word `ff(1,2) = e - 2*epair`
    /// (line 1504) is a heating quantity, **not** a photon group, and is
    /// intentionally not modelled by this matrix path — so this variant produces
    /// a single-group deposit of yield 2.
    PairProduction,
}

impl PhotonFeed {
    /// The photon yield `Y(E_in)` feeder \[photons/reaction\].
    ///
    /// For [`PhotonFeed::PairProduction`] this is the constant `2` annihilation
    /// photons (`gtff` line 1503).
    pub fn yield_xs(&self) -> PointwiseXs {
        match self {
            PhotonFeed::Continuous { yield_xs, .. } => yield_xs.clone(),
            PhotonFeed::PairProduction => PointwiseXs::Constant(2.0),
        }
    }

    /// The fixed photon-group distribution `g_j` (fractions summing to ~1) for
    /// the photon structure `photon_bounds` (ascending edges \[eV\]).
    ///
    /// - [`PhotonFeed::Continuous`]: the normalised File-15 spectrum integrated
    ///   per photon group ([`PhotonProductionSpectrum::group_fractions`]).
    /// - [`PhotonFeed::PairProduction`]: `1.0` in the group holding [`EPAIR_EV`]
    ///   (`ig2pp`), `0` elsewhere — a delta, since both annihilation photons
    ///   share the same 0.511 MeV energy.
    pub fn photon_group_fractions(&self, photon_bounds: &[f64]) -> Vec<f64> {
        match self {
            PhotonFeed::Continuous { spectrum, .. } => spectrum.group_fractions(photon_bounds),
            PhotonFeed::PairProduction => {
                let n = photon_bounds.len().saturating_sub(1);
                let mut frac = vec![0.0; n];
                if let Some(j) = ig2pp_index(photon_bounds) {
                    frac[j] = 1.0;
                }
                frac
            }
        }
    }
}

/// The 0-based photon group index `ig2pp` holding [`EPAIR_EV`] (0.511 MeV) —
/// the sink of the pair-production annihilation photons.
///
/// Mirrors `gaminr.f90:237-240`: `ig2pp` is the highest group whose *lower* edge
/// is below `epair`, i.e. the group whose half-open span `[egg[j], egg[j+1])`
/// contains `epair`. Returns `None` if `epair` lies outside `photon_bounds`.
fn ig2pp_index(photon_bounds: &[f64]) -> Option<usize> {
    if photon_bounds.len() < 2 {
        return None;
    }
    photon_bounds
        .windows(2)
        .position(|w| EPAIR_EV >= w[0] && EPAIR_EV < w[1])
}

// ---------------------------------------------------------------------------
// The production matrix
// ---------------------------------------------------------------------------

/// One initial (neutron) group's row of the photon-production matrix.
///
/// The Rust form of one `gpanel`/`dspla` matrix record after the group-flux
/// division (`gaminr.f90:1098`). `production[k]` is the P0 group-to-group
/// production cross section \[barn\] into photon sink group
/// `ig2lo + k` (1-based), for incident neutron group [`PhotonMatrixRow::ig`].
#[derive(Debug, Clone, PartialEq)]
pub struct PhotonMatrixRow {
    /// Initial neutron energy group index `ig` (1-based).
    pub ig: usize,
    /// Lowest photon sink group `IG2LO` (1-based) carrying non-zero production.
    pub ig2lo: usize,
    /// Group flux `integral phi dE` over this incident group \[arb·eV\] — the
    /// `dspla` denominator (`ans(1,1,1)`, `gaminr.f90:950`).
    pub group_flux: f64,
    /// Contiguous P0 production cross sections \[barn\] into photon groups
    /// `ig2lo, ig2lo+1, ...` — the divided matrix values `ans/ans(1,1,1)`.
    pub production: Vec<f64>,
}

impl PhotonMatrixRow {
    /// The total (row-summed) photon production cross section \[barn\] for this
    /// incident group — `sum_k production[k] = <sigma * Y>_ig`.
    pub fn total_production(&self) -> f64 {
        self.production.iter().sum()
    }
}

/// A neutron-to-photon **group-to-group production matrix** (P0).
///
/// Sparse by initial group: only incident neutron groups with non-zero flux and
/// production carry a [`PhotonMatrixRow`] (the "write only non-zero groups"
/// convention of GROUPR/GAMINR).
#[derive(Debug, Clone, PartialEq)]
pub struct PhotonMatrix {
    /// Number of incident neutron groups.
    pub n_neutron: usize,
    /// Number of photon sink groups.
    pub n_photon: usize,
    /// Per-initial-group rows, ascending in `ig`.
    pub rows: Vec<PhotonMatrixRow>,
}

impl PhotonMatrix {
    /// The row for incident neutron group `ig` (1-based), if it carries data.
    pub fn row(&self, ig: usize) -> Option<&PhotonMatrixRow> {
        self.rows.iter().find(|r| r.ig == ig)
    }

    /// The dense P0 production cross section \[barn\] into every photon group for
    /// incident group `ig` (1-based); zeros where the sparse row has no entry.
    ///
    /// Length is [`PhotonMatrix::n_photon`]; useful for a consumer that wants a
    /// full row rather than the trimmed contiguous span.
    pub fn dense_row(&self, ig: usize) -> Vec<f64> {
        let mut v = vec![0.0; self.n_photon];
        if let Some(r) = self.row(ig) {
            for (k, &p) in r.production.iter().enumerate() {
                let j = r.ig2lo - 1 + k;
                if j < v.len() {
                    v[j] = p;
                }
            }
        }
        v
    }
}

/// Assemble the neutron-to-photon P0 production matrix — the `gpanel` matrix
/// loop (`gaminr.f90:874-1011`) plus the `dspla` matrix branch
/// (`gaminr.f90:1083-1128`).
///
/// For each incident neutron group `[neutron_bounds[g], neutron_bounds[g+1])`
/// this integrates the group flux `integral phi dE` and the production integral
/// `integral sigma(E) Y(E) phi(E) dE` over the union of the cross-section,
/// yield, and flux break points, then distributes the production across photon
/// groups by the feed's fixed spectrum `g_j` and divides by the group flux — the
/// `result = ans(il,1,ig2)/ans(il,1,1)` division at `gaminr.f90:1098`.
///
/// # Parameters
/// - `neutron_bounds` — incident neutron group edges \[eV\], ascending, >= 2.
/// - `photon_bounds` — photon sink group edges \[eV\], ascending, >= 2.
/// - `sigma` — the base reaction cross section `sigma(E)` \[barn\]
///   ([`PointwiseXs`], reusing the `gtsig` feeder idiom).
/// - `flux` — the weighting flux `phi(E)` ([`GroupFlux`], the `gtflx` feeder).
/// - `feed` — the photon feed function ([`PhotonFeed`]).
///
/// # Returns
/// A [`PhotonMatrix`]; incident groups with zero flux or zero production carry
/// no row.
///
/// # Errors
/// [`NjoyError::EndfParse`] if either group structure has fewer than two edges.
///
/// # Example
/// ```
/// use std::sync::Arc;
/// use njoy_outram_park_fork::groupr::gaminr_matrix::{
///     photon_production_matrix, PhotonFeed, PhotonProductionSpectrum,
/// };
/// use njoy_outram_park_fork::groupr::panel::{GroupFlux, PointwiseXs};
///
/// // A flat photon spectrum from 0.1-2 MeV, a constant yield of 1, and a
/// // constant 4-barn capture cross section, over 2 neutron / 4 photon groups.
/// let spectrum = PhotonProductionSpectrum::new(Arc::new(vec![
///     (1.0e5, 1.0), (2.0e6, 1.0),
/// ])).unwrap();
/// let feed = PhotonFeed::Continuous { spectrum, yield_xs: PointwiseXs::Constant(1.0) };
/// let neutron = vec![1.0e3, 1.0e5, 2.0e7];
/// let photon = vec![1.0e5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
/// let m = photon_production_matrix(
///     &neutron, &photon, &PointwiseXs::Constant(4.0), &GroupFlux::Flat, &feed,
/// ).unwrap();
/// // Every incident group produces sigma*Y = 4 barn total, split across photons.
/// let row = m.row(1).unwrap();
/// assert!((row.total_production() - 4.0).abs() < 1e-9);
/// ```
pub fn photon_production_matrix(
    neutron_bounds: &[f64],
    photon_bounds: &[f64],
    sigma: &PointwiseXs,
    flux: &GroupFlux,
    feed: &PhotonFeed,
) -> Result<PhotonMatrix, NjoyError> {
    if neutron_bounds.len() < 2 {
        return Err(NjoyError::EndfParse(
            "photon_production_matrix: neutron_bounds needs >= 2 edges".into(),
        ));
    }
    if photon_bounds.len() < 2 {
        return Err(NjoyError::EndfParse(
            "photon_production_matrix: photon_bounds needs >= 2 edges".into(),
        ));
    }

    let n_neutron = neutron_bounds.len() - 1;
    let n_photon = photon_bounds.len() - 1;
    let frac = feed.photon_group_fractions(photon_bounds);
    let yield_xs = feed.yield_xs();

    let mut rows = Vec::new();
    for g in 0..n_neutron {
        let e_lo = neutron_bounds[g];
        let e_hi = neutron_bounds[g + 1];
        let (group_flux, prod_int) = integrate_production_panel(sigma, &yield_xs, flux, e_lo, e_hi);
        if group_flux == 0.0 || prod_int == 0.0 {
            continue; // empty / below-threshold incident group
        }
        // dspla division: matrix value = ans[j] / group_flux, with
        // ans[j] = frac[j] * prod_int (the ff distribution deposit).
        let scale = prod_int / group_flux;
        let dense: Vec<f64> = frac.iter().map(|&f| f * scale).collect();

        // Trim to the contiguous non-zero sink span (gpanel strips zero sink
        // groups, gaminr.f90:1446-1455): find first and last non-zero photon
        // group so IG2LO reflects the true lowest sink group.
        let first = dense.iter().position(|&v| v != 0.0);
        let last = dense.iter().rposition(|&v| v != 0.0);
        if let (Some(first), Some(last)) = (first, last) {
            rows.push(PhotonMatrixRow {
                ig: g + 1,
                ig2lo: first + 1,
                group_flux,
                production: dense[first..=last].to_vec(),
            });
        }
    }

    Ok(PhotonMatrix {
        n_neutron,
        n_photon,
        rows,
    })
}

/// Integrate one incident group: the group flux `integral phi dE` and the
/// production integral `integral sigma(E) Y(E) phi(E) dE` — the accumulators
/// `ans(1,1,1)` and `sum_ig ans(1,1,igt)` of the `gpanel` matrix loop.
///
/// Marches sub-panels bounded by the union of the cross-section, yield, and flux
/// break points (capped at `e_hi`), applying the same per-panel trapezoids the
/// vector engine uses (see [`crate::groupr::panel::group_integral`]). Returns
/// `(group_flux, production_integral)`.
fn integrate_production_panel(
    sigma: &PointwiseXs,
    yield_xs: &PointwiseXs,
    flux: &GroupFlux,
    e_lo: f64,
    e_hi: f64,
) -> (f64, f64) {
    debug_assert!(e_lo < e_hi, "group needs e_lo ({e_lo}) < e_hi ({e_hi})");
    let mut flux_int = 0.0_f64; // integral phi
    let mut prod_int = 0.0_f64; // integral sigma*yield*phi

    let mut e = e_lo;
    let mut phi_lo = flux.value(e);
    let mut src_lo = sigma.value(e) * yield_xs.value(e);

    let mut guard: u64 = 0;
    const GUARD_MAX: u64 = 100_000_000;

    while e < e_hi * (1.0 - 1e-15) {
        let mut e_next = sigma
            .next_break(e)
            .min(yield_xs.next_break(e))
            .min(flux.next_break(e))
            .min(e_hi);
        if !(e_next > e) {
            e_next = e_hi;
        }

        let phi_hi = flux.value(e_next);
        let src_hi = sigma.value(e_next) * yield_xs.value(e_next);

        let bq = 0.5 * (e_next - e);
        // Trapezoid of phi (group flux, gaminr.f90:950).
        flux_int += (phi_lo + phi_hi) * bq;
        // Trapezoid of sigma*yield*phi (the production reaction rate, the
        // rr*ff deposit summed over sink groups, gaminr.f90:987).
        prod_int += (src_lo * phi_lo + src_hi * phi_hi) * bq;

        e = e_next;
        phi_lo = phi_hi;
        src_lo = src_hi;

        guard += 1;
        if guard >= GUARD_MAX {
            break;
        }
    }

    (flux_int, prod_int)
}

// ---------------------------------------------------------------------------
// GENDF matrix record builder
// ---------------------------------------------------------------------------

/// Build a GENDF matrix [`GendfSection`] (`mfh = 26`, `ig2lo > 0`, `ng2 > 2`)
/// from a photon-production [`PhotonMatrix`].
///
/// Reproduces the GROUPR/GAMINR matrix record layout (`groupr.f90:882-946`; the
/// GAMINR matrix record shares it): one HEAD (CONT) record per section, then one
/// LIST record per initial group with `L1 = NG2` (secondary count),
/// `L2 = IG2LO` (lowest sink group), `N2 = ig`, and `NW` data words. For a
/// matrix record the data words are `[group_flux, m_1, m_2, ...]` — the group
/// flux `ans(1,1,1)` followed by the divided matrix values `ans(1,1,ig2)` for
/// `ig2 = 2..=NG2` (`dspla`, `gaminr.f90:1094-1102`), so `NG2 = n_sinks + 1`.
///
/// A row whose contiguous sink span has at least one entry yields `NG2 >= 2`;
/// spans of length >= 2 give the `NG2 > 2` matrix records the round-trip test
/// exercises.
///
/// # Parameters
/// - `mf` — ENDF file number; `26` for a photon-production matrix.
/// - `mt` — reaction MT (e.g. `102` for radiative capture photon production).
/// - `za`, `zam` — material ZA and isomer tag (HEAD `C1`/`C2`).
/// - `temperature_k` — averaging temperature \[K\] (each LIST `C1`).
/// - `matrix` — the production matrix to serialise.
///
/// `NL = NZ = 1` (P0, infinite dilution); `NGN` = number of incident groups.
pub fn photon_matrix_gendf_section(
    mf: i32,
    mt: i32,
    za: f64,
    zam: f64,
    temperature_k: f64,
    matrix: &PhotonMatrix,
) -> GendfSection {
    let records = matrix
        .rows
        .iter()
        .map(|row| {
            let mut data = Vec::with_capacity(row.production.len() + 1);
            data.push(row.group_flux);
            data.extend_from_slice(&row.production);
            GendfGroupRecord {
                ig: row.ig as i32,
                ig2lo: row.ig2lo as i32,
                ng2: (row.production.len() + 1) as i32,
                data,
            }
        })
        .collect();

    GendfSection {
        mf,
        mt,
        za,
        zam,
        nl: 1,
        nz: 1,
        lrflag: 0,
        num_groups: matrix.n_neutron as i32,
        temperature_k,
        records,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Integrate an ascending lin-lin `(x, y)` table over `[lo, hi]`, treating the
/// table as zero outside its tabulated range (the `terpa` convention).
///
/// Used to integrate the photon spectrum over a photon group and over its full
/// range. Clips `[lo, hi]` to the table span, then sums trapezoids over the
/// table sub-intervals with the clipped-endpoint values interpolated lin-lin.
fn integrate_tab(points: &[(f64, f64)], lo: f64, hi: f64) -> f64 {
    if points.len() < 2 || !(hi > lo) {
        return 0.0;
    }
    let x_min = points[0].0;
    let x_max = points[points.len() - 1].0;
    let a = lo.max(x_min);
    let b = hi.min(x_max);
    if !(b > a) {
        return 0.0;
    }

    let mut acc = 0.0_f64;
    for w in points.windows(2) {
        let (x1, y1) = w[0];
        let (x2, y2) = w[1];
        // Overlap of [a, b] with this table interval [x1, x2].
        let s = a.max(x1);
        let t = b.min(x2);
        if !(t > s) {
            continue;
        }
        let ys = lin_interp(x1, y1, x2, y2, s);
        let yt = lin_interp(x1, y1, x2, y2, t);
        acc += 0.5 * (ys + yt) * (t - s);
    }
    acc
}

/// Lin-lin interpolation of `y` at `x` on the segment `(x1,y1)-(x2,y2)`.
fn lin_interp(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if x2 == x1 {
        return y1;
    }
    y1 + (y2 - y1) * (x - x1) / (x2 - x1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tabulated File-15 photon spectrum integrates (per incident energy) to
    /// 1 across a photon structure that spans it — the normalisation the feed
    /// must satisfy before yield scaling.
    ///
    /// **Methodology.** A flat spectrum `g(E_gamma) = 1` on `[0.1, 2.0] MeV`.
    /// Take [`PhotonProductionSpectrum::group_fractions`] over a 4-group photon
    /// structure spanning `[0.1, 2.0] MeV` and assert the fractions sum to 1 to
    /// < 1e-12, and that equal-width groups get equal (0.25) shares. Also assert
    /// a symmetric triangular spectrum normalises to 1 and splits evenly.
    ///
    /// **Expected result (analytic; NOT executed in the drafting agent's
    /// environment — see handoff, must be run by the verification pass).** flat:
    /// fractions sum to 1.0 (dev < 1e-12), each equal-width group = 0.25;
    /// triangular: sum 1.0, halves equal.
    #[test]
    fn file15_spectrum_normalizes_to_one() {
        // Flat spectrum over [1e5, 2e6] eV.
        let spectrum =
            PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 1.0), (2.0e6, 1.0)])).unwrap();
        // Photon groups spanning exactly the spectrum range, 4 equal-width groups.
        let bounds = vec![1.0e5, 5.75e5, 1.05e6, 1.525e6, 2.0e6];
        let frac = spectrum.group_fractions(&bounds);
        assert_eq!(frac.len(), 4);
        let sum: f64 = frac.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12, "flat spectrum sum {sum}");
        // Equal-width groups of a flat spectrum -> equal 0.25 fractions.
        for (j, &f) in frac.iter().enumerate() {
            assert!((f - 0.25).abs() < 1e-12, "group {j} fraction {f}");
        }

        // A triangular spectrum peaking at the midpoint 1.05e6 eV still
        // normalises to 1, and split at that midpoint the two equal-base halves
        // carry equal mass (0.5 each).
        let tri = PhotonProductionSpectrum::new(Arc::new(vec![
            (1.0e5, 0.0),
            (1.05e6, 2.0),
            (2.0e6, 0.0),
        ]))
        .unwrap();
        let bounds_tri = vec![1.0e5, 1.05e6, 2.0e6];
        let frac_tri = tri.group_fractions(&bounds_tri);
        let sum_tri: f64 = frac_tri.iter().sum();
        assert!((sum_tri - 1.0).abs() < 1e-12, "triangular sum {sum_tri}");
        // Symmetric triangle split at its peak -> equal halves (0.5 each).
        assert!(
            (frac_tri[0] - 0.5).abs() < 1e-12 && (frac_tri[1] - 0.5).abs() < 1e-12,
            "triangle halves {frac_tri:?}"
        );
    }

    /// **Yield / photon-energy conservation.** Summing the P0 production matrix
    /// over photon groups for an incident neutron group equals the flux-weighted
    /// total photon-production cross section `<sigma * Y>` — to tight tolerance.
    ///
    /// **Methodology.** Constant capture `sigma = 3.0 barn`, constant yield
    /// `Y = 2.5 photons/reaction`, flat flux, over 3 neutron groups and a 5-group
    /// photon structure spanning the spectrum. For constant `sigma` and `Y` the
    /// flux-weighted total production is exactly `sigma*Y = 7.5 barn` in every
    /// incident group, independent of the flux. Assert each row sum equals 7.5 to
    /// < 1e-9, and (independently) that the dense row sums to the same value.
    ///
    /// **Expected result (analytic; NOT executed in the drafting agent's
    /// environment — see handoff).** all three incident rows sum to 7.5 barn
    /// (dense and trimmed), max dev < 1e-9. Conservation must be confirmed by run.
    #[test]
    fn production_matrix_conserves_yield() {
        let spectrum =
            PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 1.0), (2.0e6, 1.0)])).unwrap();
        let feed = PhotonFeed::Continuous {
            spectrum,
            yield_xs: PointwiseXs::Constant(2.5),
        };
        let neutron = vec![1.0e3, 1.0e4, 1.0e5, 2.0e7];
        let photon = vec![1.0e5, 5.0e5, 9.0e5, 1.3e6, 1.6e6, 2.0e6];
        let m = photon_production_matrix(
            &neutron,
            &photon,
            &PointwiseXs::Constant(3.0),
            &GroupFlux::Flat,
            &feed,
        )
        .unwrap();
        assert_eq!(m.n_neutron, 3);
        assert_eq!(m.n_photon, 5);
        assert_eq!(m.rows.len(), 3, "all three incident groups produce photons");
        for ig in 1..=3 {
            let row = m.row(ig).expect("row present");
            let total = row.total_production();
            assert!(
                (total - 7.5).abs() < 1e-9,
                "incident group {ig} total production {total} != 7.5"
            );
            let dense_total: f64 = m.dense_row(ig).iter().sum();
            assert!(
                (dense_total - 7.5).abs() < 1e-9,
                "dense row {ig} sum {dense_total} != 7.5"
            );
        }
    }

    /// Conservation also holds under a **non-flat** flux and **energy-dependent**
    /// yield: the row sum must equal the independently flux-weighted `<sigma*Y>`.
    ///
    /// **Methodology.** Linear cross section `sigma` from 0.1-1.0 barn and linear
    /// yield `Y` from 1.1-2.0 over `[1e5, 1e6]` eV, 1/E flux, single neutron
    /// group. Compute the matrix row sum, and independently the flux-weighted
    /// production `integral sigma*Y*phi / integral phi` via the same panel
    /// integrator. Assert they agree to < 1e-9 (identical trapezoid) and the
    /// total is positive.
    ///
    /// **Expected result (analytic; NOT executed in the drafting agent's
    /// environment — see handoff).** row sum == independent flux-weighted
    /// production to < 1e-9; total > 0. Must be confirmed by run.
    #[test]
    fn conservation_under_varying_flux_and_yield() {
        let sigma = PointwiseXs::LinLin(Arc::new(vec![(1.0e5, 0.1), (1.0e6, 1.0)]));
        let yield_xs = PointwiseXs::LinLin(Arc::new(vec![(1.0e5, 1.1), (1.0e6, 2.0)]));
        let spectrum =
            PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 1.0), (2.0e6, 1.0)])).unwrap();
        let feed = PhotonFeed::Continuous {
            spectrum,
            yield_xs: yield_xs.clone(),
        };
        let neutron = vec![1.0e5, 1.0e6];
        let photon = vec![1.0e5, 6.0e5, 1.1e6, 1.6e6, 2.0e6];
        let flux = crate::groupr::panel::GroupFlux::analytic(
            crate::groupr::weights::AnalyticWeight::OneOverE,
            0.0,
        );
        let m = photon_production_matrix(&neutron, &photon, &sigma, &flux, &feed).unwrap();
        let row = m.row(1).expect("row");
        let row_sum = row.total_production();

        // Independent flux-weighted production over the same group.
        let (den, prod_int) = integrate_production_panel(&sigma, &yield_xs, &flux, 1.0e5, 1.0e6);
        let expected = prod_int / den;
        assert!(
            (row_sum - expected).abs() < 1e-9,
            "row sum {row_sum} vs flux-weighted <sigma*Y> {expected}"
        );
        assert!(row_sum > 0.0, "production must be positive");
    }

    /// The GENDF **matrix** record round-trips through
    /// [`GendfSection::to_rows`]/[`GendfSection::from_rows`] with `mfh = 26`,
    /// `IG2LO > 0`, `NG2 > 2`.
    ///
    /// **Methodology.** Build a production matrix (continuous feed, 2 neutron / 4
    /// photon groups) whose rows span >= 2 photon sink groups, serialise it as a
    /// `mf=26` [`GendfSection`] via [`photon_matrix_gendf_section`], write to rows
    /// and parse back, and assert the parsed section equals the original. Also
    /// assert every LIST record carries `IG2LO >= 1` and `NG2 > 2` (a true matrix
    /// record, distinct from the `NG2 = 2` vector record).
    ///
    /// **Expected result (analytic; NOT executed in the drafting agent's
    /// environment — see handoff).** section round-trips exactly; records report
    /// IG2LO = 1 and NG2 = 5 (> 2). Must be confirmed by run.
    #[test]
    fn gendf_matrix_record_round_trips() {
        let spectrum =
            PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 1.0), (2.0e6, 1.0)])).unwrap();
        let feed = PhotonFeed::Continuous {
            spectrum,
            yield_xs: PointwiseXs::Constant(1.0),
        };
        let neutron = vec![1.0e3, 1.0e5, 2.0e7];
        let photon = vec![1.0e5, 5.0e5, 1.0e6, 1.5e6, 2.0e6];
        let m = photon_production_matrix(
            &neutron,
            &photon,
            &PointwiseXs::Constant(4.0),
            &GroupFlux::Flat,
            &feed,
        )
        .unwrap();
        assert!(!m.rows.is_empty(), "matrix must have data");

        let section = photon_matrix_gendf_section(26, 102, 92238.0, 0.0, 293.6, &m);
        assert_eq!(section.mf, 26);
        for rec in &section.records {
            assert!(rec.ig2lo >= 1, "IG2LO must be > 0, got {}", rec.ig2lo);
            assert!(rec.ng2 > 2, "matrix NG2 must be > 2, got {}", rec.ng2);
            // NW = NG2 words: [flux, m_1, ..., m_{ng2-1}].
            assert_eq!(rec.data.len() as i32, rec.ng2);
        }

        let rows = section.to_rows();
        let back = GendfSection::from_rows(26, 102, &rows).unwrap();
        assert_eq!(back, section, "mf=26 matrix section survives round trip");
    }

    /// Pair-production feed deposits the two annihilation photons into the group
    /// holding `epair` (0.511 MeV) — a single-group delta of yield 2.
    ///
    /// **Methodology.** With [`PhotonFeed::PairProduction`] and a photon
    /// structure straddling 0.511 MeV, assert [`PhotonFeed::photon_group_fractions`]
    /// is `1.0` in exactly the group containing [`EPAIR_EV`] and `0` elsewhere,
    /// and that a matrix built with a constant pair cross section `sigma = 5 barn`
    /// deposits `2 * 5 = 10 barn` into that one photon group (row sum 10).
    ///
    /// **Expected result (analytic; NOT executed in the drafting agent's
    /// environment — see handoff).** `epair` ~= 510999 eV lands in the group
    /// `[5.0e5, 6.0e5)` (1-based group 2); fraction 1.0 there, 0 elsewhere; row
    /// sum 10 barn (dev < 1e-9). Must be confirmed by run.
    #[test]
    fn pair_production_deposits_annihilation_photons() {
        // epair ~ 0.511 MeV.
        assert!(
            (EPAIR_EV - 510_998.95).abs() < 5.0,
            "epair {EPAIR_EV} eV not ~0.511 MeV"
        );
        let photon = vec![1.0e5, 5.0e5, 6.0e5, 1.0e6, 2.0e6];
        let feed = PhotonFeed::PairProduction;
        let frac = feed.photon_group_fractions(&photon);
        // epair in [5.0e5, 6.0e5) -> group index 1 (0-based).
        assert_eq!(frac.len(), 4);
        assert!(
            (frac[1] - 1.0).abs() < 1e-15,
            "delta group fraction {frac:?}"
        );
        let others: f64 = frac
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != 1)
            .map(|(_, v)| v)
            .sum();
        assert_eq!(others, 0.0, "no photons outside the epair group");

        let neutron = vec![1.0e5, 2.0e7];
        let m = photon_production_matrix(
            &neutron,
            &photon,
            &PointwiseXs::Constant(5.0),
            &GroupFlux::Flat,
            &feed,
        )
        .unwrap();
        let row = m.row(1).expect("row");
        assert_eq!(
            row.ig2lo, 2,
            "sink group is the 0.511 MeV group (1-based 2)"
        );
        assert!(
            (row.total_production() - 10.0).abs() < 1e-9,
            "pair production 2*5 = 10 barn, got {}",
            row.total_production()
        );
    }

    /// A spectrum whose mass partly falls **outside** the photon range loses that
    /// mass (sum of fractions < 1) rather than fabricating photons outside `egg`.
    ///
    /// **Methodology.** Flat spectrum on `[1e5, 2e6]` eV, photon structure only
    /// `[1e5, 1e6]` eV (covers the lower half). Assert the fractions sum to the
    /// covered mass fraction `(1e6-1e5)/(2e6-1e5) = 0.9e6/1.9e6 ~= 0.4737`, not 1.
    ///
    /// **Expected result (analytic; NOT executed in the drafting agent's
    /// environment — see handoff).** covered fraction ~= 0.47368 (< 1), matching
    /// the analytic ratio (dev < 1e-9). Must be confirmed by run.
    #[test]
    fn spectrum_mass_outside_photon_range_is_dropped() {
        let spectrum =
            PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 1.0), (2.0e6, 1.0)])).unwrap();
        let bounds = vec![1.0e5, 5.0e5, 1.0e6];
        let frac = spectrum.group_fractions(&bounds);
        let sum: f64 = frac.iter().sum();
        let expected = (1.0e6 - 1.0e5) / (2.0e6 - 1.0e5);
        assert!(
            (sum - expected).abs() < 1e-9,
            "covered fraction {sum} != {expected}"
        );
        assert!(sum < 1.0, "partial coverage must lose mass");
    }

    /// A malformed spectrum (descending energies, too few points, negative
    /// probability, zero integral) is rejected with [`NjoyError::EndfParse`]
    /// rather than panicking or fabricating a distribution.
    #[test]
    fn bad_spectrum_is_rejected() {
        // Too few points.
        assert!(PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 1.0)])).is_err());
        // Descending energies.
        assert!(PhotonProductionSpectrum::new(Arc::new(vec![(2.0e6, 1.0), (1.0e5, 1.0)])).is_err());
        // Negative probability.
        assert!(
            PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, -1.0), (2.0e6, 1.0)])).is_err()
        );
        // Zero integral (all-zero probabilities).
        assert!(PhotonProductionSpectrum::new(Arc::new(vec![(1.0e5, 0.0), (2.0e6, 0.0)])).is_err());
    }

    /// The `NO_NEXT_BREAK_EV` sentinel (reused from [`crate::groupr::panel`]) is
    /// finite so the panel marcher terminates; guards against an accidental
    /// infinite-loop feeder.
    #[test]
    fn sentinel_is_finite() {
        let sentinel = crate::groupr::panel::NO_NEXT_BREAK_EV;
        assert!(sentinel.is_finite() && sentinel > 0.0);
    }
}
