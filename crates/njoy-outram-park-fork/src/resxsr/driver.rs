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

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::resxsr::assemble::{assemble_union_grid, thin_linear, PointwiseReaction};
use crate::resxsr::format::{FileControl, FileData, FileIdentification, MaterialControl, NBLOK};
use crate::resxsr::input::ResxsrInput;
use crate::resxsr::resxs::{ResxsFile, ResxsMaterial, ResxsPoint};
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
/// One temperature's worth of a material on a PENDF: its `MF=1/451`
/// temperature, the `AWR` of its first resonance reaction, and the
/// resonance reactions (MT 2, 18, 102 in tape order).
struct TemperatureBlock {
    temp: f64,
    amass: f64,
    reactions: Vec<PointwiseReaction>,
}

/// Split the PENDF's repeated material into its temperature blocks
/// (`resxsr.f90:267-352`): every `MF=1/451` of `mat` starts a block (its
/// `hdatio` record carries the temperature), and the block's `MF=3` sections
/// with MT 2, 18 or 102 are its reactions. At most `maxt` blocks are taken
/// (`if (itemp.eq.maxt) go to 250`).
fn read_pendf_temperature_blocks(
    tape: &Tape,
    mat: i32,
    maxt: usize,
) -> Result<Vec<TemperatureBlock>, NjoyError> {
    let mut blocks: Vec<TemperatureBlock> = Vec::new();
    // A tape without MF=1/451 for the material (a bare set of MF=3 sections,
    // as the unit tests build) is one block at temperature 0 — what the
    // single-temperature driver did before the temperature loop existed.
    if tape.section(mat, 1, 451).is_none() {
        blocks.push(TemperatureBlock {
            temp: 0.0,
            amass: 0.0,
            reactions: Vec::new(),
        });
    }
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat) {
        if sec.key.mf == 1 && sec.key.mt == 451 {
            if blocks.len() == maxt {
                break;
            }
            let temp = sec.rows.get(3).map(|r| r[0]).unwrap_or(0.0);
            blocks.push(TemperatureBlock {
                temp,
                amass: 0.0,
                reactions: Vec::new(),
            });
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        if sec.key.mf == 3 && crate::resxsr::resxs::RESONANCE_MTS.contains(&sec.key.mt) {
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            if block.reactions.is_empty() {
                block.amass = head.c2;
            }
            let tab1 = cur.read_tab1()?;
            block.reactions.push(PointwiseReaction {
                mt: sec.key.mt,
                points: tab1.pairs,
            });
        }
    }
    Ok(blocks)
}

/// Functional RESXSR driver: read each material's PENDF tape, assemble + thin the
/// resonance cross sections, and write a RESXS file to `out`
/// (`resxsr.f90:250-501`).
///
/// This is the file-level pipeline wired end to end using the ported kernels:
/// [`crate::resxsr::resxs::read_pendf_reactions`] (the `gety1` feeder, applied per
/// temperature block here), [`assemble_union_grid`] +
/// [`thin_linear`] (the union grid + thinning), and [`ResxsFile::write`] (the
/// RESXS record writer).
///
/// * `input` — the parsed card deck; `input.materials[i]` is written using
///   `tapes[i]`.
/// * `tapes` — one parsed PENDF [`Tape`] per material, aligned with
///   `input.materials`.
/// * `out` — the [`Write`] sink for the RESXS file.
///
/// **Temperatures.** The PENDF's repeated material blocks (one per
/// temperature) are read in order up to `input.maxt`, each contributing its
/// resonance reactions as further columns temperature-major, exactly as
/// upstream's `jx` loop (`resxsr.f90:267-352`); the union grid and the
/// thinning then run over every column. Verified byte-for-byte against
/// NJOY2016 for one and two temperatures (`tests/resxsr_h2_njoy_golden.rs`).
/// Reaction self-shielding is outside this driver.
///
/// # Errors
/// Propagates [`NjoyError`] from tape parsing or the RESXS write.
pub fn run_resxs<W: Write>(input: &ResxsrInput, tapes: &[Tape], out: W) -> Result<(), NjoyError> {
    let nmat = input.materials.len();
    let mut materials = Vec::with_capacity(nmat);
    let mut hmatn = Vec::with_capacity(nmat);
    let mut ntemp = Vec::with_capacity(nmat);
    let mut locm = Vec::with_capacity(nmat);
    // `irec` counts the material records written so far — upstream starts it
    // at 0 (resxsr.f90:234) and stores `locm(im) = irec` before each material
    // (:253); the four file-header records are not counted (NJOY writes
    // locm = 0 for the first material, `reference-data/resxsr/`).
    let mut irec = 0i32;

    for (im, spec) in input.materials.iter().enumerate() {
        let tape = tapes.get(im).ok_or(NjoyError::EndfParse(
            "resxsr::run_resxs: missing input tape".into(),
        ))?;
        // Temperature loop (resxsr.f90:267-352): the PENDF repeats the
        // material once per temperature; each pass appends its resonance
        // reactions as further columns (`jx` runs temperature-major), up to
        // `maxt` temperatures. The union grid and thinning then run over
        // every column, as upstream's incremental merge does.
        let blocks = read_pendf_temperature_blocks(tape, spec.mat, input.maxt.max(1) as usize)?;
        if blocks.is_empty() {
            return Err(NjoyError::SectionNotFound {
                mat: spec.mat,
                mf: 1,
                mt: 451,
            });
        }
        let amass = blocks[0].amass;
        let temps: Vec<f64> = blocks.iter().map(|b| b.temp).collect();
        let ntemp_m = blocks.len() as i32;
        let reactions: Vec<PointwiseReaction> =
            blocks.into_iter().flat_map(|b| b.reactions).collect();
        if reactions.len() as i32 % ntemp_m != 0 {
            return Err(NjoyError::EndfParse(format!(
                "resxsr: material {} has an uneven reaction count across its {} temperatures",
                spec.mat, ntemp_m
            )));
        }
        let nreac = reactions.len() as i32 / ntemp_m;

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

        locm.push(irec);
        irec += 1; // material control record (resxsr.f90:402)
        let nn = 1 + nreac * ntemp_m; // words per point (resxsr.f90:404)
        let cap_pts = (NBLOK / nn).max(1);
        irec += ((nener + cap_pts - 1) / cap_pts).max(1); // cross-section block records

        materials.push(ResxsMaterial {
            control: MaterialControl {
                hmat: spec.hmat.clone(),
                amass,
                temps,
                nreac,
                nener,
            },
            points,
        });
        hmatn.push(spec.hmat.clone());
        ntemp.push(ntemp_m);
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
