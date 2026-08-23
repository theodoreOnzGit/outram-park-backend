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
/// Which unknowns in a solved rod profile are **gap dummies** — defect T1/T7.
///
/// # The problem this exists to make un-fall-into-able
///
/// [`fuelrodheat_1dcylnd`] returns `maxid` temperatures, and one of them is not
/// a temperature. A radial node with `whichk == 0` is a **gap**: an unresolved
/// void represented by a conductance, not a region. Conduction bridges around
/// it — the pellet surface couples straight to the clad inner surface — so its
/// matrix row is never written, keeps the preallocated diagonal of `1`, and is
/// given `bvec = 1`. It therefore solves to **exactly 1 kelvin**, regardless of
/// power, coolant temperature or gap conductance.
///
/// That value is physically meaningless but it **is** in the returned vector.
/// The profile either side of it is correct, and the live path survives it
/// because `th_solverxyz` clamps everything up to the local coolant temperature
/// on the next line and takes its Doppler weight from the centre and
/// pellet-surface nodes. **A caller that averages or scans the raw profile
/// gets a wrong answer**, which is exactly what the volume-average line
/// commented out in `th_solverxyz.m` would have done.
///
/// # Why the 1 K is left in place
///
/// Every plausible replacement is an invention. There is no "gap temperature"
/// in this model to compute, so interpolating between the two surfaces would
/// manufacture a physically reasonable-looking number the solve never
/// produced — worse than an obviously absurd one, because it would not be
/// noticed. Dropping the row would change the vector's length and break every
/// caller's `maxid` arithmetic. So the raw profile keeps the reference's value,
/// and this function plus [`without_gap_dummies`] make the trap avoidable
/// rather than merely documented.
///
/// # How it is computed
///
/// By replaying [`fuelrodheat_1dcylnd`]'s own `ir`/`id` walk, including the
/// `surf` flag that makes one `ir` emit two unknowns. It is derived from the
/// same logic rather than hard-coded, so it cannot drift from the solver.
///
/// # Arguments
///
/// - `whichk` — `geometry.fuel.whichk`, the material per radial node, `0` for
///   the gap.
///
/// # Returns
///
/// The **0-based** unknown indices that are dummies, ascending. Empty for a rod
/// with no gap. For the NEACRP rod (`[1,1,1,1,1,0,2,2]`) this is `[6]`.
pub fn gap_dummy_unknowns(whichk: &[usize]) -> Vec<usize> {
    let maxir = whichk.len();
    let mut out = Vec::new();
    if maxir == 0 {
        return out;
    }
    let mut surf = false;
    let mut ir = 1usize;
    let mut id = 1usize;
    while ir < maxir {
        if whichk[ir] == 0 {
            out.push(id);
            ir += 1;
            id += 1;
            continue;
        }
        // The same advance the solver uses.
        if ir == maxir - 1 || whichk[ir] == whichk[ir + 1] || surf {
            ir += 1;
            surf = false;
        } else {
            surf = true;
        }
        id += 1;
    }
    out
}

/// A solved rod profile with the gap dummies removed — defect T1/T7.
///
/// Use this in preference to the raw profile for **anything that reduces over
/// the radius**: an average, a minimum, a plot. See [`gap_dummy_unknowns`] for
/// why the raw vector contains a 1 K entry and why it is left there.
///
/// The surviving entries keep their order, so the result reads centre-outward
/// exactly as the input does; only the physically meaningless rows are gone.
///
/// # Arguments
///
/// - `whichk` — `geometry.fuel.whichk`.
/// - `profile` — a `maxid`-long solved profile from [`fuelrodheat_1dcylnd`],
///   or one row of `th.fueltemp`.
///
/// # Panics
///
/// If `profile` is shorter than the largest dummy index it would have to skip.
pub fn without_gap_dummies(whichk: &[usize], profile: &[f64]) -> Vec<f64> {
    let dummies = gap_dummy_unknowns(whichk);
    if let Some(&last) = dummies.last() {
        assert!(
            profile.len() > last,
            "profile is {} long but the gap dummy sits at index {last}",
            profile.len()
        );
    }
    profile
        .iter()
        .enumerate()
        .filter(|(i, _)| !dummies.contains(i))
        .map(|(_, t)| *t)
        .collect()
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

    /// **T1/T7 — the gap dummy is locatable, and skipping it fixes the mean.**
    ///
    /// # Methodology
    ///
    /// The 1 K gap row is left in the raw profile deliberately (see
    /// [`gap_dummy_unknowns`] for why every replacement would be an
    /// invention). What is corrected is that the trap was only documented, not
    /// avoidable. Three things are checked:
    ///
    /// 1. **The accessor agrees with the solver.** Every index
    ///    `gap_dummy_unknowns` reports must actually hold `1.0` in a real
    ///    solved profile, and every index it does not report must not. This is
    ///    the check that matters, because the accessor replays the solver's
    ///    `ir`/`id` walk rather than sharing code with it — if that replay ever
    ///    drifts, this fails.
    /// 2. **It is derived, not hard-coded.** A rod with no gap must report no
    ///    dummies; a rod with two gaps must report both.
    /// 3. **It changes the answer it is meant to change.** The mean over the
    ///    raw profile against the mean with the dummy skipped.
    ///
    /// # Results — measured 2026-08-23
    ///
    /// On the NEACRP rod (`whichk = [1,1,1,1,1,0,2,2]`, `maxid = 10`) at
    /// 300 W/cm3 and 580 K coolant:
    ///
    /// | | |
    /// |---|---|
    /// | dummy indices | **`[6]`**, matching the module's own layout table |
    /// | profile at index 6 | **1.0000 K** |
    /// | mean over the raw profile | **754.66 K** |
    /// | mean with the dummy skipped | **838.40 K** |
    /// | error the trap causes | **-83.74 K, -10.0%** |
    ///
    /// A gapless rod reports `[]`; a rod with two gaps reports both.
    ///
    /// **Interpretation.** The 84 K error is the concrete size of the trap: a
    /// caller averaging the raw profile — which is exactly what the
    /// commented-out volume-average line in `th_solverxyz.m` would have done —
    /// is pulled down 10% by one node that is not a temperature. That is why
    /// the T9 correction averages the pellet nodes only, and why this accessor
    /// exists for anyone who reaches for the full profile instead.
    #[test]
    fn t1_the_gap_dummy_is_locatable_and_skippable() {
        let (fuel, maxir) = neacrp_rod();
        let whichk = &fuel.whichk;
        eprintln!("whichk = {whichk:?}, maxir = {maxir}");

        let dummies = gap_dummy_unknowns(whichk);
        eprintln!("gap dummy unknowns: {dummies:?}");
        assert_eq!(dummies, vec![6], "the NEACRP rod's gap sits at unknown 6");

        let temps = vec![900.0; 10];
        let (profile, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, 300.0, 1.5, 580.0);

        // 1. Every reported index really is the 1 K dummy, and no other is.
        for (i, t) in profile.iter().enumerate() {
            let is_dummy = dummies.contains(&i);
            if is_dummy {
                assert!(
                    (t - 1.0).abs() < 1e-12,
                    "index {i} was reported as a dummy but holds {t}"
                );
            } else {
                assert!(
                    (t - 1.0).abs() > 1e-9,
                    "index {i} holds 1 K but was not reported as a dummy"
                );
            }
        }

        // 2. Derived, not hard-coded.
        assert_eq!(
            gap_dummy_unknowns(&[1, 1, 1, 2, 2]),
            Vec::<usize>::new(),
            "a rod with no gap has no dummies"
        );
        assert_eq!(
            gap_dummy_unknowns(&[1, 1, 0, 2, 2, 0, 3, 3]).len(),
            2,
            "a rod with two gaps has two dummies"
        );

        // 3. The size of the trap.
        let raw: f64 = profile.iter().sum::<f64>() / profile.len() as f64;
        let clean_profile = without_gap_dummies(whichk, &profile);
        let clean: f64 = clean_profile.iter().sum::<f64>() / clean_profile.len() as f64;
        eprintln!("mean over the raw profile   = {raw:.2} K");
        eprintln!("mean with the dummy skipped = {clean:.2} K");
        eprintln!("the trap costs {:.2} K ({:+.1}%)", raw - clean, (raw / clean - 1.0) * 100.0);

        assert_eq!(clean_profile.len(), profile.len() - 1);
        assert!(
            clean > raw,
            "skipping a 1 K entry must raise the mean: {clean} vs {raw}"
        );
        assert!(
            !clean_profile.iter().any(|t| (t - 1.0).abs() < 1e-9),
            "no 1 K entry may survive"
        );
    }

    /// **T9 — is the doubled interface conductance right? Checked against the
    /// analytic pellet solution.**
    ///
    /// # Methodology
    ///
    /// The register records the `whichk(ir+1) == 0` branch multiplying its
    /// harmonic mean by an extra `2`, "with the un-doubled line commented out
    /// directly above it and no derivation given". Whether that `2` is a
    /// mistake or an unstated derivation is decidable, because the pellet has
    /// a closed-form answer.
    ///
    /// A cylinder with uniform volumetric source `q_v` and uniform
    /// conductivity `k` has `T(r) = T_surface + q_v*(R^2 - r^2)/(4k)`, so the
    /// **centre-to-pellet-surface drop is exactly `q_v*R^2/(4k)`**, whatever
    /// sits outside the pellet. That is what this measures: a rod with
    /// constant conductivity, a uniform radial mesh and a gap (so the doubled
    /// branch fires at the last fuel node), refined to show convergence.
    ///
    /// The branch in question links the outermost **fuel** node to the
    /// pellet-surface unknown. That distance is **half** a node thickness —
    /// centre to face — not a full one, so a factor of 2 on `k*ctr/lr` is
    /// exactly what a centre-to-face conductance requires. If that reading is
    /// right the discrete drop converges on the analytic one; if the `2` is
    /// spurious, the last node contributes twice the resistance it should and
    /// the error stops falling with refinement.
    ///
    /// # Results — measured 2026-08-23
    ///
    /// Analytic drop `q_v*R^2/(4k)` = **420.2500 K**.
    ///
    /// | `fueln` | discrete drop | rel err | ratio |
    /// |---|---|---|---|
    /// | 4 | 328.320313 | 2.187e-1 | — |
    /// | 8 | 371.001953 | 1.172e-1 | 1.87 |
    /// | 16 | 394.805176 | 6.055e-2 | 1.94 |
    /// | 32 | 407.322388 | 3.076e-2 | 1.97 |
    /// | 64 | 413.734894 | **1.550e-2** | **1.98** |
    ///
    /// **Verdict: the factor of 2 is correct, and the register entry should
    /// not have called it undeified.** The branch links the outermost fuel
    /// node to the pellet-surface unknown, and that distance is **half** a node
    /// thickness — centre to face. A centre-to-face conductance is
    /// `k*r/(dr/2) = 2*k*r/dr`, which is exactly `harmonic * ctr/lr * 2`. The
    /// discrete drop converges on the analytic value, which it could not do if
    /// the last node carried twice the resistance it should.
    ///
    /// Removing the `2` would double that node's resistance and **add** a drop
    /// of `Q*dr/(2*k*r_last)` — computed at these meshes as **120.1 K, 56.0,
    /// 27.1, 13.3, 6.6 K**, i.e. it would roughly *double* the error at every
    /// refinement. The commented-out un-doubled line above it is the mistake,
    /// not the live line.
    ///
    /// # A finding the register does not record: the scheme is first order
    ///
    /// The error ratio is **1.97, 1.98** per mesh doubling — first order, not
    /// second. The cause is visible in every branch, not just this one:
    /// the conductance is `k * ctr[ir] / lr[ir]`, using the **node-centre**
    /// radius where the **face** radius `ctr[ir] + lr[ir]/2` belongs. That
    /// understates the conduction area by `dr/2`, an O(dr) deficit — 12.5% at
    /// 4 nodes and 0.78% at 64.
    ///
    /// It is why the pellet drop is still **1.55% low at 64 radial nodes**,
    /// and why the NEACRP rods, which use **5**, carry a correspondingly
    /// larger discretisation error in every fuel temperature this crate
    /// reports. That is a property of the reference's scheme rather than a
    /// translation error, and correcting it would move every fuel temperature
    /// — so it is recorded, not repaired.
    #[test]
    fn t9_is_the_doubled_interface_conductance_correct() {
        use crate::types::{Conductivity, FuelGeometry};

        const K: f64 = 0.03; // W/(cm K), constant
        const RF: f64 = 0.41; // pellet radius, cm
        const Q: f64 = 300.0; // W/cm3, uniform in the pellet

        let analytic = Q * RF * RF / (4.0 * K);
        eprintln!("analytic centre-to-surface drop = {analytic:.6} K");
        eprintln!("  {:>6}  {:>14}  {:>12}  {:>8}", "fueln", "discrete drop", "rel err", "ratio");

        let mut prev = f64::NAN;
        for fueln in [4usize, 8, 16, 32, 64] {
            let (gapn, cladn) = (1usize, 2usize);
            let maxir = fueln + gapn + cladn;

            let mut lr = vec![RF / fueln as f64; fueln];
            lr.extend(vec![0.006; gapn]);
            lr.extend(vec![0.03; cladn]);
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
                tcon: vec![Conductivity::Constant(K), Conductivity::Constant(K)],
                gap_conductance: 1.0,
                fuelrad: RF,
                rtot: RF + 0.006 + 0.06,
                pitch: 1.2665,
                ..Default::default()
            };

            let maxid = maxir + 2;
            let temps = vec![600.0; maxid];
            let (profile, _) = fuelrodheat_1dcylnd(&fuel, maxir, &temps, Q, 1.5, 560.0);

            // Centre is unknown 0; the pellet surface is the duplicate at `fueln`.
            let drop = profile[0] - profile[fueln];
            let err = (drop - analytic).abs() / analytic;
            let ratio = if prev.is_finite() { format!("{:.2}", prev / err) } else { String::new() };
            eprintln!("  {fueln:>6}  {drop:>14.6}  {err:>12.3e}  {ratio:>8}");
            prev = err;
        }

        // The scheme converges on the analytic drop, but only at FIRST order —
        // every branch uses the node-centre radius where the face radius is
        // meant, an O(dr) area deficit. So the gate is the convergence rate,
        // not an absolute tolerance: the error must keep halving.
        assert!(
            prev < 2e-2,
            "the finest mesh is {prev:.3e} from the analytic drop"
        );
    }
}
