// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — the emitted matplotlib geometry-plot script. GitHub #268.
//!
//! # Methodology
//!
//! Three things the issue asks for, checked separately:
//!
//! 1. **The index array matches `locate()`** for sampled points. The script's
//!    whole value is that it shows the model the code actually has, so the
//!    array it embeds must be what `locate` returns and not a re-derivation.
//!    Checked at every pixel, not a handful — the grid here is small enough
//!    that "a handful" would be a weaker test for no saving.
//! 2. **The emitted script is valid Python** and **runs on a bare interpreter
//!    with only matplotlib and numpy**, producing the PNG.
//! 3. **Regions are actually resolved**: a slice through concentric spheres
//!    must show the expected number of distinct indices in the expected radial
//!    order. A script that emitted a uniform array would pass 1 and 2 and be
//!    useless.
//!
//! # Results (2026-09-22)
//!
//! - **1681 pixels** (a 41x41 slice) checked against `locate()`, all matching.
//! - Radial sequence out from the centre: `[0, 1, 2, -1]` — inner cell, two
//!   shells, then outside the geometry — and all four indices present in the
//!   image.
//! - `/opt/ompy/bin/python` ran the emitted script and wrote a **26 917 byte**
//!   PNG.
//!
//! The interpreter check is not decoration. `python3` on this machine does
//! **not** have matplotlib; `/opt/ompy/bin/python` does. The test searches for
//! one that does and skips with a printed note otherwise, so it cannot pass
//! silently by never having run anything.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{emit_python, sample_slice, ColourBy, Slice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;

/// Three concentric shells: r<1, 1<r<2, 2<r<3.
fn shells() -> Geometry {
    let radii = [1.0, 2.0, 3.0];
    let surfaces: Vec<SurfaceKind> = radii
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r,
                bc: if i + 1 == radii.len() {
                    BoundaryType::Vacuum
                } else {
                    BoundaryType::Transmissive
                },
            })
        })
        .collect();
    let mut cells = vec![Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    )];
    for i in 1..radii.len() {
        cells.push(Cell::material(
            (i + 1) as i32,
            vec![
                RegionToken::HalfSpace {
                    surface_idx: i - 1,
                    sense: HalfSpaceSense::Outside,
                },
                RegionToken::HalfSpace {
                    surface_idx: i,
                    sense: HalfSpaceSense::Inside,
                },
                RegionToken::Intersection,
            ],
            0,
            293.6,
        ));
    }
    let cell_indices = (0..cells.len()).collect();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn xy_slice(n: usize) -> Slice {
    Slice {
        origin: Position::new(0.0, 0.0, 0.0),
        basis_u: Direction::new(1.0, 0.0, 0.0),
        basis_v: Direction::new(0.0, 1.0, 0.0),
        width_u: 8.0,
        width_v: 8.0,
        pixels_u: n,
        pixels_v: n,
    }
}

/// Every pixel of the embedded array must be what `locate()` says.
#[test]
fn the_index_array_is_what_locate_returns() {
    let geom = shells();
    let slice = xy_slice(41);
    let data = sample_slice(&geom, &slice, ColourBy::Cell);
    assert_eq!(data.len(), slice.pixels_v);

    let probe = {
        let n = 3.0_f64.sqrt();
        Direction::new(1.0 / n, 1.0 / n, 1.0 / n)
    };
    let mut checked = 0usize;
    for (j, row) in data.iter().enumerate() {
        assert_eq!(row.len(), slice.pixels_u);
        for (i, &got) in row.iter().enumerate() {
            let fu = (i as f64 + 0.5) / slice.pixels_u as f64 - 0.5;
            let fv = (j as f64 + 0.5) / slice.pixels_v as f64 - 0.5;
            let p = Position::new(fu * slice.width_u, -fv * slice.width_v, 0.0);
            let want = match geom.locate(p, probe, SurfaceToken::NONE) {
                None => -1,
                Some(path) => path.levels.last().map(|c| c.cell as i64).unwrap_or(-1),
            };
            assert_eq!(got, want, "pixel ({i},{j}) at {p:?}");
            checked += 1;
        }
    }
    println!("checked {checked} pixels against locate()");
}

/// The slice must actually resolve the three shells, in the right radial order.
///
/// Without this, an emitter that produced a uniform array would pass every
/// other test in this file.
#[test]
fn the_slice_resolves_the_shells_in_radial_order() {
    let geom = shells();
    let data = sample_slice(&geom, &xy_slice(101), ColourBy::Cell);
    let mid = data.len() / 2;
    let row = &data[mid];

    // Walk out from the centre: cell 0, then 1, then 2, then outside (-1).
    let centre = row[row.len() / 2];
    assert_eq!(centre, 0, "the centre pixel must be the innermost cell");
    let mut seen = vec![centre];
    for &v in row.iter().skip(row.len() / 2) {
        if *seen.last().unwrap() != v {
            seen.push(v);
        }
    }
    println!("radial sequence from centre outwards: {seen:?}");
    assert_eq!(
        seen,
        vec![0, 1, 2, -1],
        "expected inner cell, two shells, then outside the geometry"
    );

    // And all four indices must be present somewhere in the image.
    let mut all: Vec<i64> = data.iter().flatten().copied().collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all, vec![-1, 0, 1, 2], "distinct indices in the slice");
}

/// The emitted script must be valid Python and must run on a bare interpreter
/// with only matplotlib and numpy, producing the PNG.
///
/// Skips with a printed note when no interpreter with matplotlib is available,
/// per this crate's data-gated convention — a developer without it must not see
/// a red test, but the skip must be visible.
#[test]
fn the_emitted_script_runs_on_a_bare_interpreter() {
    let geom = shells();
    let script = emit_python(
        &geom,
        &xy_slice(31),
        ColourBy::Material,
        "three concentric shells",
        "outram-mc-libs test, GitHub #268",
    );

    // Header must carry the provenance the issue asks for.
    assert!(script.contains("# model      : three concentric shells"));
    assert!(script.contains("# coloured by: material"));
    assert!(script.contains("# provenance : outram-mc-libs test, GitHub #268"));
    assert!(script.contains("import matplotlib.pyplot as plt"));

    let Some(python) = ["/opt/ompy/bin/python", "python3"].into_iter().find(|p| {
        std::process::Command::new(p)
            .args(["-c", "import matplotlib, numpy"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }) else {
        println!(
            "[skip] no python3 with matplotlib+numpy on this machine; the emitted \
             script was still checked for its header and structure above"
        );
        return;
    };

    let dir = std::env::temp_dir().join("outram_mc_plot_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slice.py");
    std::fs::write(&path, &script).unwrap();

    // Non-interactive backend: the test machine has no display.
    let out = std::process::Command::new(python)
        .arg(path.file_name().unwrap())
        .current_dir(&dir)
        .env("MPLBACKEND", "Agg")
        .output()
        .expect("run the emitted script");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the emitted script failed under {python}:\n{stderr}"
    );
    let png = dir.join("geometry_slice.png");
    assert!(png.exists(), "the script did not write geometry_slice.png");
    let bytes = std::fs::metadata(&png).unwrap().len();
    println!("{python} ran the emitted script; wrote {bytes} bytes of PNG");
    assert!(bytes > 1000, "PNG is suspiciously small ({bytes} bytes)");
}

/// **Cross-code: our slice against OpenMC's own rasteriser, on Godiva** —
/// gh:#268 acceptance item 1, *"A slice of one of the repo's existing core
/// models (HTR-10, an FHR pebble model, or the Godiva sphere) plotted this way
/// and visually compared against the `openmc.Plot` output of the same
/// geometry."*
///
/// # Why a mask and not the colours
///
/// OpenMC assigns cell colours arbitrarily, so comparing RGB would compare a
/// palette. What both codes must agree on is **which points are in the model**,
/// which is the geometry question the plot exists to answer. The reference is
/// therefore a 0/1 cell mask read off OpenMC's PNG (white = no cell).
///
/// # Why this is runnable at all
///
/// `openmc --plot` needs **no cross-section library** — it only locates cells.
/// None is installed here, so every other cross-code comparison in this crate
/// had to go through a data pipeline first; this one did not.
///
/// # A disagreement is only forgiven at the boundary
///
/// A pixel where the two codes differ is acceptable **only** if its centre lies
/// within half a pixel diagonal of the sphere surface, where which side a
/// sample falls on is a rounding question rather than a geometry one. A
/// disagreement anywhere else is a real difference, and the test says which
/// pixel and how far from the surface it sat — because "a few pixels differ" is
/// exactly the report that hides a systematic offset.
///
/// # Results (2026-09-24, OpenMC `0.1.dev1+gafa7a14ac`)
///
/// Printed by the test. 81 x 81 = 6561 pixels, Godiva at r = 8.7407 cm in a
/// 24 x 24 cm window. OpenMC finds 2733 pixels inside (41.66 %), against the
/// analytic circle fraction `pi r^2 / 24^2` = **41.68 %**.
#[test]
fn our_godiva_slice_matches_openmcs_own_rasteriser() {
    use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
    use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use outram_mc_libs::geometry::universe::Universe;

    // Must match the reference script's constants exactly.
    const R_CM: f64 = 8.7407;
    const HALF_WIDTH_CM: f64 = 12.0;
    const PIXELS: usize = 81;

    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/geometry_plot/openmc_inputs/openmc_godiva_mask.txt");
    let Ok(text) = std::fs::read_to_string(&reference) else {
        println!("[skip] no OpenMC reference mask at {}", reference.display());
        return;
    };
    let theirs: Vec<Vec<u8>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.bytes().map(|b| b - b'0').collect())
        .collect();
    assert_eq!(theirs.len(), PIXELS, "reference row count");
    assert!(theirs.iter().all(|r| r.len() == PIXELS), "reference row widths");

    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R_CM,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            CellFill::Void,
            Position::ZERO,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let slice = Slice {
        origin: Position::ZERO,
        basis_u: Direction::new(1.0, 0.0, 0.0),
        basis_v: Direction::new(0.0, 1.0, 0.0),
        width_u: 2.0 * HALF_WIDTH_CM,
        width_v: 2.0 * HALF_WIDTH_CM,
        pixels_u: PIXELS,
        pixels_v: PIXELS,
    };
    let ours = sample_slice(&geom, &slice, ColourBy::Cell);

    // Half a pixel diagonal: inside this band, which side a sample lands on is
    // rounding, not geometry.
    let pitch = 2.0 * HALF_WIDTH_CM / PIXELS as f64;
    let band = 0.5 * pitch * 2.0_f64.sqrt();

    let mut inside_ours = 0usize;
    let mut inside_theirs = 0usize;
    let mut boundary_diffs = 0usize;
    let mut interior_diffs: Vec<String> = Vec::new();
    for j in 0..PIXELS {
        for i in 0..PIXELS {
            let a = u8::from(ours[j][i] >= 0);
            let b = theirs[j][i];
            inside_ours += usize::from(a == 1);
            inside_theirs += usize::from(b == 1);
            if a == b {
                continue;
            }
            // Pixel centre in the slice plane, same convention as `sample_slice`.
            let fu = (i as f64 + 0.5) / PIXELS as f64 - 0.5;
            let fv = (j as f64 + 0.5) / PIXELS as f64 - 0.5;
            let x = fu * 2.0 * HALF_WIDTH_CM;
            let y = -fv * 2.0 * HALF_WIDTH_CM; // row 0 is the TOP
            let dist_from_surface = ((x * x + y * y).sqrt() - R_CM).abs();
            if dist_from_surface <= band {
                boundary_diffs += 1;
            } else {
                interior_diffs.push(format!(
                    "    ({i},{j}) at ({x:.3},{y:.3}) cm, {dist_from_surface:.3} cm from \
                     the surface: ours {a}, OpenMC {b}"
                ));
            }
        }
    }

    let analytic = std::f64::consts::PI * R_CM * R_CM / (2.0 * HALF_WIDTH_CM).powi(2);
    println!(
        "{PIXELS}x{PIXELS} = {} pixels; inside: ours {inside_ours} ({:.2} %), \
         OpenMC {inside_theirs} ({:.2} %), analytic {:.2} %",
        PIXELS * PIXELS,
        100.0 * inside_ours as f64 / (PIXELS * PIXELS) as f64,
        100.0 * inside_theirs as f64 / (PIXELS * PIXELS) as f64,
        100.0 * analytic
    );
    println!(
        "  disagreements: {boundary_diffs} within half a pixel diagonal ({band:.4} cm) \
         of the surface, {} elsewhere",
        interior_diffs.len()
    );
    assert!(
        interior_diffs.is_empty(),
        "our slice differs from OpenMC's rasteriser AWAY from the surface, which is a \
         real geometry difference rather than rounding:\n{}",
        interior_diffs.join("\n")
    );
    // Both must find the analytic fraction; a systematic offset would show here
    // even with zero interior disagreements.
    for (who, n) in [("ours", inside_ours), ("OpenMC", inside_theirs)] {
        let frac = n as f64 / (PIXELS * PIXELS) as f64;
        assert!(
            (frac - analytic).abs() < 0.005,
            "{who} finds {:.2} % of the window inside the sphere against the analytic \
             {:.2} %",
            100.0 * frac,
            100.0 * analytic
        );
    }
}
