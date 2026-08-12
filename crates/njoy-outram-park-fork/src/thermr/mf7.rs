//! Read ENDF **MF=7** thermal scattering-law (S(α,β)) evaluations.
//!
//! MF=7 is the thermal sublibrary (`tsl-*`) input to THERMR. It holds, for a
//! bound scatterer (graphite, H in H₂O, Al, ZrH, …):
//!
//! - **MT=2 — thermal elastic.** `LTHR=1` **coherent** elastic (Bragg
//!   diffraction): a cumulative structure factor `S(E)` that steps up at each
//!   Bragg edge (stored histogram, interpolation law 1). `LTHR=2` **incoherent**
//!   elastic: a bound cross section plus a Debye-Waller integral `W'(T)`.
//! - **MT=4 — incoherent inelastic.** The scattering law `S(α,β)` itself: a set
//!   of B-constants (bound cross section, mass ratios, atom counts) followed by a
//!   table of `S` over a grid of momentum transfer `α` and energy transfer `β`,
//!   at one or more temperatures.
//!
//! This module parses those records into typed data at **all tabulated
//! temperatures**: the coherent-elastic `S(E)` is retained for the base
//! temperature `T₀` *and* every `LT` extra-temperature LIST (with the ENDF `LI`
//! temperature-interpolation codes), and the incoherent-inelastic `S(α,β)` is
//! selected/interpolated at a requested temperature via
//! [`parse_mf7_at_temperature`]. Ported faithfully to the ENDF-102 MF=7 format,
//! cross-checked against `thermr.f90`'s reader (`rdelas` / `calcem`).
//!
//! ## Temperature policy (shared by both channels)
//!
//! A requested temperature is resolved against the evaluation's tabulated grid
//! by [`select_temperature`]:
//!
//! 1. **Match within the NJOY tolerance** `T/1000 + 5` K (upstream
//!    `thermr.f90::rdelas`) → that tabulated table is used as-is.
//! 2. **Strictly between two tabulated temperatures** (beyond the tolerance) →
//!    interpolate `S` point-by-point between the bracketing tables using the
//!    `LI` interpolation law the evaluation specifies (ENDF-102 §7.2/§7.4;
//!    lin-lin for the VIII.0 graphite elastic, log-lin for its inelastic).
//!    Upstream NJOY would refuse here ("desired temperature not on tape");
//!    interpolating with the file's own `LI` law is the ENDF-sanctioned
//!    generalisation.
//! 3. **Outside the tabulated range** (beyond the tolerance) →
//!    [`NjoyError::TemperatureOutOfRange`]. Never a silent nearest-temperature
//!    snap.
//!
//! Units: energies and `β` scale in eV (or in kT when `LAT=1`); `α`/`β` are
//! dimensionless; `S` is per the ENDF convention (per unit `α·β`).

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::{records::SectionCursor, tape::Tape};
use crate::NjoyError;

/// Coherent-elastic (Bragg) scattering: the cumulative structure factor
/// `S(E, T)` at **every tabulated temperature**.
///
/// `S(E)` is a step function of incident energy — flat between Bragg edges and
/// jumping at each edge — so it is stored with histogram interpolation in `E`.
/// The elastic cross section is `σ(E, T) = S(E, T) / E`; the Bragg-edge
/// *energies* are temperature-independent (they are set by the lattice
/// spacing), while the `S` values fall with temperature through the
/// Debye-Waller factor. Temperature-aware evaluation lives in
/// [`super::coherent`] (`cross_section`, `bragg_reflections`, `s_of_e_at`).
#[derive(Debug, Clone)]
pub struct CoherentElastic {
    /// Bragg-edge energies `E_i` \[eV\], ascending — shared by every tabulated
    /// temperature (the `LT` extra-temperature LISTs carry `S` only).
    pub bragg_energies_ev: Vec<f64>,
    /// Tabulated temperatures `T_j` \[K\], ascending; `temperatures_k[0]` is
    /// the base temperature `T₀` of the evaluation.
    pub temperatures_k: Vec<f64>,
    /// Cumulative structure factor tables: `s_tables[j][i]` = `S(E_i, T_j)`
    /// \[eV·b\]. One row per entry of [`temperatures_k`](Self::temperatures_k),
    /// each on the shared [`bragg_energies_ev`](Self::bragg_energies_ev) grid.
    pub s_tables: Vec<Vec<f64>>,
    /// ENDF `LI` temperature-interpolation codes: `temp_interp[j]` is the law
    /// for interpolating `S` between `T_j` and `T_{j+1}` (length
    /// `temperatures_k.len() − 1`; `2` = lin-lin for the ENDF/B-VIII.0
    /// graphite evaluations).
    pub temp_interp: Vec<u32>,
}

/// Incoherent-elastic scattering (`LTHR=2`): a bound cross section plus the
/// temperature-dependent Debye-Waller integral `W'(T)`.
///
/// Used for a hydrogenous solid where the bound protons scatter elastically but
/// *incoherently* (H in ZrH, polyethylene, …): the neutron keeps its energy but
/// the angular distribution is smooth (no Bragg edges). The cross section is
///
/// ```text
///   σ(E,T) = (σ_b / 2N) · (1 − exp(−4·E·W'(T))) / (2·E·W'(T)),
/// ```
///
/// with `σ_b` the characteristic bound cross section and `W'(T)` interpolated
/// from [`wp_of_t`](Self::wp_of_t).
#[derive(Debug, Clone)]
pub struct IncoherentElastic {
    /// Base temperature `T₀` \[K\].
    pub temperature_k: f64,
    /// Characteristic bound cross section `σ_b` = SB \[barn\] (the `C1` of the
    /// `LTHR=2` TAB1).
    pub sb: f64,
    /// TAB1 interpolation law over the `(T, W')` table.
    pub interp: Vec<(u32, u32)>,
    /// `(T \[K\], W'(T) \[eV⁻¹\])` Debye-Waller integral table, ascending in `T`.
    pub wp_of_t: Vec<(f64, f64)>,
}

/// Incoherent-inelastic scattering law `S(α, β)` at the base temperature.
#[derive(Debug, Clone)]
pub struct IncoherentInelastic {
    /// `LAT`: `1` if `α`/`β` are scaled to 0.0253 eV (room temperature), else `0`.
    pub lat: i32,
    /// `LASYM`: `1` if `S(α,β)` is asymmetric in `β` (both signs stored).
    pub lasym: i32,
    /// The `B(1..NI)` constants: `B(1)` bound cross section factor (`0` ⇒ the
    /// principal scatterer uses the free/short-collision-time model, no table),
    /// `B(3)` the mass ratio `A`, `B(6)` the number of principal atoms, etc.
    pub b: Vec<f64>,
    /// The temperature \[K\] the retained `S(α,β)` tables represent: a
    /// tabulated temperature (when the request matched one within the NJOY
    /// `T/1000 + 5` K tolerance, or none was requested → base `T₀`), or the
    /// requested temperature itself when the tables were interpolated between
    /// two tabulated temperatures (see the [module docs](self)).
    pub temperature_k: f64,
    /// Energy-transfer grid `β` (dimensionless), one entry per [`s_tables`] row.
    ///
    /// [`s_tables`]: Self::s_tables
    pub beta: Vec<f64>,
    /// For each `β`, the `S(α)` table at [`temperature_k`](Self::temperature_k)
    /// (tabulated or temperature-interpolated — see the [module docs](self)).
    pub s_tables: Vec<AlphaTable>,
    /// All temperatures \[K\] tabulated in the evaluation (base `T₀` first,
    /// then the `LT` extras), whether or not they were selected.
    pub tabulated_temperatures_k: Vec<f64>,
    /// Principal-scatterer effective-temperature table `(T \[K\], T_eff \[K\])`
    /// used by the short-collision-time (SCT) branch — the trailing TAB1 of
    /// MF=7/MT=4. `T_eff(T)` (Egelstaff–Schofield) is larger than the physical
    /// `T` because it folds in the mean squared vibrational velocity of the bound
    /// atom (zero-point motion), so the SCT limit reproduces the free-gas kernel
    /// with the correct second energy moment. Empty if the evaluation omits it.
    pub teff_table: Vec<(f64, f64)>,
}

/// One `S(α)` slice at a fixed `β` (and base temperature).
#[derive(Debug, Clone)]
pub struct AlphaTable {
    /// The energy-transfer `β` this slice belongs to.
    pub beta: f64,
    /// Momentum-transfer grid `α` (dimensionless), ascending.
    pub alpha: Vec<f64>,
    /// `S(α, β)` values on `alpha`.
    pub s: Vec<f64>,
}

/// A parsed MF=7 thermal scattering evaluation for one material.
#[derive(Debug, Clone)]
pub struct Mf7 {
    /// ZA of the scatterer identifier (evaluation-specific for `tsl` materials).
    pub za: f64,
    /// Atomic weight ratio of the principal scattering atom.
    pub awr: f64,
    /// Coherent (Bragg) elastic (MT=2, `LTHR=1` or `3`), if present.
    pub coherent_elastic: Option<CoherentElastic>,
    /// Incoherent elastic (MT=2, `LTHR=2` or `3`), if present.
    pub incoherent_elastic: Option<IncoherentElastic>,
    /// Incoherent inelastic S(α,β) (MT=4), if present.
    pub incoherent_inelastic: Option<IncoherentInelastic>,
}

/// NJOY's tabulated-temperature matching tolerance \[K\] for thermal
/// scattering data: `T/1000 + 5` K around a requested temperature `T`
/// (upstream `thermr.f90::rdelas`, `abs(tnow-temp) < temp/1000+5`).
///
/// A request within this distance of a tabulated temperature uses that table
/// directly (e.g. 293.15 K → the 296 K graphite table, 2.85 K away); a request
/// farther than this from every tabulated point is either interpolated
/// (inside the tabulated range) or refused (outside it) — see
/// [`select_temperature`].
pub fn temperature_match_tolerance_k(target_k: f64) -> f64 {
    target_k.abs() / 1000.0 + 5.0
}

/// How a requested temperature resolves against an evaluation's tabulated
/// temperature grid — the outcome of [`select_temperature`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemperatureSelection {
    /// The request matched the tabulated temperature at this index (within the
    /// NJOY [`temperature_match_tolerance_k`] tolerance); use its table as-is.
    Tabulated(usize),
    /// The request falls strictly between tabulated temperatures `lo` and `hi`
    /// (= `lo + 1`), farther than the tolerance from both: interpolate `S`
    /// between their tables with ENDF law `li` at `target_k` \[K\].
    Interpolated {
        /// Index of the tabulated temperature below the request.
        lo: usize,
        /// Index of the tabulated temperature above the request (`lo + 1`).
        hi: usize,
        /// ENDF `LI` interpolation code for this temperature interval.
        li: u32,
        /// The requested temperature \[K\] to interpolate at.
        target_k: f64,
    },
}

impl TemperatureSelection {
    /// The temperature \[K\] the selected/interpolated tables represent:
    /// the tabulated value for [`Tabulated`](Self::Tabulated), the requested
    /// target for [`Interpolated`](Self::Interpolated).
    pub fn resolved_temperature_k(&self, temps: &[f64]) -> f64 {
        match *self {
            TemperatureSelection::Tabulated(j) => temps.get(j).copied().unwrap_or(0.0),
            TemperatureSelection::Interpolated { target_k, .. } => target_k,
        }
    }
}

/// Resolve a requested temperature `target_k` \[K\] against the tabulated grid
/// `temps` \[K\] (ascending), applying the policy in the [module docs](self):
/// tolerance-match → interpolate → refuse.
///
/// `interval_li[j]` is the ENDF `LI` interpolation code between `temps[j]` and
/// `temps[j+1]` (length `temps.len() − 1`); a missing entry defaults to
/// lin-lin (`2`).
///
/// # Errors
/// [`NjoyError::TemperatureOutOfRange`] when `target_k` is outside the
/// tabulated range by more than [`temperature_match_tolerance_k`], and
/// [`NjoyError::EndfParse`] for an empty temperature grid.
pub fn select_temperature(
    temps: &[f64],
    interval_li: &[u32],
    target_k: f64,
) -> Result<TemperatureSelection, NjoyError> {
    if temps.is_empty() {
        return Err(NjoyError::EndfParse(
            "thermal evaluation has no tabulated temperatures".into(),
        ));
    }
    // Nearest tabulated temperature within the NJOY tolerance → use it.
    let (nearest, &t_near) = temps
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (*a - target_k)
                .abs()
                .partial_cmp(&(*b - target_k).abs())
                .unwrap()
        })
        .unwrap();
    if (t_near - target_k).abs() <= temperature_match_tolerance_k(target_k) {
        return Ok(TemperatureSelection::Tabulated(nearest));
    }
    // Bracketed by two adjacent tabulated temperatures → interpolate.
    for j in 0..temps.len().saturating_sub(1) {
        if (temps[j] - target_k) * (temps[j + 1] - target_k) < 0.0 {
            let li = interval_li.get(j).copied().unwrap_or(2);
            return Ok(TemperatureSelection::Interpolated {
                lo: j,
                hi: j + 1,
                li,
                target_k,
            });
        }
    }
    // Outside the tabulated range: refuse rather than extrapolate or snap.
    let (mut min_k, mut max_k) = (f64::INFINITY, f64::NEG_INFINITY);
    for &t in temps {
        min_k = min_k.min(t);
        max_k = max_k.max(t);
    }
    Err(NjoyError::TemperatureOutOfRange {
        requested_k: target_k,
        min_k,
        max_k,
    })
}

/// Interpolate one `S` value between two temperatures under ENDF law `li`
/// (the `LI` code of the extra-temperature LIST record).
///
/// Degenerate/zero endpoints degrade per upstream `terp1` semantics (a flat
/// segment returns the endpoint); a log-law domain error (one endpoint zero,
/// the other not — possible for `LI=4` S(α,β) entries at the sparse corners of
/// the grid) falls back to lin-lin rather than failing the whole parse.
pub(crate) fn interp_s_temperature(
    t_lo: f64,
    s_lo: f64,
    t_hi: f64,
    s_hi: f64,
    target_k: f64,
    li: u32,
) -> f64 {
    terp1(t_lo, s_lo, t_hi, s_hi, target_k, IntLaw::from_code(li)).unwrap_or_else(|_| {
        // Lin-lin fallback for log-law domain errors (documented divergence
        // from a hard failure; upstream terp1 would emit y1 for S=0).
        s_lo + (s_hi - s_lo) * (target_k - t_lo) / (t_hi - t_lo)
    })
}

/// Parse the MF=7 thermal scattering data for material `mat` from `tape`.
///
/// Reads MT=2 (thermal elastic) and MT=4 (incoherent inelastic) if present. At
/// least one must be present or [`NjoyError::SectionNotFound`] is returned.
///
/// # Errors
/// - [`NjoyError::SectionNotFound`] if neither MT=2 nor MT=4 exists for `mat`.
/// - [`NjoyError::NotPorted`] for the short-collision-time (`B(1)=0`) principal
///   scatterer (no tabulated S).
/// - [`NjoyError::EndfParse`] on malformed records.
pub fn parse_mf7(tape: &Tape, mat: i32) -> Result<Mf7, NjoyError> {
    parse_mf7_at_temperature(tape, mat, None)
}

/// Parse the MF=7 thermal scattering data for `mat`, resolving the incoherent-
/// inelastic `S(α,β)` tables at the requested temperature `target_k` \[K\].
///
/// `tsl-*` evaluations tabulate `S(α,β)` at several temperatures (base `T₀`
/// plus extras). This variant applies the temperature policy in the
/// [module docs](self): a request within the NJOY `T/1000 + 5` K tolerance of
/// a tabulated point uses that table; a request between tabulated points gets
/// the `S(α,β)` interpolated point-by-point with the evaluation's `LI` law; a
/// request outside the tabulated range is refused. `target_k = None`
/// reproduces [`parse_mf7`] (base `T₀`).
///
/// The temperature the retained tables represent is reported in
/// [`IncoherentInelastic::temperature_k`] (the tabulated value when snapped
/// within tolerance, the request itself when interpolated); the full tabulated
/// grid is in [`IncoherentInelastic::tabulated_temperatures_k`].
///
/// The coherent-elastic `S(E)` tables (all temperatures) are always retained
/// in full; their temperature resolution happens at evaluation time in
/// [`super::coherent`].
///
/// # Errors
/// Same as [`parse_mf7`], plus [`NjoyError::TemperatureOutOfRange`] when
/// `target_k` is outside the tabulated range by more than the tolerance.
pub fn parse_mf7_at_temperature(
    tape: &Tape,
    mat: i32,
    target_k: Option<f64>,
) -> Result<Mf7, NjoyError> {
    let elastic = tape.section(mat, 7, 2);
    let inelastic = tape.section(mat, 7, 4);
    if elastic.is_none() && inelastic.is_none() {
        return Err(NjoyError::SectionNotFound { mat, mf: 7, mt: 2 });
    }

    // Grab ZA/AWR from whichever section exists (both carry it in the HEAD).
    let (za, awr) = {
        let sec = elastic.or(inelastic).unwrap();
        let head = SectionCursor::new(&sec.rows).read_cont()?;
        (head.c1, head.c2)
    };

    let (coherent_elastic, incoherent_elastic) = match elastic {
        Some(sec) => parse_elastic(sec)?,
        None => (None, None),
    };
    let incoherent_inelastic = inelastic
        .map(|sec| parse_inelastic(sec, target_k))
        .transpose()?;

    Ok(Mf7 {
        za,
        awr,
        coherent_elastic,
        incoherent_elastic,
        incoherent_inelastic,
    })
}

/// Parse MF=7/MT=2 thermal elastic. Returns `(coherent, incoherent)` — either,
/// both, or neither may be present depending on `LTHR`:
///
/// - `LTHR=1` — coherent (Bragg) elastic only: one `S(E)` TAB1 plus `LT` extra-
///   temperature LISTs.
/// - `LTHR=2` — incoherent elastic only: one `W'(T)` TAB1 (`SB = C1`).
/// - `LTHR=3` — both: the coherent `S(E)` TAB1 and its LISTs, then the
///   incoherent `W'(T)` TAB1.
///
/// Record layout follows NJOY2016 `thermr.f90::rdelas`.
fn parse_elastic(
    section: &crate::endf::tape::Section,
) -> Result<(Option<CoherentElastic>, Option<IncoherentElastic>), NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, LTHR, 0, 0, 0
    let lthr = head.l1;

    // First TAB1: for LTHR=2 it is the incoherent W'(T) table; otherwise it is
    // the coherent S(E) table.
    let tab1 = cur.read_tab1()?;

    if lthr == 2 {
        return Ok((None, Some(read_incoherent_elastic(&tab1))));
    }

    // Coherent S(E): T0 = C1, LT = L1 (extra temperatures), (E, S) pairs.
    let temperature_k = tab1.head.c1;
    let lt = tab1.head.l1;
    let (bragg_energies_ev, s_base): (Vec<f64>, Vec<f64>) = tab1.pairs.iter().copied().unzip();

    // The LT extra temperatures follow as LIST records: [T, 0, LI, 0, NP, 0 /
    // S(E_i, T)] — S only, on the shared Bragg-edge grid. Retain every table
    // (pre-2026-08-11 this kept only the temperatures, so σ_coh_el existed at
    // the base temperature alone — bead op-1y4y).
    let mut temperatures_k = Vec::with_capacity(lt as usize + 1);
    let mut s_tables = Vec::with_capacity(lt as usize + 1);
    let mut temp_interp = Vec::with_capacity(lt as usize);
    temperatures_k.push(temperature_k);
    s_tables.push(s_base);
    for _ in 0..lt {
        let list = cur.read_list()?;
        if list.data.len() != bragg_energies_ev.len() {
            return Err(NjoyError::EndfParse(format!(
                "coherent-elastic extra-temperature LIST at {} K has {} S values, \
                 expected {} (the shared Bragg-edge grid)",
                list.head.c1,
                list.data.len(),
                bragg_energies_ev.len()
            )));
        }
        temperatures_k.push(list.head.c1);
        temp_interp.push(list.head.l1.max(0) as u32); // LI
        s_tables.push(list.data.clone());
    }
    let coherent = CoherentElastic {
        bragg_energies_ev,
        temperatures_k,
        s_tables,
        temp_interp,
    };

    // LTHR=3 (mixed): the incoherent W'(T) TAB1 follows the coherent LISTs.
    let incoherent = if lthr == 3 {
        let inc_tab1 = cur.read_tab1()?;
        Some(read_incoherent_elastic(&inc_tab1))
    } else {
        None
    };

    Ok((Some(coherent), incoherent))
}

/// Build an [`IncoherentElastic`] from the `W'(T)` TAB1: `SB = C1`, the
/// interpolation law over the `(T, W')` table, and the table itself. `T₀` is
/// taken as the lowest tabulated temperature.
fn read_incoherent_elastic(tab1: &crate::endf::records::Tab1) -> IncoherentElastic {
    let sb = tab1.head.c1;
    let wp_of_t = tab1.pairs.clone();
    let temperature_k = wp_of_t.first().map(|&(t, _)| t).unwrap_or(0.0);
    IncoherentElastic {
        temperature_k,
        sb,
        interp: tab1.interp.clone(),
        wp_of_t,
    }
}

/// Parse MF=7/MT=4 incoherent inelastic S(α,β), selecting the tabulated
/// temperature nearest `target_k` \[K\] (`None` ⇒ base temperature `T₀`).
///
/// Record layout follows NJOY2016 `thermr.f90::calcem` (the reader at labels
/// 120–195): the `B(1..NI)` LIST, a TAB2 over `β`, then one `S(α)` TAB1 per `β`
/// (base `T₀`, interleaved `(α, S)`) each trailed by `LT` extra-temperature
/// LIST records (S-only, on the shared `α` grid), then the principal-scatterer
/// effective-temperature TAB1.
fn parse_inelastic(
    section: &crate::endf::tape::Section,
    target_k: Option<f64>,
) -> Result<IncoherentInelastic, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, 0, LAT, LASYM, 0
    let lat = head.l2;
    let lasym = head.n1;

    // B-constants LIST: B(1..NI); NS = number of non-principal atom types.
    let bl = cur.read_list()?;
    let b = bl.data.clone();
    if b.first().copied().unwrap_or(0.0) == 0.0 {
        // B(1)=0 ⇒ principal scatterer has no tabulated S (pure analytic /
        // short-collision-time principal, e.g. a free-gas-only evaluation). The
        // tabulated-S path below cannot serve it; that pure-analytic principal
        // path is a separate LEAPR-style model, not ported. Water (and every
        // standard tsl-* file) has B(1) > 0, so this branch is not hit for them.
        return Err(NjoyError::NotPorted(
            "MF=7 MT=4 with B(1)=0 (pure-analytic principal scatterer, no tabulated S)",
        ));
    }

    // TAB2 over β, then one TAB1 of S(α) per β (with LT extra-temperature LISTs).
    let tab2 = cur.read_tab2()?;
    let nb = tab2.head.n2;

    // The temperature set is shared across all β; resolve the requested
    // temperature once (from the first β's records), then apply the same
    // selection — tabulated index, or LI-law interpolation between two
    // bracketing tabulated tables — to every β (see the module docs).
    let mut selection = TemperatureSelection::Tabulated(0);
    let mut all_temps: Vec<f64> = Vec::new();
    let mut beta = Vec::with_capacity(nb as usize);
    let mut s_tables = Vec::with_capacity(nb as usize);

    for j in 0..nb {
        let tab1 = cur.read_tab1()?; // C1 = T0, C2 = β, L1 = LT
        let bval = tab1.head.c2;
        let lt = tab1.head.l1;
        let (alpha, base_s): (Vec<f64>, Vec<f64>) = tab1.pairs.iter().copied().unzip();

        // Candidate S(α) vectors for this β: base T₀ first, then the LT extras
        // ([T, β, LI, 0, NP, 0 / S(α_i, T)] — S only, on the shared α grid).
        let mut cand_temps = vec![tab1.head.c1];
        let mut cand_s = vec![base_s];
        let mut interval_li: Vec<u32> = Vec::with_capacity(lt as usize);
        for _ in 0..lt {
            let list = cur.read_list()?;
            if list.data.len() != alpha.len() {
                return Err(NjoyError::EndfParse(format!(
                    "S(α,β) extra-temperature LIST at {} K (β = {bval}) has {} \
                     S values, expected {} (the shared α grid)",
                    list.head.c1,
                    list.data.len(),
                    alpha.len()
                )));
            }
            cand_temps.push(list.head.c1);
            interval_li.push(list.head.l1.max(0) as u32); // LI
            cand_s.push(list.data.clone());
        }

        if j == 0 {
            all_temps = cand_temps.clone();
            selection = match target_k {
                None => TemperatureSelection::Tabulated(0),
                Some(t) => select_temperature(&cand_temps, &interval_li, t)?,
            };
        }
        let s: Vec<f64> = match selection {
            TemperatureSelection::Tabulated(i) => {
                cand_s.get(i).cloned().unwrap_or_else(|| cand_s[0].clone())
            }
            TemperatureSelection::Interpolated {
                lo,
                hi,
                li,
                target_k,
            } => cand_s[lo]
                .iter()
                .zip(cand_s[hi].iter())
                .map(|(&s_lo, &s_hi)| {
                    interp_s_temperature(cand_temps[lo], s_lo, cand_temps[hi], s_hi, target_k, li)
                })
                .collect(),
        };
        beta.push(bval);
        s_tables.push(AlphaTable {
            beta: bval,
            alpha,
            s,
        });
    }

    let temperature_k = selection.resolved_temperature_k(&all_temps);
    let tabulated_temperatures_k = all_temps;

    // Trailing principal-scatterer effective-temperature TAB1 (ENDF-6). Optional:
    // absent in some evaluations, so a read failure degrades to an empty table
    // (SCT then falls back to the physical temperature — see `inelastic`).
    let teff_table = cur.read_tab1().map(|t| t.pairs).unwrap_or_default();

    Ok(IncoherentInelastic {
        lat,
        lasym,
        b,
        temperature_k,
        beta,
        s_tables,
        tabulated_temperatures_k,
        teff_table,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::Tape;
    use std::fs::File;

    const AL_MAT: i32 = 53;

    fn al27() -> Mf7 {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/resources/tsl-013_Al_027-ENDF8.0.endf");
        let tape = Tape::read(File::open(p).unwrap()).unwrap();
        parse_mf7(&tape, AL_MAT).unwrap()
    }

    #[test]
    fn al27_has_both_elastic_and_inelastic() {
        let mf7 = al27();
        assert!((mf7.awr - 26.75).abs() < 0.1, "Al AWR ≈ 26.75");
        assert!(
            mf7.coherent_elastic.is_some(),
            "Al has coherent (Bragg) elastic"
        );
        assert!(mf7.incoherent_inelastic.is_some(), "Al has S(α,β)");
    }

    #[test]
    fn coherent_elastic_bragg_edges_are_monotone() {
        let ce = al27().coherent_elastic.unwrap();
        assert!(ce.temperatures_k[0] > 0.0, "has a base temperature");
        assert!(ce.bragg_energies_ev.len() > 50, "Al has many Bragg edges");
        // Bragg energies ascending; every temperature's cumulative S(E)
        // non-decreasing (it only steps up at each edge).
        assert!(
            ce.bragg_energies_ev.windows(2).all(|w| w[1] >= w[0]),
            "E ascending"
        );
        assert!(ce.temperatures_k.len() > 1, "Al ships extra temperatures");
        assert_eq!(
            ce.s_tables.len(),
            ce.temperatures_k.len(),
            "one S table per T"
        );
        assert_eq!(
            ce.temp_interp.len(),
            ce.temperatures_k.len() - 1,
            "one LI per interval"
        );
        assert!(
            ce.temperatures_k.windows(2).all(|w| w[1] > w[0]),
            "T ascending"
        );
        for s in &ce.s_tables {
            assert_eq!(
                s.len(),
                ce.bragg_energies_ev.len(),
                "S on the shared E grid"
            );
            assert!(
                s.windows(2).all(|w| w[1] >= w[0] - 1e-9),
                "S(E) non-decreasing"
            );
        }
    }

    #[test]
    fn inelastic_s_alpha_beta_grid_is_consistent() {
        let ii = al27().incoherent_inelastic.unwrap();
        assert!(ii.b.len() >= 6, "at least six B-constants");
        assert!(
            (ii.b[2] - 26.75).abs() < 0.1,
            "B(3) is the mass ratio A ≈ 26.75"
        );
        assert!(ii.beta.len() > 50, "a substantial β grid");
        assert_eq!(ii.beta.len(), ii.s_tables.len(), "one S(α) table per β");
        // β ascending; each S(α) table has matching α/S lengths and non-negative S.
        assert!(ii.beta.windows(2).all(|w| w[1] >= w[0]), "β ascending");
        for t in &ii.s_tables {
            assert_eq!(t.alpha.len(), t.s.len());
            assert!(t.alpha.windows(2).all(|w| w[1] >= w[0]), "α ascending");
            assert!(t.s.iter().all(|&s| s >= 0.0), "S(α,β) ≥ 0");
        }
    }

    // ── Incoherent-elastic parse paths ─────────────────────────────────────
    //
    // No checked-in TSL fixture exercises LTHR=2/3 (Al-27 is LTHR=1), so these
    // drive `parse_elastic` with hand-built MF=7/MT=2 sections. A row is the six
    // ENDF fields `[C1, C2, L1, L2, N1, N2]`; see `Cont::from_fields`.

    use crate::endf::tape::Section;
    use crate::endf::EndfKey;

    fn section(rows: Vec<[f64; 6]>) -> Section {
        Section {
            key: EndfKey {
                mat: 99,
                mf: 7,
                mt: 2,
            },
            rows,
        }
    }

    /// LTHR=2: incoherent elastic only — one `W'(T)` TAB1, `SB = C1`, no coherent.
    #[test]
    fn lthr2_incoherent_elastic_only() {
        // HEAD (LTHR=2), then TAB1[ SB, 0, 0, 0, NR=1, NT=3 ] with one interp
        // region (NBT=3, INT=2) and three (T, W') points.
        let sec = section(vec![
            [1.001, 0.9992, 2.0, 0.0, 0.0, 0.0], // HEAD: ZA, AWR, LTHR=2
            [3.4, 0.0, 0.0, 0.0, 1.0, 3.0],      // TAB1 head: SB=3.4, NR=1, NT=3
            [3.0, 2.0, 0.0, 0.0, 0.0, 0.0],      // interp: NBT=3, INT=2
            [296.0, 8.0e-3, 400.0, 1.0e-2, 600.0, 1.4e-2], // (T, W') pairs
        ]);
        let (coherent, incoherent) = parse_elastic(&sec).unwrap();
        assert!(coherent.is_none(), "LTHR=2 has no coherent part");
        let ie = incoherent.expect("LTHR=2 yields incoherent elastic");
        assert_eq!(ie.sb, 3.4, "SB = C1 of the W'(T) TAB1");
        assert_eq!(ie.temperature_k, 296.0, "T₀ = lowest tabulated temperature");
        assert_eq!(ie.interp, vec![(3, 2)]);
        assert_eq!(
            ie.wp_of_t,
            vec![(296.0, 8.0e-3), (400.0, 1.0e-2), (600.0, 1.4e-2)]
        );
        // W'(T) ascends with T (Debye-Waller integral grows with temperature).
        assert!(
            ie.wp_of_t.windows(2).all(|w| w[1].1 >= w[0].1),
            "W'(T) non-decreasing"
        );
    }

    /// LTHR=3: mixed — coherent `S(E)` TAB1 (with its `LT` extra-temperature
    /// LISTs) followed by the incoherent `W'(T)` TAB1.
    #[test]
    fn lthr3_mixed_coherent_and_incoherent() {
        let sec = section(vec![
            [1.001, 0.9992, 3.0, 0.0, 0.0, 0.0], // HEAD: LTHR=3
            // Coherent S(E) TAB1: T0=296, LT=1 (one extra temperature), NR=1, NP=2.
            [296.0, 0.0, 1.0, 0.0, 1.0, 2.0],     // TAB1 head
            [2.0, 1.0, 0.0, 0.0, 0.0, 0.0],       // interp: NBT=2, INT=1
            [1.0e-3, 5.0, 2.0e-3, 9.0, 0.0, 0.0], // (E, S) pairs
            // One extra-temperature LIST (C1 = 400 K, LI = 2, N1=2 S values).
            [400.0, 0.0, 2.0, 0.0, 2.0, 0.0], // LIST head
            [5.5, 9.5, 0.0, 0.0, 0.0, 0.0],   // LIST data
            // Incoherent W'(T) TAB1: SB=2.1, NR=1, NT=2.
            [2.1, 0.0, 0.0, 0.0, 1.0, 2.0],           // TAB1 head
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],           // interp
            [296.0, 7.0e-3, 400.0, 9.0e-3, 0.0, 0.0], // (T, W') pairs
        ]);
        let (coherent, incoherent) = parse_elastic(&sec).unwrap();
        let ce = coherent.expect("LTHR=3 yields coherent elastic");
        assert_eq!(
            ce.temperatures_k,
            vec![296.0, 400.0],
            "base + one extra temperature"
        );
        assert_eq!(ce.bragg_energies_ev, vec![1.0e-3, 2.0e-3]);
        assert_eq!(
            ce.s_tables,
            vec![vec![5.0, 9.0], vec![5.5, 9.5]],
            "both temperatures' S tables retained"
        );
        assert_eq!(ce.temp_interp, vec![2], "the LIST's LI code (lin-lin)");
        let ie = incoherent.expect("LTHR=3 yields incoherent elastic");
        assert_eq!(ie.sb, 2.1);
        assert_eq!(ie.wp_of_t, vec![(296.0, 7.0e-3), (400.0, 9.0e-3)]);
    }

    // ── Temperature-selection policy ───────────────────────────────────────

    /// Within the NJOY `T/1000 + 5` K tolerance → snap to the tabulated point.
    /// Methodology: grid {296, 400, 500} K, request 293.15 K (HTR-10 B1, 20 C);
    /// distance to 296 K is 2.85 K < tol(293.15) = 5.293 K. Expect Tabulated(0).
    #[test]
    fn select_temperature_snaps_within_njoy_tolerance() {
        let temps = [296.0, 400.0, 500.0];
        let sel = select_temperature(&temps, &[2, 2], 293.15).unwrap();
        assert_eq!(sel, TemperatureSelection::Tabulated(0));
        assert_eq!(sel.resolved_temperature_k(&temps), 296.0);
    }

    /// Between tabulated points beyond the tolerance → interpolate with the
    /// interval's LI law. Methodology: grid {296, 400, 500, 600} K, request
    /// 523.15 K (HTR-10 B4, 250 C); nearest is 500 K at 23.15 K > tol = 5.52 K,
    /// bracketed by (500, 600). Expect Interpolated{lo=2, hi=3, li=4}.
    #[test]
    fn select_temperature_interpolates_between_points() {
        let temps = [296.0, 400.0, 500.0, 600.0];
        let sel = select_temperature(&temps, &[2, 4, 4], 523.15).unwrap();
        assert_eq!(
            sel,
            TemperatureSelection::Interpolated {
                lo: 2,
                hi: 3,
                li: 4,
                target_k: 523.15
            }
        );
        assert_eq!(sel.resolved_temperature_k(&temps), 523.15);
    }

    /// Outside the tabulated range beyond the tolerance → a hard error, never a
    /// silent snap. Methodology: grid {296, 400} K, requests 100 K (below) and
    /// 2100 K (above). Expect TemperatureOutOfRange both ways.
    #[test]
    fn select_temperature_refuses_out_of_range() {
        let temps = [296.0, 400.0];
        for bad in [100.0, 2100.0] {
            match select_temperature(&temps, &[2], bad) {
                Err(NjoyError::TemperatureOutOfRange {
                    requested_k,
                    min_k,
                    max_k,
                }) => {
                    assert_eq!(requested_k, bad);
                    assert_eq!((min_k, max_k), (296.0, 400.0));
                }
                other => panic!("expected TemperatureOutOfRange for {bad} K, got {other:?}"),
            }
        }
    }
}
