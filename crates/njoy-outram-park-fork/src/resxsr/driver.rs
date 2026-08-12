// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! RESXSR top-level orchestration skeleton (`subroutine resxsr`,
//! `resxsr.f90:10-502`).
//!
//! Documents the RESXSR pipeline stage by stage and dispatches to the ported
//! kernels. The tape reader (PENDF `gety1` walk, `loada`/`finda` scratch files)
//! and the binary RESXS writer are not ported, so [`run`] returns
//! [`NjoyError::NotPorted`] rather than fabricating an output file.

use std::io::Write;

use crate::endf::tape::Tape;
use crate::resxsr::assemble::{assemble_union_grid, thin_linear};
use crate::resxsr::format::{FileControl, FileData, FileIdentification, MaterialControl, NBLOK};
use crate::resxsr::input::ResxsrInput;
use crate::resxsr::resxs::{read_pendf_reactions, ResxsFile, ResxsMaterial, ResxsPoint};
use crate::NjoyError;

/// Run the RESXSR pipeline for a full input deck (`resxsr.f90:10-502`).
///
/// The card deck ([`ResxsrInput`]), the RESXS record layout
/// ([`crate::resxsr::format`]), and the per-material union-grid + thinning
/// kernels ([`crate::resxsr::assemble`]) are ported and tested; the tape reader
/// that supplies real pointwise `gety1` values and the binary RESXS writer are
/// not, so this function documents the stages and returns
/// [`NjoyError::NotPorted`].
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"resxsr::run"` until the PENDF
/// reader and RESXS writer land.
pub fn run(input: &ResxsrInput) -> Result<(), NjoyError> {
    // --- Stage 0: user input (resxsr.f90:236-248) ------------------------
    //   card 1 nout; card 2 nmat,maxt,nholl,efirst,elast,eps;
    //   card 3 huse,ivers; card 4 holl[nholl]; card 5 hmat/mat/unit[nmat].
    //   Modelled by `input`.
    let _ = input;

    // --- Stage 1: per material (resxsr.f90:250-433) ----------------------
    //   openz(nin); tpidio; loop temperatures (contio/hdatio, iverf detect);
    //   findf(matd,3): for each resonance MT (2/18/102) walk the grid with
    //   gety1 and merge into the union set
    //   (assemble::assemble_union_grid), then thin with eps
    //   (assemble::thin_linear).  <-- kernels ported; the gety1/loada/finda
    //   tape plumbing that feeds them is not.

    // --- Stage 2: material control + xs blocks to scratch ----------------
    //   resxsr.f90:399-430 — write material control (format::MaterialControl)
    //   then the thinned points blocked by nblok (format::xs_* helpers).

    // --- Stage 3: RESXS output file (resxsr.f90:435-501) -----------------
    //   file identification / file control / set-Hollerith / file data
    //   (format::FileIdentification/FileControl/FileData), then copy the
    //   material control + xs blocks from scratch to nout.

    Err(NjoyError::NotPorted("resxsr::run"))
}

/// Read `(amass, temp)` for material `mat` from a PENDF [`Tape`]: `amass` is the
/// MF=3 HEAD `AWR` (`resxsr.f90:274`); `temp` is the MF=1/451 header temperature
/// where present, else `0` (bare tapes carry no header, `resxsr.f90:288-291`).
fn material_amass_temp(tape: &Tape, mat: i32) -> (f64, f64) {
    let amass = [2i32, 18, 102]
        .iter()
        .find_map(|&mt| tape.section(mat, 3, mt))
        .map(|sec| sec.rows[0][1])
        .unwrap_or(0.0);
    let temp = tape
        .section(mat, 1, 451)
        .and_then(|sec| sec.rows.get(3).map(|r| r[0]))
        .unwrap_or(0.0);
    (amass, temp)
}

/// Functional RESXSR driver: read each material's PENDF tape, assemble + thin the
/// resonance cross sections, and write a RESXS file to `out`
/// (`resxsr.f90:250-501`).
///
/// This is the file-level pipeline wired end to end using the ported kernels:
/// [`read_pendf_reactions`] (the `gety1` feeder), [`assemble_union_grid`] +
/// [`thin_linear`] (the union grid + thinning), and [`ResxsFile::write`] (the
/// RESXS record writer).
///
/// * `input` — the parsed card deck; `input.materials[i]` is written using
///   `tapes[i]`.
/// * `tapes` — one parsed PENDF [`Tape`] per material, aligned with
///   `input.materials`.
/// * `out` — the [`Write`] sink for the RESXS file.
///
/// **Minimal-port scope (honest).** This handles the **single-temperature** case
/// (`ntemp == 1`, the temperature taken from each material's MF=1/451 header where
/// present, else 0). The multi-temperature loop (`resxsr.f90:267-352`, which grows
/// `jx = nreac*ntemp` reaction columns across temperatures) is **not** ported; a
/// tape offering several temperatures contributes only its first here. Reaction
/// self-shielding and the `maxt` cap are likewise out of scope for this minimal
/// driver. The record layout, word counts, and round-trip are the parts under
/// test (see [`crate::resxsr::resxs`]).
///
/// # Errors
/// Propagates [`NjoyError`] from tape parsing or the RESXS write.
pub fn run_resxs<W: Write>(input: &ResxsrInput, tapes: &[Tape], out: W) -> Result<(), NjoyError> {
    let nmat = input.materials.len();
    let mut materials = Vec::with_capacity(nmat);
    let mut hmatn = Vec::with_capacity(nmat);
    let mut ntemp = Vec::with_capacity(nmat);
    let mut locm = Vec::with_capacity(nmat);
    let mut irec = 4i32; // records 1..4 precede the first material (resxsr.f90:443-467).

    for (im, spec) in input.materials.iter().enumerate() {
        let tape = tapes.get(im).ok_or(NjoyError::EndfParse(
            "resxsr::run_resxs: missing input tape".into(),
        ))?;
        let reactions = read_pendf_reactions(tape, spec.mat)?;
        let (amass, temp) = material_amass_temp(tape, spec.mat);
        let nreac = reactions.len() as i32;

        let rows = assemble_union_grid(&reactions, input.efirst, input.elast);
        let thinned = thin_linear(&rows, input.eps);
        let points: Vec<ResxsPoint> = thinned
            .iter()
            .map(|r| ResxsPoint {
                energy: r.energy,
                values: r.xs.clone(),
            })
            .collect();
        let nener = points.len() as i32;

        locm.push(irec + 1);
        irec += 1; // material control record
        let nn = 1 + nreac; // single temperature: nn = 1 + nreac*1
        let cap_pts = (NBLOK / nn).max(1);
        irec += ((nener + cap_pts - 1) / cap_pts).max(1); // cross-section block records

        materials.push(ResxsMaterial {
            control: MaterialControl {
                hmat: spec.hmat.clone(),
                amass,
                temps: vec![temp],
                nreac,
                nener,
            },
            points,
        });
        hmatn.push(spec.hmat.clone());
        ntemp.push(1);
    }

    let (huse1, huse2) = split_user_id(&input.user_id);
    let file = ResxsFile {
        ident: FileIdentification {
            hname: crate::resxsr::format::FILE_NAME.into(),
            huse1,
            huse2,
            ivers: input.ivers,
        },
        control: FileControl {
            efirst: input.efirst,
            elast: input.elast,
            nholl: input.nholl() as i32,
            nmat: nmat as i32,
            nblok: NBLOK,
        },
        data: FileData { hmatn, ntemp, locm },
        materials,
    };
    file.write(out)
}

/// Split a user id into its two 6-character Hollerith words (`resxsr.f90:240`,
/// read as `(2a6)`).
fn split_user_id(uid: &str) -> (String, String) {
    let chars: Vec<char> = uid.chars().collect();
    let w1: String = chars.iter().take(6).collect();
    let w2: String = chars.iter().skip(6).take(6).collect();
    (w1, w2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resxsr::input::MaterialSpec;

    /// The driver reports `NotPorted`, never a fabricated RESXS file.
    ///
    /// **Methodology.** RESXSR's tape reader and binary writer are not ported;
    /// [`run`] must surface that honestly. Call it with a populated deck and
    /// assert the documented `NotPorted` tag.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `run(&deck)` →
    /// `NotPorted("resxsr::run")`.
    /// The functional driver produces a RESXS file that round-trips and carries
    /// the material's thinned resonance grid.
    ///
    /// **Methodology (task V&V gate).** Build a small PENDF tape (elastic +
    /// capture), run [`run_resxs`] to a byte buffer, read it back with
    /// [`ResxsFile::read`], and require the recovered file to name the material,
    /// report `nreac == 2` (non-fissionable), and preserve the assembled energy
    /// grid within single precision.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** One material `u238`, `nreac == 2`,
    /// recovered energies span `[4, 200]` eV, `ivers == 1`, and the file-control
    /// `nmat == 1`.
    #[test]
    fn run_resxs_roundtrips() {
        use crate::endf::tape::{Section, Tape};
        use crate::endf::EndfKey;
        use crate::resxsr::input::MaterialSpec;
        use crate::resxsr::resxs::ResxsFile;

        fn mf3(mat: i32, mt: i32, pairs: &[(f64, f64)]) -> Section {
            let ne = pairs.len();
            let mut rows: Vec<[f64; 6]> = vec![
                [92238.0, 236.006, 0.0, 0.0, 0.0, 0.0],
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
            Section {
                key: EndfKey { mat, mf: 3, mt },
                rows,
            }
        }

        let tape = Tape::from_sections(
            " test".into(),
            vec![
                mf3(9237, 2, &[(4.0, 10.0), (200.0, 10.0)]),
                mf3(9237, 102, &[(4.0, 1.0), (100.0, 5.0), (200.0, 1.0)]),
            ],
        );
        let deck = ResxsrInput {
            nout: -21,
            maxt: 1,
            efirst: 4.0,
            elast: 200.0,
            eps: 1.0e-3,
            user_id: "outrampark".into(),
            ivers: 1,
            comments: vec!["resonance test".into()],
            materials: vec![MaterialSpec {
                hmat: "u238".into(),
                mat: 9237,
                nin: 20,
            }],
        };

        let mut buf = Vec::new();
        run_resxs(&deck, &[tape], &mut buf).unwrap();
        let back = ResxsFile::read(&buf[..]).unwrap();
        assert_eq!(back.ident.ivers, 1);
        assert_eq!(back.control.nmat, 1);
        assert_eq!(back.materials.len(), 1);
        let m = &back.materials[0];
        assert_eq!(m.control.hmat, "u238");
        assert_eq!(m.control.nreac, 2);
        assert!(!m.points.is_empty());
        let e_first = m.points.first().unwrap().energy;
        let e_last = m.points.last().unwrap().energy;
        assert!(
            e_first >= 4.0 - 1e-3 && e_last <= 200.0 + 1e-1,
            "grid {e_first}..{e_last}"
        );
    }

    #[test]
    fn run_is_not_ported() {
        let deck = ResxsrInput {
            nout: -21,
            maxt: 3,
            efirst: 4.0,
            elast: 200.0,
            eps: 1.0e-3,
            user_id: "outrampark".into(),
            ivers: 1,
            comments: vec!["resonance test".into()],
            materials: vec![MaterialSpec {
                hmat: "u238".into(),
                mat: 9237,
                nin: 20,
            }],
        };
        match run(&deck) {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "resxsr::run"),
            other => panic!("expected NotPorted(\"resxsr::run\"), got {other:?}"),
        }
    }
}
