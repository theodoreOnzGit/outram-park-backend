//! Convert sparse-matrix indices between the plain and half-index (diamond
//! difference) numberings.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `convertindexc2d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::types::Params;

/// The two index numberings the reference converts between.
///
/// The MATLAB passes these as the bare integers `1` and `2`, documented in
/// `convertsparseformat2d.m` as "mode 1 normal" and "mode 2 diamond difference
/// (half indices)".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexMode {
    /// Mode 1 — one index per node.
    Plain,
    /// Mode 2 — the `(2n+1)` grid carrying cell edges as well as centres.
    DiamondDifference,
}

/// `newvec = convertindexc2d(params, vec, frommode, tomode)`.
///
/// Maps a list of linear indices from one numbering to the other, routing
/// through mode 1 as the intermediate: the reference converts *to* mode 1
/// first, then *from* mode 1 to the requested output.
///
/// # Arguments
///
/// - `params` — supplies `G` and the extents.
/// - `vec` — **0-based** linear indices to convert.
/// - `frommode`, `tomode` — the source and destination numberings.
///
/// # Returns
///
/// The converted **0-based** indices, as `f64` — see below.
///
/// # This is the one place the arithmetic stays 1-based
///
/// The rest of the port converts index formulas to 0-based. This function does
/// not, and the exception is deliberate.
///
/// What it computes is a *mapping between two index spaces* whose definition —
/// the `(2n+1)` half-index grid interleaving cell centres and edges — is stated
/// in 1-based terms. The `+1`/`-1` offsets are not incidental there; they set
/// where centres fall relative to edges. Rewriting the interior in 0-based
/// means re-deriving that interleaving, which is error-prone for no gain.
///
/// So the boundary converts and the interior is transcribed verbatim: `+ 1.0`
/// on entry, the reference's formulas unchanged, `- 1.0` on exit. Callers see
/// 0-based indices throughout, consistent with everything else.
///
/// # Why the return type is `f64`
///
/// The mode-2 → mode-1 branch computes
///
/// ```text
/// ix=ceil(mod((vec(i)-1),energystep2)/xstep2)/2;
/// iy=(mod(mod((vec(i)-1),energystep2),xstep2)+1)/2;
/// ```
///
/// Both end in a division by 2 that is **not** rounded. For an index sitting on
/// a cell edge rather than a centre, `ix` and `iy` come out half-integer and
/// the result is fractional. MATLAB carries this silently in double precision;
/// returning an integer type here would quietly round it and change behaviour.
///
/// The one caller, `convertsparseformat2d`, feeds the result into a sparse
/// assembly that rejects non-integer subscripts, so a fractional result
/// surfaces as an error there — exactly as in MATLAB.
///
/// # Reference defect — the two directions are not inverse
///
/// **A mode 1 → mode 2 → mode 1 round trip does not return the original
/// indices.** This was found by running the translation, not by reading it.
///
/// With `G = 1`, `maxi1 = maxi2 = 2` (so `energystep1 = 4`, `xstep1 = 2`,
/// `energystep2 = 25`, `xstep2 = 5`), the 1-based indices `1, 2, 3, 4` map
/// forward to `2, 14, 12, 24` and back to `2, 5, 4, 7`.
///
/// The cause is in the forward direction. It computes the row as
///
/// ```text
/// ix = ceil(mod(t-1, energystep1) / xstep1) * 2
/// ```
///
/// but `ceil(local / xstep1)` is the row of a *1-based* position, while `local`
/// is 0-based. The two agree only when `local` is not a multiple of `xstep1`:
/// at `local = 0` it yields row `0`, where `floor(local / xstep1) + 1` would
/// give row `1`. So the first node of every row lands one row low, off the
/// even-numbered centre positions the `(2n+1)` grid reserves for node centres.
/// The reverse direction, which divides by 2 instead of multiplying, does not
/// make the same error, so the two do not compose to the identity.
///
/// Translated as written, per the no-silent-repairs rule in
/// the crate README, "Translation policy". The test below pins the wrong behaviour
/// so that correcting it is a visible, deliberate change with before/after
/// numbers.
///
/// **Blast radius is unknown and worth establishing before this is relied on.**
/// The only caller is [`crate::convertsparseformat2d`], which is not yet
/// reached by any translated code path, so nothing currently depends on the
/// mapping being right.
///
/// # Reference quirks carried over
///
/// - **Extents are read directly**, as `params.maxi1` and `params.maxi2`, *not*
///   through `handle2dcoords`. Its caller `convertsparseformat2d.m` does use
///   `handle2dcoords`. A `params` carrying `maxix`/`maxiy` but no
///   `maxi1`/`maxi2` therefore passes the check in the caller and fails here.
/// - **`philenf1` and `philenf2` are computed and never used** — dead code in
///   the reference, omitted from the body but noted so a reader diffing against
///   the `.m` file is not surprised.
///
/// # Panics
///
/// If `params.maxi1` or `params.maxi2` is absent, mirroring MATLAB's
/// `Reference to non-existent field`.
pub fn convertindexc2d(
    params: &Params,
    vec: &[f64],
    frommode: IndexMode,
    tomode: IndexMode,
) -> Vec<f64> {
    let g_count = params.g as f64;
    let maxi1 = params
        .maxi1
        .expect("Reference to non-existent field 'maxi1'") as f64;
    let maxi2 = params
        .maxi2
        .expect("Reference to non-existent field 'maxi2'") as f64;

    let philen1 = g_count * maxi1 * maxi2;
    let philen2 = g_count * (2.0 * maxi1 + 1.0) * (2.0 * maxi2 + 1.0);
    let energystep1 = maxi1 * maxi2;
    let energystep2 = (2.0 * maxi1 + 1.0) * (2.0 * maxi2 + 1.0);
    let xstep1 = maxi2;
    let xstep2 = 2.0 * maxi2 + 1.0;

    // Into the reference's 1-based index space.
    let one_based: Vec<f64> = vec.iter().map(|v| v + 1.0).collect();

    // --- convert from `frommode` to mode 1 --------------------------------
    let tempvec: Vec<f64> = match frommode {
        IndexMode::Plain => one_based,
        IndexMode::DiamondDifference => one_based
            .iter()
            .map(|v| {
                if *v > philen2 {
                    // Past the flux block: the precursor tail shifts by the
                    // difference in block lengths.
                    *v - philen2 + philen1
                } else {
                    let g = ((*v - 1.0) / energystep2).floor();
                    let ix = ((*v - 1.0).rem_euclid(energystep2) / xstep2).ceil() / 2.0;
                    let iy = ((*v - 1.0).rem_euclid(energystep2).rem_euclid(xstep2) + 1.0) / 2.0;
                    g * energystep1 + ix * xstep1 + iy
                }
            })
            .collect(),
    };

    // --- convert from mode 1 to `tomode` ----------------------------------
    let out: Vec<f64> = match tomode {
        IndexMode::Plain => tempvec,
        IndexMode::DiamondDifference => tempvec
            .iter()
            .map(|t| {
                if *t > philen1 {
                    *t - philen1 + philen2
                } else {
                    let g = ((*t - 1.0) / energystep1).floor();
                    let ix = ((*t - 1.0).rem_euclid(energystep1) / xstep1).ceil() * 2.0;
                    let iy = ((*t - 1.0).rem_euclid(energystep1).rem_euclid(xstep1) + 1.0) * 2.0;
                    g * energystep2 + ix * xstep2 + iy
                }
            })
            .collect(),
    };

    // Back out to 0-based.
    out.iter().map(|v| v - 1.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            maxi1: Some(2),
            maxi2: Some(2),
            g: 1,
            nc: Some(0),
            ..Default::default()
        }
    }

    #[test]
    fn plain_to_plain_is_the_identity() {
        let v = [0.0, 1.0, 2.0, 3.0];
        let out = convertindexc2d(&params(), &v, IndexMode::Plain, IndexMode::Plain);
        assert_eq!(out, vec![0.0, 1.0, 2.0, 3.0]);
    }

    /// Pins the reference defect described in the doc comment: a round trip
    /// through the half-index numbering does **not** return the original
    /// indices.
    ///
    /// Measured 2026-08-12. In 1-based terms the indices `1, 2, 3, 4` go
    /// forward to `2, 14, 12, 24` and back to `2, 5, 4, 7`. The 0-based
    /// equivalents are asserted below.
    ///
    /// If a later change makes this test fail by producing the identity, that
    /// is the defect being fixed — update the test and record the before/after
    /// rather than reverting.
    #[test]
    fn round_trip_is_not_the_identity_reference_defect() {
        let p = params();
        let v = [0.0, 1.0, 2.0, 3.0];

        let to_dd = convertindexc2d(&p, &v, IndexMode::Plain, IndexMode::DiamondDifference);
        assert_eq!(to_dd, vec![1.0, 13.0, 11.0, 23.0]);

        let back = convertindexc2d(&p, &to_dd, IndexMode::DiamondDifference, IndexMode::Plain);
        assert_eq!(back, vec![1.0, 4.0, 3.0, 6.0]);
        assert_ne!(back, v.to_vec());
    }

    /// Pins the `f64` return type: an edge-centred mode-2 index converts to a
    /// half-integer, which is why the signature cannot use an integer type.
    #[test]
    fn edge_indices_convert_to_half_integers() {
        let p = params();
        // 0-based index 0 is the corner of the 5x5 half-index grid, not a node
        // centre.
        let out = convertindexc2d(&p, &[0.0], IndexMode::DiamondDifference, IndexMode::Plain);
        assert_ne!(out[0].fract(), 0.0);
    }
}
