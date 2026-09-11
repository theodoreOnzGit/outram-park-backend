//! **The reactor-physics lessons of the FHR ring-RPT `k_eff` hunt, as executable
//! guards.**
//!
//! Between 2026-09-10 and 2026-09-11 this crate's FHR ring-RPT pebble was chased
//! from "+1681 pcm against an OpenMC reference" to a located defect, through two
//! real bug fixes that each made the disagreement *larger* and four measured
//! criticality benchmarks. The write-ups are GitHub #177–#186 and
//! `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`; bead
//! `op-qhm1` and its children track the follow-up work.
//!
//! Prose is not a guard. Every lesson below that *can* be made to fail on its
//! own defect is a test here, each one named for the mistake it prevents rather
//! than for the function it calls. The ones that cannot be — "state a
//! hypothesis's sign before implementing it" (#182), "judge a fix on its own
//! oracle, not on whether `k` moved closer" (#181) — are process rules and stay
//! in the issues.
//!
//! # Conclusion the hunt reached
//!
//! The residual is **this code's self-shielded U-238 resonance absorption**,
//! ~11 % low, and **not** the reference deck. ICSBEP LEU-COMP-THERM-008 —
//! thermal, 97.5 % U-238, 1.03 cm UO₂ pellets that are 176 mean free paths
//! across at the 6.67 eV resonance — came out at `k = 1.02950 ± 0.00061`,
//! **+2950 ± 61 pcm** from a configuration measured critical, against the
//! +3200 pcm predicted beforehand from the pebble's own six factors.
//!
//! # What each test guards
//!
//! | test | lesson | the defect it would have caught |
//! |---|---|---|
//! | `slowing_down_solver_matches_the_closed_form_hydrogen_solution` | an oracle must be exact, not merely different | a wrong Volterra scheme or source treatment |
//! | `a_grid_coarser_than_the_scatter_window_is_rejected_not_silently_wrong` | #178 | a resonance-adaptive grid that cannot represent U-238's 0.0169-lethargy window |
//! | `monte_carlo_matches_the_deterministic_slowing_down_solution` | #178 | the energy treatment of self-shielded absorption |
//! | `self_shielded_absorption_is_not_the_dilute_limit` | #178 | "our `RI_∞` matches NJOY" offered as self-shielding verification |
//! | `six_factor_group_conversion_preserves_the_product_and_moves_the_split` | #183 | reading `p` too high **and** `ε` too low as two findings |
//! | `absorption_is_mt27_so_li6_shows_the_n_t_channel` | #184 | GH #169 — MT=27 compared against NJOY's MT=102 |
//! | `packing_fraction_is_not_domain_free` | #180 | 2.6 % excess heavy metal from a cube-vs-ball ratio |
//! | `rpt_inner_radius_respects_the_geometric_ceiling` | #185 | a fitted shell that does not fit inside its fuel zone |
//! | `benchmark_mechanism_coverage_is_declared_not_assumed` | #177 | three benchmarks agreeing while one mechanism goes untested |
//!
//! Tests that need reconstructed nuclear data resolve their tapes through
//! `reference_data::reference_endf` and **skip rather than fail** when the
//! repo-only `reference-data/endf/` is absent, matching
//! `tests/thermal_graphite_elastic.rs`.

use outram_mc_libs::physics::slowing_down::{solve_on_grid, SlowingDownGrid};

// ─────────────────────────────────────────────────────────────────────────────
// The oracle has to be exact before it can judge anything
// ─────────────────────────────────────────────────────────────────────────────

/// **The deterministic slowing-down solver reproduces the one case with a
/// closed-form answer, to 5 significant figures.**
///
/// # Methodology
///
/// For a pure-hydrogen medium (`A = 1`, so `α = 0` and the scattering kernel is
/// `1/E′` over the whole range below the collision) with **energy-independent**
/// cross sections, the slowing-down equation can be solved by Laplace
/// transform. With `c = Σ_s/Σ_t` the survival probability per collision, a unit
/// mono-energetic source at `u = 0` gives a collision density
///
/// ```text
/// F(u) = δ(u) + c·e^{−(1−c)u}
/// ```
///
/// and therefore an exact escape probability over a band `[0, U]` in lethargy of
///
/// ```text
/// p_escape = c · e^{−(1−c)U}
/// ```
///
/// This is the only configuration in the module with an answer that owes nothing
/// to the solver, so it is what establishes that the solver is an *oracle*
/// rather than a second opinion. It exercises the delta-source treatment, the
/// `F(0⁺)` boundary value, the separable-kernel running integral and the
/// trapezoidal Volterra step all at once.
///
/// # Results (2026-09-11)
///
/// At `c = 0.98`, `U = 6` and 60 000 lethargy steps the solver returns
/// `p_escape` matching `c·e^{−(1−c)U}` to better than **1e-5 relative**, and the
/// convergence is second order in `Δu`: halving the step quarters the error.
///
/// # Why the `F(0⁺)` term is called out
///
/// Seeding the continuous collision density at zero instead of
/// `Σ_i (Σ_s,i/Σ_t)(0)/(1−α_i)` halves the first trapezoid and costs an `O(Δu)`
/// error that *looks* like discretisation and does not vanish under refinement
/// in the way a reader would expect. It was a real defect in the first draft of
/// this solver, found by this test.
#[test]
fn slowing_down_solver_matches_the_closed_form_hydrogen_solution() {
    let c = 0.98_f64;
    let u_max = 6.0_f64;

    let solve = |n: usize| -> f64 {
        let lethargy: Vec<f64> = (0..=n).map(|k| u_max * k as f64 / n as f64).collect();
        let grid = SlowingDownGrid {
            scatter_frac: vec![vec![c; lethargy.len()]],
            absorb_frac: vec![vec![1.0 - c; lethargy.len()]],
            alpha: vec![0.0], // A = 1
            lethargy,
        };
        solve_on_grid(&grid).escaped
    };

    let exact = c * (-(1.0 - c) * u_max).exp();
    let coarse = solve(15_000);
    let fine = solve(60_000);

    let err_coarse = (coarse / exact - 1.0).abs();
    let err_fine = (fine / exact - 1.0).abs();
    println!(
        "hydrogen closed form: exact {exact:.8}, n=15k {coarse:.8} ({:+.2e}), \
         n=60k {fine:.8} ({:+.2e})",
        coarse / exact - 1.0,
        fine / exact - 1.0
    );

    assert!(
        err_fine < 1.0e-5,
        "solver disagrees with the closed-form hydrogen solution by {err_fine:.3e} \
         (got {fine}, exact {exact})"
    );
    // Second-order convergence: a 4× refinement must cut the error by much more
    // than 4×. A first-order error — the `F(0⁺)` defect's signature — would only
    // manage ~4×.
    assert!(
        err_coarse > 6.0 * err_fine || err_coarse < 1.0e-7,
        "error fell only {:.1}× under a 4× refinement ({err_coarse:.3e} → \
         {err_fine:.3e}); that is first-order, and the usual cause is a wrong \
         boundary value at u = 0",
        err_coarse / err_fine.max(f64::MIN_POSITIVE)
    );
}

/// **A lethargy grid too coarse to represent a nuclide's scattering window is
/// rejected, not silently answered.**
///
/// # The defect this guards
///
/// The scatter-in integral for a nuclide spans `ε = ln(1/α)` in lethargy, which
/// for **U-238 is 0.0169** — narrower than the gaps a *resonance-adaptive*
/// energy grid leaves between resonances. Handed such a grid, the first draft of
/// the solver did not diverge or warn: it returned an answer that was far too
/// **absorbing**, reporting a resonance escape probability of `0.00000` where the
/// truth was `0.176`. That is the worst possible failure mode, because the
/// number is plausible in the direction one is already suspicious of, and it
/// would have "confirmed" a Monte-Carlo defect that does not exist.
///
/// # Methodology and result (2026-09-11)
///
/// A synthetic two-component medium with a heavy component (`α = 0.983`, U-238's
/// value) is solved on a grid whose step exceeds the quarter-window backstop.
/// `solve_on_grid` must panic naming the scatter window. `slowing_down_grid`
/// exists precisely so real callers never reach that state — it subdivides to
/// 20 steps per window — and `monte_carlo_matches_the_deterministic_slowing_down_solution`
/// demonstrates the resulting grid convergence on real data.
#[test]
#[should_panic(expected = "scatter window")]
fn a_grid_coarser_than_the_scatter_window_is_rejected_not_silently_wrong() {
    let alpha_u238 = 0.983_f64;
    let eps = -alpha_u238.ln(); // ≈ 0.0171
    let n = 200;
    // One step per window: 4× coarser than the backstop allows.
    let lethargy: Vec<f64> = (0..=n).map(|k| eps * k as f64).collect();
    let grid = SlowingDownGrid {
        scatter_frac: vec![vec![0.5; lethargy.len()], vec![0.45; lethargy.len()]],
        absorb_frac: vec![vec![0.0; lethargy.len()], vec![0.05; lethargy.len()]],
        alpha: vec![0.0, alpha_u238],
        lethargy,
    };
    let _ = solve_on_grid(&grid);
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson #183 — the six-factor convention trap
// ─────────────────────────────────────────────────────────────────────────────

/// **Converting the six factors between group structures moves the split
/// between `ε` and `p` and leaves their product alone — so `ε` and `p` are not
/// separately comparable across codes.**
///
/// # The mistake this guards
///
/// This crate reports a **three**-group decomposition (thermal cutoff 0.625 eV);
/// the OpenMC reference deck reports **two** groups at the same cutoff. Compared
/// naively they disagree wildly. Compared like-for-like the split is `p +8.5 %`
/// (0.5253 vs 0.4842) and `ε −5.1 %` (1.4280 vs 1.5043) — **the same statement
/// twice**, because the conversion trades one against the other.
///
/// An early reading of the ring-RPT study recorded "`p` too high *and* `ε` too
/// low" as two independent findings and spent effort hunting a MeV-range
/// mechanism for the `ε` half. There was no second finding and MeV physics was
/// never implicated (bead `op-mzvp.2.12`, 2026-09-11).
///
/// # Methodology
///
/// Build a `SixFactors` from group-resolved rates with **zero leakage** (the
/// pebble's reflective boundary), so `P_FNL = P_TNL = 1` and both conventions
/// must telescope to the same `P_total / A_total`. Then assert the product is
/// invariant to machine precision while the individual factors are not.
///
/// # Result (2026-09-11)
///
/// The two products agree to `< 1e-12` relative; `p` moves by 9.4 % and `ε` by
/// 8.6 % between conventions on these rates.
#[test]
fn six_factor_group_conversion_preserves_the_product_and_moves_the_split() {
    use outram_mc_libs::physics::reactor_physics::{Estimate, SixFactors};

    // Rates indexed [thermal, resonance, fast]; representative of a graphite
    // pebble, with a small but non-zero fast absorption — which is exactly the
    // term the two conventions disagree about.
    let a = [
        Estimate::exact(0.7400),
        Estimate::exact(0.1900),
        Estimate::exact(0.0700),
    ];
    let p = [
        Estimate::exact(1.2600),
        Estimate::exact(0.0000),
        Estimate::exact(0.1400),
    ];
    let l = [
        Estimate::exact(0.0),
        Estimate::exact(0.0),
        Estimate::exact(0.0),
    ];
    let a_fuel_thermal = Estimate::exact(0.5200);

    let a_tot: f64 = a.iter().map(|x| x.mean).sum();
    let p_tot: f64 = p.iter().map(|x| x.mean).sum();

    // Three-group form, assembled the way `assemble_six_factors` does it.
    let s_t = a[0].mean + l[0].mean;
    let s_tr = s_t + a[1].mean + l[1].mean;
    let s_trf = s_tr + l[2].mean;
    let eta3 = p[0].mean / a_fuel_thermal.mean;
    let f3 = a_fuel_thermal.mean / a[0].mean;
    let p3 = s_t / s_tr;
    let eps3 = (p_tot / p[0].mean) * ((a_tot - a[2].mean) / a_tot);
    let p_tnl = a[0].mean / s_t;
    let p_fnl = s_tr / s_trf;

    let sf = SixFactors {
        eta: Estimate::exact(eta3),
        f: Estimate::exact(f3),
        p: Estimate::exact(p3),
        epsilon: Estimate::exact(eps3),
        p_fnl: Estimate::exact(p_fnl),
        p_tnl: Estimate::exact(p_tnl),
        k_from_factors: Estimate::exact(eta3 * f3 * p3 * eps3 * p_fnl * p_tnl),
        absorption_by_group: a,
        production_by_group: p,
        leakage_by_group: l,
        thermal_absorption_fuel: a_fuel_thermal,
        group_bounds_ev: (0.625, 1.0e5),
    };

    let (eta2, f2, p2, eps2) = sf.two_group_openmc_convention();
    let prod3 = eta3 * f3 * p3 * eps3 * p_fnl * p_tnl;
    let prod2 = eta2 * f2 * p2 * eps2;

    println!(
        "3-group: eta {eta3:.5} f {f3:.5} p {p3:.5} eps {eps3:.5} (product {prod3:.8})\n\
         2-group: eta {eta2:.5} f {f2:.5} p {p2:.5} eps {eps2:.5} (product {prod2:.8})"
    );

    assert!(
        (prod2 / prod3 - 1.0).abs() < 1.0e-12,
        "the product is NOT convention-invariant: {prod3} vs {prod2}. Either the \
         conversion or the assembly is wrong — with zero leakage both must \
         telescope to P_total/A_total = {}",
        p_tot / a_tot
    );
    assert!(
        (prod3 / (p_tot / a_tot) - 1.0).abs() < 1.0e-12,
        "the three-group product does not telescope to P_total/A_total"
    );
    // …and the individual factors DO move, which is the whole point: a reader
    // comparing `p` or `ε` across the two conventions is comparing nothing.
    assert!(
        (p2 / p3 - 1.0).abs() > 0.02,
        "p barely moved between conventions ({p3} → {p2}); this test's rates no \
         longer demonstrate the trap it exists to guard"
    );
    assert!(
        (eps2 / eps3 - 1.0).abs() > 0.02,
        "epsilon barely moved between conventions ({eps3} → {eps2})"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson #180 — a volume fraction needs its domain
// ─────────────────────────────────────────────────────────────────────────────

/// **The packing fraction over the packing cube is NOT the packing fraction over
/// an inscribed ball, and the difference is worth ~1000 pcm.**
///
/// # The defect this guards (bead `op-8l2e`)
///
/// `pack_spheres_crp` reports its packing fraction over the packing **cube**,
/// but sphere *centres* are confined to `half − r_particle`, so the cube's
/// interior is denser than nominal and its outer shell emptier. An inscribed
/// ball samples only the dense interior and inherits the inflation: the
/// explicit-TRISO pebble's `r < 1.9 cm` fuel zone realised **0.3078** against the
/// deck's **0.3000** — **2.6 % more heavy metal than the model it was being
/// compared against**.
///
/// That one under-specified ratio produced *two* spurious physics conclusions
/// before it was found: a −1079 pcm "ring-RPT method error" and the +1130 pcm
/// that briefly replaced it.
///
/// # Methodology and result (2026-09-11)
///
/// Pack a cube at a nominal fraction, then measure the realised fraction over
/// inscribed balls of several radii. The ball fraction must exceed the cube
/// fraction, and must do so *more* as the ball shrinks toward the dense core.
/// The test is deliberately written to fail if someone "fixes" the discrepancy
/// by making the two equal — they are different quantities and both are correct.
#[test]
fn packing_fraction_is_not_domain_free() {
    use outram_mc_libs::pebble_beds::crp_packing::pack_spheres_crp;
    use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;

    const HALF: f64 = 1.0;
    const R_PARTICLE: f64 = 0.045;
    const NOMINAL_PF: f64 = 0.30;

    let spheres = pack_spheres_crp(R_PARTICLE, HALF, NOMINAL_PF, 0xC0FFEE).expect("pack");
    let packed = PackedSpheres::from_spheres(spheres, HALF, R_PARTICLE);
    let cube_pf = packed.packing_fraction();

    let mut previous_inflation = f64::NEG_INFINITY;
    for (i, frac) in [1.0_f64, 0.8, 0.6].iter().enumerate() {
        let r_ball = HALF * frac;
        let ball_pf = packed.volume_fraction_in_ball(r_ball, 400_000, 0x5EED + i as u64);
        let inflation = ball_pf / cube_pf - 1.0;
        println!(
            "cube pf {cube_pf:.5}; ball r = {r_ball:.2} pf {ball_pf:.5} \
             ({:+.2} % of the cube value)",
            100.0 * inflation
        );
        assert!(
            ball_pf > cube_pf,
            "an inscribed ball (r = {r_ball}) is not denser than the packing cube \
             ({ball_pf} vs {cube_pf}); either the centre-exclusion shell is gone or \
             `volume_fraction_in_ball` has stopped measuring what its name says"
        );
        assert!(
            inflation > previous_inflation,
            "the inflation did not grow as the ball shrank ({:+.4} then {:+.4}); the \
             cube's dense interior is the whole mechanism",
            previous_inflation,
            inflation
        );
        previous_inflation = inflation;
    }
    assert!(
        previous_inflation > 0.01,
        "the smallest ball is only {:+.3} % denser than the cube — this test no \
         longer demonstrates a difference worth ~1000 pcm",
        100.0 * previous_inflation
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson #185 — an RPT radius is a fitted quantity with a hard geometric bound
// ─────────────────────────────────────────────────────────────────────────────

/// **A ring-RPT fuel shell that would not fit inside its fuel zone is refused,
/// not silently clipped.**
///
/// # Why this matters (bead `op-mzvp.2.8`)
///
/// The RPT inner radius is the single knob the transformation turns, and it is
/// **fitted** — to one composition, one temperature and one packing fraction.
/// Anyone refitting it (`--search-rpt-radius` bisects over `r_inner ∈ [1.10,
/// 1.64]` cm) is working against a hard geometric ceiling:
///
/// ```text
/// r_outer³ = r_inner³ + pf · R_fuel_zone³   ≤   R_fuel_zone³
///   ⟹  r_inner ≤ R_fuel_zone · (1 − pf)^{1/3}  ≈ 1.687 cm at pf = 0.30
/// ```
///
/// A search that wandered past it would otherwise produce a shell thicker than
/// the region containing it — a geometry that still *runs* and returns a
/// perfectly plausible `k`.
///
/// # Result (2026-09-11)
///
/// The reference radius 1.493359375 cm gives `r_outer = 1.75305 cm`, inside the
/// 1.9 cm fuel zone; the ceiling is 1.68713 cm; 1.70 cm panics.
#[test]
fn rpt_inner_radius_respects_the_geometric_ceiling() {
    use outram_mc_libs::pebble_beds::fhr_pebble::rpt_fuel_outer_radius;

    const R_FUEL_ZONE: f64 = 1.9;
    const PF: f64 = 0.30;
    let ceiling = R_FUEL_ZONE * (1.0 - PF).cbrt();

    let r_outer = rpt_fuel_outer_radius(1.493_359_375, R_FUEL_ZONE, PF);
    println!("reference r_inner 1.493359375 → r_outer {r_outer:.5} cm, ceiling {ceiling:.5} cm");
    assert!(
        r_outer < R_FUEL_ZONE,
        "the reference shell does not fit inside the fuel zone"
    );
    assert!(
        (ceiling - 1.687_018).abs() < 1.0e-5,
        "the geometric ceiling moved: {ceiling}"
    );

    // Just inside the ceiling is fine; just outside must be refused.
    let _ = rpt_fuel_outer_radius(ceiling - 1.0e-6, R_FUEL_ZONE, PF);
    let over =
        std::panic::catch_unwind(|| rpt_fuel_outer_radius(ceiling + 1.0e-3, R_FUEL_ZONE, PF));
    assert!(
        over.is_err(),
        "an inner radius past the geometric ceiling was accepted — the homogenised \
         shell would be thicker than the fuel zone containing it"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson #177 — coverage is a mechanism triple, not a spectrum label
// ─────────────────────────────────────────────────────────────────────────────

/// One V&V criticality benchmark, described by the **mechanism** it exercises
/// rather than by its name.
struct BenchmarkCoverage {
    name: &'static str,
    /// Is the spectrum moderated, so neutrons are slowed *through* the U-238
    /// resolved resonances?
    moderated: bool,
    /// U-238 as a fraction of the heavy metal.
    u238_share: f64,
    /// Is the fuel lumped, so there is a resonance flux depression to get wrong?
    lumped: bool,
    /// Measured result, in pcm from the benchmark's 1.0000.
    result_pcm: f64,
    /// 1σ on that, in pcm.
    sigma_pcm: f64,
}

/// **The criticality-benchmark suite covers self-shielded U-238 resonance
/// absorption, and says so in code.**
///
/// # The mistake this guards (#177)
///
/// Read as labels, three benchmarks looked like broad coverage — two spectra,
/// U-238 from 5 % to 99 % of the heavy metal, all three reproduced inside 1σ of
/// a measured critical experiment. Read as *mechanisms*, they all missed the
/// same one. Self-shielded resonance absorption needs a **moderator** and a
/// large **U-238 share** and **lumped** fuel, all at once, and no two of the
/// three implies the third:
///
/// - Godiva and Jemima have the U-238 but no moderator — nothing is slowed
///   through the resonances;
/// - HEU-SOL-THERM-009 has the moderator but 5 % U-238 in a *homogeneous*
///   solution, so its resonance escape is ≈ 0.95 and essentially unshielded.
///
/// LEU-COMP-THERM-008 is the first case with all three, and the first to fail:
/// **+2950 ± 61 pcm**.
///
/// # What this test asserts
///
/// That at least one benchmark in the manifest exercises the full triple, and
/// that its result is *recorded* — so the suite cannot quietly lose the only
/// case that reaches the defect, and a new benchmark cannot be added without
/// declaring which mechanism it covers. It deliberately does **not** assert that
/// LEU-COMP-THERM-008 passes: it does not, that is the finding, and a test that
/// demanded otherwise would have to be weakened the moment the defect is fixed.
#[test]
fn benchmark_mechanism_coverage_is_declared_not_assumed() {
    // Measured on this crate's HIGH/ENDF path, 2026-09-11. See
    // `examples/{godiva_keff_endf_local,jemima_keff,hst009_keff,lct008_keff}.rs`.
    let suite = [
        BenchmarkCoverage {
            name: "HEU-MET-FAST-001 (Godiva)",
            moderated: false,
            u238_share: 0.05,
            lumped: false,
            result_pcm: 57.0,
            sigma_pcm: 173.0,
        },
        BenchmarkCoverage {
            name: "IEU-MET-FAST-002 (Jemima)",
            moderated: false,
            u238_share: 0.83,
            lumped: false,
            result_pcm: 6.0,
            sigma_pcm: 173.0,
        },
        BenchmarkCoverage {
            name: "HEU-SOL-THERM-009 case 1",
            moderated: true,
            u238_share: 0.05,
            lumped: false,
            result_pcm: -18.0,
            sigma_pcm: 171.0,
        },
        BenchmarkCoverage {
            name: "LEU-COMP-THERM-008",
            moderated: true,
            u238_share: 0.975,
            lumped: true,
            result_pcm: 2950.0,
            sigma_pcm: 61.0,
        },
    ];

    let self_shielding_cases: Vec<&BenchmarkCoverage> = suite
        .iter()
        .filter(|b| b.moderated && b.u238_share > 0.5 && b.lumped)
        .collect();
    assert!(
        !self_shielding_cases.is_empty(),
        "no benchmark in the suite exercises self-shielded U-238 resonance \
         absorption (moderated AND U-238-dominated AND lumped). Three benchmarks \
         agreed for two days while that mechanism went untested — see GitHub #177."
    );

    for b in &suite {
        println!(
            "{:<28} moderated {:<5} U-238 {:>5.1} % lumped {:<5} → {:+6.0} ± {:.0} pcm",
            b.name,
            b.moderated,
            100.0 * b.u238_share,
            b.lumped,
            b.result_pcm,
            b.sigma_pcm
        );
        assert!(b.sigma_pcm > 0.0, "{} has no recorded uncertainty", b.name);
    }

    // The three cases that do NOT reach the mechanism must all still agree with
    // their experiments — they are what keeps the defect localised. If one of
    // these starts failing, the defect is no longer specific to self-shielding
    // and the whole interpretation has to be revisited.
    for b in suite
        .iter()
        .filter(|b| !(b.moderated && b.u238_share > 0.5 && b.lumped))
    {
        assert!(
            b.result_pcm.abs() < 2.0 * b.sigma_pcm,
            "{} is recorded at {:+.0} ± {:.0} pcm — a benchmark that does NOT reach \
             self-shielded resonance absorption has started disagreeing, so the \
             localisation in GitHub #186 no longer holds",
            b.name,
            b.result_pcm,
            b.sigma_pcm
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data-gated tests: reconstructed cross sections
// ─────────────────────────────────────────────────────────────────────────────

/// `Some(nuclide)` when the tape is present, else `None` after printing a skip
/// note — the repo-only `reference-data/endf/` is not packaged, and a data-gated
/// test passes rather than fails when it is absent. Same contract as
/// `tests/thermal_graphite_elastic.rs`.
fn nuclide_or_skip(
    name: &str,
    file: &str,
    temp_k: f64,
) -> Option<outram_mc_libs::prelude::Nuclide> {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::prelude::Nuclide;

    let Some(path) = reference_endf(file) else {
        println!(
            "[{name}] SKIP: {file} not found in reference-data/endf/ (set OUTRAM_PARK_ENDF_DIR)"
        );
        return None;
    };
    match Nuclide::from_endf_file(&path, name, temp_k, 1.0e-3) {
        Ok(n) => Some(n),
        Err(e) => {
            println!("[{name}] SKIP: reconstruction failed: {e}");
            None
        }
    }
}

/// **`MicroXS::absorption` is ENDF MT=27 — fission plus *all* of MT=101 — and
/// Li-6 is the nuclide that makes the difference impossible to miss.**
///
/// # The defect this guards (GitHub #169)
///
/// NJOY's `MT=102` is **radiative capture alone**. This crate's `absorption` is
/// OpenMC's **MT=27**: fission plus every disappearance channel — `(n,p)`,
/// `(n,α)`, `(n,t)` and the rest. For a heavy actinide the two nearly coincide
/// and the distinction is invisible. For **Li-6** it is a factor of ~24 000:
/// `(n,t)α` (MT=105) is ~938 b at 0.0253 eV against MT=102's ~0.0385 b.
///
/// A "capture" comparison that does not name its MT is meaningless, and this one
/// was a real bug: Li-6 absorption was once 0.04 b instead of 938 b, which in a
/// FLiBe-cooled pebble is most of the coolant's poison simply missing.
///
/// # Methodology and result (2026-09-11)
///
/// Reconstruct Li-6 from the ENDF/B-VIII.0 tape at 293.6 K and read
/// `absorption − fission` at 0.0253 eV. It must be within a few percent of the
/// evaluated thermal `(n,t)` cross section — hundreds of barns — and emphatically
/// not the sub-barn radiative-capture value. Measured: **936.6 b**.
///
/// The bound is deliberately loose (100 b–2000 b): the point is the *channel*,
/// not the digit, and a tight bound here would just become a maintenance tax
/// when the evaluation is revised.
#[test]
fn absorption_is_mt27_so_li6_shows_the_n_t_channel() {
    const TEMP: f64 = 293.6;
    let Some(li6) = nuclide_or_skip("Li6", "n-003_Li_006-ENDF8.0.endf", TEMP) else {
        return;
    };
    let x = li6.xs_at_energy(0.0253, TEMP);
    let absorption_mt27 = x.absorption - x.fission;
    println!(
        "Li-6 at 0.0253 eV: total {:.3} b, MT=27 absorption {:.3} b, elastic {:.3} b",
        x.total, absorption_mt27, x.elastic
    );
    assert!(
        (100.0..2000.0).contains(&absorption_mt27),
        "Li-6 MT=27 absorption at 0.0253 eV is {absorption_mt27} b. The (n,t)α channel \
         is ~938 b there; a sub-barn answer means `absorption` has silently become \
         MT=102 radiative capture and every Li-6-bearing coolant has lost its poison \
         (GitHub #169)."
    );
    // And it must dominate the total, which is the physical statement: thermal
    // neutrons in Li-6 are absorbed, not scattered.
    assert!(
        absorption_mt27 > 0.9 * x.total,
        "Li-6's thermal total is {} b but only {absorption_mt27} b of it is absorption",
        x.total
    );
}

/// U-238 + C-12 at 293.6 K and their union energy grid, reconstructed once and
/// shared: the two heavy tests below need the same data, and RECONR + BROADR on
/// U-238 is ~20 s that there is no reason to pay twice.
///
/// `None` when the repo-only tapes are absent — the callers then skip.
fn u238_c12_medium() -> Option<&'static (Vec<outram_mc_libs::prelude::Nuclide>, Vec<f64>)> {
    use std::sync::OnceLock;
    static MEDIUM: OnceLock<Option<(Vec<outram_mc_libs::prelude::Nuclide>, Vec<f64>)>> =
        OnceLock::new();
    MEDIUM
        .get_or_init(|| {
            let c12 = nuclide_or_skip("C12", "n-006_C_012-ENDF8.0.endf", MEDIUM_TEMP_K)?;
            let u238 = nuclide_or_skip("U238", "n-092_U_238.endf", MEDIUM_TEMP_K)?;
            let mut grid = u238.native_energy_grid(BAND.e_bot * 0.9, BAND.e_top * 1.1);
            grid.extend_from_slice(&c12.native_energy_grid(BAND.e_bot * 0.9, BAND.e_top * 1.1));
            grid.sort_by(|a, b| a.partial_cmp(b).expect("finite grid"));
            grid.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-12 * b.abs());
            Some((vec![c12, u238], grid))
        })
        .as_ref()
}

/// Room temperature, matching LEU-COMP-THERM-008.
const MEDIUM_TEMP_K: f64 = 293.6;
/// C-12 potential scattering \[b\], used to express the dilution as a
/// background cross section per U-238 atom.
const SIGMA_P_C12: f64 = 4.7392;
/// Source at 10 keV — inside U-238's resolved resonance range and below its
/// 44.9 keV first inelastic level, so elastic scattering and absorption are the
/// only open channels — scored down to 1 eV.
const BAND: outram_mc_libs::physics::slowing_down::SlowingDownBand =
    outram_mc_libs::physics::slowing_down::SlowingDownBand {
        e_top: 10.0e3,
        e_bot: 1.0,
    };

/// The mixture at a given background cross section `σ_b = N_C σ_p,C / N_8`.
fn mixture_at(sigma_b: f64) -> Vec<outram_mc_libs::physics::slowing_down::MixComponent> {
    use outram_mc_libs::physics::slowing_down::MixComponent;
    vec![
        MixComponent {
            nuclide_idx: 0,
            atom_density: sigma_b / SIGMA_P_C12 * 1.0e-3,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: 1.0e-3,
        },
    ]
}

/// **The Monte-Carlo collision kernel reproduces the exact slowing-down solution
/// in an infinite homogeneous medium, at every dilution from nearly-dilute to
/// strongly self-shielded.**
///
/// # What this is for (GitHub #178)
///
/// The ring-RPT hunt ran out of eigenvalues. It had established that `σ_γ(E)` is
/// correct (±0.04 % pointwise vs NJOY's PENDF, 0.12 % in resonance shape,
/// +0.00 % in infinitely-dilute resonance integral) and that the tracking method
/// is not implicated, and yet the **self-shielded** absorption is ~11 % low on
/// two independent systems. No `k` can say *where*, because `k` is one number.
///
/// An infinite homogeneous medium is the one geometry whose answer can be
/// computed exactly. Solving the slowing-down equation directly on a fine grid,
/// with **this crate's own reconstructed cross sections**, and running the
/// crate's own collision kernel on the same medium, compares two solutions of
/// the same equation with the same data. Any difference is the transport.
///
/// It splits the remaining search: **agreement** means the defect is *spatial*
/// — how a lump's interior flux is built, which is already the pattern the
/// benchmarks show, since HEU-SOL-THERM-009 is the one *homogeneous* thermal
/// case and the one that comes out right. **Disagreement** would mean the defect
/// is in the energy treatment.
///
/// # Methodology
///
/// U-238 + C-12 at 293.6 K, mono-energetic source at 10 keV — inside the
/// resolved resonance range and below U-238's 44.9 keV first inelastic level, so
/// elastic scattering and absorption are the only open channels — scored down to
/// 1 eV. The background cross section `σ_b = N_C σ_p,C / N_8` is scanned over
/// three decades. The Monte Carlo runs
/// [`ScatterKernel::IsotropicCmAtRest`], which is the deterministic model's own
/// kernel, so a disagreement is a defect and not a modelling difference.
///
/// Grid convergence is demonstrated rather than assumed: the deterministic solve
/// is repeated on a lethargy-bisected grid and must move by less than the
/// tolerance being claimed.
///
/// # Results (2026-09-11, ENDF/B-VIII.0, 200 000 histories per point)
///
/// ```text
/// sigma_b [b]   deterministic p_esc      Monte Carlo          diff      z
///        30           0.02940        0.02880 ± 0.00037      −2.05 %   −1.61
///       100           0.17579        0.17730 ± 0.00085      +0.86 %   +1.77
///       300           0.38763        0.38904 ± 0.00109      +0.36 %   +1.29
///      1000           0.60543        0.60582 ± 0.00109      +0.06 %   +0.35
///     10000           0.88390        0.88402 ± 0.00072      +0.01 %   +0.17
///
/// grid convergence at sigma_b = 100 b: 0.17579 → 0.17576 under bisection (−0.014 %)
/// ```
///
/// **Every dilution agrees within statistics, worst 1.8σ across five points.**
/// And this is not a comparison in the easy regime: at `σ_b = 30 b` the
/// effective resonance integral has collapsed **11.7×** from its near-dilute
/// value (see `self_shielded_absorption_is_not_the_dilute_limit`), so the
/// self-shielding being reproduced is as strong as the FHR kernel's.
///
/// A separate run of `examples/slowing_down_oracle` at 400 000 histories adds
/// the other two kernels: `AnisotropicCmAtRest` and `Production` — the latter
/// being literally `transport_history`'s own branch, S(α,β) and free-gas target
/// motion included — also land within 0.5 % of the deterministic solution for
/// `σ_b ≥ 100 b`. So neither elastic anisotropy nor the free-gas kernel is
/// hiding an error there either.
///
/// **Conclusion: the energy treatment is not the defect.** The ring-RPT residual
/// (GitHub #186) is therefore *spatial* — what happens to the flux inside an
/// optically thick lump — which is also the pattern the benchmark table shows,
/// since HEU-SOL-THERM-009 is the one homogeneous thermal case and the one that
/// comes out right.
#[test]
fn monte_carlo_matches_the_deterministic_slowing_down_solution() {
    use outram_mc_libs::physics::slowing_down::{
        refine_lethargy_grid, solve_deterministic, InfiniteMediumMc, ScatterKernel,
    };

    let Some((nuclides, grid)) = u238_c12_medium() else {
        return;
    };
    let (band, temp) = (BAND, MEDIUM_TEMP_K);
    println!("union energy grid: {} points", grid.len());

    const HISTORIES: usize = 200_000;
    let mut worst = (0.0_f64, 0.0_f64); // (|z|, sigma_b)

    for &sigma_b in &[30.0_f64, 100.0, 300.0, 1000.0, 10_000.0] {
        let mix = mixture_at(sigma_b);

        let det = solve_deterministic(nuclides, &mix, band, temp, grid);
        let mc = InfiniteMediumMc {
            histories: HISTORIES,
            seed: 0xABCD_0001,
            kernel: ScatterKernel::IsotropicCmAtRest,
            max_collisions: 200_000,
        };
        let run = mc.run(nuclides, &mix, band, temp);
        let se = mc.stderr_of(run.escaped);
        let z = (run.escaped - det.escaped) / se;

        println!(
            "sigma_b {sigma_b:>7.0} b:  deterministic p_esc {:.5}   MC {:.5} ± {:.5}   \
             {:+.2} %  ({z:+.2} sigma)",
            det.escaped,
            run.escaped,
            se,
            100.0 * (run.escaped / det.escaped - 1.0),
        );

        // Grid convergence, only where it is cheap enough to matter most.
        if (sigma_b - 100.0).abs() < 1.0 {
            let fine = refine_lethargy_grid(grid);
            let det_fine = solve_deterministic(nuclides, &mix, band, temp, &fine);
            let drift = (det_fine.escaped / det.escaped - 1.0).abs();
            println!(
                "  grid convergence at sigma_b = 100 b: {:.5} → {:.5} ({:+.3} %)",
                det.escaped,
                det_fine.escaped,
                100.0 * (det_fine.escaped / det.escaped - 1.0)
            );
            assert!(
                drift < 0.01,
                "the deterministic answer moved {:.2} % under a lethargy-bisected grid, \
                 so it is not converged and cannot serve as an oracle",
                100.0 * drift
            );
        }

        if z.abs() > worst.0 {
            worst = (z.abs(), sigma_b);
        }
    }

    // 5 dilutions, so ~3.5σ is the right family-wise bound for a two-sided test
    // at a few percent. A real energy-treatment defect of the size the ring-RPT
    // study is chasing (11 % in the effective resonance integral) would show as
    // tens of sigma at the shielded end, not four.
    assert!(
        worst.0 < 3.5,
        "Monte Carlo and the exact slowing-down solution disagree by {:.1} sigma at \
         sigma_b = {} b. That is the energy treatment, and the deterministic flux \
         names the energies — see GitHub #178.",
        worst.0,
        worst.1
    );
}

/// **Self-shielded absorption is not the dilute limit, and the gap is enormous —
/// so a check against `RI_∞` verifies nothing about a lump.**
///
/// # The mistake this guards (GitHub #178)
///
/// `RI_∞ = ∫σ_γ dE/E` is a functional of `σ_γ` **alone**, and this crate's
/// matches NJOY's own PENDF and the published value to **+0.00 %**. The
/// *effective* resonance integral in a lump is a functional of the coupled
/// problem — `σ_γ` and the flux it depresses — and ours is **11 % low**. Those
/// two facts sat side by side for two days because a dilute-limit check passes
/// exactly when the interesting physics is switched off.
///
/// # Methodology and results (2026-09-11)
///
/// The same infinite U-238 + C-12 medium as
/// `monte_carlo_matches_the_deterministic_slowing_down_solution`, read as an
/// effective resonance integral through the narrow-resonance relation
/// `p = exp(−I_eff/(ξ·σ_b))` with `ξ_C = 0.15790`. As `σ_b` falls from
/// 10 000 b to 30 b the effective integral collapses by more than an order of
/// magnitude — while `σ_γ(E)`, and therefore `RI_∞`, has not changed at all.
///
/// The test asserts the collapse is monotone and large. It is not a check
/// against an external number: it is a check that this crate's self-shielding
/// *exists and is strong*, so that nobody can again offer a dilute-limit
/// agreement as evidence about a lump.
#[test]
fn self_shielded_absorption_is_not_the_dilute_limit() {
    use outram_mc_libs::physics::slowing_down::solve_deterministic;

    const XI_C12: f64 = 0.157_90; // mean log energy decrement, carbon
    let Some((nuclides, grid)) = u238_c12_medium() else {
        return;
    };
    let (band, temp) = (BAND, MEDIUM_TEMP_K);

    let mut previous = f64::INFINITY;
    let mut dilute = 0.0_f64;
    let mut shielded = 0.0_f64;
    for &sigma_b in &[10_000.0_f64, 1000.0, 300.0, 100.0, 30.0] {
        let mix = mixture_at(sigma_b);
        let det = solve_deterministic(nuclides, &mix, band, temp, grid);
        // p = exp(−I_eff / (ξ σ_b))  ⟹  I_eff = −ξ σ_b ln p
        let i_eff = -XI_C12 * sigma_b * det.escaped.max(1.0e-12).ln();
        println!(
            "sigma_b {sigma_b:>7.0} b:  p_esc {:.5}   I_eff {i_eff:>7.2} b",
            det.escaped
        );

        assert!(
            i_eff < previous,
            "the effective resonance integral did not fall as the medium became more \
             self-shielded ({previous:.2} b then {i_eff:.2} b) — self-shielding has \
             stopped working, and every dilute-limit check in this crate would still pass"
        );
        previous = i_eff;
        if sigma_b > 5000.0 {
            dilute = i_eff;
        }
        shielded = i_eff;
    }

    let collapse = dilute / shielded;
    println!(
        "effective resonance integral collapses {collapse:.1}x from sigma_b = 10 000 b to 30 b"
    );
    assert!(
        collapse > 5.0,
        "the effective resonance integral only fell {collapse:.1}x across three decades \
         of dilution. Self-shielding is the mechanism the ring-RPT residual was \
         narrowed to; if it is this weak here, the model is not exercising it and the \
         test guards nothing."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson #184 — a specification deserves a second, independent source
// ─────────────────────────────────────────────────────────────────────────────

/// **The LEU-COMP-THERM-008 core this crate runs is the same core the original
/// MCNP deck describes — all 11 025 lattice positions, checked pin by pin
/// against a completely different way of writing the geometry down.**
///
/// # Why a second source (GitHub #184)
///
/// `examples/lct008_keff.rs` parses the committed `mit-crpg/benchmarks` OpenMC
/// XML at run time rather than transcribing it, because 22 distinct 15 × 15 pin
/// lattices inside a 7 × 7 core lattice is far past what a hand transcription
/// can be trusted with. That removes *transcription* error. It does not remove
/// the possibility that the XML itself is not the benchmark — and a wrong core
/// shape would return a perfectly plausible `k`, which is the failure mode this
/// whole study exists to avoid.
///
/// The same repository ships the **original MCNP input** for the same case, and
/// it writes the geometry a completely different way: not as lattice maps at
/// all, but as an inner core plus six explicit `px`/`py` rectangles — an
/// "axially uniform quadrant" with reflective faces on `x = 0` and `y = 0`. Two
/// independent expressions of one benchmark model, so they can be made to check
/// each other.
///
/// # Methodology
///
/// The OpenMC side is **parsed** from the committed `geometry.xml`. The MCNP
/// side is **transcribed by hand** — six surface coordinates and seven cell
/// regions, which is small enough for a reviewer to check against the committed
/// `mcnp_case-1.input` line by line, exactly the size threshold #184 argues for.
///
/// Every boundary in the MCNP deck lands on a pin-cell edge
/// (`17.1754 / 1.63576 = 10.5`, `36.8046 / 1.63576 = 22.5`, and so on), so the
/// comparison is exact in integer pin indices with no tolerance at all. The pin
/// grids are shown to coincide first: the OpenMC core lattice's centre tile is
/// centred on the origin and each assembly's centre pin lands there too, which
/// is the same grid the MCNP pin lattice defines.
///
/// # Result (2026-09-11)
///
/// **4961 fuel pins on each side, 0 mismatches in 11 025 positions.**
///
/// Note what this does *not* claim: that the benchmark model is right, or that
/// `k = 1.0000` for it. It claims that this crate is running the model the
/// benchmark repository ships, in two independent renderings — which is the part
/// that was in the crate's power to get wrong.
#[test]
fn the_lct008_core_matches_the_original_mcnp_deck_pin_for_pin() {
    let spec_dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/verification_and_validation/icsbep/leu-comp-therm-008"
    );
    let xml = std::fs::read_to_string(format!("{spec_dir}/geometry.xml"))
        .expect("committed LEU-COMP-THERM-008 geometry.xml");
    let xml = strip_xml_comments(&xml);

    // --- the OpenMC side, parsed --------------------------------------------
    let lattices = read_lattices(&xml);
    let wrappers = read_lattice_wrappers(&xml);
    let core = lattices.get(&99).expect("core lattice 99");
    assert_eq!((core.0, core.1), (7, 7), "core lattice is no longer 7 × 7");

    // OpenMC writes lattice rows top-down (row 0 = max y); this crate's flat
    // index runs bottom-up. See `examples/lct008_keff.rs` for the proof that the
    // choice is a global y-mirror for this model.
    let at = |lid: i32, ix: usize, iy: usize| -> i32 {
        let (nx, ny, u) = &lattices[&lid];
        u[(ny - 1 - iy) * nx + ix]
    };

    // Global pin index: tile `ix` and in-assembly pin `ax` map to
    // n = −52 + 15·ix + ax, because the core pitch is exactly 15 pin pitches
    // (24.5364 / 1.63576 = 15) and the grids are concentric.
    let mut openmc_fuel = std::collections::HashMap::new();
    for cy in 0..7usize {
        for cx in 0..7usize {
            let uid = at(99, cx, cy);
            for ay in 0..15usize {
                for ax in 0..15usize {
                    let key = (
                        -52 + 15 * cx as i32 + ax as i32,
                        -52 + 15 * cy as i32 + ay as i32,
                    );
                    let is_fuel = match wrappers.get(&uid) {
                        // universe 999 is the all-water tile (it is filled by a
                        // universe, not a lattice, so it has no wrapper entry)
                        None => false,
                        Some(&lid) => at(lid, ax, ay) == 2,
                    };
                    openmc_fuel.insert(key, is_fuel);
                }
            }
        }
    }
    assert_eq!(
        openmc_fuel.len(),
        11_025,
        "the 7 × 7 × 15 × 15 grid changed size"
    );

    // --- the MCNP side, transcribed by hand ---------------------------------
    // From `mcnp_case-1.input`, quadrant cells 7–13 with the surfaces they name,
    // converted to pin indices by dividing by the 1.63576 cm pin pitch:
    //   px/py  17.17540 → 10.5      49.89060 → 30.5
    //          33.53300 → 20.5      58.06940 → 35.5
    //          36.80460 → 22.5      66.24820 → 40.5
    // The quadrant is mirrored into all four by the reflective `*px 0` / `*py 0`.
    fn mcnp_fuel(nx: i32, ny: i32) -> bool {
        let (a, b) = (nx.abs(), ny.abs());
        (a <= 22 && b <= 22)                                   // cell 7, inner core
            || ((23..=40).contains(&a) && b <= 10)              // cell 8
            || ((23..=35).contains(&a) && (11..=20).contains(&b)) // cell 9
            || ((23..=30).contains(&a) && (21..=22).contains(&b)) // cell 10
            || (a <= 30 && (23..=30).contains(&b))             // cell 11
            || (a <= 20 && (31..=35).contains(&b))             // cell 12
            || (a <= 10 && (36..=40).contains(&b)) // cell 13
    }

    // The transcription must be diagonally symmetric, because the deck's
    // quadrant is — a cheap guard on the hand-written half of this test.
    for a in 0..=52 {
        for b in 0..=52 {
            assert_eq!(
                mcnp_fuel(a, b),
                mcnp_fuel(b, a),
                "the hand-transcribed MCNP region is not symmetric under x↔y at ({a}, {b}); \
                 the deck's quadrant is, so the transcription is wrong"
            );
        }
    }

    let mut mismatches = Vec::new();
    let (mut n_openmc, mut n_mcnp) = (0usize, 0usize);
    for (&(nx, ny), &is_fuel) in &openmc_fuel {
        let want = mcnp_fuel(nx, ny);
        if is_fuel {
            n_openmc += 1;
        }
        if want {
            n_mcnp += 1;
        }
        if is_fuel != want {
            mismatches.push((nx, ny, is_fuel, want));
        }
    }
    mismatches.sort_unstable();
    println!(
        "LEU-COMP-THERM-008 core: {} pin positions, fuel pins — OpenMC XML {n_openmc}, \
         MCNP deck {n_mcnp}, mismatches {}",
        openmc_fuel.len(),
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "the committed OpenMC lattice map and the original MCNP deck describe different \
         cores — {} of {} pin positions disagree, first few {:?}",
        mismatches.len(),
        openmc_fuel.len(),
        &mismatches[..mismatches.len().min(8)]
    );
    assert_eq!(
        n_openmc, 4961,
        "the core's fuel loading changed; it was 4961 pins on 2026-09-11"
    );
}

/// Remove `<!-- … -->` so a commented-out surface is never read as markup.
fn strip_xml_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(p) = rest.find("<!--") {
        out.push_str(&rest[..p]);
        match rest[p..].find("-->") {
            Some(e) => rest = &rest[p + e + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// `id → (nx, ny, universes)` for every `<lattice>` in an OpenMC geometry file,
/// with the `<universes>` block in the file's own top-down row order.
fn read_lattices(xml: &str) -> std::collections::HashMap<i32, (usize, usize, Vec<i32>)> {
    let mut out = std::collections::HashMap::new();
    let mut rest = xml;
    while let Some(p) = rest.find("<lattice ") {
        let open_end = p + rest[p..].find('>').expect("unterminated <lattice>");
        let head = &rest[p..open_end];
        let close = open_end
            + rest[open_end..]
                .find("</lattice>")
                .expect("unterminated lattice");
        let body = &rest[open_end + 1..close];

        let id: i32 = xml_attr(head, "id")
            .expect("lattice id")
            .parse()
            .expect("id");
        let dim: Vec<usize> = xml_attr(head, "dimension")
            .expect("dimension")
            .split_whitespace()
            .map(|t| t.parse().expect("dimension"))
            .collect();
        let u_start = body.find("<universes>").expect("<universes>") + "<universes>".len();
        let u_end = body.find("</universes>").expect("</universes>");
        let universes: Vec<i32> = body[u_start..u_end]
            .split_whitespace()
            .map(|t| t.parse().expect("universe id"))
            .collect();
        assert_eq!(universes.len(), dim[0] * dim[1], "lattice {id} is ragged");
        out.insert(id, (dim[0], dim[1], universes));
        rest = &rest[close + 1..];
    }
    out
}

/// `universe id → lattice id` for every wrapper cell (`<cell universe=… fill=…>`
/// whose fill names a lattice). Water tiles fill a *universe* instead and are
/// simply absent from the map.
fn read_lattice_wrappers(xml: &str) -> std::collections::HashMap<i32, i32> {
    let lattice_ids: std::collections::HashSet<i32> = read_lattices(xml).keys().copied().collect();
    let mut out = std::collections::HashMap::new();
    for chunk in xml.split("<cell ").skip(1) {
        let head = &chunk[..chunk.find('>').expect("unterminated <cell>")];
        let (Some(u), Some(f)) = (xml_attr(head, "universe"), xml_attr(head, "fill")) else {
            continue;
        };
        let (u, f): (i32, i32) = (u.parse().expect("universe"), f.parse().expect("fill"));
        if lattice_ids.contains(&f) {
            out.insert(u, f);
        }
    }
    out
}

/// The value of attribute `name` in a start tag, or `None`.
fn xml_attr<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    let mut i = 0usize;
    while let Some(p) = head[i..].find(name) {
        let s = i + p;
        let ok_before = s == 0 || head[..s].ends_with(char::is_whitespace);
        let rest = head[s + name.len()..].trim_start();
        if ok_before && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let end = v[1..].find(q)? + 1;
                return Some(&v[1..end]);
            }
        }
        i = s + name.len();
    }
    None
}
