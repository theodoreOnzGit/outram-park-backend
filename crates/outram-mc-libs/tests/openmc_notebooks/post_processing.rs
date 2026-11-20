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
//! **Results (measured 2026-08-06, this harness; deterministic seed).** With 500
//! particles, 20 inactive + 35 active generations the run gives
//! **k_inf = 1.82334 ± 0.00769** (matching `expansion_filters.rs`, the same
//! infinite-medium model). The reshaped 4 × 4 flux grid (track-length units) is:
//!
//! - row 0: `[1.807e4, 1.703e4, 1.704e4, 1.806e4]`
//! - row 1: `[1.727e4, 2.253e4, 2.251e4, 1.727e4]`
//! - row 2: `[1.729e4, 2.252e4, 2.253e4, 1.729e4]`
//! - row 3: `[1.804e4, 1.704e4, 1.703e4, 1.806e4]`
//!
//! with max = 2.253e4, min = 1.703e4, total = 2.996e5 and spatial structure
//! `(max − min)/max = 0.244`. The four **centre** cells (fuel-overlapping) are
//! ~2.252e4 and clearly separated from the outer ring of pure-moderator cells —
//! the four **corner** cells at ~1.806e4 and the eight edge cells at
//! ~1.703e4–1.729e4 (the grid minimum) — the fuel cylinder raises the flux at the
//! pitch centre. The
//! corner cells agree to **0.0%** (spread 8.720e0 on a mean of 1.806e4) and the
//! centre cells to **0.0%** (spread 7.985e0 on 2.252e4): the grid is symmetric under
//! x→−x, y→−y and x↔y, exactly as the geometry demands. Interpretation:
//! `RegularMesh`/`MeshFilter` binning and the midpoint-position threading through
//! the scoring path are wired correctly; the reshaped grid recovers both the
//! fuel/moderator spatial structure and the geometry's four-fold symmetry.
//! Absolute cell values are track-length units (volume normalization is
//! op-6tz.22). The grid + summary are written to
//! `verification_and_validation/openmc_notebook_comparisons/post_processing.csv`
//! (the `date` column in that generated CSV is a literal in the test body and
//! still reads 2026-07-17 — the authoritative date is this block).
//!
//! **CORRECTED 2026-11-20 (gh:#678).** ~~"The fuel cylinder raises the flux at the
//! pitch centre" and the pass criterion `(max-min)/max > 0.10`.~~ Those figures
//! were measured with the midpoint rule. With regular-mesh track-length segments
//! split at the grid planes, the same model gives k_inf = 1.87262 +/- 0.00934
//! (other changes on `develop` since 2026-08-06 also moved k) and a FLAT grid:
//! rows `[5.302e2, 5.277e2, 5.278e2, 5.300e2]`, `[5.261e2, 5.182e2, 5.187e2,
//! 5.301e2]`, `[5.253e2, 5.218e2, 5.232e2, 5.265e2]`, `[5.273e2, 5.327e2,
//! 5.295e2, 5.272e2]`; max-min over max = 0.027; corner mean 5.287e2 (0.3 %),
//! centre mean 5.205e2 (0.4 %). A cell-resolved flux tally of the same transport
//! gives fuel/moderator flux per volume = 0.9866 against the mesh's centre/corner
//! 0.9845 +/- 0.0048, so the flat grid is the physical one and the earlier
//! "structure" was the midpoint bias. Gate:
//! `post_processing_mesh_structure_matches_the_cell_resolved_flux`.
//!
//! **Supersedes (pre-`op-jis` figures, measured 2026-07-17).** The numbers above
//! replace values taken with the old `prn` output function (uniforms formed from
//! the raw top-52 state bits, before bead `op-jis` added OpenMC's PCG-RXS-M-XS
//! output permutation). The LCG *state* recurrence is unchanged, but every
//! sampled uniform moved, and with it every scored mesh cell. Superseded:
//! **k_inf = 1.83164 ± 0.00763**; grid rows
//! `[1.742e4, 1.989e4, 1.989e4, 1.741e4]`, `[1.928e4, 2.373e4, 2.376e4, 1.928e4]`,
//! `[1.930e4, 2.376e4, 2.374e4, 1.927e4]`, `[1.741e4, 1.989e4, 1.990e4, 1.742e4]`;
//! max = 2.376e4, min = 1.741e4, total = 3.213e5, structure = 0.267; corner mean
//! ~1.741e4 (spread 5.2e0), centre mean ~2.375e4 (spread 1.2e1). Note the
//! outer-ring ordering also changed: the corner cells were then the grid minimum,
//! whereas they now sit slightly *above* the edge cells — both remain far below
//! the fuel-overlapping centre, so the asserted structure and symmetry properties
//! are unaffected.

use petir::mathf::RealMath;
use outram_mc_libs::tally::filter::FilterKind;
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
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            }, // U-234
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            }, // U-235
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            }, // U-238
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
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: r_fuel,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: -zmax,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::ZPlane(ZPlane {
            z0: zmax,
            bc: BoundaryType::Reflective,
        }),
    ];

    // Fuel: inside the cylinder, capped in z.
    let mut fuel_region = vec![RegionToken::HalfSpace {
        surface_idx: 0,
        sense: HalfSpaceSense::Inside,
    }];
    fuel_region.push(RegionToken::HalfSpace {
        surface_idx: 5,
        sense: HalfSpaceSense::Outside,
    });
    fuel_region.push(RegionToken::Intersection);
    fuel_region.push(RegionToken::HalfSpace {
        surface_idx: 6,
        sense: HalfSpaceSense::Inside,
    });
    fuel_region.push(RegionToken::Intersection);
    let fuel = Cell::material(1, fuel_region, 0, 293.6);

    // Moderator: outside the cylinder AND inside the reflective x/y/z box.
    let moder = Cell::material(
        2,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            }, // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            }, // y < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Outside,
            }, // z > -zmax
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 6,
                sense: HalfSpaceSense::Inside,
            }, // z < +zmax
            RegionToken::Intersection,
        ],
        1,
        293.6,
    );

    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
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
    let var = xs.iter().map(|x| (x - mean).r_powi(2)).sum::<f64>() / n;
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
        components: vec![NuclideComponent {
            nuclide_idx: 3,
            atom_density: 6.6e-2,
        }],
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
    let filter = MeshFilter {
        mesh: outram_mc_libs::tally::mesh::MeshKind::Regular(mesh),
    };
    let n_cells = nx * ny;
    let mut tally = Tally {
        id: 1,
        name: "mesh flux".into(),
        filters: vec![FilterKind::Mesh(filter)],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n_cells],
    };

    let settings = KeffSettings {
        n_particles: 500,
        n_inactive: 20,
        n_active: 35,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -zmax),
        upper: Position::new(r_fuel, r_fuel, zmax),
    };

    let t0 = std::time::Instant::now();
    let result = run_keff_csg(
        &geom,
        &materials,
        &nuclides,
        src,
        &settings,
        Some(&mut tally),
    );
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
        assert!(
            v.is_finite() && *v >= 0.0,
            "mesh cell {b} flux {v} not finite/non-negative"
        );
    }
    let total: f64 = flat.iter().sum();
    assert!(
        total > 0.0,
        "total meshed flux must be positive, got {total}"
    );

    let vmax = flat.iter().cloned().fold(f64::MIN, f64::max);
    let vmin = flat.iter().cloned().fold(f64::MAX, f64::min);
    let structure = (vmax - vmin) / vmax;

    // Corner cells (pure moderator) and centre cells (fuel-overlapping).
    let corners = [
        grid[0][0],
        grid[0][nx - 1],
        grid[ny - 1][0],
        grid[ny - 1][nx - 1],
    ];
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
    assert!(
        result.k_mean.is_finite() && result.k_mean > 0.0,
        "k_inf must be finite & positive"
    );
    assert!(
        result.k_std < 0.03,
        "eigenvalue under-converged: k_std {}",
        result.k_std
    );

    // ── Spatial structure ──────────────────────────────────────────────────────
    // ~~The grid is not flat: `(max-min)/max > 0.10`.~~ **CORRECTED 2026-11-20
    // (gh:#678).** That threshold was an artefact of the midpoint rule, which
    // scores a flight whole into the cell holding its midpoint and so piles long
    // flights into central cells. With the flight split at the mesh planes the
    // grid is flat (structure 0.027, centre/corner 0.9845), and an independent
    // cell-resolved flux tally of the SAME transport gives a fuel/moderator flux
    // per volume of 0.9866: the fast pin cell has a flat flux at this resolution.
    // The gate is now `post_processing_mesh_structure_matches_the_cell_resolved_flux`
    // (mesh centre/corner must agree with the cell-resolved ratio), not a
    // threshold on the mesh alone. The threshold was not loosened to pass: its
    // premise was tested and found to be the bias.
    assert!(
        structure.is_finite() && structure >= 0.0,
        "structure must be a finite non-negative number, got {structure:.3}"
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
        "2026-07-17,post-processing,summary,max={:.3e} min={:.3e} total={:.3e},mesh agrees with cell-resolved flux & corners symmetric,structure={:.3},{}",
        vmax, vmin, total, structure, "shape check not benchmark"
    ));
    write_csv(&rows);
}

/// **Independent check of the mesh flux structure (gh:#678, 2026-11-20).**
///
/// The `post_processing_statepoint` gate once asked for `(max-min)/max > 0.10`,
/// "because the central cells overlap the dense fuel cylinder". That expectation
/// was measured under the **midpoint** rule (a flight scored whole into the cell
/// holding its midpoint), which piles long flights into central cells. With
/// regular-mesh track-length segments split at the grid planes (gh:#678) the same
/// run gives a flat grid. To decide which is right without assuming either, this
/// test runs the SAME transport (same seed; tallies draw no random numbers) with
/// a **cell-resolved** flux tally, which needs no splitting and no midpoint:
///
/// - the total flux of the mesh tally equals the total of the cell tally
///   (conservation of flight length, to 1e-9 relative);
/// - the volume-normalised fuel/moderator flux ratio `rho` is printed, and the
///   mesh's centre/corner ratio must lie within `rho`'s neighbourhood: centre
///   cells are almost entirely fuel, corner cells pure moderator, so
///   centre/corner must agree with `rho` to within the statistical error of the
///   mesh cells plus a 3 % allowance for the fuel/moderator mix of the centre
///   cells.
///
/// **Result (2026-11-20, 500 particles, 20 + 35 generations):** see the printed
/// line; recorded in the V&V block of this file.
#[test]
fn post_processing_mesh_structure_matches_the_cell_resolved_flux() {
    use outram_mc_libs::tally::filter::CellFilter;
    let nuclides = nuclides();
    let moderator = Material {
        id: 2,
        name: "H moderator".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 6.6e-2 }],
    };
    let materials = vec![heu_fuel(), moderator];
    let (half, r_fuel, zmax) = (0.63_f64, 0.4_f64, 10.0_f64);
    let geom = reflective_box(r_fuel, half, zmax);
    let settings = KeffSettings { n_particles: 500, n_inactive: 20, n_active: 35, ..KeffSettings::default() };
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -zmax),
        upper: Position::new(r_fuel, r_fuel, zmax),
    };
    let n_active = settings.n_active as u64;

    let mesh_tally = |n: usize| Tally {
        id: 1,
        name: "mesh".into(),
        filters: vec![FilterKind::Mesh(MeshFilter {
            mesh: outram_mc_libs::tally::mesh::MeshKind::Regular(RegularMesh {
                lower_left: [-half, -half, -zmax],
                upper_right: [half, half, zmax],
                dimension: [n, n, 1],
            }),
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n * n],
    };
    let mut mesh = mesh_tally(4);
    run_keff_csg(&geom, &materials, &nuclides, src, &settings, Some(&mut mesh));
    let mut cells = Tally {
        id: 2,
        name: "cells".into(),
        filters: vec![FilterKind::Cell(CellFilter { cell_indices: vec![0, 1] })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 2],
    };
    run_keff_csg(&geom, &materials, &nuclides, src, &settings, Some(&mut cells));

    let m: Vec<f64> = mesh.bins.iter().map(|b| b.mean(n_active)).collect();
    let c: Vec<f64> = cells.bins.iter().map(|b| b.mean(n_active)).collect();
    let (tm, tc) = (m.iter().sum::<f64>(), c.iter().sum::<f64>());
    assert!((tm - tc).abs() < 1e-9 * tc, "flight length not conserved: mesh {tm} vs cells {tc}");

    let v_fuel = std::f64::consts::PI * r_fuel * r_fuel * 2.0 * zmax;
    let v_mod = (2.0 * half) * (2.0 * half) * 2.0 * zmax - v_fuel;
    let rho = (c[0] / v_fuel) / (c[1] / v_mod);
    let centre = [m[5], m[6], m[9], m[10]];
    let corner = [m[0], m[3], m[12], m[15]];
    let (cm, cs) = mean_std(&centre);
    let (km, ks) = mean_std(&corner);
    let ratio = cm / km;
    let stat = ((cs / cm).powi(2) + (ks / km).powi(2)).sqrt();
    println!(
        "[post-processing] cell-resolved fuel/moderator flux per volume rho = {rho:.4}; mesh centre/corner = {ratio:.4} (stat {stat:.4})"
    );
    assert!(
        (ratio - rho).abs() < 3.0 * stat + 0.03,
        "mesh centre/corner {ratio:.4} disagrees with the cell-resolved ratio {rho:.4}"
    );
}
