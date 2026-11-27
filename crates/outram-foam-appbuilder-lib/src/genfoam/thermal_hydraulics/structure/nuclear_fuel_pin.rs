// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerModels/nuclearFuelPin/nuclearFuelPin.{C,H} (and powerModel.{C,H})
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman, Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `nuclearFuelPin` — fuel pellet, gap and cladding with radial conduction (gh:#1230)
//!
//! Translation of GeN-Foam's `powerModels::nuclearFuelPin`: per mesh cell one radial finite-volume problem
//! over `fuelMeshSize + cladMeshSize` nodes, power only in the fuel (`q = powerDensityNeutronics *
//! fractionOfPowerFromNeutronics`), a gap conductance `gapH` (constant, or the `gapHPowerDensity` table
//! evaluated at `q`) between the last fuel node and the first cladding node (the cladding side scaled by
//! `r_fo/r_ci` to conserve the heat flow), and the convective coupling `HSum`, `HTSum` at the outer cladding.
//! `Foam::solve` ([`super::radial_solve::foam_solve`]) solves each system.
//!
//! ## Upstream defects reproduced on purpose ("upstream is the specification")
//! `nuclearFuelPin.C` builds the ring areas `dA` with two slips, both visible in the source and both
//! reproduced here (reported to the maintainer, not fixed; the control
//! [`NuclearFuelPin::with_corrected_ring_areas`] shows what they cost):
//! 1. the innermost cladding node uses the FUEL step: `pi((r[nf] + drf/2)^2 - r[nf]^2)` (should be `drc`);
//! 2. the cladding bulk loop `for(i = 1; i < cladMeshSize-1; ++i)` evaluates `r[i]`, a FUEL node radius, with
//!    `drc`, and not `r[nf + i]`.
//! `dA` multiplies only the storage term `rhoCp/dt` in the cladding (no power there), so the slips change the
//! cladding's thermal inertia in a transient and nothing in steady state.
//!
//! ## Not ported (refused by name)
//! * The reconstruction of the initial profile from `Tfi/Tfo/Tci/Tco.nuclearFuelPin` files and the read of
//!   `Trad.nuclearFuelPin` from a time directory (`fuelT`/`cladT` of the dictionary are used).
//! * `nuclearFuelPinTest` (gh:#1787) is a different class.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use uom::si::f64::Time;
use uom::si::time::second;

use super::radial_solve::{
    foam_solve, lu_control_solve, sqr, update_average_and_extrema, OldTimeFieldField, OldTimePolicy, RadialError,
    PI,
};
use crate::genfoam::common::TimeProfile;

/// The gap conductance of a region: `gapH`, or the `gapHPowerDensity` table of the power density.
#[derive(Debug, Clone, PartialEq)]
pub enum GapConductance {
    /// `gapH` [W/m^2/K].
    Constant(f64),
    /// `gapHPowerDensity table ((q h) ...)`: OpenFOAM-ESI `Function1` table, linear interpolation, `clamp`.
    /// Evaluated with the same code as `TimeProfile::table` (the ordinate is the power density in W/m^3,
    /// carried through a `Time` in seconds with a zero start time, which is the identity).
    TableOfPowerDensity(TimeProfile),
}

impl GapConductance {
    /// `gapH` for the power density `q` [W/m^3].
    #[must_use]
    pub fn value(&self, q: f64) -> f64 {
        match self {
            Self::Constant(h) => *h,
            Self::TableOfPowerDensity(p) => p.value(Time::new::<second>(q)),
        }
    }
}

/// One `nuclearFuelPin` region.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclearFuelPinRegion {
    /// Cells of the zone in cell-zone order.
    pub cells: Vec<usize>,
    /// `fractionOfPowerFromNeutronics` (default 1).
    pub fraction_of_power: f64,
    /// `fuelInnerRadius`, `fuelOuterRadius`, `cladInnerRadius`, `cladOuterRadius` [m].
    pub rfi: f64,
    /// See `rfi`.
    pub rfo: f64,
    /// See `rfi`.
    pub rci: f64,
    /// See `rfi`.
    pub rco: f64,
    /// `fuelMeshSize`, at least 2.
    pub fuel_mesh_size: usize,
    /// `cladMeshSize`, at least 2.
    pub clad_mesh_size: usize,
    /// `fuelRho*fuelCp` or `fuelRhoCp` [J/m^3/K].
    pub rho_cp_fuel: f64,
    /// `cladRho*cladCp` or `cladRhoCp` [J/m^3/K].
    pub rho_cp_clad: f64,
    /// `fuelK`, `cladK` [W/m/K].
    pub k_fuel: f64,
    /// See `k_fuel`.
    pub k_clad: f64,
    /// `fuelT`, `cladT` [K] (initial).
    pub fuel_t0: f64,
    /// See `fuel_t0`.
    pub clad_t0: f64,
    /// The gap conductance.
    pub gap: GapConductance,
    /// The powerModel `volumeFraction` if given.
    pub volume_fraction: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct Derived {
    r: Vec<f64>,
    d_a: Vec<f64>,
    drf: f64,
    drc: f64,
}

/// The `nuclearFuelPin` power model over a whole mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclearFuelPin {
    regions: Vec<NuclearFuelPinRegion>,
    derived: Vec<Derived>,
    cell_list: Vec<usize>,
    cell_to_region: Vec<usize>,
    trad: OldTimeFieldField,
    tfi: Vec<f64>,
    tfo: Vec<f64>,
    tci: Vec<f64>,
    tco: Vec<f64>,
    tfav: Vec<f64>,
    tcav: Vec<f64>,
    ia: Vec<f64>,
    alpha: Vec<f64>,
    tfmax: f64,
    tfmin: f64,
    tcmax: f64,
    tcmin: f64,
    tfavav: f64,
    tcavav: f64,
    use_lu_control: bool,
}

fn ring_areas(reg: &NuclearFuelPinRegion, r: &[f64], drf: f64, drc: f64, corrected: bool) -> Vec<f64> {
    let nf = reg.fuel_mesh_size;
    let nc = reg.clad_mesh_size;
    let n = nf + nc;
    let mut d_a = vec![PI * (sqr(r[0] + drf / 2.0) - sqr(r[0]))];
    for i in 1..(nf - 1) {
        d_a.push(PI * (sqr(r[i] + drf / 2.0) - sqr(r[i] - drf / 2.0)));
    }
    d_a.push(PI * (sqr(r[nf - 1]) - sqr(r[nf - 1] - drf / 2.0)));
    // upstream: the cladding inner node uses drf (slip 1); the control uses drc
    let d_in = if corrected { drc } else { drf };
    d_a.push(PI * (sqr(r[nf] + d_in / 2.0) - sqr(r[nf])));
    for i in 1..(nc - 1) {
        // upstream: r[i], a fuel radius (slip 2); the control uses the cladding node r[nf + i]
        let ri = if corrected { r[nf + i] } else { r[i] };
        d_a.push(PI * (sqr(ri + drc / 2.0) - sqr(ri - drc / 2.0)));
    }
    d_a.push(PI * (sqr(r[n - 1]) - sqr(r[n - 1] - drc / 2.0)));
    d_a
}

impl NuclearFuelPin {
    /// `nuclearFuelPin::nuclearFuelPin` (see [`super::heated_pin::HeatedPin::new`] for the arguments).
    ///
    /// # Errors
    /// [`RadialError::Invalid`] for a mesh size below 2.
    pub fn new(
        n_cells: usize,
        regions: Vec<NuclearFuelPinRegion>,
        structure_alpha: &[f64],
        volumes: &[f64],
        time_index: i64,
    ) -> Result<Self, RadialError> {
        Self::build(n_cells, regions, structure_alpha, volumes, time_index, false)
    }

    /// CONTROL ONLY: the ring areas a careful author would have written (see the module docs).
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn with_corrected_ring_areas(
        n_cells: usize,
        regions: Vec<NuclearFuelPinRegion>,
        structure_alpha: &[f64],
        volumes: &[f64],
        time_index: i64,
    ) -> Result<Self, RadialError> {
        Self::build(n_cells, regions, structure_alpha, volumes, time_index, true)
    }

    fn build(
        n_cells: usize,
        regions: Vec<NuclearFuelPinRegion>,
        structure_alpha: &[f64],
        volumes: &[f64],
        time_index: i64,
        corrected: bool,
    ) -> Result<Self, RadialError> {
        let mut cell_list = Vec::new();
        let mut cell_to_region = vec![0_usize; n_cells];
        let mut alpha = vec![0.0; n_cells];
        let mut trad_cur: Vec<Vec<f64>> = vec![Vec::new(); n_cells];
        let mut derived = Vec::new();
        for (regioni, reg) in regions.iter().enumerate() {
            if reg.fuel_mesh_size < 2 || reg.clad_mesh_size < 2 {
                return Err(RadialError::Invalid("nuclearFuelPin: fuelMeshSize and cladMeshSize must be at least 2"));
            }
            let nf = reg.fuel_mesh_size;
            let nc = reg.clad_mesh_size;
            let drf = (reg.rfo - reg.rfi) / ((nf as f64) - 1.0);
            let drc = (reg.rco - reg.rci) / ((nc as f64) - 1.0);
            let drg = reg.rci - reg.rfo;
            let mut r = vec![reg.rfi];
            for _ in 0..(nf - 1) {
                let l = r[r.len() - 1];
                r.push(l + drf);
            }
            let l = r[r.len() - 1];
            r.push(l + drg);
            for _ in 0..(nc - 1) {
                let l = r[r.len() - 1];
                r.push(l + drc);
            }
            let d_a = ring_areas(reg, &r, drf, drc, corrected);
            for &c in &reg.cells {
                cell_list.push(c);
                cell_to_region[c] = regioni;
                alpha[c] = reg.volume_fraction.unwrap_or(structure_alpha[c]);
                let mut t = vec![reg.fuel_t0; nf];
                t.extend(vec![reg.clad_t0; nc]);
                trad_cur[c] = t;
            }
            derived.push(Derived { r, d_a, drf, drc });
        }
        let mut me = Self {
            regions,
            derived,
            cell_list,
            cell_to_region,
            trad: OldTimeFieldField::new(trad_cur, time_index),
            tfi: vec![0.0; n_cells],
            tfo: vec![0.0; n_cells],
            tci: vec![0.0; n_cells],
            tco: vec![0.0; n_cells],
            tfav: vec![0.0; n_cells],
            tcav: vec![0.0; n_cells],
            ia: vec![0.0; n_cells],
            alpha,
            tfmax: 0.0,
            tfmin: 1e69,
            tcmax: 0.0,
            tcmin: 1e69,
            tfavav: 0.0,
            tcavav: 0.0,
            use_lu_control: false,
        };
        me.refresh_io_fields(volumes);
        me.set_interfacial_area();
        Ok(me)
    }

    /// The model constructed with `Trad.nuclearFuelPin` READ from the start directory (`nuclearFuelPin.C:204-215, 538-540`: `foundTrad`,
    /// "Setting nuclearFuelPin initial temperatures from Trad"): the radial profile of every cell of the model replaces the dictionary's
    /// `fuelT`/`cladT` one, and the I/O fields and the extrema are set from it as the constructor does (gh:#1926, README round 66).
    ///
    /// # Errors
    /// [`RadialError::Invalid`] if the list has not one entry per mesh cell or an entry of a model cell is not `fuelMeshSize + cladMeshSize` long.
    pub fn with_read_trad(mut self, trad: Vec<Vec<f64>>, volumes: &[f64]) -> Result<Self, RadialError> {
        if trad.len() != self.trad.current.len() {
            return Err(RadialError::Invalid("Trad.nuclearFuelPin: not one list per cell"));
        }
        for &c in &self.cell_list {
            let reg = &self.regions[self.cell_to_region[c]];
            if trad[c].len() != reg.fuel_mesh_size + reg.clad_mesh_size {
                return Err(RadialError::Invalid("Trad.nuclearFuelPin: a cell list is not fuelMeshSize + cladMeshSize long"));
            }
        }
        for &c in &self.cell_list {
            self.trad.current[c] = trad[c].clone();
        }
        self.refresh_io_fields(volumes);
        self.set_interfacial_area();
        Ok(self)
    }

    /// `nuclearFuelPin.C:540-615`: from `Trad_` set `Tfi`, `Tfo`, `Tci`, `Tco`, `Tfav`, `Tcav` and the initial scalar extrema and volume averages.
    fn refresh_io_fields(&mut self, volumes: &[f64]) {
        let me = self;
        me.tfmax = 0.0;
        me.tfmin = 1e69;
        me.tcmax = 0.0;
        me.tcmin = 1e69;
        let mut tfavav = 0.0;
        let mut tcavav = 1e69;
        let mut tot_v = 0.0;
        for &celli in &me.cell_list.clone() {
            let regioni = me.cell_to_region[celli];
            let nf = me.regions[regioni].fuel_mesh_size;
            let n = nf + me.regions[regioni].clad_mesh_size;
            let trad = me.trad.current[celli].clone();
            me.tfi[celli] = trad[0];
            me.tfo[celli] = trad[nf - 1];
            me.tci[celli] = trad[nf];
            me.tco[celli] = trad[n - 1];
            let (drf, drc) = (me.derived[regioni].drf, me.derived[regioni].drc);
            let (mut lo, mut hi) = (me.tfmin, me.tfmax);
            let tf = update_average_and_extrema(0, nf, &me.derived[regioni].r, drf, &trad, &mut lo, &mut hi);
            me.tfmin = lo;
            me.tfmax = hi;
            let (mut lo, mut hi) = (me.tcmin, me.tcmax);
            let tc = update_average_and_extrema(nf, n, &me.derived[regioni].r, drc, &trad, &mut lo, &mut hi);
            me.tcmin = lo;
            me.tcmax = hi;
            me.tfav[celli] = tf;
            me.tcav[celli] = tc;
            let dv = volumes[celli];
            tot_v += dv;
            tfavav += tf * dv;
            tcavav += tc * dv;
        }
        tfavav /= tot_v;
        tcavav /= tot_v;
        me.tfavav = tfavav;
        me.tcavav = tcavav;
    }

    /// CONTROL ONLY: solve with `lu_decompose` instead of `Foam::solve`.
    #[must_use]
    pub fn with_lu_control_solver(mut self) -> Self {
        self.use_lu_control = true;
        self
    }

    /// Select the old-time policy of `Trad`.
    #[must_use]
    pub fn with_old_time_policy(mut self, policy: OldTimePolicy) -> Self {
        self.trad = self.trad.clone().with_policy(policy);
        self
    }

    /// `nuclearFuelPin::setInterfacialArea`: `iA = 2 alpha / r_co`.
    pub fn set_interfacial_area(&mut self) {
        for &celli in &self.cell_list {
            self.ia[celli] = 2.0 * self.alpha[celli] / self.regions[self.cell_to_region[celli]].rco;
        }
    }

    fn update_local_temperature_profile(
        &mut self,
        celli: usize,
        q_neutronics: f64,
        ht_sumi: f64,
        h_sumi: f64,
        dt: f64,
        time_index: i64,
    ) -> Result<(), RadialError> {
        let regioni = self.cell_to_region[celli];
        let reg = &self.regions[regioni];
        let der = &self.derived[regioni];
        let nf = reg.fuel_mesh_size;
        let n = nf + reg.clad_mesh_size;
        let (drf, drc) = (der.drf, der.drc);
        let (kf, kc) = (reg.k_fuel, reg.k_clad);
        let (rfo, rci) = (reg.rfo, reg.rci);
        let t_old = self.trad.old_time_of(celli, time_index);
        // update power density
        let q = q_neutronics * reg.fraction_of_power;
        let gap_h = reg.gap.value(q);
        let mut m = vec![0.0_f64; n * n];
        let mut s = vec![0.0_f64; n];
        // recurrent quantities
        let xf = reg.rho_cp_fuel / dt;
        let xc = reg.rho_cp_clad / dt;
        let two_pk_by_drf = 2.0 * PI * kf / drf;
        let two_pk_by_drc = 2.0 * PI * kc / drc;
        let drhf = drf / 2.0;
        let drhc = drc / 2.0;
        {
            // zeroGradient BC at the fuel inner surface
            let r = der.r[0];
            let d_a = der.d_a[0];
            let b = two_pk_by_drf * (r + drhf);
            let xd_a = xf * d_a;
            m[1] = -b;
            m[0] = b + xd_a;
            s[0] = q * d_a + t_old[0] * xd_a;
        }
        for i in 1..(nf - 1) {
            let r = der.r[i];
            let d_a = der.d_a[i];
            let b = two_pk_by_drf * (r + drhf);
            let c = two_pk_by_drf * (r - drhf);
            let xd_a = xf * d_a;
            m[i * n + i + 1] = -b;
            m[i * n + i - 1] = -c;
            m[i * n + i] = b + c + xd_a;
            s[i] = q * d_a + t_old[i] * xd_a;
        }
        {
            // fuel outer surface, convective BC with the inner cladding surface via the gap conductance
            let i = nf - 1;
            let r = der.r[i];
            let d_a = der.d_a[i];
            let c = two_pk_by_drf * (r - drhf);
            let d = 2.0 * PI * r * gap_h;
            let xd_a = xf * d_a;
            m[i * n + i + 1] = -d;
            m[i * n + i - 1] = -c;
            m[i * n + i] = c + d + xd_a;
            s[i] = q * d_a + t_old[i] * xd_a;
        }
        {
            // cladding inner surface: gap conductance adjusted by the radii ratio
            let i = nf;
            let r = der.r[i];
            let d_a = der.d_a[i];
            let c = two_pk_by_drc * (r + drhc);
            let d = 2.0 * PI * r * (rfo / rci) * gap_h;
            let xd_a = xc * d_a;
            m[i * n + i + 1] = -c;
            m[i * n + i - 1] = -d;
            m[i * n + i] = c + d + xd_a;
            s[i] = t_old[i] * xd_a;
        }
        for i in (nf + 1)..(n - 1) {
            let r = der.r[i];
            let d_a = der.d_a[i];
            let b = two_pk_by_drc * (r + drhc);
            let c = two_pk_by_drc * (r - drhc);
            let xd_a = xc * d_a;
            m[i * n + i + 1] = -b;
            m[i * n + i - 1] = -c;
            m[i * n + i] = b + c + xd_a;
            s[i] = t_old[i] * xd_a;
        }
        {
            // cladding outer surface, convective BC with the fluid(s) wetting the pin
            let i = n - 1;
            let r = der.r[i];
            let d_a = der.d_a[i];
            let c = two_pk_by_drc * (r - drhc);
            let d = 2.0 * PI * r;
            let xd_a = xc * d_a;
            m[i * n + i - 1] = -c;
            m[i * n + i] = c + d * h_sumi + xd_a;
            s[i] = t_old[i] * xd_a + d * ht_sumi;
        }
        let trad = if self.use_lu_control { lu_control_solve(&m, n, &s)? } else { foam_solve(&m, n, &s)? };
        self.tfi[celli] = trad[0];
        self.tfo[celli] = trad[nf - 1];
        self.tci[celli] = trad[nf];
        self.tco[celli] = trad[n - 1];
        let (mut lo, mut hi) = (self.tfmin, self.tfmax);
        let tf = update_average_and_extrema(0, nf, &self.derived[regioni].r, drf, &trad, &mut lo, &mut hi);
        self.tfmin = lo;
        self.tfmax = hi;
        let (mut lo, mut hi) = (self.tcmin, self.tcmax);
        let tc = update_average_and_extrema(nf, n, &self.derived[regioni].r, drc, &trad, &mut lo, &mut hi);
        self.tcmin = lo;
        self.tcmax = hi;
        self.tfav[celli] = tf;
        self.tcav[celli] = tc;
        self.trad.current[celli] = trad;
        Ok(())
    }

    /// `nuclearFuelPin::correct(HT, H)`. `q_neutronics` is `structure_.powerDensityNeutronics()` per cell.
    ///
    /// # Errors
    /// A singular radial matrix.
    pub fn correct(
        &mut self,
        ht_sum: &[f64],
        h_sum: &[f64],
        q_neutronics: &[f64],
        volumes: &[f64],
        dt: f64,
        time_index: i64,
    ) -> Result<(), RadialError> {
        self.tfmax = 0.0;
        self.tfmin = 1e69;
        self.tcmax = 0.0;
        self.tcmin = 1e69;
        let mut tot_v = 0.0;
        let mut tfavav = 0.0;
        let mut tcavav = 0.0;
        for i in 0..self.cell_list.len() {
            let celli = self.cell_list[i];
            self.update_local_temperature_profile(celli, q_neutronics[celli], ht_sum[celli], h_sum[celli], dt, time_index)?;
            let dv = volumes[celli];
            tot_v += dv;
            tfavav += self.tfav[celli] * dv;
            tcavav += self.tcav[celli] * dv;
        }
        tfavav /= tot_v;
        tcavav /= tot_v;
        self.tfavav = tfavav;
        self.tcavav = tcavav;
        Ok(())
    }

    /// `correctT`: the outer cladding temperature `Tco`.
    #[must_use]
    pub fn surface_temperature(&self, cell: usize) -> f64 {
        self.tco[cell]
    }

    /// The model's cells in `cellList_` order.
    #[must_use]
    pub fn cells(&self) -> &[usize] {
        &self.cell_list
    }

    /// `Trad.nuclearFuelPin` of a cell (fuel nodes then cladding nodes).
    #[must_use]
    pub fn trad(&self, cell: usize) -> &[f64] {
        &self.trad.current[cell]
    }

    /// `(Tfi, Tfo, Tci, Tco)` of a cell.
    #[must_use]
    pub fn boundary_temperatures(&self, cell: usize) -> (f64, f64, f64, f64) {
        (self.tfi[cell], self.tfo[cell], self.tci[cell], self.tco[cell])
    }

    /// `(Tfav, Tcav)`: the local fuel and cladding averages (`TFuelAv`, `TCladAv` of the structure).
    #[must_use]
    pub fn averages(&self, cell: usize) -> (f64, f64) {
        (self.tfav[cell], self.tcav[cell])
    }

    /// `volumetricArea.powerModel` of a cell.
    #[must_use]
    pub fn interfacial_area(&self, cell: usize) -> f64 {
        self.ia[cell]
    }

    /// Dictionary entries `(Tfavav, Tcavav, Tfmax, Tfmin, Tcmax, Tcmin)`.
    #[must_use]
    pub fn global_values(&self) -> (f64, f64, f64, f64, f64, f64) {
        (self.tfavav, self.tcavav, self.tfmax, self.tfmin, self.tcmax, self.tcmin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(corrected_gap: Option<GapConductance>) -> NuclearFuelPinRegion {
        NuclearFuelPinRegion {
            cells: vec![0],
            fraction_of_power: 1.0,
            rfi: 0.0,
            rfo: 0.00247,
            rci: 0.00254,
            rco: 0.002921,
            fuel_mesh_size: 10,
            clad_mesh_size: 5,
            rho_cp_fuel: 10480.0 * 250.0,
            rho_cp_clad: 7870.0 * 500.0,
            k_fuel: 3.0,
            k_clad: 20.0,
            fuel_t0: 574.56,
            clad_t0: 574.56,
            gap: corrected_gap.unwrap_or(GapConductance::Constant(3000.0)),
            volume_fraction: Some(0.55),
        }
    }

    /// Steady state with `dt` huge: the linear power balance `2 pi r_co (H Tco - HT) = q pi (r_fo^2 - r_fi^2)`
    /// holds to round-off; the ring-area slips change nothing in steady state (the P3 prediction); in a
    /// transient they do.
    #[test]
    fn steady_balance_and_ring_area_control() {
        let (q, h, tf) = (3.0e8, 3.0e4, 600.0);
        let run = |corrected: bool, dt: f64, steps: i64| {
            let mut p = if corrected {
                NuclearFuelPin::with_corrected_ring_areas(1, vec![region(None)], &[0.5], &[1.0], 0).unwrap()
            } else {
                NuclearFuelPin::new(1, vec![region(None)], &[0.5], &[1.0], 0).unwrap()
            };
            for s in 1..=steps {
                p.correct(&[h * tf], &[h], &[q], &[1.0], dt, s).unwrap();
            }
            p
        };
        let a = run(false, 1e12, 3);
        let tco = a.surface_temperature(0);
        let lin = 2.0 * PI * 0.002921 * (h * tco - h * tf);
        let exact = q * PI * (0.00247_f64 * 0.00247);
        assert!(((lin - exact) / exact).abs() < 1e-11, "linear power {lin} vs {exact}");
        let b = run(true, 1e12, 3);
        let ds = a.trad(0).iter().zip(b.trad(0)).map(|(x, y)| (x - y).abs() / x).fold(0.0, f64::max);
        assert!(ds < 1e-11, "steady state is insensitive to the slips: {ds}");
        let (a1, b1) = (run(false, 0.01, 3), run(true, 0.01, 3));
        let dt_rel = a1.trad(0).iter().zip(b1.trad(0)).map(|(x, y)| (x - y).abs() / x).fold(0.0, f64::max);
        println!("nuclearFuelPin transient: upstream vs corrected ring areas, worst relative {dt_rel:.3e}");
        assert!(dt_rel > 1e-6, "the slips matter in a transient: {dt_rel}");
        assert_eq!(a.cells(), &[0]);
        assert!(a.interfacial_area(0) > 0.0);
        let (tfi, tfo, tci, tco2) = a.boundary_temperatures(0);
        assert!(tfi > tfo && tfo > tci && tci > tco2 - 1e-9);
        let (f, c) = a.averages(0);
        assert!(f > c);
        let g = a.global_values();
        assert!(g.2 >= g.3 && g.4 >= g.5);
    }

    /// The `gapHPowerDensity` table and the constant agree where the table is flat.
    #[test]
    fn gap_conductance_table_interpolates() {
        let tab = TimeProfile::table(
            vec![Time::new::<second>(0.0), Time::new::<second>(8.0e8)],
            vec![500.0, 3000.0],
        )
        .unwrap();
        let g = GapConductance::TableOfPowerDensity(tab);
        assert_eq!(g.value(-1.0), 500.0);
        assert_eq!(g.value(9.0e8), 3000.0);
        assert!((g.value(4.0e8) - 1750.0).abs() < 1e-9);
        assert_eq!(GapConductance::Constant(7.0).value(1e9), 7.0);
        let mut p = NuclearFuelPin::new(1, vec![region(Some(g))], &[0.5], &[1.0], 0).unwrap().with_lu_control_solver();
        p.correct(&[1.8e4], &[3.0e4], &[3.0e8], &[1.0], 0.5, 1).unwrap();
        let _ = p.with_old_time_policy(OldTimePolicy::StartOfStep);
    }

    #[test]
    fn mesh_size_one_is_refused() {
        let mut r = region(None);
        r.clad_mesh_size = 1;
        assert!(NuclearFuelPin::new(1, vec![r], &[0.5], &[1.0], 0).is_err());
    }
}
