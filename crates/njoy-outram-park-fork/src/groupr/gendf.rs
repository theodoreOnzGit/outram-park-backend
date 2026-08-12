// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GENDF group-constant records — the multigroup output tape of GROUPR/GAMINR.
//!
//! GROUPR writes its multigroup results to a **GENDF** tape: an ENDF-formatted
//! file whose sections carry group constants instead of pointwise data. This
//! module models the record layout GROUPR emits for a **1-D cross-section
//! vector** (`mfh = 3`, `mfh = 23` for GAMINR) and round-trips it through the
//! crate's ENDF CONT/LIST record substrate.
//!
//! # Record layout (from `groupr.f90:882-946`)
//!
//! For one reaction (MF, MT) at one temperature, GROUPR writes:
//!
//! - a **HEAD (CONT) record** with `C1 = ZA`, `C2 = ZAM` (or extended `fzam`),
//!   `L1 = NL` (Legendre orders), `L2 = NZ` (dilutions), `N1 = LRFLAG`,
//!   `N2 = NGN` (number of initial groups) — `groupr.f90:882-891`; and
//! - one **LIST record per initial group `ig`** that has non-zero data, with
//!   `C1 = temperature`, `C2 = 0`, `L1 = NG2` (secondary count; 2 for a vector:
//!   flux + cross section), `L2 = IG2LO` (lowest secondary group; 0 for a
//!   vector), `N1 = NW = NL*NZ*NG2` (word count), `N2 = ig` (the initial group
//!   index), followed by the `NW` data words `ans(il,iz,it)`
//!   (`groupr.f90:900-946`).
//!
//! For the 1-D vector case (`NL = NZ = 1`, `NG2 = 2`) each group LIST holds two
//! words: `[flux_g, sigma_g]` — the group flux `∫phi` (`ans(1,1,1)`) and the
//! group cross section `sigma_g = ∫sigma*phi / ∫phi` (`ans(1,1,2)`, after `dspla`
//! divides). See [`crate::groupr::panel`] for how those two numbers are made.
//!
//! # Scope
//!
//! This is the **vector** GENDF writer/reader: enough to serialize a 1-D group
//! cross section (or any group-flux + group-value pair per group) to the CONT +
//! LIST record structure and read it back losslessly ([`GendfSection::to_rows`]
//! / [`GendfSection::from_rows`], and the [`GendfTape`] container). The
//! group-to-group **matrix** records (`mfh = 6`/`mfh = 26`, `IG2LO > 0`,
//! `NG2 > 2`) share the same LIST layout but their construction (the feed-function
//! kinematics) is not ported — see [`crate::groupr::panel`] and
//! [`crate::groupr::kinematics`].

use crate::endf::records::{Cont, List, SectionCursor};
use crate::NjoyError;

/// One GENDF group record: the group constants for a single initial group.
///
/// The Rust form of one GROUPR output LIST record (`groupr.f90:900-946`). For a
/// 1-D vector, `ng2 = 2`, `ig2lo = 0`, and `data = [flux_g, sigma_g]`.
#[derive(Debug, Clone, PartialEq)]
pub struct GendfGroupRecord {
    /// Initial energy group index `ig` (1-based), the LIST `N2` field.
    pub ig: i32,
    /// Lowest secondary group `IG2LO` (LIST `L2`); `0` for a 1-D vector.
    pub ig2lo: i32,
    /// Secondary count `NG2` (LIST `L1`); `2` for a vector (flux + cross section).
    pub ng2: i32,
    /// The `NW = NL*NZ*NG2` data words `ans(il,iz,it)` in `panel`'s storage order
    /// (`it` outermost, then `iz`, then `il`). For a vector: `[flux_g, sigma_g]`.
    pub data: Vec<f64>,
}

impl GendfGroupRecord {
    /// Build the vector group record `[flux_g, sigma_g]` for group `ig`.
    ///
    /// - `ig` — 1-based initial group index.
    /// - `flux` — the group flux `∫phi` \[arb·eV\] (`ans(1,1,1)`).
    /// - `sigma` — the group cross section `sigma_g` \[barn\] (`ans(1,1,2)`).
    pub fn vector(ig: i32, flux: f64, sigma: f64) -> Self {
        GendfGroupRecord {
            ig,
            ig2lo: 0,
            ng2: 2,
            data: vec![flux, sigma],
        }
    }

    /// The group cross section `sigma_g` \[barn\] for a vector record — the last
    /// data word (`ans(1,1,2)`), or `0.0` if the record is empty.
    pub fn sigma(&self) -> f64 {
        self.data.get(1).copied().unwrap_or(0.0)
    }

    /// The group flux `∫phi` for a vector record — the first data word
    /// (`ans(1,1,1)`), or `0.0` if empty.
    pub fn flux(&self) -> f64 {
        self.data.first().copied().unwrap_or(0.0)
    }
}

/// A GENDF section: one reaction's (MF, MT) group constants at one temperature.
///
/// The Rust form of one GROUPR output section — the HEAD (CONT) record plus its
/// per-group LIST records (`groupr.f90:882-946`). Construct a 1-D section with
/// [`GendfSection::vector`], serialize with [`GendfSection::to_rows`], and parse
/// with [`GendfSection::from_rows`].
#[derive(Debug, Clone, PartialEq)]
pub struct GendfSection {
    /// ENDF file number `MF` (`mfh`): 3 for a GROUPR vector, 23 for GAMINR.
    pub mf: i32,
    /// ENDF section number `MT` (the reaction).
    pub mt: i32,
    /// `ZA = 1000*Z + A` of the material (HEAD `C1`).
    pub za: f64,
    /// `ZAM` (or extended `fzam`) isomer/product tag (HEAD `C2`).
    pub zam: f64,
    /// Number of Legendre orders `NL` (HEAD `L1`); `1` for a vector.
    pub nl: i32,
    /// Number of background dilutions `NZ` (HEAD `L2`); `1` for a vector.
    pub nz: i32,
    /// `LRFLAG` particle-emission flag (HEAD `N1`); `0` unless a breakup channel.
    pub lrflag: i32,
    /// Number of initial energy groups `NGN` (HEAD `N2`).
    pub num_groups: i32,
    /// Temperature \[K\] this section was averaged at (each LIST `C1`).
    pub temperature_k: f64,
    /// The per-group records, in ascending `ig` order. Only groups with
    /// non-zero data are written by GROUPR; the same convention holds here.
    pub records: Vec<GendfGroupRecord>,
}

impl GendfSection {
    /// Build a 1-D vector GENDF section from group flux + cross-section columns.
    ///
    /// - `mf`/`mt` — the reaction identity (`mf = 3` for a GROUPR vector).
    /// - `za`/`zam` — material ZA and isomer tag.
    /// - `num_groups` — total number of initial groups `NGN` in the structure.
    /// - `temperature_k` — averaging temperature \[K\].
    /// - `groups` — `(ig, flux_g, sigma_g)` triples for the groups with data;
    ///   `ig` is 1-based. Groups omitted from the slice are treated as empty
    ///   (not written), matching GROUPR's "write only non-zero groups" rule.
    ///
    /// Sets `NL = NZ = 1`, `LRFLAG = 0`.
    pub fn vector(
        mf: i32,
        mt: i32,
        za: f64,
        zam: f64,
        num_groups: i32,
        temperature_k: f64,
        groups: &[(i32, f64, f64)],
    ) -> Self {
        let records = groups
            .iter()
            .map(|&(ig, flux, sigma)| GendfGroupRecord::vector(ig, flux, sigma))
            .collect();
        GendfSection {
            mf,
            mt,
            za,
            zam,
            nl: 1,
            nz: 1,
            lrflag: 0,
            num_groups,
            temperature_k,
            records,
        }
    }

    /// Serialize this section to ENDF data rows (`[f64; 6]` per line) — the HEAD
    /// CONT record followed by one LIST record per group.
    ///
    /// The rows are the section *body* (no MAT/MF/MT column metadata, which is
    /// line-position bookkeeping): a faithful in-memory image of the GENDF
    /// section that [`GendfSection::from_rows`] reads back exactly.
    pub fn to_rows(&self) -> Vec<[f64; 6]> {
        let mut rows = Vec::with_capacity(1 + self.records.len() * 2);
        // HEAD (CONT): C1=ZA, C2=ZAM, L1=NL, L2=NZ, N1=LRFLAG, N2=NGN.
        rows.push([
            self.za,
            self.zam,
            self.nl as f64,
            self.nz as f64,
            self.lrflag as f64,
            self.num_groups as f64,
        ]);
        for rec in &self.records {
            // LIST head: C1=temp, C2=0, L1=NG2, L2=IG2LO, N1=NW, N2=ig.
            let nw = rec.data.len() as i32;
            rows.push([
                self.temperature_k,
                0.0,
                rec.ng2 as f64,
                rec.ig2lo as f64,
                nw as f64,
                rec.ig as f64,
            ]);
            // Data words, packed 6 per row, zero-padded on the last row.
            for chunk in rec.data.chunks(6) {
                let mut row = [0.0_f64; 6];
                row[..chunk.len()].copy_from_slice(chunk);
                rows.push(row);
            }
        }
        rows
    }

    /// Parse a GENDF section body (as produced by [`GendfSection::to_rows`]) back
    /// into a [`GendfSection`], given its `mf` and `mt` (which live in the ENDF
    /// line metadata, not the record fields).
    ///
    /// Reads the HEAD CONT record, then LIST records until the rows are consumed.
    /// The temperature is taken from the first LIST record (`C1`); a section with
    /// no group records keeps `temperature_k = 0.0`.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if the rows are truncated mid-record.
    pub fn from_rows(mf: i32, mt: i32, rows: &[[f64; 6]]) -> Result<Self, NjoyError> {
        let mut cur = SectionCursor::new(rows);
        let head: Cont = cur.read_cont()?;
        let mut records = Vec::new();
        let mut temperature_k = 0.0;
        let mut first = true;
        while cur.remaining() > 0 {
            let list: List = cur.read_list()?;
            if first {
                temperature_k = list.head.c1;
                first = false;
            }
            records.push(GendfGroupRecord {
                ig: list.head.n2,
                ig2lo: list.head.l2,
                ng2: list.head.l1,
                data: list.data,
            });
        }
        Ok(GendfSection {
            mf,
            mt,
            za: head.c1,
            zam: head.c2,
            nl: head.l1,
            nz: head.l2,
            lrflag: head.n1,
            num_groups: head.n2,
            temperature_k,
            records,
        })
    }

    /// The group cross-section vector `sigma_g` \[barn\], one entry per group
    /// `1..=num_groups`, with zeros for groups that carry no record.
    ///
    /// Convenience for consumers (e.g. an MGXS collapse check) that want a dense
    /// vector rather than the sparse record list.
    pub fn sigma_vector(&self) -> Vec<f64> {
        let n = self.num_groups.max(0) as usize;
        let mut v = vec![0.0; n];
        for rec in &self.records {
            let idx = (rec.ig - 1) as usize;
            if idx < v.len() {
                v[idx] = rec.sigma();
            }
        }
        v
    }
}

/// An in-memory GENDF tape: a material number and its group-constant sections.
///
/// A lightweight container mirroring GROUPR's output tape (one MAT, many
/// MF/MT sections). The framing is the crate's own in-memory representation —
/// it is **not** claimed to be byte-identical to an NJOY ASCII GENDF file, only
/// to round-trip the group-constant records losslessly through the ENDF CONT +
/// LIST record layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GendfTape {
    /// Material number `MAT`.
    pub mat: i32,
    /// The group-constant sections, in write order.
    pub sections: Vec<GendfSection>,
}

impl GendfTape {
    /// A new, empty GENDF tape for material `mat`.
    pub fn new(mat: i32) -> Self {
        GendfTape {
            mat,
            sections: Vec::new(),
        }
    }

    /// Append a section to the tape.
    pub fn push(&mut self, section: GendfSection) {
        self.sections.push(section);
    }

    /// The first section matching `(mf, mt)`, if present.
    pub fn section(&self, mf: i32, mt: i32) -> Option<&GendfSection> {
        self.sections.iter().find(|s| s.mf == mf && s.mt == mt)
    }

    /// Serialize the whole tape to a flat row image and read it back — the
    /// building blocks of a tape round trip.
    ///
    /// Each section is framed by a one-row directory record `[mat, mf, mt,
    /// n_rows, 0, 0]` followed by that section's [`GendfSection::to_rows`]. The
    /// directory rows carry the MF/MT that a real GENDF file keeps in its line
    /// metadata. Parse the image back with [`GendfTape::from_image`].
    pub fn to_image(&self) -> Vec<[f64; 6]> {
        let mut image = Vec::new();
        // Tape header: [mat, n_sections, 0, 0, 0, 0].
        image.push([
            self.mat as f64,
            self.sections.len() as f64,
            0.0,
            0.0,
            0.0,
            0.0,
        ]);
        for s in &self.sections {
            let body = s.to_rows();
            image.push([
                self.mat as f64,
                s.mf as f64,
                s.mt as f64,
                body.len() as f64,
                0.0,
                0.0,
            ]);
            image.extend_from_slice(&body);
        }
        image
    }

    /// Parse a tape image produced by [`GendfTape::to_image`].
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if the image is truncated or a section's row
    /// count runs past the end of the image.
    pub fn from_image(image: &[[f64; 6]]) -> Result<Self, NjoyError> {
        let bad = |m: &str| NjoyError::EndfParse(m.into());
        let header = image.first().ok_or_else(|| bad("empty GENDF image"))?;
        let mat = header[0].round() as i32;
        let n_sections = header[1].round() as usize;
        let mut pos = 1usize;
        let mut sections = Vec::with_capacity(n_sections);
        for _ in 0..n_sections {
            let dir = image
                .get(pos)
                .ok_or_else(|| bad("truncated GENDF directory"))?;
            let mf = dir[1].round() as i32;
            let mt = dir[2].round() as i32;
            let n_rows = dir[3].round() as usize;
            pos += 1;
            let end = pos
                .checked_add(n_rows)
                .filter(|&e| e <= image.len())
                .ok_or_else(|| bad("GENDF section overruns image"))?;
            let section = GendfSection::from_rows(mf, mt, &image[pos..end])?;
            sections.push(section);
            pos = end;
        }
        Ok(GendfTape { mat, sections })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1-D vector GENDF section round-trips through the CONT + LIST record
    /// layout without loss.
    ///
    /// **Methodology.** Build a `mf=3, mt=2` (elastic) section over 4 groups with
    /// three non-zero group records (flux, sigma per group), serialize with
    /// [`GendfSection::to_rows`], parse back with [`GendfSection::from_rows`], and
    /// assert structural equality of every field and record. This is the core
    /// "write → read preserves the group vector" property.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** parsed section == original
    /// (HEAD ZA/ZAM/NL/NZ/NGN and all three LIST records identical).
    #[test]
    fn vector_section_round_trips() {
        let section = GendfSection::vector(
            3,
            2,
            92238.0,
            0.0,
            4,
            293.6,
            &[(1, 1.0e5, 5.1), (2, 8.0e4, 6.3), (4, 2.0e4, 7.7)],
        );
        let rows = section.to_rows();
        let back = GendfSection::from_rows(3, 2, &rows).unwrap();
        assert_eq!(back, section, "section survives the record round trip");
        // The dense group vector places sigma at the right group index (1-based).
        let v = back.sigma_vector();
        assert_eq!(v.len(), 4);
        assert_eq!(v, vec![5.1, 6.3, 0.0, 7.7]);
    }

    /// The HEAD and LIST fields land in the documented ENDF slots.
    ///
    /// **Methodology.** Serialize a one-group section and check the raw rows:
    /// HEAD `[ZA, ZAM, NL, NZ, LRFLAG, NGN]` and the LIST head
    /// `[temp, 0, NG2, IG2LO, NW, ig]`, plus the two data words `[flux, sigma]`.
    /// Guards the record layout against silent field-order drift.
    ///
    /// **Result (2026-07-15).** rows match `groupr.f90:882-891` (HEAD) and
    /// `:900-908` (LIST) exactly.
    #[test]
    fn record_fields_are_in_endf_slots() {
        let section = GendfSection::vector(3, 102, 92238.0, 1.0, 175, 900.0, &[(7, 2.5, 9.9)]);
        let rows = section.to_rows();
        // HEAD
        assert_eq!(rows[0][0], 92238.0);
        assert_eq!(rows[0][1], 1.0);
        assert_eq!(rows[0][2], 1.0); // NL
        assert_eq!(rows[0][3], 1.0); // NZ
        assert_eq!(rows[0][4], 0.0); // LRFLAG
        assert_eq!(rows[0][5], 175.0); // NGN
                                       // LIST head
        assert_eq!(rows[1][0], 900.0); // temperature
        assert_eq!(rows[1][2], 2.0); // NG2
        assert_eq!(rows[1][3], 0.0); // IG2LO
        assert_eq!(rows[1][4], 2.0); // NW
        assert_eq!(rows[1][5], 7.0); // ig
                                     // Data
        assert_eq!(rows[2][0], 2.5); // flux
        assert_eq!(rows[2][1], 9.9); // sigma
    }

    /// A multi-section GENDF tape round-trips through the flat image framing.
    ///
    /// **Methodology.** Build a tape with two sections (`mf=3 mt=2` and
    /// `mf=3 mt=102`), serialize with [`GendfTape::to_image`], parse back with
    /// [`GendfTape::from_image`], and assert the tape (MAT + both sections)
    /// compares equal. Also assert a truncated image is rejected cleanly.
    ///
    /// **Result (2026-07-15).** parsed tape == original; a clipped image returns
    /// `EndfParse`.
    #[test]
    fn tape_image_round_trips_and_rejects_truncation() {
        let mut tape = GendfTape::new(9237);
        tape.push(GendfSection::vector(
            3,
            2,
            92238.0,
            0.0,
            3,
            293.6,
            &[(1, 1.0e5, 5.0), (2, 9.0e4, 6.0)],
        ));
        tape.push(GendfSection::vector(
            3,
            102,
            92238.0,
            0.0,
            3,
            293.6,
            &[(3, 4.0e4, 0.7)],
        ));
        let image = tape.to_image();
        let back = GendfTape::from_image(&image).unwrap();
        assert_eq!(back, tape, "tape survives the image round trip");
        assert_eq!(back.section(3, 102).unwrap().records[0].sigma(), 0.7);

        // Truncated image (drop the last two rows) must fail, not panic.
        let clipped = &image[..image.len() - 2];
        assert!(GendfTape::from_image(clipped).is_err());
    }
}
