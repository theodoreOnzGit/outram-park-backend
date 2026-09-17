//! ENDF **MF=6** (coupled energy-angle distributions) — `acelf6` (acefc.f90
//! 7139–7947) and its subsection-skipper `skip6` (endf.f90:1437-1476).
//!
//! An MF=6 section lists `NK` subsections, one per emitted product (`ZAP`):
//! the neutron (`ZAP=1`) plus, for many reactions, photons/recoils/charged
//! particles. `acelf6` itself only ever *extracts* the neutron's own
//! subsection(s) — for a neutron subsection it requires `LAW∈{1,6,7}`
//! (`error('acelf6','illegal law for endf6 file6 neutrons')` otherwise) — and
//! *skips* every other subsection via `skip6`, whose skip pattern depends on
//! that subsection's own `LAW` (any of 1–7).
//!
//! This module mirrors that split:
//! - [`skip_mf6_subsection`] — the `skip6` port, used to step over a
//!   subsection this parser is not extracting.
//! - [`parse_mf6_law1_neutrons`] / [`parse_mf6_law1_neutron`] — every leading
//!   **and non-leading** `ZAP=1, LAW=1` subsection (continuum tabulated
//!   energy, optionally with discrete lines).
//! - [`parse_mf6_law6_phase_space`] — the `ZAP=1, LAW=6` n-body phase-space
//!   subsection.
//! - [`parse_mf6_law7_lab_angle_energy`] — the `ZAP=1, LAW=7` lab-frame
//!   angle-then-energy subsection.

use crate::endf::{
    records::{SectionCursor, Tab1},
    tape::Section,
};
use crate::NjoyError;

use super::core::{
    build_outgoing, build_outgoing_with_discrete, collapse_interp, normalize_pdf_cdf_weighted,
    Law4, OutgoingEnergy, EMEV,
};

/// The ENDF **MF=6 LAW=1 angular representation** (`LANG`, the `L1` field of the
/// law's TAB2), retained so a transport code can sample the emission angle that
/// the evaluation actually correlates with `E'` instead of assuming isotropy.
///
/// `LANG` describes what the `NA` numbers following `f₀` on each
/// `[E'_out, f₀, f₁ … f_NA]` row *mean*; it does not vary within a subsection.
/// Reading it is what distinguishes "this evaluation says the emission is
/// isotropic" (`NA = 0` on every row) from "this port did not read the angular
/// data" — two situations that are indistinguishable once the coefficients are
/// dropped, and which were indistinguishable here until this type existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mf6AngularLaw {
    /// `LANG = 1` — Legendre coefficients `f₁ … f_NA` in the section's own frame
    /// (`LCT`), on the same scale as `f₀`. The normalised coefficients a Legendre
    /// density wants are `a_l = f_l / f₀`, with `a₀ ≡ 1`; see
    /// [`Mf6AngularRow::legendre_coefficients`].
    Legendre,
    /// `LANG = 2` — Kalbach-Mann systematics. `NA = 1` carries the pre-compound
    /// fraction `r`; `NA = 2` carries `r` and the slope `a`. These are
    /// dimensionless parameters and are **not** divided by `f₀`.
    KalbachMann,
    /// `LANG = 11 … 15` — a tabulated `(μ, f)` law; the `NA` numbers are
    /// `μ₁, f₁, μ₂, f₂ …` and the ENDF interpolation scheme is `LANG − 10`.
    Tabulated {
        /// ENDF interpolation law for the tabulated cosines (`LANG − 10`).
        interp: u32,
    },
    /// Any other `LANG`, kept verbatim so a consumer can refuse it explicitly
    /// rather than mistake an unread law for a genuinely isotropic one.
    Other(i32),
}

impl Mf6AngularLaw {
    /// Decode the raw ENDF `LANG` value.
    pub fn from_lang(lang: i32) -> Self {
        match lang {
            1 => Mf6AngularLaw::Legendre,
            2 => Mf6AngularLaw::KalbachMann,
            11..=15 => Mf6AngularLaw::Tabulated {
                interp: (lang - 10) as u32,
            },
            other => Mf6AngularLaw::Other(other),
        }
    }
}

/// The angular coefficients of one incident-energy LIST record, aligned
/// row-for-row with the matching [`OutgoingEnergy`] table.
///
/// Row `i` of this table describes the emission angle **conditional on** the
/// outgoing energy `e_out_mev[i]` of the same index in that table — MF=6 LAW=1
/// is a correlated energy-angle law, which is the whole reason the coefficients
/// cannot simply be averaged away.
#[derive(Debug, Clone)]
pub struct Mf6AngularTable {
    /// Incident neutron energy \[MeV\] — matches [`OutgoingEnergy::e_in_mev`].
    pub e_in_mev: f64,
    /// Number of angular numbers per outgoing-energy row (ENDF `NA`). `0` means
    /// the evaluation itself declares this incident energy isotropic.
    pub na: u32,
    /// The energy density `f₀` of each outgoing-energy row, **as it appears on
    /// the tape** (not renormalised the way [`OutgoingEnergy::pdf`] is). Kept
    /// because `LANG = 1`'s Legendre coefficients are on `f₀`'s scale and must be
    /// divided by *this* `f₀`, not by the normalised pdf.
    pub f0: Vec<f64>,
    /// The `na` angular numbers of every row, row-major: row `i` occupies
    /// `coeffs[i*na .. (i+1)*na]`. Empty when `na == 0`.
    pub coeffs: Vec<f64>,
}

impl Mf6AngularTable {
    /// Number of outgoing-energy rows.
    pub fn len(&self) -> usize {
        self.f0.len()
    }

    /// Whether this table has no rows at all.
    pub fn is_empty(&self) -> bool {
        self.f0.is_empty()
    }

    /// The raw angular numbers of row `i`, or an empty slice when `na == 0` or
    /// `i` is out of range.
    pub fn row(&self, i: usize) -> &[f64] {
        let na = self.na as usize;
        if na == 0 || i >= self.len() {
            return &[];
        }
        &self.coeffs[i * na..(i + 1) * na]
    }

    /// The **normalised** Legendre coefficients `a_l = f_l / f₀` of row `i`, the
    /// form [`crate::acer::angular`]'s Legendre density expects (`a₀ ≡ 1`,
    /// `f(μ) = Σ_l ((2l+1)/2)·a_l·P_l(μ)`).
    ///
    /// Only meaningful when the subsection's law is
    /// [`Mf6AngularLaw::Legendre`] — the caller must check. Returns an empty
    /// vector when the row is isotropic (`na == 0`) or carries no density
    /// (`f₀ ≤ 0`, where the conditional angular law is unreachable and any value
    /// would do).
    pub fn legendre_coefficients(&self, i: usize) -> Vec<f64> {
        let row = self.row(i);
        if row.is_empty() {
            return Vec::new();
        }
        let f0 = self.f0.get(i).copied().unwrap_or(0.0);
        if f0 <= 0.0 {
            return Vec::new();
        }
        row.iter().map(|&f| f / f0).collect()
    }

    /// The mean cosine `⟨μ⟩` of row `i` under a Legendre law, which is exactly
    /// `a₁ = f₁/f₀` — every higher term integrates to zero against `μ`.
    ///
    /// Returns `0.0` for an isotropic row. As with
    /// [`Self::legendre_coefficients`], this assumes
    /// [`Mf6AngularLaw::Legendre`].
    pub fn legendre_mubar(&self, i: usize) -> f64 {
        let row = self.row(i);
        if row.is_empty() {
            return 0.0;
        }
        let f0 = self.f0.get(i).copied().unwrap_or(0.0);
        if f0 <= 0.0 {
            0.0
        } else {
            row[0] / f0
        }
    }
}

/// The neutron emission of an MF=6 LAW=1 reaction, reduced to an ACE Law 4
/// energy distribution.
///
/// Built by [`parse_mf6_law1_neutron`]. Carries the multiplicity (yield) and
/// reference frame so the caller can fill the ACE TYR entry, plus the energy
/// distribution [`law4`](Self::law4).
///
/// **The angular half of the law is carried too**, in [`lang`](Self::lang) and
/// [`angular`](Self::angular), and is *not* folded into `law4`: ACE Law 4 is an
/// energy-only law by definition, so writing an ACE file still uses `law4` alone
/// and its serialisation is unchanged. A transport code that wants the
/// correlated emission angle reads `angular` instead of assuming isotropy — the
/// assumption this port made until the coefficients were retained (bead
/// `op-og56`).
#[derive(Debug, Clone)]
pub struct Mf6Neutron {
    /// Reference frame of the distribution: `1` = laboratory, `2` = centre-of-mass
    /// (LCT from the MF=6 HEAD). Determines the sign of the ACE TYR entry.
    pub lct: i32,
    /// Neutron multiplicity (yield) vs incident energy `(E [eV], y)` — the
    /// subsection's TAB1. A constant `y` (e.g. 2 for (n,2n)) gives `TYR = ±y`.
    pub yield_pairs: Vec<(f64, f64)>,
    /// The outgoing-energy distribution as an ACE Law 4.
    pub law4: Law4,
    /// What the angular numbers on each row mean (ENDF `LANG`).
    pub lang: Mf6AngularLaw,
    /// The angular coefficients, one table per incident energy, in the **same
    /// order and of the same length** as `law4.incident`.
    pub angular: Vec<Mf6AngularTable>,
    /// The target's `ZA = 1000·Z + A`, from the MF=6 HEAD record.
    ///
    /// Carried because the Kalbach-Mann slope systematics need it: an
    /// evaluation storing only `r` (`NA = 1`) leaves `a` to be computed from
    /// the projectile/ejectile/target masses by
    /// [`crate::groupr::kinematics::bach`], which is a function of the nuclide
    /// and not of the emission law alone.
    pub za_target: i32,
}

impl Mf6Neutron {
    /// Whether this emission is isotropic *according to the evaluation* — i.e.
    /// every incident-energy table declares `NA = 0`.
    ///
    /// This is the distinction that matters when auditing the port: a `true`
    /// here means the tape itself carries no angular structure, whereas an
    /// absent [`angular`](Self::angular) would only mean nobody read it.
    pub fn is_angular_isotropic(&self) -> bool {
        self.angular.iter().all(|t| t.na == 0)
    }

    /// The largest `|⟨μ⟩|` any outgoing-energy row of any incident energy
    /// carries, under a Legendre (`LANG = 1`) reading. `0.0` for a law that is
    /// isotropic or not Legendre.
    ///
    /// Intended as a cheap "is there anything here worth sampling" probe and as
    /// the assertion an ablation control needs: a control that switches off an
    /// angular law which was flat to begin with reports "no difference" and
    /// reads as "this physics does not matter".
    pub fn peak_legendre_mubar(&self) -> f64 {
        if self.lang != Mf6AngularLaw::Legendre {
            return 0.0;
        }
        self.angular
            .iter()
            .flat_map(|t| (0..t.len()).map(move |i| t.legendre_mubar(i).abs()))
            .fold(0.0, f64::max)
    }
}

impl Mf6Neutron {
    /// Mean outgoing-neutron energy `⟨E'⟩` \[eV\] at incident energy `e_in` \[eV\]
    /// — the first moment `∫ E'·f₀(E') dE'` of the emission pdf, linearly
    /// interpolated between the two bracketing incident-energy tables.
    ///
    /// This ports `heatr.f90::getsix`'s lab-frame LAW=1 `ebar` (its label 400:
    /// `ebar = ∫ E'·f₀ dE'`, trapezoid for LEP≥2, histogram for LEP=1) — and,
    /// where a table carries leading discrete lines (`ND>0`, see
    /// [`OutgoingEnergy::nd`]), each line contributes its probability weight
    /// times its energy directly, with the continuum tail integrated from the
    /// first continuum point onward (matching `build_outgoing_with_discrete`'s
    /// zero-width discrete→continuum boundary). It is exact when the MF=6 data
    /// is in the **laboratory** frame (`lct == 1`); when the data is in the
    /// **centre-of-mass** frame (`lct == 2`) this returns the CM-frame mean, a
    /// (small, bounded) approximation to the lab-frame mean — the full CM→lab
    /// angle transform (`getsix`'s `h6cm` path) is deferred. Used by HEATR H5
    /// ([`crate::heatr`]) for the `ȳ·⟨E'⟩` escaping-neutron term.
    pub fn mean_energy(&self, e_in: f64) -> f64 {
        let tables = &self.law4.incident;
        if tables.is_empty() {
            return 0.0;
        }
        // First moment of one incident-energy table, in MeV (e_out/pdf are MeV).
        let table_mean = |t: &OutgoingEnergy| -> f64 {
            let (eo, pdf) = (&t.e_out_mev, &t.pdf);
            let nd = (t.nd() as usize).min(eo.len());
            let lep = t.lep();
            let mut m = 0.0;
            // Discrete lines: plain probability weight × energy.
            for i in 0..nd {
                m += pdf[i] * eo[i];
            }
            // Continuum tail: no contribution across the discrete→continuum
            // boundary itself (zero width there — see build_outgoing_with_discrete).
            for i in (nd + 1)..eo.len() {
                let (x0, x1) = (eo[i - 1], eo[i]);
                if lep == 1 {
                    // histogram: pdf constant at pdf[i-1] over [x0, x1)
                    m += pdf[i - 1] * (x1 * x1 - x0 * x0) / 2.0;
                } else {
                    // lin-lin: trapezoid of E'·pdf
                    m += (x1 - x0) * (x1 * pdf[i] + x0 * pdf[i - 1]) / 2.0;
                }
            }
            m
        };
        let e_in_mev = e_in / EMEV;
        // Below/above the tabulated incident range: clamp to the end table.
        if e_in_mev <= tables[0].e_in_mev {
            return table_mean(&tables[0]) * EMEV;
        }
        for i in 1..tables.len() {
            let (e0, e1) = (tables[i - 1].e_in_mev, tables[i].e_in_mev);
            if e_in_mev <= e1 {
                let (m0, m1) = (table_mean(&tables[i - 1]), table_mean(&tables[i]));
                let frac = if e1 > e0 {
                    (e_in_mev - e0) / (e1 - e0)
                } else {
                    0.0
                };
                return (m0 + frac * (m1 - m0)) * EMEV;
            }
        }
        table_mean(tables.last().unwrap()) * EMEV
    }
}

/// Skip the body of one MF=6 subsection this parser is **not** extracting, so a
/// later matching subsection can still be reached.
///
/// Faithful port of **`skip6`** (endf.f90:1437-1476), *not* `skip6a`.
///
/// # Which upstream routine, and why it matters
///
/// NJOY has two: `skip6` walks a **raw ENDF-6 tape**, and `skip6a`
/// (acefc.f90:7949) walks ACER's own internal File-6 variant. Its header says so
/// outright — *"Special version of skip6 for special version of File 6 used in
/// ACER. Law=7 has a TAB1 containing the angular distribution instead of the
/// normal TAB2 for each incident energy."* This crate reads raw tapes, so
/// `skip6` is the correct routine, and the difference is not cosmetic: under
/// `skip6a`'s LAW=7 stride the cursor lands mid-record and every subsequent
/// subsection header is garbage. ENDF/B-VIII.0's Be-9 MF=6/MT=16 is LAW=7 and
/// exercises exactly this.
///
/// The subsection's own yield TAB1 (`ZAP, AWP, LIP, LAW, NR, NP; E, y`) must
/// already have been consumed by the caller (via
/// [`SectionCursor::read_tab1`]) — this advances `cur` past whatever body
/// follows it, which depends entirely on `law`:
/// - `LAW=1,2,5` — one TAB2(`NE`) then `NE` LIST records (energy-angle tables
///   for LAW=1, discrete two-body Legendre for LAW=2, charged-particle elastic
///   for LAW=5).
/// - `LAW=6` — one CONT record (`APSX, NPSX`; n-body phase space header).
/// - `LAW=7` — one TAB2(`NE`) then, per incident energy, **one TAB2** whose
///   `N2` is `NMU`, followed by `NMU` TAB1s (the per-cosine energy spectra).
/// - `LAW=3,4` (discrete two-body / discrete two-body recoil) — **no body**:
///   the yield TAB1 is the entire subsection (the kinematics live in MF=4/14),
///   matching `skip6`'s silent no-op for these two values (and for any other
///   value — `skip6` has no `else` branch either).
///
/// # Errors
/// [`NjoyError::EndfParse`] if the section runs out of rows while skipping.
pub(super) fn skip_mf6_subsection(cur: &mut SectionCursor<'_>, law: i32) -> Result<(), NjoyError> {
    match law {
        6 => {
            cur.read_cont()?;
        }
        1 | 2 | 5 => {
            let tab2 = cur.read_tab2()?;
            let ne = tab2.head.n2.max(0);
            for _ in 0..ne {
                cur.read_list()?;
            }
        }
        7 => {
            let tab2 = cur.read_tab2()?;
            let ne = tab2.head.n2.max(0);
            for _ in 0..ne {
                // TAB2, not TAB1 — see the note above on `skip6` vs `skip6a`.
                // `NMU` is the record's `N2`, exactly as `skip6` reads `n2h`.
                let mu_tab = cur.read_tab2()?;
                let nmu = mu_tab.head.n2.max(0);
                for _ in 0..nmu {
                    cur.read_tab1()?;
                }
            }
        }
        _ => {
            // LAW=3,4 (and anything skip6 itself does not recognise): no body.
        }
    }
    Ok(())
}

/// Parse the **neutron** product of an MF=6 LAW=1 section into an ACE Law 4
/// energy distribution.
///
/// MF=6 LAW=1 stores, per incident energy, a LIST of `[E'_out, f₀, f₁ … f_NA]`
/// rows (`NA` angular coefficients after the energy pdf `f₀`). This extracts the
/// energy pdf `f₀` — faithful to `acelf6` for the isotropic (`NA=0`) case, and
/// the energy-only reduction of the anisotropic case. The neutron product is the
/// **first** `ZAP=1, LAW=1` subsection found (see [`parse_mf6_law1_neutrons`]
/// for every one of them).
///
/// # Errors
/// [`NjoyError::NotPorted`] if the section carries no `ZAP=1, LAW=1` neutron
/// subsection; [`NjoyError::EndfParse`] on malformed records.
pub fn parse_mf6_law1_neutron(section: &Section) -> Result<Mf6Neutron, NjoyError> {
    let mut all = parse_mf6_law1_neutrons(section)?;
    Ok(all.remove(0))
}

/// Parse **every** neutron (`ZAP=1`, `LAW=1`) subsection of an MF=6 section, in
/// file order — wherever they appear among the section's `NK` subsections.
///
/// Some evaluations give (n,2n) as one neutron subsection with yield 2 (U-238,
/// U-235 in ENDF/B-VIII.0); others split it into **two** ZAP=1 subsections of
/// yield 1 each, carrying different spectra for the first and second emitted
/// neutron (F-19 in the same library, MF=6/MT=16, NK=4). A reader that takes
/// only the first subsection silently emits one neutron where the evaluation
/// says two, so the multiplicity must come from the sum over subsections.
///
/// Every subsection is visited via [`SectionCursor::read_tab1`] for its yield
/// header; a subsection that is not `(ZAP=1, LAW=1)` is stepped over with
/// [`skip_mf6_subsection`] (the `skip6` port) and scanning continues — so a
/// neutron subsection placed *after* a photon/recoil one is still found. A
/// `ZAP=1` subsection whose law is neither `1`, `6`, nor `7` is an ENDF error
/// `acelf6` itself refuses (`error('acelf6','illegal law for endf6 file6
/// neutrons')`); this mirrors that refusal as [`NjoyError::NotPorted`] rather
/// than skipping it silently. `LAW=6`/`LAW=7` neutron subsections are legal but
/// are a different shape ([`parse_mf6_law6_phase_space`] /
/// [`parse_mf6_law7_lab_angle_energy`]) and are stepped over here.
///
/// This port does not replicate `acelf6`'s `JP`-based early-termination
/// heuristic (`jpn==2`, or `jpn==1` past the first subsection) — it simply
/// scans every one of the `NK` subsections and keeps whatever matches, which is
/// behaviourally equivalent for every evaluation this crate reads (none relies
/// on `JP` to disambiguate two products sharing a ZAP) and simpler to reason
/// about.
///
/// # Errors
/// [`NjoyError::NotPorted`] if no `ZAP=1, LAW=1` subsection is found (naming
/// whether a `LAW=6/7` neutron subsection was seen instead), or if a `ZAP=1`
/// subsection has an illegal law; [`NjoyError::EndfParse`] on malformed
/// records.
pub fn parse_mf6_law1_neutrons(section: &Section) -> Result<Vec<Mf6Neutron>, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, JP, LCT, NK, 0
    let lct = head.l2;
    let nk = head.n1.max(0);
    // The material ZA, needed by the Kalbach-Mann slope systematics when an
    // evaluation stores only `r`.
    let za_target = head.c1.round() as i32;

    let mut out: Vec<Mf6Neutron> = Vec::new();
    let mut saw_other_law: Option<i32> = None;
    for _ in 0..nk {
        let ymult = match cur.read_tab1() {
            Ok(t) => t,
            Err(e) => {
                if out.is_empty() {
                    return Err(e);
                }
                break; // a trailing subsection we cannot read is not fatal
            }
        };
        let zap = ymult.head.c1.round() as i32;
        let law = ymult.head.l2;

        if zap != 1 {
            if skip_mf6_subsection(&mut cur, law).is_err() {
                if out.is_empty() {
                    return Err(NjoyError::EndfParse(
                        "MF=6: could not skip a non-neutron subsection".into(),
                    ));
                }
                break;
            }
            continue;
        }
        if law == 6 || law == 7 {
            saw_other_law.get_or_insert(law);
            if skip_mf6_subsection(&mut cur, law).is_err() {
                if out.is_empty() {
                    return Err(NjoyError::EndfParse(
                        "MF=6: could not skip a LAW=6/7 neutron subsection".into(),
                    ));
                }
                break;
            }
            continue;
        }
        if law != 1 {
            return Err(NjoyError::NotPorted(
                "MF=6 ZAP=1 (neutron) subsection has an illegal LAW (only 1, 6, 7 are legal \
                 for a neutron per acelf6's own check: `error('acelf6','illegal law for \
                 endf6 file6 neutrons')`)",
            ));
        }

        match parse_law1_neutron_body(&mut cur, lct, za_target, ymult) {
            Ok(n) => out.push(n),
            Err(e) => {
                if out.is_empty() {
                    return Err(e);
                }
                break;
            }
        }
    }
    if out.is_empty() {
        return Err(NjoyError::NotPorted(if saw_other_law.is_some() {
            "MF=6 section's neutron (ZAP=1) subsection uses LAW=6 or LAW=7, not LAW=1 — use \
             parse_mf6_law6_phase_space / parse_mf6_law7_lab_angle_energy instead"
        } else {
            "MF=6 section carries no ZAP=1 LAW=1 neutron subsection"
        }));
    }
    Ok(out)
}

/// The LAW=1 body of one neutron subsection: TAB2 (LANG, LEP, NE) then one LIST
/// per incident energy. Split out of [`parse_mf6_law1_neutrons`] so both the
/// single- and multi-subsection entry points share one parser.
fn parse_law1_neutron_body(
    cur: &mut SectionCursor<'_>,
    lct: i32,
    za_target: i32,
    ymult: Tab1,
) -> Result<Mf6Neutron, NjoyError> {
    let tab2 = cur.read_tab2()?;
    let lang = tab2.head.l1; // angular representation, shared by every row
    let lep = tab2.head.l2; // secondary-energy interpolation
    let ne = tab2.head.n2;
    let intt = if lep >= 2 { 2 } else { 1 };
    let e_in_interp = collapse_interp(&tab2.interp);

    let mut incident = Vec::with_capacity(ne as usize);
    let mut angular = Vec::with_capacity(ne as usize);
    for _ in 0..ne {
        let list = cur.read_list()?;
        let e_in_ev = list.head.c2;
        let nd = list.head.l1.max(0) as usize; // number of discrete lines
        let na = list.head.l2.max(0); // number of angular coefficients per E_out
        let nep = list.head.n2 as usize; // number of secondary-energy points
                                         // Each row is [E'_out, f0, f1 … f_NA]; stride = NA + 2. The energy law
                                         // takes (E', f0); the `na` numbers after f0 are the angular half and
                                         // are kept in `angular` rather than discarded (bead `op-og56`).
        let stride = (na + 2) as usize;
        let na_usize = na as usize;
        let mut pairs: Vec<(f64, f64)> = Vec::with_capacity(nep);
        let mut f0_raw: Vec<f64> = Vec::with_capacity(nep);
        let mut coeffs: Vec<f64> = Vec::with_capacity(nep * na_usize);
        for r in 0..nep {
            let base = r * stride;
            let e_out = list.data[base];
            let f0 = list.data[base + 1];
            pairs.push((e_out, f0));
            f0_raw.push(f0);
            // `f₀` is retained unnormalised beside the coefficients: LANG=1's
            // `f_l` are on `f₀`'s own scale, and `build_outgoing` renormalises
            // the pdf it builds, so dividing by the stored pdf would be wrong.
            coeffs.extend_from_slice(&list.data[base + 2..base + 2 + na_usize]);
        }
        angular.push(Mf6AngularTable {
            e_in_mev: e_in_ev / EMEV,
            na: na as u32,
            f0: f0_raw,
            coeffs,
        });
        incident.push(if nd == 0 {
            build_outgoing(e_in_ev, intt, &pairs)
        } else {
            // ND>0: the first `nd` rows are discrete lines, not a continuum
            // density — acefc.f90's ki<=nd / ki>nd split (~7674–7702).
            build_outgoing_with_discrete(e_in_ev, lep, nd, &pairs)
        });
    }

    Ok(Mf6Neutron {
        lct,
        yield_pairs: ymult.pairs,
        law4: Law4 {
            e_in_interp,
            incident,
        },
        lang: Mf6AngularLaw::from_lang(lang),
        angular,
        za_target,
    })
}

/// The neutron emission of an MF=6 `LAW=6` (n-body phase space) subsection.
///
/// ENDF-6 §6.2.7's phase-space law: the outgoing-energy shape is a *universal*
/// function of `x = E'/E'_max(E)` depending only on `npsx` (the number of
/// particles sharing the phase space) — it does not vary with incident energy
/// beyond that scaling, so (unlike [`Mf6Neutron::law4`]) there is a single
/// `(x, pdf, cdf)` table, not one per incident energy. `apsx` (the total mass
/// ratio of the particles sharing the phase space, in neutron masses) and
/// `E'_max(E)` itself are the transport code's inputs for turning `x` back into
/// a physical outgoing energy; this port does not compute `E'_max` (that needs
/// the reaction Q-value and the full particle mass list, outside this
/// section), so it is left as `npsx`/`apsx` for the caller.
#[derive(Debug, Clone)]
pub struct Mf6PhaseSpace {
    /// Reference frame (`1` lab, `2` CM) from the MF=6 HEAD.
    pub lct: i32,
    /// Neutron multiplicity (yield) vs incident energy `(E [eV], y)`.
    pub yield_pairs: Vec<(f64, f64)>,
    /// Number of particles distributed via phase-space theory (ENDF `NPSX`).
    pub npsx: i32,
    /// Total mass of the particles sharing the phase space, in neutron masses
    /// (ENDF `AP`).
    pub apsx: f64,
    /// `x = E'/E'_max` grid, ascending on `[0, 1]`.
    pub x_frac: Vec<f64>,
    /// Probability density on `x_frac`, normalised so `∫pdf dx = 1`.
    pub pdf: Vec<f64>,
    /// Cumulative distribution: `cdf[0] = 0`, `cdf[last] = 1`.
    pub cdf: Vec<f64>,
}

/// Build the universal phase-space shape table `(x, pdf, cdf)` for `npsx`
/// particles — ENDF-6 formula 6.21, `pdf(x) = √x·(1−x)^(3·npsx/2 − 4)`.
/// Faithful, deterministic port of `acelf6`'s adaptive-step grid (acefc.f90
/// ~7776–7815): a geometric step (`×10^0.2`) below `x=0.1`, then an additive
/// step of `0.02` up to `x=1`, integrated by the trapezoid rule and
/// renormalised to `∫pdf dx = 1`.
fn law66_shape_table(npsx: i32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let step1 = 10f64.powf(1.0 / 5.0);
    let step2 = 1.0 / 50.0;
    let elow = 1.0e-5;
    let test1 = 1.0 + 1.0 / 100_000.0;
    let test3 = 1.0 - 1.0 / 10_000.0;
    let step_threshold = 0.1 - 1.0 / 10_000.0;

    let mut x = vec![0.0f64];
    let mut pdf = vec![0.0f64];
    let mut cdf = vec![0.0f64];
    let (mut xl, mut pl, mut yn) = (0.0f64, 0.0f64, 0.0f64);
    let mut xx: f64 = elow;
    while xx < test1 {
        let pn = if xx > test3 {
            xx = 1.0;
            0.0
        } else {
            let rn = 3.0;
            xx.sqrt() * (1.0 - xx).powf(rn * npsx as f64 / 2.0 - 4.0)
        };
        yn += (xx - xl) * (pn + pl) / 2.0;
        x.push(xx);
        pdf.push(pn);
        cdf.push(yn);
        xl = xx;
        pl = pn;
        if xx < step_threshold {
            xx *= step1;
        } else {
            xx += step2;
        }
    }
    let sum = *cdf.last().unwrap();
    if sum > 0.0 {
        for p in &mut pdf {
            *p /= sum;
        }
        for c in &mut cdf {
            *c /= sum;
        }
    }
    (x, pdf, cdf)
}

/// Parse the **neutron** (`ZAP=1`) `LAW=6` (n-body phase space) subsection of an
/// MF=6 section. Scans every subsection in file order, skipping non-matching
/// ones with [`skip_mf6_subsection`] — including `ZAP=1` subsections of a
/// *different* law — exactly as [`parse_mf6_law1_neutrons`] does.
///
/// # Errors
/// [`NjoyError::NotPorted`] if the section carries no `ZAP=1, LAW=6`
/// subsection; [`NjoyError::EndfParse`] on malformed records.
pub fn parse_mf6_law6_phase_space(section: &Section) -> Result<Mf6PhaseSpace, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?;
    let lct = head.l2;
    let nk = head.n1.max(0);

    for _ in 0..nk {
        let ymult = cur.read_tab1()?;
        let zap = ymult.head.c1.round() as i32;
        let law = ymult.head.l2;
        if zap == 1 && law == 6 {
            let cont = cur.read_cont()?; // APSX (C1), NPSX (N2)
            let apsx = cont.c1;
            let npsx = cont.n2;
            let (x_frac, pdf, cdf) = law66_shape_table(npsx);
            return Ok(Mf6PhaseSpace {
                lct,
                yield_pairs: ymult.pairs,
                npsx,
                apsx,
                x_frac,
                pdf,
                cdf,
            });
        }
        skip_mf6_subsection(&mut cur, law)?;
    }
    Err(NjoyError::NotPorted(
        "MF=6 section carries no ZAP=1 LAW=6 (n-body phase space) neutron subsection",
    ))
}

/// One outgoing-energy table at a fixed lab cosine `mu`, within one incident
/// energy of a [`Mf6LabAngleEnergy`].
#[derive(Debug, Clone)]
pub struct Law7MuTable {
    /// **The relative weight of this cosine**, i.e. the integral of the
    /// evaluation's own `f(μ, E')` over `E'` at this `μ`, *before* the table
    /// was renormalised to unit area \[per MeV\].
    ///
    /// # Why this exists
    ///
    /// [`normalize_pdf_cdf`] scales each cosine's table to integrate to 1, and
    /// until 2026-09-16 that normalisation was simply thrown away — which
    /// discards the **entire angular distribution**, since the relative size of
    /// the per-`μ` integrals *is* `f(μ)`. A consumer building an emission law
    /// from the normalised tables alone would sample `μ` uniformly and have no
    /// way to know it was wrong.
    ///
    /// Same class of defect as `op-og56` (MF=6 `LANG` coefficients dropped at
    /// parse time): data read, silently discarded, leaving a law that samples
    /// plausibly and incorrectly.
    pub weight: f64,
    /// Outgoing-energy interpolation (`1` histogram, `2` lin-lin).
    pub intt: u32,
    /// Outgoing-energy grid \[MeV\], ascending.
    pub e_out_mev: Vec<f64>,
    /// Probability density on `e_out_mev` \[1/MeV\], normalised to integrate to 1.
    pub pdf: Vec<f64>,
    /// Cumulative distribution: `cdf[0] = 0`, `cdf[last] = 1`.
    pub cdf: Vec<f64>,
}

/// One incident energy of a [`Mf6LabAngleEnergy`]: the lab-cosine grid and, for
/// each cosine, the outgoing-energy spectrum at that angle.
#[derive(Debug, Clone)]
pub struct Law7Incident {
    /// Incident neutron energy \[MeV\].
    pub e_in_mev: f64,
    /// Interpolation law across the cosine grid (ENDF `INTMU`).
    pub mu_interp: u32,
    /// Lab-frame cosine grid, ascending on `[-1, 1]`.
    pub mu: Vec<f64>,
    /// Outgoing-energy table at each `mu` (same length as `mu`).
    pub tables: Vec<Law7MuTable>,
}

/// The neutron emission of an MF=6 `LAW=7` (lab-frame angle-then-energy)
/// subsection: at each incident energy, a cosine grid and, per cosine, a
/// tabulated outgoing-energy spectrum — already in the laboratory frame (no
/// CM→lab transform needed, unlike LAW=1).
#[derive(Debug, Clone)]
pub struct Mf6LabAngleEnergy {
    /// Reference frame (`1` lab, `2` CM) from the MF=6 HEAD. `LAW=7` data is
    /// defined directly in the lab frame regardless of this flag.
    pub lct: i32,
    /// Neutron multiplicity (yield) vs incident energy `(E [eV], y)`.
    pub yield_pairs: Vec<(f64, f64)>,
    /// Interpolation regions over the incident-energy grid (empty ⇒ single
    /// lin-lin region).
    pub e_in_interp: Vec<(u32, u32)>,
    /// Per-incident-energy angle/energy tables, ascending in `e_in_mev`.
    pub incident: Vec<Law7Incident>,
}

/// The LAW=7 body of one neutron subsection: an outer TAB2 over incident
/// energies, then **per incident energy a second TAB2** over the lab-cosine
/// grid (`C2` = `E_in`, `N2` = `NMU`, its own interpolation list = `INTMU`),
/// followed by `NMU` TAB1s, each a `(E'_out, f)` spectrum at one lab cosine
/// (its header `C2` = that cosine).
///
/// Ported from **`groupr.f90`'s `getmf6`** (`law.eq.7` branch, ~7876-7911),
/// which is NJOY's reader for a *genuine ENDF-6 tape* -- `tab2io` for the
/// per-incident record, `nmu = tmp(l+5)`, i.e. `N2`.
///
/// # Do not port `acefc.f90`'s LAW=7 reader here (2026-09-16)
///
/// `acelf6` (acefc.f90 ~7880-7935) reads this record with `tab1io` and takes
/// `intmu = l1h`, `nmu = l2h`. That is correct **there and only there**: ACER
/// runs on NJOY's own intermediate File 6, and `skip6a`'s header comment says
/// so outright -- *"Special version of skip6 for special version of File 6
/// used in ACER. Law=7 has a TAB1 containing the angular distribution instead
/// of the normal TAB2 for each incident energy."*
///
/// This port reads evaluation tapes directly, so it needs the normal TAB2.
/// Until 2026-09-16 it followed `acelf6` instead, and on Be-9 MT=16 -- the only
/// LAW=7 neutron subsection in `reference-data/endf/` -- that read `L1 = L2 = 0`
/// and so built **zero cosine tables**, while mis-consuming the real data as the
/// TAB1's own pairs. The lesson is the standing one: read the upstream routine
/// that owns *this* input format, not the one whose name matches.
///
/// [`skip_mf6_subsection`] above had this right the whole time, and its doc
/// comment states the same split (and names Be-9 MT=16). Knowing the rule in one
/// function did not carry it to the next; the gates in
/// `tests/mf6_law7_mu_weights.rs` are what actually hold it.
fn parse_law7_lab_angle_energy_body(
    cur: &mut SectionCursor<'_>,
    lct: i32,
    ymult: Tab1,
) -> Result<Mf6LabAngleEnergy, NjoyError> {
    let tab2 = cur.read_tab2()?;
    let ne = tab2.head.n2.max(0);
    let e_in_interp = collapse_interp(&tab2.interp);

    let mut incident = Vec::with_capacity(ne as usize);
    for _ in 0..ne {
        // TAB2 over the cosine grid: C2 = E_in [eV], N2 = NMU, interp = INTMU.
        let outer = cur.read_tab2()?;
        let mu_interp = outer.interp.first().map(|&(_, i)| i).unwrap_or(2);
        let nmu = outer.head.n2.max(0);
        let e_in_mev = outer.head.c2 / EMEV;

        let mut mu = Vec::with_capacity(nmu as usize);
        let mut tables = Vec::with_capacity(nmu as usize);
        for _ in 0..nmu {
            let t = cur.read_tab1()?;
            let mu_val = t.head.c2;
            let raw_intep = t.interp.first().map(|&(_, i)| i).unwrap_or(2);
            let intt = if raw_intep >= 2 { 2 } else { 1 };
            let (e_out_mev, pdf, cdf, weight) = normalize_pdf_cdf_weighted(intt, &t.pairs);
            mu.push(mu_val);
            tables.push(Law7MuTable {
                weight,
                intt,
                e_out_mev,
                pdf,
                cdf,
            });
        }
        incident.push(Law7Incident {
            e_in_mev,
            mu_interp,
            mu,
            tables,
        });
    }

    Ok(Mf6LabAngleEnergy {
        lct,
        yield_pairs: ymult.pairs,
        e_in_interp,
        incident,
    })
}

/// Parse the **neutron** (`ZAP=1`) `LAW=7` (lab angle-energy) subsection of an
/// MF=6 section. Scans every subsection in file order, skipping non-matching
/// ones with [`skip_mf6_subsection`] — including `ZAP=1` subsections of a
/// *different* law — exactly as [`parse_mf6_law1_neutrons`] does.
///
/// # Errors
/// [`NjoyError::NotPorted`] if the section carries no `ZAP=1, LAW=7`
/// subsection; [`NjoyError::EndfParse`] on malformed records.
pub fn parse_mf6_law7_lab_angle_energy(section: &Section) -> Result<Mf6LabAngleEnergy, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?;
    let lct = head.l2;
    let nk = head.n1.max(0);

    for _ in 0..nk {
        let ymult = cur.read_tab1()?;
        let zap = ymult.head.c1.round() as i32;
        let law = ymult.head.l2;
        if zap == 1 && law == 7 {
            return parse_law7_lab_angle_energy_body(&mut cur, lct, ymult);
        }
        skip_mf6_subsection(&mut cur, law)?;
    }
    Err(NjoyError::NotPorted(
        "MF=6 section carries no ZAP=1 LAW=7 (lab angle-energy) neutron subsection",
    ))
}

#[cfg(test)]
mod incident_interp_survey {
    use super::*;
    use crate::endf::tape::Tape;
    use crate::reference_data::reference_data_dir;

    /// **Which incident-energy interpolation laws do real evaluations actually
    /// use on MF=6 LAW=1?**
    ///
    /// This matters because `ChiTabular` — the transport-side structure the
    /// Monte Carlo sampler consumes — carries the *outgoing* energy
    /// interpolation (`LEP`, as `ChiEout::linlin`) but, until 2026-09-16,
    /// dropped the **incident**-energy law (`e_in_interp`, the TAB2's own
    /// `INT`). The sampler therefore applied unit-base *linear* interpolation
    /// between incident rows unconditionally.
    ///
    /// For `INT = 2` (lin-lin) that is correct. For `INT = 1` (histogram) it is
    /// not: the evaluation is saying "use the lower row, do not interpolate".
    ///
    /// This test measures rather than assumes, over every neutron-sublibrary
    /// tape in `reference-data/endf/`, and **prints the tally**. It is
    /// deliberately not a pass/fail gate on the laws found — it is the evidence
    /// for how far the drop actually matters, and it will surface the day a
    /// tape with a histogram law is added.
    #[test]
    fn survey_incident_energy_interpolation_laws() {
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
        if files.is_empty() {
            println!("no neutron tapes; skipping");
            return;
        }

        let mut counts: std::collections::BTreeMap<i32, usize> = Default::default();
        let mut non_linlin = Vec::new();
        // Skips are COUNTED, not silent. A survey that quietly drops what it
        // cannot parse reports a clean answer about a fraction of the data --
        // the same failure mode as a skipped test reading as a pass.
        let (mut n_sec, mut n_parse_err, mut n_no_section) = (0usize, 0usize, 0usize);
        // Sections that are neither LAW=1 nor any law this crate converts. Each
        // one is a neutron emission silently falling back to the Weisskopf
        // stand-in, which is the exact failure this survey exists to surface.
        let mut unexplained: Vec<String> = Vec::new();
        // LANG: which angular representations appear. 11..15 (tabulated
        // cosines) are RETAINED but not sampled, so an evaluation using one
        // would silently fall back to isotropic -- this makes that loud.
        let mut lang_seen: std::collections::BTreeSet<String> = Default::default();
        for f in &files {
            let Ok(tape) = Tape::read_file(f) else {
                continue;
            };
            let Some(&mat) = tape.materials().first() else {
                continue;
            };
            for mt in [16i32, 17, 91] {
                let Some(sec) = tape.section(mat, 6, mt) else {
                    n_no_section += 1;
                    continue;
                };
                n_sec += 1;
                let subs = match parse_mf6_law1_neutrons(&sec) {
                    Ok(v) => v,
                    Err(e) => {
                        n_parse_err += 1;
                        // A LAW=1 parse failure is only acceptable if the
                        // section carries a law this crate DOES handle. Counting
                        // the skip made the survey honest; naming the reason is
                        // what stops "not LAW=1" from quietly covering a law
                        // nobody has ported. Both alternatives are implemented
                        // as of 2026-09-16 (LAW=6 phase space, LAW=7 lab
                        // angle-energy), so an unexplained skip is a real gap.
                        let alt = if parse_mf6_law6_phase_space(&sec).is_ok() {
                            "LAW=6 (phase space) -- converted"
                        } else if parse_mf6_law7_lab_angle_energy(&sec).is_ok() {
                            "LAW=7 (lab angle-energy) -- converted"
                        } else {
                            "NO SUPPORTED LAW"
                        };
                        let name = f.file_name().unwrap().to_string_lossy().into_owned();
                        println!("   not LAW=1: {name} MT={mt}: {e} -> {alt}");
                        if alt == "NO SUPPORTED LAW" {
                            unexplained.push(format!("{name} MT={mt}: {e}"));
                        }
                        continue;
                    }
                };
                for s in &subs {
                    lang_seen.insert(format!("{:?}", s.lang));
                    for &(_, int) in &s.law4.e_in_interp {
                        *counts.entry(int as i32).or_insert(0) += 1;
                    }
                    if s.law4.e_in_interp.iter().any(|&(_, i)| i != 2) {
                        non_linlin.push(format!(
                            "{} MT={mt} {:?}",
                            f.file_name().unwrap().to_string_lossy(),
                            s.law4.e_in_interp
                        ));
                    }
                }
            }
        }

        println!(
            "MF=6 LAW=1 incident-energy interpolation across {} neutron tapes (MT=16/17/91):\n\
             \x20  sections found {n_sec}, parse-skipped {n_parse_err}, absent {n_no_section}",
            files.len()
        );
        println!("   angular representations (LANG) seen: {lang_seen:?}");
        assert!(
            !lang_seen.iter().any(|l| l.starts_with("Tabulated")),
            "an evaluation in reference-data/endf uses MF=6 LANG = 11..15 (tabulated cosines), \
             which this port RETAINS but does not sample -- it would silently fall back to \
             isotropic. Seen: {lang_seen:?}. Implement it or record the affected nuclide."
        );
        let name = |i: i32| match i {
            1 => "histogram",
            2 => "lin-lin",
            3 => "lin-log",
            4 => "log-lin",
            5 => "log-log",
            11..=15 => "CORRESPONDING-POINT",
            21..=25 => "UNIT-BASE",
            _ => "?",
        };
        for (k, v) in &counts {
            println!("   INT={k} ({}) -> {v} interpolation range(s)", name(*k));
        }
        assert!(
            n_sec > 0,
            "no MF=6 MT=16/17/91 sections found at all; the survey measured nothing."
        );
        assert!(
            unexplained.is_empty(),
            "{} MF=6 section(s) carry a neutron emission law this crate neither parses as \
             LAW=1 nor converts (LAW=6 / LAW=7). Each one falls back to the Weisskopf \
             evaporation stand-in with nothing recording it -- the same silent gap Be-9 and \
             H-2 sat in until 2026-09-16. Sections: {:?}",
            unexplained.len(),
            unexplained
        );
    }
}
