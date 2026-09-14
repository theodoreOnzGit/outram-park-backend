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
    build_outgoing, build_outgoing_with_discrete, collapse_interp, normalize_pdf_cdf, Law4,
    OutgoingEnergy, EMEV,
};

/// The neutron emission of an MF=6 LAW=1 reaction, reduced to an ACE Law 4
/// energy distribution.
///
/// Built by [`parse_mf6_law1_neutron`]. Carries the multiplicity (yield) and
/// reference frame so the caller can fill the ACE TYR entry, plus the energy
/// distribution [`law4`](Self::law4). The **angular** dependence present in the
/// ENDF data (Legendre coefficients when LANG=1, Kalbach `r`/`a` when LANG=2) is
/// **not** carried here — extracting it into ACE Law 61/44 is the follow-up; this
/// captures the energy spectrum (`f₀`) as Law 4 (isotropic emission).
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

        match parse_law1_neutron_body(&mut cur, lct, ymult) {
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
    ymult: Tab1,
) -> Result<Mf6Neutron, NjoyError> {
    let tab2 = cur.read_tab2()?;
    let lep = tab2.head.l2; // secondary-energy interpolation
    let ne = tab2.head.n2;
    let intt = if lep >= 2 { 2 } else { 1 };
    let e_in_interp = collapse_interp(&tab2.interp);

    let mut incident = Vec::with_capacity(ne as usize);
    for _ in 0..ne {
        let list = cur.read_list()?;
        let e_in_ev = list.head.c2;
        let nd = list.head.l1.max(0) as usize; // number of discrete lines
        let na = list.head.l2; // number of angular coefficients per E_out
        let nep = list.head.n2 as usize; // number of secondary-energy points
                                          // Each row is [E'_out, f0, f1 … f_NA]; stride = NA + 2. Extract (E', f0).
        let stride = (na + 2) as usize;
        let mut pairs: Vec<(f64, f64)> = Vec::with_capacity(nep);
        for r in 0..nep {
            let base = r * stride;
            let e_out = list.data[base];
            let f0 = list.data[base + 1];
            pairs.push((e_out, f0));
        }
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
/// energies, then per incident energy one TAB1 (header-only: `INTMU`, `NMU`,
/// `E_in`; its own data pairs are discarded, matching `acelf6`'s `moreio`
/// pass that never reads them into a used variable) followed by `NMU` TAB1s,
/// each a `(E'_out, f)` spectrum at one lab cosine (its header `C2` = that
/// cosine). Faithful to acefc.f90's "law 7, angle-energy format" branch
/// (~7802–7846).
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
        let outer = cur.read_tab1()?; // header only: L1=INTMU, L2=NMU, C2=E_in [eV]
        let mu_interp = outer.head.l1.max(0) as u32;
        let nmu = outer.head.l2.max(0);
        let e_in_mev = outer.head.c2 / EMEV;

        let mut mu = Vec::with_capacity(nmu as usize);
        let mut tables = Vec::with_capacity(nmu as usize);
        for _ in 0..nmu {
            let t = cur.read_tab1()?;
            let mu_val = t.head.c2;
            let raw_intep = t.interp.first().map(|&(_, i)| i).unwrap_or(2);
            let intt = if raw_intep >= 2 { 2 } else { 1 };
            let (e_out_mev, pdf, cdf) = normalize_pdf_cdf(intt, &t.pairs);
            mu.push(mu_val);
            tables.push(Law7MuTable {
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
