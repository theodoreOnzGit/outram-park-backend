// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine wminit`, l.246-421 — the GENDF header scan (group bounds,
//     temperatures, sigma-zero count, the MT=18/MT=252 presence checks).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The GENDF as WIMSR walks it: one [`TempBlock`] per temperature (a GENDF
//! written for several temperatures repeats the material, MF=1/451 first,
//! once per temperature) with its reaction sections in file order. Built
//! on [`crate::dtfr::gendf::GendfGroupRecord`] — the same `lz`-relative
//! data layout `wimsr.f90` indexes with `loca = l + lz + …`.

use crate::dtfr::gendf::GendfGroupRecord;
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// One reaction section of a temperature block.
#[derive(Debug, Clone)]
pub struct GendfSection {
    pub mf: i32,
    pub mt: i32,
    /// `nl`, `nz` of the section HEAD.
    pub nl: i32,
    pub nz: i32,
    /// One record per initial group, in file order.
    pub records: Vec<GendfGroupRecord>,
}

/// One temperature's material block.
#[derive(Debug, Clone)]
pub struct TempBlock {
    /// `tempr(jtemp)` — the header LIST's `C1` \[K\].
    pub temp: f64,
    /// `nz` (sigma-zero count) and `ntw` of the HEAD.
    pub nz: i32,
    pub ntw: i32,
    /// The `nz` sigma-zeros as stored (`scr(6+ntw+i)`, `i = 1..nz`).
    pub sigz: Vec<f64>,
    /// `ngn` of the LIST.
    pub ngn: i32,
    /// The `ngn+1` neutron group bounds as stored (ascending).
    pub egn: Vec<f64>,
    pub sections: Vec<GendfSection>,
}

/// The material's header quantities `wminit` keeps (`wimsr.f90:262-275`).
#[derive(Debug, Clone)]
pub struct GendfMaterial {
    /// `iza`, `iznum = iza/1000`, `awr`.
    pub iza: i32,
    pub iznum: i32,
    pub awr: f64,
    /// `egb(1:ngnd+1)` — the group bounds in WIMS order (descending,
    /// `egb(i) = scr(ngnd1-i+i1)`, l.271-273).
    pub egb: Vec<f64>,
    /// One block per temperature, in file order.
    pub blocks: Vec<TempBlock>,
}

/// Read every temperature block of `mat` from a GENDF `tape`
/// (`wminit`, `wimsr.f90:246-421`, minus the prints).
///
/// # Errors
/// `EndfParse` if the material is absent ("desired material is not on
/// gendf tape") or its group count is not `ngnd` ("incorrect group
/// structure").
pub fn read_material(tape: &Tape, mat: i32, ngnd: usize) -> Result<GendfMaterial, NjoyError> {
    let mut blocks: Vec<TempBlock> = Vec::new();
    let mut iza = 0i32;
    let mut awr = 0.0f64;
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat) {
        if sec.key.mf == 1 && sec.key.mt == 451 {
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            let list = cur.read_list()?;
            if blocks.is_empty() {
                iza = head.c1.round() as i32;
                awr = head.c2;
            }
            let nz = head.l2;
            let ntw = head.n2;
            let ngn = list.head.l1;
            let off = ntw.max(0) as usize;
            let nzu = nz.max(0) as usize;
            let sigz = list
                .data
                .get(off..off + nzu)
                .map(|s| s.to_vec())
                .unwrap_or_default();
            let ngn1 = (ngn.max(0) as usize) + 1;
            let egn = list
                .data
                .get(off + nzu..off + nzu + ngn1)
                .map(|s| s.to_vec())
                .ok_or_else(|| {
                    NjoyError::EndfParse("wimsr::wminit: header LIST truncated".into())
                })?;
            blocks.push(TempBlock {
                temp: list.head.c1,
                nz,
                ntw,
                sigz,
                ngn,
                egn,
                sections: Vec::new(),
            });
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        if sec.key.mf < 3 || sec.key.mt == 0 {
            continue;
        }
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let nl = head.l1;
        let nz = head.l2;
        let mut records = Vec::new();
        while cur.remaining() > 0 {
            let list = cur.read_list()?;
            records.push(GendfGroupRecord {
                nl,
                nz,
                ng2: list.head.l1,
                ig2lo: list.head.l2,
                ig: list.head.n2,
                data: list.data,
            });
        }
        block.sections.push(GendfSection {
            mf: sec.key.mf,
            mt: sec.key.mt,
            nl,
            nz,
            records,
        });
    }
    let Some(first) = blocks.first() else {
        return Err(NjoyError::EndfParse(
            "wimsr::wminit: desired material is not on gendf tape".into(),
        ));
    };
    if first.ngn != ngnd as i32 {
        return Err(NjoyError::EndfParse(format!(
            "wimsr::wminit: incorrect group structure (gendf has {} groups, deck says {ngnd})",
            first.ngn
        )));
    }
    let mut egb = first.egn.clone();
    egb.reverse();
    Ok(GendfMaterial {
        iza,
        iznum: iza / 1000,
        awr,
        egb,
        blocks,
    })
}
