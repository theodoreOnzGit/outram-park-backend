// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # Draw every `power_shape::PowerShape` kind (gh:#1184)
//!
//! Writes one R-Z picture per shape (and an x-y one for the
//! non-axisymmetric ones) over the HTR-10 full-core bed (`r < 90 cm`,
//! `|z| < 90 cm`, Monte Carlo frame), plus the shapes put on the `htr10_rz`
//! mesh's bed cells, into
//! `crates/nee_soon/verification_and_validation/power_shapes/images/`.
//!
//! ```bash
//! cargo run --release -p nee_soon --example power_shape_images
//! ```
//!
//! Each picture is drawn from the shape's own evaluation (the same function
//! the cell integrals integrate), so what you see is what a coupling puts
//! on the thermal-hydraulic cells.

use nee_soon::power_shape::{Frame, FrameMap, PowerCells, PowerShape, RadialProfile, ZoneLump};
use outram_foam_appbuilder_lib::htr10_rz::{build_mesh, geometry::idx, Htr10RzOptions};
use uom::si::f64::{Length, Power};
use uom::si::length::centimeter;
use uom::si::power::megawatt;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

fn main() -> Result<(), String> {
    let l = Length::new::<centimeter>;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/power_shapes/images");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let shapes: Vec<(&str, &str, PowerShape)> = vec![
        ("uniform", "UNIFORM", PowerShape::Uniform),
        (
            "zones",
            "ZONES: 70 PCT INNER LOWER, 30 PCT OUTER UPPER",
            PowerShape::Zones(vec![
                ZoneLump { r: [l(0.0), l(40.0)], z: [l(-90.0), l(10.0)], fraction: 0.7 },
                ZoneLump { r: [l(40.0), l(90.0)], z: [l(-30.0), l(90.0)], fraction: 0.3 },
            ]),
        ),
        ("chopped_cosine", "CHOPPED COSINE HE 230 CM", PowerShape::ChoppedCosine { centre: l(0.0), extrapolated_height: l(230.0) }),
        ("sine", "SINE OVER THE BED", PowerShape::Sine { bottom: l(-90.0), height: l(180.0) }),
        (
            "j0_cosine",
            "J0 X COS, RE 110 CM, HE 230 CM",
            PowerShape::J0Cosine { centre: l(0.0), extrapolated_radius: l(110.0), extrapolated_height: l(230.0) },
        ),
        (
            "legendre_z",
            "LEGENDRE-Z ORDER 3",
            PowerShape::LegendreZ { z_min: l(-90.0), z_max: l(90.0), coefficients: vec![1.0, 0.35, -0.4, -0.1] },
        ),
        (
            "zernike_legendre",
            "ZERNIKE-LEGENDRE, A TILTED BOWL",
            PowerShape::ZernikeLegendre {
                centre: [l(0.0), l(0.0)],
                radius: l(90.0),
                z_min: l(-90.0),
                z_max: l(90.0),
                coefficients: vec![vec![1.0, 0.0, 0.25, 0.0, -0.4, 0.0], vec![0.3, 0.0, 0.0, 0.0, 0.0, 0.0]],
            },
        ),
        (
            "spherical_harmonics",
            "SPATIAL SPHERICAL HARMONICS Y00 + Y11, BARE-SPHERE RADIAL",
            PowerShape::SphericalHarmonics {
                centre: [l(0.0), l(0.0), l(0.0)],
                radius: l(90.0),
                coefficients: vec![1.0, 0.0, 0.2, 0.6],
                radial: RadialProfile::BareSphere,
            },
        ),
        (
            "rz_table",
            "R-Z TABLE 3 X 3",
            PowerShape::RzTable {
                r_edges: vec![l(0.0), l(33.0), l(70.0), l(90.0)],
                z_edges: vec![l(-90.0), l(-17.0), l(50.0), l(90.0)],
                values: vec![1.0, 0.8, 0.5, 1.4, 1.1, 0.7, 0.9, 0.6, 0.4],
            },
        ),
    ];

    let mesh = build_mesh(&Htr10RzOptions::default()).map_err(|e| e.to_string())?;
    let cells = PowerCells::from_rz_mesh(&mesh, FrameMap::new(l(90.0)), |c| mesh.cell_region(c) == idx::BED);
    println!("htr10_rz bed cells: {}", cells.cells.len());
    for (file, title, s) in &shapes {
        s.write_rz_png(dir.join(format!("{file}_rz.png")), &format!("{title} (R-Z, MC FRAME)"), l(90.0), [l(-90.0), l(90.0)], 2.0)
            .map_err(|e| e.to_string())?;
        if !s.is_axisymmetric() {
            s.write_xy_png(dir.join(format!("{file}_xy.png")), &format!("{title} (X-Y AT Z = 0)"), l(0.0), l(90.0), 2.0)
                .map_err(|e| e.to_string())?;
        }
        // On the TH mesh: the per-cell densities, drawn as an R-Z table on
        // the mesh cells (TECDOC frame converted back to MC), conserved.
        let total = Power::new::<megawatt>(10.0);
        let q = s.cell_power_density(&cells, total).map_err(|e| e.to_string())?;
        let sum: f64 = q
            .iter()
            .zip(&cells.cells)
            .map(|(q, c)| q.get::<watt_per_cubic_meter>() * c.volume().get::<uom::si::volume::cubic_meter>())
            .sum();
        let peak = q.iter().map(|q| q.get::<watt_per_cubic_meter>()).fold(0.0, f64::max);
        println!("{file:>20}: on {} bed cells, sum {:.12e} W (target 1e7), peak {:.3e} W/m^3", cells.cells.len(), sum, peak);
        // The mesh's tensor grid as an R-Z table in the MC frame (z flips,
        // so the axial order reverses), heated cells carrying their density.
        let (re, ze) = (mesh.r_edges(), mesh.z_edges());
        let (nr, nz) = (re.n_cells(), ze.n_cells());
        let map = cells.map;
        let z_mc: Vec<Length> = (0..=nz).rev().map(|j| map.z_to_mc(Frame::Tecdoc, ze.edge(j))).collect();
        let mut values = vec![0.0; nr * nz];
        for (k, c) in cells.mesh_cell.iter().enumerate() {
            let (i, j) = (c % nr, c / nr);
            values[i + nr * (nz - 1 - j)] = q[k].get::<watt_per_cubic_meter>();
        }
        let on_mesh = PowerShape::RzTable { r_edges: (0..=nr).map(|i| re.edge(i)).collect(), z_edges: z_mc, values };
        on_mesh
            .write_rz_png(
                dir.join(format!("{file}_on_htr10_rz_mesh.png")),
                &format!("{title}: 10 MW ON THE HTR10_RZ BED CELLS"),
                l(90.0),
                [l(-130.0), l(90.0)],
                2.0,
            )
            .map_err(|e| e.to_string())?;
    }
    // Zernike -> Fourier-Bessel: an order-4 Zernike profile against its
    // Neumann (Dini) and Dirichlet (R_b = R) reconstructions with 1, 3, 6
    // terms. The Zernike profile is non-zero at the edge, which only the
    // Neumann series can represent.
    let mut z = vec![0.0; 15];
    z[0] = 1.0;
    z[4] = -0.3;
    z[12] = 0.1;
    nee_soon::power_shape::write_bessel_comparison_png(
        dir.join("zernike_vs_bessel_radial.png"),
        &z,
        l(90.0),
        l(90.0),
        &[1, 3, 6],
    )?;
    println!("wrote {}", dir.display());
    Ok(())
}
