//! ENDF **MF=5** (secondary-energy distributions for MT reactions given as a
//! plain energy law, not coupled to angle) — `acelf5` (acefc.f90 6684–7137).
//!
//! Two entry points:
//! - [`parse_mf5_law4`] — the original narrow parser: exactly one subsection
//!   (`NK=1`), `LF=1` tabulated `g(E→E')`. Kept byte-for-byte as before (its
//!   tests pin this exact contract) for the fission-χ callers that already use
//!   it.
//! - [`parse_mf5_section`] — the general parser: any `NK`, dispatching each
//!   subsection's law (`LF=1/7/9/11`) and carrying its own applicability
//!   `p_k(E)` — the `acelf5` gap this crate previously refused via
//!   `NjoyError::NotPorted` at two separate checks (`NK≠1` and `LF≠1`).

use crate::endf::{records::SectionCursor, tape::Section};
use crate::NjoyError;
use super::madland_nix;

use super::core::{build_outgoing, collapse_interp, collapse_intt, Law4, EMEV};

/// Parse an MF=5 section with a single LF=1 subsection into an ACE [`Law4`].
///
/// This is the uncorrelated tabulated secondary-energy form — the fission
/// χ(E→E') of MT=18, and the prompt spectra of MT=455, etc. Faithful to the
/// LF=1 branch of `acelf5`.
///
/// # Limitations
/// Handles `NK = 1` subsection with `LF = 1` only, by design — for `NK > 1`
/// (probability-weighted mixtures) or the analytic laws (`LF=5/7/9/11`), use
/// [`parse_mf5_section`] instead.
///
/// # Errors
/// - [`NjoyError::NotPorted`] if `NK ≠ 1` or `LF ≠ 1`.
/// - [`NjoyError::EndfParse`] if the record structure is malformed.
pub fn parse_mf5_law4(section: &Section) -> Result<Law4, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, 0, 0, NK, 0
    let nk = head.n1;
    if nk != 1 {
        return Err(NjoyError::NotPorted(
            "MF=5 with multiple subsections (NK>1) — use parse_mf5_section",
        ));
    }

    // Subsection: TAB1 of p_k(E) (the law-applicability probability); its header
    // carries LF (= L2). For the fission χ this is p ≡ 1 across the range.
    let prob = cur.read_tab1()?;
    let lf = prob.head.l2;
    if lf != 1 {
        return Err(NjoyError::NotPorted(
            "MF=5 analytic spectra (LF=5/7/9/11) — use parse_mf5_section",
        ));
    }

    // LF=1 body: TAB2 over incident energies, each a TAB1 of g(E→E') vs E'.
    let tab2 = cur.read_tab2()?;
    let ne = tab2.head.n2;
    let e_in_interp = collapse_interp(&tab2.interp);

    let mut incident = Vec::with_capacity(ne as usize);
    for _ in 0..ne {
        let g = cur.read_tab1()?; // head.c2 = E_in [eV]; pairs = (E_out, f)
        let e_in_ev = g.head.c2;
        // Outgoing interpolation (clamp to histogram/lin-lin as ACE allows).
        let intt = collapse_intt(&g.interp);
        incident.push(build_outgoing(e_in_ev, intt, &g.pairs));
    }

    Ok(Law4 {
        e_in_interp,
        incident,
    })
}

/// One `p_k(E)` **applicability** table: the probability \[dimensionless\] that
/// subsection `k` of a multi-law MF=5 section applies, tabulated vs incident
/// energy \[MeV\]. This is the subsection's own leading TAB1 header + pairs —
/// the same record `acelf5` writes into the DLW law-validity header's `(NR,
/// NBT/INT, NE, E, P)` fields.
#[derive(Debug, Clone)]
pub struct Applicability {
    /// Interpolation regions over `e_mev` (empty ⇒ single lin-lin region).
    pub interp: Vec<(u32, u32)>,
    /// Incident energy grid \[MeV\], ascending.
    pub e_mev: Vec<f64>,
    /// Applicability probability at each `e_mev`, `0 ≤ p ≤ 1`.
    pub p: Vec<f64>,
}

/// A tabulated function `y(E)` \[MeV in\] with its own interpolation regions —
/// the `θ(E)` (LF=7/9) or `a(E)`/`b(E)` (LF=11) parameter tables.
#[derive(Debug, Clone)]
pub struct TabFn {
    /// Interpolation regions over `e_mev` (empty ⇒ single lin-lin region).
    pub interp: Vec<(u32, u32)>,
    /// Incident energy grid \[MeV\], ascending.
    pub e_mev: Vec<f64>,
    /// The tabulated value at each `e_mev`. Units depend on the parameter:
    /// `θ` and `a` are \[MeV\], `b` is \[1/MeV\] (see [`Mf5Law::Watt`]).
    pub y: Vec<f64>,
}

/// One MF=5 subsection's outgoing-energy law, dispatched on the ENDF `LF` (Law
/// Flag) header field. Faithful to the `if (lf.eq.…)` chain of `acelf5`.
#[derive(Debug, Clone)]
pub enum Mf5Law {
    /// `LF=1` — arbitrary tabulated `g(E→E')` (the fission χ and similar
    /// evaluated spectra). Maps to **ACE Law 4**; identical to
    /// [`parse_mf5_law4`]'s single-subsection output.
    Tabulated(Law4),
    /// `LF=7` (simple Maxwellian fission spectrum,
    /// `f(E→E') ∝ √E'·exp(−E'/θ(E))`) or `LF=9` (evaporation spectrum,
    /// `f(E→E') ∝ E'·exp(−E'/θ(E))`), both restricted to `0 ≤ E' ≤ E−U`.
    /// `ace_law` (`7` or `9`) says which; the data layout `acelf5` writes is
    /// identical for both (`θ(E)` table then `U`).
    Evaporation {
        /// The ACE law number this maps to (`7` or `9`, mirroring the ENDF LF).
        ace_law: u32,
        /// `U` \[MeV\] — the energy-independent restriction on the maximum
        /// outgoing energy (`E' ≤ E − U`).
        u_mev: f64,
        /// `θ(E)` \[MeV\] — the effective nuclear temperature vs incident
        /// energy.
        theta: TabFn,
    },
    /// `LF=11` — Watt fission spectrum,
    /// `f(E→E') ∝ sinh(√(b(E)·E'))·exp(−E'/a(E))`, `0 ≤ E' ≤ E−U`. Maps to
    /// **ACE Law 11**.
    Watt {
        /// `U` \[MeV\] — the energy-independent restriction on the maximum
        /// outgoing energy (`E' ≤ E − U`).
        u_mev: f64,
        /// `a(E)` \[MeV\] vs incident energy.
        a: TabFn,
        /// `b(E)` \[1/MeV\] vs incident energy.
        b: TabFn,
    },
}

/// One subsection of a (possibly multi-law) MF=5 section: its applicability
/// `p_k(E)` and its outgoing-energy law. `acelf5` chains `NK` of these via the
/// ACE DLW **LNW** locator, applying subsection `k` with probability `p_k(E)`
/// (the subsections' probabilities sum to 1 at every incident energy, per the
/// ENDF-6 File 5 convention — not re-checked here).
#[derive(Debug, Clone)]
pub struct Mf5Subsection {
    /// The probability this subsection's law applies, vs incident energy.
    pub applicability: Applicability,
    /// The outgoing-energy law itself.
    pub law: Mf5Law,
}

/// Convergence tolerance for the Madland-Nix adaptive linearisation
/// (`acefc.f90`'s `tol` for the `lf = 12` branch).
const MADLAND_NIX_TOL: f64 = 0.01;

fn read_tab_fn(cur: &mut SectionCursor<'_>) -> Result<TabFn, NjoyError> {
    let t = cur.read_tab1()?;
    Ok(TabFn {
        interp: collapse_interp(&t.interp),
        e_mev: t.pairs.iter().map(|&(e, _)| e / EMEV).collect(),
        y: t.pairs.iter().map(|&(_, y)| y).collect(),
    })
}

/// Parse a full MF=5 section into its `NK` subsections, each with its own
/// applicability table and outgoing-energy law. Faithful to the `do k=1,nk`
/// loop of `acelf5` (acefc.f90 6684–7137), including its per-`LF` dispatch.
///
/// Supersedes [`parse_mf5_law4`]'s `NK=1`/`LF=1` restriction: this handles any
/// `NK` (the probability-weighted mixture case) and `LF∈{1,7,9,11}`.
///
/// # What is intentionally still refused
/// - **`LF=5`** (generalized evaporation spectrum) — `acelf5` itself contains
///   `call error('acelf5','sorry. acer cannot handle lf=5.', 'you will have to
///   patch the evaluation to use lf=1.')` immediately on seeing `LF=5`, i.e.
///   upstream NJOY has **never** supported this law in ACER; every line of the
///   Fortran after that call is dead code (the `error` call aborts the run).
///   This is not a port gap, so it is not tracked as one — this crate mirrors
///   the same hard refusal, `NjoyError::NotPorted` in place of `error()`'s
///   process abort.
/// - **`LF=12`** (Madland-Nix fission spectrum) — genuinely unported; it needs
///   `acelf5`'s adaptive-linearization integral (`fmn`, the `ismax`/`jsmax`
///   stack) to convert the analytic Madland-Nix form to an ACE Law-4 table,
///   which is out of the LF=1/5/7/9/11 scope this pass was asked to close.
///
/// # Errors
/// [`NjoyError::NotPorted`] for `LF=5`, `LF=12`, or any `LF` outside
/// `{1,5,7,9,11,12}` (an illegal ENDF value); [`NjoyError::EndfParse`] on a
/// malformed record.
pub fn parse_mf5_section(section: &Section) -> Result<Vec<Mf5Subsection>, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, 0, 0, NK, 0
    let nk = head.n1.max(0);

    let mut out = Vec::with_capacity(nk as usize);
    for _ in 0..nk {
        let prob = cur.read_tab1()?;
        let lf = prob.head.l2;
        // `acelf5` reads `u=scr(1)` from this SAME (first) TAB1 of the
        // subsection regardless of `LF` — for LF=1 it is simply unused; for
        // LF=7/9/11/12 it is the restriction energy `U` (`E' ≤ E − U`).
        let u_ev = prob.head.c1;
        let applicability = Applicability {
            interp: collapse_interp(&prob.interp),
            e_mev: prob.pairs.iter().map(|&(e, _)| e / EMEV).collect(),
            p: prob.pairs.iter().map(|&(_, p)| p).collect(),
        };

        let law = match lf {
            1 => {
                let tab2 = cur.read_tab2()?;
                let ne = tab2.head.n2;
                let e_in_interp = collapse_interp(&tab2.interp);
                let mut incident = Vec::with_capacity(ne as usize);
                for _ in 0..ne {
                    let g = cur.read_tab1()?;
                    let e_in_ev = g.head.c2;
                    let intt = collapse_intt(&g.interp);
                    incident.push(build_outgoing(e_in_ev, intt, &g.pairs));
                }
                Mf5Law::Tabulated(Law4 {
                    e_in_interp,
                    incident,
                })
            }
            5 => {
                return Err(NjoyError::NotPorted(
                    "MF=5 LF=5 (generalized evaporation spectrum) — acelf5 itself refuses \
                     this (`call error('acelf5','sorry. acer cannot handle lf=5.',…)`); NJOY \
                     has never supported it, so this mirrors that hard stop rather than a \
                     port gap",
                ));
            }
            7 | 9 => {
                let theta = read_tab_fn(&mut cur)?;
                Mf5Law::Evaporation {
                    ace_law: lf as u32,
                    u_mev: u_ev / EMEV,
                    theta,
                }
            }
            11 => {
                let a = read_tab_fn(&mut cur)?;
                let b = read_tab_fn(&mut cur)?;
                Mf5Law::Watt {
                    u_mev: u_ev / EMEV,
                    a,
                    b,
                }
            }
            12 => {
                // `acefc.f90:7039-7056`: the TAB1 carries EFL and EFH in its
                // C1/C2, then `(E, T_m)` pairs over incident energy.
                let t = cur.read_tab1()?;
                let (efl_ev, efh_ev) = (t.head.c1, t.head.c2);
                if !(efl_ev > 0.0 && efh_ev > 0.0) {
                    return Err(NjoyError::EndfParse(format!(
                        "MF=5 LF=12: EFL and EFH must be positive, got {efl_ev} and {efh_ev}"
                    )));
                }
                // `emin`/`emax` are the ends of the tabulated incident grid
                // (`:7050-7051`); the outgoing grid spans the same range.
                let emin = t.pairs.first().map(|&(e, _)| e).unwrap_or(0.0);
                let emax = t.pairs.last().map(|&(e, _)| e).unwrap_or(0.0);

                let mut incident = Vec::with_capacity(t.pairs.len());
                for &(e_in_ev, tm) in &t.pairs {
                    let sp = madland_nix::linearise(
                        e_in_ev,
                        efl_ev,
                        efh_ev,
                        tm,
                        emin.max(1.0e-5),
                        emax,
                        MADLAND_NIX_TOL,
                    )?;
                    let pairs: Vec<(f64, f64)> = sp
                        .e_out_ev
                        .iter()
                        .zip(&sp.pdf)
                        .map(|(&x, &y)| (x, y))
                        .collect();
                    // `jnt = 2` (`:7107`): lin-lin in the outgoing energy.
                    incident.push(build_outgoing(e_in_ev, 2, &pairs));
                }
                // Upstream converts Madland-Nix to ACE LAW=4 rather than
                // giving it a law of its own (`:7057` "convert madland-nix to
                // ace law=4 using the given e grid").
                Mf5Law::Tabulated(Law4 {
                    e_in_interp: collapse_interp(&t.interp),
                    incident,
                })
            }
            _ => {
                return Err(NjoyError::EndfParse(format!(
                    "MF=5 subsection has an illegal LF={lf}"
                )));
            }
        };

        out.push(Mf5Subsection { applicability, law });
    }
    Ok(out)
}
