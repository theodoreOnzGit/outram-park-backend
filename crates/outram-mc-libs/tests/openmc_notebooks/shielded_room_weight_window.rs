//! `shielded_room_weight_window` notebook -> outram-mc verification (LIVE).
//!
//! Notebook: `shielded_room_weight_window.ipynb`
//! (openmc-notebooks, MIT). GitHub #258.
//!
//! # A correction to this file's own previous contents
//!
//! ~~**GAP.** No weight-window or variance-reduction machinery exists, AND the
//! source notebook is absent upstream, so its exact API surface cannot be
//! confirmed.~~ **CORRECTED 2026-09-22.** Both halves of that were wrong by
//! the time this was read again.
//!
//! - The machinery now exists: [`WeightWindows`] and
//!   [`VarianceReduction`] (#258).
//! - **The notebook is NOT absent.** `shielded_room_weight_window.ipynb` is
//!   present in `openmc-dev/openmc-notebooks` and was read in full on
//!   2026-09-22 (repository HEAD `bb39f25`). The previous claim was checked
//!   rather than trusted, per the workspace rule that a doc claim the facts
//!   contradict is a defect to fix where it is found — and this one had the
//!   worst possible shape, a "missing" claim that justified not doing the
//!   work.
//!
//! # V&V — methodology
//!
//! **What the notebook does.** A concrete bunker with an L-shaped corridor
//! entrance, a 2.5 MeV isotropic point source at the centre of the inner room,
//! and a regular mesh flux tally over the whole domain. It runs the model
//! analog, then runs it again in two stages — a short run to fill the tally,
//! `wws.update_magic(tally)` to build weight windows from it, and a second run
//! with `weight_windows_on = True` — with the particle counts chosen so both
//! **take about the same wall-clock**.
//!
//! **What the notebook reports.** No numbers. Its committed cell outputs are
//! two figures and one prose sentence:
//!
//! > *"On my laptop both simulations took 20 seconds to complete but the
//! > resulting flux map from the simulation with weight windows shows neutrons
//! > got further through the geometry."*
//!
//! So, per this crate's V&V rule — reference values come from the notebook and
//! are never invented — **there is no reference number to compare against**,
//! and this test does not manufacture one. What it tests is the notebook's
//! actual claim: *at matched cost, the weight-window run resolves flux in
//! regions the analog run does not reach.*
//!
//! **Geometry and source are the notebook's, exactly** — the same eight
//! x-planes, seven y-planes and four z-planes, the same fourteen cells, the
//! same source position `(550, 825, 350)` and energy 2.5 MeV.
//!
//! **Materials deviate, and here is exactly how and why.** The notebook's air
//! is N/O/Ar and its concrete is H/C/O/Na/Mg/Al/Si/K/Ca/Fe. This checkout's
//! `reference-data/endf/` has **no N, Ar, K or Ca** tape, so:
//!
//! - **Air**: N and Ar are replaced by O at the same atom fractions, keeping
//!   the notebook's mass density 0.001205 g/cc. Air is ~1/1900 the density of
//!   the concrete and contributes ~1e-4 cm^-1 to a shield of ~0.1 cm^-1; this
//!   substitution cannot change which regions are reachable.
//! - **Concrete**: K (0.5656 at%) and Ca (1.8674 at%) are replaced by Si at
//!   the same atom fractions, keeping the mass density 2.3 g/cm3. Si is the
//!   nearest available scatterer and already the dominant heavy constituent.
//!   **This is not free bookkeeping**: at fixed mass density, substituting
//!   lighter nuclides at equal atom fraction raises the number density. The
//!   notebook's mean molar mass is 17.0098 g/mol and the substituted one is
//!   16.7236, so this concrete carries **1.7 % more atoms per cm3** than the
//!   notebook's. Stated rather than absorbed, because a shielding result is
//!   exponential in the atom density.
//!
//! Since the notebook publishes no number, neither deviation invalidates a
//! comparison — there is none to invalidate. They are recorded so that if a
//! numeric comparison is ever added, it starts from a known composition.
//!
//! # V&V — results
//!
//! Printed at run time with `--nocapture`; the write-up is
//! `verification_and_validation/variance_reduction/shielded_room_2026_09_22.md`.

use std::time::Instant;

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::physics::variance_reduction::VarianceReduction;
use outram_mc_libs::physics::weight_windows::WeightWindows;
use outram_mc_libs::tally::filter::{FilterKind, MeshFilter};
use outram_mc_libs::tally::mesh::{MeshKind, RegularMesh};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const TEMP: f64 = 293.6;
const AVOGADRO: f64 = 6.022_140_76e23;

// ── The notebook's plane positions ──────────────────────────────────────────
const X: [f64; 8] = [0.0, 100.0, 300.0, 800.0, 1050.0, 1250.0, 1450.0, 1550.0];
const Y: [f64; 7] = [0.0, 100.0, 300.0, 1000.0, 1600.0, 1800.0, 1900.0];
const Z: [f64; 4] = [0.0, 100.0, 600.0, 700.0];

/// `(name, tape, element molar mass)` for every nuclide this case needs.
///
/// # One isotope per element, and why
///
/// The first version of this test carried all sixteen natural isotopes of the
/// eight elements. It was **OOM-killed** (SIGKILL) after 37 minutes at 13 GB
/// of a 15 GB machine with no swap. Two causes, both worth recording rather
/// than quietly working around:
///
/// - Sixteen simultaneous pointwise reconstructions at a 1e-3 tolerance is
///   simply a lot of resident grid.
/// - **Fe-57 is a known RECONR blow-up in this very workspace** — it and
///   Mo-95 are `LRF=7` evaluations that OOM at every tolerance tried, which is
///   already an open item in this repo's own task list. Including it was the
///   mistake, and it was made by listing isotopes from an abundance table
///   without checking them against what this workspace already knows.
///
/// So each element is folded onto its **principal** isotope at the element's
/// full atom fraction. That is a third documented deviation from the
/// notebook's materials, on top of the two in the module docs. It matters
/// least for the elements that dominate — H is 99.99 % H-1 and O is 99.76 %
/// O-16 — and most for Mg (78.99 % Mg-24) and Si (92.23 % Si-28). Since the
/// notebook publishes no number, there is nothing for it to bias; it is
/// recorded so a future numeric comparison does not start from the wrong
/// composition.
const NUCLIDES: &[(&str, &str, f64)] = &[
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf", 1.008),
    ("C12", "n-006_C_012-ENDF8.0.endf", 12.011),
    ("O16", "n-008_O_016-ENDF8.0.endf", 15.999),
    ("Na23", "n-011_Na_023-ENDF8.0.endf", 22.990),
    ("Mg24", "n-012_Mg_024-ENDF8.0.endf", 24.305),
    ("Al27", "n-013_Al_027-ENDF8.0.endf", 26.982),
    ("Si28", "n-014_Si_028-ENDF8.0.endf", 28.085),
    ("Fe56", "n-026_Fe_056-ENDF8.0.endf", 55.845),
];

/// Reconstruction tolerance. Looser than the 1e-3 used for criticality work
/// here: this case compares no number against the notebook, so grid fidelity
/// buys nothing and costs the memory that killed the first attempt.
const RECON_TOL: f64 = 5.0e-3;

fn load_nuclides() -> Option<Vec<Nuclide>> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let mut out = Vec::with_capacity(NUCLIDES.len());
    for (name, file, _) in NUCLIDES {
        let p = base.join(file);
        if !p.exists() {
            eprintln!("SKIP: {} not in this checkout", p.display());
            return None;
        }
        out.push(Nuclide::from_endf_file(&p, name, TEMP, RECON_TOL).ok()?);
    }
    Some(out)
}

fn index_of(name: &str) -> usize {
    NUCLIDES.iter().position(|(n, ..)| *n == name).unwrap()
}

/// Build a material from `(principal isotope, atom fraction)` pairs at a mass
/// density.
///
/// Atom densities come out in atoms/barn-cm, which is what this crate's
/// `NuclideComponent::atom_density` means (see the units table in the crate
/// `CLAUDE.md`) — a units error here is the kind that produces a plausible but
/// meaningless answer, so the conversion is written out:
/// `N_i = rho * N_A / M_bar * x_i * 1e-24`.
///
/// `M_bar` uses each **element's** molar mass, not its principal isotope's
/// mass, so the number density matches the notebook's composition even though
/// the isotopics do not.
fn material(id: i32, name: &str, density_g_cc: f64, elements: &[(&str, f64)]) -> Material {
    let m_bar: f64 = elements
        .iter()
        .map(|(iso, frac)| frac * NUCLIDES[index_of(iso)].2)
        .sum();
    let n_total = density_g_cc * AVOGADRO / m_bar * 1.0e-24; // atoms/barn-cm
    let components = elements
        .iter()
        .map(|(iso, frac)| NuclideComponent {
            nuclide_idx: index_of(iso),
            atom_density: n_total * frac,
        })
        .collect();
    Material {
        id,
        name: name.into(),
        components,
        temperature: TEMP,
    }
}

/// Material 0 = air (N and Ar folded into O), material 1 = concrete (K and Ca
/// folded into Si). See the module docs for the substitutions and their size.
fn materials() -> Vec<Material> {
    let air = material(
        1,
        "air (N,Ar -> O)",
        0.001205,
        &[("O16", 0.784431 + 0.210748 + 0.0046)],
    );
    let concrete = material(
        2,
        "concrete (K,Ca -> Si)",
        2.3,
        &[
            ("H1", 0.168759),
            ("C12", 0.001416),
            ("O16", 0.562524),
            ("Na23", 0.011838),
            ("Mg24", 0.0014),
            ("Al27", 0.021354),
            // Si, plus K (0.005656) and Ca (0.018674) folded in.
            ("Si28", 0.204115 + 0.005656 + 0.018674),
            ("Fe56", 0.00426),
        ],
    );
    vec![air, concrete]
}

/// `+lo & -hi` on each axis, as an RPN region.
fn boxed(xl: usize, xh: usize, yl: usize, yh: usize, zl: usize, zh: usize) -> Vec<RegionToken> {
    let half = |i: usize, sense: HalfSpaceSense| RegionToken::HalfSpace {
        surface_idx: i,
        sense,
    };
    let mut r = vec![
        half(xl, HalfSpaceSense::Outside),
        half(xh, HalfSpaceSense::Inside),
        RegionToken::Intersection,
    ];
    for (lo, hi) in [(yl, yh), (zl, zh)] {
        r.push(half(lo, HalfSpaceSense::Outside));
        r.push(RegionToken::Intersection);
        r.push(half(hi, HalfSpaceSense::Inside));
        r.push(RegionToken::Intersection);
    }
    r
}

fn geometry() -> Geometry {
    let mut surfaces = Vec::new();
    for (i, x0) in X.iter().enumerate() {
        let bc = if i == 0 || i == X.len() - 1 {
            BoundaryType::Vacuum
        } else {
            BoundaryType::Transmissive
        };
        surfaces.push(SurfaceKind::XPlane(XPlane { x0: *x0, bc }));
    }
    for (i, y0) in Y.iter().enumerate() {
        let bc = if i == 0 || i == Y.len() - 1 {
            BoundaryType::Vacuum
        } else {
            BoundaryType::Transmissive
        };
        surfaces.push(SurfaceKind::YPlane(YPlane { y0: *y0, bc }));
    }
    for (i, z0) in Z.iter().enumerate() {
        let bc = if i == 0 || i == Z.len() - 1 {
            BoundaryType::Vacuum
        } else {
            BoundaryType::Transmissive
        };
        surfaces.push(SurfaceKind::ZPlane(ZPlane { z0: *z0, bc }));
    }
    // Plane index helpers: x_i = i, y_i = 8 + i, z_i = 15 + (i - 1).
    let xi = |i: usize| i;
    let yi = |i: usize| 8 + i;
    let zi = |i: usize| 15 + (i - 1);

    const AIR: usize = 0;
    const CONCRETE: usize = 1;
    // (name, x range, y range, z range, material) — the notebook's fourteen
    // cells, in its order.
    let spec: [(&str, usize, usize, usize, usize, usize, usize, usize); 14] = [
        ("outside_bottom", 0, 7, 0, 1, 1, 4, AIR),
        ("outside_top", 0, 7, 5, 6, 1, 4, AIR),
        ("outside_left", 0, 1, 1, 5, 1, 4, AIR),
        ("outside_right", 6, 7, 1, 5, 1, 4, AIR),
        ("wall_left", 1, 2, 2, 4, 2, 3, CONCRETE),
        ("wall_right", 5, 6, 2, 5, 2, 3, CONCRETE),
        ("wall_top", 1, 4, 4, 5, 2, 3, CONCRETE),
        ("wall_bottom", 1, 6, 1, 2, 2, 3, CONCRETE),
        ("wall_middle", 3, 4, 3, 4, 2, 3, CONCRETE),
        ("room", 2, 3, 2, 4, 2, 3, AIR),
        ("gap", 3, 4, 2, 3, 2, 3, AIR),
        ("corridor", 4, 5, 2, 5, 2, 3, AIR),
        ("roof", 1, 6, 1, 5, 1, 2, CONCRETE),
        ("floor", 1, 6, 1, 5, 3, 4, CONCRETE),
    ];
    let cells = spec
        .iter()
        .enumerate()
        .map(|(i, (_, xl, xh, yl, yh, zl, zh, mat))| {
            Cell::material(
                i as i32 + 1,
                boxed(xi(*xl), xi(*xh), yi(*yl), yi(*yh), zi(*zl), zi(*zh)),
                *mat,
                TEMP,
            )
        })
        .collect::<Vec<_>>();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices: (0..14).collect(),
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// The tally mesh: the notebook's `RegularMesh.from_domain(geometry)`, coarser
/// (50 cm cells rather than ~3 cm) so this runs in test time.
fn mesh() -> RegularMesh {
    RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [1550.0, 1900.0, 700.0],
        dimension: [31, 38, 1],
    }
}

fn flux_tally() -> Tally {
    let m = mesh();
    let n = m.n_bins();
    Tally {
        id: 42,
        name: "flux tally".into(),
        filters: vec![FilterKind::Mesh(MeshFilter {
            mesh: MeshKind::Regular(m),
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n],
    }
}

fn source() -> FixedSource {
    FixedSource::Point {
        r: Position::new(550.0, 825.0, 350.0),
        energy_ev: 2.5e6,
    }
}

fn settings(n_particles: usize, seed: u64, vr: VarianceReduction) -> FixedSourceSettings {
    FixedSourceSettings {
        n_particles,
        n_batches: 10,
        temperature_k: TEMP,
        seed,
        variance_reduction: vr,
        ..FixedSourceSettings::default()
    }
}

/// Mesh bins covering the **deep** region: `outside_right`, behind 200 cm of
/// concrete (`wall_right`, x = 1250..1450). Nothing reaches here without
/// either penetrating the shield or streaming the whole corridor and coming
/// back round.
fn deep_bins() -> Vec<usize> {
    let m = mesh();
    let mut out = Vec::new();
    for j in 0..m.dimension[1] {
        for i in 0..m.dimension[0] {
            let x = (i as f64 + 0.5) * 1550.0 / m.dimension[0] as f64;
            let y = (j as f64 + 0.5) * 1900.0 / m.dimension[1] as f64;
            if x > 1450.0 && (100.0..1800.0).contains(&y) {
                out.push(i + m.dimension[0] * j);
            }
        }
    }
    out
}

fn n_scored(t: &Tally, bins: &[usize]) -> usize {
    bins.iter().filter(|&&b| t.bins[b].sum > 0.0).count()
}

/// Minimum number of scoring events a deep cell must carry before its variance
/// estimate is used in a figure of merit.
///
/// A bin hit once or twice has a sample variance dominated by its own sampling
/// noise: `rel_std_dev` on `n = 1` scores is not small-but-uncertain, it is
/// meaningless. Every cell below this threshold is reported by count and
/// excluded from the aggregate rather than being folded in at face value,
/// because a deep-penetration FOM is exactly where a handful of lucky scores
/// would otherwise manufacture an impressive-looking ratio.
const MIN_SCORES_FOR_FOM: u64 = 10;

/// Figure of merit for one tally bin, `FOM = 1 / (R^2 · t)` with `R` the
/// relative standard deviation — OpenMC's definition and the one #258's
/// acceptance criterion names.
///
/// `None` means "this cell cannot supply a FOM", which is a different
/// statement from "this cell's FOM is zero" and is kept distinct on purpose:
/// an unscored cell has an *undefined* FOM, and reporting it as `0.0` would
/// let it be divided into a ratio as though it had been measured.
fn fom(bin: &TallyBin, n_realizations: u64, t_seconds: f64) -> Option<f64> {
    if bin.count < MIN_SCORES_FOR_FOM || bin.sum <= 0.0 || t_seconds <= 0.0 {
        return None;
    }
    let rel = bin.rel_std_dev(n_realizations);
    if !rel.is_finite() || rel <= 0.0 {
        return None;
    }
    Some(1.0 / (rel * rel * t_seconds))
}

/// **LIVE**: the notebook's claim, tested. At matched wall-clock, the
/// weight-window run must resolve flux in mesh cells the analog run never
/// reaches.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs 8 nuclides from ENDF and runs four shielding transports (~15 min); runs by default"
)]
fn shielded_room_weight_window() {
    let Some(nucs) = load_nuclides() else {
        return; // tapes absent; `load_nuclides` already said which
    };
    let mats = materials();
    let geom = geometry();
    let src = source();
    let deep = deep_bins();
    println!("deep region: {} mesh cells beyond 200 cm of concrete", deep.len());

    // ── Arm 1: analog ──────────────────────────────────────────────────────
    let mut analog_tally = flux_tally();
    // Bound once, used twice: the FOM below divides by this same count, and a
    // literal repeated in two places is a silent wrong answer waiting to happen.
    let n_analog = 2_500usize;
    let t0 = Instant::now();
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &src,
        &settings(n_analog, 20_260_922, VarianceReduction::default()),
        Some(&mut analog_tally),
    );
    let t_analog = t0.elapsed().as_secs_f64();
    let analog_deep = n_scored(&analog_tally, &deep);
    let analog_total: f64 = analog_tally.bins.iter().map(|b| b.sum).sum();
    println!(
        "ANALOG   : {t_analog:.1} s, {analog_deep}/{} deep cells scored, total flux {analog_total:.3e}",
        deep.len()
    );

    // ── Stage 2a: a short run to fill a tally MAGIC can learn from ─────────
    let mut gen_tally = flux_tally();
    let t0 = Instant::now();
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &src,
        &settings(800, 7_919, VarianceReduction::default()),
        Some(&mut gen_tally),
    );
    let t_generate = t0.elapsed().as_secs_f64();

    let m = mesh();
    let n_m = m.n_bins();
    let sum: Vec<f64> = gen_tally.bins.iter().map(|b| b.sum).collect();
    let sum_sq: Vec<f64> = gen_tally.bins.iter().map(|b| b.sum_sq).collect();
    let cell_volume = (1550.0 / 31.0) * (1900.0 / 38.0) * 700.0;
    let mut ww = WeightWindows::new(
        m,
        vec![0.0, 2.0e7],
        vec![-1.0; n_m],
        vec![-1.0; n_m],
    )
    .unwrap();
    ww.update_magic(&sum, &sum_sq, &vec![cell_volume; n_m], 10, 1.0, 5.0)
        .unwrap();
    let n_valid = ww.lower.iter().filter(|&&l| l > 0.0).count();
    println!(
        "MAGIC    : {t_generate:.1} s to fill the tally; {n_valid}/{n_m} cells carry a window"
    );
    assert!(
        n_valid > 0,
        "MAGIC produced no windows at all — the generating run scored nothing, \
         so there is nothing to test"
    );

    // ── Arm 2: weight windows, at matched wall-clock ───────────────────────
    //
    // The notebook's own fairness criterion: spend the same total time. The
    // generation run counts against the weight-window arm's budget, because a
    // user who wants windows has to pay for them.
    let budget = (t_analog - t_generate).max(0.1);
    let mut ww_tally = flux_tally();
    let vr = VarianceReduction::default().with_weight_windows(ww);
    // Calibrate the particle count from a short timing probe rather than
    // guessing: a split-heavy run is much slower per source particle, and
    // guessing would make the "matched cost" claim untrue in whichever
    // direction flattered the result.
    let mut probe_tally = flux_tally();
    let t0 = Instant::now();
    let n_probe = 100usize;
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &src,
        &settings(n_probe, 13, vr.clone()),
        Some(&mut probe_tally),
    );
    let per_particle = t0.elapsed().as_secs_f64() / n_probe as f64;
    // **Hard particle cap as well as the time budget.**
    //
    // A first version of this test sized the weight-window arm from the time
    // budget alone, with a 200 000 upper clamp. That is unbounded in practice:
    // the probe's 200 particles are drawn from the source region, where the
    // MAGIC windows are dense and histories die quickly, so `per_particle`
    // underestimates the cost of a particle that actually gets steered into
    // the shield. The run then blows straight past its budget and the
    // `t_ww < 3 t_analog` assertion only fires HOURS later, after the damage.
    // That test ran 45 minutes before it was stopped and rewritten.
    //
    // The cap is 25x the probe, which bounds the arm at ~25x the probe's
    // measured wall-clock whatever the extrapolation says.
    let from_budget = (budget / per_particle) as usize;
    let n_ww = from_budget.clamp(50, 25 * n_probe);
    println!(
        "WW probe : {:.3} s per source particle; budget {budget:.1} s -> \
         {from_budget} particles, capped to {n_ww}",
        per_particle
    );

    let t0 = Instant::now();
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &src,
        &settings(n_ww, 20_260_922, vr),
        Some(&mut ww_tally),
    );
    let t_ww = t0.elapsed().as_secs_f64() + t_generate;
    let ww_deep = n_scored(&ww_tally, &deep);
    let ww_total: f64 = ww_tally.bins.iter().map(|b| b.sum).sum();
    println!(
        "WW       : {t_ww:.1} s total (incl. generation), {ww_deep}/{} deep cells scored, \
         total flux {ww_total:.3e}",
        deep.len()
    );

    println!(
        "\nRESULT: analog reached {analog_deep} deep cells in {t_analog:.1} s; \
         weight windows reached {ww_deep} in {t_ww:.1} s"
    );

    // ── Figure of merit, #258's third acceptance criterion ─────────────────
    //
    // `FOM = 1/(R^2 t)`. This is REPORTED, not asserted: the criterion asks
    // for a measured improvement, and a gate on it would be a threshold I had
    // chosen after seeing the number. The existing deep-cell assertion above
    // is the pass criterion; this block is the measurement.
    //
    // Three populations are kept apart, because collapsing them is how a
    // shielding FOM gets overstated:
    //
    //   * `both`     — cells where BOTH arms carry enough scores to estimate a
    //                  variance. Only these yield a ratio, and the aggregate
    //                  quoted is the GEOMETRIC mean, since a ratio's arithmetic
    //                  mean is dominated by whichever cell happened to do best.
    //   * `ww_only`  — cells the windows resolved and analog did not. The
    //                  honest statement is that the ratio is UNDEFINED here
    //                  (analog has no variance estimate to divide by), not
    //                  that it is infinite. Their count is the real result.
    //   * `neither`  — cells neither arm resolved. Windows did not help there
    //                  either, and saying so is part of the measurement.
    let mut ratios: Vec<f64> = Vec::new();
    let (mut ww_only, mut analog_only, mut neither) = (0usize, 0usize, 0usize);
    for &b in &deep {
        let fa = fom(&analog_tally.bins[b], n_analog as u64, t_analog);
        let fw = fom(&ww_tally.bins[b], n_ww as u64, t_ww);
        match (fa, fw) {
            (Some(a), Some(w)) => ratios.push(w / a),
            (None, Some(_)) => ww_only += 1,
            (Some(_), None) => analog_only += 1,
            (None, None) => neither += 1,
        }
    }
    println!(
        "\nFOM (1/(R^2 t)) over {} deep cells, min {MIN_SCORES_FOR_FOM} scores to qualify:",
        deep.len()
    );
    println!(
        "  both arms resolved : {:3}   windows only: {ww_only:3}   analog only: {analog_only:3}   \
         neither: {neither:3}",
        ratios.len()
    );
    if ratios.is_empty() {
        println!(
            "  FOM ratio: NOT MEASURABLE — no deep cell carries >= {MIN_SCORES_FOR_FOM} scores \
             in both arms, so there is no cell on which the ratio is defined. The \
             {ww_only} windows-only cells are the improvement; it cannot be expressed \
             as a FOM ratio without inventing a variance for the analog arm."
        );
    } else {
        let log_sum: f64 = ratios.iter().map(|r| r.ln()).sum();
        let geo = (log_sum / ratios.len() as f64).exp();
        let mut sorted = ratios.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let lo = sorted[0];
        let hi = sorted[sorted.len() - 1];
        let med = sorted[sorted.len() / 2];
        println!(
            "  FOM ratio (windows / analog): geometric mean {geo:.2}x, median {med:.2}x, \
             range {lo:.2}x - {hi:.2}x over {} cells",
            ratios.len()
        );
    }

    // The notebook's claim, and the only thing asserted: at matched cost the
    // weight-window run gets further. Written as `>=` plus a strict
    // improvement requirement only when the analog arm actually failed to
    // cover the region, because if analog already reaches everything there is
    // nothing for windows to improve and that is not a failure of the port.
    if analog_deep < deep.len() {
        assert!(
            ww_deep > analog_deep,
            "weight windows reached {ww_deep} deep cells against analog's {analog_deep} \
             at matched cost. The notebook's stated result is that they get FURTHER; \
             if they do not, the windows are not steering particles towards the \
             shield and the MAGIC bounds or the checkpoints are wrong."
        );
    } else {
        println!("NOTE: analog already covered every deep cell; nothing to improve.");
    }
    assert!(
        t_ww < 3.0 * t_analog,
        "the weight-window arm took {t_ww:.1} s against analog's {t_analog:.1} s; \
         that is not a matched-cost comparison"
    );
}
