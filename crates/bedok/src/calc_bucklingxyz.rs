//! The buckling operators of the semi-analytic nodal update.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_bucklingxyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use std::collections::HashMap;

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::SparseMatrix;
use crate::types::{Geometry, Params, Sigma};

/// The buckling operator on each axis, `philen` square.
#[derive(Clone, Debug, Default)]
pub struct Buckling {
    /// `Buck.x`.
    pub x: SparseMatrix,
    /// `Buck.y`.
    pub y: SparseMatrix,
    /// `Buck.z`.
    pub z: SparseMatrix,
}

/// Fingerprint of the `keff`-independent inputs.
///
/// Reproduces the reference's `newkey` row vector field for field. Comparison
/// is exact equality on `f64`, which is what MATLAB's `isequal` does — and
/// which, like `isequal`, treats a `NaN` anywhere as never matching, forcing a
/// rebuild.
///
/// # This is a weak fingerprint, and that is the reference's design
///
/// Three sums and three non-zero counts cannot distinguish every pair of
/// distinct cross-section sets. Two states that differ by a permutation of
/// values, or by compensating changes, collide — and a collision means the
/// cached coefficients are silently reused for the wrong inputs.
///
/// This is translated as written. It is a real risk rather than a theoretical
/// one for a T-H feedback loop, where the cross sections change by small
/// amounts every pass and a cancelling pair is not far-fetched. Recorded here
/// rather than strengthened, per `docs/bedok-port-scoping.md` §1.0.
#[derive(Clone, Debug, PartialEq)]
struct BucklingKey {
    philen: usize,
    g: usize,
    nnz_tot: usize,
    nnz_s: usize,
    nnz_f: usize,
    sum_tot: f64,
    sum_s: f64,
    sum_f: f64,
    sum_diff: f64,
    sum_l: f64,
}

/// The cached `keff`-independent part of the buckling assembly.
///
/// # Why this exists as a struct
///
/// The reference holds this in MATLAB `persistent` variables — function-scoped
/// state that survives between calls for the lifetime of the process. Rust has
/// no equivalent that is not global mutable state, so the cache is an explicit
/// value the caller owns and passes by `&mut`.
///
/// **The deviation, stated plainly:** MATLAB's cache is *per process and shared
/// by every caller*; this one is *per `BucklingCache` value*. Two solvers
/// running in sequence share one cache in MATLAB and would have separate ones
/// here unless the same value is threaded through. That cannot produce a wrong
/// answer — the fingerprint guards correctness either way, and a fresh cache
/// simply rebuilds on first use — but it does mean the *number of rebuilds* can
/// differ from the reference. Nothing downstream depends on that count.
///
/// Create with [`BucklingCache::new`] and keep it alive across the `keff`
/// iterations of a solve; that is where it pays.
#[derive(Clone, Debug, Default)]
pub struct BucklingCache {
    key: Option<BucklingKey>,
    row: Vec<usize>,
    col: Vec<usize>,
    aval: Vec<f64>,
    bval: Vec<f64>,
    lxe: Vec<f64>,
    lye: Vec<f64>,
    lze: Vec<f64>,
    de: Vec<f64>,
    plen: usize,
}

impl BucklingCache {
    /// An empty cache, which rebuilds on its first use.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the cache currently holds a built assembly.
    ///
    /// Exposed for tests and diagnostics; the solver never needs to ask.
    pub fn is_populated(&self) -> bool {
        self.key.is_some()
    }
}

/// `Buck = calc_bucklingxyz(params, geometry, sigma, diffvalues, keff)`.
///
/// Assembles the three buckling operators. Each entry is
///
/// $$ \left(\Sigma_{tot} - \Sigma_s - \frac{\Sigma_f}{k_{eff}}\right) \cdot \frac{L^2}{4 D} $$
///
/// evaluated at the node's width `L` along that axis and its diffusion
/// coefficient `D`, over the `(group, group)` block at each node.
///
/// # Arguments
///
/// - `cache` — the `keff`-independent assembly; see [`BucklingCache`].
/// - `params` — supplies `G` and the extents.
/// - `geometry` — supplies the per-node widths `lx`, `ly`, `lz`.
/// - `sigma` — `tot`, `s` and `f`, all `philen` square.
/// - `diffvalues` — **a flat `philen` vector**, not the 4-D array. See below.
/// - `keff` — the current eigenvalue estimate.
///
/// # `diffvalues` is flat here, unlike in `calc_ABEFGHxyz`
///
/// This is the one thing to get right at a call site. The reference indexes
/// `diffvalues` linearly with no `permute`, and its fingerprint uses
/// `sum(diffvalues)` as a scalar — both only work if the argument is already a
/// `philen` vector.
///
/// The sole caller, `calc_a1234_expansionxyz.m`, passes `diffvaluesD`, which is
/// exactly that: the 4-D array flattened to
/// `g*es + ix*maxiy*maxiz + iy*maxiz + iz`. A commented-out block at the top of
/// that file shows the loop that used to build it.
///
/// Its sibling [`crate::calc_abefghxyz::calc_abefghxyz`] takes the **4-D**
/// array and flattens internally. The asymmetry is the reference's.
///
/// Note also that the flattening in that commented-out block substituted
/// `1000000` for zero entries "to prevent division by 0 later". That
/// substitution is **not** applied to the vector reaching this function — the
/// caller keeps genuine zeros and applies the substitution separately, to a
/// copy named `diffvaluesDfix`, used only for a different division. So zeros
/// arrive here intact, and the void-skip below is what keeps them out of the
/// denominator.
///
/// # Which nodes are skipped
///
/// A node is skipped when `diffvalues[node] == 0`, testing the **group-1**
/// entry only — the index carries no group offset. A node whose first group has
/// a non-zero `D` but some later group has zero would pass the test and then
/// divide by that zero, yielding an infinite entry. `calcdiffvalues3d` fills
/// all groups of a node together, so the mixed case cannot arise from it.
///
/// # Returns
///
/// [`Buckling`] — three `philen`-square sparse matrices.
///
/// # Panics
///
/// If the assembled entry count exceeds `philen * G`, reproducing the
/// reference's `error('Error in calc_buckling')`. This is defensive in both:
/// the count is at most `(number of live nodes) * G * G <= philen * G`.
pub fn calc_bucklingxyz(
    cache: &mut BucklingCache,
    params: &Params,
    geometry: &Geometry,
    sigma: &mut Sigma,
    diffvalues: &[f64],
    keff: f64,
) -> Buckling {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    let tot = sigma.tot.find();
    let s = sigma.s.find();
    let f = sigma.f.find();

    // `repmat(Lx, G, 1)` is a view: entry `idx` reads node `idx % es`.
    let sum_l: f64 = (0..philen)
        .map(|idx| geometry.lx[idx % es] + geometry.ly[idx % es] + geometry.lz[idx % es])
        .sum();

    let newkey = BucklingKey {
        philen,
        g: g_count,
        nnz_tot: tot.len(),
        nnz_s: s.len(),
        nnz_f: f.len(),
        sum_tot: tot.iter().map(|t| t.v).sum(),
        sum_s: s.iter().map(|t| t.v).sum(),
        sum_f: f.iter().map(|t| t.v).sum(),
        sum_diff: diffvalues.iter().sum(),
        sum_l,
    };

    if cache.key.as_ref() != Some(&newkey) {
        // `Aterm = sigma.tot - sigma.s`, `Bterm = sigma.f`. Only scattered
        // single entries are read, so the difference is held as a lookup rather
        // than formed as a matrix. A missing key reads as 0, which is what
        // indexing a MATLAB sparse at a structural zero gives.
        let mut aterm: HashMap<(usize, usize), f64> = HashMap::new();
        for t in tot.iter() {
            *aterm.entry((t.i, t.j)).or_insert(0.0) += t.v;
        }
        for t in s.iter() {
            *aterm.entry((t.i, t.j)).or_insert(0.0) -= t.v;
        }
        let mut bterm: HashMap<(usize, usize), f64> = HashMap::new();
        for t in f.iter() {
            *bterm.entry((t.i, t.j)).or_insert(0.0) += t.v;
        }

        let mut row = Vec::new();
        let mut col = Vec::new();
        let mut aval = Vec::new();
        let mut bval = Vec::new();
        let mut lxe = Vec::new();
        let mut lye = Vec::new();
        let mut lze = Vec::new();
        let mut de = Vec::new();

        for ix in 0..maxix {
            for iy in 0..maxiy {
                for iz in 0..maxiz {
                    let node = ix * xstep + iy * maxiz + iz;
                    // Group-1 entry only — see "Which nodes are skipped".
                    if diffvalues[node] == 0.0 {
                        continue;
                    }
                    for g in 0..g_count {
                        let idx = g * es + node;
                        // `idxvec` — the same node across every group.
                        for gg in 0..g_count {
                            let column = gg * es + node;
                            row.push(idx);
                            col.push(column);
                            aval.push(aterm.get(&(idx, column)).copied().unwrap_or(0.0));
                            bval.push(bterm.get(&(idx, column)).copied().unwrap_or(0.0));
                            lxe.push(geometry.lx[node]);
                            lye.push(geometry.ly[node]);
                            lze.push(geometry.lz[node]);
                            de.push(diffvalues[idx]);
                        }
                    }
                }
            }
        }

        assert!(
            row.len() <= philen * g_count,
            "Error in calc_buckling: assembled {} entries, limit {}",
            row.len(),
            philen * g_count
        );

        cache.row = row;
        cache.col = col;
        cache.aval = aval;
        cache.bval = bval;
        cache.lxe = lxe;
        cache.lye = lye;
        cache.lze = lze;
        cache.de = de;
        cache.plen = philen;
        cache.key = Some(newkey);
    }

    // Per-call assembly: only the keff-dependent values change.
    // `Bt == (sigma.tot - sigma.s - sigma.f/keff)` at the cached entries.
    let bt: Vec<f64> = cache
        .aval
        .iter()
        .zip(cache.bval.iter())
        .map(|(a, b)| a - b / keff)
        .collect();

    // Element order mirrors the reference's `Bt*0.25 .* L .* L ./ D` exactly;
    // reassociating would change the floating-point result.
    let scale = |l: &[f64]| -> Vec<f64> {
        (0..bt.len())
            .map(|n| bt[n] * 0.25 * l[n] * l[n] / cache.de[n])
            .collect()
    };

    Buckling {
        x: SparseMatrix::assemble(
            &cache.row,
            &cache.col,
            &scale(&cache.lxe),
            cache.plen,
            cache.plen,
        ),
        y: SparseMatrix::assemble(
            &cache.row,
            &cache.col,
            &scale(&cache.lye),
            cache.plen,
            cache.plen,
        ),
        z: SparseMatrix::assemble(
            &cache.row,
            &cache.col,
            &scale(&cache.lze),
            cache.plen,
            cache.plen,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One node, two groups. `es = 1`, `philen = 2`.
    fn setup() -> (Params, Geometry, Sigma, Vec<f64>) {
        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(1),
            g: 2,
            ..Default::default()
        };
        let geometry = Geometry {
            lx: vec![2.0],
            ly: vec![3.0],
            lz: vec![4.0],
            ..Default::default()
        };
        let sigma = Sigma {
            tot: SparseMatrix::assemble(&[0, 1], &[0, 1], &[1.0, 2.0], 2, 2),
            // Within-group scattering plus a downscatter term at (1, 0).
            s: SparseMatrix::assemble(&[0, 1, 1], &[0, 0, 1], &[0.1, 0.3, 0.2], 2, 2),
            // Fission production, including the group-2 -> group-1 term.
            f: SparseMatrix::assemble(&[0, 0], &[0, 1], &[0.5, 0.6], 2, 2),
            ..Default::default()
        };
        let diffvalues = vec![1.0, 2.0];
        (params, geometry, sigma, diffvalues)
    }

    /// Hand-computed entries of `Buck.x` at `keff = 1`.
    ///
    /// # Methodology
    ///
    /// With `Lx = 2`, the scale factor is `0.25 * 2 * 2 / D = 1/D`. At
    /// `keff = 1`, `Bt = (tot - s) - f`, giving per entry:
    /// `(0,0) = 0.9 - 0.5 = 0.4`, `(0,1) = 0 - 0.6 = -0.6`,
    /// `(1,0) = -0.3 - 0 = -0.3`, `(1,1) = 1.8 - 0 = 1.8`. Rows 0 and 1 carry
    /// `D = 1` and `D = 2` respectively.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Matches to the bit at all four entries.
    #[test]
    fn buckling_entries_match_a_hand_computation() {
        let (params, geometry, mut sigma, diffvalues) = setup();
        let mut cache = BucklingCache::new();

        let mut buck =
            calc_bucklingxyz(&mut cache, &params, &geometry, &mut sigma, &diffvalues, 1.0);

        let found = buck.x.find();
        assert_eq!(found.len(), 4);
        let at = |i: usize, j: usize| found.iter().find(|t| t.i == i && t.j == j).unwrap().v;

        assert_eq!(at(0, 0), 0.4);
        assert_eq!(at(0, 1), -0.6);
        assert_eq!(at(1, 0), -0.15);
        assert_eq!(at(1, 1), 0.9);
    }

    /// The reference's comment claims the cached path is bit-identical to a
    /// fresh assembly. This checks that directly: a warm cache and a cold one
    /// must agree exactly, not approximately.
    #[test]
    fn a_warm_cache_is_bit_identical_to_a_cold_one() {
        let (params, geometry, mut sigma, diffvalues) = setup();

        let mut warm = BucklingCache::new();
        let _ = calc_bucklingxyz(&mut warm, &params, &geometry, &mut sigma, &diffvalues, 1.0);
        assert!(warm.is_populated());
        let mut from_warm =
            calc_bucklingxyz(&mut warm, &params, &geometry, &mut sigma, &diffvalues, 1.3);

        let mut cold = BucklingCache::new();
        let mut from_cold =
            calc_bucklingxyz(&mut cold, &params, &geometry, &mut sigma, &diffvalues, 1.3);

        let a = from_warm.z.find();
        let b = from_cold.z.find();
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.i, y.i);
            assert_eq!(x.j, y.j);
            assert_eq!(x.v, y.v, "cached and fresh assembly differ");
        }
    }

    /// Only the fission term is divided by `keff`, so raising it moves the
    /// entries that carry fission and leaves the others alone.
    #[test]
    fn keff_scales_only_the_fission_contribution() {
        let (params, geometry, mut sigma, diffvalues) = setup();
        let mut cache = BucklingCache::new();

        let mut one =
            calc_bucklingxyz(&mut cache, &params, &geometry, &mut sigma, &diffvalues, 1.0);
        let mut two =
            calc_bucklingxyz(&mut cache, &params, &geometry, &mut sigma, &diffvalues, 2.0);

        let f1 = one.x.find();
        let f2 = two.x.find();
        let at = |v: &Vec<crate::matlab::Triplet>, i: usize, j: usize| {
            v.iter().find(|t| t.i == i && t.j == j).unwrap().v
        };

        // (1,1) has no fission term, so it is unchanged.
        assert_eq!(at(&f1, 1, 1), at(&f2, 1, 1));
        // (0,0) carries f = 0.5: 0.9 - 0.5/2 = 0.65, scaled by 1/D = 1.
        assert_eq!(at(&f2, 0, 0), 0.65);
    }

    /// Changing the cross sections must move the fingerprint and force a
    /// rebuild rather than silently reusing stale coefficients.
    #[test]
    fn changing_sigma_triggers_a_rebuild() {
        let (params, geometry, mut sigma, diffvalues) = setup();
        let mut cache = BucklingCache::new();

        let mut before =
            calc_bucklingxyz(&mut cache, &params, &geometry, &mut sigma, &diffvalues, 1.0);
        let v_before = before.x.find()[0].v;

        // Bump the total cross section; the fingerprint's sum changes with it.
        sigma.tot = SparseMatrix::assemble(&[0, 1], &[0, 1], &[1.5, 2.0], 2, 2);
        let mut after =
            calc_bucklingxyz(&mut cache, &params, &geometry, &mut sigma, &diffvalues, 1.0);
        let v_after = after.x.find()[0].v;

        assert_ne!(v_before, v_after, "stale cache reused after sigma changed");
        // 1.5 - 0.1 - 0.5 = 0.9, scaled by 0.25*2*2/1 = 1.
        //
        // Compared with a tolerance, not exactly: none of 1.5, 0.1 or 0.5
        // subtract cleanly in binary, and the expression evaluates to
        // 0.8999999999999999. The other tests in this module assert bit
        // equality because their operands happen to be exact in binary — that
        // is a property of the chosen numbers, not a rule to apply blindly.
        assert!((v_after - 0.9).abs() < 1e-12, "got {v_after}");
    }

    /// A void node contributes nothing, leaving an empty operator rather than
    /// dividing by its zero diffusion coefficient.
    #[test]
    fn void_nodes_are_skipped() {
        let (params, geometry, mut sigma, _) = setup();
        let mut cache = BucklingCache::new();

        let mut buck =
            calc_bucklingxyz(&mut cache, &params, &geometry, &mut sigma, &[0.0, 0.0], 1.0);
        assert_eq!(buck.x.nnz(), 0);
    }
}
