// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Minimal, self-contained GENDF (GROUPR output) reader for DTFR
//! (`dtfr.f90:183-427`).
//!
//! DTFR consumes the multigroup tape written by GROUPR. This module reads just
//! enough of that GENDF layout — over this crate's in-memory
//! [`crate::endf::tape::Tape`] — to feed the already-ported DTF table assembly
//! ([`crate::dtfr::table`]) and formatting ([`crate::dtfr::format`]). It is
//! **deliberately self-contained**: it does not depend on the GROUPR writer
//! module, only on the generic ENDF record cursor
//! ([`crate::endf::records::SectionCursor`]).
//!
//! ## GENDF record layout (as read by `dtfr.f90`)
//!
//! **Material header — MF=1/451** (`dtfr.f90:199-237`): a HEAD CONT whose fields
//! carry `nsigz` (word 4, `a(4)`) and `ntw` (word 6, `a(6)`), followed by a LIST
//! record whose head carries `temp` (`C1`), `ngn` (`L1`, `a(9)`) and `ngg` (`L2`,
//! `a(10)`). Its data block is `[ntw title words][nsigz sigma-zeros][ngn+1 neutron
//! group bounds][ngg+1 gamma group bounds]`.
//!
//! **Reaction sections — MF=3/6/23/26** (`dtfr.f90:295-330`): a HEAD CONT whose
//! `L1 = nl` (Legendre orders) and `L2 = nz` (sigma-zeros), then one LIST record
//! per initial group. Each LIST head carries `ng2` (`L1`, `a(3)`), `ig2lo` (`L2`,
//! `a(4)`) and `ig` (`N2`, `a(6)`); its data block holds, at zero-based offset
//! `(il-1) + nl*((jz-1) + nz*(k-1))`, the value for Legendre order `il`
//! (`1..=nl`), sigma-zero `jz` (`1..=nz`) and secondary index `k` (`1..=ng2`)
//! (`dtfr.f90:343,356,418`). `k = 1` is the group flux; `k = 2` is the group
//! cross section (MF=3) or the first transfer (MF=6).
//!
//! Cross sections are in **barns**, energies in **eV**, temperature in **kelvin**.

use crate::dtfr::input::NeutronTables;
use crate::dtfr::table::{dtf_group, DtfTable};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// The GENDF material header (`dtfr.f90:199-237`).
#[derive(Debug, Clone, PartialEq)]
pub struct GendfHeader {
    /// Material temperature, **kelvin** (`a(7)`, `dtfr.f90:202`).
    pub temp: f64,
    /// Number of sigma-zero (dilution) values (`nsigz`, `dtfr.f90:213`).
    pub nsigz: i32,
    /// Number of title words on the header LIST record (`ntw`, `dtfr.f90:214`).
    pub ntw: i32,
    /// Number of neutron groups (`ngn`, `dtfr.f90:215`).
    pub ngn: i32,
    /// Number of gamma groups (`ngg`, `dtfr.f90:219`).
    pub ngg: i32,
    /// The `nsigz` sigma-zero values, **barns** (`dtfr.f90:225-227`).
    pub sigz: Vec<f64>,
    /// The `ngn+1` neutron group boundaries, **eV**, descending as stored
    /// (`dtfr.f90:229-232`).
    pub egn: Vec<f64>,
    /// The `ngg+1` gamma group boundaries, **eV** (`dtfr.f90:234-237`).
    pub egg: Vec<f64>,
}

/// One initial-group LIST record of a GENDF reaction section
/// (`dtfr.f90:311-323`).
#[derive(Debug, Clone, PartialEq)]
pub struct GendfGroupRecord {
    /// Number of Legendre orders on the section (`nl`, `dtfr.f90:302`).
    pub nl: i32,
    /// Number of sigma-zeros on the section (`nz`, `dtfr.f90:304`).
    pub nz: i32,
    /// Number of secondary groups in this record including the flux
    /// (`ng2`, `dtfr.f90:313`).
    pub ng2: i32,
    /// GENDF group index of the first secondary group (`ig2lo`, `dtfr.f90:314`).
    pub ig2lo: i32,
    /// GENDF index of this initial group (`ig`, `dtfr.f90:315`).
    pub ig: i32,
    /// The `lz`-relative data block (`dtfr.f90:316-323`); see [`GendfGroupRecord::value`].
    pub data: Vec<f64>,
}

impl GendfGroupRecord {
    /// The value for Legendre order `il` (`1..=nl`), sigma-zero `jz` (`1..=nz`)
    /// and secondary index `k` (`1..=ng2`) at zero-based data offset
    /// `(il-1) + nl*((jz-1) + nz*(k-1))` (`dtfr.f90:343,356,418`).
    ///
    /// Returns `0.0` if the computed offset is out of range (a short record).
    pub fn value(&self, il: i32, jz: i32, k: i32) -> f64 {
        let idx = (il - 1) + self.nl * ((jz - 1) + self.nz * (k - 1));
        if idx < 0 {
            return 0.0;
        }
        self.data.get(idx as usize).copied().unwrap_or(0.0)
    }

    /// The group cross section for `(il, jz)` — the `k = 2` slot (`dtfr.f90:343`,
    /// `loca = lz + il + nl*((jz-1) + nz)`). `k = 1` holds the flux.
    pub fn cross_section(&self, il: i32, jz: i32) -> f64 {
        self.value(il, jz, 2)
    }
}

/// Read the GENDF material header (MF=1/451) for material `mat`
/// (`dtfr.f90:199-237`).
///
/// # Errors
/// Returns [`NjoyError::EndfParse`] if the material has no MF=1/451 section or the
/// header LIST is truncated.
pub fn read_header(tape: &Tape, mat: i32) -> Result<GendfHeader, NjoyError> {
    let sec = tape.section(mat, 1, 451).ok_or(NjoyError::EndfParse(
        "dtfr::gendf: MF=1/451 header missing".into(),
    ))?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?; // HEAD: nsigz = l2 (a(4)), ntw = n2 (a(6)).
    let nsigz = head.l2;
    let ntw = head.n2;
    let list = cur.read_list()?; // LIST: temp = c1, ngn = l1 (a(9)), ngg = l2 (a(10)).
    let temp = list.head.c1;
    let ngn = list.head.l1;
    let ngg = list.head.l2;

    // Data block: [ntw title][nsigz sigz][ngn+1 egn][ngg+1 egg].
    let mut off = ntw as usize;
    let get = |data: &[f64], start: usize, n: usize| -> Result<Vec<f64>, NjoyError> {
        data.get(start..start + n)
            .map(|s| s.to_vec())
            .ok_or(NjoyError::EndfParse(
                "dtfr::gendf: header LIST truncated".into(),
            ))
    };
    let sigz = get(&list.data, off, nsigz as usize)?;
    off += nsigz as usize;
    let egn = get(&list.data, off, (ngn + 1) as usize)?;
    off += (ngn + 1) as usize;
    let egg = get(&list.data, off, (ngg + 1) as usize).unwrap_or_default();

    Ok(GendfHeader {
        temp,
        nsigz,
        ntw,
        ngn,
        ngg,
        sigz,
        egn,
        egg,
    })
}

/// Read every initial-group LIST record of a GENDF reaction section
/// `(mat, mf, mt)` (`dtfr.f90:295-323`).
///
/// Returns an empty vector if the section is absent.
///
/// # Errors
/// Returns [`NjoyError`] if a record is truncated.
pub fn read_section_records(
    tape: &Tape,
    mat: i32,
    mf: i32,
    mt: i32,
) -> Result<Vec<GendfGroupRecord>, NjoyError> {
    let Some(sec) = tape.section(mat, mf, mt) else {
        return Ok(Vec::new());
    };
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?; // HEAD: nl = l1 (a(3)), nz = l2 (a(4)).
    let nl = head.l1;
    let nz = head.l2;
    let mut records = Vec::new();
    while cur.remaining() > 0 {
        let list = cur.read_list()?;
        records.push(GendfGroupRecord {
            nl,
            nz,
            ng2: list.head.l1,   // a(3)
            ig2lo: list.head.l2, // a(4)
            ig: list.head.n2,    // a(6)
            data: list.data,
        });
    }
    Ok(records)
}

/// Assemble a neutron DTF table (P0/total) from a GENDF tape — the minimal subset
/// of the DTFR accumulation loop (`dtfr.f90:200-427`) needed to exercise the
/// ported table assembly.
///
/// This handles two reaction channels at Legendre order `il = 1`, sigma-zero
/// `jz`:
///
/// * **MF=3 MT=1 total** → stored at position `iptotl` for each DTF group, and
///   accumulated (as `+total`) into the absorption position `iptotl-2`
///   (`dtfr.f90:340-349`).
/// * **MF=6 MT=2 elastic transfer** → packed into the reduced scatter band via
///   [`DtfTable::add_scatter_record`], which also subtracts each transfer from the
///   absorption position, so `absorption = total − scatter` builds up
///   (`dtfr.f90:409-427`).
///
/// The GENDF neutron-group count must equal `neutron.ng` (`dtfr.f90:216-218`).
/// Other channels (fission ν·σ_f/χ, edits, thermal, photons, higher Legendre
/// orders) are **not** assembled here — see [`crate::dtfr::run`].
///
/// # Errors
/// Returns [`NjoyError::EndfParse`] on a group-count mismatch or a truncated
/// record.
pub fn build_neutron_table(
    tape: &Tape,
    mat: i32,
    neutron: &NeutronTables,
    jz: i32,
) -> Result<DtfTable, NjoyError> {
    let header = read_header(tape, mat)?;
    if header.ngn != neutron.ng {
        return Err(NjoyError::EndfParse(
            "dtfr::gendf: GENDF neutron group count disagrees with requested ng".into(),
        ));
    }
    let ng = neutron.ng;
    let iptotl = neutron.iptotl;
    let ipingp = neutron.ipingp;
    let mut table = DtfTable::new(ng, neutron.itabl);

    // MF=3 MT=1 total cross section (dtfr.f90:340-349).
    for rec in read_section_records(tape, mat, 3, 1)? {
        let jg = dtf_group(rec.ig, ng);
        if jg < 1 || jg > ng {
            continue;
        }
        let total = rec.cross_section(1, jz);
        table.set(iptotl, jg, total);
        // absorption seed: sig(iptotl-2) += total (dtfr.f90:348-349).
        table.add(iptotl - 2, jg, total);
    }

    // MF=6 MT=2 elastic transfer matrix (dtfr.f90:409-427).
    for rec in read_section_records(tape, mat, 6, 2)? {
        let jg = dtf_group(rec.ig, ng);
        if jg < 1 || jg > ng {
            continue;
        }
        // Secondary values k = 2..=ng2 (k = 1 is the flux).
        let values: Vec<f64> = (2..=rec.ng2).map(|k| rec.value(1, jz, k)).collect();
        table.add_scatter_record(jg, rec.ig2lo, ipingp, iptotl, 1, &values);
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtfr::input::DtfrInput;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;

    /// Pack a flat value list six-per-row into `[f64; 6]` ENDF continuation rows.
    fn pack(values: &[f64]) -> Vec<[f64; 6]> {
        values
            .chunks(6)
            .map(|c| {
                let mut r = [0.0f64; 6];
                r[..c.len()].copy_from_slice(c);
                r
            })
            .collect()
    }

    /// Build a GENDF MF=1/451 header section: HEAD then one LIST record.
    fn header_section(mat: i32, temp: f64, ng: i32, sigz: &[f64], egn: &[f64]) -> Section {
        let nsigz = sigz.len() as i32;
        let ntw = 0i32;
        // HEAD: [za, awr, l1, nsigz(l2), n1, ntw(n2)].
        let head = [92238.0, 236.0, 0.0, nsigz as f64, 0.0, ntw as f64];
        // LIST head: [temp(c1), 0, ngn(l1), ngg(l2), nw(n1), 0].
        let mut data = Vec::new();
        data.extend_from_slice(sigz); // nsigz sigma-zeros (ntw=0 title words)
        data.extend_from_slice(egn); // ngn+1 neutron bounds
        let nw = data.len() as i32;
        let list_head = [temp, 0.0, ng as f64, 0.0, nw as f64, 0.0];
        let mut rows = vec![head, list_head];
        rows.extend(pack(&data));
        Section {
            key: EndfKey {
                mat,
                mf: 1,
                mt: 451,
            },
            rows,
        }
    }

    /// Build a GENDF reaction section: HEAD (nl, nz) then one LIST per group.
    /// Each `groups` entry is `(ig, ig2lo, ng2, data)`.
    fn reaction_section(
        mat: i32,
        mf: i32,
        mt: i32,
        nl: i32,
        nz: i32,
        groups: &[(i32, i32, i32, Vec<f64>)],
    ) -> Section {
        let head = [92238.0, 236.0, nl as f64, nz as f64, 0.0, 0.0];
        let mut rows = vec![head];
        for (ig, ig2lo, ng2, data) in groups {
            let nw = data.len() as i32;
            // LIST head: [c1, c2, ng2(l1), ig2lo(l2), nw(n1), ig(n2)].
            let list_head = [0.0, 0.0, *ng2 as f64, *ig2lo as f64, nw as f64, *ig as f64];
            rows.push(list_head);
            rows.extend(pack(data));
        }
        Section {
            key: EndfKey { mat, mf, mt },
            rows,
        }
    }

    /// The header reader recovers temp, group counts, sigma-zeros, and bounds.
    ///
    /// **Methodology.** `dtfr.f90:199-237`. Build a header with `temp=293.6`,
    /// `ng=3`, sigma-zeros `{1e10}`, and 4 neutron bounds; assert every field is
    /// read back.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `temp=293.6, ngn=3, nsigz=1,
    /// sigz=[1e10], egn=[2e7, 1e5, 1e2, 1e-5]`.
    #[test]
    fn header_roundtrips() {
        let egn = vec![2.0e7, 1.0e5, 1.0e2, 1.0e-5];
        let tape = Tape::from_sections(
            " gendf".into(),
            vec![header_section(9237, 293.6, 3, &[1.0e10], &egn)],
        );
        let h = read_header(&tape, 9237).unwrap();
        assert_eq!(h.ngn, 3);
        assert_eq!(h.nsigz, 1);
        assert!((h.temp - 293.6).abs() < 1e-9);
        assert_eq!(h.sigz, vec![1.0e10]);
        assert_eq!(h.egn, egn);
    }

    /// A group record indexes values by `(il, jz, k)`.
    ///
    /// **Methodology.** `dtfr.f90:343,356,418`: offset `(il-1)+nl*((jz-1)+nz*(k-1))`.
    /// With `nl=1, nz=1` the data is `[flux, xs]` (k=1 flux, k=2 cross section);
    /// `cross_section` returns the k=2 slot.
    ///
    /// **Result (2026-07-15).** data `[0.9, 4.5]` → `value(1,1,1)=0.9` (flux),
    /// `cross_section(1,1)=4.5`.
    #[test]
    fn group_record_value_indexing() {
        let rec = GendfGroupRecord {
            nl: 1,
            nz: 1,
            ng2: 2,
            ig2lo: 3,
            ig: 3,
            data: vec![0.9, 4.5],
        };
        assert_eq!(rec.value(1, 1, 1), 0.9);
        assert_eq!(rec.cross_section(1, 1), 4.5);
    }

    /// GENDF read → DTF table round-trips a small group set: the total lands at
    /// `iptotl` in DTF order, and the elastic transfer packs into the reduced band
    /// with the absorption correction.
    ///
    /// **Methodology (task V&V gate).** `ng=3`, CLAW geometry
    /// (`iptotl=46, ipingp=47, itabl=49`). Build a GENDF tape with:
    /// - MF=3 MT=1 totals: group `ig=3` (DTF group 1) total 5.0 b; `ig=2` (DTF 2)
    ///   total 3.0 b; `ig=1` (DTF 3) total 2.0 b — each as `[flux, xs]`.
    /// - MF=6 MT=2 elastic: for `ig=3` an in-group transfer of 4.0 b
    ///   (`ig2lo=3, ng2=2` → k=2 lands at `jg2=1=jg`, position `ipingp`).
    ///
    /// Assert the total lands at `sig(iptotl, jg)`, the in-group scatter at
    /// `sig(ipingp, 1)=4.0`, and the absorption at `sig(iptotl-2, 1) = 5.0 − 4.0 =
    /// 1.0` (total accumulated then scatter subtracted, `dtfr.f90:348,422-423`).
    ///
    /// **Result (2026-07-15, commit ac5adf5).** DTF-order totals
    /// `sig(46,1)=5, sig(46,2)=3, sig(46,3)=2`; in-group scatter `sig(47,1)=4`;
    /// absorption `sig(44,1)=1`.
    #[test]
    fn gendf_to_dtf_table_roundtrip() {
        let ng = 3;
        let deck = DtfrInput::claw_defaults(5, ng);
        let neutron = &deck.neutron;
        assert_eq!(neutron.iptotl, 46);
        assert_eq!(neutron.ipingp, 47);

        let egn = vec![2.0e7, 1.0e5, 1.0e2, 1.0e-5];
        let mf3 = reaction_section(
            9237,
            3,
            1,
            1,
            1,
            &[
                (3, 3, 2, vec![1.0, 5.0]), // DTF group 1, total 5.0
                (2, 2, 2, vec![1.0, 3.0]), // DTF group 2, total 3.0
                (1, 1, 2, vec![1.0, 2.0]), // DTF group 3, total 2.0
            ],
        );
        // Elastic in-group transfer from ig=3 (jg=1): ig2lo=3, ng2=2 so k=2 ->
        // jg2 = ng - ig2lo - k + 3 = 3 - 3 - 2 + 3 = 1 = jg (in-group).
        let mf6 = reaction_section(
            9237,
            6,
            2,
            1,
            1,
            &[(3, 3, 2, vec![1.0, 4.0])], // [flux, transfer=4.0]
        );
        let tape = Tape::from_sections(
            " gendf".into(),
            vec![header_section(9237, 293.6, ng, &[1.0e10], &egn), mf3, mf6],
        );

        let table = build_neutron_table(&tape, 9237, neutron, 1).unwrap();
        assert_eq!(table.get(neutron.iptotl, 1), 5.0);
        assert_eq!(table.get(neutron.iptotl, 2), 3.0);
        assert_eq!(table.get(neutron.iptotl, 3), 2.0);
        // in-group scatter at ipingp, DTF group 1
        assert_eq!(table.get(neutron.ipingp, 1), 4.0);
        // absorption = total - scatter at iptotl-2, DTF group 1
        assert_eq!(table.get(neutron.iptotl - 2, 1), 1.0);
    }

    /// The DTF table built from GENDF formats through `dtfout` (format 0).
    ///
    /// **Methodology.** Feed the round-tripped table's total column to
    /// [`crate::dtfr::format::column`] + [`crate::dtfr::format::format0_body`] and
    /// check the DTF-order totals appear. This closes the GENDF → DTF-IV format
    /// path the task asks for.
    ///
    /// **Result (2026-07-15).** The `iptotl` column is `[5, 3, 2]` (DTF groups
    /// 1..3) and formats to one 36-char line `"  5.0000E+00  3.0000E+00
    /// 2.0000E+00"`.
    #[test]
    fn gendf_table_formats_to_dtf() {
        use crate::dtfr::format::{column, format0_body};
        let ng = 3;
        let deck = DtfrInput::claw_defaults(5, ng);
        let neutron = &deck.neutron;
        let egn = vec![2.0e7, 1.0e5, 1.0e2, 1.0e-5];
        let mf3 = reaction_section(
            9237,
            3,
            1,
            1,
            1,
            &[
                (3, 3, 2, vec![1.0, 5.0]),
                (2, 2, 2, vec![1.0, 3.0]),
                (1, 1, 2, vec![1.0, 2.0]),
            ],
        );
        let tape = Tape::from_sections(
            " gendf".into(),
            vec![header_section(9237, 293.6, ng, &[1.0e10], &egn), mf3],
        );
        let table = build_neutron_table(&tape, 9237, neutron, 1).unwrap();
        let totals = column(&table.sig, neutron.iptotl, neutron.itabl, ng);
        assert_eq!(totals, vec![5.0, 3.0, 2.0]);
        let body = format0_body(&totals);
        assert_eq!(body.len(), 1);
        assert!(body[0].contains("5.0000E+00"));
        assert!(body[0].contains("2.0000E+00"));
    }
}
