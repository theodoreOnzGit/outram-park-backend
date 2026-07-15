// Ported from NJOY2016 `src/mixr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MIXR mixing engine — form weighted sums of cross sections on a union grid.
//!
//! This is the physics of MIXR (`mixr.f90:257-390`, the reaction loop and its
//! `gety` helper `mixr.f90:392-530`). For each requested reaction MT it:
//!
//! 1. locates the MF=3 section for that MT on each contributing input
//!    material (`mixr.f90:261-289`);
//! 2. unionises the tabulated **energy grids** (in eV) of the contributing
//!    cross sections (`mixr.f90:291-301`, driven by `gety`'s `xnext`);
//! 3. at every union energy `E`, interpolates each contributing cross section
//!    (in barns) with its own ENDF interpolation law and forms the weighted
//!    sum `sigma_out(E) = sum_i w_i * sigma_i(E)` (`mixr.f90:296-303`);
//! 4. writes the result as a single lin-lin (INT=2) MF=3 TAB1 section
//!    (`mixr.f90:319-370`).
//!
//! The output tape carries **File 1 (MT=451) and File 3 only**, lin-lin
//! interpolation assumed — exactly MIXR's contract (`mixr.f90:22-29`).
//!
//! **Units.** Energies eV, cross sections barns, weights dimensionless — all
//! `f64`, matching the [`crate::endf`] tabulated model (see
//! [`crate::mixr::input`] for why `uom` is not used at this boundary).
//!
//! ## Fidelity notes (documented divergences from genuine NJOY)
//!
//! - **Above-range extrapolation.** `gety` holds a cross section constant at
//!   its last tabulated value for energies above that input's grid
//!   (`mixr.f90:514-517`, label 140), and returns 0 below the grid
//!   (`mixr.f90:509-512`, label 135). [`gety_value`] reproduces both.
//! - **Union grid = unique tabulated energies.** The union is the sorted set
//!   of distinct input grid energies. Duplicate-energy step discontinuities
//!   (two points at the same `E`) are collapsed to one — the crate's
//!   [`crate::endf::interp::eval_tab1`] returns a single value per energy, so
//!   discontinuities are not reproduced. For lin-lin PENDF input (MIXR's main
//!   use) this is exact.
//! - **Output `sigma` rounded to 7 significant figures** via [`sigfig`], as
//!   `mixr.f90:350` does (`sigfig(c(2),7,0)`). [`mix_reaction`] returns the
//!   *exact* weighted sum (unrounded) so callers can test the arithmetic;
//!   [`mix`] applies `sigfig` when it lays the values into the output tape.
//! - **MF=1/451 comment text is not preserved.** The crate's `[f64; 6]`
//!   section-row model cannot store Hollerith characters, so the card-6
//!   description becomes a blank comment record. AWI/EMAX/NSUB header fields
//!   use MIXR's default seeds (`mixr.f90:147-149`) rather than being read back
//!   from the first input's MF=1 header.

use crate::endf::interp::eval_tab1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::mixr::input::MixrInput;
use crate::NjoyError;

/// Adjust `x` to have `ndig` significant figures, optionally shading it up or
/// down by `idig` in the last digit.
///
/// Faithful port of NJOY2016's `sigfig` (`util.f90:361-393`). MIXR rounds every
/// output cross section to 7 significant figures with `idig = 0`
/// (`mixr.f90:350`), so the written tape matches the Fortran output.
///
/// `x` is any real (a cross section in barns here); the result is `x` rounded
/// to `ndig` significant figures and scaled by NJOY's `1.0000000000001` bias.
pub fn sigfig(x: f64, ndig: i32, idig: i32) -> f64 {
    const BIAS: f64 = 1.000_000_000_000_1_f64;
    const TEN: f64 = 10.0;
    let mut xx = 0.0f64;
    if x != 0.0 {
        let aa = x.abs().log10();
        let mut ipwr = aa as i32; // Fortran int(): truncate toward zero
        if aa < 0.0 {
            ipwr -= 1;
        }
        ipwr = ndig - 1 - ipwr;
        // nint(): round half away from zero, matching Fortran and f64::round.
        let mut ii = (x * TEN.powi(ipwr) + TEN.powi(ndig - 11)).round() as i64;
        let mut ipwr2 = ipwr;
        if ii >= 10i64.pow(ndig as u32) {
            ii /= 10;
            ipwr2 -= 1;
        }
        ii += idig as i64;
        xx = (ii as f64) * TEN.powi(-ipwr2);
    }
    xx *= BIAS;
    xx
}

/// Evaluate a tabulated cross section `sigma(E)` at energy `E` (eV) with MIXR's
/// out-of-range conventions — the value-retrieval half of `gety`
/// (`mixr.f90:392-530`).
///
/// `interp` is the ENDF region table `(nbt, int_code)`; `pairs` are the
/// `(energy_eV, sigma_barn)` points in ascending energy. Behaviour at the
/// boundaries mirrors `gety`:
///
/// - `E` **below** the first tabulated energy -> `0` (`mixr.f90:509-512`).
/// - `E` **at or above** the last tabulated energy -> the last `sigma`
///   (constant extrapolation, `mixr.f90:514-517`).
/// - `E` **inside** the grid -> interpolated by the region's ENDF law via
///   [`crate::endf::interp::eval_tab1`] (`gety` calls `terp1` with the region
///   `int`, `mixr.f90:499-501`).
///
/// # Errors
/// Propagates [`NjoyError::EndfParse`] from a log/log interpolation on a
/// non-positive value (raised inside `eval_tab1`).
pub fn gety_value(e: f64, interp: &[(u32, u32)], pairs: &[(f64, f64)]) -> Result<f64, NjoyError> {
    match pairs.last() {
        None => Ok(0.0),
        Some(&(e_max, sig_max)) => {
            if e >= e_max {
                // mixr.f90:514-517 (label 140): hold last value above the grid,
                // and return the exact last point at E == E_max.
                Ok(sig_max)
            } else {
                // Below-range -> 0 and in-range interpolation both handled by
                // eval_tab1 (mixr.f90:498-512).
                eval_tab1(e, interp, pairs)
            }
        }
    }
}

/// One contributing cross section: its ENDF region table, `(E, sigma)` points,
/// and mixing weight. Internal to the reaction loop.
struct Contribution {
    interp: Vec<(u32, u32)>,
    pairs: Vec<(f64, f64)>,
    weight: f64,
}

/// Gather the present contributions for one reaction `mt`, warning (and
/// skipping) any component whose section is absent — the section-search loop
/// `mixr.f90:261-289` (`mess` + `findf` on a missing MT, `mixr.f90:283-287`).
fn contributions_for(
    input: &MixrInput,
    inputs: &[Tape],
    mt: i32,
) -> Result<Vec<Contribution>, NjoyError> {
    let mut contribs = Vec::with_capacity(input.components.len());
    for comp in &input.components {
        let Some(tape) = inputs.get(comp.tape_index) else {
            // No Fortran analogue (an unopened unit errors at read time); this
            // crate has every input in memory, so a bad index is warned+skipped.
            log::warn!(
                "mixr: no input tape at index {} (mt={}, mat={})",
                comp.tape_index,
                mt,
                comp.mat
            );
            continue;
        };
        let Some(sec) = tape.section(comp.mat, 3, mt) else {
            // mixr.f90:283-287 — not present is a warning, not fatal.
            log::warn!("mixr: mt={} not present for mat={}", mt, comp.mat);
            continue;
        };
        // An MF=3 section is a HEAD record (ZA, AWR, 0,0,0,0) followed by the
        // TAB1 (cross section). Skip the HEAD before reading the TAB1.
        let mut cur = SectionCursor::new(&sec.rows);
        let _head = cur.read_cont()?;
        let tab1 = cur.read_tab1()?;
        contribs.push(Contribution {
            interp: tab1.interp,
            pairs: tab1.pairs,
            weight: comp.weight,
        });
    }
    Ok(contribs)
}

/// Mix one reaction: return the `(energy_eV, sigma_barn)` points of
/// `sigma_out(E) = sum_i w_i * sigma_i(E)` on the union energy grid.
///
/// This is the exact (unrounded) weighted sum — the core arithmetic of the
/// reaction loop (`mixr.f90:291-313`), exposed on its own so the mixing can be
/// tested without going through tape (de)serialisation. [`mix`] applies
/// [`sigfig`] to these values when writing the output tape.
///
/// Components whose MF=3 section for `mt` is absent are warned and skipped
/// (`mixr.f90:283-287`); if **no** component provides `mt`, an empty grid is
/// returned (MIXR would write an empty TAB1).
///
/// # Errors
/// Propagates [`NjoyError`] from record parsing or interpolation.
pub fn mix_reaction(
    input: &MixrInput,
    inputs: &[Tape],
    mt: i32,
) -> Result<Vec<(f64, f64)>, NjoyError> {
    let contribs = contributions_for(input, inputs, mt)?;
    if contribs.is_empty() {
        return Ok(Vec::new());
    }

    // Union energy grid: the sorted set of distinct input grid energies
    // (mixr.f90:291-301). NaN energies are not expected in ENDF data.
    let mut grid: Vec<f64> = contribs
        .iter()
        .flat_map(|c| c.pairs.iter().map(|&(e, _)| e))
        .collect();
    grid.sort_by(|a, b| a.partial_cmp(b).expect("ENDF energies are finite"));
    grid.dedup();

    // Weighted sum at each union energy (mixr.f90:296-303).
    let mut out = Vec::with_capacity(grid.len());
    for &e in &grid {
        let mut sig = 0.0;
        for c in &contribs {
            sig += c.weight * gety_value(e, &c.interp, &c.pairs)?;
        }
        out.push((e, sig));
    }
    Ok(out)
}

/// Build the MF=3 section for one mixed reaction (`mixr.f90:319-370`).
///
/// `pairs` are the exact `(energy_eV, sigma_barn)` points from
/// [`mix_reaction`]; each `sigma` is rounded to 7 significant figures via
/// [`sigfig`] as it is laid into the TAB1 (`mixr.f90:350`). The section is a
/// single lin-lin region (NR=1, NBT=NP, INT=2).
fn build_mf3_section(mat: i32, za: f64, awr: f64, mt: i32, pairs: &[(f64, f64)]) -> Section {
    let ne = pairs.len();
    // mixr.f90:323-328 — MF=3 HEAD: ZA, AWR, 0, 0, 0, 0.
    let mut rows: Vec<[f64; 6]> = vec![
        [za, awr, 0.0, 0.0, 0.0, 0.0],
        // mixr.f90:332-338 — TAB1 header: QM=0, QI=0, L1=0, L2=0, NR=1, NP=ne.
        [0.0, 0.0, 0.0, 0.0, 1.0, ne as f64],
    ];
    // mixr.f90:336-339 — interpolation region (NBT=ne, INT=2). In ENDF-6 the
    // interpolation table and the (x, y) pairs each begin on their own line
    // boundary (they are not packed onto a shared record), matching this
    // crate's `SectionCursor::read_tab1` reader.
    push_packed(&mut rows, &[ne as f64, 2.0]);
    // mixr.f90:344-350 — (E, sigma) pairs, sigma to 7 sig-figs, packed six
    // values per line, starting on a fresh line.
    let mut pair_flat: Vec<f64> = Vec::with_capacity(2 * ne);
    for &(e, sig) in pairs {
        pair_flat.push(e);
        pair_flat.push(sigfig(sig, 7, 0));
    }
    push_packed(&mut rows, &pair_flat);
    Section { key: EndfKey { mat, mf: 3, mt }, rows }
}

/// Append `values` to `rows`, packed six per `[f64; 6]` line and zero-padded on
/// the final partial line (ENDF continuation-record packing).
fn push_packed(rows: &mut Vec<[f64; 6]>, values: &[f64]) {
    for chunk in values.chunks(6) {
        let mut row = [0.0f64; 6];
        row[..chunk.len()].copy_from_slice(chunk);
        rows.push(row);
    }
}

/// Build the output MF=1/451 descriptive section (`mixr.f90:197-255`),
/// ENDF-6 form.
///
/// Emits the HEAD, the two ENDF-6 control CONT records, the HDATIO control
/// record, one blank comment record, and the reaction directory (one
/// `(MFD=3, MTD)` entry per output MT). `emax` (eV) is the largest mixed
/// energy; `awi`, `nsub` use MIXR's default seeds. Comment **text** is not
/// stored (Hollerith limitation — see the module docs).
fn build_mf1_section(
    input: &MixrInput,
    awi: f64,
    emax: f64,
    nsub: i32,
) -> Section {
    let mat = input.output_mat;
    let za = input.output_za;
    let awr = input.output_awr;
    let temp = input.temperature_kelvin;
    let nmt = input.mt_list.len();
    const NWD: i32 = 1; // one (blank) comment record; see module docs.

    let mut rows: Vec<[f64; 6]> = vec![
        // mixr.f90:204-210 — HEAD: ZA, AWR, LRP=-1, LFI=0, NLIB=0, NMOD=0.
        [za, awr, -1.0, 0.0, 0.0, 0.0],
        // mixr.f90:213-219 — ELIS=0, STA=0, LIS=0, LISO=0, 0, NFOR=6.
        [0.0, 0.0, 0.0, 0.0, 0.0, 6.0],
        // mixr.f90:222-227 — AWI, EMAX, LREL=0, 0, NSUB, NVER=0.
        [awi, emax, 0.0, 0.0, nsub as f64, 0.0],
        // mixr.f90:231-238 — TEMP, 0, LDRV=1, 0, NWD, NXC=nmt.
        [temp, 0.0, 1.0, 0.0, NWD as f64, nmt as f64],
    ];
    // One blank comment record (description text not representable).
    rows.push([0.0; 6]);
    // mixr.f90:244-252 — directory: (blank, blank, MFD=3, MTD, NCD=0, MOD=0).
    for &mt in &input.mt_list {
        rows.push([0.0, 0.0, 3.0, mt as f64, 0.0, 0.0]);
    }
    Section { key: EndfKey { mat, mf: 1, mt: 451 }, rows }
}

/// Map an incident-particle atomic-weight ratio `awi` to its ENDF sublibrary
/// number `NSUB` (`mixr.f90:174-180`).
///
/// The default is incident-neutron (`NSUB = 10`, `mixr.f90:149`). MIXR overrides
/// it only when the first input's `AWI` falls in one of the recognised
/// charged-particle mass-ratio windows (`awi < 0.1` → gammas; ≈0.999 protons;
/// ≈1.996 deuterons; ≈2.9896 tritons; ≈2.9890 He-3; ≈3.967 alphas). An `awi`
/// outside every window (e.g. the incident-neutron `awi ≈ 1`) leaves `NSUB` at
/// its default 10.
pub fn nsub_from_awi(awi: f64) -> i32 {
    if awi < 0.1 {
        0
    } else if awi > 0.9980 && awi < 0.9990 {
        10010
    } else if awi > 1.9950 && awi < 1.9970 {
        10020
    } else if awi > 2.9895 && awi < 2.9897 {
        10030
    } else if awi > 2.9890 && awi < 2.9891 {
        20030
    } else if awi > 3.9670 && awi < 3.9680 {
        20040
    } else {
        10
    }
}

/// Read the MF=1/451 header seeds `(AWI, EMAX, NSUB)` from the input tapes,
/// mirroring the header scan `mixr.f90:147-181`.
///
/// `AWI` and `NSUB` come from the **first** contributing input's MF=1/451
/// (via [`nsub_from_awi`]); `EMAX` is raised to the largest `EMAX` word found on
/// any contributing input. Inputs without an MF=1/451 (bare PENDF MF=3 tapes)
/// leave the MIXR defaults untouched (`AWI = 1`, `EMAX = 20 MeV`, `NSUB = 10`),
/// exactly as Fortran MIXR does when `iverf <= 5`.
///
/// The ENDF-6 MF=1/451 third CONT row is `[AWI, EMAX, LREL, 0, NSUB, NVER]`
/// (`mixr.f90:222-227`); this reads word 1 (`AWI`) and word 2 (`EMAX`) from
/// `rows[2]`.
fn header_seeds(input: &MixrInput, inputs: &[Tape]) -> (f64, f64, i32) {
    let mut awi = 1.0_f64;
    let mut emax = 20.0e6_f64;
    let mut nsub = 10_i32;
    for (i, comp) in input.components.iter().enumerate() {
        let Some(tape) = inputs.get(comp.tape_index) else { continue };
        let Some(sec) = tape.section(comp.mat, 1, 451) else { continue };
        if sec.rows.len() < 3 {
            continue; // not an ENDF-6 header with an AWI/EMAX CONT row.
        }
        let row2 = sec.rows[2];
        if row2[1] > emax {
            emax = row2[1]; // mixr.f90:171 — raise emax for any material.
        }
        if i == 0 {
            // mixr.f90:172-180 — awi/nsub only from the first input.
            awi = row2[0];
            nsub = nsub_from_awi(awi);
        }
    }
    (awi, emax, nsub)
}

impl MixrInput {
    /// Run the MIXR mixing engine, returning a new in-memory PENDF
    /// [`Tape`] whose reactions are the weighted sums described by this input.
    ///
    /// Convenience wrapper over [`mix`]; `inputs` are the parsed input tapes,
    /// supplied in the same order as this input's card-1 `nin` list (so
    /// [`crate::mixr::MixComponent::tape_index`] indexes them).
    pub fn mix(&self, inputs: &[Tape]) -> Result<Tape, NjoyError> {
        mix(self, inputs)
    }
}

/// Run the MIXR mixing engine end to end (`mixr.f90:196-378`).
///
/// Produces a new PENDF [`Tape`] carrying:
/// - one MF=1/451 descriptive section for the output material, and
/// - one MF=3 section per output MT, each the union-grid weighted sum
///   `sigma_out(E) = sum_i w_i * sigma_i(E)` (barns), lin-lin.
///
/// `inputs` must be given in the same order as the card-1 `nin` list, so each
/// [`crate::mixr::MixComponent::tape_index`] indexes into it. Missing sections
/// are warned and skipped (`mixr.f90:283-287`); a reaction absent from every
/// input yields an empty MF=3 TAB1.
///
/// # Example
/// ```
/// use njoy_outram_park_fork::endf::tape::Tape;
/// use njoy_outram_park_fork::mixr::MixrInput;
///
/// # fn demo(tape_a: Tape, tape_b: Tape) -> Result<(), njoy_outram_park_fork::NjoyError> {
/// let input = MixrInput::from_cards(
///     20, &[0, 1], vec![2], &[(125, 0.5), (128, 0.5)],
///     0.0, 9999, 1001.0, 0.999, "half-and-half",
/// );
/// let out: Tape = input.mix(&[tape_a, tape_b])?;
/// assert!(out.section(9999, 3, 2).is_some());
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Propagates [`NjoyError`] from record parsing or interpolation.
pub fn mix(input: &MixrInput, inputs: &[Tape]) -> Result<Tape, NjoyError> {
    // Header seeds (AWI, EMAX, NSUB) read back from the input MF=1/451 headers
    // where present (mixr.f90:147-181); bare PENDF MF=3 inputs keep the MIXR
    // defaults AWI=1, EMAX=20 MeV, NSUB=10.
    let (awi, mut emax, nsub) = header_seeds(input, inputs);

    // Mix every reaction; EMAX (for the MF=1 header) is further raised to the
    // largest produced grid energy (mixr.f90 seeds emax then raises it, line 148).
    let mut mixed: Vec<(i32, Vec<(f64, f64)>)> = Vec::with_capacity(input.mt_list.len());
    for &mt in &input.mt_list {
        let pairs = mix_reaction(input, inputs, mt)?;
        if let Some(&(e, _)) = pairs.last() {
            if e > emax {
                emax = e;
            }
        }
        mixed.push((mt, pairs));
    }

    let mut sections: Vec<Section> = Vec::with_capacity(1 + mixed.len());
    sections.push(build_mf1_section(input, awi, emax, nsub));
    for (mt, pairs) in &mixed {
        sections.push(build_mf3_section(
            input.output_mat,
            input.output_za,
            input.output_awr,
            *mt,
            pairs,
        ));
    }

    let tpid = format!(
        " mixr mixed cross sections for mat {:>4}                          1 0  0    0",
        input.output_mat
    );
    Ok(Tape::from_sections(tpid, sections))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::{Section, Tape};

    // ----- helpers -------------------------------------------------------

    /// Build a one-material tape carrying a single MF=3 TAB1 for `mt` from
    /// `(E, sigma)` points (lin-lin, single region) — the minimal input MIXR
    /// needs. Layout mirrors `build_mf3_section` / `read_tab1`.
    fn tape_with_mf3(mat: i32, za: f64, awr: f64, mt: i32, pairs: &[(f64, f64)]) -> Tape {
        let ne = pairs.len();
        let mut rows: Vec<[f64; 6]> = vec![
            [za, awr, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0, ne as f64],
        ];
        // Interp table (NBT=ne, INT=2) on its own line, then (E, sigma) pairs
        // on fresh lines — the ENDF-6 layout the reader expects.
        push_packed(&mut rows, &[ne as f64, 2.0]);
        let mut pair_flat: Vec<f64> = Vec::with_capacity(2 * ne);
        for &(e, s) in pairs {
            pair_flat.push(e);
            pair_flat.push(s);
        }
        push_packed(&mut rows, &pair_flat);
        let sec = Section { key: EndfKey { mat, mf: 3, mt }, rows };
        Tape::from_sections(" test".to_string(), vec![sec])
    }

    /// Read the `(E, sigma)` points back out of an output MF=3 section.
    fn read_mf3_pairs(tape: &Tape, mat: i32, mt: i32) -> Vec<(f64, f64)> {
        let sec = tape.section(mat, 3, mt).expect("section present");
        let mut cur = SectionCursor::new(&sec.rows);
        let _head = cur.read_cont().unwrap(); // skip MF=3 HEAD
        cur.read_tab1().unwrap().pairs
    }

    // ----- sigfig --------------------------------------------------------

    #[test]
    fn sigfig_rounds_to_seven_figures() {
        // Methodology: sigfig(x,7,0) keeps 7 significant figures then applies
        // NJOY's 1.0000000000001 bias (util.f90:361-393). Result (2026-07-15):
        // sigfig(1.23456789,7,0) == 1.234568 to within the bias (~1.2e-13 rel).
        let y = sigfig(1.234_567_89, 7, 0);
        assert!((y - 1.234568).abs() < 1e-6, "got {y}");
        assert_eq!(sigfig(0.0, 7, 0), 0.0);
    }

    // ----- gety_value out-of-range conventions ---------------------------

    #[test]
    fn gety_value_boundaries_match_fortran() {
        // Methodology: gety returns 0 below the grid (mixr.f90:509-512) and the
        // last value at/above the grid (mixr.f90:514-517). Grid: (1,10)->(3,30)
        // lin-lin. Results (2026-07-15): below=0, in-range midpoint=20,
        // at-max=30, above-max=30 (held constant).
        let interp = [(2u32, 2u32)];
        let pairs = [(1.0, 10.0), (3.0, 30.0)];
        assert_eq!(gety_value(0.5, &interp, &pairs).unwrap(), 0.0); // below
        assert!((gety_value(2.0, &interp, &pairs).unwrap() - 20.0).abs() < 1e-12); // mid
        assert_eq!(gety_value(3.0, &interp, &pairs).unwrap(), 30.0); // at max
        assert_eq!(gety_value(9.9, &interp, &pairs).unwrap(), 30.0); // above -> held
    }

    // ----- mixing arithmetic --------------------------------------------

    #[test]
    fn identity_single_input_weight_one() {
        // Methodology: a single component with weight 1.0 must reproduce the
        // input cross section exactly on its own grid (mixr.f90:302 with one
        // term). Input sigma at E=(1,2,4)eV = (5,7,11)b. Result (2026-07-15):
        // output == input, bit-for-bit before sigfig.
        let a = tape_with_mf3(125, 1001.0, 0.999, 2, &[(1.0, 5.0), (2.0, 7.0), (4.0, 11.0)]);
        let input =
            MixrInput::from_cards(20, &[0], vec![2], &[(125, 1.0)], 0.0, 9999, 1001.0, 0.999, "id");
        let pairs = mix_reaction(&input, &[a], 2).unwrap();
        assert_eq!(pairs, vec![(1.0, 5.0), (2.0, 7.0), (4.0, 11.0)]);
    }

    #[test]
    fn half_and_half_is_average_on_shared_grid() {
        // Methodology (task V&V gate): two inputs on the SAME grid mixed with
        // weights (0.5, 0.5) give the pointwise average (mixr.f90:302).
        // A: (1,10)(2,20)(3,30); B: (1,0)(2,40)(3,50).
        // Result (2026-07-15): (1,5)(2,30)(3,40) exactly.
        let a = tape_with_mf3(1, 1.0, 1.0, 102, &[(1.0, 10.0), (2.0, 20.0), (3.0, 30.0)]);
        let b = tape_with_mf3(2, 2.0, 2.0, 102, &[(1.0, 0.0), (2.0, 40.0), (3.0, 50.0)]);
        let input = MixrInput::from_cards(
            20, &[0, 1], vec![102], &[(1, 0.5), (2, 0.5)], 0.0, 9999, 1.0, 1.0, "avg",
        );
        let pairs = mix_reaction(&input, &[a, b], 102).unwrap();
        assert_eq!(pairs, vec![(1.0, 5.0), (2.0, 30.0), (3.0, 40.0)]);
    }

    #[test]
    fn union_grid_interpolates_disjoint_energies() {
        // Methodology: inputs on DIFFERENT grids are interpolated onto the
        // union (mixr.f90:291-301). A: (1,10)(3,30) lin-lin; B: (2,100)(4,200).
        // Union = {1,2,3,4}. Weights 1,1.
        //   E=1: A=10,  B=0 (below B) -> 10
        //   E=2: A=20,  B=100         -> 120
        //   E=3: A=30,  B=150         -> 180
        //   E=4: A=30 (held above A), B=200 -> 230
        // Result verified 2026-07-15.
        let a = tape_with_mf3(1, 1.0, 1.0, 1, &[(1.0, 10.0), (3.0, 30.0)]);
        let b = tape_with_mf3(2, 2.0, 2.0, 1, &[(2.0, 100.0), (4.0, 200.0)]);
        let input = MixrInput::from_cards(
            20, &[0, 1], vec![1], &[(1, 1.0), (2, 1.0)], 0.0, 9999, 1.0, 1.0, "union",
        );
        let pairs = mix_reaction(&input, &[a, b], 1).unwrap();
        assert_eq!(
            pairs,
            vec![(1.0, 10.0), (2.0, 120.0), (3.0, 180.0), (4.0, 230.0)]
        );
    }

    #[test]
    fn additivity_weighted_sum() {
        // Methodology: linearity — mixing with weights (2,3) equals 2*A + 3*B
        // pointwise on the shared grid (mixr.f90:302). A:(1,1)(2,2);
        // B:(1,10)(2,20). Result (2026-07-15): (1, 2*1+3*10=32)(2, 2*2+3*20=64).
        let a = tape_with_mf3(1, 1.0, 1.0, 2, &[(1.0, 1.0), (2.0, 2.0)]);
        let b = tape_with_mf3(2, 2.0, 2.0, 2, &[(1.0, 10.0), (2.0, 20.0)]);
        let input = MixrInput::from_cards(
            20, &[0, 1], vec![2], &[(1, 2.0), (2, 3.0)], 0.0, 9999, 1.0, 1.0, "add",
        );
        let pairs = mix_reaction(&input, &[a, b], 2).unwrap();
        assert_eq!(pairs, vec![(1.0, 32.0), (2.0, 64.0)]);
    }

    #[test]
    fn zero_where_one_input_absent_below_range() {
        // Methodology: a component contributes 0 below its own grid
        // (mixr.f90:509-512), so where only one input is defined the mix equals
        // that input's weighted value. A:(1,10)(2,20) w=1; B:(2,100)(3,300) w=1.
        // Union {1,2,3}. E=1: only A -> 10. E=2: 20+100=120. E=3: A held 20 +
        // B 300 = 320. Result 2026-07-15.
        let a = tape_with_mf3(1, 1.0, 1.0, 5, &[(1.0, 10.0), (2.0, 20.0)]);
        let b = tape_with_mf3(2, 2.0, 2.0, 5, &[(2.0, 100.0), (3.0, 300.0)]);
        let input = MixrInput::from_cards(
            20, &[0, 1], vec![5], &[(1, 1.0), (2, 1.0)], 0.0, 9999, 1.0, 1.0, "zero",
        );
        let pairs = mix_reaction(&input, &[a, b], 5).unwrap();
        assert_eq!(pairs, vec![(1.0, 10.0), (2.0, 120.0), (3.0, 320.0)]);
    }

    // ----- end-to-end tape assembly + round-trip ------------------------

    #[test]
    fn mix_produces_roundtrippable_tape() {
        // Methodology: the full engine builds a Tape with MF=1/451 + MF=3, and
        // the MF=3 values round-trip through this crate's TAB1 reader (with the
        // 7-figure sigfig applied). Two inputs averaged; check MT=2 pairs and
        // that the directory/File-1 section exists. Result 2026-07-15: pairs
        // equal the sigfig'd average; both sections present.
        let a = tape_with_mf3(125, 1001.0, 0.999, 2, &[(1.0, 10.0), (2.0, 20.0)]);
        let b = tape_with_mf3(128, 1001.0, 0.999, 2, &[(1.0, 30.0), (2.0, 40.0)]);
        let input = MixrInput::from_cards(
            20, &[0, 1], vec![2], &[(125, 0.5), (128, 0.5)], 0.0, 9999, 1001.0, 0.999, "mix",
        );
        let out = input.mix(&[a, b]).unwrap();
        assert!(out.section(9999, 1, 451).is_some(), "MF=1/451 present");
        let pairs = read_mf3_pairs(&out, 9999, 2);
        // averages are 20 and 30; sigfig(20,7,0)~20, sigfig(30,7,0)~30.
        assert_eq!(pairs.len(), 2);
        assert!((pairs[0].1 - 20.0).abs() < 1e-4, "got {}", pairs[0].1);
        assert!((pairs[1].1 - 30.0).abs() < 1e-4, "got {}", pairs[1].1);

        // The whole tape must serialise without error.
        let mut buf = Vec::new();
        out.write(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("9999"), "output mat number appears in tape");
    }

    /// Build a one-material tape carrying an ENDF-6 MF=1/451 header (with a
    /// chosen AWI/EMAX) followed by a single MF=3 TAB1, so the header-seed scan
    /// has something to read. Header rows mirror `build_mf1_section`.
    fn tape_with_header_and_mf3(
        mat: i32,
        za: f64,
        awr: f64,
        awi: f64,
        emax: f64,
        nsub: i32,
        mt: i32,
        pairs: &[(f64, f64)],
    ) -> Tape {
        let mf1 = Section {
            key: EndfKey { mat, mf: 1, mt: 451 },
            rows: vec![
                [za, awr, -1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0, 0.0, 6.0],
                [awi, emax, 0.0, 0.0, nsub as f64, 0.0],
                [0.0, 0.0, 1.0, 0.0, 1.0, 1.0],
                [0.0; 6],
                [0.0, 0.0, 3.0, mt as f64, 0.0, 0.0],
            ],
        };
        let base = tape_with_mf3(mat, za, awr, mt, pairs);
        let mut secs = vec![mf1];
        secs.extend(base.sections().iter().cloned());
        Tape::from_sections(" test".to_string(), secs)
    }

    #[test]
    fn nsub_from_awi_windows_match_fortran() {
        // Methodology: nsub_from_awi mirrors mixr.f90:174-180. Probe each window
        // plus the default. Result (2026-07-15): awi=1 -> 10 (neutron default),
        // 0.9985 -> 10010 (protons), 0.05 -> 0 (gammas), 3.9675 -> 20040 (alphas).
        assert_eq!(nsub_from_awi(1.0), 10);
        assert_eq!(nsub_from_awi(0.9985), 10010);
        assert_eq!(nsub_from_awi(0.05), 0);
        assert_eq!(nsub_from_awi(1.9960), 10020);
        assert_eq!(nsub_from_awi(2.9896), 10030);
        assert_eq!(nsub_from_awi(3.9675), 20040);
    }

    #[test]
    fn header_seeds_read_awi_emax_nsub_from_first_input() {
        // Methodology (task V&V gate — AWI/EMAX/NSUB fidelity): the output
        // MF=1/451 must carry AWI/NSUB from the FIRST input's header and EMAX
        // raised to the largest input EMAX (mixr.f90:147-181). First input carries
        // AWI=0.9986 (proton window -> NSUB=10010) and EMAX=3e7; second carries a
        // larger EMAX=1e8. Result (2026-07-15): output row2 = [0.9986, 1e8, 0, 0,
        // 10010, 0].
        let a = tape_with_header_and_mf3(
            125, 1001.0, 0.999, 0.9986, 3.0e7, 0, 2, &[(1.0, 10.0), (2.0, 20.0)],
        );
        let b = tape_with_header_and_mf3(
            128, 1001.0, 0.999, 2.0, 1.0e8, 10, 2, &[(1.0, 30.0), (2.0, 40.0)],
        );
        let input = MixrInput::from_cards(
            20, &[0, 1], vec![2], &[(125, 0.5), (128, 0.5)], 0.0, 9999, 1001.0, 0.999, "mix",
        );
        let out = input.mix(&[a, b]).unwrap();
        let sec = out.section(9999, 1, 451).expect("MF=1/451 present");
        let row2 = sec.rows[2];
        assert!((row2[0] - 0.9986).abs() < 1e-9, "AWI from first input: {}", row2[0]);
        assert!((row2[1] - 1.0e8).abs() < 1.0, "EMAX raised to largest: {}", row2[1]);
        assert_eq!(row2[4].round() as i32, 10010, "NSUB from first input AWI");
    }

    #[test]
    fn header_seeds_default_for_bare_pendf() {
        // Methodology: inputs without MF=1/451 (bare MF=3 PENDF) leave the MIXR
        // defaults AWI=1, EMAX=20 MeV (unless a mixed grid energy exceeds it),
        // NSUB=10 (mixr.f90:147-149). Result (2026-07-15): output row2 = [1, 2e7,
        // 0, 0, 10, 0] since the grid maxes at 2 eV << 20 MeV.
        let a = tape_with_mf3(125, 1001.0, 0.999, 2, &[(1.0, 10.0), (2.0, 20.0)]);
        let input = MixrInput::from_cards(
            20, &[0], vec![2], &[(125, 1.0)], 0.0, 9999, 1001.0, 0.999, "id",
        );
        let out = input.mix(&[a]).unwrap();
        let sec = out.section(9999, 1, 451).unwrap();
        let row2 = sec.rows[2];
        assert_eq!(row2[0], 1.0);
        assert_eq!(row2[1], 20.0e6);
        assert_eq!(row2[4].round() as i32, 10);
    }

    #[test]
    fn missing_reaction_yields_empty_grid() {
        // Methodology: a requested MT present on no input is warned and returns
        // an empty grid (mixr.f90:283-287). Result 2026-07-15: empty vec.
        let a = tape_with_mf3(125, 1001.0, 0.999, 2, &[(1.0, 5.0)]);
        let input = MixrInput::from_cards(
            20, &[0], vec![18], &[(125, 1.0)], 0.0, 9999, 1001.0, 0.999, "miss",
        );
        let pairs = mix_reaction(&input, &[a], 18).unwrap();
        assert!(pairs.is_empty());
    }
}
