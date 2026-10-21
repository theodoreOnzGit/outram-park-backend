// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # Verification of the HTTR model's construction (no nuclear data)
//!
//! **Methodology.** Each test checks one property of the built model against
//! a number R1 states (or that follows from R1 by arithmetic shown in the
//! test), never against the model's own output:
//! - the TRISO layer holds 333 whole particles, i.e. 181 818 per rod, and
//!   their uranium is R1's 188.58 g per rod;
//! - each material's total atom density is R1's table total;
//! - the core map has R1 Fig. 3.17's rings and roles, 4770 fuel rods;
//! - points located through the assembled geometry land in the material the
//!   specification puts there: a kernel of the right enrichment, a rotated
//!   column's BP pin and empty hole, a control rod's absorber at its
//!   critical height, an empty RSS hole;
//! - sampled uniformly in one compact, the kernel volume fraction is the
//!   specified 333 x (4/3 π 0.03³) per 0.1 cm of annulus;
//! - no point of the model is outside every cell.
//!
//! **Results (2026-10-16).** All pass; the measured numbers are in each
//! test's assertion message and in the V&V README.

use petir::mathf::RealMath;
use std::f64::consts::PI;

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};

use super::geometry::{assemble_httr, CompactTracking};
use super::materials::{httr_material_set, mat, nuc, HttrMaterialConfig, B11_PER_B10};
use super::spec::{self, core_map, hex_ring, ColumnRole, FuelZone, RodBank, RodPositions};

fn core() -> super::AssembledHttr {
    assemble_httr(RodPositions::CRITICAL, CompactTracking::Delta)
}

fn material_at(g: &outram_mc_libs::geometry::geometry::Geometry, x: f64, y: f64, z: f64) -> Option<usize> {
    let u = Direction::new(0.3, 0.4, (1.0f64 - 0.25).sqrt());
    g.locate(Position::new(x, y, z), u, SurfaceToken::NONE)
        .and_then(|p| p.material)
}

/// Column centre of the column named `name`.
fn centre(name: &str) -> (f64, f64) {
    core_map()
        .into_iter()
        .find(|c| c.name == name)
        .map(|c| c.centre())
        .unwrap_or_else(|| panic!("no column {name}"))
}

#[test]
fn the_layer_pattern_holds_333_whole_particles() {
    let mut n = 0;
    for (j, row) in spec::TRISO_LAYER.iter().enumerate() {
        assert_eq!(row.len(), 27);
        for (i, c) in row.bytes().enumerate() {
            if c == b'o' {
                n += 1;
                let x = (i as f64 - 13.0) * spec::TRISO_PITCH_CM[0];
                let y = (j as f64 - 13.0) * spec::TRISO_PITCH_CM[1];
                let r = x.r_hypot(y);
                let opyc = spec::TRISO_RADII_CM[4];
                assert!(
                    r - opyc > spec::COMPACT_INNER_R && r + opyc < spec::COMPACT_OUTER_R,
                    "particle ({i}, {j}) at r = {r} is not whole in the annulus"
                );
            }
        }
    }
    assert_eq!(n, spec::TRISO_PER_LAYER);
    // R1 §3.1.1: 12 987 per 3.9 cm compact x 14 compacts = 181 818 per rod.
    assert_eq!(n * spec::TRISO_LAYERS, 12_987 * 14);
    assert_eq!(core().particles_per_rod, 181_818);
}

/// R1 §3.1.3.1: the kernel density 10.40363 g/cm³ was chosen so that a rod
/// holds 188.58 g of uranium. From R1 Table 3.1's atom densities and the
/// built particle count, the 3.4 % kernel gives that mass to 0.05 %.
#[test]
fn a_rod_holds_r1s_188_58_g_of_uranium() {
    let v_kernel = 4.0 / 3.0 * PI * spec::TRISO_RADII_CM[0].r_powi(3);
    let na = 0.602_214_076; // atoms/(b cm) per mol/cm³
    let [u234, u235, u238] = super::materials::KERNEL_U[0];
    let grams_per_cm3 = (u234 * 234.040_952 + u235 * 235.043_930 + u238 * 238.050_788) / na;
    let mass = grams_per_cm3 * v_kernel * 181_818.0;
    assert!(
        (mass - 188.58).abs() / 188.58 < 5.0e-4,
        "uranium per rod {mass:.3} g against R1's 188.58 g"
    );
}

/// Each material's total atom density is R1's (Tables 3.1-3.9), less the
/// impurity B-11 this model adds; and that B-11 is natural boron's.
#[test]
fn material_totals_are_r1s_tables() {
    let m = httr_material_set(HttrMaterialConfig::benchmark(300.0));
    let spec_m = httr_material_set(HttrMaterialConfig::benchmark(300.0).without_impurity_b11());
    let total = |x: &outram_mc_libs::material::material::Material| -> f64 {
        x.components.iter().map(|c| c.atom_density).sum()
    };
    let table: [(usize, f64); 15] = [
        (mat::KERNEL0, 6.9614e-02),
        (mat::KERNEL0 + 11, 6.9634e-02),
        (mat::BUFFER, 5.5153e-02),
        (mat::SIC, 9.6122e-02),
        (mat::OVERCOAT, 8.5237e-02),
        (mat::MATRIX, 8.5237e-02),
        (mat::SLEEVE, 8.8747e-02),
        (mat::BP_2_0, 9.0451e-02),
        (mat::BP_2_5, 9.0501e-02),
        (mat::ABSORBER, 9.8436e-02),
        (mat::ALLOY_800H, 8.7530e-02),
        (mat::BLOCK, 8.7804e-02),
        (mat::PGX, 8.6134e-02),
        (mat::HELIUM, 2.4616e-05),
        (mat::DISKS, 8.8747e-02),
    ];
    for (i, want) in table {
        let got = total(&spec_m[i]);
        assert!(
            (got - want).abs() <= 2.0e-4 * want,
            "{}: total {got:.5e} against R1's {want:.5e}",
            spec_m[i].name
        );
    }
    // The B-11 added is exactly natural boron's partner of each B-10.
    let b = |x: &outram_mc_libs::material::material::Material, s: usize| -> f64 {
        x.components.iter().filter(|c| c.nuclide_idx == s).map(|c| c.atom_density).sum()
    };
    let block = &m[mat::BLOCK];
    assert!((b(block, nuc::B11) / b(block, nuc::B10) - B11_PER_B10).abs() < 1e-12);
    assert_eq!(b(&spec_m[mat::BLOCK], nuc::B11), 0.0);
    // The absorber's free carbon is a quarter of its boron (B4C).
    let a = &m[mat::ABSORBER];
    let free_c = b(a, nuc::C12_FREE) + b(a, nuc::C13_FREE);
    assert!((free_c - (6.3184e-03 + 2.5432e-02) / 4.0).abs() < 1e-12);
}

#[test]
fn the_core_map_has_rings_a_to_e() {
    let map = core_map();
    assert_eq!(map.len(), 61);
    let mut by_ring = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for c in &map {
        by_ring[hex_ring(c.i, c.j) as usize].push(c.role);
    }
    let count = |r: &[ColumnRole], want: ColumnRole| r.iter().filter(|&&x| x == want).count();
    use ColumnRole::*;
    assert_eq!(by_ring[0], vec![Control(RodBank::C)]);
    assert_eq!(count(&by_ring[1], Fuel(FuelZone::Z1)), 6);
    assert_eq!(count(&by_ring[2], Fuel(FuelZone::Z2)), 6);
    assert_eq!(count(&by_ring[2], Control(RodBank::R1)), 6);
    assert_eq!(count(&by_ring[3], Fuel(FuelZone::Z3)), 12);
    assert_eq!(count(&by_ring[3], Fuel(FuelZone::Z4)), 6);
    assert_eq!(count(&by_ring[4], Control(RodBank::R2)), 6);
    assert_eq!(count(&by_ring[4], Control(RodBank::R3)), 3);
    assert_eq!(count(&by_ring[4], Instrumentation), 3);
    assert_eq!(count(&by_ring[4], Reflector), 12);
    // Zone 4 is ring D's corners, R3 and instrumentation ring E's.
    for c in &map {
        let corner = c.i == 0 || c.j == 0 || c.i + c.j == 0;
        match c.role {
            Fuel(FuelZone::Z4) | Control(RodBank::R3) | Instrumentation => assert!(corner),
            Fuel(FuelZone::Z3) => assert!(!corner),
            _ => {}
        }
    }
    // R1 §1.1.2.3: 4770 fuel rods.
    assert_eq!(core().fuel_rods, 4770);
}

/// A kernel in column B04 (zone 1, unrotated), top fuel layer L1: R1 Table
/// 1.11 puts 6.7 wt.% there (material KERNEL0 + 7).
#[test]
fn a_kernel_in_b04_layer_1_is_6_7_percent() {
    let g = core().geometry;
    let (cx, cy) = centre("B04");
    // Pin at block index (1, 0): (5.15, 0). Particle at layer pattern
    // (i = 10, j = 2): (-0.318, -1.166). Layer centre z = 0.05 in the block
    // whose centre is z = 2 x 58.
    let (px, py) = (spec::PIN_PITCH, 0.0);
    let (tx, ty) = ((10.0 - 13.0) * 0.106, (2.0 - 13.0) * 0.106);
    let m = material_at(&g, cx + px + tx, cy + py + ty, 116.05);
    assert_eq!(m, Some(mat::KERNEL0 + 7), "B04 L1 kernel");
    // The same particle in layer L4/5 (block centre z = -58): 3.4 %.
    let m = material_at(&g, cx + px + tx, cy + py + ty, -58.0 + 0.05);
    assert_eq!(m, Some(mat::KERNEL0));
    // Zone 4 (D07, unrotated), layer L1: 9.9 %.
    let (dx, dy) = centre("D07");
    let m = material_at(&g, dx + px + tx, dy + py + ty, 116.05);
    assert_eq!(m, Some(mat::KERNEL0 + 11));
}

/// Column B01 is rotated by 180 degrees (deck `*trcl`): its empty BP hole,
/// at (+15.45, 0) in an unrotated block, is at (-15.45, 0), and a BP pin at
/// (7.725, 13.38).
#[test]
fn a_rotated_column_carries_its_bp_pins_rotated() {
    let g = core().geometry;
    let (cx, cy) = centre("B01");
    let s = spec::PIN_PITCH;
    // L1 block (z centre 116): 2.0 wt.% BP; the lower poison stack is at
    // z_local -25..-5.
    assert_eq!(material_at(&g, cx - 3.0 * s, cy, 116.0 - 15.0), Some(mat::HELIUM));
    let (bx, by) = (1.5 * s, 3.0 * s * 3.0_f64.sqrt() / 2.0);
    assert_eq!(material_at(&g, cx + bx, cy + by, 116.0 - 15.0), Some(mat::BP_2_0));
    // L2 (z centre 58): 2.5 wt.%.
    assert_eq!(material_at(&g, cx + bx, cy + by, 58.0 - 15.0), Some(mat::BP_2_5));
    // And unrotated B04 has its empty hole at +x.
    let (ux, uy) = centre("B04");
    assert_eq!(material_at(&g, ux + 3.0 * s, uy, 116.0 - 15.0), Some(mat::HELIUM));
    assert_eq!(material_at(&g, ux - 3.0 * s, uy, 116.0 - 15.0), Some(mat::HELIUM), "fuel-pin helium hole");
}

/// The centre rod (A01) at the critical height: absorber 15 cm above its
/// bottom (fuel bottom + 177.5 cm) in the 0-degree hole, helium below it,
/// and the 120-degree RSS hole empty.
#[test]
fn the_centre_rod_sits_at_its_critical_height() {
    let g = core().geometry;
    let zb = spec::FUEL_BOTTOM_Z + RodPositions::CRITICAL.c;
    let x = spec::CR_HOLE_OFFSET + 4.5; // r = 4.5 cm: in the B4C annulus
    assert_eq!(material_at(&g, x, 0.0, zb + 15.0), Some(mat::ABSORBER));
    assert_eq!(material_at(&g, x, 0.0, zb - 5.0), Some(mat::HELIUM));
    assert_eq!(material_at(&g, x, 0.0, zb + 0.5), Some(mat::ALLOY_800H), "bottom end plate");
    let a = 120.0 * PI / 180.0;
    let (rx, ry) = (spec::CR_HOLE_OFFSET * a.r_cos(), spec::CR_HOLE_OFFSET * a.r_sin());
    assert_eq!(material_at(&g, rx + 4.5, ry, zb + 15.0), Some(mat::HELIUM), "RSS hole");
    // Below the hole bottom the column is solid graphite.
    assert_eq!(material_at(&g, x, 0.0, spec::CR_HOLE_BOTTOM_Z - 1.0), Some(mat::BLOCK));
    // The subcritical state lowers the rod to -5.5 cm.
    let sub = assemble_httr(RodPositions::SUBCRITICAL, CompactTracking::Delta).geometry;
    let zs = spec::FUEL_BOTTOM_Z - 5.5;
    assert_eq!(material_at(&sub, x, 0.0, zs + 15.0), Some(mat::ABSORBER));
}

/// The kernel volume fraction of one compact, sampled through the global
/// geometry, is 333 x (4/3 π r³) / (π (1.3² - 0.5²) x 0.1) = 0.08324.
#[test]
fn the_compact_kernel_fraction_is_the_specified_one() {
    let g = core().geometry;
    let (cx, cy) = centre("C02");
    let (px, py) = (spec::PIN_PITCH, 0.0);
    let want = 333.0 * 4.0 / 3.0 * PI * 0.03f64.r_powi(3) / (PI * (1.3f64.r_powi(2) - 0.25) * 0.1);
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut prn = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut hits, mut n) = (0usize, 0usize);
    while n < 200_000 {
        let r = (0.25 + prn() * (1.69 - 0.25)).sqrt();
        let t = 2.0 * PI * prn();
        let z = -20.0 + 40.0 * prn();
        n += 1;
        let m = material_at(&g, cx + px + r * t.r_cos(), cy + py + r * t.r_sin(), z);
        if m.is_some_and(|m| m < 12) {
            hits += 1;
        }
    }
    let f = hits as f64 / n as f64;
    let sigma = (want * (1.0 - want) / n as f64).sqrt();
    assert!(
        (f - want).abs() < 4.0 * sigma,
        "kernel fraction {f:.5} against {want:.5} (4 sigma = {:.5})",
        4.0 * sigma
    );
}

/// No point inside the model is outside every cell.
#[test]
fn no_point_of_the_model_is_lost() {
    let g = core().geometry;
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut prn = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let r_max = spec::PERMANENT_REFLECTOR_R - 1e-6;
    for _ in 0..50_000 {
        let r = r_max * prn().sqrt();
        let t = 2.0 * PI * prn();
        let z = (2.0 * prn() - 1.0) * (spec::CORE_HALF_HEIGHT - 1e-6);
        let (x, y) = (r * t.r_cos(), r * t.r_sin());
        assert!(material_at(&g, x, y, z).is_some(), "lost at ({x}, {y}, {z})");
    }
}
