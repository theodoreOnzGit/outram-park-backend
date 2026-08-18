//! Steady 1-D cylindrical fuel-rod conduction — the live path.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `fuelrodheat_1dcylnd.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # What this computes, and why it matters
//!
//! The radial temperature profile through one fuel rod at one axial node:
//! pellet centre out through the fuel, across the fuel-cladding gap, through
//! the cladding, and into the coolant through a convective boundary condition.
//! `th_solverxyz.m` calls it once per fuelled node, and its output drives the
//! **Doppler feedback** — so an error here moves reactivity, not just a
//! reported temperature.
//!
//! The whole integrated heat equation is divided through by `2*pi`, as the
//! reference's own header line states. That is why the source terms are
//! `0.5*q*(r_out^2 - r_in^2)` rather than `pi*q*(...)`.
//!
//! # The interface-node doubling — read this before the code
//!
//! The radial mesh has `maxir` nodes, but the linear system has
//! **`maxid = maxir + surfcount`** unknowns, where `surfcount` counts
//! solid/void transitions. The extra unknowns are *surface* temperatures at
//! material interfaces, where the profile has a kink the cell-centred nodes
//! cannot represent.
//!
//! The loop therefore walks two counters: `ir` over the radial mesh and `id`
//! over the unknowns. A `surf` flag makes it visit one `ir` **twice**, emitting
//! two unknowns. Worked through for the NEACRP mesh — 5 fuel, 1 gap, 2 clad,
//! so `whichk = [1,1,1,1,1,0,2,2]`, `maxir = 8`, `surfcount = 2`,
//! `maxid = 10`:
//!
//! | `ir` | `id` | what it is |
//! |---|---|---|
//! | 1-4 | 1-4 | fuel interior |
//! | 5 | 5 | last fuel node |
//! | 5 (again) | 6 | **fuel outer surface** |
//! | 6 | 7 | the gap — a dummy row, see below |
//! | 7 | 8 | cladding |
//! | 8 | 9 | last cladding node |
//! | — | 10 | **cladding outer surface**, carrying the coolant BC |
//!
//! Conduction across the gap links `id = 6` directly to `id = 8`, skipping the
//! dummy. Note the count works out for a reason that is not the obvious one:
//! `surfcount = 2` counts the fuel/gap *and* gap/clad transitions, but only the
//! first produces a doubled node — the second extra unknown is the outer
//! surface, created by the `ir == maxir` branch. The arithmetic is right for
//! this configuration; it is not obviously right for every configuration.

use crate::matlab::{Decomposition, SparseMatrix};
use crate::types::FuelGeometry;

/// The conduction solve's outcome, alongside the profile.
///
/// The reference returns only the temperature vector; when it contains `NaN` it
/// *displays* `laplc`, `bvec` and `pwr` and returns anyway. Callers cannot
/// distinguish that from a good solve, and `th_solverxyz.m` has a whole
/// `any(isnan(...))` recovery block downstream to cope. Returning the fact
/// explicitly is cheaper than reproducing the print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Solve {
    /// Every temperature is finite.
    Ok,
    /// The solve produced at least one `NaN` — a singular or near-singular
    /// operator. The reference dumps diagnostics to the console here.
    NotFinite,
}

/// `results = fuelrodheat_1dcylnd(params, geometry, temps, pwr, bc, modtemp)`.
///
/// # Arguments
///
/// - `fuel` — needs `whichk`, `tcon`, `gap_conductance`, `lr` and `ctr`.
/// - `maxir` — radial node count, the reference's `params.fuel.maxir`.
/// - `temps` — the **previous** temperature profile, `maxid` long, in **K**.
///   Used only to evaluate the temperature-dependent conductivities, which is
///   what makes the whole solve a Picard iteration when the caller feeds its
///   own output back. Note it is indexed by `id`, not `ir`.
/// - `pwr` — volumetric power density in the pellet, **W/cm³**.
/// - `bc` — outer boundary conductance, **W/(cm·K)**; `hcoeff * Rtot` in the
///   live path.
/// - `modtemp` — coolant (moderator) temperature, **K**, the sink the boundary
///   condition drives towards.
///
/// # Returns
///
/// `(temperatures, outcome)` — the profile over the `maxid` unknowns in **K**,
/// and whether it came back finite.
///
/// # Reference defects carried here
///
/// - **The gap row is a dummy that reads as `T = 1`.** A node with
///   `whichk == 0` gets `bvec(id) = 1` and keeps the preallocated diagonal
///   `1`, so its temperature solves to exactly `1` — in K, a physically absurd
///   value. Conduction bypasses it, so it does not corrupt the profile, and
///   `th_solverxyz.m` clamps it up to the coolant temperature immediately
///   after. But it is in the returned vector, and any caller that averages over
///   the profile without knowing this gets a wrong answer. Recorded as T7.
/// - **A missing branch leaves a stale conductance (T8).** In the `surf == 1`
///   pass the body is guarded by `if whichk(ir+1) == 0` with **no `else`**. If
///   two *different, both solid* materials are adjacent — fuel directly against
///   cladding, no gap — that pass emits no forward link at all and
///   `laplcele(id) = kminus + kplus` uses the previous pass's `kplus`,
///   producing a row with an inflated diagonal and a missing off-diagonal. The
///   row no longer balances, so the operator silently stops conserving energy.
///   Unreachable for the benchmark meshes, which always put a gap between fuel
///   and cladding.
/// - **`temps` is read at `id + 1` before that unknown exists.** Every
///   conductivity pair reads `temps(id)` and `temps(id+1)`, including at
///   `id = maxid - 1`. The vector must therefore be `maxid` long even though
///   the mesh has `maxir` nodes — a caller sizing it from `maxir` reads past
///   the end. Asserted here.
/// - **The `ir == maxir` branch uses one material for both sides.** It reads
///   `tcon{whichk(ir)}` for `cond` *and* `condplus`, where every other branch
///   reads `whichk(ir+1)` for the second. Deliberate — there is no `ir+1` — but
///   it means the outer surface conductance is a self-harmonic-mean, i.e. just
///   `cond`, evaluated at two different temperatures.
/// - **A doubled interface conductance.** The `whichk(ir+1) == 0` branch
///   multiplies its harmonic mean by an extra `2`, with the un-doubled line
///   commented out directly above. No derivation is given.
///
/// # Panics
///
/// If `temps` is shorter than the `maxid` the mesh implies, if a `whichk`
/// value has no matching conductivity, or if the mesh has fewer than two nodes.
pub fn fuelrodheat_1dcylnd(
    fuel: &FuelGeometry,
    maxir: usize,
    temps: &[f64],
    pwr: f64,
    bc: f64,
    modtemp: f64,
) -> (Vec<f64>, Solve) {
    assert!(
        maxir >= 2,
        "the stencil needs at least two radial nodes, got {maxir}"
    );

    let whichk = &fuel.whichk;
    let lr = &fuel.lr;
    let ctr = &fuel.ctr;
    // `whichf = (whichk == 1)` — the fuel, where power is deposited.
    let is_fuel = |ir: usize| whichk[ir] == 1;

    let mut sum_lr = vec![0.0; lr.len()];
    let mut acc = 0.0;
    for (i, &l) in lr.iter().enumerate() {
        acc += l;
        sum_lr[i] = acc;
    }

    // `surfcount` — solid/void transitions in either direction.
    let mut surfcount = 0usize;
    for ir in 0..maxir - 1 {
        let a = whichk[ir] != 0;
        let b = whichk[ir + 1] != 0;
        if a != b {
            surfcount += 1;
        }
    }
    let maxid = maxir + surfcount;

    assert!(
        temps.len() >= maxid,
        "temps is {} long; the stencil reads up to maxid = {maxid}",
        temps.len()
    );

    let conductivity = |m: usize, t: f64| -> f64 {
        assert!(
            m >= 1 && m <= fuel.tcon.len(),
            "whichk = {m} has no matching conductivity; tcon has {} entries",
            fuel.tcon.len()
        );
        fuel.tcon[m - 1].at(t)
    };
    // `2*(k1*k2)/(k1 + k2)` — the harmonic mean the live path uses.
    let harmonic = |a: f64, b: f64| 2.0 * (a * b) / (a + b);

    let mut diag = vec![1.0; maxid];
    let mut off: Vec<(usize, usize, f64)> = Vec::new();
    let mut bvec = vec![0.0; maxid];

    // Node 0: the axis. No inward face.
    let cond = conductivity(whichk[0], temps[0]);
    let condplus = conductivity(whichk[1], temps[1]);
    let mut kplus = harmonic(cond, condplus) * ctr[0] / lr[0];
    diag[0] = kplus;
    off.push((0, 1, -kplus));
    if is_fuel(0) {
        bvec[0] = 0.5 * pwr * ctr[0] * ctr[0];
    }

    let mut idminus = 0usize;
    let mut surf = false;
    // 0-based: the reference's `ir = 2` and `id = 2`.
    let mut ir = 1usize;
    let mut id = 1usize;

    while ir < maxir {
        if whichk[ir] == 0 {
            // The dummy gap row — diagonal stays 1, so this solves to T = 1.
            bvec[id] = 1.0;
            ir += 1;
            id += 1;
            continue;
        }

        let kminus = kplus;

        if surf {
            // No `else` branch in the reference — defect T8.
            if whichk[ir + 1] == 0 {
                kplus = fuel.gap_conductance * ctr[ir + 1];
                off.push((id, id + 2, -kplus));
                if is_fuel(ir) {
                    bvec[id] = 0.5 * pwr * (sum_lr[ir] * sum_lr[ir] - ctr[ir] * ctr[ir]);
                }
            }
        } else if ir == maxir - 1 {
            // Both sides read the same material; there is no `ir + 1`.
            let cond = conductivity(whichk[ir], temps[id]);
            let condplus = conductivity(whichk[ir], temps[id + 1]);
            kplus = harmonic(cond, condplus) * ctr[ir] / lr[ir];
            off.push((id, id + 1, -kplus));
            if is_fuel(ir) {
                bvec[id] = 0.5 * pwr * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
        } else if whichk[ir + 1] == 0 {
            let cond = conductivity(whichk[ir], temps[id]);
            let condplus = conductivity(whichk[ir], temps[id + 1]);
            // The extra `* 2`, with the un-doubled form commented out above it.
            kplus = harmonic(cond, condplus) * ctr[ir] / lr[ir] * 2.0;
            off.push((id, id + 1, -kplus));
            if is_fuel(ir) {
                bvec[id] = 0.5 * pwr * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
        } else {
            let cond = conductivity(whichk[ir], temps[id]);
            let condplus = conductivity(whichk[ir + 1], temps[id + 1]);
            kplus = harmonic(cond, condplus) * ctr[ir] / lr[ir];
            off.push((id, id + 1, -kplus));
            if is_fuel(ir) {
                bvec[id] = 0.5 * pwr * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
        }

        diag[id] = kminus + kplus;
        off.push((id, idminus, -kminus));
        idminus = id;

        // Advance `ir` unless this is the first of a doubled pair.
        if ir == maxir - 1 || whichk[ir] == whichk[ir + 1] || surf {
            ir += 1;
            surf = false;
        } else {
            surf = true;
        }
        id += 1;
    }

    // The outer surface unknown, carrying the convective boundary condition.
    let kminus = kplus;
    diag[maxid - 1] = kminus + bc;
    off.push((maxid - 1, idminus, -kminus));
    bvec[maxid - 1] = bc * modtemp;

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
    use crate::types::Conductivity;

    /// The NEACRP-shaped rod: 5 fuel nodes, 1 gap, 2 cladding.
    ///
    /// Dimensions are representative rather than exact — a 0.41 cm pellet
    /// radius, a 0.006 cm gap and a 0.06 cm clad, which is a typical PWR pin.
    fn neacrp_rod() -> (FuelGeometry, usize) {
        let fueln = 5;
        let gapn = 1;
        let cladn = 2;
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
            gap_conductance: 0.35,
            ..Default::default()
        };
        (fuel, maxir)
    }

    /// The unknown count is `maxir + surfcount`, and the mesh walk lands
    /// exactly on it.
    ///
    /// # Methodology
    ///
    /// For the NEACRP rod `whichk = [1,1,1,1,1,0,2,2]` there are two solid/void
    /// transitions, so `maxid = 8 + 2 = 10`. If the `ir`/`id` walk described in
    /// the module docs were off by one anywhere, the returned vector would be
    /// the wrong length or the assembly would panic on a bounds check.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `maxid = 10` as predicted by the table in the module docs, and the walk
    /// completed without a bounds panic — so the `ir`/`id` bookkeeping lands
    /// exactly on the last unknown.
    #[test]
    fn the_unknown_count_matches_the_interface_doubling() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![800.0; 10];
        let (t, outcome) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 1.5, 580.0);

        assert_eq!(outcome, Solve::Ok);
        assert_eq!(t.len(), 10, "maxid should be maxir + surfcount = 8 + 2");
    }

    /// The temperature profile falls monotonically from the pellet centre to
    /// the cladding surface, and every fuel temperature exceeds the coolant.
    ///
    /// # Methodology
    ///
    /// A PWR pin at 300 W/cm³ with a coolant sink at 580 K and a boundary
    /// conductance of 1.5 W/(cm·K). Physically the profile must be a
    /// parabola-like fall through the pellet, a jump across the gap, a small
    /// fall through the cladding, and a final drop into the coolant. So:
    /// centre hottest, cladding outer surface coolest, everything above 580 K.
    ///
    /// The gap dummy at index 6 is **excluded** — it solves to exactly 1 K by
    /// construction (defect T7), which is the subject of its own test below.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// At 300 W/cm³ into a 580 K coolant, in K:
    ///
    /// | Node | | T |
    /// |---|---|---|
    /// | 0-4 | fuel, centre outward | 1071.3, 1057.5, 1016.0, 947.0, 850.3 |
    /// | 5 | pellet surface | 788.1 |
    /// | 6 | gap dummy | **1.0** (defect T7) |
    /// | 7-8 | cladding | 613.6, 604.9 |
    /// | 9 | clad outer surface | 596.8 |
    ///
    /// **Interpretation.** The breakdown is physically right for a PWR pin at
    /// high power: a 283 K fall through the pellet (the parabolic conduction
    /// profile), a **174 K jump across the gap** — the dominant resistance, as
    /// it should be for a 0.35 W/(cm²·K) gap conductance — then only 8.7 K
    /// through the cladding, which is a good conductor, and a final 16.8 K into
    /// the coolant. A centre temperature near 1070 K at 300 W/cm³ is the right
    /// order for a PWR at power.
    ///
    /// This is a verification that the stencil is assembled correctly and the
    /// units are consistent. It is **not** validated against a fuel-performance
    /// code or measured data.
    #[test]
    fn the_profile_falls_from_centre_to_surface() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![900.0; 10];
        let (t, outcome) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 1.5, 580.0);
        assert_eq!(outcome, Solve::Ok);

        eprintln!("profile: {t:?}");

        // Fuel nodes 0..=5 (5 interior plus the pellet surface).
        for i in 1..=5 {
            assert!(
                t[i] < t[i - 1],
                "node {i} at {} is not below node {} at {}",
                t[i],
                i - 1,
                t[i - 1]
            );
        }
        // Cladding: indices 7, 8, 9 (index 6 is the gap dummy).
        for i in 8..=9 {
            assert!(t[i] < t[i - 1], "clad node {i} is not below its inner neighbour");
        }
        // The gap jump: the pellet surface is hotter than the clad inner face.
        assert!(t[5] > t[7], "no temperature drop across the gap");
        // Everything except the dummy is above the coolant.
        for (i, v) in t.iter().enumerate() {
            if i == 6 {
                continue;
            }
            assert!(*v > 580.0, "node {i} at {v} K is below the coolant");
        }
    }

    /// Defect T7, pinned: the gap node solves to exactly 1 K.
    ///
    /// # Methodology
    ///
    /// A node with `whichk == 0` takes `bvec(id) = 1` and keeps the
    /// preallocated diagonal `1`, and nothing else writes to its row — the
    /// conduction path bridges from `id` to `id + 2` around it. So its row is
    /// literally `1 * T = 1`.
    ///
    /// Pass criterion: index 6 comes back as 1.0 to within floating-point
    /// tolerance, regardless of power, coolant temperature or boundary
    /// conductance.
    ///
    /// This is not a harmless quirk in every context: the value sits in the
    /// returned profile. `th_solverxyz.m` happens to clamp it up to the coolant
    /// temperature on the next line and takes its Doppler average from the
    /// centre and pellet-surface nodes rather than a volume average, so the
    /// live path is unaffected — but a caller that volume-averages the profile,
    /// as the commented-out `fueltempavg` line in `th_solverxyz.m` would have,
    /// gets a corrupted mean.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// The gap node returned exactly `1.0` at all three conditions —
    /// (300 W/cm³, 580 K), (50, 500) and (600, 620) — confirming it is a
    /// constant independent of every physical input. Defect T7 confirmed.
    #[test]
    fn the_gap_node_is_a_dummy_pinned_at_one_kelvin() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![900.0; 10];

        for (pwr, modtemp) in [(300.0, 580.0), (50.0, 500.0), (600.0, 620.0)] {
            let (t, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, pwr, 1.5, modtemp);
            eprintln!("pwr = {pwr}, modtemp = {modtemp}: gap node = {}", t[6]);
            assert!(
                (t[6] - 1.0).abs() < 1e-12,
                "the gap dummy should be exactly 1 K, got {}",
                t[6]
            );
        }
    }

    /// Zero power gives a rod in equilibrium with its coolant.
    ///
    /// # Methodology
    ///
    /// With no heat source, the only driving term is `bc * modtemp` on the
    /// outer row, and every interior row balances to zero. The steady solution
    /// is therefore a uniform profile at the coolant temperature. This is an
    /// **analytical** check on the whole assembly — any row that does not
    /// conserve energy shows up as a departure from uniformity.
    ///
    /// The gap dummy at index 6 is excluded, since its row is not a
    /// conservation statement.
    ///
    /// Pass criterion: every non-dummy node within 1e-9 K of 580 K.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Every non-dummy node came back at 580 K to within **1.8e-12 K** — the
    /// worst was node 0 at 579.9999999999982.
    ///
    /// **Interpretation.** This is the strongest check in the module. An
    /// unpowered rod in contact with a fixed-temperature sink has the uniform
    /// profile as its exact analytical solution, and reaching it to 1e-12
    /// means every row of the operator sums to zero — including the two that
    /// bridge the gap, which the row-sum test in
    /// [`crate::makeheatlaplacian_1dcylnd`] cannot reach. So the interface
    /// doubling, the gap bridge and the boundary row are all internally
    /// consistent. It does not verify the *magnitudes* of the conductances,
    /// which cancel out of this particular solution.
    #[test]
    fn an_unpowered_rod_sits_at_the_coolant_temperature() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![580.0; 10];
        let (t, outcome) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 0.0, 1.5, 580.0);
        assert_eq!(outcome, Solve::Ok);

        eprintln!("unpowered profile: {t:?}");
        for (i, v) in t.iter().enumerate() {
            if i == 6 {
                continue;
            }
            assert!(
                (v - 580.0).abs() < 1e-9,
                "node {i} at {v} K should be at the coolant temperature"
            );
        }
    }

    /// The centre temperature rises linearly with power, as a linear conduction
    /// problem requires.
    ///
    /// # Methodology
    ///
    /// The operator does depend on temperature through the conductivities, but
    /// `temps` is held fixed here, so within one call the problem is linear:
    /// `T = T_coolant + A * pwr` for a fixed geometry. Doubling the power must
    /// exactly double the temperature *rise* above the coolant.
    ///
    /// This is a strong check on the source-term assembly: the
    /// `0.5*q*(r_out^2 - r_in^2)` annular integrals are the only place `pwr`
    /// enters, so an error in any of them breaks the proportionality.
    ///
    /// Pass criterion: the rise ratio is 2 to within 1e-9.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// A centre rise of **327.538 K** at 200 W/cm³ and **655.076 K** at
    /// 400 W/cm³ — a ratio of 2.0 to better than 1e-9, i.e. exact in floating
    /// point.
    ///
    /// **Interpretation.** Every one of the annular source integrals
    /// contributes to the centre temperature, so exact proportionality across
    /// all of them is a sharp check on the `0.5*q*(r_out^2 - r_in^2)` assembly
    /// and on the `2*pi` division the reference's header describes. A single
    /// mis-stated radius would break it.
    #[test]
    fn the_centre_temperature_rise_is_linear_in_power() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![900.0; 10];
        let coolant = 580.0;

        let (a, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 200.0, 1.5, coolant);
        let (b, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 400.0, 1.5, coolant);

        let rise_a = a[0] - coolant;
        let rise_b = b[0] - coolant;
        eprintln!("centre rise: {rise_a} K at 200 W/cm3, {rise_b} K at 400 W/cm3");
        assert!(
            ((rise_b / rise_a) - 2.0).abs() < 1e-9,
            "doubling the power gave a rise ratio of {}",
            rise_b / rise_a
        );
    }

    /// A better-cooled rod runs colder.
    #[test]
    fn a_higher_boundary_conductance_lowers_the_whole_profile() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![900.0; 10];

        let (poor, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 0.5, 580.0);
        let (good, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 5.0, 580.0);

        assert!(
            good[0] < poor[0],
            "better cooling should lower the centre: {} vs {}",
            good[0],
            poor[0]
        );
        assert!(good[9] < poor[9], "and the surface too");
    }

    /// A `temps` vector sized from `maxir` rather than `maxid` reads past the
    /// end — the trap the doc comment warns about.
    #[test]
    #[should_panic(expected = "the stencil reads up to maxid")]
    fn a_temps_vector_sized_from_maxir_is_too_short() {
        let (fuel, maxir) = neacrp_rod();
        let temps = vec![800.0; 8]; // maxir, not maxid
        let _ = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 1.5, 580.0);
    }
}
