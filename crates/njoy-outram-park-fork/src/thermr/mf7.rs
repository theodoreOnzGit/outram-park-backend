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
//! This module parses those records into typed data at the **base temperature**
//! `T₀`; the additional temperatures (`LT` extra tables) are counted and their
//! temperatures captured, but their `S` values are not yet retained (the first
//! THERMR increment works at one temperature). Ported faithfully to the ENDF-102
//! MF=7 format, cross-checked against `thermr.f90`'s reader.
//!
//! Units: energies and `β` scale in eV (or in kT when `LAT=1`); `α`/`β` are
//! dimensionless; `S` is per the ENDF convention (per unit `α·β`).

use crate::endf::{records::SectionCursor, tape::Tape};
use crate::NjoyError;

/// Coherent-elastic (Bragg) scattering: the cumulative structure factor `S(E)`.
///
/// `S(E)` is a step function of incident energy — flat between Bragg edges and
/// jumping at each edge — so it is stored with histogram interpolation. The
/// elastic cross section is `σ(E) = S(E) / E`.
#[derive(Debug, Clone)]
pub struct CoherentElastic {
    /// Base temperature `T₀` \[K\].
    pub temperature_k: f64,
    /// `(E \[eV\], S(E) \[eV·b\])` Bragg-edge table, ascending in `E`.
    pub s_of_e: Vec<(f64, f64)>,
    /// Additional temperatures \[K\] present in the evaluation (their `S` tables
    /// are not yet retained — see the [module docs](self)).
    pub extra_temperatures_k: Vec<f64>,
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
    /// Base temperature `T₀` \[K\].
    pub temperature_k: f64,
    /// Energy-transfer grid `β` (dimensionless), one entry per [`s_tables`] row.
    ///
    /// [`s_tables`]: Self::s_tables
    pub beta: Vec<f64>,
    /// For each `β`, the `S(α)` table at the *selected* temperature
    /// [`temperature_k`](Self::temperature_k).
    pub s_tables: Vec<AlphaTable>,
    /// The other temperatures \[K\] present in the evaluation (their `S` tables
    /// are not retained — only the selected temperature's are).
    pub extra_temperatures_k: Vec<f64>,
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

/// Parse the MF=7 thermal scattering data for `mat`, selecting the incoherent-
/// inelastic `S(α,β)` tables at the temperature nearest `target_k` \[K\].
///
/// `tsl-*` evaluations tabulate `S(α,β)` at several temperatures (base `T₀`
/// plus extras). [`parse_mf7`] keeps only the base `T₀`; this variant keeps the
/// tables of whichever tabulated temperature is closest to `target_k`, so a
/// downstream generator at (say) 293.6 K uses the 293.6 K scattering law rather
/// than the base one. `target_k = None` reproduces [`parse_mf7`] (base `T₀`).
///
/// The selected temperature is reported in
/// [`IncoherentInelastic::temperature_k`]; the caller should read it back and
/// verify it is acceptably close to the request (the nearest tabulated point may
/// differ by a few K).
///
/// # Errors
/// Same as [`parse_mf7`].
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
    let incoherent_inelastic =
        inelastic.map(|sec| parse_inelastic(sec, target_k)).transpose()?;

    Ok(Mf7 { za, awr, coherent_elastic, incoherent_elastic, incoherent_inelastic })
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
    let s_of_e = tab1.pairs.clone();

    // The LT extra temperatures follow as LIST records (S on the same E grid).
    let mut extra_temperatures_k = Vec::with_capacity(lt as usize);
    for _ in 0..lt {
        let list = cur.read_list()?;
        extra_temperatures_k.push(list.head.c1);
    }
    let coherent = CoherentElastic { temperature_k, s_of_e, extra_temperatures_k };

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
    IncoherentElastic { temperature_k, sb, interp: tab1.interp.clone(), wp_of_t }
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

    // The temperature set is shared across all β; decide the selected index once
    // (from the first β), then pull that temperature's S(α) for every β.
    let mut selected_index: usize = 0;
    let mut all_temps: Vec<f64> = Vec::new();
    let mut beta = Vec::with_capacity(nb as usize);
    let mut s_tables = Vec::with_capacity(nb as usize);

    for j in 0..nb {
        let tab1 = cur.read_tab1()?; // C1 = T0, C2 = β, L1 = LT
        let bval = tab1.head.c2;
        let lt = tab1.head.l1;
        let (alpha, base_s): (Vec<f64>, Vec<f64>) = tab1.pairs.iter().copied().unzip();

        // Candidate S(α) vectors for this β: base T₀ first, then the LT extras.
        let mut cand_temps = vec![tab1.head.c1];
        let mut cand_s = vec![base_s];
        for _ in 0..lt {
            let list = cur.read_list()?;
            cand_temps.push(list.head.c1);
            cand_s.push(list.data.clone());
        }

        if j == 0 {
            all_temps = cand_temps.clone();
            selected_index = match target_k {
                None => 0,
                Some(t) => cand_temps
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, c)| {
                        (*a - t).abs().partial_cmp(&(*c - t).abs()).unwrap()
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0),
            };
        }
        let s = cand_s
            .get(selected_index)
            .cloned()
            .unwrap_or_else(|| cand_s[0].clone());
        beta.push(bval);
        s_tables.push(AlphaTable { beta: bval, alpha, s });
    }

    let temperature_k = all_temps.get(selected_index).copied().unwrap_or(0.0);
    let extra_temperatures_k: Vec<f64> = all_temps
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != selected_index)
        .map(|(_, &t)| t)
        .collect();

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
        extra_temperatures_k,
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
        assert!(mf7.coherent_elastic.is_some(), "Al has coherent (Bragg) elastic");
        assert!(mf7.incoherent_inelastic.is_some(), "Al has S(α,β)");
    }

    #[test]
    fn coherent_elastic_bragg_edges_are_monotone() {
        let ce = al27().coherent_elastic.unwrap();
        assert!(ce.temperature_k > 0.0, "has a base temperature");
        assert!(ce.s_of_e.len() > 50, "Al has many Bragg edges");
        // Bragg energies ascending; cumulative S(E) non-decreasing (it only steps
        // up at each edge).
        assert!(ce.s_of_e.windows(2).all(|w| w[1].0 >= w[0].0), "E ascending");
        assert!(ce.s_of_e.windows(2).all(|w| w[1].1 >= w[0].1 - 1e-9), "S(E) non-decreasing");
        assert!(!ce.extra_temperatures_k.is_empty(), "Al ships extra temperatures");
    }

    #[test]
    fn inelastic_s_alpha_beta_grid_is_consistent() {
        let ii = al27().incoherent_inelastic.unwrap();
        assert!(ii.b.len() >= 6, "at least six B-constants");
        assert!((ii.b[2] - 26.75).abs() < 0.1, "B(3) is the mass ratio A ≈ 26.75");
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
        Section { key: EndfKey { mat: 99, mf: 7, mt: 2 }, rows }
    }

    /// LTHR=2: incoherent elastic only — one `W'(T)` TAB1, `SB = C1`, no coherent.
    #[test]
    fn lthr2_incoherent_elastic_only() {
        // HEAD (LTHR=2), then TAB1[ SB, 0, 0, 0, NR=1, NT=3 ] with one interp
        // region (NBT=3, INT=2) and three (T, W') points.
        let sec = section(vec![
            [1.001, 0.9992, 2.0, 0.0, 0.0, 0.0],   // HEAD: ZA, AWR, LTHR=2
            [3.4, 0.0, 0.0, 0.0, 1.0, 3.0],        // TAB1 head: SB=3.4, NR=1, NT=3
            [3.0, 2.0, 0.0, 0.0, 0.0, 0.0],        // interp: NBT=3, INT=2
            [296.0, 8.0e-3, 400.0, 1.0e-2, 600.0, 1.4e-2], // (T, W') pairs
        ]);
        let (coherent, incoherent) = parse_elastic(&sec).unwrap();
        assert!(coherent.is_none(), "LTHR=2 has no coherent part");
        let ie = incoherent.expect("LTHR=2 yields incoherent elastic");
        assert_eq!(ie.sb, 3.4, "SB = C1 of the W'(T) TAB1");
        assert_eq!(ie.temperature_k, 296.0, "T₀ = lowest tabulated temperature");
        assert_eq!(ie.interp, vec![(3, 2)]);
        assert_eq!(ie.wp_of_t, vec![(296.0, 8.0e-3), (400.0, 1.0e-2), (600.0, 1.4e-2)]);
        // W'(T) ascends with T (Debye-Waller integral grows with temperature).
        assert!(ie.wp_of_t.windows(2).all(|w| w[1].1 >= w[0].1), "W'(T) non-decreasing");
    }

    /// LTHR=3: mixed — coherent `S(E)` TAB1 (with its `LT` extra-temperature
    /// LISTs) followed by the incoherent `W'(T)` TAB1.
    #[test]
    fn lthr3_mixed_coherent_and_incoherent() {
        let sec = section(vec![
            [1.001, 0.9992, 3.0, 0.0, 0.0, 0.0],   // HEAD: LTHR=3
            // Coherent S(E) TAB1: T0=296, LT=1 (one extra temperature), NR=1, NP=2.
            [296.0, 0.0, 1.0, 0.0, 1.0, 2.0],      // TAB1 head
            [2.0, 1.0, 0.0, 0.0, 0.0, 0.0],        // interp: NBT=2, INT=1
            [1.0e-3, 5.0, 2.0e-3, 9.0, 0.0, 0.0],  // (E, S) pairs
            // One extra-temperature LIST (C1 = 400 K, N1=2 S values).
            [400.0, 0.0, 0.0, 0.0, 2.0, 0.0],      // LIST head
            [5.5, 9.5, 0.0, 0.0, 0.0, 0.0],        // LIST data
            // Incoherent W'(T) TAB1: SB=2.1, NR=1, NT=2.
            [2.1, 0.0, 0.0, 0.0, 1.0, 2.0],        // TAB1 head
            [2.0, 2.0, 0.0, 0.0, 0.0, 0.0],        // interp
            [296.0, 7.0e-3, 400.0, 9.0e-3, 0.0, 0.0], // (T, W') pairs
        ]);
        let (coherent, incoherent) = parse_elastic(&sec).unwrap();
        let ce = coherent.expect("LTHR=3 yields coherent elastic");
        assert_eq!(ce.temperature_k, 296.0);
        assert_eq!(ce.s_of_e, vec![(1.0e-3, 5.0), (2.0e-3, 9.0)]);
        assert_eq!(ce.extra_temperatures_k, vec![400.0], "the LT extra-temperature LIST");
        let ie = incoherent.expect("LTHR=3 yields incoherent elastic");
        assert_eq!(ie.sb, 2.1);
        assert_eq!(ie.wp_of_t, vec![(296.0, 7.0e-3), (400.0, 9.0e-3)]);
    }
}
