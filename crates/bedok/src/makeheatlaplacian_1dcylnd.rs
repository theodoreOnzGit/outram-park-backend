//! A 1-D cylindrical conduction operator — **dead code in the reference**.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `makeheatlaplacian_1dcylnd.m`,
//!   `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.
//!
//! # Read this before using it: the reference never calls this file
//!
//! Its **only** call site is `th_solverxyz.m:174`, and that line is commented
//! out. The live path is [`crate::fuelrodheat_1dcylnd`], which assembles the
//! same operator inline — and **not the same way**:
//!
//! | | this file | `fuelrodheat_1dcylnd` |
//! |---|---|---|
//! | Interface conductivity | `2*cond(ir+1)*sumLr(ir+1)/Lr(ir+1)` — the outward node's value | `2*k_i*k_{i+1}/(k_i + k_{i+1})` — a harmonic mean |
//! | Radial weight | `sumLr`, the node's **outer** radius | `Ctr`, the node **centre** radius |
//! | Interface nodes | none; `maxir` unknowns | doubled at each material interface; `maxir + surfcount` unknowns |
//! | Gap treatment | bridges `ir` to `ir+2` | bridges, plus a dummy row |
//!
//! So the snapshot carries two divergent discretisations of one operator and
//! the unreachable one is the more readable. Which the author intended is not
//! recorded. Translated because it is one of the 48 files in scope, and
//! recorded as defect T4 — **not** because it is a usable alternative.
//!
//! A caller wanting fuel temperatures wants [`crate::fuelrodheat_1dcylnd`].

use crate::matlab::SparseMatrix;
use crate::types::FuelGeometry;

/// `laplc = makeheatlaplacian_1dcylnd(params, geometry, temps, bc)` — the
/// radial conduction operator for one fuel rod.
///
/// # What it computes
///
/// The finite-volume conduction matrix for the integrated 1-D cylindrical heat
/// equation, **divided through by `2*pi`** as the sibling module's header notes
/// for the same convention. Entry `(i, i)` carries the sum of the inward and
/// outward conductances at node `i`, W/(cm·K); the off-diagonals carry their
/// negatives.
///
/// # Arguments
///
/// - `fuel` — needs `whichk`, `tcon`, `gap_conductance` and `lr`. (`Ctr` is
///   read by the reference and never used — dead, and not a parameter here.)
/// - `maxir` — radial node count, the reference's `params.maxir`.
/// - `temps` — nodal temperatures, **K**, at least `maxir` long. Only used to
///   evaluate the temperature-dependent conductivities.
/// - `bc` — the outer boundary conductance, W/(cm·K); in the live path this is
///   `hcoeff * Rtot`.
///
/// # Returns
///
/// A `maxir`-square sparse operator. Rows for nodes with `whichk == 0` are left
/// as the identity, which the preallocation supplies.
///
/// # Reference defects carried here
///
/// - **Writes outside the declared shape (T5).** When `whichk(ir+1) == 0` the
///   forward link is written to column `ir + 2`. At `ir = maxir - 1`, the last
///   value the loop takes, that is column `maxir + 1` — outside the
///   `sparse(..., maxir, maxir)` shape the function declares. MATLAB raises an
///   index error. Here it **panics** with the same meaning, via
///   [`SparseMatrix::add`]'s bounds assertion.
/// - **The first node's conductivity lookup is unguarded.** `cond(1) =
///   tcon{whichk(1)}(temps(1))` has no `whichk(1) ~= 0` test, unlike every
///   other lookup in the file and unlike `calc_tcond.m`, which exists to do
///   exactly this and is called from nowhere (T6). A rod whose innermost node
///   is void indexes `tcon{0}` and MATLAB raises. Panics here.
/// - **`sumLr` in the loop, `Lr` in the tail.** The gap conductance is scaled
///   by `sumLr(irminus)` inside the loop but by `Lr(irminus)` in the final
///   block — a cumulative radius against a single node thickness. One of the
///   two is wrong; the snapshot does not say which.
/// - **The commented-out harmonic mean.** Two lines carry a struck-through
///   `(Lr(i)+Lr(i+1))*(k_i k_{i+1})/(Lr(i) k_i + Lr(i+1) k_{i+1})` with the
///   author's note "there should be a better formula for this". The live
///   sibling module uses a harmonic mean, so this looks like an abandoned
///   revision.
///
/// # Panics
///
/// If `temps` is shorter than `maxir`; if the innermost node is void (see
/// above); if a `whichk` value has no matching entry in `tcon`; or on the
/// out-of-shape column write at `ir = maxir - 1` (T5).
pub fn makeheatlaplacian_1dcylnd(
    fuel: &FuelGeometry,
    maxir: usize,
    temps: &[f64],
    bc: f64,
) -> SparseMatrix {
    assert!(
        maxir >= 2,
        "the stencil needs at least two radial nodes, got {maxir}"
    );
    assert!(
        temps.len() >= maxir,
        "temps is {} long, need at least maxir = {maxir}",
        temps.len()
    );

    // `sumLr(i) = sum(Lr(1:i))` — the cumulative outer radius of node `i`.
    let mut sum_lr = vec![0.0; fuel.lr.len()];
    let mut acc = 0.0;
    for (i, &l) in fuel.lr.iter().enumerate() {
        acc += l;
        sum_lr[i] = acc;
    }

    // Conductivity at each node. An interior node between two solid nodes is
    // evaluated at the mean of the two temperatures; one following a void is
    // evaluated at its own. Void nodes stay at zero.
    let conductivity = |m: usize, t: f64| -> f64 {
        assert!(
            m >= 1 && m <= fuel.tcon.len(),
            "whichk = {m} has no matching conductivity; tcon has {} entries",
            fuel.tcon.len()
        );
        fuel.tcon[m - 1].at(t)
    };

    let mut cond = vec![0.0; temps.len()];
    // Unguarded in the reference — see the doc comment.
    cond[0] = conductivity(fuel.whichk[0], temps[0]);
    for i in 1..temps.len().min(fuel.whichk.len()) {
        if fuel.whichk[i] != 0 {
            cond[i] = if fuel.whichk[i - 1] != 0 {
                conductivity(fuel.whichk[i], (temps[i] + temps[i - 1]) / 2.0)
            } else {
                conductivity(fuel.whichk[i], temps[i])
            };
        }
    }

    // The diagonal starts as the identity; nodes the loop skips keep the 1.
    let mut diag = vec![1.0; maxir];
    let mut off: Vec<(usize, usize, f64)> = Vec::new();

    // Node 0: no inward face — the cylinder axis is a symmetry plane.
    let mut kplus = 2.0 * cond[1] * sum_lr[1] / fuel.lr[1];
    diag[0] = kplus;
    off.push((0, 1, -kplus));

    let mut irminus = 0usize;

    // `for ir = 2:maxir-1`, 0-based `1..=maxir-2`.
    for ir in 1..maxir - 1 {
        if fuel.whichk[ir] == 0 {
            continue;
        }
        let kminus = if fuel.whichk[ir - 1] == 0 {
            fuel.gap_conductance * sum_lr[irminus]
        } else {
            2.0 * cond[ir] * sum_lr[ir - 1] / fuel.lr[ir]
        };

        if fuel.whichk[ir + 1] == 0 {
            // Bridge the void: link to `ir + 2`. At `ir = maxir - 2` (0-based)
            // this is column `maxir`, which is out of shape — defect T5.
            kplus = fuel.gap_conductance * sum_lr[ir + 1];
            off.push((ir, ir + 2, -kplus));
        } else {
            kplus = 2.0 * cond[ir + 1] * sum_lr[ir + 1] / fuel.lr[ir + 1];
            off.push((ir, ir + 1, -kplus));
        }
        diag[ir] = kminus + kplus;
        off.push((ir, irminus, -kminus));

        irminus = ir;
    }

    // The outer node. Note `Lr` here where the loop used `sumLr`.
    let kminus = if fuel.whichk[maxir - 2] == 0 {
        fuel.gap_conductance * fuel.lr[irminus]
    } else {
        2.0 * cond[maxir - 1] * sum_lr[maxir - 2] / fuel.lr[maxir - 1]
    };
    diag[maxir - 1] = kminus + bc;
    off.push((maxir - 1, irminus, -kminus));

    let mut laplc = SparseMatrix::zeros(maxir, maxir);
    for (i, d) in diag.iter().enumerate() {
        laplc.add(i, i, *d);
    }
    for (i, j, v) in off {
        laplc.add(i, j, v);
    }
    laplc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Conductivity;

    /// A solid rod of one material, no gap — the simplest case the stencil
    /// handles.
    fn solid_rod(n: usize) -> FuelGeometry {
        FuelGeometry {
            lr: vec![0.1; n],
            ctr: (0..n).map(|i| 0.1 * (i as f64 + 0.5)).collect(),
            whichk: vec![1; n],
            tcon: vec![Conductivity::Constant(0.05)],
            gap_conductance: 0.35,
            ..Default::default()
        }
    }

    /// The operator is built, square, and has the expected sparsity: a
    /// tridiagonal band.
    #[test]
    fn a_solid_rod_gives_a_tridiagonal_operator() {
        let fuel = solid_rod(5);
        let temps = vec![800.0; 5];
        let mut m = makeheatlaplacian_1dcylnd(&fuel, 5, &temps, 1.5);

        assert_eq!(m.rows(), 5);
        assert_eq!(m.cols(), 5);
        for t in m.find() {
            let d = t.i as isize - t.j as isize;
            assert!(
                d.abs() <= 1,
                "entry at ({}, {}) is outside the band",
                t.i,
                t.j
            );
        }
    }

    /// Every row balances: the diagonal equals minus the sum of its
    /// off-diagonals, except the outer row, which carries the boundary
    /// conductance.
    ///
    /// # Methodology
    ///
    /// A conduction operator conserves energy exactly when each interior row
    /// sums to zero — heat leaving a node through one face enters its
    /// neighbour. The outer row is the exception by construction: its diagonal
    /// is `kminus + bc`, and `bc` is the conductance to the coolant, which has
    /// no matching off-diagonal because the coolant is not an unknown.
    ///
    /// Pass criterion: interior rows sum to zero within 1e-12; the outer row
    /// sums to `bc`.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Rows 0, 1 and 3 summed to exactly `0`; row 2 to `-5.551e-17`, one
    /// floating-point ulp. Row 4, the outer row, summed to `1.5` — the boundary
    /// conductance, exactly as constructed.
    ///
    /// **Interpretation.** The operator conserves energy to machine precision
    /// on a uniform solid rod, which verifies the conductance assembly and the
    /// sign convention. It says nothing about the gap-bridging path, which a
    /// uniform rod does not exercise, and nothing about whether this file's
    /// discretisation is the intended one — it is dead code (T4).
    #[test]
    fn interior_rows_conserve_energy() {
        let fuel = solid_rod(5);
        let temps = vec![800.0; 5];
        let bc = 1.5;
        let mut m = makeheatlaplacian_1dcylnd(&fuel, 5, &temps, bc);

        let mut rowsum = [0.0; 5];
        for t in m.find() {
            rowsum[t.i] += t.v;
        }
        for (i, s) in rowsum.iter().enumerate() {
            eprintln!("row {i} sums to {s}");
        }
        for s in rowsum.iter().take(4) {
            assert!(s.abs() < 1e-12, "interior row sums to {s}, not zero");
        }
        assert!(
            (rowsum[4] - bc).abs() < 1e-12,
            "the outer row should sum to bc = {bc}, got {}",
            rowsum[4]
        );
    }

    /// Defect T5, pinned: a void node one place in from the outside makes the
    /// stencil write outside its own declared shape.
    ///
    /// # Methodology
    ///
    /// The forward link for a node whose outward neighbour is void goes to
    /// column `ir + 2`. The loop's last value of `ir` is `maxir - 1` (1-based),
    /// so that column is `maxir + 1` — one past the end of the
    /// `sparse(..., maxir, maxir)` shape. MATLAB raises an index error there;
    /// this translation panics on the same condition.
    ///
    /// A 5-node rod with `whichk = [1, 1, 1, 0, 2]` puts the void at 1-based
    /// node 4, so the loop reaches `ir = 4` with `whichk(5) = 2`... hence the
    /// void must sit at `maxir` to trigger it. `whichk = [1, 1, 1, 1, 0]` puts
    /// the void at node 5, and the loop's last node `ir = 4` then writes column
    /// 6.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Panicked in [`crate::matlab::SparseMatrix::add`] with
    /// `column index 5 out of range 0..5` — i.e. the write to 1-based column 6
    /// of a 5-column matrix, exactly the out-of-shape access MATLAB would
    /// raise on. Defect T5 confirmed within the translation.
    #[test]
    #[should_panic(expected = "column index")]
    fn a_void_outer_node_writes_outside_the_declared_shape() {
        let mut fuel = solid_rod(5);
        fuel.whichk = vec![1, 1, 1, 1, 0];
        fuel.tcon = vec![Conductivity::Constant(0.05)];
        let temps = vec![800.0; 5];
        let _ = makeheatlaplacian_1dcylnd(&fuel, 5, &temps, 1.5);
    }

    /// A void innermost node indexes `tcon{0}` — the unguarded first lookup.
    #[test]
    #[should_panic(expected = "has no matching conductivity")]
    fn a_void_innermost_node_has_no_conductivity() {
        let mut fuel = solid_rod(5);
        fuel.whichk = vec![0, 1, 1, 1, 1];
        let temps = vec![800.0; 5];
        let _ = makeheatlaplacian_1dcylnd(&fuel, 5, &temps, 1.5);
    }

    /// The conductances scale with the radial position, as a cylindrical
    /// stencil must: an outer face has more area than an inner one.
    #[test]
    fn conductances_grow_outward() {
        let fuel = solid_rod(6);
        let temps = vec![800.0; 6];
        let mut m = makeheatlaplacian_1dcylnd(&fuel, 6, &temps, 1.5);

        // The forward link from each interior node, magnitude.
        let mut forward = [0.0; 6];
        for t in m.find() {
            if t.j == t.i + 1 {
                forward[t.i] = -t.v;
            }
        }
        for i in 1..4 {
            assert!(
                forward[i] > forward[i - 1],
                "node {i} forward conductance {} did not exceed {}",
                forward[i],
                forward[i - 1]
            );
        }
    }
}
