// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerModels/lumpedNuclearStructure/lumpedNuclearStructure.{C,H} (and powerModel.{C,H})
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `lumpedNuclearStructure` — a chain of lumped nodes from the core to the coolant (gh:#1230)
//!
//! Translation of GeN-Foam's `powerModels::lumpedNuclearStructure`: `n` temperature nodes joined by `n + 1`
//! heat conductances (per structure volume, W/K/m^3), zero gradient at the innermost node, the series of the
//! last conductance and the coolant conductance `Hcool = HSum iA / alpha` at the outermost, the power density
//! `q = q_neutronics * fractionOfPowerFromNeutronics` split by `powerFractions`, and an optional lateral
//! heat diffusion `kappaMatrix * laplacian(Tmatrix)` into the node `nodeMatrix`.
//! Fields kept per cell: the node temperatures `T`, `Tmax = T0 + q qf0 / Hs0`, `Tsurface`, `Tmatrix`.
//!
//! **The Laplacian is an input.** Upstream evaluates `laplacianTmatrix_ = fvc::laplacian(Tmatrix_)` (a mesh
//! operator over the whole mesh, with `Tmatrix` zero outside the structure) at the start of `correct`; this
//! model owns no mesh, so [`LumpedNuclearStructure::tmatrix`] exposes `Tmatrix` and `correct` takes the
//! Laplacian the caller computed from it. The differential test feeds the value the real binary used.
//!
//! One node (`nodesNumber 1`) takes upstream's scalar branch (`T = S/M`), more than one the matrix branch
//! solved with `Foam::solve`. `T0` (the initial temperature) is read from the dictionary; ~~the
//! `T.lumpedNuclearStructure` time-directory read is not ported~~ **CHANGED 2026-11-28 (gh:#1962, README round 81):** a start
//! directory's `T`/`Tmax`/`Tmatrix`/`Tsurface.lumpedNuclearStructure` are read ([`LumpedNuclearStructure::with_read_state`]).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::radial_solve::{foam_solve, OldTimeFieldField, OldTimePolicy, RadialError, SMALL};

/// One `lumpedNuclearStructure` region.
#[derive(Debug, Clone, PartialEq)]
pub struct LumpedRegion {
    /// Cells of the zone in cell-zone order.
    pub cells: Vec<usize>,
    /// `volumetricArea` [1/m].
    pub volumetric_area: f64,
    /// `fractionOfPowerFromNeutronics` (default 1).
    pub fraction_of_power: f64,
    /// `nodesNumber`.
    pub nodes_number: usize,
    /// `nodeFuel`, `nodeClad`, `nodeMatrix` (default 0).
    pub node_fuel: usize,
    /// See `node_fuel`.
    pub node_clad: usize,
    /// See `node_fuel`.
    pub node_matrix: usize,
    /// `kappaMatrix` [W/m/K] (default 0: no diffusion).
    pub kappa_matrix: f64,
    /// `heatConductances`: `nodes + 1` values.
    pub heat_conductances: Vec<f64>,
    /// `rhoCp` per node.
    pub rho_cp: Vec<f64>,
    /// `volumeFractions` per node.
    pub volume_fractions: Vec<f64>,
    /// `powerFractions` per node.
    pub power_fractions: Vec<f64>,
    /// `T0`: the initial temperature [K].
    pub t0: f64,
    /// The powerModel `volumeFraction` if given.
    pub volume_fraction: Option<f64>,
}

/// The `lumpedNuclearStructure` power model over a whole mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct LumpedNuclearStructure {
    regions: Vec<LumpedRegion>,
    cell_list: Vec<usize>,
    cell_to_region: Vec<usize>,
    t: OldTimeFieldField,
    tmax: Vec<f64>,
    tmatrix: Vec<f64>,
    tsurface: Vec<f64>,
    ia: Vec<f64>,
    alpha: Vec<f64>,
    t_fuel_av: Vec<f64>,
    t_clad_av: Vec<f64>,
}

impl LumpedNuclearStructure {
    /// `lumpedNuclearStructure::lumpedNuclearStructure`.
    ///
    /// # Errors
    /// [`RadialError::Invalid`] for list lengths that do not match `nodesNumber`.
    pub fn new(
        n_cells: usize,
        regions: Vec<LumpedRegion>,
        structure_alpha: &[f64],
        time_index: i64,
    ) -> Result<Self, RadialError> {
        let mut cell_list = Vec::new();
        let mut cell_to_region = vec![0_usize; n_cells];
        let mut alpha = vec![0.0; n_cells];
        let mut ia = vec![0.0; n_cells];
        let mut t_cur: Vec<Vec<f64>> = vec![Vec::new(); n_cells];
        for (regioni, reg) in regions.iter().enumerate() {
            let n = reg.nodes_number;
            if n == 0
                || reg.heat_conductances.len() != n + 1
                || reg.rho_cp.len() != n
                || reg.volume_fractions.len() != n
                || reg.power_fractions.len() != n
                || reg.node_fuel >= n
                || reg.node_clad >= n
                || reg.node_matrix >= n
            {
                return Err(RadialError::Invalid("lumpedNuclearStructure: list lengths must match nodesNumber"));
            }
            for &c in &reg.cells {
                cell_list.push(c);
                cell_to_region[c] = regioni;
                alpha[c] = reg.volume_fraction.unwrap_or(structure_alpha[c]);
                ia[c] = reg.volumetric_area; // setInterfacialArea
                t_cur[c] = vec![reg.t0; n];
            }
        }
        let mut me = Self {
            regions,
            cell_list,
            cell_to_region,
            t: OldTimeFieldField::new(t_cur, time_index),
            tmax: vec![0.0; n_cells],
            tmatrix: vec![0.0; n_cells],
            tsurface: vec![0.0; n_cells],
            ia,
            alpha,
            t_fuel_av: vec![0.0; n_cells],
            t_clad_av: vec![0.0; n_cells],
        };
        // set I/O fields: TFuelAv, TCladAv, Tmatrix
        for &celli in &me.cell_list.clone() {
            let reg = &me.regions[me.cell_to_region[celli]];
            let t = &me.t.current[celli];
            me.t_fuel_av[celli] = t[reg.node_fuel];
            me.t_clad_av[celli] = t[reg.node_clad];
            if reg.kappa_matrix != 0.0 {
                me.tmatrix[celli] = t[reg.node_matrix];
            }
        }
        Ok(me)
    }

    /// The constructor's `READ_IF_PRESENT` branch (gh:#1962, README round 81; `lumpedNuclearStructure.C:76-125, 163-167, 238-277`): a start
    /// directory that holds `T.lumpedNuclearStructure` (`foundT`; `None`: the `T0` of [`Self::new`]) gives the node temperatures (one list per cell, `0()` outside the model),
    /// and `Tmax`/`Tmatrix`/`Tsurface.lumpedNuclearStructure`, when present, their cell values. Then, as the constructor does after either
    /// branch, `TFuelAv`/`TCladAv` from the nodes and, where `kappaMatrix != 0`, `Tmatrix` from the node `nodeMatrix`. `Tsurface` is what
    /// `structure` hands to `Tact_` through `correctT` before the first step (`structure.C:828-840`). The old-time bookkeeping is the
    /// constructor's (no old time stored yet), as upstream, where the field is read before any `oldTime()`.
    ///
    /// # Errors
    /// [`RadialError::Invalid`] for a list per cell whose length is not the region's `nodesNumber`, or fields of the wrong size.
    pub fn with_read_state(
        mut self,
        t: Option<Vec<Vec<f64>>>,
        tmax: Option<Vec<f64>>,
        tmatrix: Option<Vec<f64>>,
        tsurface: Option<Vec<f64>>,
    ) -> Result<Self, RadialError> {
        let n = self.cell_to_region.len();
        if t.as_ref().is_some_and(|t| t.len() != n) || [&tmax, &tmatrix, &tsurface].iter().any(|f| f.as_ref().is_some_and(|v| v.len() != n)) {
            return Err(RadialError::Invalid("lumpedNuclearStructure: a read field is not one value (or list) per cell"));
        }
        if let Some(t) = t {
            for &c in &self.cell_list {
                if t[c].len() != self.regions[self.cell_to_region[c]].nodes_number {
                    return Err(RadialError::Invalid("T.lumpedNuclearStructure: a cell list is not nodesNumber long"));
                }
            }
            self.t.current = t;
        }
        if let Some(v) = tmax {
            self.tmax = v;
        }
        if let Some(v) = tmatrix {
            self.tmatrix = v;
        }
        if let Some(v) = tsurface {
            self.tsurface = v;
        }
        for &celli in &self.cell_list.clone() {
            let reg = &self.regions[self.cell_to_region[celli]];
            let tc = &self.t.current[celli];
            self.t_fuel_av[celli] = tc[reg.node_fuel];
            self.t_clad_av[celli] = tc[reg.node_clad];
            if reg.kappa_matrix != 0.0 {
                self.tmatrix[celli] = tc[reg.node_matrix];
            }
        }
        Ok(self)
    }

    /// `Tsurface` of every cell (0 outside the model).
    #[must_use]
    pub fn tsurface(&self) -> &[f64] {
        &self.tsurface
    }

    /// Select the old-time policy of `T`.
    #[must_use]
    pub fn with_old_time_policy(mut self, policy: OldTimePolicy) -> Self {
        self.t = self.t.clone().with_policy(policy);
        self
    }

    /// `updateLocalTemperatureProfile` for one cell.
    #[allow(clippy::too_many_arguments)]
    fn update_local(
        &mut self,
        celli: usize,
        q_ref: f64,
        laplacian: f64,
        ht_sumi: f64,
        h_sumi: f64,
        dt: f64,
        time_index: i64,
    ) -> Result<(), RadialError> {
        let regioni = self.cell_to_region[celli];
        let reg = &self.regions[regioni];
        let nodes = reg.nodes_number;
        let hs = &reg.heat_conductances;
        let rho_cp = &reg.rho_cp;
        let vf = &reg.volume_fractions;
        let qf = &reg.power_fractions;
        let (node_matrix, kappa) = (reg.node_matrix, reg.kappa_matrix);
        let t_old = self.t.old_time_of(celli, time_index);
        let ia = self.ia[celli];
        // update power density
        let q = q_ref * reg.fraction_of_power;
        // recurrent quantities
        let t_cool = ht_sumi / h_sumi.max(SMALL);
        let h_cool = h_sumi * ia / self.alpha[celli];
        let diffusion_at = |i: usize| -> f64 {
            if node_matrix == i && kappa != 0.0 {
                kappa * laplacian
            } else {
                0.0
            }
        };
        let t_new: Vec<f64>;
        if nodes > 1 {
            let n = nodes;
            let mut m = vec![0.0_f64; n * n];
            let mut s = vec![0.0_f64; n];
            {
                // zeroGradient BC at the innermost node
                let diffusion = diffusion_at(0);
                m[1] = -hs[1];
                m[0] = vf[0] * rho_cp[0] / dt + hs[1];
                s[0] = diffusion + q * qf[0] + t_old[0] * vf[0] * rho_cp[0] / dt;
            }
            if n > 2 {
                for i in 1..(n - 1) {
                    let diffusion = diffusion_at(i);
                    m[i * n + i + 1] = -hs[i + 1];
                    m[i * n + i - 1] = -hs[i];
                    m[i * n + i] = vf[i] * rho_cp[i] / dt + hs[i + 1] + hs[i];
                    s[i] = diffusion + q * qf[i] + t_old[i] * vf[i] * rho_cp[i] / dt;
                }
            }
            {
                // outer surface, convective BC with the fluid(s)
                let i = n - 1;
                let diffusion = diffusion_at(i);
                let h_to_cool = hs[i + 1] * h_cool / (hs[i + 1] + h_cool);
                m[i * n + i - 1] = -hs[i];
                m[i * n + i] = vf[i] * rho_cp[i] / dt + hs[i] + h_to_cool;
                s[i] = diffusion + q * qf[i] + t_old[i] * vf[i] * rho_cp[i] / dt + h_to_cool * t_cool;
            }
            t_new = foam_solve(&m, n, &s)?;
        } else {
            let diffusion = diffusion_at(0);
            let h_to_cool = hs[1] * h_cool / (hs[1] + h_cool);
            let m = vf[0] * rho_cp[0] / dt + h_to_cool;
            let s = diffusion + q * qf[0] + t_old[0] * vf[0] * rho_cp[0] / dt + h_to_cool * t_cool;
            t_new = vec![s / m];
        }
        // set fields (max and outer)
        self.tmax[celli] = t_new[0] + q * qf[0] / hs[0];
        self.tsurface[celli] = (hs[nodes] * t_new[nodes - 1] + h_cool * t_cool) / (hs[nodes] + h_cool);
        // update average fuel and clad temperature used for coupling
        self.t_fuel_av[celli] = t_new[reg.node_fuel];
        self.t_clad_av[celli] = t_new[reg.node_clad];
        // update matrix temperature
        if kappa != 0.0 {
            self.tmatrix[celli] = t_new[node_matrix];
        }
        self.t.current[celli] = t_new;
        Ok(())
    }

    /// `lumpedNuclearStructure::correct(HT, H)`. `laplacian_t_matrix` is `fvc::laplacian(Tmatrix)` evaluated by
    /// the caller on [`Self::tmatrix`] as it stands before this call; `q_neutronics` is the structure's
    /// `powerDensityNeutronics`.
    ///
    /// # Errors
    /// A singular nodal matrix.
    pub fn correct(
        &mut self,
        ht_sum: &[f64],
        h_sum: &[f64],
        q_neutronics: &[f64],
        laplacian_t_matrix: &[f64],
        dt: f64,
        time_index: i64,
    ) -> Result<(), RadialError> {
        for i in 0..self.cell_list.len() {
            let celli = self.cell_list[i];
            self.update_local(celli, q_neutronics[celli], laplacian_t_matrix[celli], ht_sum[celli], h_sum[celli], dt, time_index)?;
        }
        Ok(())
    }

    /// `correctT`: `Tsurface` of a cell of the model.
    #[must_use]
    pub fn surface_temperature(&self, cell: usize) -> f64 {
        self.tsurface[cell]
    }

    /// The model's cells in `cellList_` order.
    #[must_use]
    pub fn cells(&self) -> &[usize] {
        &self.cell_list
    }

    /// The node temperatures `T.lumpedNuclearStructure` of a cell.
    #[must_use]
    pub fn nodes(&self, cell: usize) -> &[f64] {
        &self.t.current[cell]
    }

    /// `Tmax` of a cell.
    #[must_use]
    pub fn tmax(&self, cell: usize) -> f64 {
        self.tmax[cell]
    }

    /// The `Tmatrix` field (zero where the model has no matrix node).
    #[must_use]
    pub fn tmatrix(&self) -> &[f64] {
        &self.tmatrix
    }

    /// `(TFuelAv, TCladAv)` of a cell (the structure's coupling temperatures).
    #[must_use]
    pub fn coupling_temperatures(&self, cell: usize) -> (f64, f64) {
        (self.t_fuel_av[cell], self.t_clad_av[cell])
    }

    /// `volumetricArea.powerModel` of a cell.
    #[must_use]
    pub fn interfacial_area(&self, cell: usize) -> f64 {
        self.ia[cell]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(nodes: usize, kappa: f64) -> LumpedRegion {
        LumpedRegion {
            cells: vec![0],
            volumetric_area: 40.0,
            fraction_of_power: 1.0,
            nodes_number: nodes,
            node_fuel: 0,
            node_clad: nodes - 1,
            node_matrix: nodes - 1,
            kappa_matrix: kappa,
            heat_conductances: vec![6.0e5; nodes + 1],
            rho_cp: vec![5.0e4; nodes],
            volume_fractions: vec![1.0 / (nodes as f64); nodes],
            power_fractions: { let mut v = vec![0.0; nodes]; v[0] = 1.0; v },
            t0: 800.0,
            volume_fraction: None,
        }
    }

    /// Steady-state energy balance of the chain (`dt` huge, no diffusion): all the power leaves through
    /// the coolant, `q = Hcool'(T_out - Tcool)` with the series conductance, for 1, 2 and 3 nodes.
    #[test]
    fn steady_chain_conserves_energy() {
        for nodes in [1_usize, 2, 3] {
            let mut p = LumpedNuclearStructure::new(1, vec![reg(nodes, 0.0)], &[0.5], 0).unwrap();
            let (q, h, tf) = (5.0e6, 1.0e4, 700.0);
            for s in 1..=3 {
                p.correct(&[h * tf], &[h], &[q], &[0.0], 1.0e12, s).unwrap();
            }
            let h_cool = h * 40.0 / 0.5;
            let hs_last = 6.0e5;
            let h_series = hs_last * h_cool / (hs_last + h_cool);
            let t_out = p.nodes(0)[nodes - 1];
            let flux = h_series * (t_out - tf);
            assert!(((flux - q) / q).abs() < 1e-9, "nodes={nodes}: {flux} vs {q}");
            // Tsurface is the interface temperature: its drop against the coolant carries the same flux
            let t_s = p.surface_temperature(0);
            assert!(((h_cool * (t_s - tf) - q) / q).abs() < 1e-9, "Tsurface {t_s}");
            assert!(p.tmax(0) >= p.nodes(0)[0]);
            let (f, c) = p.coupling_temperatures(0);
            assert_eq!((f, c), (p.nodes(0)[0], p.nodes(0)[nodes - 1]));
            assert_eq!(p.interfacial_area(0), 40.0);
            assert_eq!(p.cells(), &[0]);
        }
    }

    /// The diffusion input enters the matrix node and `Tmatrix` follows it; bad lists are refused.
    #[test]
    fn diffusion_and_validation() {
        let mut p = LumpedNuclearStructure::new(1, vec![reg(3, 12.0)], &[0.5], 0).unwrap();
        assert_eq!(p.tmatrix()[0], 800.0);
        let mut q = p.clone();
        p.correct(&[7.0e6], &[1.0e4], &[5.0e6], &[0.0], 0.5, 1).unwrap();
        q.correct(&[7.0e6], &[1.0e4], &[5.0e6], &[1.0e3], 0.5, 1).unwrap();
        assert!(q.nodes(0)[2] > p.nodes(0)[2], "positive Laplacian heats the matrix node");
        assert_eq!(p.tmatrix()[0], p.nodes(0)[2]);
        let mut bad = reg(3, 0.0);
        bad.rho_cp.pop();
        assert!(LumpedNuclearStructure::new(1, vec![bad], &[0.5], 0).is_err());
        let _ = p.with_old_time_policy(OldTimePolicy::StartOfStep);
    }
}
