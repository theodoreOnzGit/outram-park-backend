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
    /// For each `β`, the `S(α)` table at `T₀`.
    pub s_tables: Vec<AlphaTable>,
    /// Additional temperatures \[K\] present (their tables not yet retained).
    pub extra_temperatures_k: Vec<f64>,
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
    /// Thermal elastic (MT=2), if present.
    pub coherent_elastic: Option<CoherentElastic>,
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
/// - [`NjoyError::NotPorted`] for `LTHR=2` incoherent-elastic and for the
///   short-collision-time (`B(1)=0`) principal scatterer (no tabulated S).
/// - [`NjoyError::EndfParse`] on malformed records.
pub fn parse_mf7(tape: &Tape, mat: i32) -> Result<Mf7, NjoyError> {
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

    let coherent_elastic = elastic.map(parse_elastic).transpose()?.flatten();
    let incoherent_inelastic = inelastic.map(parse_inelastic).transpose()?;

    Ok(Mf7 { za, awr, coherent_elastic, incoherent_inelastic })
}

/// Parse MF=7/MT=2 thermal elastic. Returns `None` for `LTHR=2` (incoherent
/// elastic — not yet ported) rather than erroring, so a material that has both an
/// inelastic table and an incoherent-elastic table still parses.
fn parse_elastic(section: &crate::endf::tape::Section) -> Result<Option<CoherentElastic>, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, LTHR, 0, 0, 0
    let lthr = head.l1;
    if lthr != 1 {
        // LTHR=2 (incoherent elastic) or 3 (both) — deferred.
        return Ok(None);
    }

    // Base-temperature TAB1: T0 = C1, LT = L1 (extra temperatures), (E, S) pairs.
    let tab1 = cur.read_tab1()?;
    let temperature_k = tab1.head.c1;
    let lt = tab1.head.l1;
    let s_of_e = tab1.pairs.clone();

    // The LT extra temperatures follow as LIST records (S on the same E grid).
    let mut extra_temperatures_k = Vec::with_capacity(lt as usize);
    for _ in 0..lt {
        let list = cur.read_list()?;
        extra_temperatures_k.push(list.head.c1);
    }

    Ok(Some(CoherentElastic { temperature_k, s_of_e, extra_temperatures_k }))
}

/// Parse MF=7/MT=4 incoherent inelastic S(α,β) at the base temperature.
fn parse_inelastic(section: &crate::endf::tape::Section) -> Result<IncoherentInelastic, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, 0, LAT, LASYM, 0
    let lat = head.l2;
    let lasym = head.n1;

    // B-constants LIST: B(1..NI); NS = number of non-principal atom types.
    let bl = cur.read_list()?;
    let b = bl.data.clone();
    if b.first().copied().unwrap_or(0.0) == 0.0 {
        // B(1)=0 ⇒ principal scatterer has no tabulated S (short-collision-time
        // / free-gas analytic model) — a different THERMR path, deferred.
        return Err(NjoyError::NotPorted(
            "MF=7 MT=4 with B(1)=0 (analytic/SCT principal scatterer) — future",
        ));
    }

    // TAB2 over β, then one TAB1 of S(α) per β (with LT extra-temperature LISTs).
    let tab2 = cur.read_tab2()?;
    let nb = tab2.head.n2;

    let mut temperature_k = 0.0;
    let mut extra_temperatures_k: Vec<f64> = Vec::new();
    let mut beta = Vec::with_capacity(nb as usize);
    let mut s_tables = Vec::with_capacity(nb as usize);

    for j in 0..nb {
        let tab1 = cur.read_tab1()?; // C1 = T0, C2 = β, L1 = LT
        let bval = tab1.head.c2;
        let lt = tab1.head.l1;
        if j == 0 {
            temperature_k = tab1.head.c1;
        }
        let (alpha, s): (Vec<f64>, Vec<f64>) = tab1.pairs.iter().copied().unzip();
        beta.push(bval);
        s_tables.push(AlphaTable { beta: bval, alpha, s });

        // Extra-temperature LISTs for this β (S(α) at other temperatures).
        for _ in 0..lt {
            let list = cur.read_list()?;
            if j == 0 {
                extra_temperatures_k.push(list.head.c1);
            }
        }
    }

    Ok(IncoherentInelastic {
        lat,
        lasym,
        b,
        temperature_k,
        beta,
        s_tables,
        extra_temperatures_k,
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
}
