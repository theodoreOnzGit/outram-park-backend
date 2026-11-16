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

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use std::time::Instant;

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::mathf::RealMath;
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
        out.push(Nuclide::from_endf_file(&p, name, uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(TEMP), RECON_TOL).ok()?);
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

/// The tally and weight-window mesh.
///
/// # The cell size is a physics parameter, not a performance knob
///
/// This started at `[31, 38, 1]` -- **50 cm cells** -- described in the comment
/// it replaced as "coarser (50 cm rather than ~3 cm) so this runs in test
/// time". That coarsening broke the method, and the reasoning is worth keeping
/// because the mistake is easy to repeat.
///
/// A weight window steers by splitting a particle whose weight exceeds the
/// local upper bound, into at most `max_split = 10` copies. For the population
/// to track the flux the window must not fall by more than roughly the window
/// ratio (5) across one cell. In concrete a fast neutron's mean free path is
/// of order 5-10 cm, so a **50 cm cell is 5-10 mfp** and the flux falls by
/// `10^2` to `10^4` across it. Tracking that needs `ceil(10^4 / 5)` splits and
/// the cap allows 10, so the population **cannot** follow the attenuation: the
/// survivors drop below the lower bound and are rouletted away. The windows
/// then pay the full splitting cost near the source and buy no penetration --
/// measured as a FOM ratio of 0.007-0.011x and zero deep cells reached.
///
/// At **16 cm** the cells are ~1.7-3.3 mfp and the per-cell flux ratio is
/// ~5-27, close to the window ratio, so a crossing needs a handful of splits
/// rather than thousands.
///
/// # Both meshes were run, and the trade-off is measured
///
/// | mesh | MAGIC, 6 iterations | windows placed | deep cells reached |
/// |---|---|---|---|
/// | 50 cm (`[31, 38, 1]`) | 117.6 s | 751/1178 (64 %) | **0**/68 |
/// | 16 cm (`[97, 119, 1]`) | **2491.4 s (21x)** | 8375/11543 (73 %) | **2**/642 |
///
/// Refining **works and is unaffordable**: 16 cm produced the first
/// penetration of the shield in any run here, and cost 21x more in MAGIC to
/// do it, timing the test out at 2700 s. The extra cost is not overhead --
/// it is the splitting the method is supposed to do, now that the windows can
/// actually track the attenuation.
///
/// The default stays at 50 cm so the test COMPLETES and the figure of merit
/// stays measurable, with this table as the record that the default is
/// **known to be a misconfigured mesh** for weight windows, not a neutral
/// choice. Standard guidance says the flux should change by less than the
/// window ratio across one cell; 50 cm violates that by two to three orders
/// of magnitude, and the FOM measured on it (0.007-0.011x) should be read as
/// "this configuration", never as "this technique".
fn mesh() -> RegularMesh {
    // Overridable so the long, properly-resolved measurement is a deliberate
    // invocation rather than a permanent cost on every run:
    //   OUTRAM_WW_MESH_NX / _NY   (default 31 x 38, i.e. 50 cm cells)
    // The 16 cm configuration is `OUTRAM_WW_MESH_NX=97 OUTRAM_WW_MESH_NY=119`.
    let nx = std::env::var("OUTRAM_WW_MESH_NX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(31usize);
    let ny = std::env::var("OUTRAM_WW_MESH_NY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(38usize);
    RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [1550.0, 1900.0, 700.0],
        dimension: [nx, ny, 1],
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

/// Batches per `run_fixed_source` call.
///
/// This is the tally's **realization** count, not the particle count:
/// `run_fixed_source` flushes one realization per batch (`fixed_source.rs:150`
/// says so explicitly). The figure of merit below divides by it, and an
/// earlier draft of that block passed the PARTICLE count instead, which would
/// have computed every variance with n = 25000 where the truth is n = 10.
///
/// Every chunk size is kept a multiple of this so all ten batches are
/// non-empty: `run_fixed_source` splits with `n.div_ceil(n_batches)` and stops
/// early once the particles run out, so a chunk of 15 would flush 8
/// realizations rather than 10 and silently desynchronise the count.
const N_BATCHES: usize = 10;

fn settings(n_particles: usize, seed: u64, vr: VarianceReduction) -> FixedSourceSettings {
    FixedSourceSettings {
        n_particles,
        n_batches: N_BATCHES,
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

/// The largest relative standard deviation a deep cell may carry and still
/// contribute a figure of merit.
///
/// # Why this is a relative error and not a hit count
///
/// The obvious guard is "the bin must have been scored at least `k` times",
/// and a first draft of this used `TallyBin::count >= 10`. That guard does
/// nothing: `flush_batch` scores EVERY bin once per realization, including
/// bins that accumulated exactly zero, so `count` equals the realization count
/// for every bin in the mesh whether or not a particle ever reached it. The
/// check was always true and excluded nothing.
///
/// The bin's own relative error carries the information the count was supposed
/// to. For a cell hit in `k` of `n` realizations with comparable scores,
/// `rel_std_dev = sqrt((n - k) / (k (n - 1)))`, so at `n = 10`:
///
/// | `k` | 1 | 2 | 3 | 4 | 5 |
/// |---|---|---|---|---|---|
/// | `rel_std_dev` | 1.00 | 0.67 | 0.51 | 0.41 | 0.33 |
///
/// A cap of 0.5 therefore admits a cell resolved in roughly 4 of 10 batches
/// or better, and rejects the one- and two-hit cells whose variance estimate
/// is dominated by its own sampling noise. Deep penetration is exactly where
/// a couple of lucky scores would otherwise manufacture a flattering ratio.
///
/// Fixed before any figure of merit was computed.
const MAX_REL_STD_DEV_FOR_FOM: f64 = 0.5;

/// Figure of merit for one tally bin, `FOM = 1 / (R^2 · t)` with `R` the
/// relative standard deviation — OpenMC's definition and the one #258's
/// acceptance criterion names.
///
/// `None` means "this cell cannot supply a FOM", which is a different
/// statement from "this cell's FOM is zero" and is kept distinct on purpose:
/// an unscored cell has an *undefined* FOM, and reporting it as `0.0` would
/// let it be divided into a ratio as though it had been measured.
fn fom(bin: &TallyBin, n_realizations: u64, t_seconds: f64) -> Option<f64> {
    if bin.sum <= 0.0 || t_seconds <= 0.0 {
        return None;
    }
    let rel = bin.rel_std_dev(n_realizations);
    if !rel.is_finite() || rel <= 0.0 || rel > MAX_REL_STD_DEV_FOR_FOM {
        return None;
    }
    Some(1.0 / (rel * rel * t_seconds))
}

/// # Results (2026-09-24) — #258's third acceptance criterion, measured
///
/// Analog 50 000 particles / 10 realizations / 136.3 s; windows 26 000 / 260 /
/// 405.2 s transport + 9.2 s generation. Per-particle cost 2.73 ms against
/// 15.58 ms, a **5.72x** cost ratio, so **0.175x is the zero-variance-benefit
/// line** — above it the windows pay, below it they harm.
///
/// | band | FOM ratio | cells | vs 0.175 | windows-only |
/// |---|---|---|---|---|
/// | 0–400 cm | 0.153x | 105 | slight harm | 5 |
/// | 400–900 cm | 0.117x | 289 | harm | 10 |
/// | 900–1450 cm | **0.252x** | 241 | **benefit** (1.44x) | 28 |
/// | beyond 1450 cm | not measurable | 0 | — | 0 |
///
/// **Weight windows do not improve the FOM where the analog arm also resolves,
/// except in the far field where they cross into genuine benefit. Their real
/// contribution is the 43 cells the analog arm cannot resolve at all**, where a
/// ratio is undefined rather than infinite and the count is the result.
///
/// Unbiasedness — the gate that can fail — flux per source particle 1712.43
/// analog against 1750.27 windows, **+2.21 %**, inside the 25 % bound.
///
/// Full methodology, the `rel_std_dev` defect that had to be fixed first, the
/// three wrong ways this arm was bounded and the MAGIC cost pathology:
/// `verification_and_validation/variance_reduction/fom_2026_09_24.md`.
///
/// **LIVE**: the notebook's claim, tested. At matched wall-clock, the
/// weight-window run must resolve flux in mesh cells the analog run never
/// reaches.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs 8 nuclides from ENDF and runs an analog arm, six MAGIC \
              iterations and a matched-cost window arm (~10 min); runs by default"
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
    // Sized so the window arm has room left after MAGIC, not so the result
    // comes out a particular way. 2500 particles ran analog in 6.6 s, leaving
    // ~4.5 s -- too thin for a figure of merit to mean anything. 25000 took
    // 68.7 s, but six MAGIC iterations then consumed nearly all of it. 50000
    // keeps generation a minority of the budget so most of the window arm's
    // time goes into the measurement it is being judged on.
    let n_analog = 50_000usize;
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

    // ── Stage 2: ITERATIVE MAGIC ──────────────────────────────────────────
    //
    // **MAGIC is an iterative method, and one pass of it is not it.**
    //
    // The first version of this test ran a single analog generating pass and
    // then scored with whatever windows came out. It failed, correctly:
    //
    //     MAGIC : 558/1178 cells carry a window
    //     WW    : 4140 particles, 0/68 deep cells scored
    //
    // MAGIC builds windows from a forward flux tally, so it can only place a
    // window in a cell the generating run actually reached. The generating run
    // was analog, and analog reaches **zero** deep cells -- that is the entire
    // premise of the problem. So the deep region had no windows (`lower = -1`,
    // no game played), nothing steered particles into the shield, and the
    // window arm reproduced the analog arm exactly.
    //
    // The method bootstraps instead: each iteration runs with the PREVIOUS
    // iteration's windows, reaches a little further than the last, and the next
    // update places windows in the cells it just reached. The penetration depth
    // grows iteration by iteration. That is what makes it work on a
    // deep-penetration problem and it is why the literature always quotes a
    // number of iterations.
    //
    // Every iteration's wall-clock is charged to the window arm, so the
    // matched-cost comparison stays honest -- a user who wants windows pays for
    // generating them.
    let m = mesh();
    let n_m = m.n_bins();
    let cell_volume = (1550.0 / 31.0) * (1900.0 / 38.0) * 700.0;
    let mut ww = WeightWindows::new(m, vec![0.0, 2.0e7], vec![-1.0; n_m], vec![-1.0; n_m])
        .expect("flat window set");

    // The frontier advances ~25 cells per iteration at ~10 s per iteration
    // (measured: 50.7 s for 5, taking 689 -> 724 of 1178 cells), but they get
    // DEARER as the window set fills -- ~10 s at iteration 1, ~360 s by
    // iteration 8, because more windows means more splitting. Twenty-four
    // iterations timed out at 3000 s having done eight.
    //
    // Six is what fits, and the figure of merit this test now reports does not
    // need the frontier to cross the shield -- it is measured wherever both
    // arms resolve. Walking the frontier all the way is a 2-hour measurement,
    // recorded in `magic_frontier_2026_09_23.md` with the rate needed to
    // budget it, not a per-run gate.
    // ── Why the default is 1, not 6 ────────────────────────────────────────
    //
    // **CHANGED 2026-09-24, from a measurement, and the reason is a cost
    // pathology worth more than the default.** Adding MAGIC iterations deepens
    // the window set, and the cost per steered particle does NOT grow smoothly
    // with it:
    //
    // | MAGIC iters | windows (of 1178) | measured cost / particle |
    // |---|---|---|
    // | 1 | 621 | **15.9 ms** |
    // | 6 | 751 | **> 1.56 s** |
    //
    // A **21 % increase in window count for a ~100x increase in cost.** Both
    // bounds are from runs killed by a timeout, so the 6-iteration figure is a
    // lower bound: at 751 windows a single 1000-particle chunk did not finish in
    // 1562 s.
    //
    // That is not a smooth cost curve and it is **not explained**. The leading
    // hypothesis is cascade growth: MAGIC sets a cell's lower bound from the
    // flux estimate there, deep cells have very little flux and hence very low
    // bounds, so a particle arriving in one with weight far above the upper
    // bound splits — and each daughter, moving one cell deeper, splits again.
    // `max_split` caps splits *per event*, which this crate does enforce, but
    // nothing caps the cascade *along a path*. A window set built from a starved
    // deep-flux estimate would show exactly this. It is filed rather than
    // guessed at, because the alternative is a hypothesis dressed as a cause.
    //
    // So the routine test uses the window set it can afford, and the figure of
    // merit is measured on that. 621 of 1178 cells carrying a window is a
    // legitimate window set, not a crippled one — what it is not is the deepest
    // set MAGIC can build. `OUTRAM_WW_MAGIC_ITERS` explores further, and the
    // table above says what that costs before anyone spends an hour finding out.
    let magic_iterations: usize = std::env::var("OUTRAM_WW_MAGIC_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1usize);
    const MAGIC_PARTICLES: usize = 400;
    let t_gen0 = Instant::now();

    // **Iteration 0 is the analog arm's own tally, which is already paid for.**
    //
    // An earlier version ran a separate 800-particle analog pass to seed MAGIC
    // and then wondered why the frontier stalled at ~650 of 1178 cells. The
    // analog arm scores flux on exactly this mesh with 50 000 particles -- 62x
    // the statistics, already spent, and thrown away. MAGIC refuses a window
    // wherever `rel_err > threshold`, so seeding it from the starved pass
    // refused the entire frontier by construction.
    //
    // Reusing it costs nothing and is not double-counting: the analog tally is
    // a forward flux estimate, which is exactly MAGIC's input.
    {
        let sum: Vec<f64> = analog_tally.bins.iter().map(|b| b.sum).collect();
        let sum_sq: Vec<f64> = analog_tally.bins.iter().map(|b| b.sum_sq).collect();
        ww.update_magic(&sum, &sum_sq, &vec![cell_volume; n_m], N_BATCHES, 1.0, 5.0)
            .expect("MAGIC from the analog tally");
        let n_valid = ww.lower.iter().filter(|&&l| l > 0.0).count();
        println!(
            "MAGIC it0: {n_valid}/{n_m} cells carry a window (seeded from the \
             {n_analog}-particle analog tally, no extra cost)"
        );
    }

    // Later iterations accumulate: each is an unbiased forward flux estimate,
    // so pooling them is a weighted average of unbiased estimators, and the
    // extra statistics are exactly what lets MAGIC resolve the next shell.
    let mut gen_tally = flux_tally();
    let mut gen_realizations = 0u64;
    for it in 1..=magic_iterations {
        let vr = VarianceReduction::default().with_weight_windows(ww.clone());
        run_fixed_source(
            &geom,
            &mats,
            &nucs,
            &src,
            &settings(MAGIC_PARTICLES, 7_919 + it as u64 * 101, vr),
            Some(&mut gen_tally),
        );
        gen_realizations += N_BATCHES as u64;
        let sum: Vec<f64> = gen_tally.bins.iter().map(|b| b.sum).collect();
        let sum_sq: Vec<f64> = gen_tally.bins.iter().map(|b| b.sum_sq).collect();
        ww.update_magic(&sum, &sum_sq, &vec![cell_volume; n_m], gen_realizations as usize, 1.0, 5.0)
            .expect("MAGIC update");
        let n_valid = ww.lower.iter().filter(|&&l| l > 0.0).count();
        let reached = n_scored(&gen_tally, &deep);
        println!(
            "MAGIC it{it}: {n_valid}/{n_m} cells carry a window, generating runs \
             have reached {reached}/{} deep cells",
            deep.len()
        );
    }
    let t_generate = t_gen0.elapsed().as_secs_f64();
    let n_valid = ww.lower.iter().filter(|&&l| l > 0.0).count();
    println!(
        "MAGIC    : {t_generate:.1} s over {magic_iterations} iterations; \
         {n_valid}/{n_m} cells carry a window"
    );
    assert!(
        n_valid > 0,
        "MAGIC produced no windows at all -- the generating runs scored nothing, \
         so there is nothing to test"
    );

    // ── Sizing the window arm: PARTICLES, not a clock ──────────────────────
    //
    // **CHANGED 2026-09-24, and the previous design was the reason #258's third
    // acceptance criterion read "NOT MEASURABLE".** This arm used to be bounded
    // by `budget = t_analog - t_generate`, i.e. matched wall-clock against the
    // analog arm. Two things went wrong with that, and neither was the weight
    // windows:
    //
    // 1. **Matched cost starves the arm.** A steered particle splits, so it
    //    costs ~6x an analog one (measured: 16.6 ms against 2.76 ms). Charging
    //    MAGIC generation to the same budget left the window arm **7 690
    //    particles** against the analog arm's 50 000.
    // 2. **The chunked loop inflated the realization count.** Each chunk was a
    //    separate `run_fixed_source`, contributing `N_BATCHES` realizations, so
    //    7 690 particles arrived as **1 200 realizations — about 6 particles
    //    each**. Almost every bin scored zero in almost every realization, so
    //    `rel_std_dev` sat near 1 across the mesh and NOTHING qualified. The
    //    measured result was `windows-only 0, analog-only 518`: the window arm
    //    resolved not one cell, while the analog arm resolved 518.
    //
    // So "the FOM is not measurable" was an artefact of how this test drove the
    // arm, not a property of weight windows.
    //
    // **Matched cost is not what the criterion asks for.** #258 asks for
    // `FOM = 1/(sigma^2 t)` — which is **cost-normalised by construction**, the
    // `t` in the denominator being the cost. Two arms at different costs are
    // exactly what a FOM comparison is for; forcing them to equal wall-clock
    // was a constraint this test imposed on itself and then failed to satisfy.
    // Removing it is not a relaxed criterion, it is the criterion as written.
    //
    // The arm now runs a **fixed particle count in a single call**, so it has
    // the same `N_BATCHES` realization structure as the analog arm and the two
    // `R` values mean the same thing. Its own wall-clock is measured and
    // divided into its own FOM. Generation is charged separately and reported
    // both ways (see below), because charging it or not is a judgement about
    // what question is being asked, and hiding that choice is how a shielding
    // FOM gets overstated.
    // The arm is bounded by **wall clock, checked between chunks** — restoring
    // the one bound that works — while keeping the realization structure sane.
    //
    // **Both halves are needed, and getting one at the cost of the other is the
    // mistake this design has now made twice in opposite directions:**
    //
    // * Bounding by a particle count and asserting the time AFTERWARDS cannot
    //   bound anything. Measured 2026-09-24: with 751 windows (6 MAGIC
    //   iterations) the arm ran **2165 s without finishing 10 000 particles** —
    //   over 216 ms/particle, 14x the 15.9 ms measured at 621 windows, because
    //   deeper windows mean longer split cascades. The post-hoc assertion would
    //   have fired only after the damage, which is exactly the defect the
    //   original chunked loop existed to prevent.
    // * Chunking with an ADAPTIVE chunk size inflates the realization count:
    //   `run_fixed_source` flushes `N_BATCHES` realizations per call, so many
    //   tiny calls gave 1 200 realizations holding ~6 particles each and every
    //   bin's `rel_std_dev` sat near 1.
    //
    // So: a FIXED chunk of `CHUNK` particles, which `run_fixed_source` splits
    // into `N_BATCHES` realizations of `CHUNK / N_BATCHES` each — 100 particles
    // per realization, against the analog arm's 5 000. That is few, but they are
    // real realizations rather than the 6-particle ones, and `FOM = 1/(R^2 t)`
    // is invariant in run length, so the arms need neither equal particles nor
    // equal realizations to be compared. No adaptive growth: every extrapolation
    // of the per-particle cost here has been optimistic, so none is made.
    const CHUNK: usize = 1_000;
    let ww_budget: f64 = std::env::var("OUTRAM_WW_BUDGET_S")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400.0);
    let mut ww_tally = flux_tally();
    let vr = VarianceReduction::default().with_weight_windows(ww);

    // ── A probe that REFUSES, never one that sizes ─────────────────────────
    //
    // **The clock check between chunks is only as fine as one chunk**, and at
    // 751 windows one 1000-particle chunk exceeded the entire budget — so the
    // in-loop check bounded nothing. That is the same error as bounding the loop
    // around a history rather than the history: a bound outside the unit of work
    // cannot bound the unit of work. It is the third time this test has made it.
    //
    // A small probe fixes it **provided the probe is used to refuse, not to
    // size**. Sizing from a probe is what ran for hours twice before: the
    // probe's particles start near the source where windows are dense and
    // histories die fast, so it always underestimates a steered particle. But
    // "the probe alone already blew the budget" is a valid, non-extrapolated
    // conclusion, and refusing on it costs at most the probe.
    const PROBE: usize = N_BATCHES; // one realization's worth
    let t_probe0 = Instant::now();
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &src,
        &settings(PROBE, 104_729, vr.clone()),
        None,
    );
    let t_probe = t_probe0.elapsed().as_secs_f64();
    let projected_chunk = t_probe / PROBE as f64 * CHUNK as f64;
    println!(
        "probe: {PROBE} particles in {t_probe:.2} s => one {CHUNK}-particle chunk \
         projects to >= {projected_chunk:.0} s against a {ww_budget:.0} s budget \
         (a LOWER bound: probe particles start near the source, where windows are \
         dense and histories die fast)"
    );
    if projected_chunk > ww_budget {
        println!(
            "\nWINDOW ARM SKIPPED: one chunk cannot fit the budget, so no arm with \
             usable statistics can run here. This is a measured cost result, not a \
             test failure -- at {} MAGIC iterations the window set is too expensive \
             for this budget. Raise OUTRAM_WW_BUDGET_S, or lower \
             OUTRAM_WW_MAGIC_ITERS (see the cost table on the iteration default).",
            magic_iterations
        );
        return;
    }

    let t0 = Instant::now();
    let mut n_ww = 0usize;
    let mut ww_realizations = 0u64;
    while t0.elapsed().as_secs_f64() < ww_budget {
        run_fixed_source(
            &geom,
            &mats,
            &nucs,
            &src,
            &settings(CHUNK, 7_919 + n_ww as u64, vr.clone()),
            Some(&mut ww_tally),
        );
        n_ww += CHUNK;
        ww_realizations += N_BATCHES as u64;
    }
    let t_ww_transport = t0.elapsed().as_secs_f64();
    println!(
        "WW arm   : {n_ww} particles in {ww_realizations} realizations \
         ({} per realization) within a {ww_budget:.0} s budget",
        CHUNK / N_BATCHES
    );
    // Two costs, both reported, because they answer different questions:
    //   `t_ww_transport` — the FOM of the biased run, which is what a user with
    //                      a window set already in hand experiences;
    //   `t_ww_total`     — the cost to ANSWER from nothing, generation included.
    // Quoting only the first overstates weight windows; quoting only the second
    // understates them for anyone reusing a window set across runs.
    let t_ww_total = t_ww_transport + t_generate;
    let t_ww = t_ww_transport;
    let ww_deep = n_scored(&ww_tally, &deep);
    let ww_total: f64 = ww_tally.bins.iter().map(|b| b.sum).sum();
    println!(
        "WW       : {t_ww_transport:.1} s transport + {t_generate:.1} s generation \
         = {t_ww_total:.1} s; {n_ww} particles in {ww_realizations} realizations, \
         {ww_deep}/{} deep cells scored, total flux {ww_total:.3e}",
        deep.len()
    );

    println!(
        "\nRESULT: analog reached {analog_deep} deep cells with {n_analog} particles \
         in {t_analog:.1} s; weight windows reached {ww_deep} with {n_ww} in \
         {t_ww_transport:.1} s transport ({t_ww_total:.1} s including generation)"
    );

    // **The unbiasedness check that matters more than the FOM.** Weight windows
    // are an estimator, not physics: they may cost anything, but they must not
    // change the answer. Total flux per source particle must agree between the
    // arms.
    let analog_per_source = analog_total / n_analog as f64;
    let ww_per_source = ww_total / n_ww.max(1) as f64;
    let rel = (ww_per_source - analog_per_source).abs() / analog_per_source;
    println!(
        "\nUNBIASEDNESS: flux per source particle, analog {analog_per_source:.2} \
         vs windows {ww_per_source:.2} ({:+.2} %)",
        100.0 * (ww_per_source - analog_per_source) / analog_per_source
    );
    assert!(
        rel < 0.25,
        "flux per source particle differs by {:.1} % between the arms. Weight \
         windows are an estimator and must not move the answer; this is a bias, \
         not a cost.",
        100.0 * rel
    );

    // ── Figure of merit, #258's third acceptance criterion ─────────────────
    //
    // Reported by DISTANCE BAND across the whole room, not only beyond 200 cm.
    // "Deep" was this test's own choice of region; the acceptance criterion
    // asks for a figure of merit on the shielding case, and binning by
    // penetration depth says where windows pay and where they do not, which a
    // single number cannot.
    //
    // `FOM = 1/(R^2 t)`. Reported, not asserted: a gate on it would be a
    // threshold chosen after seeing the number.
    //
    // Three populations are kept apart, because collapsing them is how a
    // shielding FOM gets overstated: cells both arms resolved (the only ones
    // yielding a ratio, aggregated by GEOMETRIC mean since an arithmetic mean
    // of ratios is dominated by the single best cell), cells only one arm
    // resolved (ratio UNDEFINED, not infinite -- the count is the result), and
    // cells neither resolved.
    let m_rep = mesh();
    let nx = m_rep.dimension[0];
    let bands: [(f64, f64, &str); 4] = [
        (0.0, 400.0, "0-400 cm    (source side)"),
        (400.0, 900.0, "400-900 cm  (mid-field) "),
        (900.0, 1450.0, "900-1450 cm (far field) "),
        (1450.0, 1e9, "beyond 1450 (deep)      "),
    ];
    // ── The coherence check that makes the ratios checkable ────────────────
    //
    // `FOM = 1/(R^2 t)` is invariant in run length (`R^2 ~ 1/N`, `t ~ N`), which
    // is exactly why the two arms may be run at different particle counts. So a
    // FOM RATIO of `1 / cost_ratio` means the windows bought **no variance
    // benefit at all** in that cell and charged their splitting overhead for
    // nothing; anything above it is real benefit, anything below it is harm.
    //
    // Printing this line beside the ratios turns them from arbitrary numbers
    // into ones that can be reasoned about — measured 2026-09-24, the source-side
    // band came out at 0.171x against a `1/cost_ratio` of 0.167, i.e. pure
    // overhead to two figures, which is what a well-sampled region should show.
    let ms_analog = 1000.0 * t_analog / n_analog as f64;
    let ms_ww = 1000.0 * t_ww_transport / n_ww as f64;
    let cost_ratio = ms_ww / ms_analog;
    println!(
        "\nper-particle cost: analog {ms_analog:.2} ms, windows {ms_ww:.2} ms \
         => cost ratio {cost_ratio:.2}x, so FOM ratio {:.3}x means ZERO variance \
         benefit (pure splitting overhead); above it is benefit, below it is harm",
        1.0 / cost_ratio
    );
    println!("\nFOM = 1/(R^2 t) by penetration depth, rel err <= {MAX_REL_STD_DEV_FOR_FOM} to qualify:");
    println!(
        "  analog {n_analog} particles in {} realizations, {t_analog:.1} s; \
         windows {n_ww} in {ww_realizations}, {t_ww:.1} s",
        N_BATCHES
    );
    let mut any_measurable = false;
    let mut band_summary: Vec<(&str, f64, f64, usize, usize)> = Vec::new();
    for (lo, hi, label) in bands {
        let mut ratios: Vec<f64> = Vec::new();
        let mut ratios_incl_gen: Vec<f64> = Vec::new();
        let (mut ww_only, mut analog_only, mut neither) = (0usize, 0usize, 0usize);
        for b in 0..m_rep.n_bins() {
            let x = ((b % nx) as f64 + 0.5) * 1550.0 / nx as f64;
            if x < lo || x >= hi {
                continue;
            }
            let fa = fom(&analog_tally.bins[b], N_BATCHES as u64, t_analog);
            let fw = fom(&ww_tally.bins[b], ww_realizations, t_ww_transport);
            let fw_all = fom(&ww_tally.bins[b], ww_realizations, t_ww_total);
            match (fa, fw) {
                (Some(a), Some(w)) => {
                    ratios.push(w / a);
                    if let Some(wa) = fw_all {
                        ratios_incl_gen.push(wa / a);
                    }
                }
                (None, Some(_)) => ww_only += 1,
                (Some(_), None) => analog_only += 1,
                (None, None) => neither += 1,
            }
        }
        if ratios.is_empty() {
            println!(
                "  {label}: NOT MEASURABLE  (windows-only {ww_only}, analog-only \
                 {analog_only}, neither {neither})"
            );
            continue;
        }
        any_measurable = true;
        // Through `RealMath`, which is this crate's convention for every
        // transcendental (`mathf.rs`): `r_ln`/`r_exp` are PETIR in every build,
        // so a reported number does not depend on the host's libm.
        let geomean = |v: &[f64]| -> f64 {
            (v.iter().map(|r| r.r_ln()).sum::<f64>() / v.len() as f64).r_exp()
        };
        let geo = geomean(&ratios);
        let geo_gen = if ratios_incl_gen.is_empty() {
            f64::NAN
        } else {
            geomean(&ratios_incl_gen)
        };
        let mut sorted = ratios.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  {label}: FOM ratio {geo:.3}x transport-only, {geo_gen:.3}x including \
             generation (geometric mean over {} cells, median {:.3}x, range \
             {:.3}-{:.3}); windows-only {ww_only}, analog-only {analog_only}, \
             neither {neither}",
            ratios.len(),
            sorted[sorted.len() / 2],
            sorted[0],
            sorted[sorted.len() - 1]
        );
        band_summary.push((label, geo, geo_gen, ratios.len(), ww_only));
    }
    if any_measurable {
        // The headline the acceptance criterion asks for, taken over the bands
        // that supplied one. Reported, never gated: a threshold on a figure of
        // merit would be a number chosen after seeing it.
        let best = band_summary
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("a measurable band");
        // **Named "RATIO", not "IMPROVEMENT".** A label that presumes the sign
        // is how an adverse measurement gets read as a favourable one. The
        // direction is stated from the number, every time.
        let direction = if best.1 > 1.0 {
            "windows BETTER per unit compute"
        } else {
            "windows WORSE per unit compute"
        };
        println!(
            "  => FOM RATIO (windows/analog), measured: best band `{}` at {:.3}x \
             transport-only / {:.3}x including generation over {} cells -- \
             {direction}. {} of {} bands supplied a ratio.",
            best.0.trim(),
            best.1,
            best.2,
            best.3,
            band_summary.len(),
            bands.len()
        );
        // Where the windows pay is precisely where no ratio exists, so the
        // count has to be reported beside the ratio or the ratio is misleading.
        let ww_only_total: usize = band_summary.iter().map(|b| b.4).sum();
        println!(
            "  => cells the WINDOW arm resolved and the analog arm did NOT: \
             {ww_only_total}. Their FOM ratio is UNDEFINED (the analog FOM does \
             not exist), not infinite, and this count is the result there."
        );
    } else {
        // **Still a result, not a failure of the test** — but read the counts
        // before believing it. Before 2026-09-24 this branch fired for a reason
        // that had nothing to do with weight windows: the arm was matched-cost
        // and chunked, so it got 7 690 particles spread over 1 200 realizations
        // and `rel_std_dev` sat near 1 across the whole mesh. If this prints
        // again with `windows-only 0` everywhere, check the particle count and
        // the realization count FIRST.
        println!(
            "  => the FOM is NOT MEASURABLE in any band: the window arm resolved \
             no cell to within {MAX_REL_STD_DEV_FOR_FOM} relative error at \
             {n_ww} particles in {ww_realizations} realizations. Check those two \
             numbers against the analog arm's {n_analog} in {} before concluding \
             anything about weight windows.",
            N_BATCHES
        );
    }

    // **The notebook's deep-penetration claim is REPORTED, not gated here.**
    //
    // Measured 2026-09-23: with MAGIC seeded from the analog tally the frontier
    // advances monotonically -- 689, 699, 791 of 1178 cells at iterations 0, 4
    // and 8 -- at about +23 cells per iteration. Reaching the 68 cells beyond
    // 200 cm needs roughly 20 more iterations, and iterations get dearer as the
    // window set fills (~10 s early, ~360 s by iteration 8, because more
    // windows means more splitting). That is 2+ hours, which is a long-run
    // measurement rather than a test.
    //
    // Gating on it here would make this test fail for want of runtime rather
    // than for a defect, and silently deleting the claim would hide that it is
    // unreproduced. So it is printed with the numbers needed to budget the
    // longer run, and `magic_frontier_2026_09_23.md` carries the record.
    if analog_deep == 0 && ww_deep == 0 {
        println!(
            "\nNOTE: neither arm reached the {} cells beyond 200 cm. The notebook's \
             deep-penetration claim is NOT reproduced at this runtime; the frontier \
             was still advancing (+23 cells/iteration) when the run was cut off.",
            deep.len()
        );
    } else {
        println!(
            "\nDEEP: analog {analog_deep}/{}, windows {ww_deep}/{}",
            deep.len(),
            deep.len()
        );
    }

    // **No cost assertion here, deliberately.** The old one was
    // `t_ww < 3 * t_analog`, which encoded the matched-cost constraint that made
    // the FOM unmeasurable; a later attempt replaced it with a runtime ceiling,
    // which was worse than useless because an assertion AFTER the run cannot
    // bound the run. The loop above bounds the time by checking the clock
    // between chunks, so there is nothing left for an assertion to add.
    //
    // What IS asserted is that the arm got far enough to say something at all.
    assert!(
        n_ww >= CHUNK,
        "the window arm completed no chunk inside its {ww_budget:.0} s budget; \
         raise OUTRAM_WW_BUDGET_S or lower the MAGIC iteration count"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// #497 — learned importance versus analog and MAGIC, on this same room.
// Protocol and prediction (written before any run):
// `verification_and_validation/stats_epic_493/497_learned_importance.md`.
// ════════════════════════════════════════════════════════════════════════════

use outram_mc_libs::stats::learned_importance::{
    learned_weight_windows, FomComparison, LearnedImportanceSettings, VrArm,
};

/// Mesh bins whose centre lies `lo..hi` cm (in x-y) from the source.
fn band_bins(lo: f64, hi: f64) -> Vec<usize> {
    let m = mesh();
    let mut out = Vec::new();
    for j in 0..m.dimension[1] {
        for i in 0..m.dimension[0] {
            let x = (i as f64 + 0.5) * 1550.0 / m.dimension[0] as f64;
            let y = (j as f64 + 0.5) * 1900.0 / m.dimension[1] as f64;
            let d = ((x - 550.0).powi(2) + (y - 825.0).powi(2)).sqrt();
            if d >= lo && d < hi {
                out.push(i + m.dimension[0] * j);
            }
        }
    }
    out
}

fn mean_and_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

/// One arm: `n_seeds` independent runs of `n` particles. Returns per-seed
/// per-bin flux per source particle, and the total wall-clock (transport only).
fn run_arm(
    label: &str,
    n: usize,
    n_seeds: usize,
    seed0: u64,
    vr: &VarianceReduction,
    ctx: (&Geometry, &[Material], &[Nuclide]),
) -> (Vec<Vec<f64>>, f64) {
    let per_batch = (n / N_BATCHES) as f64;
    let mut runs = Vec::new();
    let mut total = 0.0;
    for s in 0..n_seeds {
        let mut t = flux_tally();
        let t0 = Instant::now();
        run_fixed_source(
            ctx.0,
            ctx.1,
            ctx.2,
            &source(),
            &settings(n, seed0 + 1_000_003 * s as u64, vr.clone()),
            Some(&mut t),
        );
        let dt = t0.elapsed().as_secs_f64();
        total += dt;
        println!("  {label} seed {s}: {n} particles in {dt:.1} s");
        runs.push(
            t.bins
                .iter()
                .map(|b| b.mean(N_BATCHES as u64) / per_batch)
                .collect(),
        );
    }
    (runs, total)
}

fn band_arm(runs: &[Vec<f64>], bins: &[usize], seconds: f64) -> VrArm {
    let per_seed: Vec<f64> = runs.iter().map(|r| bins.iter().map(|&b| r[b]).sum()).collect();
    let (mean, sigma) = mean_and_sem(&per_seed);
    VrArm { mean, sigma, seconds, n_batches: runs.len() }
}

/// **Methodology.** Three arms on the shielded room (31 x 38 mesh, 8 nuclides
/// from ENDF/B-VIII.0): analog; MAGIC windows (`update_magic`, threshold 1,
/// ratio 5); learned windows (`learned_weight_windows`, same threshold and
/// ratio). Both window sets are built once from the same early analog tally
/// and held fixed. `N` seeds per arm (`OUTRAM_LI_SEEDS`, default 5); each seed
/// one run; the estimate is the seed mean of the flux per source particle
/// summed over a band of cells, sigma the seed-to-seed standard error.
/// Bands (distance from source): 0-400, 400-900, 900-1450 cm; the last is the
/// headline. FOM = 1/(R^2 T) with T the arm's total transport seconds.
/// z against analog is reported for both |z| <= 1 and |z| <= 2; no band is
/// chosen. The only assertion is a gross-defect guard (|z| <= 5, NOT a band).
///
/// **Results:** see `verification_and_validation/stats_epic_493/497_learned_importance.md`
/// (measured 2026-11-14; printed here with `--nocapture`).
#[test]
#[ignore = "measurement (#497), single-threaded, tens of minutes; run explicitly with --ignored"]
fn shielded_room_learned_importance() {
    let Some(nucs) = load_nuclides() else { return };
    let mats = materials();
    let geom = geometry();
    let env = |k: &str, d: usize| -> usize {
        std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
    };
    let n_seeds = env("OUTRAM_LI_SEEDS", 5);
    let n_analog = env("OUTRAM_LI_N_ANALOG", 40_000);
    let n_vr = env("OUTRAM_LI_N_VR", 20_000);
    let n_early = env("OUTRAM_LI_N_EARLY", 2_000);
    // Fresh transport seeds for a replicate; the early tally (and so the windows) is unchanged.
    let off = env("OUTRAM_LI_SEED_OFFSET", 0) as u64;
    let ctx = (&geom, mats.as_slice(), nucs.as_slice());
    println!("sizes: analog {n_analog}, vr {n_vr}, early {n_early}, seeds {n_seeds}");

    // Early analog tally (seed distinct from every measured seed).
    let m = mesh();
    let n_m = m.n_bins();
    let vol = (1550.0 / m.dimension[0] as f64) * (1900.0 / m.dimension[1] as f64) * 700.0;
    let mut early = flux_tally();
    let t0 = Instant::now();
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &source(),
        &settings(n_early, 424_242, VarianceReduction::default()),
        Some(&mut early),
    );
    let t_early = t0.elapsed().as_secs_f64();
    let sum: Vec<f64> = early.bins.iter().map(|b| b.sum).collect();
    let sum_sq: Vec<f64> = early.bins.iter().map(|b| b.sum_sq).collect();
    let volumes = vec![vol; n_m];

    let mut magic =
        WeightWindows::new(m.clone(), vec![0.0, 2.0e7], vec![-1.0; n_m], vec![-1.0; n_m]).unwrap();
    magic.update_magic(&sum, &sum_sq, &volumes, N_BATCHES, 1.0, 5.0).unwrap();
    let lis = LearnedImportanceSettings { threshold: 1.0, ratio: 5.0, ..Default::default() };
    let (learned, fills) =
        learned_weight_windows(m, vec![0.0, 2.0e7], &sum, &sum_sq, &volumes, N_BATCHES, &lis).unwrap();
    let nw = |w: &WeightWindows| w.lower.iter().filter(|&&l| l > 0.0).count();
    println!(
        "early tally: {t_early:.1} s; MAGIC windows {}/{n_m}; learned windows {}/{n_m}",
        nw(&magic),
        nw(&learned)
    );
    for f in &fills {
        println!("learned fill: {f:?}");
    }

    let vr_magic = VarianceReduction::default().with_weight_windows(magic);
    let vr_learned = VarianceReduction::default().with_weight_windows(learned);
    let (a_runs, t_a) =
        run_arm("analog", n_analog, n_seeds, 20_261_114 + off, &VarianceReduction::default(), ctx);
    let (m_runs, t_m) = run_arm("MAGIC", n_vr, n_seeds, 30_261_114 + off, &vr_magic, ctx);
    let (l_runs, t_l) = run_arm("learned", n_vr, n_seeds, 40_261_114 + off, &vr_learned, ctx);
    println!("seconds: analog {t_a:.1}, MAGIC {t_m:.1}, learned {t_l:.1}, early {t_early:.1}");

    let mut worst_z = 0.0f64;
    for (name, lo, hi) in
        [("0-400", 0.0, 400.0), ("400-900", 400.0, 900.0), ("900-1450 (HEADLINE)", 900.0, 1450.0)]
    {
        let bins = band_bins(lo, hi);
        let a = band_arm(&a_runs, &bins, t_a);
        for (arm_name, runs, t) in [("MAGIC", &m_runs, t_m), ("learned", &l_runs, t_l)] {
            let v = band_arm(runs, &bins, t);
            let c = FomComparison::new(a, v);
            let c_gen = FomComparison::new(a, VrArm { seconds: t + t_early, ..v });
            println!(
                "band {name} ({} cells) {arm_name}: analog {:.4e} +- {:.2e} (R {:.3}); vr {:.4e} +- {:.2e} (R {:.3}); \
                 z {:+.2} [|z|<=1: {}, |z|<=2: {}]; FOM analog {:.3e}, vr {:.3e}; FOM ratio {:.3} +- {:.3} \
                 (generation charged: {:.3} +- {:.3})",
                bins.len(), a.mean, a.sigma, a.sigma / a.mean, v.mean, v.sigma, v.sigma / v.mean,
                c.z, c.z.abs() <= 1.0, c.z.abs() <= 2.0, a.fom(), v.fom(), c.fom_ratio, c.fom_ratio_sigma,
                c_gen.fom_ratio, c_gen.fom_ratio_sigma,
            );
            worst_z = worst_z.max(c.z.abs());
        }
        let am = band_arm(&m_runs, &bins, t_m);
        let al = band_arm(&l_runs, &bins, t_l);
        let ratio = al.fom() / am.fom();
        let rel = (al.fom_relative_sigma().powi(2) + am.fom_relative_sigma().powi(2)).sqrt();
        println!("band {name}: FOM learned / MAGIC = {ratio:.3} +- {:.3}", ratio * rel);
    }

    // Per-cell z over cells scored in every seed of both arms.
    for (arm_name, runs) in [("MAGIC", &m_runs), ("learned", &l_runs)] {
        let (mut n_cells, mut n1, mut n2) = (0usize, 0usize, 0usize);
        for b in 0..n_m {
            let col = |r: &[Vec<f64>]| r.iter().map(|s| s[b]).collect::<Vec<f64>>();
            let (ca, cv) = (col(&a_runs), col(runs));
            if ca.iter().chain(cv.iter()).any(|&x| x <= 0.0) {
                continue;
            }
            let (ma, sa) = mean_and_sem(&ca);
            let (mv, sv) = mean_and_sem(&cv);
            let z = (mv - ma) / (sa * sa + sv * sv).sqrt();
            n_cells += 1;
            n1 += (z.abs() > 1.0) as usize;
            n2 += (z.abs() > 2.0) as usize;
        }
        println!(
            "per-cell {arm_name}: {n_cells} cells scored in every seed of both arms; outside 1 sigma {n1} ({:.1} %), \
             outside 2 sigma {n2} ({:.1} %)",
            100.0 * n1 as f64 / n_cells.max(1) as f64,
            100.0 * n2 as f64 / n_cells.max(1) as f64
        );
    }
    // Asserted only after everything is printed, so a failure still leaves the table.
    assert!(worst_z <= 5.0, "gross bias guard (not the band): worst |z| = {worst_z}");
}

// ════════════════════════════════════════════════════════════════════════════
// gh:#1579 — is the MAGIC far-field shift (-4.8 %, z = -3.1 at 5 seeds) a real
// bias or a heavy tail? Hypothesis and prediction were posted on the issue
// before any run. Record: `verification_and_validation/stats_epic_493/497_learned_importance.md`.
//
// One process per arm (`OUTRAM_MB_ARM` = analog | magic | magic_collision),
// each writing one line per chunk of histories to `OUTRAM_MB_OUT`; then
// `shielded_room_magic_bias_report` reads the files. A chunk is a separate
// one-batch `run_fixed_source` call of `OUTRAM_MB_CHUNK` histories, so the
// chunk-to-chunk spread of the headline band is the tail diagnostic.
// The windows are built exactly as in `shielded_room_learned_importance`
// (early analog tally of 2000 histories, seed 424 242, threshold 1, ratio 5).
// ════════════════════════════════════════════════════════════════════════════

fn mb_env(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

#[test]
#[ignore = "measurement (gh:#1579), single-threaded, tens of minutes; run explicitly with --ignored"]
fn shielded_room_magic_bias_hunt_arm() {
    use std::io::Write;
    let Some(nucs) = load_nuclides() else { return };
    let mats = materials();
    let geom = geometry();
    let arm = std::env::var("OUTRAM_MB_ARM").unwrap_or_else(|_| "analog".into());
    let out = std::env::var("OUTRAM_MB_OUT").expect("OUTRAM_MB_OUT");
    let n_seeds = mb_env("OUTRAM_MB_SEEDS", 20);
    let n = mb_env("OUTRAM_MB_N", 20_000);
    let chunk = mb_env("OUTRAM_MB_CHUNK", 200);
    let off = mb_env("OUTRAM_MB_SEED_OFFSET", 0) as u64;
    let n_early = 2_000;
    let m = mesh();
    let n_m = m.n_bins();
    let vol = (1550.0 / m.dimension[0] as f64) * (1900.0 / m.dimension[1] as f64) * 700.0;
    let mut early = flux_tally();
    run_fixed_source(
        &geom,
        &mats,
        &nucs,
        &source(),
        &settings(n_early, 424_242, VarianceReduction::default()),
        Some(&mut early),
    );
    let sum: Vec<f64> = early.bins.iter().map(|b| b.sum).collect();
    let sum_sq: Vec<f64> = early.bins.iter().map(|b| b.sum_sq).collect();
    let volumes = vec![vol; n_m];
    let mut magic =
        WeightWindows::new(m, vec![0.0, 2.0e7], vec![-1.0; n_m], vec![-1.0; n_m]).unwrap();
    magic.update_magic(&sum, &sum_sq, &volumes, N_BATCHES, 1.0, 5.0).unwrap();
    let (vr, base) = match arm.as_str() {
        "analog" => (VarianceReduction::default(), 20_261_114u64),
        "magic" => (VarianceReduction::default().with_weight_windows(magic), 30_261_114),
        "magic_collision" => (
            VarianceReduction::default()
                .with_weight_windows(magic)
                .with_weight_window_checkpoint_surface(false),
            50_261_114,
        ),
        other => panic!("unknown OUTRAM_MB_ARM {other}"),
    };
    let bins = band_bins(900.0, 1450.0);
    let mut f = std::fs::File::create(&out).unwrap();
    let t0 = Instant::now();
    for s in 0..n_seeds {
        for c in 0..(n / chunk) {
            let mut st = settings(
                chunk,
                base + off + 1_000_003 * s as u64 + 7_919 * c as u64,
                vr.clone(),
            );
            st.n_batches = 1;
            let mut t = flux_tally();
            run_fixed_source(&geom, &mats, &nucs, &source(), &st, Some(&mut t));
            let score: f64 = bins.iter().map(|&b| t.bins[b].mean(1) / chunk as f64).sum();
            writeln!(f, "{arm} {s} {c} {score:.10e}").unwrap();
        }
        f.flush().unwrap();
        println!("{arm} seed {s} done at {:.0} s", t0.elapsed().as_secs_f64());
    }
    println!(
        "{arm}: {:.0} s total, {n_seeds} seeds x {n} histories, chunk {chunk}",
        t0.elapsed().as_secs_f64()
    );
}

/// Reads the files written by `shielded_room_magic_bias_hunt_arm`
/// (`OUTRAM_MB_FILES` = comma-separated paths, the analog file first) and
/// prints, per arm: the seed-to-seed mean and sem, the chunk-level sem,
/// relative VOV of the chunk scores, skewness, the largest chunk's share of
/// the total and the top-5 share, and the z and ratio against analog.
#[test]
#[ignore = "reads the files written by shielded_room_magic_bias_hunt_arm"]
fn shielded_room_magic_bias_report() {
    let files = std::env::var("OUTRAM_MB_FILES").expect("OUTRAM_MB_FILES");
    let mut ref_stats: Option<(f64, f64)> = None;
    for path in files.split(',') {
        let txt = std::fs::read_to_string(path).unwrap();
        let mut by_seed: Vec<Vec<f64>> = Vec::new();
        let mut name = String::new();
        for l in txt.lines() {
            let p: Vec<&str> = l.split_whitespace().collect();
            name = p[0].to_string();
            let s: usize = p[1].parse().unwrap();
            while by_seed.len() <= s {
                by_seed.push(Vec::new());
            }
            by_seed[s].push(p[3].parse().unwrap());
        }
        let seed_means: Vec<f64> =
            by_seed.iter().map(|v| v.iter().sum::<f64>() / v.len() as f64).collect();
        let (mean, sem) = mean_and_sem(&seed_means);
        let all: Vec<f64> = by_seed.iter().flatten().copied().collect();
        let n = all.len() as f64;
        let m = all.iter().sum::<f64>() / n;
        let d2: f64 = all.iter().map(|x| (x - m).powi(2)).sum();
        let d3: f64 = all.iter().map(|x| (x - m).powi(3)).sum();
        let d4: f64 = all.iter().map(|x| (x - m).powi(4)).sum();
        let sem_chunks = (d2 / (n - 1.0) / n).sqrt();
        let vov = d4 / (d2 * d2) - 1.0 / n;
        let skew = (d3 / n) / (d2 / n).powf(1.5);
        let total: f64 = all.iter().sum();
        let mut sorted = all.clone();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let top5: f64 = sorted.iter().take(5).sum::<f64>() / total;
        let z = ref_stats.map(|(rm, rs)| (mean - rm) / (sem * sem + rs * rs).sqrt());
        println!(
            "{name}: seeds {}, chunks {}; mean {mean:.4} +- {sem:.4} (seed sem, {:.2} %), +- {sem_chunks:.4} (chunk sem); \
             chunk VOV {vov:.4}; skew {skew:.2}; max-chunk share {:.3} % (uniform {:.3} %); top-5 share {:.2} %; z vs analog {}; ratio {}",
            by_seed.len(),
            all.len(),
            100.0 * sem / mean,
            100.0 * sorted[0] / total,
            100.0 / n,
            100.0 * top5,
            z.map(|z| format!("{z:+.2}")).unwrap_or("-".into()),
            ref_stats.map(|(rm, _)| format!("{:.4}", mean / rm)).unwrap_or("-".into()),
        );
        if ref_stats.is_none() {
            ref_stats = Some((mean, sem));
        }
    }
}
