//! `post-processing` notebook -> outram-mc verification (LIVE, op-6tz.13).
//!
//! Notebook: `post-processing.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Reads a `StatePoint`, extracts a `RegularMesh`
//! flux tally, reshapes the flat bins into the spatial grid, and plots the flux
//! distribution. OpenMC API exercised: `StatePoint`, `RegularMesh`, `MeshFilter`,
//! `Tally`, mesh reshaping.
//!
//! **What outram-mc does here (this run wires it, op-6tz.13).** The CSG
//! k-eigenvalue loop ([`outram_mc_libs::physics::transport_csg::run_keff_csg`])
//! drives the **track-length flux estimator**
//! ([`outram_mc_libs::tally::scoring::score_track_length`]); a
//! [`outram_mc_libs::tally::filter::MeshFilter`] over a
//! [`outram_mc_libs::tally::mesh::RegularMesh`] bins each streamed segment into
//! the mesh cell containing its midpoint (`RegularMesh::get_bin`, mirroring
//! `src/mesh.cpp:1473`/`:1039`). The flat bins are then **reshaped in-memory**
//! into the 2-D spatial grid and the spatial flux structure is asserted — the
//! same post-processing the notebook does after reading a StatePoint.
//!
//! **Scope / gap.** The on-disk `StatePoint` round-trip (write tally to HDF5,
//! reload, post-process) is **not** ported (bead op-6tz.22): this test
//! post-processes the *in-memory* tally directly. The notebook's *exact* per-cell
//! flux numbers also need full continuous-energy data, so (as in
//! `flux_spectrum.rs`) this is a **shape** check — spatial structure + symmetry —
//! not a benchmark of the cell values.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** A heterogeneous reflective pin cell (Godiva HEU fuel cylinder,
//! r = 0.4 cm, in an H-1 moderator filling a 1.26 cm square pitch), fully
//! reflective on x/y/z (an infinite-medium k_inf model with a finite z span
//! z ∈ [−10, 10] cm). A single flux [`Tally`] carries one [`MeshFilter`] over a
//! **4 × 4 × 1** `RegularMesh` spanning the pitch square, so the 16 bins resolve
//! the flux across the fuel/moderator sub-regions. Analog transport, LOW-tier
//! embedded data (`Nuclide::from_core`), free-gas hydrogen. The flat bins are
//! reshaped `bin = iy·nx + ix` into a 4 × 4 grid.
//!
//! **Pass criterion (spatial structure + symmetry, shape not benchmark).**
//! - Every mesh cell finite and non-negative (a valid flux estimate).
//! - Total meshed flux strictly positive.
//! - **Spatial structure**: the grid is not flat — `(max − min)/max` is
//!   appreciable — because the central cells overlap the dense fuel cylinder while
//!   the corner cells are pure moderator.
//! - **Four-fold symmetry**: the geometry is symmetric under x→−x, y→−y and
//!   x↔y, so the four corner cells (and, separately, the four central cells) must
//!   agree to within Monte-Carlo statistics.
//!
//! **Results (measured 2026-07-17, this harness; deterministic seed).** With 500
//! particles, 20 inactive + 35 active generations the run gives
//! **k_inf = 1.83164 ± 0.00763** (matching `expansion_filters.rs`, the same
//! infinite-medium model). The reshaped 4 × 4 flux grid (track-length units) is:
//!
//! - row 0: `[1.742e4, 1.989e4, 1.989e4, 1.741e4]`
//! - row 1: `[1.928e4, 2.373e4, 2.376e4, 1.928e4]`
//! - row 2: `[1.930e4, 2.376e4, 2.374e4, 1.927e4]`
//! - row 3: `[1.741e4, 1.989e4, 1.990e4, 1.742e4]`
//!
//! with max = 2.376e4, min = 1.741e4, total = 3.213e5 and spatial structure
//! `(max − min)/max = 0.267`. The four **centre** cells (fuel-overlapping) are
//! ~2.375e4 and clearly separated from the four **corner** cells (pure moderator)
//! at ~1.741e4 — the fuel cylinder raises the flux at the pitch centre. The
//! corner cells agree to **0.0%** (spread 5.2e0 on a mean of 1.741e4) and the
//! centre cells to **0.0%** (spread 1.2e1 on 2.375e4): the grid is symmetric under
//! x→−x, y→−y and x↔y, exactly as the geometry demands. Interpretation:
//! `RegularMesh`/`MeshFilter` binning and the midpoint-position threading through
//! the scoring path are wired correctly; the reshaped grid recovers both the
//! fuel/moderator spatial structure and the geometry's four-fold symmetry.
//! Absolute cell values are track-length units (volume normalization is
//! op-6tz.22). The grid + summary are written to
//! `verification_and_validation/openmc_notebook_comparisons/post_processing.csv`.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::MeshFilter;
use outram_mc_libs::tally::mesh::RegularMesh;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
use std::io::Write;

/// Godiva HEU atom densities (HEU-MET-FAST-001), atoms/barn-cm.
fn heu_fuel() -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    }
}

/// Nuclide array: `0 U234, 1 U235, 2 U238, 3 H1` (free-gas H, no S(α,β)).
fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
        Nuclide::from_core("H1").expect("H1 in CORE WMP library"),
    ]
}

/// Fully-reflective HEU pin cell: fuel cylinder (radius `r_fuel`) in an H-1
/// moderator, `2·half` wide in x/y and z ∈ [−zmax, zmax], all six faces
/// reflective. Both cells carry the z-plane half-spaces so the particle reflects
/// off them. Cell 0 = fuel (material 0), cell 1 = moderator (material 1).
fn reflective_box(r_fuel: f64, half: f64, zmax: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: r_fuel, bc: BoundaryType::Transmissive }),
        SurfaceKind::XPlane(XPlane { x0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -zmax, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: zmax, bc: BoundaryType::Reflective }),
    ];

    // Fuel: inside the cylinder, capped in z.
    let mut fuel_region = vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }];
    fuel_region.push(RegionToken::HalfSpace { surface_idx: 5, sense: HalfSpaceSense::Outside });
    fuel_region.push(RegionToken::Intersection);
    fuel_region.push(RegionToken::HalfSpace { surface_idx: 6, sense: HalfSpaceSense::Inside });
    fuel_region.push(RegionToken::Intersection);
    let fuel = Cell::material(1, fuel_region, 0, 293.6);

    // Moderator: outside the cylinder AND inside the reflective x/y/z box.
    let moder = Cell::material(
        2,
        vec![
            RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
            RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside }, // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Outside }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Inside }, // y < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 5, sense: HalfSpaceSense::Outside }, // z > -zmax
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 6, sense: HalfSpaceSense::Inside }, // z < +zmax
            RegionToken::Intersection,
        ],
        1,
        293.6,
    );

    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Write the notebook-comparison CSV (gitignored dir).
fn write_csv(rows: &[String]) {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/verification_and_validation/openmc_notebook_comparisons"
    );
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{dir}/post_processing.csv");
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = writeln!(f, "date,notebook,case,our_value±σ,reference,delta,note");
        for r in rows {
            let _ = writeln!(f, "{r}");
        }
    }
}

/// Mean and sample standard deviation of a slice (for the symmetry check).
fn mean_std(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    (mean, var.sqrt())
}

/// LIVE (op-6tz.13): **RegularMesh flux tally** over a heterogeneous reflective
/// pin cell, reshaped to a 4×4 spatial grid. Asserts finite/non-negative cells,
/// positive total, real spatial structure, and four-fold geometric symmetry. See
/// the module V&V note for methodology and measured results.
#[test]
fn post_processing_statepoint() {
    let nuclides = nuclides();
    let fuel = heu_fuel();
    let moderator = Material {
        id: 2,
        name: "H moderator".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 6.6e-2 }],
    };
    let materials = vec![fuel, moderator];

    let half = 0.63;
    let r_fuel = 0.4;
    let zmax = 10.0;
    let geom = reflective_box(r_fuel, half, zmax);

    // 4×4×1 RegularMesh over the pitch square, full z span.
    let nx = 4usize;
    let ny = 4usize;
    let mesh = RegularMesh {
        lower_left: [-half, -half, -zmax],
        upper_right: [half, half, zmax],
        dimension: [nx, ny, 1],
    };
    let filter = MeshFilter { mesh };
    let n_cells = nx * ny;
    let mut tally = Tally {
        id: 1,
        name: "mesh flux".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n_cells],
    };

    let settings = KeffSettings { n_particles: 500, n_inactive: 20, n_active: 35, ..KeffSettings::default() };
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -zmax),
        upper: Position::new(r_fuel, r_fuel, zmax),
    };

    let t0 = std::time::Instant::now();
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, Some(&mut tally));
    let dt = t0.elapsed();

    let n_active = settings.n_active as u64;
    // Reshape flat bins → grid[iy][ix] with bin = iy·nx + ix.
    let flat: Vec<f64> = tally.bins.iter().map(|b| b.mean(n_active)).collect();
    let mut grid = vec![vec![0.0f64; nx]; ny];
    for iy in 0..ny {
        for ix in 0..nx {
            grid[iy][ix] = flat[iy * nx + ix];
        }
    }

    // ── Validity: every cell finite and non-negative ───────────────────────────
    for (b, v) in flat.iter().enumerate() {
        assert!(v.is_finite() && *v >= 0.0, "mesh cell {b} flux {v} not finite/non-negative");
    }
    let total: f64 = flat.iter().sum();
    assert!(total > 0.0, "total meshed flux must be positive, got {total}");

    let vmax = flat.iter().cloned().fold(f64::MIN, f64::max);
    let vmin = flat.iter().cloned().fold(f64::MAX, f64::min);
    let structure = (vmax - vmin) / vmax;

    // Corner cells (pure moderator) and centre cells (fuel-overlapping).
    let corners = [grid[0][0], grid[0][nx - 1], grid[ny - 1][0], grid[ny - 1][nx - 1]];
    let centre = [grid[1][1], grid[1][2], grid[2][1], grid[2][2]];
    let (corner_mean, corner_std) = mean_std(&corners);
    let (centre_mean, centre_std) = mean_std(&centre);

    eprintln!(
        "[post-processing] k_inf = {:.5} ± {:.5}  |  4×4 mesh flux grid (track-length units):",
        result.k_mean, result.k_std
    );
    for iy in 0..ny {
        let row: Vec<String> = (0..nx).map(|ix| format!("{:.3e}", grid[iy][ix])).collect();
        eprintln!("    [{}]", row.join(", "));
    }
    eprintln!(
        "    max = {:.3e}, min = {:.3e}, total = {:.3e}, structure (max-min)/max = {:.3}",
        vmax, vmin, total, structure
    );
    eprintln!(
        "    corner cells: mean {:.3e} ± {:.3e} ({:.1}%);  centre cells: mean {:.3e} ± {:.3e} ({:.1}%)",
        corner_mean, corner_std, 100.0 * corner_std / corner_mean,
        centre_mean, centre_std, 100.0 * centre_std / centre_mean
    );
    eprintln!(
        "    (npart={}, {}+{} gen, {:.1?})",
        settings.n_particles, settings.n_inactive, settings.n_active, dt
    );

    // ── k sanity ───────────────────────────────────────────────────────────────
    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k_inf must be finite & positive");
    assert!(result.k_std < 0.03, "eigenvalue under-converged: k_std {}", result.k_std);

    // ── Spatial structure: the grid is not flat ────────────────────────────────
    assert!(
        structure > 0.10,
        "expected appreciable spatial structure (fuel vs moderator cells), got (max-min)/max = {structure:.3}"
    );

    // ── Four-fold symmetry: corner cells agree within statistics ───────────────
    // The geometry is symmetric under x→−x, y→−y, x↔y; the four corner cells are
    // physically identical, so their spread must be a small fraction of their mean.
    // 40% tolerates the modest particle count while still catching a broken
    // position→bin mapping (which would scatter the corners arbitrarily).
    assert!(
        corner_std / corner_mean < 0.40,
        "corner cells should be symmetric (equal within stats), got spread {:.1}% of mean",
        100.0 * corner_std / corner_mean
    );
    assert!(
        centre_std / centre_mean < 0.40,
        "centre cells should be symmetric (equal within stats), got spread {:.1}% of mean",
        100.0 * centre_std / centre_mean
    );

    // CSV: the grid + summary stats.
    let mut rows = Vec::new();
    for iy in 0..ny {
        for ix in 0..nx {
            rows.push(format!(
                "2026-07-17,post-processing,cell(ix={ix} iy={iy}),{:.4e},structured+symmetric,n/a,{}",
                grid[iy][ix], "track-length units; StatePoint round-trip op-6tz.22"
            ));
        }
    }
    rows.push(format!(
        "2026-07-17,post-processing,summary,max={:.3e} min={:.3e} total={:.3e},structure>0.10 & corners symmetric,structure={:.3},{}",
        vmax, vmin, total, structure, "shape check not benchmark"
    ));
    write_csv(&rows);
}
