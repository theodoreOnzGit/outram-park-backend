// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! The HTR-10 R-Z solid model as a cited mesh-builder input
//! (IAEA-TECDOC-1694 Sec 2.2.1.2, Tab 2.3, Tab 2.4, Fig 2.5).
//!
//! **Coordinates.** As in TECDOC-1694 Sec 2.1.1.3 and the input sheet
//! `docs/reactor-scoping/htr10-th-benchmark.md`: `r` from the core axis,
//! `z = 0` at the upper surface of the pebble bed, **`z` positive downward**,
//! cm in this file. The mesh is built in that coordinate, so the mesh's
//! [`RzPatch::Bottom`](crate::genfoam::thermal_hydraulics::solid_conduction::RzPatch)
//! (`z = z_min = -351 cm`) is the **physical top** (the outer surface of the
//! RPV top head) and its `Top` patch (`z = 665 cm`) is the **physical
//! bottom** (the outer surface of the RPV bottom head).
//!
//! **Domain.** `0 <= r <= 218 cm` (RPV outer surface, Tab 2.3: ID 420, wall
//! 8 cm), `-351 <= z <= 665 cm` (RPV top head to bottom head). The cavity
//! outside the RPV and the RCCS panels are not meshed: the RPV outer surface
//! radiates to them as a boundary condition (see `model.rs`).
//!
//! **Confidence.** Every extent below is read from Fig 2.5 in the input sheet
//! and is **Uncertain** there; it stays Uncertain here. Each rectangle says
//! whether its extent is the sheet's Fig 2.5 reading, a Tab 2.3 dimension
//! (Quoted), arithmetic on those (Derived), or a fill this module had to
//! choose because the sheet records the extent as not resolved (**Assumed**).
//! The mesh lines are the Fig 2.5 printed labels (Quoted from legible
//! labels), so every region boundary lies on a face.

use super::materials::RegionMaterial;

/// How a region's extent is known (the input sheet's confidence levels,
/// plus `Assumed` for fills this module chose).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtentConfidence {
    /// Read from TECDOC-1694 Fig 2.5 by the input sheet: **Uncertain**.
    Fig25Uncertain,
    /// A Tab 2.3 dimension (Quoted) or arithmetic on one (Derived).
    Tab23,
    /// Not resolved in the input sheet; this module's fill, stated.
    Assumed,
}

/// One homogenised region of the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionSpec {
    /// Region number in TECDOC-1694 Tab 2.4.
    pub tab24: u8,
    /// Short label (upper case: the drawings' bitmap font has no lower case).
    pub name: &'static str,
    /// Material and homogenisation.
    pub material: RegionMaterial,
}

/// An axis-aligned rectangle in TECDOC coordinates (cm), painted in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CitedRect {
    /// Index into [`regions`].
    pub region: usize,
    /// Inner and outer radius, cm.
    pub r_cm: [f64; 2],
    /// Upper and lower `z` (TECDOC, downward), cm.
    pub z_cm: [f64; 2],
    /// Where the extent comes from.
    pub confidence: ExtentConfidence,
}

/// Where the core vessel (barrel) sits: input-sheet conflict N5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoreVesselPlacement {
    /// `r 194-197 cm`, the Fig 2.5 reading (implies ID 388 cm), with the
    /// helium gap 38 at `r 190-194`. **The default**: it is the INET model
    /// the benchmark's own region table describes.
    #[default]
    Fig25,
    /// `r 191-194 cm`, Tab 2.3's ID 382 cm and 3 cm wall; gap 38 shrinks to
    /// 1 cm and gap 23 widens to `r 194-210`. Sensitivity case `N5-TAB23`.
    Tab23,
}

/// Radial mesh lines, cm: the Fig 2.5 labels from 0 to the RPV outer
/// surface (218), as printed (input sheet Sec 3.2). The Tab 2.3 barrel
/// placement adds the line at 191.
#[must_use]
pub fn radial_lines_cm(vessel: CoreVesselPlacement) -> Vec<f64> {
    let mut v = vec![
        0.0, 6.5, 13.0, 19.0, 25.0, 39.0, 50.0, 60.0, 70.5, 83.5, 90.0, 95.6, 108.6, 140.6, 148.6,
        167.5, 178.8, 190.0, 194.0, 197.0, 210.0, 218.0,
    ];
    if vessel == CoreVesselPlacement::Tab23 {
        v.insert(18, 191.0);
    }
    v
}

/// Axial mesh lines, cm (TECDOC `z`, downward): the Fig 2.5 labels from the
/// RPV top head (-351) to the RPV bottom head (665), as printed.
pub const AXIAL_LINES_CM: [f64; 40] = [
    -351.0, -343.0, -219.0, -170.0, -130.0, -75.0, -65.0, -40.0, 0.0, 18.0, 36.0, 54.0, 72.0, 90.0,
    108.0, 126.0, 144.0, 162.0, 180.0, 187.5, 195.0, 202.5, 210.0, 217.5, 222.2, 227.0, 232.0,
    263.5, 295.0, 325.0, 340.0, 360.0, 400.0, 440.0, 446.0, 461.0, 484.0, 507.0, 657.0, 665.0,
];

/// Model region indices (positions in [`regions`]).
#[allow(missing_docs)]
pub mod idx {
    pub const BED: usize = 0;
    pub const SIDE_GRAPHITE: usize = 1;
    pub const TOP_BOTTOM_GRAPHITE: usize = 2;
    pub const CR_RING: usize = 3;
    pub const RISERS: usize = 4;
    pub const CARBON_BRICK: usize = 5;
    pub const RPV: usize = 6;
    pub const CORE_VESSEL: usize = 7;
    pub const UPPER_BOTTOM_REFL: usize = 8;
    pub const LOWER_BOTTOM_REFL: usize = 9;
    pub const BOTTOM_STEEL_1: usize = 10;
    pub const BOTTOM_STEEL_2: usize = 11;
    pub const BOTTOM_ANNULAR_HE: usize = 12;
    pub const LEAK_STEEL: usize = 13;
    pub const SIDE_INNER_GRAPHITE: usize = 14;
    pub const TOP_HE_CAVITY: usize = 15;
    pub const CORE_ENTRY_PLENUM: usize = 16;
    pub const BOTTOM_ROUNDING_HE: usize = 17;
    pub const BOTTOM_HE_CAVITY: usize = 18;
    pub const GAP_VESSEL_RPV: usize = 19;
    pub const HOT_PLENUM: usize = 20;
    pub const SMALL_PLENUM: usize = 21;
    pub const DISCHARGE_THROTTLE: usize = 22;
    pub const CR_EXIT_THROTTLE: usize = 23;
    pub const BOTTOM_REFL_SOLID: usize = 24;
    pub const GAP_BRICK_VESSEL: usize = 25;
    pub const HE_ENTRY_PATH: usize = 26;
    pub const ANNULAR_PATH: usize = 27;
    pub const TOP_PLENUM: usize = 28;
    pub const TOP_FLOW_ZONE: usize = 29;
    pub const DISCHARGE_TUBE: usize = 30;
}

/// Number of model regions.
pub const N_REGIONS: usize = 31;

/// The 31 model regions. Tab 2.4 region 1 (pebble bed) is split into the
/// core with its cone (the Tab 2.3 5 m^3 core volume, which is what a held or
/// heated bed means here) and the discharge tube below it. Tab 2.4 region 2 ("graphite without flow path") is
/// split into its side-reflector and top/bottom-reflector parts because
/// Tab 2.11 gives the two different conductivity fits; every other model
/// region is one Tab 2.4 region. Tab 2.4 regions outside the RPV (15, 16, 17,
/// 24, 26, 27, 35, 36, 37, 39, 42) are represented by the boundary
/// conditions, and 22, 32-34 are null.
///
/// The widths of the two annular gaps around the core vessel follow its
/// placement (input sheet N5): 4 and 13 cm with the Fig 2.5 barrel, 1 and
/// 16 cm with the Tab 2.3 one, so the radiative conductivity of each gap is
/// always that of the gap actually meshed.
#[must_use]
pub fn regions(vessel: CoreVesselPlacement) -> [RegionSpec; N_REGIONS] {
    use RegionMaterial as M;
    let (w38, w23) = match vessel {
        CoreVesselPlacement::Fig25 => (4.0, 13.0),
        CoreVesselPlacement::Tab23 => (1.0, 16.0),
    };
    let gas = |emissivity: f64, width_cm: f64| M::HeliumCavity {
        emissivity,
        width_m: width_cm / 100.0,
    };
    [
        RegionSpec { tab24: 1, name: "R1 PEBBLE BED (CORE)", material: M::PebbleBed },
        RegionSpec { tab24: 2, name: "R2 SIDE GRAPHITE", material: M::SideReflector { solid_fraction: 1.0 } },
        RegionSpec { tab24: 2, name: "R2 TOP/BOT GRAPHITE", material: M::TopBottomReflector { solid_fraction: 1.0 } },
        RegionSpec { tab24: 3, name: "R3 CR CHANNEL RING", material: M::SideReflector { solid_fraction: 0.793 } },
        RegionSpec { tab24: 4, name: "R4 RISER RING", material: M::SideReflector { solid_fraction: 0.862 } },
        RegionSpec { tab24: 5, name: "R5 CARBON BRICK", material: M::CarbonBrick },
        RegionSpec { tab24: 6, name: "R6 RPV", material: M::RpvSteel { solid_fraction: 1.0 } },
        RegionSpec { tab24: 7, name: "R7 CORE VESSEL", material: M::BarrelSteel { solid_fraction: 1.0 } },
        RegionSpec { tab24: 8, name: "R8 UPPER BOT REFL", material: M::TopBottomReflector { solid_fraction: 0.769 } },
        RegionSpec { tab24: 9, name: "R9 LOWER BOT REFL", material: M::TopBottomReflector { solid_fraction: 0.932 } },
        RegionSpec { tab24: 10, name: "R10 BOT STEEL 1", material: M::BarrelSteel { solid_fraction: 0.5 } },
        RegionSpec { tab24: 11, name: "R11 BOT STEEL 2", material: M::BarrelSteel { solid_fraction: 1.0 } },
        // Tab 2.4: helium, solid fraction 0.01 (neglected). Emissivity not in
        // Tab 2.13: 0.85 assumed.
        RegionSpec { tab24: 12, name: "R12 BOT ANNULAR HE", material: gas(0.85, 11.0) },
        RegionSpec { tab24: 13, name: "R13 LEAK STEEL", material: M::BarrelSteel { solid_fraction: 0.96 } },
        RegionSpec { tab24: 14, name: "R14 SIDE INNER GRAPH", material: M::SideReflector { solid_fraction: 1.0 } },
        // Tab 2.13 "helium cavity inside the RPV" 0.79; width = its axial
        // height -343..-170.
        RegionSpec { tab24: 18, name: "R18 TOP HE CAVITY", material: gas(0.79, 173.0) },
        RegionSpec { tab24: 19, name: "R19 CORE ENTRY PLENUM", material: gas(0.85, 40.0) },
        // Not in Tab 2.13: 0.85 assumed.
        RegionSpec { tab24: 20, name: "R20 BOT ROUNDING HE", material: gas(0.85, 15.0) },
        // Not in Tab 2.13: 0.79 assumed (as region 18, the other RPV cavity).
        RegionSpec { tab24: 21, name: "R21 BOT HE CAVITY", material: gas(0.79, 150.0) },
        RegionSpec { tab24: 23, name: "R23 GAP VESSEL-RPV", material: gas(0.85, w23) },
        RegionSpec { tab24: 25, name: "R25 HOT PLENUM", material: gas(0.85, 30.0) },
        // Not in Tab 2.13: 0.85 assumed.
        RegionSpec { tab24: 28, name: "R28 SMALL PLENUM", material: gas(0.85, 5.0) },
        RegionSpec { tab24: 29, name: "R29 TUBE THROTTLE", material: M::TopBottomReflector { solid_fraction: 0.993 } },
        RegionSpec { tab24: 30, name: "R30 CR EXIT THROTTLE", material: M::SideReflector { solid_fraction: 0.998 } },
        RegionSpec { tab24: 31, name: "R31 BOT REFL SOLID", material: M::TopBottomReflector { solid_fraction: 1.0 } },
        RegionSpec { tab24: 38, name: "R38 GAP BRICK-VESSEL", material: gas(0.82, w38) },
        // Not in Tab 2.13: 0.79 assumed (the same annulus as region 41).
        RegionSpec { tab24: 40, name: "R40 HE ENTRY PATH", material: gas(0.79, w23) },
        RegionSpec { tab24: 41, name: "R41 ANNULAR PATH", material: gas(0.79, w23) },
        RegionSpec { tab24: 43, name: "R43 TOP PLENUM", material: gas(0.85, 10.0) },
        RegionSpec { tab24: 44, name: "R44 TOP FLOW ZONE", material: M::TopBottomReflector { solid_fraction: 0.911 } },
        RegionSpec { tab24: 1, name: "R1 DISCHARGE TUBE", material: M::PebbleBed },
    ]
}

/// The region map: the default region (the top helium cavity, Tab 2.4
/// region 18, which fills whatever inside the RPV no rectangle claims) and the
/// rectangles in painting order (the last one containing a point wins).
#[must_use]
pub fn region_rects(vessel: CoreVesselPlacement) -> (usize, Vec<CitedRect>) {
    use idx::*;
    use ExtentConfidence::{Assumed, Fig25Uncertain as F, Tab23 as T};
    let r = |region, r0, r1, z0, z1, confidence| CitedRect {
        region,
        r_cm: [r0, r1],
        z_cm: [z0, z1],
        confidence,
    };
    let (gap38, vessel_r, gap23_inner) = match vessel {
        CoreVesselPlacement::Fig25 => ((190.0, 194.0), (194.0, 197.0), 197.0),
        CoreVesselPlacement::Tab23 => ((190.0, 191.0), (191.0, 194.0), 194.0),
    };
    let rects = vec![
        // Inside the RPV, outermost first.
        r(BOTTOM_HE_CAVITY, 0.0, 210.0, 507.0, 657.0, F),
        // Region 23 "above z 295"; its top end is not resolved: run to the
        // RPV head (Assumed).
        r(GAP_VESSEL_RPV, gap23_inner, 210.0, -343.0, 295.0, F),
        r(HE_ENTRY_PATH, gap23_inner, 210.0, 295.0, 325.0, F),
        r(ANNULAR_PATH, gap23_inner, 210.0, 325.0, 507.0, F),
        // Barrel and the gap inside it: z -219..507 Assumed. Derived check:
        // 726 cm against Tab 2.3's 737 cm height (-1.5 %).
        r(GAP_BRICK_VESSEL, gap38.0, gap38.1, -219.0, 507.0, Assumed),
        r(CORE_VESSEL, vessel_r.0, vessel_r.1, -219.0, 507.0, Assumed),
        // Top boronated brick, 40 cm (Tab 2.3), on the top reflector.
        r(CARBON_BRICK, 0.0, 190.0, -170.0, -130.0, T),
        r(CARBON_BRICK, 167.5, 190.0, -170.0, 440.0, F),
        // Top reflector, 90 cm (Tab 2.3): z -130..-40.
        r(TOP_BOTTOM_GRAPHITE, 0.0, 90.0, -130.0, -40.0, T),
        // Side reflector column; its top at the top reflector's top and its
        // bottom at the bottom brick's bottom are Assumed.
        r(SIDE_GRAPHITE, 90.0, 167.5, -130.0, 440.0, Assumed),
        r(SIDE_INNER_GRAPHITE, 90.0, 95.6, -130.0, 227.0, F),
        r(CR_RING, 95.6, 108.6, -130.0, 222.2, F),
        r(CR_EXIT_THROTTLE, 95.6, 108.6, 222.2, 227.0, F),
        // Top plenum: Tab 2.3 cold plenum OD 166 / ID 50 / height 10 cm ->
        // r 25..83, placed on the Fig 2.5 line 83.5 (Derived, +0.5 cm).
        r(TOP_PLENUM, 25.0, 83.5, -75.0, -65.0, T),
        r(TOP_FLOW_ZONE, 0.0, 90.0, -65.0, -40.0, F),
        r(CORE_ENTRY_PLENUM, 0.0, 90.0, -40.0, 0.0, F),
        // Bottom structures under the cone (the bed overpaints the cone).
        r(BOTTOM_REFL_SOLID, 60.0, 90.0, 180.0, 227.0, F),
        r(UPPER_BOTTOM_REFL, 39.0, 60.0, 180.0, 227.0, F),
        r(BOTTOM_REFL_SOLID, 0.0, 39.0, 180.0, 295.0, F),
        r(SMALL_PLENUM, 39.0, 108.6, 227.0, 232.0, F),
        r(LOWER_BOTTOM_REFL, 39.0, 90.0, 232.0, 295.0, F),
        r(HOT_PLENUM, 25.0, 90.0, 295.0, 325.0, F),
        r(TOP_BOTTOM_GRAPHITE, 25.0, 39.0, 325.0, 340.0, F),
        r(TOP_BOTTOM_GRAPHITE, 50.0, 140.6, 325.0, 340.0, F),
        r(LEAK_STEEL, 39.0, 50.0, 325.0, 446.0, F),
        // Bottom brick, 30 + 70 cm (Tab 2.3) = z 340..440.
        r(CARBON_BRICK, 25.0, 39.0, 340.0, 440.0, F),
        r(CARBON_BRICK, 50.0, 140.6, 340.0, 440.0, F),
        r(BOTTOM_STEEL_2, 50.0, 190.0, 440.0, 446.0, F),
        r(BOTTOM_STEEL_2, 25.0, 39.0, 440.0, 507.0, F),
        r(BOTTOM_ROUNDING_HE, 39.0, 148.6, 446.0, 461.0, F),
        r(BOTTOM_STEEL_1, 148.6, 190.0, 446.0, 461.0, Assumed),
        r(BOTTOM_STEEL_1, 50.0, 190.0, 461.0, 507.0, F),
        r(BOTTOM_ANNULAR_HE, 39.0, 50.0, 461.0, 507.0, F),
        // Pebble bed: core, stepped cone (6 x 7.5 cm, Derived 37.5 cm =
        // Tab 2.3 cone height), discharge tube.
        r(BED, 0.0, 90.0, 0.0, 180.0, F),
        r(BED, 0.0, 83.5, 180.0, 187.5, F),
        r(BED, 0.0, 70.5, 187.5, 195.0, F),
        r(BED, 0.0, 60.0, 195.0, 202.5, F),
        r(BED, 0.0, 50.0, 202.5, 210.0, F),
        r(BED, 0.0, 39.0, 210.0, 217.5, F),
        r(DISCHARGE_TUBE, 0.0, 25.0, 217.5, 484.0, F),
        r(DISCHARGE_THROTTLE, 0.0, 25.0, 484.0, 507.0, F),
        // Risers, Tab 2.5 region 10 extent z -65..446, painted over the
        // bottom steel at 440..446.
        r(RISERS, 140.6, 148.6, -65.0, 446.0, F),
        // RPV: side wall r 210..218 (Tab 2.3 ID 420, 8 cm), bottom head
        // z 657..665 (Fig 2.5), top head z -351..-343 (Assumed: the only
        // 8 cm interval of the Fig 2.5 lines above the top cavity; Derived
        // check 665 - (-351) = 1016 cm against Tab 2.3's total 1115.5 cm).
        r(RPV, 210.0, 218.0, -351.0, 665.0, T),
        r(RPV, 0.0, 218.0, 657.0, 665.0, F),
        r(RPV, 0.0, 218.0, -351.0, -343.0, Assumed),
    ];
    (TOP_HE_CAVITY, rects)
}

/// Region at `(r, z)` in cm (TECDOC coordinates), by the painting rule.
#[must_use]
pub fn region_at_cm(rects: &[CitedRect], default_region: usize, r: f64, z: f64) -> usize {
    rects
        .iter()
        .rev()
        .find(|b| r >= b.r_cm[0] && r <= b.r_cm[1] && z >= b.z_cm[0] && z <= b.z_cm[1])
        .map_or(default_region, |b| b.region)
}
