// Ported from NJOY2016 `src/mixr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MIXR file-level driver — wire the card deck to real tape I/O
//! (`subroutine mixr`, `mixr.f90:57-390`).
//!
//! Fortran MIXR reads its six cards from `nsysi`, opens each input logical unit
//! `nin(i)`, walks it with `tpidio`/`contio` to the requested material and
//! temperature, mixes, and writes the result to `nout` (`mixr.f90:84-389`).
//!
//! This crate holds a whole tape in memory ([`crate::endf::tape::Tape`]) instead
//! of a Fortran logical unit, so the driver here is the in-memory analogue:
//! **read** each input from a byte source, **mix** with the already-ported engine
//! ([`crate::mixr::mix`]), and **write** the result to a byte sink. The card deck
//! is supplied as a typed [`MixrInput`] (see [`crate::mixr::input`]) rather than
//! re-parsed from free-format text; that is the one deliberate divergence, and it
//! is the same shape every other module's driver in this crate takes.

use std::io::{Read, Write};

use crate::endf::tape::Tape;
use crate::mixr::input::MixrInput;
use crate::mixr::mix::mix;
use crate::NjoyError;

/// Run MIXR end to end over byte streams: read each input tape, mix, write the
/// output tape (`mixr.f90:144-389`).
///
/// * `input` — the parsed six-card deck ([`MixrInput`]). Each
///   [`crate::mixr::MixComponent::tape_index`] indexes into `inputs` in the same
///   order as the card-1 `nin` list.
/// * `inputs` — one [`Read`] source per input tape, in `nin` order. Each is
///   parsed with [`Tape::read`] (the analogue of `openz`/`tpidio` +
///   the `contio` material walk, `mixr.f90:150-195`).
/// * `out` — the [`Write`] sink for the mixed PENDF tape, written with
///   [`Tape::write`] (the analogue of the `nout` writes, `mixr.f90:197-378`).
///
/// The output tape carries one MF=1/451 descriptive section (with AWI/EMAX/NSUB
/// read back from the inputs' headers where present) and one MF=3 section per
/// output MT, exactly as [`mix`] produces.
///
/// # Example
/// ```
/// use njoy_outram_park_fork::mixr::{driver::run_mix, MixrInput};
/// use njoy_outram_park_fork::endf::tape::Tape;
///
/// # fn demo(tape_a_bytes: &[u8], tape_b_bytes: &[u8]) -> Result<(), njoy_outram_park_fork::NjoyError> {
/// let input = MixrInput::from_cards(
///     20, &[0, 1], vec![2], &[(125, 0.5), (128, 0.5)],
///     0.0, 9999, 1001.0, 0.999, "half-and-half",
/// );
/// let mut out = Vec::new();
/// run_mix(&input, vec![tape_a_bytes, tape_b_bytes], &mut out)?;
/// let mixed = Tape::read(&out[..])?;
/// assert!(mixed.section(9999, 3, 2).is_some());
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Propagates [`NjoyError::Io`] / [`NjoyError::EndfParse`] from reading an input
/// tape, and any [`NjoyError`] from the mixing engine or output write.
pub fn run_mix<R: Read, W: Write>(
    input: &MixrInput,
    inputs: Vec<R>,
    out: W,
) -> Result<(), NjoyError> {
    // Stage 1 — read every input tape into memory (openz/tpidio + material walk).
    let tapes: Vec<Tape> = inputs
        .into_iter()
        .map(Tape::read)
        .collect::<Result<_, _>>()?;

    // Stage 2 — mix (mixr.f90:196-378): the already-tested union-grid engine.
    let mixed = mix(input, &tapes)?;

    // Stage 3 — write the output PENDF tape (mixr.f90:380-389).
    mixed.write(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};
    use crate::endf::EndfKey;

    /// Build a one-material tape carrying a single MF=3 TAB1 for `mt`.
    fn tape_with_mf3(mat: i32, za: f64, awr: f64, mt: i32, pairs: &[(f64, f64)]) -> Tape {
        let ne = pairs.len();
        let mut rows: Vec<[f64; 6]> = vec![
            [za, awr, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, ne as f64],
            [ne as f64, 2.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let mut flat: Vec<f64> = Vec::new();
        for &(e, s) in pairs {
            flat.push(e);
            flat.push(s);
        }
        for chunk in flat.chunks(6) {
            let mut row = [0.0f64; 6];
            row[..chunk.len()].copy_from_slice(chunk);
            rows.push(row);
        }
        let sec = Section {
            key: EndfKey { mat, mf: 3, mt },
            rows,
        };
        Tape::from_sections(" test".to_string(), vec![sec])
    }

    fn read_mf3_pairs(tape: &Tape, mat: i32, mt: i32) -> Vec<(f64, f64)> {
        use crate::endf::records::SectionCursor;
        let sec = tape.section(mat, 3, mt).expect("section present");
        let mut cur = SectionCursor::new(&sec.rows);
        let _head = cur.read_cont().unwrap();
        cur.read_tab1().unwrap().pairs
    }

    /// The file driver reproduces the direct `mix()` result through real tape I/O.
    ///
    /// **Methodology (task V&V gate).** Regression-lock the engine through the
    /// driver: serialise two input tapes to ENDF ASCII, feed the bytes to
    /// [`run_mix`], read the produced tape back, and require its MF=3 pairs to
    /// equal those from the already-tested in-memory [`mix`] over the same tapes.
    /// This exercises `Tape::read -> mix -> Tape::write` end to end.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** For a half-and-half mix of
    /// (10,20) and (30,40) barns the driver output and the direct `mix()` output,
    /// both taken through the same ENDF-ASCII `write`/`read` round-trip, agree
    /// bit-for-bit on the MT=2 pairs (20 and 30 barns), and both carry the
    /// MF=1/451 section. (Comparing the driver output against the *unwritten*
    /// direct tape would differ only in the sub-`1e-12` sigfig-bias digits that
    /// the ASCII `a11` formatting truncates — hence both sides are round-tripped.)
    #[test]
    fn driver_matches_direct_mix() {
        let a = tape_with_mf3(125, 1001.0, 0.999, 2, &[(1.0, 10.0), (2.0, 20.0)]);
        let b = tape_with_mf3(128, 1001.0, 0.999, 2, &[(1.0, 30.0), (2.0, 40.0)]);
        let input = MixrInput::from_cards(
            20,
            &[0, 1],
            vec![2],
            &[(125, 0.5), (128, 0.5)],
            0.0,
            9999,
            1001.0,
            0.999,
            "mix",
        );

        // Direct engine result, taken through the same ASCII round-trip the
        // driver uses (so the sigfig-bias digits are truncated identically).
        let direct = mix(
            &input,
            &[
                tape_with_mf3(125, 1001.0, 0.999, 2, &[(1.0, 10.0), (2.0, 20.0)]),
                tape_with_mf3(128, 1001.0, 0.999, 2, &[(1.0, 30.0), (2.0, 40.0)]),
            ],
        )
        .unwrap();
        let mut direct_bytes = Vec::new();
        direct.write(&mut direct_bytes).unwrap();
        let direct_rt = Tape::read(&direct_bytes[..]).unwrap();
        let direct_pairs = read_mf3_pairs(&direct_rt, 9999, 2);

        // Serialise inputs to ASCII and drive the whole pipeline over bytes.
        let mut a_bytes = Vec::new();
        a.write(&mut a_bytes).unwrap();
        let mut b_bytes = Vec::new();
        b.write(&mut b_bytes).unwrap();
        let mut out = Vec::new();
        run_mix(
            &input,
            vec![a_bytes.as_slice(), b_bytes.as_slice()],
            &mut out,
        )
        .unwrap();

        let driven = Tape::read(&out[..]).unwrap();
        assert!(driven.section(9999, 1, 451).is_some(), "MF=1/451 present");
        let driven_pairs = read_mf3_pairs(&driven, 9999, 2);
        assert_eq!(driven_pairs, direct_pairs, "driver == direct mix");
        // The physical mix values are 20 and 30 barns.
        assert!((driven_pairs[0].1 - 20.0).abs() < 1e-4);
        assert!((driven_pairs[1].1 - 30.0).abs() < 1e-4);
    }
}
