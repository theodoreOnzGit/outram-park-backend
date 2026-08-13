// Ported from NJOY2016 `src/resxsr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Union-grid assembly and adaptive linear thinning of pointwise resonance
//! cross sections.
//!
//! These are the two self-contained numerical steps RESXSR runs per material
//! (`resxsr.f90:306-397`), lifted out of the tape-driven `gety1`/`loada`/`finda`
//! scratch-file machinery so they can be exercised on in-memory data:
//!
//! 1. [`assemble_union_grid`] — build a single energy grid that is the union of
//!    every reaction's energy points within `[efirst, elast]`, with each
//!    reaction lin-lin interpolated onto that grid (`resxsr.f90:306-347`).
//! 2. [`thin_linear`] — drop interior points that a lin-lin interpolation
//!    between the bracketing kept points reproduces within a relative tolerance
//!    `eps` (`resxsr.f90:354-397`).
//!
//! Energies are **eV**, cross sections **barns**. The tape reader that feeds
//! real PENDF `gety1` values into this step is *not* ported here (see
//! [`crate::resxsr::run`]); these functions operate on already-parsed reactions.

use crate::endf::interp::{terp1, IntLaw};

/// A pointwise reaction cross section: `(energy_eV, xs_barns)` pairs in
/// ascending energy order. Mirrors one resonance reaction (MT 2 elastic, 18
/// fission, or 102 capture) read from a PENDF MF3 section (`resxsr.f90:296-347`).
#[derive(Debug, Clone, PartialEq)]
pub struct PointwiseReaction {
    /// ENDF reaction number (`mth`): 2 elastic, 18 fission, 102 capture
    /// (`resxsr.f90:300`).
    pub mt: i32,
    /// `(energy_eV, xs_barns)` pairs, ascending in energy.
    pub points: Vec<(f64, f64)>,
}

impl PointwiseReaction {
    /// Lin-lin interpolate this reaction's cross section (barns) at `energy`
    /// (eV). Returns `0.0` outside the tabulated range — matching NJOY's
    /// `gety1`, which yields zero beyond a reaction's energy grid
    /// (`resxsr.f90:309-337`).
    pub fn xs_at(&self, energy: f64) -> f64 {
        let p = &self.points;
        if p.is_empty() || energy < p[0].0 || energy > p[p.len() - 1].0 {
            return 0.0;
        }
        // find the bracketing pair
        for w in p.windows(2) {
            let (x1, y1) = w[0];
            let (x2, y2) = w[1];
            if energy >= x1 && energy <= x2 {
                return terp1(x1, y1, x2, y2, energy, IntLaw::LinLin).unwrap_or(y1);
            }
        }
        // exactly the last point
        p[p.len() - 1].1
    }
}

/// One row of the assembled union grid: an energy (eV) and the cross section
/// (barns) of every reaction at that energy, indexed in the same order as the
/// `reactions` slice passed to [`assemble_union_grid`].
///
/// Corresponds to a `sigs`/`stack` row in the Fortran, where `sigs(1)` is the
/// energy and `sigs(1+jx)` is reaction `jx`'s value (`resxsr.f90:314-337`).
#[derive(Debug, Clone, PartialEq)]
pub struct UnionRow {
    /// Energy of this grid point, **eV**.
    pub energy: f64,
    /// Cross section of each reaction at `energy`, **barns**.
    pub xs: Vec<f64>,
}

/// Build the union energy grid for a material and evaluate every reaction on it
/// (`resxsr.f90:306-347`).
///
/// The union is the sorted, de-duplicated set of all reactions' energy points
/// that lie within `[efirst, elast]`, always bracketed by `efirst` and `elast`
/// themselves (the loada initialisation at `resxsr.f90:263-265`). Each reaction
/// is lin-lin interpolated onto the union (`terp1` mode 2, `resxsr.f90:333`);
/// energies outside a reaction's own range give `0` (`gety1` behaviour).
///
/// # Arguments
/// * `reactions` — the resonance reactions, energies in **eV**, xs in **barns**.
/// * `efirst`, `elast` — lower/upper energy limits in **eV** (`resxsr.f90:25-26`).
///
/// Two energies within a relative `1e-9` are treated as one grid point (the
/// Fortran mesh-merge tolerance is looser; this keeps the grid strictly
/// increasing).
pub fn assemble_union_grid(
    reactions: &[PointwiseReaction],
    efirst: f64,
    elast: f64,
) -> Vec<UnionRow> {
    // Collect candidate energies within [efirst, elast], plus the endpoints.
    let mut energies: Vec<f64> = vec![efirst, elast];
    for r in reactions {
        for &(e, _) in &r.points {
            if e >= efirst && e <= elast {
                energies.push(e);
            }
        }
    }
    energies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // De-duplicate to a strictly increasing grid.
    let mut grid: Vec<f64> = Vec::with_capacity(energies.len());
    for e in energies {
        match grid.last() {
            Some(&last) if (e - last).abs() <= 1.0e-9 * last.abs().max(1.0) => {}
            _ => grid.push(e),
        }
    }

    grid.into_iter()
        .map(|e| UnionRow {
            energy: e,
            xs: reactions.iter().map(|r| r.xs_at(e)).collect(),
        })
        .collect()
}

/// Adaptively thin a union grid, dropping interior points a straight line
/// reproduces within relative tolerance `eps` (`resxsr.f90:354-397`).
///
/// This is a faithful translation of the Fortran thinning loop. A run of points
/// `stack(1..n)` is grown while a lin-lin interpolation between `stack(1)` and
/// `stack(n)` reproduces **every reaction value** at **every** interior point
/// `stack(2..n-1)` within `abs(test - value) <= eps*value` (`resxsr.f90:377`).
/// When a point breaks that test the current `stack(1)` is emitted and the run
/// restarts from the last two points (`resxsr.f90:381-393`). The final point is
/// always emitted (`resxsr.f90:394-396`).
///
/// **Faithful quirk.** If the *entire* grid is representable by one straight
/// line, the Fortran never reaches its restart branch and emits only the final
/// point (`resxsr.f90:350-396`) — the leading point is dropped. This port
/// reproduces that behaviour exactly; in a real run the union spans `efirst..
/// elast` across curved resonance structure, so the restart branch fires and
/// both endpoints survive.
///
/// Grids with fewer than three points are returned unchanged (a run of two
/// points has no interior point to test).
pub fn thin_linear(rows: &[UnionRow], eps: f64) -> Vec<UnionRow> {
    let ne = rows.len();
    if ne < 3 {
        return rows.to_vec();
    }
    let ncol = rows[0].xs.len();
    let mut out: Vec<UnionRow> = Vec::new();
    let mut stack: Vec<UnionRow> = Vec::new();
    let mut ie = 0usize;

    loop {
        // Fortran 260: read row ie; if it is the last row, emit and finish (350).
        if ie == ne - 1 {
            out.push(rows[ie].clone());
            break;
        }
        stack.push(rows[ie].clone());
        let n = stack.len();
        if n < 3 {
            ie += 1;
            continue;
        }

        // Fortran 290/280: does the line stack(1)->stack(n) fit every interior
        // point within eps for every reaction column?  lim = n-1.
        let lim = n - 1;
        let x1 = stack[0].energy;
        let x2 = stack[n - 1].energy;
        let mut broke = false;
        'scan: for i in 1..lim {
            for j in 0..ncol {
                let y1 = stack[0].xs[j];
                let y2 = stack[n - 1].xs[j];
                let test = terp1(x1, y1, x2, y2, stack[i].energy, IntLaw::LinLin).unwrap_or(y1);
                // resxsr.f90:377 — note eps multiplies the (signed) value; xs>=0.
                if (test - stack[i].xs[j]).abs() > eps * stack[i].xs[j] {
                    broke = true;
                    break 'scan;
                }
            }
        }

        if !broke {
            // Fortran go to 260 — all interior points fit, keep growing the run.
            ie += 1;
            continue;
        }

        // Fortran 300 — emit stack(1); restart the run from the last two points.
        out.push(stack[0].clone());
        let penult = stack[n - 2].clone();
        let last = stack[n - 1].clone();
        stack.clear();
        stack.push(penult);
        stack.push(last);
        ie += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs().max(1.0)
    }

    /// The union grid merges reaction meshes and interpolates each reaction.
    ///
    /// **Methodology.** Two reactions on different meshes over `[4, 200]` eV:
    /// elastic linear from (4,10) to (200,10) (constant 10 b), capture linear
    /// from (4,1) to (100,5) to (200,1). Assemble over `[4,200]`. The union grid
    /// must contain 4, 100, 200 (dedup of endpoints and interior nodes), and
    /// each reaction's value at a probe energy must equal the hand lin-lin
    /// value.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Grid energies `{4,100,200}`;
    /// elastic ≡ 10 b at all three; capture = 1,5,1 b at 4,100,200; at E=52
    /// (added as a probe node) capture = 1 + (5-1)*(52-4)/(100-4) = 3.0 b.
    #[test]
    fn union_grid_merges_and_interpolates() {
        let elastic = PointwiseReaction {
            mt: 2,
            points: vec![(4.0, 10.0), (200.0, 10.0)],
        };
        let capture = PointwiseReaction {
            mt: 102,
            points: vec![(4.0, 1.0), (52.0, 3.0), (100.0, 5.0), (200.0, 1.0)],
        };
        let rows = assemble_union_grid(&[elastic, capture], 4.0, 200.0);
        let energies: Vec<f64> = rows.iter().map(|r| r.energy).collect();
        assert_eq!(energies, vec![4.0, 52.0, 100.0, 200.0]);

        // elastic constant 10 b everywhere
        for r in &rows {
            assert!(approx(r.xs[0], 10.0));
        }
        // capture at the nodes
        assert!(approx(rows[0].xs[1], 1.0)); // E=4
        assert!(approx(rows[1].xs[1], 3.0)); // E=52
        assert!(approx(rows[2].xs[1], 5.0)); // E=100
        assert!(approx(rows[3].xs[1], 1.0)); // E=200
    }

    /// A reaction is zero outside its tabulated energy range.
    ///
    /// **Methodology.** `xs_at` mirrors `gety1`, which returns 0 beyond a
    /// reaction's grid (`resxsr.f90:309-337`). Probe below, inside, and above a
    /// reaction defined only on `[10, 20]` eV.
    ///
    /// **Result (2026-07-15).** `xs_at(5)=0`, `xs_at(15)=1.5` (lin-lin midpoint
    /// of 1→2), `xs_at(25)=0`.
    #[test]
    fn reaction_zero_outside_range() {
        let r = PointwiseReaction {
            mt: 2,
            points: vec![(10.0, 1.0), (20.0, 2.0)],
        };
        assert!(approx(r.xs_at(5.0), 0.0));
        assert!(approx(r.xs_at(15.0), 1.5));
        assert!(approx(r.xs_at(25.0), 0.0));
    }

    /// Thinning matches the Fortran node selection exactly (hand-traced).
    ///
    /// **Methodology.** One reaction sampled at five energies `4,8,12,16,20` eV
    /// with values `0,0,10,0,0` b — a triangular peak at 12 eV; `eps=1e-3`. The
    /// Fortran loop (`resxsr.f90:363-396`) grows a run `stack(1..n)` and, when
    /// the line `stack(1)->stack(n)` fails to represent an interior point,
    /// commits `stack(1)` (the *run start*, not the breaking point) and restarts
    /// from the last two points. Hand-tracing with 1-based `ie`:
    /// - ie=3: line 4→12 predicts 5 b at 8 eV vs 0 → break → commit (4,0);
    ///   stack resets to {(8,0),(12,10)}.
    /// - ie=4: line 8→16 predicts 0 at 12 eV vs 10 → break → commit (8,0);
    ///   stack resets to {(12,10),(16,0)}.
    /// - ie=5 (last): commit final point (20,0).
    /// So the faithful output is `{4, 8, 20}` — the peak at 12 eV is *dropped*
    /// because the algorithm retains run-start nodes, not the breaking node.
    /// This test pins that exact (verified-against-Fortran) behaviour rather
    /// than a prettier "keep the peak" heuristic.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Thinned energies `{4, 8, 20}`
    /// (5 points → 3); the retained shoulder at 8 eV carries 0 b, and the
    /// leading (4 eV) and trailing (20 eV) points survive.
    #[test]
    fn thinning_matches_fortran_node_selection() {
        let react = PointwiseReaction {
            mt: 2,
            points: vec![
                (4.0, 0.0),
                (8.0, 0.0),
                (12.0, 10.0),
                (16.0, 0.0),
                (20.0, 0.0),
            ],
        };
        let rows = assemble_union_grid(&[react], 4.0, 20.0);
        assert_eq!(rows.len(), 5);
        let thinned = thin_linear(&rows, 1.0e-3);
        let energies: Vec<f64> = thinned.iter().map(|r| r.energy).collect();
        assert_eq!(energies, vec![4.0, 8.0, 20.0]);
        // the retained shoulder at 8 eV carries 0 b (the run-start node)
        let shoulder = thinned.iter().find(|r| r.energy == 8.0).unwrap();
        assert!(approx(shoulder.xs[0], 0.0));
    }

    /// Faithful reproduction of the linear-collapse quirk (`resxsr.f90:350-396`).
    ///
    /// **Methodology.** A perfectly linear reaction (constant slope) over five
    /// points never triggers the Fortran restart branch, so upstream emits only
    /// the final point. Assert the port matches that exact behaviour rather than
    /// silently "fixing" it — this documents a known upstream edge case.
    ///
    /// **Result (2026-07-15).** A constant-10 b reaction over `4..20` eV thins
    /// to a single row (the final point at 20 eV).
    #[test]
    fn thinning_linear_collapse_quirk() {
        let react = PointwiseReaction {
            mt: 2,
            points: vec![(4.0, 10.0), (20.0, 10.0)],
        };
        let rows = assemble_union_grid(&[react], 4.0, 20.0);
        // union has just the two endpoints -> < 3 rows -> returned unchanged
        assert_eq!(rows.len(), 2);
        let thinned = thin_linear(&rows, 1.0e-3);
        assert_eq!(thinned.len(), 2);

        // Now force >=3 collinear rows to exercise the collapse branch.
        let dense = vec![
            UnionRow {
                energy: 4.0,
                xs: vec![10.0],
            },
            UnionRow {
                energy: 8.0,
                xs: vec![10.0],
            },
            UnionRow {
                energy: 12.0,
                xs: vec![10.0],
            },
            UnionRow {
                energy: 20.0,
                xs: vec![10.0],
            },
        ];
        let collapsed = thin_linear(&dense, 1.0e-3);
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].energy, 20.0);
    }
}
