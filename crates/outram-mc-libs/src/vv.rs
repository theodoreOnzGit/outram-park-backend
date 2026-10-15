//! Verification & validation gates and the committed oracle tables this crate is
//! measured against.
//!
//! The gate helpers themselves live one crate down, in
//! [`njoy_outram_park_fork::vv`], because both crates run oracle comparisons and
//! this one depends on that one — so there is a single implementation rather
//! than two that drift. They are re-exported here so callers need only one path.
//!
//! [`njoy_golden`] holds the NJOY2016 oracle **values** for the comparisons that
//! are about *this* crate's fidelity: the S(alpha,beta) laws and the U-238 point
//! cross sections. Those belong here rather than in the data crate, because what
//! they measure is `outram-mc-libs` reproducing NJOY, not NJOY reproducing
//! itself.

pub mod njoy_golden;
/// The uranium-graphite compositions of tutorial rungs 2 and 3 (GitHub #524,
/// #525), shared by their examples and the web demo's rungs.
pub mod ugraphite;

pub use njoy_outram_park_fork::vv::{
    assert_absolute, assert_monotone, assert_relative, assert_reproduces_keff,
    assert_reproduces_recorded, assert_table_relative, RecordedKeff, WorstDeviation,
};

/// Pooled statistics of a seed ensemble: `(mean, sample sd, standard error)`.
///
/// # Why a shared helper and not a local closure
///
/// Every criticality benchmark in this crate is a Monte Carlo estimate whose
/// **single-run scatter is far larger than the effect sizes being argued
/// about** — Godiva carries `sd ≈ 175 pcm` per run at 5000 histories ×
/// [40 + 120]. A single run therefore cannot resolve a 100–200 pcm change, and
/// this crate's record contains three headline numbers that were single draws
/// and moved by more than their own quoted uncertainty when pooled
/// (`+57 → +228`, `+512 → +247`, URR `+79 → +43`).
///
/// The fix is to make pooling the default way a benchmark reports, which means
/// it has to be one line at every call site.
///
/// # The distinction that matters
///
/// - **`sd`** is what *one* run scatters by. Quote it when telling a reader
///   what a single reproduction of the example will give them.
/// - **`sem = sd/√n`** is the uncertainty *on the pooled mean*. Quote it when
///   comparing against a benchmark or another code.
///
/// Confusing the two is how a result gets over- or under-claimed. Both are
/// returned so a caller cannot silently pick the flattering one.
///
/// Returns `(mean, 0.0, 0.0)` for a single sample: one draw has no measurable
/// spread, and reporting `0` uncertainty is more honest than inventing one.
///
/// # Example
///
/// ```
/// use outram_mc_libs::vv::pooled;
/// let (mean, sd, sem) = pooled(&[100.0, 200.0, 300.0]);
/// assert!((mean - 200.0).abs() < 1e-9);
/// assert!((sd - 100.0).abs() < 1e-9);
/// assert!((sem - 100.0 / 3.0_f64.sqrt()).abs() < 1e-9);
/// ```
///
/// **Moved 2026-10-02** to [`raffles::estimators::pooled`] (GitHub #500), byte
/// for byte; this path re-exports it, and the bench knobs below stay here.
pub use raffles::estimators::pooled;

/// How many seeds a benchmark example should run, from `OUTRAM_BENCH_SEEDS`.
///
/// Defaults to `1`, so an example keeps its original single-seed behaviour and
/// its original runtime unless a caller asks for an ensemble. Set it to the
/// count that reaches the uncertainty you need: at seed-to-seed `sd`, the
/// pooled `sem` is `sd/√n`, so 7 seeds reach ~70 pcm on a case with
/// `sd ≈ 175 pcm` and 64 reach ~22.
pub fn bench_seeds() -> usize {
    std::env::var("OUTRAM_BENCH_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n >= 1)
        .unwrap_or(1)
}

/// A benchmark example's run size, overridable from the environment:
/// `OUTRAM_NPART` (histories per generation), `OUTRAM_NINACTIVE` and
/// `OUTRAM_NACTIVE` (generations). Each variable that is unset or unparsable
/// leaves that example's own default in place, so a plain run is unchanged.
///
/// For profiling and cost studies (`scripts/profile-icsbep.sh callgrind` runs
/// under valgrind at a few hundred histories), not for quoting `k`: a reduced
/// run is not converged and its eigenvalue means nothing.
pub fn bench_run_size(n_particles: usize, n_inactive: usize, n_active: usize) -> (usize, usize, usize) {
    let get = |var: &str, default: usize| {
        std::env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n: &usize| n >= 1)
            .unwrap_or(default)
    };
    (
        get("OUTRAM_NPART", n_particles),
        get("OUTRAM_NINACTIVE", n_inactive),
        get("OUTRAM_NACTIVE", n_active),
    )
}

/// The [`SpeedTier`](crate::material::speed::SpeedTier) a benchmark example
/// should build its nuclides at, from `OUTRAM_SPEED` (`standard`, `fast` or
/// `very-fast`).
///
/// Unset means the default tier (`Fast`, which is exact), so an example's
/// results do not depend on whether the variable is set unless a caller asks
/// for `very-fast`. An unrecognised value **panics** with the list of valid
/// ones, rather than silently running a tier the caller did not ask for.
pub fn bench_speed() -> crate::material::speed::SpeedTier {
    match std::env::var("OUTRAM_SPEED") {
        Ok(v) => v.parse().unwrap_or_else(|e| panic!("OUTRAM_SPEED: {e}")),
        Err(_) => crate::material::speed::SpeedTier::default(),
    }
}

/// Report the transport-loss channels of a [`KeffResult`] to stderr.
///
/// **What this is for.** A Monte Carlo history that is lost, or that exhausts
/// its event budget, is normally **scored as a leak**. The neutron balance
/// then closes and `k_eff` looks entirely healthy, so a transport defect of
/// this class is invisible in the reported eigenvalue and its uncertainty.
/// This crate has already been bitten by exactly that: an HTR-10 core model
/// in which **68.5 % of histories** ended on the event budget and were
/// counted as leakage, with no outward sign in `k`.
///
/// Call this on any case whose residual is being interpreted as physics.
/// A non-zero `lost_locate` or `stuck_events` means some fraction of the
/// answer is a geometry defect wearing a leakage costume, and the residual
/// cannot be attributed to nuclear data until it is zero.
///
/// **Interpreting the fields**
///
/// - `lost_locate` — [`Geometry::locate`] failed to place a particle. Always
///   a defect; there is no physical configuration in which a point inside the
///   model has no cell.
/// - `stuck_events` — the history hit the per-history event cap. Either the
///   geometry traps it (coincident or mis-sensed surfaces) or the cap is too
///   low for a legitimately long walk. `stuck_path_cm` and `stuck_last_e`
///   distinguish the two: a trapped particle accumulates almost no path.
/// - `neg_dist` — a negative distance-to-boundary was returned.
///   `neg_from_lattice` splits out the lattice path, which is where the
///   coordinate-frame bugs of this crate's history have lived.
///
/// All rates are expressed per history, because an absolute count is
/// meaningless without the denominator.
///
/// This writes to **stderr** so it composes with examples whose stdout is a
/// machine-read data stream.
pub fn report_transport_losses(label: &str, r: &crate::physics::keff::KeffResult) {
    let n = r.histories.max(1) as f64;
    let pct = |x: u64| 100.0 * x as f64 / n;

    eprintln!("  [transport losses] {label}");
    eprintln!(
        "    histories {}   collisions {}   leaks: vacuum {} / infinity {}",
        r.histories, r.collisions, r.leak_vacuum, r.leak_infinity
    );
    eprintln!(
        "    lost_locate  {:>10}  ({:.4} % of histories)",
        r.lost_locate,
        pct(r.lost_locate)
    );
    eprintln!(
        "    stuck_events {:>10}  ({:.4} %)   mean path {:.3e} cm   last E {:.3e} eV",
        r.stuck_events,
        pct(r.stuck_events),
        r.stuck_path_cm,
        r.stuck_last_e
    );
    eprintln!(
        "    neg_dist     {:>10}  ({:.4} %)   worst {:.3e}   from lattice {}",
        r.neg_dist,
        pct(r.neg_dist),
        r.neg_worst,
        r.neg_from_lattice
    );

    let bad = r.lost_locate + r.stuck_events + r.neg_dist;
    if bad == 0 {
        eprintln!("    => CLEAN: no history was lost, trapped, or given a negative distance.");
    } else {
        eprintln!(
            "    => {bad} histories ({:.4} %) ended in a TRANSPORT DEFECT, not physics. \
             Any residual interpreted as nuclear data is contaminated by this.",
            pct(bad)
        );
    }
}

/// Report source-convergence evidence for a k-eigenvalue run.
///
/// **Why this is not optional.** A Monte Carlo eigenvalue run scores `k` while
/// the fission source is still relaxing toward its fundamental mode if too few
/// inactive generations were run. The result is a **bias, not a variance** —
/// it does not shrink when you pool more seeds, and the reported `sem` gives
/// no hint of it, because every seed is biased the same way. Pooling 32 seeds
/// of a badly converged case produces a tight, confident, wrong number.
///
/// Loosely coupled systems — large lattices, reflected assemblies, anything
/// with a dominance ratio near 1 — need far more inactive generations than a
/// compact bare sphere. Copying a settings block from one case to another is
/// therefore not safe, which is exactly how this goes unnoticed.
///
/// **The diagnostic.** Split the ACTIVE generations in half and compare the
/// two means. Under a converged source the halves differ only by statistics;
/// a systematic drift between them means the source was still moving while
/// scoring, and `n_inactive` is too low. Shannon entropy over the fission
/// source (`entropy`) is the companion check and is reported when present.
///
/// `drift_pcm` is returned so a caller can gate on it rather than eyeball it.
/// A drift comparable to, or larger than, the residual being interpreted means
/// that residual cannot be attributed to nuclear data at all.
///
/// **See also (2026-10-03, GitHub #496):**
/// [`crate::stats::convergence::SourceConvergence`] runs MSER-5, Geweke and a
/// change-point test over the WHOLE entropy and `k` traces (inactive
/// included) and recommends an inactive count with its evidence; and
/// [`crate::stats::correlated_sigma::KeffUncertainty`] gives the
/// autocorrelation-corrected `σ` that the lower-bound error below lacks.
/// This function is unchanged and still what the ICSBEP examples print.
pub fn report_source_convergence(
    label: &str,
    r: &crate::physics::keff::KeffResult,
    n_inactive: usize,
) -> f64 {
    let k = &r.k_by_generation;
    let n_in = n_inactive.min(k.len());
    let active = &k[n_in..];
    eprintln!("  [source convergence] {label}");
    if active.len() < 4 {
        eprintln!("    too few active generations ({}) to split", active.len());
        return f64::NAN;
    }
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let h = active.len() / 2;
    let (a, b) = (mean(&active[..h]), mean(&active[h..]));
    let drift = (b - a) * 1.0e5;

    let q = n_in / 4;
    if q > 0 {
        let qs: Vec<String> = (0..4)
            .map(|i| format!("{:.4}", mean(&k[i * q..(i + 1) * q])))
            .collect();
        eprintln!("    inactive quarters: {}", qs.join("  "));
    }
    // The drift needs its own uncertainty or it is not a measurement. Estimate
    // it from the scatter of k ACROSS active generations. NOTE this is a LOWER
    // BOUND on the true uncertainty: successive generations share a fission
    // source and are positively correlated, so the independent-sample formula
    // understates the error bar. A drift that is not resolved against even
    // this optimistic bound is certainly not resolved.
    let n = active.len() as f64;
    let var = active.iter().map(|x| (x - mean(active)).powi(2)).sum::<f64>() / (n - 1.0);
    let drift_sem = (var * 2.0 / (n / 2.0)).sqrt() * 1.0e5;
    eprintln!(
        "    active halves:     {:.5} then {:.5}   drift {:+.0} +/- {:.0} pcm (>= this err)",
        a, b, drift, drift_sem
    );
    if !r.entropy.is_empty() {
        let e = &r.entropy;
        eprintln!(
            "    Shannon entropy:   first {:.4}  last {:.4}  (flat => converged)",
            e[0],
            e[e.len() - 1]
        );
    } else {
        eprintln!("    Shannon entropy:   NOT COMPUTED for this run");
    }
    if drift.abs() > 2.0 * drift_sem && drift.abs() > 100.0 {
        eprintln!(
            "    => DRIFT {:+.0} pcm across the active phase. The source was still \
             moving while scoring: raise n_inactive. Any residual smaller than this \
             is not interpretable.",
            drift
        );
    } else {
        eprintln!("    => no resolved drift; source looks converged.");
    }
    drift
}

/// Active-phase drift in pcm, computed quietly (no printing).
///
/// Splits the active generations in half and returns `(second - first)` in
/// pcm. See [`report_source_convergence`] for what this measures and why it
/// matters; this variant exists so the statistic can be **pooled across
/// seeds**, which is the only way to tell a real convergence bias from noise.
///
/// **Why pooling is the decisive test.** At one seed the drift carries an
/// uncertainty comparable to its own size, so a single value proves nothing.
/// A genuine convergence bias is *systematic* — the source relaxes the same
/// way every time — so it survives averaging over independent seeds. Statistical
/// scatter does not. Pooling `n` seeds shrinks the error by `sqrt(n)` while
/// leaving a true bias untouched.
///
/// Returns `NaN` when there are too few active generations to split.
pub fn source_convergence_drift_pcm(
    r: &crate::physics::keff::KeffResult,
    n_inactive: usize,
) -> f64 {
    let k = &r.k_by_generation;
    let n_in = n_inactive.min(k.len());
    let active = &k[n_in..];
    if active.len() < 4 {
        return f64::NAN;
    }
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let h = active.len() / 2;
    (mean(&active[h..]) - mean(&active[..h])) * 1.0e5
}

/// **ICSBEP HEU-MET-FAST-001 (Godiva)**, the bare HEU metal sphere, as this
/// crate's ENDF examples model it: one sphere, one homogeneous material,
/// U-234/235/238 at the evaluation's atom densities, vacuum outside.
///
/// These are the numbers `examples/godiva_keff_endf_local.rs` (the crate's
/// maturity evidence, and the model of the five-route study,
/// `verification_and_validation/icsbep/five_route_keff_2026_09_29.md`) has
/// always run. They were typed into that example and into many tests beside
/// it; they are collected here (2026-10-04, gh:#521) so the tutorial and the
/// web demo use the model the record was measured on, not another copy of it.
/// Changing a value here changes that example.
pub mod godiva {
    use crate::material::material::{Material, NuclideComponent};

    // ANCHOR: model
    /// Sphere radius \[cm\].
    pub const RADIUS_CM: f64 = 8.7407;
    /// Material temperature \[K\]: room temperature (a metal assembly).
    pub const TEMPERATURE_K: f64 = 293.6;
    /// `(name, ENDF/B-VIII.0 tape in reference-data/endf/, atoms per barn-cm)`.
    pub const NUCLIDES: [(&str, &str, f64); 3] = [
        ("U234", "n-092_U_234-ENDF8.0.endf", 4.9184e-4),
        ("U235", "n-092_U_235-ENDF8.0.endf", 4.4994e-2),
        ("U238", "n-092_U_238.endf", 2.4984e-3),
    ];
    // ANCHOR_END: model

    /// The benchmark `k_eff`: the assembly was critical, so 1.0000 by
    /// construction.
    pub const BENCHMARK_K: f64 = 1.0000;
    /// The evaluation's stated uncertainty on [`BENCHMARK_K`], as tabulated
    /// in Mosteller, LA-UR-10-06230 (2010) and van der Marck, Nucl. Data
    /// Sheets 107 (2006) 3061 (rows checked by the maintainer, 2026-10-13).
    pub const BENCHMARK_SIGMA: f64 = 0.0010;

    /// The Godiva material, indexing nuclides `0, 1, 2` in [`NUCLIDES`] order.
    pub fn material() -> Material {
        Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMPERATURE_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, &(_, _, n))| NuclideComponent { nuclide_idx: i, atom_density: n })
                .collect(),
        }
    }
}

/// Build `Material`s from `(name, [(nuclide, atoms per barn-cm)])` rows,
/// with nuclide indices looked up by name in `nuclides` (the order the caller
/// loads them in). Ids are `1..=n` in row order.
///
/// # Panics
/// If a row names a nuclide that `nuclides` does not hold: a model's own
/// tables disagreeing with each other is a defect in this file.
fn materials_from(
    rows: &[(&str, &[(&str, f64)])],
    nuclides: &[(&str, &str)],
    temperature_k: f64,
) -> Vec<crate::material::material::Material> {
    use crate::material::material::{Material, NuclideComponent};
    rows.iter()
        .enumerate()
        .map(|(i, (name, comps))| Material {
            id: i as i32 + 1,
            name: (*name).into(),
            temperature: temperature_k,
            components: comps
                .iter()
                .map(|(n, d)| NuclideComponent {
                    nuclide_idx: nuclides
                        .iter()
                        .position(|(m, _)| m == n)
                        .unwrap_or_else(|| panic!("material {name} names nuclide {n}, not in the model's list")),
                    atom_density: *d,
                })
                .collect(),
        })
        .collect()
}

/// **ICSBEP IEU-MET-FAST-002 (Jemima)**: a cylinder of 16 % enriched uranium
/// metal with natural-uranium reflectors below, around and above it, vacuum
/// outside.
///
/// Specification: the ICSBEP model in `mit-crpg/benchmarks`
/// (`icsbep/ieu-met-fast-002/openmc`, Paul Romano, 2012-01-08), as
/// `examples/jemima_keff.rs` transcribed it and as the five-route study runs
/// it (`verification_and_validation/icsbep/five_route_keff_2026_09_29.md`).
/// Collected here on 2026-10-13 (GitHub #859) so the acceptance check
/// (`examples/icsbep_five_route_keff.rs`) and the Monte Carlo web demo build
/// ONE model. `examples/jemima_keff.rs` still carries its own copy of these
/// numbers, with its ablation knobs.
///
/// | region | z \[cm\] | r \[cm\] | material |
/// |---|---|---|---|
/// | bottom reflector | 0 – 7.62 | < 26.6446 | natural U |
/// | core | 7.62 – 39.571 | < 19.05 | 16 % enriched U |
/// | radial reflector | 7.62 – 39.571 | 19.05 – 26.6446 | natural U |
/// | top reflector | 39.571 – 47.0894 | < 26.6446 | natural U |
pub mod jemima {
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::geometry::Geometry;
    use crate::geometry::position::Position;
    use crate::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder, ZPlane};
    use crate::geometry::universe::Universe;
    use crate::material::material::Material;
    use crate::physics::transport_csg::SourceBox;

    /// Bottom of the assembly \[cm\].
    pub const Z_BOT: f64 = 0.0;
    /// Bottom of the core \[cm\].
    pub const Z_CORE_LO: f64 = 7.62;
    /// Top of the core \[cm\].
    pub const Z_CORE_HI: f64 = 39.571;
    /// Top of the assembly \[cm\].
    pub const Z_TOP: f64 = 47.0894;
    /// Core radius \[cm\].
    pub const R_CORE: f64 = 19.05;
    /// Outer radius of the reflector \[cm\].
    pub const R_OUT: f64 = 26.6446;
    /// Material temperature \[K\].
    pub const TEMPERATURE_K: f64 = 293.6;
    /// `(name, ENDF/B-VIII.0 tape in reference-data/endf/)`, in the order
    /// [`materials`] indexes them.
    pub const NUCLIDES: [(&str, &str); 3] = [
        ("U234", "n-092_U_234-ENDF8.0.endf"),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
    ];
    /// Index of the core material in [`materials`].
    pub const MAT_CORE: usize = 0;
    /// Index of the reflector material in [`materials`].
    pub const MAT_REFLECTOR: usize = 1;
    /// `(material, [(nuclide, atoms per barn-cm)])`. U-238 is 83 % of the
    /// core's heavy metal and 99.3 % of the reflector's.
    pub const MATERIALS: [(&str, &[(&str, f64)]); 2] = [
        ("oralloy core", &[("U234", 8.4430e-05), ("U235", 7.7777e-03), ("U238", 3.9671e-02)]),
        ("natural uranium reflector", &[("U234", 2.6433e-06), ("U235", 3.4603e-04), ("U238", 4.7711e-02)]),
    ];
    /// The benchmark `k_eff`, 1.0000 ± 0.0030. ~~No handbook uncertainty is
    /// cited.~~ **CORRECTED 2026-10-14:** the handbook value as tabulated in
    /// two open secondary sources (the handbook itself is licence
    /// restricted, `DATA_POLICY.md`): Mosteller, *An Expanded Criticality
    /// Validation Suite for MCNP*, LA-UR-10-06230 (2010), and van der Marck,
    /// *Benchmarking ENDF/B-VII.0*, Nucl. Data Sheets 107 (2006) 3061. The
    /// maintainer checked the rows against the source pages on 2026-10-13.
    pub const BENCHMARK_K: f64 = 1.0000;
    /// The benchmark's stated uncertainty on [`BENCHMARK_K`], same sources.
    pub const BENCHMARK_SIGMA: f64 = 0.0030;

    /// The two materials, indexing nuclides in [`NUCLIDES`] order.
    pub fn materials() -> Vec<Material> {
        super::materials_from(&MATERIALS, &NUCLIDES, TEMPERATURE_K)
    }

    /// Six surfaces and four cells, as the ICSBEP model's XML lists them:
    /// cell 0 bottom reflector, 1 core, 2 radial reflector, 3 top reflector.
    pub fn geometry() -> Geometry {
        let zp = |z0: f64, bc: BoundaryType| SurfaceKind::ZPlane(ZPlane { z0, bc });
        let zc = |r: f64, bc: BoundaryType| SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r, bc });
        // 0..5 mirror the model's surfaces 1..6.
        let surfaces = vec![
            zp(Z_BOT, BoundaryType::Vacuum),
            zp(Z_CORE_LO, BoundaryType::Transmissive),
            zp(Z_CORE_HI, BoundaryType::Transmissive),
            zp(Z_TOP, BoundaryType::Vacuum),
            zc(R_CORE, BoundaryType::Transmissive),
            zc(R_OUT, BoundaryType::Vacuum),
        ];
        let hs = |surface_idx: usize, sense: HalfSpaceSense| RegionToken::HalfSpace { surface_idx, sense };
        let out = |i: usize| hs(i, HalfSpaceSense::Outside);
        let ins = |i: usize| hs(i, HalfSpaceSense::Inside);
        let and = RegionToken::Intersection;
        let cells = vec![
            // bottom reflector: +1 -2 -6
            Cell::material(1, vec![out(0), ins(1), and, ins(5), and], MAT_REFLECTOR, TEMPERATURE_K),
            // core: +2 -3 -5
            Cell::material(2, vec![out(1), ins(2), and, ins(4), and], MAT_CORE, TEMPERATURE_K),
            // radial reflector: +2 -3 +5 -6
            Cell::material(3, vec![out(1), ins(2), and, out(4), and, ins(5), and], MAT_REFLECTOR, TEMPERATURE_K),
            // top reflector: +3 -4 -6
            Cell::material(4, vec![out(2), ins(3), and, ins(5), and], MAT_REFLECTOR, TEMPERATURE_K),
        ];
        let cell_indices = (0..cells.len()).collect();
        Geometry { surfaces, cells, universes: vec![Universe { id: 0, cell_indices }], lattices: vec![], root_universe: 0 }
    }

    /// Where the first generation starts: the core's bounding box.
    pub fn source_box() -> SourceBox {
        SourceBox { lower: Position::new(-R_CORE, -R_CORE, Z_CORE_LO), upper: Position::new(R_CORE, R_CORE, Z_CORE_HI) }
    }
}

/// **ICSBEP HEU-SOL-THERM-009 case 1**: a sphere of highly enriched uranium
/// oxyfluoride solution in a thin aluminium tank, reflected by water, vacuum
/// outside.
///
/// As `examples/hst009_keff.rs` transcribed it and as the five-route study
/// runs it, with that example's stated approximations: ~~**U-236
/// omitted** (8.8837e-6 /b·cm, 0.5 % of the uranium; no tape here),~~
/// **O-17 folded into O-16**, and **Cu and Zn omitted from the tank**.
/// **CHANGED 2026-10-15 (maintainer direction, gh:#875):** U-236 is in the
/// model at the benchmark's 8.8837e-6 /b·cm, appended last in [`NUCLIDES`]
/// so the other indices are unchanged. Collected
/// here on 2026-10-13 (GitHub #859) for the same reason as [`jemima`];
/// `examples/hst009_keff.rs` still carries its own copy.
///
/// H-1 takes the H in H2O thermal scattering law ([`SAB_TAPE`]), in the
/// solution and in the reflector.
pub mod hst009 {
    use crate::geometry::geometry::Geometry;
    use crate::geometry::position::Position;
    use crate::geometry::surface::BoundaryType;
    use crate::material::material::Material;
    use crate::pebble_beds::fhr_pebble::fhr_pebble_geometry;
    use crate::physics::transport_csg::SourceBox;

    /// Radius of the solution \[cm\].
    pub const R_SOLUTION: f64 = 11.5177;
    /// Outer radius of the aluminium tank \[cm\].
    pub const R_TANK: f64 = 11.6764;
    /// Outer radius of the water reflector \[cm\].
    pub const R_REFLECTOR: f64 = 35.0;
    /// Material temperature \[K\].
    pub const TEMPERATURE_K: f64 = 293.6;
    /// `(name, ENDF/B-VIII.0 tape in reference-data/endf/)`, in the order
    /// [`materials`] indexes them.
    pub const NUCLIDES: [(&str, &str); 12] = [
        ("U234", "n-092_U_234-ENDF8.0.endf"),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
        ("F19", "n-009_F_019-ENDF8.0.endf"),
        ("O16", "n-008_O_016-ENDF8.0.endf"),
        ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
        ("Al27", "n-013_Al_027-ENDF8.0.endf"),
        ("Si28", "n-014_Si_028-ENDF8.0.endf"),
        ("Si29", "n-014_Si_029-ENDF8.0.endf"),
        ("Si30", "n-014_Si_030-ENDF8.0.endf"),
        ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
        ("U236", "n-092_U_236-ENDF8.0.endf"),
    ];
    /// Index of H-1 in [`NUCLIDES`] (it takes the thermal scattering law).
    pub const N_H1: usize = 5;
    /// The H in H2O thermal scattering law, ENDF/B-VIII.0.
    pub const SAB_TAPE: &str = "tsl-HinH2O.endf";
    /// Indices of the three materials in [`materials`].
    pub const MAT_SOLUTION: usize = 0;
    pub const MAT_TANK: usize = 1;
    pub const MAT_WATER: usize = 2;
    /// `(material, [(nuclide, atoms per barn-cm)])`. The O-16 entries are
    /// the model's O-16 plus its O-17.
    pub const MATERIALS: [(&str, &[(&str, f64)]); 3] = [
        (
            "uranium oxyfluoride solution",
            &[
                ("U234", 1.7561e-05),
                ("U235", 1.6626e-03),
                ("U236", 8.8837e-06),
                ("U238", 9.4079e-05),
                ("F19", 3.5663e-03),
                ("O16", 3.334735656e-02 + 1.264344e-05),
                ("H1", 5.9587e-02),
            ],
        ),
        (
            "1100 aluminium tank",
            &[
                ("Al27", 5.9699e-02),
                ("Si28", 5.09126279536e-04),
                ("Si29", 2.5851979832e-05),
                ("Si30", 1.7041740632e-05),
                ("Mn55", 1.4853e-05),
            ],
        ),
        ("water reflector", &[("H1", 6.6659e-02), ("O16", 3.3316368309e-02 + 1.2631691e-05)]),
    ];
    /// The benchmark `k_eff`, 0.9990 ± 0.0043. ~~Critical, so 1.0000 by
    /// construction.~~ **CORRECTED 2026-10-14:** the benchmark model's k is
    /// not 1. This is the handbook value as tabulated in an open secondary
    /// source (the handbook itself is licence restricted, `DATA_POLICY.md`):
    /// van der Marck, *Benchmarking ENDF/B-VII.0*, Nucl. Data Sheets 107
    /// (2006) 3061, HST-009 case 1. The maintainer checked the row against
    /// the source page on 2026-10-13.
    pub const BENCHMARK_K: f64 = 0.9990;
    /// The benchmark's stated uncertainty on [`BENCHMARK_K`], same source.
    pub const BENCHMARK_SIGMA: f64 = 0.0043;

    /// The three materials, indexing nuclides in [`NUCLIDES`] order.
    pub fn materials() -> Vec<Material> {
        super::materials_from(&MATERIALS, &NUCLIDES, TEMPERATURE_K)
    }

    /// Three concentric spheres: solution, tank, water, vacuum outside.
    /// (`fhr_pebble_geometry` is named for the pebble but builds exactly
    /// this; with no inner ball its "fuel" is the solution, its "shell" the
    /// tank and its "coolant" the reflector.)
    pub fn geometry() -> Geometry {
        fhr_pebble_geometry(0.0, R_SOLUTION, R_TANK, R_REFLECTOR, MAT_SOLUTION, MAT_TANK, MAT_WATER, BoundaryType::Vacuum, TEMPERATURE_K)
    }

    /// Where the first generation starts: the solution's bounding box.
    pub fn source_box() -> SourceBox {
        SourceBox {
            lower: Position::new(-R_SOLUTION, -R_SOLUTION, -R_SOLUTION),
            upper: Position::new(R_SOLUTION, R_SOLUTION, R_SOLUTION),
        }
    }
}

#[cfg(test)]
mod icsbep_model_tests {
    use super::{hst009, jemima};
    use crate::geometry::cell::{CellFill, SurfaceToken};
    use crate::geometry::position::{Direction, Position};

    /// A deterministic uniform stream for the point samples.
    fn stream(mut seed: u64) -> impl FnMut() -> f64 {
        move || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    /// The material the assembled geometry holds at `p`, `None` outside it.
    fn material_at(g: &crate::geometry::geometry::Geometry, p: Position) -> Option<usize> {
        let path = g.locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)?;
        match g.cells[path.leaf().cell].fill {
            CellFill::Material(m) => Some(m),
            _ => panic!("a cell of this model holds no material"),
        }
    }

    /// **Methodology.** 200 000 uniform points in a box 5 % larger than the
    /// Jemima assembly; at each, the material the assembled CSG reports is
    /// compared with a predicate written from the specification table (the
    /// check `examples/jemima_keff.rs` runs before transporting). Materials:
    /// U-238 is 83 % of the core's uranium atoms and 99.3 % of the
    /// reflector's. **Result (2026-10-13):** no mismatch, and the core takes
    /// its volume share of the points inside the assembly, to within 0.01
    /// (19.05² × 31.951 over 26.6446² × 47.0894 = 0.347).
    #[test]
    fn jemima_is_the_specified_cylinder_in_its_reflector() {
        let g = jemima::geometry();
        let expect = |p: Position| -> Option<usize> {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            if r >= jemima::R_OUT || p.z <= jemima::Z_BOT || p.z >= jemima::Z_TOP {
                None
            } else if p.z > jemima::Z_CORE_LO && p.z < jemima::Z_CORE_HI && r < jemima::R_CORE {
                Some(jemima::MAT_CORE)
            } else {
                Some(jemima::MAT_REFLECTOR)
            }
        };
        let mut prn = stream(1_234_567);
        let (mut core, mut inside) = (0usize, 0usize);
        for _ in 0..200_000 {
            let p = Position::new(
                (2.0 * prn() - 1.0) * jemima::R_OUT * 1.05,
                (2.0 * prn() - 1.0) * jemima::R_OUT * 1.05,
                jemima::Z_BOT - 1.0 + prn() * (jemima::Z_TOP - jemima::Z_BOT + 2.0),
            );
            let want = expect(p);
            assert_eq!(material_at(&g, p), want, "at {p:?}");
            inside += want.is_some() as usize;
            core += (want == Some(jemima::MAT_CORE)) as usize;
        }
        let share = core as f64 / inside as f64;
        assert!((share - 0.347).abs() < 0.01, "core share {share}");
        let m = jemima::materials();
        let u238 = |i: usize| m[i].components[2].atom_density / m[i].components.iter().map(|c| c.atom_density).sum::<f64>();
        assert!((u238(jemima::MAT_CORE) - 0.835).abs() < 0.005 && (u238(jemima::MAT_REFLECTOR) - 0.993).abs() < 0.001);
        let b = jemima::source_box();
        assert_eq!(material_at(&g, Position::new(0.0, 0.0, 0.5 * (b.lower.z + b.upper.z))), Some(jemima::MAT_CORE));
    }

    /// **Methodology.** 200 000 uniform points in a box 5 % larger than the
    /// HEU-SOL-THERM-009 reflector; the assembled CSG's material against the
    /// three radii of the specification. H-1 is nuclide [`hst009::N_H1`] and
    /// appears in the solution and the water, not the tank. **Result
    /// (2026-10-13):** no mismatch, and the solution takes its volume share
    /// of the points inside the model, to within 0.003
    /// ((11.5177 / 35)³ = 0.0356). Since 2026-10-15 (gh:#875) it also pins
    /// U-236 in the solution, and only there, at the benchmark's
    /// 8.8837e-6 /b·cm.
    #[test]
    fn hst009_is_three_concentric_spheres() {
        let g = hst009::geometry();
        let expect = |p: Position| -> Option<usize> {
            let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
            if r >= hst009::R_REFLECTOR {
                None
            } else if r < hst009::R_SOLUTION {
                Some(hst009::MAT_SOLUTION)
            } else if r < hst009::R_TANK {
                Some(hst009::MAT_TANK)
            } else {
                Some(hst009::MAT_WATER)
            }
        };
        let mut prn = stream(7_654_321);
        let (mut sol, mut inside) = (0usize, 0usize);
        for _ in 0..200_000 {
            let p = Position::new(
                (2.0 * prn() - 1.0) * hst009::R_REFLECTOR * 1.05,
                (2.0 * prn() - 1.0) * hst009::R_REFLECTOR * 1.05,
                (2.0 * prn() - 1.0) * hst009::R_REFLECTOR * 1.05,
            );
            let want = expect(p);
            assert_eq!(material_at(&g, p), want, "at {p:?}");
            inside += want.is_some() as usize;
            sol += (want == Some(hst009::MAT_SOLUTION)) as usize;
        }
        let share = sol as f64 / inside as f64;
        assert!((share - 0.0356).abs() < 0.003, "solution share {share}");
        let m = hst009::materials();
        assert_eq!(hst009::NUCLIDES[hst009::N_H1].0, "H1");
        let has_h = |i: usize| m[i].components.iter().any(|c| c.nuclide_idx == hst009::N_H1);
        assert!(has_h(hst009::MAT_SOLUTION) && !has_h(hst009::MAT_TANK) && has_h(hst009::MAT_WATER));
        let u236 = hst009::NUCLIDES.iter().position(|(n, _)| *n == "U236").expect("U236 in NUCLIDES");
        let u236_in = |i: usize| m[i].components.iter().find(|c| c.nuclide_idx == u236).map(|c| c.atom_density);
        assert_eq!(u236_in(hst009::MAT_SOLUTION), Some(8.8837e-06));
        assert!(u236_in(hst009::MAT_TANK).is_none() && u236_in(hst009::MAT_WATER).is_none());
        assert_eq!(m.iter().map(|x| x.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        let b = hst009::source_box();
        assert_eq!(material_at(&g, Position::new(0.5 * b.upper.x, 0.0, 0.0)), Some(hst009::MAT_SOLUTION));
    }
}
