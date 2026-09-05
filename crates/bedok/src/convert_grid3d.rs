//! Build the compaction map between the full rectangular grid and the
//! fuelled-node-only unknown vector.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `convert_grid3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::Array3;
use crate::types::Params;

/// `[key, reversekey] = convert_grid3d(params, whichsigma)`.
///
/// The solvers assemble on the full `(G + Nc) * maxix * maxiy * maxiz`
/// rectangular grid but only want to *solve* on the nodes that carry material.
/// This builds the two lookup tables that move between the numberings:
///
/// - `key[full_index]` → `Some(compacted_index)`, or `None` if that node
///   carries no material.
/// - `reversekey[compacted_index]` → the full-grid index it came from.
///
/// # Why `key` is `Option<usize>` and not a plain index
///
/// The reference stores `0` to mean "no material here", which is unambiguous
/// only because MATLAB indices start at 1. Once the translation moved to
/// 0-based indexing, `0` became a perfectly valid compacted index and the
/// sentinel stopped working.
///
/// Rather than pick a different magic value, the map carries `Option<usize>`.
/// The two places the reference wrote `key(...) == 0` now read `is_none()`.
/// Same information, no ambiguity — and it is the one place in this port where
/// the reindexing was not a mechanical rewrite.
///
/// `reversekey` needs no such treatment: it is only read below the fuelled-node
/// count, and is zero-padded above it exactly as the reference leaves it.
///
/// # Arguments
///
/// - `params` — supplies `G`, `Nc` (defaulting to `0` when absent — this is
///   the one site that guards the field) and the extents.
/// - `whichsigma` — material index per node, `0` meaning no material.
///
/// # Returns
///
/// `(key, reversekey)`, both of length `(G + Nc) * maxix * maxiy * maxiz`.
/// Dimensionless indices.
///
/// # Reference defect — precursor indices collide when `Nc > 1`
///
/// Inside the precursor loop the reference computes
///
/// ```text
/// idx=(G+Nc-1)*energyindexstep+(ix-1)*xstep+(iy-1)*maxiz+iz;
/// ```
///
/// The expression does not depend on the loop variable `nn`, so **every**
/// precursor family in a node maps to the same full-grid index. Each pass
/// overwrites the entry with a fresh counter, and the `reversekey` entries for
/// the earlier families point at an index whose `key` no longer refers back to
/// them. From the surrounding code the intent was `(G+nn-1)`.
///
/// **This is harmless at `Nc == 1`** — both expressions give the same block —
/// and every benchmark case in this snapshot that populates `Nc` uses a single
/// family, which is presumably why it went unnoticed. It corrupts the map for
/// any `Nc > 1`.
///
/// Translated as written, per the no-silent-repairs rule in
/// `docs/bedok-port-scoping.md` §1.0. A fix belongs in stage 2 with
/// before/after numbers, not here.
pub fn convert_grid3d(
    params: &Params,
    whichsigma: &Array3<usize>,
) -> (Vec<Option<usize>>, Vec<usize>) {
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let g_count = params.g;
    let nc = params.nc_or_zero();

    let energyindexstep = maxix * maxiy * maxiz;
    let xstep = maxiy * maxiz;
    let philenf = (g_count + nc) * maxix * maxiy * maxiz;

    let mut counter = 0usize;
    let mut key: Vec<Option<usize>> = vec![None; philenf];
    let mut reversekey: Vec<usize> = vec![0; philenf];

    for ix in 0..maxix {
        for iy in 0..maxiy {
            for iz in 0..maxiz {
                if whichsigma.get(ix, iy, iz) != 0 {
                    for g in 0..g_count {
                        let idx = g * energyindexstep + ix * xstep + iy * maxiz + iz;
                        key[idx] = Some(counter);
                        reversekey[counter] = idx;
                        counter += 1;
                    }
                    if nc != 0 {
                        for _nn in 0..nc {
                            // REFERENCE DEFECT: the block offset does not vary
                            // with the loop variable — see the doc comment.
                            let idx =
                                (g_count + nc - 1) * energyindexstep + ix * xstep + iy * maxiz + iz;
                            key[idx] = Some(counter);
                            reversekey[counter] = idx;
                            counter += 1;
                        }
                    }
                }
            }
        }
    }

    (key, reversekey)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single_node(g: usize, nc: Option<usize>) -> (Params, Array3<usize>) {
        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(1),
            g,
            nc,
            ..Default::default()
        };
        let mut whichsigma = Array3::<usize>::zeros(1, 1, 1);
        whichsigma.set(0, 0, 0, 1);
        (params, whichsigma)
    }

    #[test]
    fn only_fuelled_nodes_are_numbered() {
        let params = Params {
            maxix: Some(2),
            maxiy: Some(1),
            maxiz: Some(1),
            g: 1,
            ..Default::default()
        };
        let mut whichsigma = Array3::<usize>::zeros(2, 1, 1);
        whichsigma.set(0, 0, 0, 1);

        let (key, reversekey) = convert_grid3d(&params, &whichsigma);
        assert_eq!(key[0], Some(0));
        // The void node is absent from the solve.
        assert_eq!(key[1], None);
        assert_eq!(reversekey[0], 0);
    }

    #[test]
    fn groups_are_numbered_consecutively_within_a_node() {
        let (params, whichsigma) = single_node(2, None);
        let (key, _) = convert_grid3d(&params, &whichsigma);
        assert_eq!(key[0], Some(0));
        assert_eq!(key[1], Some(1));
    }

    /// Pins the collision described in the doc comment: with `Nc = 2` both
    /// precursor families land on the same full-grid index, so only the last
    /// counter survives and the first family's slot is never written.
    #[test]
    fn precursor_indices_collide_when_nc_exceeds_one() {
        let (params, whichsigma) = single_node(1, Some(2));
        let (key, reversekey) = convert_grid3d(&params, &whichsigma);

        // Both precursor passes target block (1 + 2 - 1) = 2, i.e. index 2.
        assert_eq!(reversekey[1], 2);
        assert_eq!(reversekey[2], 2);
        // Only the second pass's counter survives there.
        assert_eq!(key[2], Some(2));
        // Index 1 — where the first family should have gone — is never written.
        assert_eq!(key[1], None);
    }

    /// The same expression is correct at `Nc == 1`, which is why the defect is
    /// latent in every benchmark case this snapshot ships.
    #[test]
    fn a_single_precursor_family_maps_correctly() {
        let (params, whichsigma) = single_node(1, Some(1));
        let (key, reversekey) = convert_grid3d(&params, &whichsigma);
        assert_eq!(key[0], Some(0));
        assert_eq!(key[1], Some(1));
        assert_eq!(reversekey[1], 1);
    }
}
