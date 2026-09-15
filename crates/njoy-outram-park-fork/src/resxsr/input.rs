// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! RESXSR user-input card model.
//!
//! Faithful, line-traceable translation of the **free-format card sequence**
//! read by NJOY2016's `subroutine resxsr` (`resxsr.f90:236-248`). RESXSR builds
//! a RESXS pointwise-resonance cross-section file from one or more NJOY PENDF
//! tapes, for near-epithermal (≈4–200 eV) self-shielding treatments.
//!
//! ## Card summary (`resxsr.f90:16-40`)
//!
//! | Card | Fortran vars | Repeat |
//! |------|--------------|--------|
//! | 1 | `nout` | once |
//! | 2 | `nmat maxt nholl efirst elast eps` | once |
//! | 3 | `huse ivers` (`uid,ivers`) | once |
//! | 4 | `holl` (72-char comment line) | `nholl` times |
//! | 5 | `hmat mat unit` | `nmat` times |
//!
//! Field names keep the upstream Fortran identifiers in their doc comments so a
//! discrepancy can be localised to a specific `read(nsysi,*)` statement.

/// One material specification (card 5, `resxsr.f90:36-39`, read at `:247`).
///
/// Repeated `nmat` times. Names an ENDF material and the NJOY logical unit that
/// holds its PENDF pointwise cross sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialSpec {
    /// `hmat` — Hollerith name for the material, up to 8 characters
    /// (`resxsr.f90:37`). Dimensionless label.
    pub hmat: String,
    /// `mat` — ENDF MAT number for the material (`resxsr.f90:38`).
    pub mat: i32,
    /// `unit` (`ninm`) — NJOY logical unit number carrying this material's PENDF
    /// data (`resxsr.f90:39`). A negative number denotes a binary tape.
    pub nin: i32,
}

/// Complete RESXSR user-input deck (`resxsr.f90:16-40`).
///
/// Aggregates every card RESXSR reads. `nmat` (number of materials) is
/// `materials.len()`; `nholl` (number of comment lines) is `comments.len()`.
/// All energies are in **electron-volts (eV)**; `eps` is a dimensionless
/// relative thinning tolerance.
///
/// Construct via [`ResxsrInput::default`] and override fields, or build directly.
#[derive(Debug, Clone, PartialEq)]
pub struct ResxsrInput {
    /// Card 1 — `nout`: output logical unit for the RESXS file (`resxsr.f90:19`).
    /// A negative number denotes a binary tape. Transport-layer handle, no units.
    pub nout: i32,

    /// Card 2 — `maxt`: maximum number of temperatures to take from any material
    /// (`resxsr.f90:23`). The temperature loop stops once `maxt` temperatures
    /// have been read (`resxsr.f90:273`).
    pub maxt: i32,
    /// Card 2 — `efirst`: lower energy limit in **eV** (`resxsr.f90:25`). Lowest
    /// energy written to the file.
    pub efirst: f64,
    /// Card 2 — `elast`: upper energy limit in **eV** (`resxsr.f90:26`). Highest
    /// energy written to the file.
    pub elast: f64,
    /// Card 2 — `eps`: relative thinning tolerance, dimensionless
    /// (`resxsr.f90:27`). A point is dropped when every reaction value at that
    /// point is within `eps` (fractional) of a lin-lin interpolation between the
    /// bracketing kept points (`resxsr.f90:354-397`).
    pub eps: f64,

    /// Card 3 — `huse` (`uid`): Hollerith user identification, up to 16 chars
    /// (`resxsr.f90:31`, read as two 6-char words via `(2a6)` at `:240`).
    pub user_id: String,
    /// Card 3 — `ivers`: file version number (`resxsr.f90:32`).
    pub ivers: i32,

    /// Card 4 — `holl`: descriptive comment lines, ≤ 72 characters each
    /// (`resxsr.f90:34`, read at `:241-245`). `nholl` is `comments.len()`.
    pub comments: Vec<String>,

    /// Card 5 — the per-material specifications (`resxsr.f90:36-39`, read at
    /// `:246-248`). `nmat` is `materials.len()`.
    pub materials: Vec<MaterialSpec>,
}

impl ResxsrInput {
    /// `nmat` — number of materials on the file (`resxsr.f90:22`).
    pub fn nmat(&self) -> usize {
        self.materials.len()
    }

    /// `nholl` — number of descriptive comment lines (`resxsr.f90:24`).
    pub fn nholl(&self) -> usize {
        self.comments.len()
    }
}

impl Default for ResxsrInput {
    /// RESXSR has no initialised-default block; the card-2/3 scalars are read
    /// directly (`resxsr.f90:237-240`). This default zeroes the tape units and
    /// scalars and leaves the lists empty; a caller must fill them for a real
    /// run. `maxt` defaults to the storage cap `10` (`temp(10)`,
    /// `resxsr.f90:199`).
    fn default() -> Self {
        Self {
            nout: 0,
            maxt: 10,
            efirst: 0.0,
            elast: 0.0,
            eps: 0.0,
            user_id: String::new(),
            ivers: 0,
            comments: Vec::new(),
            materials: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default deck reproduces RESXSR's implicit (zeroed) starting state.
    ///
    /// **Methodology.** RESXSR reads card 2/3 scalars directly with no default
    /// block (`resxsr.f90:237-240`); the only storage-driven cap is
    /// `temp(10)` → `maxt <= 10` (`resxsr.f90:199, 273`). Assert the Rust
    /// default zeroes the units/energies and leaves lists empty, with
    /// `maxt == 10`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `nout=0, maxt=10,
    /// efirst=elast=eps=0, ivers=0`, no comments, no materials, `nmat()==0`,
    /// `nholl()==0`.
    #[test]
    fn defaults_are_zeroed() {
        let inp = ResxsrInput::default();
        assert_eq!(inp.nout, 0);
        assert_eq!(inp.maxt, 10);
        assert_eq!(inp.efirst, 0.0);
        assert_eq!(inp.elast, 0.0);
        assert_eq!(inp.eps, 0.0);
        assert_eq!(inp.ivers, 0);
        assert_eq!(inp.nmat(), 0);
        assert_eq!(inp.nholl(), 0);
    }

    /// `nmat`/`nholl` track the list lengths, mirroring the Fortran counters.
    ///
    /// **Methodology.** In upstream, `nmat`/`nholl` are read on card 2 and then
    /// drive the card-4/card-5 read loops (`resxsr.f90:241-248`). Model that by
    /// deriving both from the vectors; assert they agree with a populated deck.
    ///
    /// **Result (2026-07-15).** A deck with 2 comments and 3 materials reports
    /// `nholl()==2`, `nmat()==3`, and `materials[1].mat == 9237`.
    #[test]
    fn counters_track_lists() {
        let inp = ResxsrInput {
            nout: -21,
            maxt: 3,
            efirst: 4.0,
            elast: 200.0,
            eps: 1.0e-3,
            user_id: "outrampark".to_string(),
            ivers: 1,
            comments: vec!["line one".to_string(), "line two".to_string()],
            materials: vec![
                MaterialSpec {
                    hmat: "u235".to_string(),
                    mat: 9228,
                    nin: 20,
                },
                MaterialSpec {
                    hmat: "u238".to_string(),
                    mat: 9237,
                    nin: 21,
                },
                MaterialSpec {
                    hmat: "pu239".to_string(),
                    mat: 9437,
                    nin: 22,
                },
            ],
        };
        assert_eq!(inp.nholl(), 2);
        assert_eq!(inp.nmat(), 3);
        assert_eq!(inp.materials[1].mat, 9237);
    }
}
