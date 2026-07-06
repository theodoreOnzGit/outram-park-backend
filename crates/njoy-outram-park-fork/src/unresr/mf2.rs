//! ENDF File 2 LRU=2 (unresolved resonance region) parameter reader, and File
//! 3 background cross sections at the unresolved energy grid.
//!
//! Ported from `rdunf2` (`unresr.f90:385-751`) and `rdunf3`
//! (`unresr.f90:783-840`). Where upstream packs every l/j-state's parameters
//! into one flat scratch array (`arry(*)`, walked by hand-tracked integer
//! offsets in both `rdunf2` and, later, `unresl`), this port follows
//! `docs/porting-plan.md` §5's "scratch arrays → owned structs" convention:
//! [`UnresolvedRange`]/[`UnresolvedCase`]/[`LState`] name every field
//! semantically instead of replicating the offset arithmetic. Field meanings
//! were cross-checked both ways — against how `rdunf2` writes each slot *and*
//! against how `unresl` reads it back — since a couple of ENDF LIST-record
//! header fields (notably Case B's per-J `L2` = `AMUF`, not the more
//! obviously-guessed `AJ`) are easy to mis-assign from `rdunf2` alone.
//!
//! ENDF-102 recognises three representations for LRU=2 (LFW/LRF-selected,
//! following the field names `rdunf2` itself uses):
//!
//! - **Case A** (`LFW=0`) — every average parameter is energy-independent.
//! - **Case B** (`LFW=1`, `LRF≠2`) — only the fission width is
//!   energy-dependent, tabulated on one shared energy grid per range.
//! - **Case C** (`LRF=2`) — every average parameter (`D`, `GX`, `GNO`, `GG`,
//!   `GF`) is energy-dependent, tabulated independently per J-state.

use crate::NjoyError;

/// One J-state (spin sequence) under Case A (`LFW=0`, energy-independent).
/// Ported from `rdunf2:542-567`'s `i=1..5` capture.
#[derive(Debug, Clone, Copy)]
pub struct JStateA {
    /// Average level spacing `D` \[eV\].
    pub d: f64,
    /// Spin `J` of this sequence.
    pub aj: f64,
    /// Degrees of freedom for the neutron width distribution (`AMUN`).
    pub amun: f64,
    /// Reduced average neutron width `GNO` (`Γn(E) = GNO·√E·V_l(E)`) \[eV\].
    pub gno: f64,
    /// Average radiative capture width `GG` \[eV\].
    pub gg: f64,
}

/// One J-state under Case B (`LFW=1`, only the fission width is
/// energy-dependent). Ported from `rdunf2:611-626`.
#[derive(Debug, Clone)]
pub struct JStateB {
    /// Degrees of freedom for the fission-width distribution (`AMUF`) — this
    /// is the per-J `L2` field of its ENDF LIST-record header, **not** `AJ`
    /// (verified by cross-checking against `unresl:1043` reading it back as
    /// `amuf=arry(inow)`, the first of the six header slots).
    pub amuf: f64,
    /// Average level spacing `D` \[eV\].
    pub d: f64,
    /// Spin `J` of this sequence.
    pub aj: f64,
    /// Degrees of freedom for the neutron width distribution (`AMUN`).
    pub amun: f64,
    /// Reduced average neutron width `GNO` \[eV\].
    pub gno: f64,
    /// Average radiative capture width `GG` \[eV\].
    pub gg: f64,
    /// Fission widths `GF` \[eV\] tabulated on the range's shared
    /// [`CaseB::fission_energies`] grid (same length).
    pub gf: Vec<f64>,
}

/// One energy-dependent parameter set under Case C (`LRF=2`, every average
/// parameter tabulated per J-state). Ported from `rdunf2:670-682`.
#[derive(Debug, Clone, Copy)]
pub struct UnresolvedPointC {
    /// Energy of this tabulated point \[eV\].
    pub e: f64,
    /// Average level spacing `D` \[eV\] at this energy.
    pub d: f64,
    /// Average competitive-reaction width `GX` \[eV\] at this energy.
    pub gx: f64,
    /// Reduced average neutron width `GNO` \[eV\] at this energy.
    pub gno: f64,
    /// Average radiative capture width `GG` \[eV\] at this energy.
    pub gg: f64,
    /// Average fission width `GF` \[eV\] at this energy.
    pub gf: f64,
}

/// One J-state under Case C. Ported from `rdunf2:651-688`.
#[derive(Debug, Clone)]
pub struct JStateC {
    /// Spin `J` of this sequence.
    pub aj: f64,
    /// Degrees of freedom for the competitive-width distribution (`AMUX`).
    pub amux: f64,
    /// Degrees of freedom for the neutron-width distribution (`AMUN`).
    pub amun: f64,
    /// Degrees of freedom for the fission-width distribution (`AMUF`).
    pub amuf: f64,
    /// Energy-dependent parameter table (`D`, `GX`, `GNO`, `GG`, `GF` vs
    /// `E`), interpolated between points by [`crate::unresr::interp_case_c`].
    pub points: Vec<UnresolvedPointC>,
}

/// One orbital angular momentum (`L`) group, holding its J-state sequences.
/// The `L` value drives the penetrability/phase-shift factors
/// ([`crate::unresr::penetrability_factor`]).
#[derive(Debug, Clone)]
pub struct LState<J> {
    /// Orbital angular momentum `L` (`0, 1, 2, ...`).
    pub l: i32,
    /// The J-state sequences under this `L`.
    pub j_states: Vec<J>,
}

/// The three ENDF-102 LRU=2 parameter representations (see the module docs).
#[derive(Debug, Clone)]
pub enum UnresolvedCase {
    /// `LFW=0` — every average parameter energy-independent.
    CaseA {
        /// Mass ratio to the neutron (`AWRI`).
        awri: f64,
        /// Scattering radius `AP` \[10⁻¹² cm\] (used when `NAPS=1`).
        ap: f64,
        /// Target spin `SPI`.
        spi: f64,
        l_states: Vec<LState<JStateA>>,
    },
    /// `LFW=1`, `LRF≠2` — only the fission width is energy-dependent, on one
    /// shared energy grid for the whole range.
    CaseB {
        /// Mass ratio to the neutron (`AWRI`).
        awri: f64,
        /// Scattering radius `AP` \[10⁻¹² cm\].
        ap: f64,
        /// Target spin `SPI`.
        spi: f64,
        /// The shared fission-width energy grid \[eV\] (`rdunf2:585-594`).
        fission_energies: Vec<f64>,
        l_states: Vec<LState<JStateB>>,
    },
    /// `LRF=2` — every average parameter tabulated per J-state.
    CaseC {
        /// Mass ratio to the neutron (`AWRI`).
        awri: f64,
        /// Scattering radius `AP` \[10⁻¹² cm\].
        ap: f64,
        /// Target spin `SPI` (`rdunf2:638-639`'s `c2h`/`c1h`, cross-checked
        /// against the analogous Case A/B header — see the module docs).
        spi: f64,
        l_states: Vec<LState<JStateC>>,
    },
}

/// One LRU=2 energy range (one ENDF File-2 sub-section) plus the isotope
/// abundance and NAPS/NRO flags it was read with.
#[derive(Debug, Clone)]
pub struct UnresolvedRange {
    /// Lower energy bound of this range \[eV\].
    pub el: f64,
    /// Upper energy bound of this range \[eV\].
    pub eh: f64,
    /// Isotope abundance (`ABN`, for multi-isotope evaluations).
    pub abn: f64,
    /// `NAPS` flag: how the channel radius `a` relates to `AP` (see
    /// `unresl`'s `naps` handling, ported alongside [`crate::unresr`]).
    pub naps: i32,
    /// `NRO` flag: whether an energy-dependent scattering-radius `TAB1`
    /// precedes the parameters (`rdunf2:517-528`). **Not yet ported** — see
    /// the crate-level README caveats; ranges with `nro=1` are rejected by
    /// [`parse_lru2_ranges`] rather than silently parsed wrong.
    pub nro: i32,
    /// `LSSF` flag: if nonzero, the evaluator supplies already-self-shielded
    /// cross sections directly (skip the File-3 background read and the
    /// fluctuation-integral calculation — see `unresr.f90:172`).
    pub lssf: i32,
    /// Which of the three ENDF-102 representations this range uses, and its
    /// parameters.
    pub case_: UnresolvedCase,
}

/// Insert `e` into the sorted, duplicate-free list `list` in ascending order
/// — ported from `ilist` (`unresr.f90:753-781`). The list is primed with one
/// energy larger than any value ever inserted (`unresr.f90`'s doc comment);
/// callers seed it with a large sentinel (e.g. 1 MeV) before the first call.
pub fn ilist(e: f64, list: &mut Vec<f64>) {
    match list.iter().position(|&x| e <= x) {
        Some(i) if list[i] == e => {} // duplicate: no-op
        Some(i) => list.insert(i, e),
        None => list.push(e),
    }
}

/// Minimal ENDF-record cursor over one section's already-parsed
/// [`crate::endf::tape::Section`] rows, tracking only what `rdunf2`/`rdunf3`
/// need: sequential CONT/LIST/TAB1 reads with `NB`/`NW` continuation
/// (`moreio`), mirroring [`crate::reconr::mf2::SectionCursor`]'s role for the
/// resolved-region parser.
pub(crate) struct Cursor<'a> {
    rows: &'a [[f64; 6]],
    pos: usize,
}

/// One CONT record's six fields, named as NJOY's `endf` module would expose
/// them (`C1H, C2H, L1H, L2H, N1H, N2H`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cont {
    pub c1: f64,
    pub c2: f64,
    pub l1: i32,
    pub l2: i32,
    pub n1: i32,
    pub n2: i32,
}

impl<'a> Cursor<'a> {
    pub fn new(rows: &'a [[f64; 6]]) -> Self {
        Cursor { rows, pos: 0 }
    }

    fn next_row(&mut self) -> Result<[f64; 6], NjoyError> {
        let row = *self
            .rows
            .get(self.pos)
            .ok_or_else(|| NjoyError::EndfParse("unresr: unexpected end of section".to_string()))?;
        self.pos += 1;
        Ok(row)
    }

    /// Read one CONT record (one row, interpreted as `C1,C2,L1,L2,N1,N2`).
    pub fn cont(&mut self) -> Result<Cont, NjoyError> {
        let r = self.next_row()?;
        Ok(Cont { c1: r[0], c2: r[1], l1: r[2] as i32, l2: r[3] as i32, n1: r[4] as i32, n2: r[5] as i32 })
    }

    /// Read one LIST record: the CONT header plus `N1` (`NPL`) body values
    /// spread over as many continuation rows (`moreio`) as needed (6 values
    /// per row, the last row zero-padded by ENDF convention).
    pub fn list(&mut self) -> Result<(Cont, Vec<f64>), NjoyError> {
        let head = self.cont()?;
        let npl = head.n1.max(0) as usize;
        let mut body = Vec::with_capacity(npl);
        while body.len() < npl {
            let row = self.next_row()?;
            body.extend_from_slice(&row);
        }
        body.truncate(npl);
        Ok((head, body))
    }
}

/// Parse every LRU=2 range on this material's MF=2 sections, skipping (but
/// correctly walking past) LRU=0/1 resolved-region ranges — the LRU-2
/// counterpart of `rdunf2`'s combined resolved-skip / unresolved-store walk
/// (`unresr.f90:445-695`).
///
/// `sections` is every MF=2 section's rows for one isotope in file order (one
/// entry per `NIS` sub-section in the ENDF sense — for a single-isotope
/// evaluation this is one section). Each section is walked range by range
/// exactly as `rdunf2` does: a resolved range (`LRU≠2`) is read structurally
/// far enough to advance past it and discarded (RECONR already owns resolved
/// parameters, per [`crate::reconr::mf2`]); an `LRU=2` range is parsed in
/// full via [`UnresolvedCase`].
///
/// # Errors
///
/// Returns [`NjoyError::EndfParse`] if a section is malformed, or if a range
/// has `NRO=1` (energy-dependent scattering radius) — not yet ported (see
/// [`UnresolvedRange::nro`]).
pub fn parse_lru2_ranges(section_rows: &[[f64; 6]]) -> Result<Vec<UnresolvedRange>, NjoyError> {
    let mut cur = Cursor::new(section_rows);
    let mut ranges = Vec::new();

    // rdunf2:439-442 — per-isotope header CONT (ABN=C2, LFW=L2, NER=N1).
    let iso_head = cur.cont()?;
    let abn = iso_head.c2;
    let lfw = iso_head.l2;
    let ner = iso_head.n1;

    for _ier in 0..ner {
        let range_head = cur.cont()?;
        let el = sigfig7(range_head.c1);
        let eh = sigfig7(range_head.c2);
        let lru = range_head.l1;
        let lrf = range_head.l2;
        let nro = range_head.n1;
        let naps = range_head.n2;

        if lru != 2 {
            skip_resolved_range(&mut cur, lrf)?;
            continue;
        }
        if nro == 1 {
            return Err(NjoyError::EndfParse(
                "unresr: NRO=1 (energy-dependent scattering radius) in LRU=2 range not yet ported".to_string(),
            ));
        }

        let (case_, lssf) = if lrf == 2 {
            parse_case_c(&mut cur)?
        } else if lfw == 1 {
            parse_case_b(&mut cur)?
        } else {
            parse_case_a(&mut cur)?
        };

        ranges.push(UnresolvedRange { el, eh, abn, naps, nro, lssf, case_ });
    }

    Ok(ranges)
}

/// `sigfig(x, 7, 0)` — NJOY rounds range boundary energies to 7 significant
/// figures before using them as grid nodes (`unresr.f90:449-450`), so
/// boundary comparisons match exactly across ranges read from independently
/// rounded ENDF values. `SIGFIG` itself lives in `util.f90` and is not yet
/// ported as a general utility; this is the minimal 7-sig-fig rounding it
/// needs for LRU=2 range boundaries specifically.
fn sigfig7(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let mag = x.abs().log10().floor() as i32;
    let scale = 10f64.powi(6 - mag);
    (x * scale).round() / scale
}

/// Read (and discard) a resolved-region range (`LRU=0` or `LRU=1`) far enough
/// to advance the cursor past it — mirrors `rdunf2:459-491`. RECONR already
/// owns resolved-parameter parsing ([`crate::reconr::mf2`]); UNRESR only
/// needs the cursor correctly positioned at the next range.
fn skip_resolved_range(cur: &mut Cursor<'_>, lrf: i32) -> Result<(), NjoyError> {
    let head = cur.cont()?;
    let nls = head.n1;
    if lrf == 4 || lrf == 7 {
        cur.list()?;
    }
    for _ in 0..nls {
        if lrf != 4 {
            cur.list()?;
            if lrf == 7 {
                cur.list()?;
            }
        } else {
            let lcont = cur.cont()?;
            for _ in 0..lcont.n1 {
                cur.list()?;
            }
        }
    }
    Ok(())
}

/// Case A (`LFW=0`): `rdunf2:536-570`. Returns `(case, lssf)`.
fn parse_case_a(cur: &mut Cursor<'_>) -> Result<(UnresolvedCase, i32), NjoyError> {
    let head = cur.cont()?;
    let spi = head.c1;
    let ap = head.c2;
    let lssf = head.l1;
    let nls = head.n1;

    let mut awri = 0.0;
    let mut l_states = Vec::with_capacity(nls.max(0) as usize);
    for l_idx in 0..nls {
        let (h, body) = cur.list()?;
        if l_idx == 0 {
            awri = h.c1;
        }
        let ll = h.l1;
        let njs = h.n2.max(0) as usize;
        let mut j_states = Vec::with_capacity(njs);
        for j in 0..njs {
            let base = j * 6;
            let get = |k: usize| body.get(base + k).copied().unwrap_or(0.0);
            j_states.push(JStateA { d: get(0), aj: get(1), amun: get(2), gno: get(3), gg: get(4) });
        }
        l_states.push(LState { l: ll, j_states });
    }

    Ok((UnresolvedCase::CaseA { awri, ap, spi, l_states }, lssf))
}

/// Case B (`LFW=1`, `LRF≠2`): `rdunf2:573-629`. Returns `(case, lssf)`.
fn parse_case_b(cur: &mut Cursor<'_>) -> Result<(UnresolvedCase, i32), NjoyError> {
    let (head, body) = cur.list()?;
    let spi = head.c1;
    let ap = head.c2;
    let lssf = head.l1;
    let ne = head.n1.max(0) as usize;
    let nls = head.n2;

    // rdunf2:585-594 — the shared fission-width energy grid, 7-sigfig
    // rounded exactly as the range boundaries are.
    let fission_energies: Vec<f64> = (0..ne).map(|i| sigfig7(body.get(i).copied().unwrap_or(0.0))).collect();

    let mut awri = 0.0;
    let mut l_states = Vec::with_capacity(nls.max(0) as usize);
    for l_idx in 0..nls {
        let l_head = cur.cont()?;
        if l_idx == 0 {
            awri = l_head.c1;
        }
        let njs = l_head.n1.max(0) as usize;
        let ll = l_head.l1;

        let mut j_states = Vec::with_capacity(njs);
        for _ in 0..njs {
            let (jh, jbody) = cur.list()?;
            // rdunf2:614 — NE recomputed from this J's own NPL (=6+NE);
            // should equal the shared `ne` above.
            let ne_j = (jh.n1 - 6).max(0) as usize;
            let amuf = jh.l2 as f64; // see JStateB::amuf doc — L2, not AJ.
            let d = jbody.first().copied().unwrap_or(0.0);
            let aj = jbody.get(1).copied().unwrap_or(0.0);
            let amun = jbody.get(2).copied().unwrap_or(0.0);
            let gno = jbody.get(3).copied().unwrap_or(0.0);
            let gg = jbody.get(4).copied().unwrap_or(0.0);
            let gf: Vec<f64> = (0..ne_j).map(|k| jbody.get(6 + k).copied().unwrap_or(0.0)).collect();
            j_states.push(JStateB { amuf, d, aj, amun, gno, gg, gf });
        }
        l_states.push(LState { l: ll, j_states });
    }

    Ok((UnresolvedCase::CaseB { awri, ap, spi, fission_energies, l_states }, lssf))
}

/// Case C (`LRF=2`): `rdunf2:632-690`. Returns `(case, lssf)`.
fn parse_case_c(cur: &mut Cursor<'_>) -> Result<(UnresolvedCase, i32), NjoyError> {
    let head = cur.cont()?;
    let ap = head.c2;
    let spi = head.c1;
    let lssf = head.l1;
    let nls = head.n1;

    let mut awri = 0.0;
    let mut l_states = Vec::with_capacity(nls.max(0) as usize);
    for l_idx in 0..nls {
        let l_head = cur.cont()?;
        if l_idx == 0 {
            awri = l_head.c1;
        }
        let njs = l_head.n1.max(0) as usize;
        let ll = l_head.l1;

        let mut j_states = Vec::with_capacity(njs);
        for _ in 0..njs {
            let (jh, jbody) = cur.list()?;
            // rdunf2:659-667: AJ/INT/NE come from THIS J's own CONT header
            // (c1h/l1h/n2h — `jh.c1`/`jh.l1`(discarded)/`jh.n2`), not the
            // LIST body; the body (`jbody`, i.e. `scr(7..)`) only supplies
            // AMUX/AMUN/[unused]/AMUF at `scr(9..12)` == `jbody[2..5]`
            // (`do k=3,6: arry(k+inow)=scr(k+6)`).
            let aj = jh.c1;
            let ne = jh.n2.max(0) as usize;
            let amux = jbody.get(2).copied().unwrap_or(0.0);
            let amun = jbody.get(3).copied().unwrap_or(0.0);
            // jbody[4] (rdunf2's `arry(inow+5)`) is read into the fixed
            // header but never consulted by `unresl` — carried along
            // structurally, not stored (see `unresl:1055-1066`).
            let amuf = jbody.get(5).copied().unwrap_or(0.0);

            let mut points = Vec::with_capacity(ne);
            for n in 0..ne {
                // rdunf2:668-682: energy-point tuples start at `jnow=12`
                // (`scr(13)` == `jbody[6]`), 6 values per point.
                let base = 6 + n * 6;
                let get = |k: usize| {
                    let v = jbody.get(base + k).copied().unwrap_or(0.0);
                    // rdunf2:672 — non-positive values are floored to a
                    // small positive number (avoids div-by-zero in unresl).
                    if v <= 0.0 {
                        1e-12
                    } else {
                        v
                    }
                };
                points.push(UnresolvedPointC {
                    e: sigfig7(get(0)),
                    d: get(1),
                    gx: get(2),
                    gno: get(3),
                    gg: get(4),
                    gf: get(5),
                });
            }
            j_states.push(JStateC { aj, amux, amun, amuf, points });
        }
        l_states.push(LState { l: ll, j_states });
    }

    Ok((UnresolvedCase::CaseC { awri, ap, spi, l_states }, lssf))
}

/// Read File-3 background cross sections (`MT=1,2,18,102`; `MT=19` shares
/// `MT=18`'s column) at the unresolved energy grid, for use as `sigbkg` in
/// [`crate::unresr::unresolved_cross_sections`] — ported from `rdunf3`
/// (`unresr.f90:783-840`).
///
/// `mt_xy` supplies each background reaction's already-parsed ENDF `TAB1`
/// `(x, y)` pairs (from [`crate::endf::tape`]) plus its interpolation-law
/// list, keyed by MT; reactions absent from `mt_xy` contribute zero
/// background (matching `rdunf3`'s `sb(ie,kx)=0` when the MT is missing from
/// the tape, `unresr.f90:808-811`).
///
/// Returns `sb[energy_index][0..=3]` = `[total, elastic, fission, capture]`
/// background cross sections \[barn\] at each of `eunr`'s (absolute-valued)
/// energies, using the same edge-nudge (`unresr.f90:822-824`: the first/last
/// grid point is evaluated fractionally inside the range, avoiding a
/// boundary discontinuity) as upstream.
pub fn background_cross_sections(
    eunr: &[f64],
    mt_data: &[(i32, Vec<(u32, u32)>, Vec<(f64, f64)>)],
) -> Result<Vec<[f64; 4]>, NjoyError> {
    const UP: f64 = 1.000_01;
    const DN: f64 = 0.999_99;

    let find = |mt: i32| mt_data.iter().find(|(m, _, _)| *m == mt);
    let mts = [1, 2, 18, 102]; // MT=19 reuses MT=18's column, matching rdunf3's kx collapse.

    let mut out = vec![[0.0f64; 4]; eunr.len()];
    for (kx, &mt) in mts.iter().enumerate() {
        let Some((_, interp, xy)) = find(mt) else { continue };
        for (ie, &e_signed) in eunr.iter().enumerate() {
            let mut e = e_signed.abs();
            if ie == 0 {
                e *= UP;
            }
            if ie == eunr.len() - 1 {
                e *= DN;
            }
            out[ie][kx] = crate::endf::interp::eval_tab1(e, interp, xy)?;
        }
    }
    Ok(out)
}
