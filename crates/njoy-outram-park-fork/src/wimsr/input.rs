// Ported from NJOY2016 `src/wimsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine wimsr`, l.51-244 — the input cards and their defaults.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The WIMSR deck (`wimsr.f90:51-140`) as an owned struct — cards 2–8 with
//! upstream's defaults; card 1 (unit numbers) is the caller's tape.

/// Cards 2–8 of the WIMSR deck.
#[derive(Debug, Clone)]
pub struct WimsrInput {
    /// Card 2 `iprint` (0 minimum, 1 regular, 2 with intermediate results)
    /// — only affects what upstream prints; kept for the record.
    pub iprint: i32,
    /// Card 2 `iverw`: 4 = WIMS-D (default), 5 = WIMS-E.
    pub iverw: i32,
    /// Card 2a: `ngnd` groups, `nfg` fast groups, `nrg` resonance groups,
    /// `igref` reference group (`igroup = 9`); `igroup = 0` is the 69-group
    /// WIMS structure `(69, 14, 13, 14)` (`wimsr.f90:176-186`).
    pub ngnd: usize,
    pub nfg: usize,
    pub nrg: usize,
    pub igref: usize,
    /// Card 3: ENDF `mat`, WIMS identifier `rdfid` (`nfid = nint(rdfid)`),
    /// `iburn` (-1 suppress, 0 none, 1 burnup cards 5/6).
    pub mat: i32,
    pub rdfid: f64,
    pub iburn: i32,
    /// Card 4.
    pub ntemp: usize,
    pub nsigz: usize,
    pub sgref: f64,
    pub ires: usize,
    pub sigp: f64,
    pub mti: i32,
    pub mtc: i32,
    /// `ip1opt`: 0 = include P1 matrices, 1 = correct the P0 in-groups
    /// (default).
    pub ip1opt: i32,
    pub inorf: i32,
    pub isof: i32,
    pub ifprod: i32,
    /// Card 8 (`jp1 > 0`): the leading entries of the current spectrum.
    pub p1flx_user: Vec<f64>,
    /// Cards 5/6 (`iburn > 0`): `efiss` and the `(identa, yield)` pairs in
    /// card order — `ntis` entries: capture product, decay product, then
    /// the fission products (`wimsr.f90:228-240`).
    pub efiss: f64,
    pub burn: Vec<(i32, f64)>,
    /// Card 7: the `nrg` Goldstein–Cohen lambdas.
    pub glam: Vec<f64>,
}

impl WimsrInput {
    /// `nfid = nint(rdfid)` (`wimsr.f90:191`).
    pub fn nfid(&self) -> i32 {
        self.rdfid.round() as i32
    }

    /// `ifprod` after the card-4 normalisation (`wimsr.f90:198-201`):
    /// `1` for a fission product without, `2` with resonance tables.
    pub fn ifprod_eff(&self) -> i32 {
        if self.ifprod > 0 {
            if self.ires > 0 {
                2
            } else {
                1
            }
        } else {
            0
        }
    }

    /// `yield(1:jcc/2)` / `ifisp(1:jcc/2)` and `jcc` (`wimsr.f90:223-241`,
    /// `2072`): `yield(1) = 0`, `ifisp(1) = nfid`; with burnup data
    /// `yield(4) = efiss`, `ifisp(4) = 0` and the card-6 pairs at
    /// positions 2, 3, 5, 6, …
    pub fn burn_table(&self) -> (Vec<(f64, i32)>, usize) {
        let mut yield_ = vec![0.0f64; 100];
        let mut ifisp = vec![0i32; 100];
        ifisp[0] = self.nfid();
        let jcc;
        if self.iburn > 0 {
            let ntis = self.burn.len();
            jcc = ntis * 2 + 4;
            yield_[3] = self.efiss;
            ifisp[3] = 0;
            let mut ij = 1usize; // Fortran ij (1-based) before the loop
            for (i, &(id, y)) in self.burn.iter().enumerate() {
                let i1 = i + 1;
                if i1 == 1 || i1 == 3 {
                    ij += 1;
                }
                ifisp[ij - 1] = id;
                yield_[ij - 1] = y;
                ij += 1;
            }
        } else {
            jcc = 2;
        }
        let n = jcc / 2;
        (
            (0..n).map(|i| (yield_[i], ifisp[i])).collect(),
            jcc,
        )
    }
}
