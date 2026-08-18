//! Transient 1-D cylindrical fuel-rod conduction — one implicit-Euler step.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `fuelrodheattime_1dcylnd.m`,
//!   `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What this adds to the steady version
//!
//! One implicit-Euler step of
//!
//! ```text
//! rho cp dT/dt = (1/r) d/dr ( k r dT/dr ) + q'''
//! ```
//!
//! The discretisation, the interface-node doubling, the gap bridge and the
//! boundary treatment are **identical** to [`crate::fuelrodheat_1dcylnd`] —
//! read that module's docs for how the `ir`/`id` walk works, because none of it
//! is repeated here. The only additions are a capacity term on each diagonal
//! and its matching source contribution:
//!
//! ```text
//! cap_id = rho_cp(T_old_id) * (r_cur^2 - r_prev^2) / 2 / dt
//! ```
//!
//! with `cap_id * T_old_id` added to the right-hand side. `[r_prev, r_cur]` is
//! the radial interval that solution node represents.
//!
//! # The steady solver is this one at `dt = infinity`
//!
//! Setting `cap = 0` recovers `fuelrodheat_1dcylnd` **exactly** — same
//! diagonal, same source, same off-diagonals. That is not a loose analogy; it
//! is checked by a test here, and it is worth knowing because the two files
//! were transcribed independently. A disagreement between them would mean one
//! of the two transcriptions is wrong, and the test says which.
//!
//! They are nevertheless kept as separate modules rather than one
//! parameterised solver: the reference ships two files, the capacity terms are
//! interleaved through the loop rather than separable the way
//! `singleflow1devap`'s stage 2 was, and collapsing them would mean editing
//! already-verified code to accommodate the new one.
//!
//! # Semi-implicit, not fully implicit
//!
//! Thermal properties are evaluated at the **previous** temperatures:
//! conductivity at `temps` (the current Picard iterate) and heat capacity at
//! `tempsold` (the previous time step). So the matrix is linear in the unknown
//! temperature and the non-linearity is lagged. A caller wanting the fully
//! implicit answer iterates, feeding each result back in as `temps`.

use crate::matlab::{Decomposition, SparseMatrix};
use crate::types::FuelGeometry;

pub use crate::fuelrodheat_1dcylnd::Solve;

/// `results = fuelrodheattime_1dcylnd(params, geometry, temps, tempsold, pwr, bc, modtemp, dt)`.
///
/// # Arguments
///
/// As [`crate::fuelrodheat_1dcylnd::fuelrodheat_1dcylnd`], plus:
///
/// - `tempsold` — temperatures at the **previous time step**, `maxid` long, in
///   **K**. Used for the capacity terms, both to evaluate `rho*cp` and as the
///   source `cap * T_old`.
/// - `dt` — the time step, **seconds**.
///
/// and `fuel` additionally needs [`crate::types::FuelGeometry::rhocp`].
///
/// Note `temps` and `tempsold` are different vectors and mean different things:
/// `temps` is the current Picard iterate, used only for the conductivities;
/// `tempsold` is the previous time level, and it carries the physics of the
/// time derivative.
///
/// # Returns
///
/// `(temperatures, outcome)` — the profile over the `maxid` unknowns in **K**,
/// and whether it came back finite.
///
/// # The radial intervals, which are not the obvious ones
///
/// Each solution node owns a radial interval `[r_prev, r_cur]`, and the
/// bookkeeping is worth spelling out because it is not simply "the node's own
/// cell":
///
/// | Node | Interval |
/// |---|---|
/// | innermost | `[0, Ctr(1)]` — only the **inner half** of the first mesh cell |
/// | a gap node | none; `r_prev` jumps to `sumLr(ir)` and no capacity is added |
/// | a surface node (`surf`) | `[Ctr(ir), sumLr(ir)]` — the **outer half** of that cell |
/// | an ordinary node | `[r_prev, Ctr(ir)]` |
/// | outermost | `[r_prev, sumLr(maxir)]` |
///
/// So the capacity is distributed over half-cells at the interfaces, which is
/// consistent with the interface doubling: the two unknowns that share a mesh
/// cell split its mass between them.
///
/// # Reference defects carried here
///
/// The same set as [`crate::fuelrodheat_1dcylnd`], since the loop is the same:
/// the gap dummy row pinned at `T = 1` (T7), the missing `else` that leaves a
/// stale conductance when two different solid materials touch (T8), `temps`
/// being read at `id + 1`, the self-harmonic-mean at `ir == maxir`, and the
/// doubled interface conductance. See that module for each.
///
/// One is specific to this file: **the gap dummy row is `1*T = 1` here too**,
/// and because it has no capacity term it stays exactly 1 regardless of `dt`,
/// `tempsold` or anything else.
///
/// # Panics
///
/// If `temps` or `tempsold` is shorter than the `maxid` the mesh implies, if a
/// `whichk` value has no matching conductivity or heat capacity, or if the mesh
/// has fewer than two nodes.
#[allow(clippy::too_many_arguments)]
pub fn fuelrodheattime_1dcylnd(
    fuel: &FuelGeometry,
    maxir: usize,
    temps: &[f64],
    tempsold: &[f64],
    pwr: f64,
    bc: f64,
    modtemp: f64,
    dt: f64,
) -> (Vec<f64>, Solve) {
    assert!(
        maxir >= 2,
        "the stencil needs at least two radial nodes, got {maxir}"
    );

    let whichk = &fuel.whichk;
    let lr = &fuel.lr;
    let ctr = &fuel.ctr;
    let is_fuel = |ir: usize| whichk[ir] == 1;

    let mut sum_lr = vec![0.0; lr.len()];
    let mut acc = 0.0;
    for (i, &l) in lr.iter().enumerate() {
        acc += l;
        sum_lr[i] = acc;
    }

    let mut surfcount = 0usize;
    for ir in 0..maxir - 1 {
        if (whichk[ir] != 0) != (whichk[ir + 1] != 0) {
            surfcount += 1;
        }
    }
    let maxid = maxir + surfcount;

    for (name, len) in [("temps", temps.len()), ("tempsold", tempsold.len())] {
        assert!(
            len >= maxid,
            "{name} is {len} long; the stencil reads up to maxid = {maxid}"
        );
    }

    let conductivity = |m: usize, t: f64| -> f64 {
        assert!(
            m >= 1 && m <= fuel.tcon.len(),
            "whichk = {m} has no matching conductivity; tcon has {} entries",
            fuel.tcon.len()
        );
        fuel.tcon[m - 1].at(t)
    };
    let heat_capacity = |m: usize, t: f64| -> f64 {
        assert!(
            m >= 1 && m <= fuel.rhocp.len(),
            "whichk = {m} has no matching heat capacity; rhocp has {} entries",
            fuel.rhocp.len()
        );
        fuel.rhocp[m - 1].at(t)
    };
    let harmonic = |a: f64, b: f64| 2.0 * (a * b) / (a + b);

    let mut diag = vec![1.0; maxid];
    let mut off: Vec<(usize, usize, f64)> = Vec::new();
    let mut bvec = vec![0.0; maxid];

    // Node 0: the axis, owning `[0, Ctr(1)]`.
    let cond = conductivity(whichk[0], temps[0]);
    let condplus = conductivity(whichk[1], temps[1]);
    let mut kplus = harmonic(cond, condplus) * ctr[0] / lr[0];
    let cap = heat_capacity(whichk[0], tempsold[0]) * ctr[0] * ctr[0] / 2.0 / dt;
    diag[0] = kplus + cap;
    off.push((0, 1, -kplus));
    if is_fuel(0) {
        bvec[0] = 0.5 * pwr * ctr[0] * ctr[0];
    }
    bvec[0] += cap * tempsold[0];

    // Outer radius of the interval the previous node represented.
    let mut rprev = ctr[0];

    let mut idminus = 0usize;
    let mut surf = false;
    let mut ir = 1usize;
    let mut id = 1usize;

    while ir < maxir {
        if whichk[ir] == 0 {
            // The gap: a dummy row, and no heat capacity. The radius marker
            // still advances past it.
            bvec[id] = 1.0;
            rprev = sum_lr[ir];
            ir += 1;
            id += 1;
            continue;
        }

        let kminus = kplus;
        let rcur;

        if surf {
            // No `else` in the reference — defect T8.
            if whichk[ir + 1] == 0 {
                kplus = fuel.gap_conductance * ctr[ir + 1];
                off.push((id, id + 2, -kplus));
                if is_fuel(ir) {
                    bvec[id] = 0.5 * pwr * (sum_lr[ir] * sum_lr[ir] - ctr[ir] * ctr[ir]);
                }
            }
            // A surface node owns the outer half of its mesh cell.
            rcur = sum_lr[ir];
        } else if ir == maxir - 1 {
            let cond = conductivity(whichk[ir], temps[id]);
            let condplus = conductivity(whichk[ir], temps[id + 1]);
            kplus = harmonic(cond, condplus) * ctr[ir] / lr[ir];
            off.push((id, id + 1, -kplus));
            if is_fuel(ir) {
                bvec[id] = 0.5 * pwr * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
            rcur = ctr[ir];
        } else if whichk[ir + 1] == 0 {
            let cond = conductivity(whichk[ir], temps[id]);
            let condplus = conductivity(whichk[ir], temps[id + 1]);
            kplus = harmonic(cond, condplus) * ctr[ir] / lr[ir] * 2.0;
            off.push((id, id + 1, -kplus));
            if is_fuel(ir) {
                bvec[id] = 0.5 * pwr * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
            rcur = ctr[ir];
        } else {
            let cond = conductivity(whichk[ir], temps[id]);
            let condplus = conductivity(whichk[ir + 1], temps[id + 1]);
            kplus = harmonic(cond, condplus) * ctr[ir] / lr[ir];
            off.push((id, id + 1, -kplus));
            if is_fuel(ir) {
                bvec[id] = 0.5 * pwr * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
            rcur = ctr[ir];
        }

        // The capacity of the interval `[rprev, rcur]` this node represents.
        let cap = heat_capacity(whichk[ir], tempsold[id]) * (rcur * rcur - rprev * rprev) / 2.0 / dt;
        diag[id] = kminus + kplus + cap;
        bvec[id] += cap * tempsold[id];
        rprev = rcur;

        off.push((id, idminus, -kminus));
        idminus = id;

        if ir == maxir - 1 || whichk[ir] == whichk[ir + 1] || surf {
            ir += 1;
            surf = false;
        } else {
            surf = true;
        }
        id += 1;
    }

    // The outer surface unknown, owning `[rprev, sumLr(maxir)]`.
    let kminus = kplus;
    let cap = heat_capacity(whichk[maxir - 1], tempsold[maxid - 1])
        * (sum_lr[maxir - 1] * sum_lr[maxir - 1] - rprev * rprev)
        / 2.0
        / dt;
    diag[maxid - 1] = kminus + bc + cap;
    off.push((maxid - 1, idminus, -kminus));
    bvec[maxid - 1] = bc * modtemp + cap * tempsold[maxid - 1];

    let mut laplc = SparseMatrix::zeros(maxid, maxid);
    for (i, d) in diag.iter().enumerate() {
        laplc.add(i, i, *d);
    }
    for (i, j, v) in off {
        laplc.add(i, j, v);
    }

    let results = Decomposition::new(&mut laplc).solve(&bvec);
    let outcome = if results.iter().all(|x| x.is_finite()) {
        Solve::Ok
    } else {
        Solve::NotFinite
    };

    (results, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuelrodheat_1dcylnd::fuelrodheat_1dcylnd;
    use crate::types::{Conductivity, VolumetricHeatCapacity};

    /// The NEACRP-shaped rod: 5 fuel nodes, 1 gap, 2 cladding, now carrying
    /// heat capacities as well as conductivities.
    fn neacrp_rod() -> (FuelGeometry, usize) {
        let (fueln, gapn, cladn) = (5usize, 1usize, 2usize);
        let maxir = fueln + gapn + cladn;

        let mut lr = vec![0.41 / fueln as f64; fueln];
        lr.extend(vec![0.006 / gapn as f64; gapn]);
        lr.extend(vec![0.06 / cladn as f64; cladn]);

        let mut ctr = Vec::with_capacity(maxir);
        let mut acc = 0.0;
        for l in &lr {
            acc += l;
            ctr.push(acc - 0.5 * l);
        }

        let mut whichk = vec![1usize; fueln];
        whichk.extend(vec![0usize; gapn]);
        whichk.extend(vec![2usize; cladn]);

        let fuel = FuelGeometry {
            lr,
            ctr,
            whichk,
            tcon: vec![Conductivity::Uo2Fuel, Conductivity::ZircaloyClad],
            rhocp: vec![
                VolumetricHeatCapacity::Uo2Fuel,
                VolumetricHeatCapacity::ZircaloyClad,
            ],
            gap_conductance: 0.35,
            ..Default::default()
        };
        (fuel, maxir)
    }

    /// A very large time step reproduces the steady solver, node for node.
    ///
    /// # Methodology
    ///
    /// `cap = rho_cp * (r_cur^2 - r_prev^2) / 2 / dt` vanishes as `dt` grows,
    /// and with it every difference between this file and
    /// [`crate::fuelrodheat_1dcylnd`]. So a step at `dt = 1e12 s` must
    /// reproduce the steady profile exactly.
    ///
    /// **This is a cross-check between two independently transcribed files.**
    /// The two `.m` sources duplicate ~150 lines of stencil between them; they
    /// were translated separately, and neither was written by reference to the
    /// other's Rust. A disagreement would mean one of the two transcriptions is
    /// wrong, and this test is what would say so.
    ///
    /// Pass criterion: every node within 1e-9 relative.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// Worst relative difference over all ten unknowns: **2.744e-13**. The
    /// steady profile it matched, in K, is
    /// `[1071.31, 1057.49, 1016.04, 946.97, 850.26, 788.09, 1.0, 613.65,
    /// 604.95, 596.81]` — including the gap dummy at 1.0, which both files
    /// produce identically.
    ///
    /// **Interpretation.** This is the most valuable check in the two
    /// conduction modules, because it is genuinely *independent*: ~150 lines of
    /// stencil were transcribed twice from two `.m` files, and the two
    /// transcriptions agree to 13 significant figures once the capacity terms
    /// are removed. The residual at 1e-13 rather than 1e-16 is the finite
    /// `dt = 1e12` leaving a tiny capacity contribution, exactly as expected.
    #[test]
    fn a_large_time_step_reproduces_the_steady_solver() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![900.0; 10];
        let (steady, s_ok) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 1.5, 580.0);
        let (stepped, t_ok) =
            fuelrodheattime_1dcylnd(&fuel, maxir, &temps, &temps, 300.0, 1.5, 580.0, 1e12);

        assert_eq!(s_ok, Solve::Ok);
        assert_eq!(t_ok, Solve::Ok);

        let mut worst: f64 = 0.0;
        for i in 0..steady.len() {
            worst = worst.max((steady[i] - stepped[i]).abs() / steady[i].abs().max(1.0));
        }
        eprintln!("dt -> inf vs steady: worst relative difference = {worst:.3e}");
        eprintln!("  steady  = {steady:?}");
        eprintln!("  stepped = {stepped:?}");
        assert!(worst < 1e-9, "worst {worst}");
    }

    /// A tiny time step barely moves the rod off its old temperature.
    ///
    /// # Methodology
    ///
    /// The opposite limit: as `dt -> 0` the capacity term dominates every row,
    /// so `cap*T = cap*T_old` forces `T -> T_old`. Starting from a uniform
    /// 600 K rod and applying full power for 1 microsecond must leave it
    /// essentially unchanged.
    ///
    /// The gap dummy at index 6 is excluded — it has no capacity term and sits
    /// at 1 K regardless (defect T7).
    ///
    /// Pass criterion: every non-dummy node within 0.1 K of 600.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// Every non-dummy node moved by less than **0.002 K** from 600 K on a 1 µs
    /// step at full power; the largest movement was at the outer surface, which
    /// owns the least mass. The gap dummy sat at 1.0 as always.
    ///
    /// **Interpretation.** The capacity terms dominate as `dt -> 0`, so the
    /// scheme is stable in that limit rather than producing a spurious jump.
    /// Together with the `dt -> inf` test this brackets the scheme at both
    /// ends.
    #[test]
    fn a_tiny_time_step_freezes_the_rod() {
        let (fuel, maxir) = neacrp_rod();
        let old = vec![600.0; 10];
        let (out, ok) =
            fuelrodheattime_1dcylnd(&fuel, maxir, &old, &old, 300.0, 1.5, 580.0, 1e-6);
        assert_eq!(ok, Solve::Ok);

        eprintln!("1 us step from 600 K: {out:?}");
        for (i, t) in out.iter().enumerate() {
            if i == 6 {
                continue;
            }
            assert!(
                (t - 600.0).abs() < 0.1,
                "node {i} moved to {t} on a 1 us step"
            );
        }
    }

    /// Marching from cold reaches the steady solution, and takes a physically
    /// sensible time to do it.
    ///
    /// # Methodology
    ///
    /// The rod starts uniform at the coolant temperature and is marched at
    /// `dt = 0.01 s` under constant power, feeding each result back as both the
    /// property iterate and the old-time state. It must converge on the steady
    /// answer.
    ///
    /// **The steady reference must be Picard-converged too, and that is the
    /// point of this test.** Both solvers are only *semi*-implicit: the
    /// conductivities are evaluated at whatever `temps` is passed in, so a
    /// single steady call is a solution of a linearised problem, not of the
    /// non-linear one. The first version of this test compared the march
    /// against one steady call made at a fixed 900 K property iterate, and they
    /// settled **10.6 K apart** — 1081.9 K against 1071.3 K. Neither was wrong;
    /// they are fixed points of two different linearisations.
    ///
    /// So the reference here is the steady solver iterated to self-consistency,
    /// and what the test then establishes is stronger: the transient march and
    /// the steady Picard iteration converge on the **same** non-linear
    /// solution.
    ///
    /// The characteristic time is the fuel's thermal diffusion time,
    /// `rho cp R^2 / k` — for UO2 near 1000 K roughly
    /// `3.2 J/(cm^3 K) * 0.41^2 cm^2 / 0.03 W/(cm K)`, of order 18 s. So 60 s
    /// of marching should be essentially converged.
    ///
    /// Pass criterion: after 6000 steps (60 s) the centre temperature is within
    /// 1 K of the Picard-converged steady solution, and the approach is
    /// monotonic.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// After 60 s of marching the centre reached **1081.899 K**, against the
    /// Picard-converged steady solution's **1081.899 K** — agreement to the
    /// three decimals printed. The heat-up was monotonic throughout.
    ///
    /// **Interpretation.** Two different routes to the same non-linear fixed
    /// point: a time march that never solves a steady problem, and a Picard
    /// iteration that never takes a time step. Their agreement verifies that
    /// the capacity terms vanish correctly at convergence and that both
    /// solvers linearise the conductivity the same way.
    ///
    /// Note the converged centre is **10.6 K above** the 1071.3 K a single
    /// steady call at a 900 K property iterate gives — the non-linearity is
    /// not negligible at this power, which is why the reference's callers run
    /// Picard passes.
    #[test]
    fn marching_from_cold_converges_on_the_steady_solution() {
        let (fuel, maxir) = neacrp_rod();
        let coolant = 580.0;

        // Picard-converge the steady solver, so the comparison is against a
        // solution of the same non-linear problem.
        let mut steady = vec![900.0; 10];
        for _ in 0..100 {
            let (next, ok) = fuelrodheat_1dcylnd(&fuel, maxir, &steady, 300.0, 1.5, coolant);
            assert_eq!(ok, Solve::Ok);
            steady = next;
        }

        let mut state = vec![coolant; 10];
        let mut prev_centre = state[0];
        let mut monotonic = true;
        for step in 0..6000 {
            let (next, ok) =
                fuelrodheattime_1dcylnd(&fuel, maxir, &state, &state, 300.0, 1.5, coolant, 0.01);
            assert_eq!(ok, Solve::Ok, "step {step}");
            if next[0] < prev_centre - 1e-9 {
                monotonic = false;
            }
            prev_centre = next[0];
            state = next;
        }

        eprintln!(
            "after 60 s: centre = {:.3} K, steady = {:.3} K",
            state[0], steady[0]
        );
        assert!(monotonic, "the heat-up was not monotonic");
        assert!(
            (state[0] - steady[0]).abs() < 1.0,
            "centre {} did not reach steady {}",
            state[0],
            steady[0]
        );
    }

    /// The rod heats up on a timescale of seconds, not milliseconds or hours.
    ///
    /// # Methodology
    ///
    /// A magnitude check on the capacity terms, which the two limiting tests
    /// above cannot provide: they verify `cap -> 0` and `cap -> inf` behave,
    /// but not that `cap` is the right *size* in between. Starting cold, the
    /// centre temperature is sampled at 1 s and 10 s of marching.
    ///
    /// Expected from `rho cp R^2 / k` ~ 18 s: appreciable but far from complete
    /// heating at 1 s, and most of the way there by 10 s.
    ///
    /// Pass criterion: at 1 s between 5% and 60% of the total rise; at 10 s
    /// above 80%. Wide bands, but an order-of-magnitude error in `rho*cp` — a
    /// missing `/1000`, say — would miss them by far more than that.
    ///
    /// # Results — measured 2026-08-17
    ///
    /// Total rise **501.9 K**; the centre had heated **20.8%** of the way at
    /// 1 s and **91.8%** at 10 s.
    ///
    /// **Interpretation.** That is a response time of order 5-6 s, the right
    /// scale for a UO2 pellet's thermal diffusion time and comfortably inside
    /// the plausible band. This is the check the two limiting tests cannot
    /// provide: they confirm `cap -> 0` and `cap -> inf` behave, but only this
    /// one confirms `cap` is the right *size*. A missing `/1000` in the
    /// volumetric heat capacity — the most likely transcription error in
    /// `rhocp` — would put the timescale out by three orders of magnitude and
    /// fail decisively.
    #[test]
    fn the_heat_up_timescale_is_seconds() {
        let (fuel, maxir) = neacrp_rod();
        let coolant = 580.0;
        // Picard-converged, as in the test above.
        let mut steady = vec![900.0; 10];
        for _ in 0..100 {
            let (next, _) = fuelrodheat_1dcylnd(&fuel, maxir, &steady, 300.0, 1.5, coolant);
            steady = next;
        }
        let total_rise = steady[0] - coolant;

        let mut state = vec![coolant; 10];
        let mut at_1s = 0.0;
        for step in 0..1000 {
            let (next, _) =
                fuelrodheattime_1dcylnd(&fuel, maxir, &state, &state, 300.0, 1.5, coolant, 0.01);
            state = next;
            if step == 99 {
                at_1s = (state[0] - coolant) / total_rise;
            }
        }
        let at_10s = (state[0] - coolant) / total_rise;

        eprintln!(
            "total rise {total_rise:.1} K; heated {:.1}% at 1 s, {:.1}% at 10 s",
            at_1s * 100.0,
            at_10s * 100.0
        );
        assert!(
            (0.05..0.60).contains(&at_1s),
            "1 s fraction {at_1s} is outside the plausible band"
        );
        assert!(at_10s > 0.80, "10 s fraction {at_10s} is too low");
    }

    /// The gap dummy is pinned at 1 K here too, and no `dt` changes it.
    #[test]
    fn the_gap_node_is_still_a_dummy_at_one_kelvin() {
        let (fuel, maxir) = neacrp_rod();
        let old = vec![700.0; 10];
        for dt in [1e-6, 0.01, 1.0, 1e12] {
            let (out, _) =
                fuelrodheattime_1dcylnd(&fuel, maxir, &old, &old, 300.0, 1.5, 580.0, dt);
            assert!(
                (out[6] - 1.0).abs() < 1e-12,
                "dt = {dt}: gap node is {}, not 1",
                out[6]
            );
        }
    }

    /// A material with no heat capacity entry is rejected, since `rhocp` is one
    /// shorter than `tcon`.
    #[test]
    #[should_panic(expected = "has no matching heat capacity")]
    fn a_material_without_a_heat_capacity_is_rejected() {
        let (mut fuel, maxir) = neacrp_rod();
        fuel.rhocp = vec![VolumetricHeatCapacity::Uo2Fuel]; // clad entry missing
        let old = vec![700.0; 10];
        let _ = fuelrodheattime_1dcylnd(&fuel, maxir, &old, &old, 300.0, 1.5, 580.0, 0.01);
    }
}
