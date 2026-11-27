// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             heatExchanger/heatExchanger.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `heat_exchanger` — two-sided wall coupling of primary and secondary fluids
//!
//! Port of the **flux-conservation wall-temperature solve** at the heart of
//! GeN-Foam's `heatExchanger::correct`. A heat exchanger couples a primary and a
//! secondary fluid region across a conducting tube wall of series conductance
//! `H_w = k_wall / t_wall` ([`WallConductance`]). GeN-Foam requires the heat
//! flux through the wall to equal the convective flux into each fluid, giving a
//! 2×2 linear system for the two wall-surface temperatures `T_p`, `T_s`:
//!
//! ```text
//! T_s = A_p*T_p - B_p ,   T_p = A_s*T_s - B_s
//! ```
//!
//! with, on each side `i` (`p` or `s`),
//!
//! ```text
//! A_i = (H_w + h_sum_i) / H_w        (dimensionless)
//! B_i = ht_sum_i / H_w               (temperature)
//! ```
//!
//! where `h_sum_i`/`ht_sum_i` are that side's fluid-coupling sums (the same
//! `H`/`HT` pair used everywhere else in this module — see
//! [`super::heat_source`]). The closed-form solution is
//!
//! ```text
//! T_p = (B_p*A_s + B_s) / (A_p*A_s - 1)
//! T_s = (B_s*A_p + B_p) / (A_s*A_p - 1)
//! ```
//!
//! guarded against the degenerate `A_p*A_s = 1` case (both convective
//! coefficients zero ⇒ no heat exchanged) exactly as upstream, with a `1e-69`
//! floor on the denominator.
//!
//! ## Scope
//!
//! [`hx_wall_temperature`] ports the per-cell algebra. ~~GeN-Foam additionally handles the **mesh-to-mesh mapping** ... that mapping
//! belongs to the solver bead (op-p6p.7.11). Here the caller supplies both sides' already-mapped coupling sums.~~ **CORRECTED
//! 2026-11-27 (gh:#1847, README round 44):** the whole of `structure::constructHeatExchangers`, `heatExchanger::constructSecondaryHX`
//! and `heatExchanger::correct` is ported: [`HeatExchangerZone`] (the zone centre-of-volume translation of the secondary mesh and the
//! exact `meshToMesh` mapping of round 28), [`HeatExchangerSet`] (`iAHX`, `T.HX`, the `max(THX) > 1e-69` guard, the current-`T`
//! secondary lookup) and its use in the energy equations of `one_phase_fc` and `one_phase_cc` (compared with the real binary on
//! `featureCases/1D_HX/onePhase`). **Refused by name:** a `secondaryRegion` other than the region itself, an `interpolationMethod`
//! other than 2, and the two-phase region (gh:#1867).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use uom::si::f64::ThermodynamicTemperature;
use outram_foam_basic_lib::prelude::Vector3;

use crate::genfoam::multi_region::cell_overlap::{CellVolumeWeightAddressing, OverlapMesh};

use super::units::{HeatFlux, HeatTransferCoefficient, WallConductance};

/// A heat-exchanger wall: its series conductance.
///
/// Carries the tube-wall conductance `H_w = k_wall / t_wall`. Evaluate the
/// wall-surface temperatures with [`HeatExchanger::wall_temperature`], which
/// wraps [`hx_wall_temperature`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatExchanger {
    /// Wall conductance `H_w` [W/(m^2 K)].
    wall_conductance: WallConductance,
}

impl HeatExchanger {
    /// Build a heat exchanger from its wall conductance `H_w = k_wall/t_wall`.
    #[must_use]
    pub fn new(wall_conductance: WallConductance) -> Self {
        Self { wall_conductance }
    }

    /// Wall conductance `H_w` [W/(m^2 K)].
    #[must_use]
    pub fn wall_conductance(&self) -> WallConductance {
        self.wall_conductance
    }

    /// The **primary-side** wall-surface temperature `T_p` [K] for this cell.
    ///
    /// Convenience wrapper over [`hx_wall_temperature`] using this exchanger's
    /// wall conductance. Pass the primary side's coupling sums first, then the
    /// secondary side's (already mapped to this cell).
    #[must_use]
    pub fn wall_temperature(
        &self,
        h_sum_primary: HeatTransferCoefficient,
        ht_sum_primary: HeatFlux,
        h_sum_secondary: HeatTransferCoefficient,
        ht_sum_secondary: HeatFlux,
    ) -> ThermodynamicTemperature {
        hx_wall_temperature(
            self.wall_conductance,
            h_sum_primary,
            ht_sum_primary,
            h_sum_secondary,
            ht_sum_secondary,
        )
    }
}

/// Primary-side heat-exchanger wall-surface temperature `T_p` [K].
///
/// Port of the primary loop of `heatExchanger::correct`:
/// `T_p = (B_p*A_s + B_s) / max(A_p*A_s - 1, 1e-69)` (see the [module
/// documentation](self) for the derivation and the `A_i`, `B_i` definitions).
/// The `1e-69` floor reproduces upstream's guard against the no-heat-transfer
/// degeneracy where both convective coefficients vanish.
#[must_use]
pub fn hx_wall_temperature(
    wall_conductance: WallConductance,
    h_sum_primary: HeatTransferCoefficient,
    ht_sum_primary: HeatFlux,
    h_sum_secondary: HeatTransferCoefficient,
    ht_sum_secondary: HeatFlux,
) -> ThermodynamicTemperature {
    use uom::si::heat_flux_density::watt_per_square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    let hw = wall_conductance.get::<watt_per_square_meter_kelvin>();
    let hp = h_sum_primary.get::<watt_per_square_meter_kelvin>();
    let hs = h_sum_secondary.get::<watt_per_square_meter_kelvin>();
    let htp = ht_sum_primary.get::<watt_per_square_meter>();
    let hts = ht_sum_secondary.get::<watt_per_square_meter>();

    let a_p = (hw + hp) / hw;
    let b_p = htp / hw;
    let a_s = (hw + hs) / hw;
    let b_s = hts / hw;

    let t_p = (b_p * a_s + b_s) / (a_p * a_s - 1.0).max(1e-69);
    ThermodynamicTemperature::new::<kelvin>(t_p)
}

// ---------------------------------------------------------------------------------------------------------------------
// README round 44 (gh:#1847): the heat exchangers of `structureProperties.heatExchangers` as the solvers use them
// ---------------------------------------------------------------------------------------------------------------------

/// One `heatExchanger` of `structureProperties.heatExchangers`: its dictionary values, the primary cell zone and the mapping
/// from the secondary mesh (translated onto the primary zone) to the primary mesh.
///
/// Upstream: `heatExchanger::heatExchanger` (`volumetricArea`, `wallConductance` floored at `1e-69`, `primary`) and
/// `heatExchanger::constructSecondaryHX` (centre-of-volume translation, `meshToMesh(translatedMesh, mesh_, imCellVolumeWeight,
/// pmAABB, false)`, built here with [`CellVolumeWeightAddressing::new_normalised`], the exact port of round 28).
#[derive(Debug, Clone)]
pub struct HeatExchangerZone {
    /// `primaryCells_`: the cells of the primary cellZone, in the zone's order.
    pub primary_cells: Vec<usize>,
    /// `iA_` [1/m]: the volumetric wall area.
    pub i_a: f64,
    /// `Hw_ = max(wallConductance, 1e-69)` [W/m^2/K].
    pub hw: f64,
    /// `mappingPtr_`: source = the translated secondary mesh, target = the primary mesh.
    pub mapping: CellVolumeWeightAddressing,
    /// CONTROL, not upstream (README round 44, C2): the same mapping with every primary cell taking its NEAREST translated
    /// secondary cell with weight 1. Used only by [`HeatExchangerSet::with_nearest_cell_mapping`].
    pub nearest_cell_mapping: CellVolumeWeightAddressing,
    /// `delta`: the translation applied to the secondary mesh (`pCOV - sCOV`), logged by upstream as
    /// `Heat exchanger <name> mapping: applied translation of (..) m`.
    pub translation: Vector3,
}

/// The raw polyMesh lists and the geometry `heatExchanger::constructSecondaryHX` reads from `mesh_` and the secondary mesh
/// (here the same mesh: `secondaryRegion` defaults to the region itself).
#[derive(Debug, Clone, Copy)]
pub struct HxMeshInput<'a> {
    /// `points`.
    pub points: &'a [Vector3],
    /// `faces`.
    pub faces: &'a [Vec<usize>],
    /// `owner` (all faces).
    pub owner: &'a [usize],
    /// `neighbour` (internal faces).
    pub neighbour: &'a [usize],
    /// `V()`.
    pub cell_volumes: &'a [f64],
    /// `C()`.
    pub cell_centres: &'a [Vector3],
}

/// Why a heat exchanger could not be built (upstream's `FatalError`, plus inconsistent mesh lists).
#[derive(Debug, Clone, PartialEq)]
pub enum HeatExchangerError {
    /// `Primary and secondary zone volumes differ by <errV>` (`(pV - sV)/pV >= 1e-2`, signed as upstream).
    VolumeMismatch(f64),
    /// The mesh lists are inconsistent ([`OverlapMesh::new`]).
    Mesh(String),
}

impl core::fmt::Display for HeatExchangerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HeatExchangerError::VolumeMismatch(e) => write!(f, "Primary and secondary zone volumes differ by {e}"),
            HeatExchangerError::Mesh(m) => write!(f, "heat exchanger mesh: {m}"),
        }
    }
}

impl core::error::Error for HeatExchangerError {}

/// `sum_i V_i C_i / sum_i V_i` over a zone, in the zone's order (`constructSecondaryHX`:
/// `pCOV += primV[celli]*primC[celli]; pV += primV[celli]; pCOV /= pV`), and `sum_i V_i`.
fn centre_of_volume(cells: &[usize], v: &[f64], c: &[Vector3]) -> (Vector3, f64) {
    let mut cov = Vector3::new(0.0, 0.0, 0.0);
    let mut vol = 0.0;
    for &i in cells {
        let w = v[i];
        let ci = c[i];
        cov = Vector3::new(cov.x + w * ci.x, cov.y + w * ci.y, cov.z + w * ci.z);
        vol += w;
    }
    (Vector3::new(cov.x / vol, cov.y / vol, cov.z / vol), vol)
}

/// CONTROL (README round 44, C2): every target cell gets the nearest source cell centre with weight 1 (no overlap volumes).
fn nearest_cell_addressing(src: &OverlapMesh, tgt: &OverlapMesh) -> CellVolumeWeightAddressing {
    let sc = src.cell_centres();
    let tc = tgt.cell_centres();
    let mut tgt_to_src_addr = Vec::with_capacity(tc.len());
    for t in &tc {
        let mut best = 0usize;
        let mut best_d = f64::INFINITY;
        for (i, s) in sc.iter().enumerate() {
            let d = (s.x - t.x) * (s.x - t.x) + (s.y - t.y) * (s.y - t.y) + (s.z - t.z) * (s.z - t.z);
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        tgt_to_src_addr.push(vec![best]);
    }
    CellVolumeWeightAddressing {
        src_to_tgt_addr: vec![Vec::new(); sc.len()],
        src_to_tgt_wght: vec![Vec::new(); sc.len()],
        tgt_to_src_wght: vec![vec![1.0]; tc.len()],
        tgt_to_src_addr,
        v: 0.0,
    }
}

impl HeatExchangerZone {
    /// `heatExchanger::heatExchanger` + `constructSecondaryHX` for a secondary zone in the SAME mesh.
    ///
    /// # Errors
    /// [`HeatExchangerError::VolumeMismatch`] when the zone volumes differ by 1 % or more (upstream aborts), and
    /// [`HeatExchangerError::Mesh`] for inconsistent mesh lists.
    pub fn new(
        mesh: &HxMeshInput<'_>,
        primary_cells: Vec<usize>,
        secondary_cells: &[usize],
        i_a: f64,
        wall_conductance: f64,
    ) -> Result<Self, HeatExchangerError> {
        let (p_cov, p_v) = centre_of_volume(&primary_cells, mesh.cell_volumes, mesh.cell_centres);
        let (s_cov, s_v) = centre_of_volume(secondary_cells, mesh.cell_volumes, mesh.cell_centres);
        let delta = Vector3::new(p_cov.x - s_cov.x, p_cov.y - s_cov.y, p_cov.z - s_cov.z);
        let err_v = (p_v - s_v) / p_v;
        if (p_v - s_v) / p_v >= 1e-2 {
            return Err(HeatExchangerError::VolumeMismatch(err_v));
        }
        // the translated copy of the secondary mesh: `points[i] = pointsRef[i] + delta`, faces and cells unchanged
        let moved: Vec<Vector3> =
            mesh.points.iter().map(|p| Vector3::new(p.x + delta.x, p.y + delta.y, p.z + delta.z)).collect();
        let translated = OverlapMesh::new(&moved, mesh.faces.to_vec(), mesh.owner.to_vec(), mesh.neighbour.to_vec())
            .map_err(|e| HeatExchangerError::Mesh(e.to_string()))?;
        let target = OverlapMesh::new(mesh.points, mesh.faces.to_vec(), mesh.owner.to_vec(), mesh.neighbour.to_vec())
            .map_err(|e| HeatExchangerError::Mesh(e.to_string()))?;
        let mapping = CellVolumeWeightAddressing::new_normalised(&translated, &target);
        let nearest_cell_mapping = nearest_cell_addressing(&translated, &target);
        Ok(Self { primary_cells, i_a, hw: wall_conductance.max(1e-69), mapping, nearest_cell_mapping, translation: delta })
    }
}

/// The heat exchangers of one region with the two fields `structure` keeps for them: `iAHX` and `T.HX` (`THXPtr_`).
#[derive(Debug, Clone, Default)]
pub struct HeatExchangerSet {
    /// The exchangers, in dictionary order (their cell sets are disjoint in the shipped cases; the order is then immaterial).
    pub zones: Vec<HeatExchangerZone>,
    /// `iAHX`: the sum of every exchanger's `iA()` (`iA_` on its primary cells), per cell.
    pub i_a_hx: Vec<f64>,
    /// `THX` (`T.HX`): zero until the first `structure::correct`, then the surface temperature on the primary cells.
    pub t_hx: Vec<f64>,
    /// CONTROL (README round 44, C2): map with the nearest source cell instead of the overlap weights. Default `false`.
    pub nearest_cell_mapping: bool,
    /// CONTROL (README round 44, C3): the secondary `htc*T` taken with the temperature BEFORE the energy solve (as the primary
    /// side's `HT` is) instead of upstream's current `T`. Default `false`.
    pub secondary_uses_prev_iter_t: bool,
}

impl HeatExchangerSet {
    /// `structure::constructHeatExchangers`: `iAHX += iter().iA()` for every exchanger, `THX = 0`.
    #[must_use]
    pub fn new(n_cells: usize, zones: Vec<HeatExchangerZone>) -> Self {
        let mut i_a_hx = vec![0.0; n_cells];
        for z in &zones {
            for &c in &z.primary_cells {
                i_a_hx[c] += z.i_a;
            }
        }
        Self { zones, i_a_hx, t_hx: vec![0.0; n_cells], nearest_cell_mapping: false, secondary_uses_prev_iter_t: false }
    }

    /// Whether the region has heat exchangers (`THXPtr_.valid()`).
    #[must_use]
    pub fn is_present(&self) -> bool {
        !self.zones.is_empty()
    }

    /// `Foam::max(THXPtr_()).value() > 1e-69`: the guard of `linearizedSemiImplicitHeatSource` / `explicitHeatSource`
    /// (`THX` is `zeroGradient` on every patch, so the boundary values add nothing to the maximum).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_present() && self.t_hx.iter().copied().fold(f64::NEG_INFINITY, f64::max) > 1e-69
    }

    /// `heatExchanger::correct(HT, H, THX)` for every exchanger (`structure::correct`), for a secondary side in the same
    /// region: `s_h = htc`, `s_ht = htc*T` with the CURRENT temperature (`sMesh_.lookupObject("T")`, evaluated after the
    /// energy solve) while `ht` is `Hs*T.prevIter()`. `HsOp`, `HTsOp` are `mapSrcToTgt(.., plusEqOp)` into a zero field;
    /// `HTpOs`, `HpOs` (`mapTgtToSrc`) are computed upstream and never read, so they are not computed here.
    pub fn correct(&mut self, ht: &[f64], h: &[f64], s_h: &[f64], s_ht: &[f64]) {
        use uom::si::heat_flux_density::watt_per_square_meter;
        use uom::si::heat_transfer::watt_per_square_meter_kelvin as wm2k;
        let n = self.t_hx.len();
        for z in &self.zones {
            let mut hs_op = vec![0.0; n];
            let mut hts_op = vec![0.0; n];
            let mapping = if self.nearest_cell_mapping { &z.nearest_cell_mapping } else { &z.mapping };
            mapping.map_src_to_tgt_scalar(s_h, &mut hs_op);
            mapping.map_src_to_tgt_scalar(s_ht, &mut hts_op);
            for &c in &z.primary_cells {
                let t = hx_wall_temperature(
                    WallConductance::new::<wm2k>(z.hw),
                    HeatTransferCoefficient::new::<wm2k>(h[c]),
                    HeatFlux::new::<watt_per_square_meter>(ht[c]),
                    HeatTransferCoefficient::new::<wm2k>(hs_op[c]),
                    HeatFlux::new::<watt_per_square_meter>(hts_op[c]),
                );
                self.t_hx[c] = t.get::<uom::si::thermodynamic_temperature::kelvin>();
            }
        }
    }

    /// The cell source of `structure::linearizedSemiImplicitHeatSource` from the exchangers, `iAHX*H*(THX - T + he/Cp)`, and the
    /// implicit coefficient `iAHX*H/Cp` (before the cell volume), at cell `c`. Both are `(0, 0)` while the guard of
    /// [`Self::is_active`] fails. Same operation order as the active-structure term in the solvers.
    #[must_use]
    pub fn source_and_coeff(&self, c: usize, h: f64, t: f64, he: f64, cp: f64) -> (f64, f64) {
        let ih = self.i_a_hx[c] * h;
        (ih * (self.t_hx[c] - t + he / cp), ih / cp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::heat_flux_density::watt_per_square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    fn hw(x: f64) -> WallConductance {
        WallConductance::new::<watt_per_square_meter_kelvin>(x)
    }
    fn h(x: f64) -> HeatTransferCoefficient {
        HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(x)
    }
    fn ht(coeff: f64, temp: f64) -> HeatFlux {
        HeatFlux::new::<watt_per_square_meter>(coeff * temp)
    }

    /// V&V — symmetric heat exchanger sits the wall midway.
    ///
    /// Methodology: with equal convective coefficients on both sides
    /// (`h_p = h_s = h`), symmetry demands the primary wall-surface temperature
    /// be the average of the two fluid temperatures. Derivation: A_p = A_s = A,
    /// B_p = A_h*T_p_fluid where B_i = h*T_i/H_w. With h = H_w = 1.0e4,
    /// T_p_fluid = 600 K, T_s_fluid = 400 K:
    /// A = (1e4+1e4)/1e4 = 2, B_p = 1e4*600/1e4 = 600, B_s = 400.
    /// T_p = (600*2 + 400)/(2*2 - 1) = 1600/3 = 533.333 K.
    /// Cross-check T_s = (400*2 + 600)/3 = 1400/3 = 466.667 K; the wall drop
    /// T_p - T_s = 66.667 K and the mean (T_p+T_s)/2 = 500 K = mean fluid temp.
    ///
    /// Result (2026-07-15): T_p = 533.333 K (|error| < 1e-3), mean of T_p and
    /// the analytic T_s = 500.000 K. Pass.
    #[test]
    fn symmetric_hx_wall_temperature() {
        let t_p = hx_wall_temperature(
            hw(1.0e4),
            h(1.0e4),
            ht(1.0e4, 600.0),
            h(1.0e4),
            ht(1.0e4, 400.0),
        );
        assert!((t_p.get::<kelvin>() - 1600.0 / 3.0).abs() < 1e-3);

        // The symmetric secondary temperature (swap roles) confirms the mean.
        let t_s = hx_wall_temperature(
            hw(1.0e4),
            h(1.0e4),
            ht(1.0e4, 400.0),
            h(1.0e4),
            ht(1.0e4, 600.0),
        );
        let mean = 0.5 * (t_p.get::<kelvin>() + t_s.get::<kelvin>());
        assert!((mean - 500.0).abs() < 1e-3);
    }

    /// Degenerate guard — zero convective coefficients do not divide by zero.
    ///
    /// With h_p = h_s = 0, A_p = A_s = 1 so A_p*A_s - 1 = 0; upstream floors the
    /// denominator at 1e-69. B_p = B_s = 0 too, so the result is a finite 0 K
    /// (no heat exchanged), not a NaN/inf.
    #[test]
    fn zero_coefficients_do_not_nan() {
        let t_p = hx_wall_temperature(hw(1.0e4), h(0.0), ht(0.0, 0.0), h(0.0), ht(0.0, 0.0));
        assert!(t_p.get::<kelvin>().is_finite());
    }

    /// A closed pencil of `n` unit hexahedra along z (internal faces first), the lists `heatExchanger::constructSecondaryHX` reads.
    fn pencil(n: usize) -> (Vec<Vector3>, Vec<Vec<usize>>, Vec<usize>, Vec<usize>, Vec<f64>, Vec<Vector3>) {
        let corner = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let mut points = Vec::new();
        for k in 0..=n {
            for (x, y) in corner {
                points.push(Vector3::new(x, y, k as f64));
            }
        }
        let (mut faces, mut owner, mut neighbour) = (Vec::new(), Vec::new(), Vec::new());
        for k in 1..n {
            faces.push(vec![4 * k, 4 * k + 1, 4 * k + 2, 4 * k + 3]);
            owner.push(k - 1);
            neighbour.push(k);
        }
        faces.push(vec![3, 2, 1, 0]);
        owner.push(0);
        faces.push(vec![4 * n, 4 * n + 1, 4 * n + 2, 4 * n + 3]);
        owner.push(n - 1);
        for k in 0..n {
            for c in 0..4 {
                faces.push(vec![4 * k + c, 4 * k + (c + 1) % 4, 4 * (k + 1) + (c + 1) % 4, 4 * (k + 1) + c]);
                owner.push(k);
            }
        }
        let volumes = vec![1.0; n];
        let centres = (0..n).map(|k| Vector3::new(0.5, 0.5, k as f64 + 0.5)).collect();
        (points, faces, owner, neighbour, volumes, centres)
    }

    /// V&V (README round 44): a two-zone exchanger on a 4-cell pencil, primary cells 0-1 and secondary cells 2-3. Upstream's
    /// construction translates the whole secondary mesh by `pCOV - sCOV = (0, 0, -2)`, so primary cell 0 overlaps secondary cell 2
    /// and primary cell 1 overlaps secondary cell 3 with weight 1 (read from `heatExchanger.C:310-431`); the symmetric solve of
    /// [`symmetric_hx_wall_temperature`] then gives `THX = 1600/3` on the primary cells and `THX` stays 0 elsewhere.
    #[test]
    fn two_zone_exchanger_on_a_pencil_maps_and_solves_as_upstream() {
        let (points, faces, owner, neighbour, volumes, centres) = pencil(4);
        let input = HxMeshInput {
            points: &points,
            faces: &faces,
            owner: &owner,
            neighbour: &neighbour,
            cell_volumes: &volumes,
            cell_centres: &centres,
        };
        let z = HeatExchangerZone::new(&input, vec![0, 1], &[2, 3], 250.0, 1.0e4).expect("equal volumes");
        assert_eq!((z.translation.x, z.translation.y, z.translation.z), (0.0, 0.0, -2.0));
        assert_eq!(z.mapping.tgt_to_src_addr[0], vec![2]);
        assert_eq!(z.mapping.tgt_to_src_addr[1], vec![3]);
        assert_eq!(z.mapping.tgt_to_src_wght[0], vec![1.0]);
        // the control mapping is the same on a conformal pair
        assert_eq!(z.nearest_cell_mapping.tgt_to_src_addr[0], vec![2]);
        assert_eq!(z.nearest_cell_mapping.tgt_to_src_addr[1], vec![3]);

        let mut set = HeatExchangerSet::new(4, vec![z]);
        assert!(set.is_present() && !set.is_active());
        assert_eq!(set.i_a_hx, vec![250.0, 250.0, 0.0, 0.0]);
        let h = vec![1.0e4; 4];
        let ht = vec![1.0e4 * 600.0; 4];
        let s_ht = vec![1.0e4 * 600.0, 1.0e4 * 600.0, 1.0e4 * 400.0, 1.0e4 * 400.0];
        set.correct(&ht, &h, &h, &s_ht);
        assert!(set.is_active());
        assert!((set.t_hx[0] - 1600.0 / 3.0).abs() < 1e-9 && (set.t_hx[1] - 1600.0 / 3.0).abs() < 1e-9);
        assert_eq!((set.t_hx[2], set.t_hx[3]), (0.0, 0.0));
        // the control mapping gives the same answer here
        set.nearest_cell_mapping = true;
        let mut near = set.clone();
        near.correct(&ht, &h, &h, &s_ht);
        assert_eq!(near.t_hx, set.t_hx);
        // the source: iAHX*H*(THX - T + he/Cp) and iAHX*H/Cp
        let (su, sp) = set.source_and_coeff(0, 1.0e4, 600.0, 7.5e5, 1250.0);
        assert_eq!(su, 250.0 * 1.0e4 * (set.t_hx[0] - 600.0 + 7.5e5 / 1250.0));
        assert_eq!(sp, 250.0 * 1.0e4 / 1250.0);
    }

    /// Upstream's `FatalError` for zones whose volumes differ by 1 % or more, with its message.
    #[test]
    fn unequal_zone_volumes_are_upstreams_fatal_error() {
        let (points, faces, owner, neighbour, volumes, centres) = pencil(4);
        let input = HxMeshInput {
            points: &points,
            faces: &faces,
            owner: &owner,
            neighbour: &neighbour,
            cell_volumes: &volumes,
            cell_centres: &centres,
        };
        let err = HeatExchangerZone::new(&input, vec![0, 1, 2], &[3], 250.0, 1.0e4).expect_err("volumes differ");
        assert_eq!(err, HeatExchangerError::VolumeMismatch(2.0 / 3.0));
        assert!(format!("{err}").starts_with("Primary and secondary zone volumes differ by"));
        let bad = HeatExchangerError::Mesh("x".to_string());
        assert_eq!(format!("{bad}"), "heat exchanger mesh: x");
    }

    #[test]
    fn struct_wrapper_matches_free_function() {
        let hx = HeatExchanger::new(hw(1.0e4));
        let via_struct =
            hx.wall_temperature(h(1.0e4), ht(1.0e4, 600.0), h(1.0e4), ht(1.0e4, 400.0));
        let via_fn = hx_wall_temperature(
            hw(1.0e4),
            h(1.0e4),
            ht(1.0e4, 600.0),
            h(1.0e4),
            ht(1.0e4, 400.0),
        );
        assert_eq!(via_struct.get::<kelvin>(), via_fn.get::<kelvin>());
    }
}
