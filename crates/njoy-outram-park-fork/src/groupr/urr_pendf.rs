// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! PENDF **unresolved-resonance-range (URR) self-shielded cross-section reader**
//! — the tape-navigation half of `stounr` (`groupr.f90:6802-6894`).
//!
//! # What this reads
//!
//! After UNRESR/PURR processes an evaluation, it writes the URR self-shielded
//! cross sections to the PENDF tape as an **MF=2 / MT=152** section: a HEAD CONT
//! record carrying the self-shielding flag `lssf` and the sigma-zero
//! interpolation law `intunr`, followed by one LIST record per stored
//! temperature. This module locates that record and parses it into the
//! already-typed [`UnresolvedTable`] that [`crate::groupr::unresolved`]'s
//! `getunr` port ([`UnresolvedTable::shield`]) reads.
//!
//! The self-shielding *math* (sigma-zero + energy interpolation, the additive /
//! multiplicative `lssf` conventions, `terpu`) already lives in
//! [`crate::groupr::unresolved`]. This module is only the **feeder**: it turns
//! ENDF `[f64; 6]` rows into an [`UnresolvedTable`]. It adds no new physics.
//!
//! # MF=2/MT=152 record layout (the `unr(*)` array `getunr` reads)
//!
//! From the offsets `getunr` uses (`groupr.f90:6919-6924,6944-6956`), the stored
//! `unr(*)` array (1-based, = the LIST record body starting at its first data
//! word) is laid out:
//!
//! ```text
//! unr(1)              = temz   (temperature [K], LIST C1)
//! unr(2)              = (spi / unused here, LIST C2)
//! unr(3)              = nx     (number of reaction columns, LIST L1)
//! unr(4)              = nsig0  (number of sigma-zero values, LIST L2)
//! unr(5)              = (LIST N1)
//! unr(6)              = nunr   (number of URR energy points, LIST N2)
//! unr(7 .. 6+nsig0)   = sigma-zero grid [barn]           (locs = 7)
//! then, for each of nunr energy points, a block of ncyc = 1 + nx*nsig0 words:
//!     unr(loc)                          = energy [eV]  (negative marks resolved/
//!                                          unresolved overlap; see `iovl`)
//!     unr(loc+1 .. loc+nsig0)           = reaction 1 (total)   xs per sigma-zero
//!     unr(loc+nsig0+1 .. loc+2*nsig0)   = reaction 2 (elastic) xs per sigma-zero
//!     ...                                 (nx blocks, in the UrrReaction order)
//! ```
//!
//! `lssf` and `intunr` come from the section **HEAD CONT** (`stounr` reads
//! `lssf = l1h`, `intunr = n2h`, `groupr.f90:6847-6849`), not the LIST record.
//!
//! # Scope
//!
//! - **TODO(subagent):** implement [`read_urr_table`] — mirror `stounr`
//!   `groupr.f90:6822-6875`: read the HEAD CONT (`lssf`, `intunr`), read the LIST
//!   record (`temz`, `nsig0`, `nunr`, `nx`, the sigma-zero grid, the per-energy
//!   blocks), select the LIST whose temperature matches `temperature_k` (the
//!   `abs(temz - temp) <= 1` gate, `groupr.f90:6857`), and build an
//!   [`UnresolvedTable`] via [`UnresolvedTable::store`]. The negative-energy
//!   overlap marker becomes [`UrrEnergyPoint::overlap`].
//! - The MAT/MF/MT tape navigation (`findf`/`tosend`) is the caller's job via the
//!   [`crate::endf::tape::Tape`] API; this reader takes the **MF=2/MT=152 section
//!   rows** already extracted. See the doc on [`read_urr_table`].

use crate::endf::interp::IntLaw;
use crate::endf::records::SectionCursor;
use crate::groupr::unresolved::{LssfFlag, UnresolvedTable, UrrEnergyPoint};
use crate::NjoyError;

/// Map an ENDF/PENDF MF=2/MT=152 `lssf` integer flag (the HEAD CONT `L1`) to the
/// typed [`LssfFlag`] (`stounr` reads `lssf = l1h`, `groupr.f90:6849`).
///
/// `lssf == 0` → additive partial cross sections; `lssf > 0` → self-shielding
/// factors (multiplicative). See [`LssfFlag`] for the exact correction each
/// implies.
pub fn lssf_from_flag(lssf: i32) -> LssfFlag {
    if lssf == 0 {
        LssfFlag::Additive
    } else {
        LssfFlag::Multiplicative
    }
}

/// Read + parse the PENDF **MF=2/MT=152** URR self-shielded cross-section record
/// into an [`UnresolvedTable`] — the data-reading half of `stounr`
/// (`groupr.f90:6822-6875`).
///
/// # Parameters
/// - `section_rows` — the `[f64; 6]` data rows of the **MF=2/MT=152** section,
///   already extracted from the PENDF tape by the caller (via
///   [`crate::endf::tape::Tape`]). The first row is the HEAD CONT record; the
///   LIST record(s) follow. Read these with a [`SectionCursor`].
/// - `temperature_k` — the temperature \[K\] to select. A stored LIST whose
///   `temz` is within 1 K is accepted (`stounr`'s `abs(temz - temp) <= 1`
///   gate, `groupr.f90:6857`); if the first stored temperature already exceeds
///   the request, `stounr` errors — mirror that as [`NjoyError::EndfParse`].
///
/// # Returns
/// An [`UnresolvedTable`] ready for [`UnresolvedTable::shield`], or
/// [`NjoyError::EndfParse`] on a malformed record / missing temperature.
///
/// # Implementation (mirrors `stounr`, `groupr.f90:6822-6875`)
/// 1. Read the section **HEAD CONT** (`groupr.f90:6832`): `lssf =
///    lssf_from_flag(head.l1)` (`lssf = l1h`, `groupr.f90:6849`) and `intunr =
///    IntLaw::from_code(head.n2 as u32)` (`intunr = n2h`, `groupr.f90:6848`).
/// 2. Loop over the stored temperature LIST records (`listio`,
///    `groupr.f90:6850`). From each LIST header: `temz = list.head.c1` (LIST
///    C1), `nx = list.head.l1` (LIST L1 → `unr(3)`), `nsig0 = list.head.l2`
///    (LIST L2 → `unr(4)`), `nunr = list.head.n2` (LIST N2 → `unr(6)`). Apply
///    the temperature gate (`groupr.f90:6851-6858`): accept the LIST if
///    `|temz - temperature_k| <= 1.0`; if `temz > temperature_k` first, error
///    ("cannot find temp"); otherwise skip to the next temperature LIST.
/// 3. Slice the accepted LIST body (`list.data` = the `unr(7..)` payload):
///    `sigma0_grid = data[0..nsig0]` (`unr(7..6+nsig0)`), then `nunr` blocks of
///    `ncyc = 1 + nx*nsig0` words. Per block at `loc = nsig0 + ip*ncyc`:
///    `energy_ev = data[loc].abs()`, `overlap = data[loc] < 0.0` (the
///    negative-energy overlap marker, `groupr.f90:6961`), and reaction column
///    `ix` at dilution `is` is `data[loc + 1 + ix*nsig0 + is]` — the same
///    `unr(loc + ib + is)` layout `getunr` reads (`ib = nsig0*(ix-1)+1`,
///    `groupr.f90:6983`). Columns are the first `nx` [`crate::groupr::unresolved::UrrReaction`]s
///    (Total, Elastic, Fission, Capture, Current).
/// 4. Build via [`UnresolvedTable::store`], which validates ascending positive
///    energies and consistent per-point shapes.
pub fn read_urr_table(
    section_rows: &[[f64; 6]],
    temperature_k: f64,
) -> Result<UnresolvedTable, NjoyError> {
    let mut cur = SectionCursor::new(section_rows);

    // --- section HEAD CONT: lssf (l1h) and intunr (n2h) (groupr.f90:6848-6849).
    let head = cur.read_cont()?;
    let lssf = lssf_from_flag(head.l1);
    let intunr = IntLaw::from_code(head.n2 as u32);

    // --- loop the stored temperature LISTs, selecting the requested temperature.
    loop {
        if cur.remaining() == 0 {
            return Err(NjoyError::EndfParse(format!(
                "stounr: cannot find URR data for temp={temperature_k} K \
                 (MF=2/MT=152 section exhausted)"
            )));
        }
        let list = cur.read_list()?;
        let temz = list.head.c1;
        let nx = list.head.l1;
        let nsig0 = list.head.l2;
        let nunr = list.head.n2;

        // Temperature gate (groupr.f90:6851-6858).
        if (temz - temperature_k).abs() <= 1.0 {
            return parse_urr_list(&list.data, temperature_k, lssf, intunr, nx, nsig0, nunr);
        }
        if temz > temperature_k {
            return Err(NjoyError::EndfParse(format!(
                "stounr: cannot find temp={temperature_k} K; next stored temz={temz} K exceeds it"
            )));
        }
        // temz < temperature_k - 1: skip this LIST, try the next stored temperature.
    }
}

/// Decode one accepted MF=2/MT=152 LIST body (the `unr(7..)` payload) into an
/// [`UnresolvedTable`] — the data-slicing half of `stounr`
/// (`groupr.f90:6859-6873`), matching the offsets `getunr` reads
/// (`groupr.f90:6919-6956`).
///
/// `data` is the LIST record body; `nx`/`nsig0`/`nunr` are the LIST header
/// counts (`unr(3)`/`unr(4)`/`unr(6)`). Returns [`NjoyError::EndfParse`] on a
/// non-positive count or a truncated body.
fn parse_urr_list(
    data: &[f64],
    temperature_k: f64,
    lssf: LssfFlag,
    interp: IntLaw,
    nx: i32,
    nsig0: i32,
    nunr: i32,
) -> Result<UnresolvedTable, NjoyError> {
    if nx <= 0 || nsig0 <= 0 || nunr <= 0 {
        return Err(NjoyError::EndfParse(format!(
            "stounr: non-positive URR counts (nx={nx}, nsig0={nsig0}, nunr={nunr})"
        )));
    }
    let nx = nx as usize;
    let nsig0 = nsig0 as usize;
    let nunr = nunr as usize;
    let ncyc = 1 + nx * nsig0;
    let expected = nsig0 + nunr * ncyc;
    if data.len() < expected {
        return Err(NjoyError::EndfParse(format!(
            "stounr: LIST body too short (have {}, need {expected} for \
             nsig0={nsig0} + nunr={nunr} * ncyc={ncyc})",
            data.len()
        )));
    }

    // sigma-zero grid: unr(7..6+nsig0) (groupr.f90:6890).
    let sigma0_grid = data[0..nsig0].to_vec();

    // nunr energy blocks, each ncyc = 1 + nx*nsig0 words.
    let mut points = Vec::with_capacity(nunr);
    for ip in 0..nunr {
        let loc = nsig0 + ip * ncyc;
        let raw_energy = data[loc];
        let overlap = raw_energy < 0.0; // negative-energy overlap marker (groupr.f90:6961)
        let energy_ev = raw_energy.abs();
        // nx reaction columns, each nsig0 self-shielded cross sections [barn]:
        // unr(loc + nsig0*ix + is + 1) with ix,is 0-based (getunr `ib`,
        // groupr.f90:6983).
        let mut xs = Vec::with_capacity(nx);
        for ix in 0..nx {
            let base = loc + 1 + ix * nsig0;
            xs.push(data[base..base + nsig0].to_vec());
        }
        points.push(UrrEnergyPoint {
            energy_ev,
            overlap,
            xs,
        });
    }

    UnresolvedTable::store(temperature_k, lssf, interp, sigma0_grid, points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groupr::unresolved::UrrReaction;

    /// Pack a CONT header + `data` payload into `[f64; 6]` rows, exactly as an
    /// ENDF LIST record is laid out on tape: the six-word header row
    /// `[c1, c2, l1, l2, n1, n2]` (`n1` = NPL = `data.len()`), then the payload
    /// packed 6 words per row, last row zero-padded. This is the inverse of
    /// [`SectionCursor::read_list`], so it lets a test round-trip a known
    /// [`UnresolvedTable`] through the reader.
    fn list_rows(c1: f64, c2: f64, l1: i32, l2: i32, n2: i32, data: &[f64]) -> Vec<[f64; 6]> {
        let head = [c1, c2, l1 as f64, l2 as f64, data.len() as f64, n2 as f64];
        let mut rows = vec![head];
        for chunk in data.chunks(6) {
            let mut row = [0.0f64; 6];
            row[..chunk.len()].copy_from_slice(chunk);
            rows.push(row);
        }
        rows
    }

    /// One CONT row `[c1, c2, l1, l2, n1, n2]` (the section HEAD record).
    fn cont_row(c1: f64, c2: f64, l1: i32, l2: i32, n1: i32, n2: i32) -> [f64; 6] {
        [c1, c2, l1 as f64, l2 as f64, n1 as f64, n2 as f64]
    }

    /// A small, fully-known MF=2/MT=152 LIST body: nx=3 reactions
    /// (Total, Elastic, Fission), nsig0=2 dilutions `[1e10, 100]` barn, nunr=3
    /// energy points (the middle one flagged as resolved/unresolved overlap via
    /// a negative energy). Returns `(data, sigma0_grid, points)` so a test can
    /// reuse it as both tape input and the expected [`UnresolvedTable`] content.
    fn synthetic_body(
        temz_energies: [f64; 3],
    ) -> (Vec<f64>, Vec<f64>, Vec<UrrEnergyPoint>) {
        let sigma0 = vec![1.0e10, 100.0];
        // xs[point][reaction][dilution], barn.
        let xs_by_point: [[[f64; 2]; 3]; 3] = [
            [[10.0, 9.0], [8.0, 7.5], [1.0, 0.9]], // point 0
            [[12.0, 11.0], [9.0, 8.5], [1.5, 1.4]], // point 1 (overlap)
            [[15.0, 14.0], [10.0, 9.5], [2.0, 1.9]], // point 2
        ];
        let overlap = [false, true, false];

        // Signed energies as stored on tape (overlap => negative).
        let signed = [
            temz_energies[0],
            -temz_energies[1],
            temz_energies[2],
        ];

        let mut data = sigma0.clone();
        for ip in 0..3 {
            data.push(signed[ip]);
            for rx in 0..3 {
                for is in 0..2 {
                    data.push(xs_by_point[ip][rx][is]);
                }
            }
        }

        let points: Vec<UrrEnergyPoint> = (0..3)
            .map(|ip| UrrEnergyPoint {
                energy_ev: temz_energies[ip],
                overlap: overlap[ip],
                xs: (0..3).map(|rx| xs_by_point[ip][rx].to_vec()).collect(),
            })
            .collect();

        (data, sigma0, points)
    }

    /// **V&V — round-trip of a synthetic MF=2/MT=152 section through the
    /// `stounr` reader.**
    ///
    /// *Methodology.* A synthetic MF=2/MT=152 section is built from a fully
    /// known [`UnresolvedTable`] shape: a HEAD CONT carrying `lssf=0`
    /// (additive) and `intunr=2` (lin-lin), then one LIST record at
    /// `temz=293.6 K` with `nx=3` reactions (Total, Elastic, Fission),
    /// `nsig0=2` dilutions `[1e10, 100]` barn, and `nunr=3` energy points
    /// `[1000, 2000, 5000]` eV — the middle point flagged as
    /// resolved/unresolved overlap via a negative on-tape energy. The `[f64; 6]`
    /// rows are packed by [`list_rows`] (the exact inverse of
    /// [`SectionCursor::read_list`]). [`read_urr_table`] parses the rows; the
    /// parsed table is compared to a reference built directly with
    /// [`UnresolvedTable::store`] from the same known values. Pass criterion:
    /// the parsed table reproduces the sigma-zero grid, energies, overlap
    /// flags, and per-reaction cross sections **exactly** (`< 1e-12`,
    /// in fact bit-for-bit via `Debug`), and its self-shielded cross sections
    /// recovered through [`UnresolvedTable::shield`] at each stored (energy,
    /// dilution, reaction) node match the stored barns within `1e-12`.
    ///
    /// *Results (2026-07-15, synthetic data).* Parsed `sigma0_grid = [1e10,
    /// 100]` barn (exact); `n_points = 3` (exact). Full-structure `Debug`
    /// equality against the reference holds (bit-exact — energies `[1000, 2000,
    /// 5000]` eV, overlap flags `[false, true, false]`, all 18 cross sections).
    /// `shield`-recovered cross sections reproduced every stored value to
    /// `0.0` absolute difference (e.g. Total@1000eV = `[10.0, 9.0]`,
    /// Fission@5000eV = `[2.0, 1.9]` barn). Interpretation: the PENDF URR-tape
    /// reader lays the `unr(*)` array out exactly as `getunr` expects, for both
    /// the additive `lssf` convention and the negative-energy overlap marker.
    #[test]
    fn urr_reader_round_trips_synthetic_mf2_mt152() {
        let temz = 293.6;
        let energies = [1000.0, 2000.0, 5000.0];
        let (data, sigma0, points) = synthetic_body(energies);

        // HEAD CONT: c1=ZA, c2=AWR, l1=lssf=0, l2=0, n1=0, n2=intunr=2 (lin-lin).
        let mut rows = vec![cont_row(92238.0, 236.0, 0, 0, 0, 2)];
        // One temperature LIST: c1=temz, c2=0, l1=nx=3, l2=nsig0=2, n2=nunr=3.
        rows.extend(list_rows(temz, 0.0, 3, 2, 3, &data));

        let parsed = read_urr_table(&rows, temz).expect("parse synthetic URR section");

        // Reference table from the same known values.
        let reference = UnresolvedTable::store(
            temz,
            LssfFlag::Additive,
            IntLaw::LinLin,
            sigma0.clone(),
            points.clone(),
        )
        .expect("build reference table");

        // Exact structural equality (energies, overlap flags, xs, sigma0, ...).
        assert_eq!(
            format!("{parsed:?}"),
            format!("{reference:?}"),
            "parsed table must reproduce the reference bit-for-bit"
        );

        // Public numeric checks: sigma-zero grid and point count.
        assert_eq!(parsed.sigma0_grid().len(), sigma0.len());
        for (got, want) in parsed.sigma0_grid().iter().zip(&sigma0) {
            assert!((got - want).abs() < 1e-12, "sigma0 {got} vs {want}");
        }
        assert_eq!(parsed.n_points(), 3);

        // Recover each stored cross section through `shield` at the stored
        // (energy, dilution) nodes and check < 1e-12 against the known values.
        let reactions = [UrrReaction::Total, UrrReaction::Elastic, UrrReaction::Fission];
        for (ip, e) in energies.iter().enumerate() {
            for (rx_col, &rx) in reactions.iter().enumerate() {
                let want = &points[ip].xs[rx_col];
                // Additive combine: sig = bg + s - sinf; set bg = sinf (col[0])
                // so sig recovers s = stored xs at each stored dilution node.
                let sinf = want[0];
                let bg = vec![sinf; sigma0.len()];
                let out = parsed
                    .shield(rx, *e, &bg, &sigma0)
                    .expect("shield at stored node");
                for (k, &w) in want.iter().enumerate() {
                    assert!(
                        (out.sig[k] - w).abs() < 1e-12,
                        "reaction {rx:?} point {ip} dilution {k}: got {} want {w}",
                        out.sig[k]
                    );
                }
            }
        }
    }

    /// The reader skips temperature LISTs below the request and selects the
    /// first LIST within 1 K of it (`stounr`'s `abs(temz-temp) <= 1` gate with
    /// the skip-forward path, `groupr.f90:6851-6858`).
    ///
    /// *Methodology.* A section with two LISTs — `temz = 294.0 K` (distinct
    /// cross sections) then `temz = 600.5 K` — is parsed with a request of
    /// `600.0 K`. The first LIST (294 < 600) must be skipped and the second
    /// (`|600.5-600| = 0.5 <= 1`) selected. *Result (2026-07-15):* the parsed
    /// table's Total@1000eV cross sections equal the **second** LIST's stored
    /// values `[15, 14]` barn (recovered via `shield`), confirming the correct
    /// temperature LIST was chosen.
    #[test]
    fn urr_reader_skips_to_matching_temperature() {
        let energies = [1000.0, 2000.0, 5000.0];
        // First LIST at 294 K (arbitrary body).
        let (data_lo, _, _) = synthetic_body(energies);
        // Second LIST at 600.5 K: same shape, but shift every cross section by
        // +5 barn (energies unchanged) so the two temperatures are
        // distinguishable in the recovered values.
        let (data_hi, sigma0, points_hi) = {
            let (_, s, mut p) = synthetic_body(energies);
            let mut data = s.clone();
            for pt in &mut p {
                let signed_e = if pt.overlap { -pt.energy_ev } else { pt.energy_ev };
                data.push(signed_e);
                for col in &mut pt.xs {
                    for x in col.iter_mut() {
                        *x += 5.0;
                    }
                    data.extend_from_slice(col);
                }
            }
            (data, s, p)
        };

        let mut rows = vec![cont_row(92238.0, 236.0, 0, 0, 0, 2)];
        rows.extend(list_rows(294.0, 0.0, 3, 2, 3, &data_lo));
        rows.extend(list_rows(600.5, 0.0, 3, 2, 3, &data_hi));

        let parsed = read_urr_table(&rows, 600.0).expect("select second temperature LIST");

        // Confirm we got the high-temperature body: Total@1000eV = [15, 14].
        let want = &points_hi[0].xs[0];
        let sinf = want[0];
        let bg = vec![sinf; sigma0.len()];
        let out = parsed
            .shield(UrrReaction::Total, energies[0], &bg, &sigma0)
            .expect("shield");
        for (k, &w) in want.iter().enumerate() {
            assert!(
                (out.sig[k] - w).abs() < 1e-12,
                "selected wrong LIST: dilution {k} got {} want {w}",
                out.sig[k]
            );
        }
    }

    /// A request below the first stored temperature errors, mirroring
    /// `stounr`'s `temz > temp` fatal path (`groupr.f90:6853-6856`).
    ///
    /// *Result (2026-07-15):* requesting `100 K` against a section whose first
    /// (and only) LIST is at `294 K` returns [`NjoyError::EndfParse`].
    #[test]
    fn urr_reader_errors_when_temperature_too_low() {
        let (data, _, _) = synthetic_body([1000.0, 2000.0, 5000.0]);
        let mut rows = vec![cont_row(92238.0, 236.0, 0, 0, 0, 2)];
        rows.extend(list_rows(294.0, 0.0, 3, 2, 3, &data));

        assert!(matches!(
            read_urr_table(&rows, 100.0),
            Err(NjoyError::EndfParse(_))
        ));
    }

    /// `lssf` flag mapping matches the additive / multiplicative convention.
    #[test]
    fn lssf_flag_mapping() {
        assert_eq!(lssf_from_flag(0), LssfFlag::Additive);
        assert_eq!(lssf_from_flag(1), LssfFlag::Multiplicative);
        assert_eq!(lssf_from_flag(2), LssfFlag::Multiplicative);
    }
}
