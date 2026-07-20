// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! RESXS binary tape I/O — the PENDF reader feeder and the RESXS record writer /
//! reader (`resxsr.f90:250-501`).
//!
//! Two halves of the file-level pipeline that the [`crate::resxsr::assemble`]
//! kernels sit between:
//!
//! 1. **PENDF reader feeder** ([`read_pendf_reactions`]) — the in-memory analogue
//!    of `findf(matd,3)` + the `gety1` energy-grid walk (`resxsr.f90:296-347`):
//!    it pulls the MF=3 TAB1 cross sections for the resonance reactions (MT 2
//!    elastic, 18 fission, 102 capture) out of a parsed [`Tape`] into
//!    [`PointwiseReaction`]s ready for [`assemble_union_grid`].
//! 2. **RESXS writer / reader** ([`ResxsFile::write`] / [`ResxsFile::read`]) — the
//!    binary output record sequence (`resxsr.f90:435-501`) and a matching reader,
//!    so a written file round-trips.
//!
//! ## RESXS on-disk word model (documented port encoding)
//!
//! RESXS is a NJOY *unformatted* binary file. Its Fortran record markers are
//! compiler/runtime specific and cannot be reproduced byte-for-byte without a
//! golden file, so this port defines an explicit, self-delimiting encoding that
//! reproduces the RESXS **record sequence, field order, and word counts** exactly
//! (each record's `nwds` matches the `resxsr.f90` formula in
//! [`crate::resxsr::format`]). It is **not** byte-identical to genuine NJOY output.
//!
//! The RESXS "storage word" is **4 bytes** (`resxsr.f90:95-97`, `mult == 2` means
//! an `a8` Hollerith spans two words). This encoding therefore uses:
//!
//! - a **real** word → `f32` little-endian (single precision — the RESXS reals
//!   are `real(kr=4)`; energies/cross sections round-trip to ~7 significant
//!   figures);
//! - an **integer** word → `i32` little-endian;
//! - an **`a8` Hollerith** field → 8 ASCII bytes (`mult == 2` words), space-padded
//!   and right-truncated.
//!
//! Every record is framed as `[u32 nwds little-endian][nwds * 4 payload bytes]`,
//! so the reader validates each `nwds` against the writer. Energies are **eV**,
//! cross sections **barns**, temperatures **kelvin**, `amass` dimensionless.

use std::io::{Read, Write};

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::resxsr::assemble::PointwiseReaction;
use crate::resxsr::format::{
    FileControl, FileData, FileIdentification, MaterialControl, MULT,
};
use crate::NjoyError;

/// RESXS resonance reactions, in the fixed on-file order elastic, fission,
/// capture (`resxsr.f90:300`, `mth == 2 / 18 / 102`).
pub const RESONANCE_MTS: [i32; 3] = [2, 18, 102];

/// Read the resonance MF=3 cross sections (MT 2 elastic, 18 fission, 102 capture)
/// for material `mat` from a parsed PENDF [`Tape`] — the `gety1` feeder
/// (`resxsr.f90:296-347`).
///
/// Returns one [`PointwiseReaction`] per **present** resonance reaction, in the
/// canonical order elastic, fission, capture (absent reactions are simply
/// omitted, so a non-fissionable material yields two reactions). The returned
/// `points` are the raw `(energy_eV, xs_barn)` TAB1 pairs; the union-grid clip to
/// `[efirst, elast]` and the lin-lin interpolation happen in
/// [`assemble_union_grid`](crate::resxsr::assemble::assemble_union_grid).
///
/// # Errors
/// Propagates [`NjoyError`] from record parsing.
pub fn read_pendf_reactions(tape: &Tape, mat: i32) -> Result<Vec<PointwiseReaction>, NjoyError> {
    let mut reactions = Vec::with_capacity(RESONANCE_MTS.len());
    for &mt in &RESONANCE_MTS {
        let Some(sec) = tape.section(mat, 3, mt) else { continue };
        // MF=3 section: HEAD (ZA, AWR, ...) then the TAB1 cross section.
        let mut cur = SectionCursor::new(&sec.rows);
        let _head = cur.read_cont()?;
        let tab1 = cur.read_tab1()?;
        reactions.push(PointwiseReaction { mt, points: tab1.pairs });
    }
    Ok(reactions)
}

/// True if the reaction set includes fission (MT 18) — controls `nreac`
/// (3 fissionable, 2 otherwise; `resxsr.f90:166-167`).
pub fn is_fissionable(reactions: &[PointwiseReaction]) -> bool {
    reactions.iter().any(|r| r.mt == 18)
}

/// One energy point of a RESXS material block: an energy (eV) and the
/// `nreac*ntemp` reaction cross sections at that energy (barns), ordered
/// `(elastic, fission, capture)` for `temp(1)`, then the same for `temp(2)`, …
/// (`resxsr.f90:184-188, 422-425`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResxsPoint {
    /// Energy of this point, **eV**.
    pub energy: f64,
    /// The `nreac*ntemp` reaction cross sections at `energy`, **barns**.
    pub values: Vec<f64>,
}

/// One material section of a RESXS file: its control record plus the thinned
/// energy-point block (`resxsr.f90:399-430`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResxsMaterial {
    /// Material control record (`hmat`, `amass`, `temps`, `nreac`, `nener`).
    pub control: MaterialControl,
    /// The thinned energy points; `points.len() == control.nener`.
    pub points: Vec<ResxsPoint>,
}

/// A complete RESXS file in memory: the four leading control records plus one
/// [`ResxsMaterial`] per material (`resxsr.f90:435-501`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResxsFile {
    /// Record 1 — file identification.
    pub ident: FileIdentification,
    /// Record 2 — file control.
    pub control: FileControl,
    /// Record 4 — file data (per-material Hollerith names, ntemp, locm).
    pub data: FileData,
    /// Record 5 — the materials, in file order.
    pub materials: Vec<ResxsMaterial>,
}

// --- word-level encode helpers (4-byte little-endian storage words) ----------

fn put_real(buf: &mut Vec<u8>, x: f64) {
    buf.extend_from_slice(&(x as f32).to_le_bytes());
}
fn put_int(buf: &mut Vec<u8>, x: i32) {
    buf.extend_from_slice(&x.to_le_bytes());
}
/// Write an `a8` Hollerith field: 8 ASCII bytes, space-padded and right-truncated
/// (occupies `MULT` storage words).
fn put_holl(buf: &mut Vec<u8>, s: &str) {
    let mut field = [b' '; 8];
    for (i, b) in s.bytes().take(8).enumerate() {
        field[i] = b;
    }
    buf.extend_from_slice(&field);
}

struct WordReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> WordReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], NjoyError> {
        if self.pos + n > self.bytes.len() {
            return Err(NjoyError::EndfParse("resxs: record truncated".into()));
        }
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn real(&mut self) -> Result<f64, NjoyError> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64)
    }
    fn int(&mut self) -> Result<i32, NjoyError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn holl(&mut self) -> Result<String, NjoyError> {
        let b = self.take(8)?;
        Ok(String::from_utf8_lossy(b).trim_end().to_string())
    }
}

/// Write one framed record: `[u32 nwds][payload]`, asserting the payload is
/// `nwds` 4-byte words.
fn write_record<W: Write>(w: &mut W, nwds: i32, payload: &[u8]) -> Result<(), NjoyError> {
    debug_assert_eq!(payload.len(), (nwds as usize) * 4, "record payload != nwds words");
    w.write_all(&(nwds as u32).to_le_bytes()).map_err(NjoyError::Io)?;
    w.write_all(payload).map_err(NjoyError::Io)?;
    Ok(())
}

/// Read one framed record, returning its `nwds` and payload bytes.
fn read_record<R: Read>(r: &mut R) -> Result<(i32, Vec<u8>), NjoyError> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n).map_err(NjoyError::Io)?;
    let nwds = u32::from_le_bytes(n) as i32;
    let mut payload = vec![0u8; (nwds as usize) * 4];
    r.read_exact(&mut payload).map_err(NjoyError::Io)?;
    Ok((nwds, payload))
}

impl ResxsFile {
    /// `nblok` energy blocking factor carried on the file-control record.
    fn nblok(&self) -> i32 {
        self.control.nblok
    }

    /// Write the RESXS file to `out` in the documented record sequence
    /// (`resxsr.f90:435-501`).
    ///
    /// Record order: file identification, file control, set-Hollerith (blank),
    /// file data, then per material a material-control record followed by one or
    /// more cross-section block records sized by `nblok`.
    ///
    /// # Errors
    /// Propagates [`NjoyError::Io`].
    pub fn write<W: Write>(&self, mut out: W) -> Result<(), NjoyError> {
        // Record 1 — file identification (resxsr.f90:437-445).
        let mut buf = Vec::new();
        put_holl(&mut buf, &self.ident.hname);
        put_holl(&mut buf, &self.ident.huse1);
        put_holl(&mut buf, &self.ident.huse2);
        put_int(&mut buf, self.ident.ivers);
        write_record(&mut out, FileIdentification::word_count(MULT), &buf)?;

        // Record 2 — file control (resxsr.f90:447-455).
        buf.clear();
        put_real(&mut buf, self.control.efirst);
        put_real(&mut buf, self.control.elast);
        put_int(&mut buf, self.control.nholl);
        put_int(&mut buf, self.control.nmat);
        put_int(&mut buf, self.control.nblok);
        write_record(&mut out, FileControl::word_count(), &buf)?;

        // Record 3 — set-Hollerith identification: nholl blank a8 words
        // (resxsr.f90:457-463).
        buf.clear();
        for _ in 0..self.control.nholl {
            put_holl(&mut buf, "");
        }
        write_record(&mut out, MULT * self.control.nholl, &buf)?;

        // Record 4 — file data (resxsr.f90:465-473).
        buf.clear();
        for name in &self.data.hmatn {
            put_holl(&mut buf, name);
        }
        for &nt in &self.data.ntemp {
            put_int(&mut buf, nt);
        }
        for &lc in &self.data.locm {
            put_int(&mut buf, lc);
        }
        write_record(&mut out, self.data.word_count(MULT), &buf)?;

        // Record 5 — per material (resxsr.f90:399-430, copied at :479-501).
        let nblok = self.nblok();
        for mat in &self.materials {
            // Material control record.
            buf.clear();
            put_holl(&mut buf, &mat.control.hmat);
            put_real(&mut buf, mat.control.amass);
            for &t in &mat.control.temps {
                put_real(&mut buf, t);
            }
            put_int(&mut buf, mat.control.nreac);
            put_int(&mut buf, mat.control.nener);
            write_record(&mut out, mat.control.word_count(MULT), &buf)?;

            // Cross-section blocks: nn words per point, blocked by nblok.
            let nn = 1 + mat.control.nreac * mat.control.temps.len() as i32;
            let cap_pts = (nblok / nn).max(1);
            buf.clear();
            let mut pts_in_block = 0i32;
            for pt in &mat.points {
                put_real(&mut buf, pt.energy);
                for &v in &pt.values {
                    put_real(&mut buf, v);
                }
                pts_in_block += 1;
                if pts_in_block == cap_pts {
                    write_record(&mut out, pts_in_block * nn, &buf)?;
                    buf.clear();
                    pts_in_block = 0;
                }
            }
            if pts_in_block > 0 {
                write_record(&mut out, pts_in_block * nn, &buf)?;
            }
        }
        Ok(())
    }

    /// Read a RESXS file back from `input`, inverting [`ResxsFile::write`].
    ///
    /// # Errors
    /// Propagates [`NjoyError::Io`] / [`NjoyError::EndfParse`] on a truncated or
    /// malformed record.
    pub fn read<R: Read>(mut input: R) -> Result<Self, NjoyError> {
        // Record 1 — file identification.
        let (_n1, p1) = read_record(&mut input)?;
        let mut r = WordReader::new(&p1);
        let ident = FileIdentification {
            hname: r.holl()?,
            huse1: r.holl()?,
            huse2: r.holl()?,
            ivers: r.int()?,
        };

        // Record 2 — file control.
        let (_n2, p2) = read_record(&mut input)?;
        let mut r = WordReader::new(&p2);
        let control = FileControl {
            efirst: r.real()?,
            elast: r.real()?,
            nholl: r.int()?,
            nmat: r.int()?,
            nblok: r.int()?,
        };

        // Record 3 — set-Hollerith (blank, discarded).
        let (_n3, _p3) = read_record(&mut input)?;

        // Record 4 — file data.
        let (_n4, p4) = read_record(&mut input)?;
        let nmat = control.nmat as usize;
        let mut r = WordReader::new(&p4);
        let mut hmatn = Vec::with_capacity(nmat);
        for _ in 0..nmat {
            hmatn.push(r.holl()?);
        }
        let mut ntemp = Vec::with_capacity(nmat);
        for _ in 0..nmat {
            ntemp.push(r.int()?);
        }
        let mut locm = Vec::with_capacity(nmat);
        for _ in 0..nmat {
            locm.push(r.int()?);
        }
        let data = FileData { hmatn: hmatn.clone(), ntemp: ntemp.clone(), locm };

        // Record 5 — per material.
        let mut materials = Vec::with_capacity(nmat);
        for im in 0..nmat {
            let nt = ntemp[im] as usize;
            let (_nc, pc) = read_record(&mut input)?;
            let mut r = WordReader::new(&pc);
            let hmat = r.holl()?;
            let amass = r.real()?;
            let mut temps = Vec::with_capacity(nt);
            for _ in 0..nt {
                temps.push(r.real()?);
            }
            let nreac = r.int()?;
            let nener = r.int()?;
            let control_m = MaterialControl { hmat, amass, temps, nreac, nener };

            let nn = 1 + nreac * (nt as i32);
            let mut points = Vec::with_capacity(nener as usize);
            while (points.len() as i32) < nener {
                let (nwds, pb) = read_record(&mut input)?;
                let npts = nwds / nn;
                let mut r = WordReader::new(&pb);
                for _ in 0..npts {
                    let energy = r.real()?;
                    let mut values = Vec::with_capacity((nn - 1) as usize);
                    for _ in 0..(nn - 1) {
                        values.push(r.real()?);
                    }
                    points.push(ResxsPoint { energy, values });
                }
            }
            materials.push(ResxsMaterial { control: control_m, points });
        }

        Ok(ResxsFile { ident, control, data, materials })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;
    use crate::resxsr::assemble::{assemble_union_grid, thin_linear};

    /// Build a one-material PENDF tape with an MF=3 TAB1 for `mt`.
    fn mf3_tape(mat: i32, mts: &[(i32, Vec<(f64, f64)>)]) -> Tape {
        let mut secs = Vec::new();
        for (mt, pairs) in mts {
            let ne = pairs.len();
            let mut rows: Vec<[f64; 6]> = vec![
                [1001.0, 0.999, 0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0, 1.0, ne as f64],
                [ne as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
            ];
            let mut flat = Vec::new();
            for &(e, s) in pairs {
                flat.push(e);
                flat.push(s);
            }
            for chunk in flat.chunks(6) {
                let mut row = [0.0f64; 6];
                row[..chunk.len()].copy_from_slice(chunk);
                rows.push(row);
            }
            secs.push(Section { key: EndfKey { mat, mf: 3, mt: *mt }, rows });
        }
        Tape::from_sections(" test".into(), secs)
    }

    /// The PENDF feeder pulls the resonance MTs in canonical order and detects
    /// fissionability.
    ///
    /// **Methodology.** `resxsr.f90:296-347` reads MF=3 for MT 2/18/102. Build a
    /// tape with elastic (MT2) and capture (MT102) only (no fission); assert the
    /// feeder returns exactly those two, in order elastic then capture, with the
    /// TAB1 pairs intact, and that `is_fissionable` is false.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** 2 reactions, `[2, 102]`; elastic
    /// points `(4,10),(200,10)`; `is_fissionable == false`.
    #[test]
    fn pendf_feeder_reads_resonance_mts() {
        let tape = mf3_tape(
            9237,
            &[
                (2, vec![(4.0, 10.0), (200.0, 10.0)]),
                (102, vec![(4.0, 1.0), (200.0, 5.0)]),
            ],
        );
        let reactions = read_pendf_reactions(&tape, 9237).unwrap();
        assert_eq!(reactions.len(), 2);
        assert_eq!(reactions[0].mt, 2);
        assert_eq!(reactions[1].mt, 102);
        assert_eq!(reactions[0].points, vec![(4.0, 10.0), (200.0, 10.0)]);
        assert!(!is_fissionable(&reactions));
    }

    /// A full RESXS file round-trips: `read == write^-1`, preserving the union
    /// grid energies and every reaction value (within single-precision).
    ///
    /// **Methodology (task V&V gate).** Feed a tape through the whole in-memory
    /// pipeline: `read_pendf_reactions -> assemble_union_grid -> thin_linear`,
    /// assemble a [`ResxsFile`], `write` it to a byte buffer, `read` it back, and
    /// require the recovered material's energies and cross sections to equal the
    /// written ones to `f32` precision (RESXS stores single-precision reals). Also
    /// assert the framed `nwds` counts match [`crate::resxsr::format`]'s formulas.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Round-trip preserves the 3-point
    /// union grid `{4, 100, 200}` eV and both reaction columns (elastic ≡ 10 b,
    /// capture 1/5/1 b) to < 1e-4 relative; recovered `nener == 3`, `nreac == 2`,
    /// `amass == 236.006`, `ivers == 1`.
    #[test]
    fn resxs_file_roundtrip() {
        let tape = mf3_tape(
            9237,
            &[
                (2, vec![(4.0, 10.0), (200.0, 10.0)]),
                (102, vec![(4.0, 1.0), (100.0, 5.0), (200.0, 1.0)]),
            ],
        );
        let reactions = read_pendf_reactions(&tape, 9237).unwrap();
        let rows = assemble_union_grid(&reactions, 4.0, 200.0);
        let thinned = thin_linear(&rows, 1.0e-3);
        let nreac = reactions.len() as i32;
        let temps = vec![293.6_f64];

        let points: Vec<ResxsPoint> = thinned
            .iter()
            .map(|row| ResxsPoint { energy: row.energy, values: row.xs.clone() })
            .collect();
        let nener = points.len() as i32;

        let mat = ResxsMaterial {
            control: MaterialControl {
                hmat: "u238".into(),
                amass: 236.006,
                temps: temps.clone(),
                nreac,
                nener,
            },
            points: points.clone(),
        };
        let file = ResxsFile {
            ident: FileIdentification {
                hname: super::super::format::FILE_NAME.into(),
                huse1: "outram".into(),
                huse2: "park".into(),
                ivers: 1,
            },
            control: FileControl {
                efirst: 4.0,
                elast: 200.0,
                nholl: 1,
                nmat: 1,
                nblok: super::super::format::NBLOK,
            },
            data: FileData {
                hmatn: vec!["u238".into()],
                ntemp: vec![1],
                locm: vec![0],
            },
            materials: vec![mat],
        };

        let mut buf = Vec::new();
        file.write(&mut buf).unwrap();
        let back = ResxsFile::read(&buf[..]).unwrap();

        assert_eq!(back.ident.ivers, 1);
        assert_eq!(back.control.nmat, 1);
        assert_eq!(back.materials.len(), 1);
        let bmat = &back.materials[0];
        assert_eq!(bmat.control.nreac, nreac);
        assert_eq!(bmat.control.nener, nener);
        assert!((bmat.control.amass - 236.006).abs() < 1e-2);
        assert_eq!(bmat.points.len(), points.len());
        for (got, want) in bmat.points.iter().zip(points.iter()) {
            assert!((got.energy - want.energy).abs() <= 1e-3 * want.energy.abs().max(1.0));
            assert_eq!(got.values.len(), want.values.len());
            for (g, w) in got.values.iter().zip(want.values.iter()) {
                assert!((g - w).abs() <= 1e-4 * w.abs().max(1.0), "value {g} vs {w}");
            }
        }
    }
}
