// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerModels/heatedPin/heatedPin.{C,H} (and powerModel.{C,H})
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman, Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `heatedPin` — a hollow or solid pin with a prescribed power density and a convective surface
//!
//! Translation of GeN-Foam's `powerModels::heatedPin` (constant `rho Cp`, `k`; one radial finite-volume
//! problem per mesh cell, zero-gradient at the inner surface, convective at the outer one). Per cell and
//! step it builds the tridiagonal system of `heatedPin::updateLocalTemperatureProfile` and solves it with
//! `Foam::solve` ([`super::radial_solve::foam_solve`]).
//!
//! ## Scheme (read from the source, `heatedPin.C`)
//! Nodes `r_0 = r_i, r_j = r_{j-1} + dr` (a running sum, as upstream), `dr = (r_o - r_i)/(N-1)`; ring areas
//! `dA_0 = pi((r_0+dr/2)^2 - r_0^2)`, `dA_j = pi((r_j+dr/2)^2 - (r_j-dr/2)^2)`,
//! `dA_{N-1} = pi(r_{N-1}^2 - (r_{N-1}-dr/2)^2)`. With `X = rhoCp/dt`, `2 pi k/dr` and `D = 2 pi r_{N-1}`:
//! ```text
//!   node 0     : (B + X dA) T0 - B T1                       = q dA + X dA T0_old     , B = 2 pi k/dr (r+dr/2)
//!   node j     : -C T_{j-1} + (B + C + X dA) T_j - B T_{j+1} = q dA + X dA Tj_old    , C = 2 pi k/dr (r-dr/2)
//!   node N-1   : -C T_{N-2} + (C + D H + X dA) T_{N-1}       = q dA + X dA T_old + D HT
//! ```
//! `H = sum_j h_j frac_j`, `HT = sum_j h_j frac_j T_j` (the fluid coupling). The matrix is conservative: in
//! steady state the surface flux balances `q pi (r_o^2 - r_i^2)` exactly (the V&V test checks that and the
//! second-order convergence of the centre temperature to `T_o + q (r_o^2 - r^2)/(4k)`).
//!
//! ## Not ported (refused by name)
//! * The reconstruction of the initial profile from `Ti/To.heatedPin` files (`foundBoundaryTemperatures`) and
//!   the read of `Trad.heatedPin` from a time directory: the initial profile is `T` of the dictionary, as the
//!   shipped tutorials have no such files.
//! * `Info` output and the dictionary write-back of `Tavav/Tmax/Tmin` are kept as plain fields
//!   ([`HeatedPin::t_avav`], [`HeatedPin::t_max`], [`HeatedPin::t_min`]).
//!
//! The `reduce(...)` over processors is the identity (serial only).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::radial_solve::{
    foam_solve, lu_control_solve, sqr, update_average_and_extrema, OldTimeFieldField, OldTimePolicy, RadialError,
    PI,
};

/// One `heatedPin` region (one cell zone's `powerModel` sub-dictionary).
#[derive(Debug, Clone, PartialEq)]
pub struct HeatedPinRegion {
    /// Cells of the zone, in cell-zone order (the order of `structure_.cellLists()[region]`).
    pub cells: Vec<usize>,
    /// `innerRadius` [m] (0 for a solid pin).
    pub inner_radius: f64,
    /// `outerRadius` [m].
    pub outer_radius: f64,
    /// `meshSize`: number of radial nodes, at least 2.
    pub mesh_size: usize,
    /// `rho * Cp` (or `rhoCp`) [J/m^3/K].
    pub rho_cp: f64,
    /// `k` [W/m/K].
    pub k: f64,
    /// `T`: initial pin temperature [K].
    pub initial_temperature: f64,
    /// `powerDensity` [W/m^3] (`setRegionField(powerDensity_, "powerDensity")`).
    pub power_density: f64,
    /// The powerModel `volumeFraction` if the sub-dictionary has one, else `None` (the structure alpha).
    pub volume_fraction: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct RegionDerived {
    r: Vec<f64>,
    d_a: Vec<f64>,
    dr: f64,
}

/// The `heatedPin` power model over a whole mesh (all its regions).
#[derive(Debug, Clone, PartialEq)]
pub struct HeatedPin {
    regions: Vec<HeatedPinRegion>,
    derived: Vec<RegionDerived>,
    cell_list: Vec<usize>,
    cell_to_region: Vec<usize>,
    trad: OldTimeFieldField,
    power_density: Vec<f64>,
    ti: Vec<f64>,
    to: Vec<f64>,
    tav: Vec<f64>,
    ia: Vec<f64>,
    alpha: Vec<f64>,
    t_max: f64,
    t_min: f64,
    t_avav: f64,
    use_lu_control: bool,
}

impl HeatedPin {
    /// `heatedPin::heatedPin` (`n_cells` mesh cells, `structure_alpha` the structure volume fraction per
    /// cell, `volumes` the cell volumes, `time_index` the time index of the construction, 0 at start).
    ///
    /// # Errors
    /// [`RadialError::Invalid`] for a mesh size below 2 (upstream divides by `meshSize - 1`).
    pub fn new(
        n_cells: usize,
        regions: Vec<HeatedPinRegion>,
        structure_alpha: &[f64],
        volumes: &[f64],
        time_index: i64,
    ) -> Result<Self, RadialError> {
        let mut cell_list = Vec::new();
        let mut cell_to_region = vec![0_usize; n_cells];
        let mut alpha = vec![0.0; n_cells];
        let mut power_density = vec![0.0; n_cells];
        let mut trad_cur: Vec<Vec<f64>> = vec![Vec::new(); n_cells];
        let mut derived = Vec::new();
        for (regioni, reg) in regions.iter().enumerate() {
            if reg.mesh_size < 2 {
                return Err(RadialError::Invalid("heatedPin: meshSize must be at least 2"));
            }
            let dr = (reg.outer_radius - reg.inner_radius) / ((reg.mesh_size as f64) - 1.0);
            let mut r = vec![reg.inner_radius];
            for _ in 0..(reg.mesh_size - 1) {
                let last = r[r.len() - 1];
                r.push(last + dr);
            }
            let n = reg.mesh_size;
            let mut d_a = vec![PI * (sqr(r[0] + dr / 2.0) - sqr(r[0]))];
            for i in 1..(n - 1) {
                d_a.push(PI * (sqr(r[i] + dr / 2.0) - sqr(r[i] - dr / 2.0)));
            }
            d_a.push(PI * (sqr(r[n - 1]) - sqr(r[n - 1] - dr / 2.0)));
            for &c in &reg.cells {
                cell_list.push(c);
                cell_to_region[c] = regioni;
                alpha[c] = reg.volume_fraction.unwrap_or(structure_alpha[c]);
                power_density[c] = reg.power_density;
                trad_cur[c] = vec![reg.initial_temperature; n];
            }
            derived.push(RegionDerived { r, d_a, dr });
        }
        let mut me = Self {
            regions,
            derived,
            cell_list,
            cell_to_region,
            trad: OldTimeFieldField::new(trad_cur, time_index),
            power_density,
            ti: vec![0.0; n_cells],
            to: vec![0.0; n_cells],
            tav: vec![0.0; n_cells],
            ia: vec![0.0; n_cells],
            alpha,
            t_max: 0.0,
            t_min: 1e69,
            t_avav: 0.0,
            use_lu_control: false,
        };
        // set I/O fields and compute the initial scalar max, min
        let mut tavav = 0.0;
        let mut tot_v = 0.0;
        for &celli in &me.cell_list.clone() {
            let regioni = me.cell_to_region[celli];
            let n = me.regions[regioni].mesh_size;
            let trad = me.trad.current[celli].clone();
            me.ti[celli] = trad[0];
            me.to[celli] = trad[n - 1];
            let (mut lo, mut hi) = (me.t_min, me.t_max);
            let tavi = update_average_and_extrema(0, n, &me.derived[regioni].r, me.derived[regioni].dr, &trad, &mut lo, &mut hi);
            me.t_min = lo;
            me.t_max = hi;
            me.tav[celli] = tavi;
            let dv = volumes[celli];
            tot_v += dv;
            tavav += tavi * dv;
        }
        tavav /= tot_v;
        me.t_avav = tavav;
        me.set_interfacial_area();
        Ok(me)
    }

    /// CONTROL ONLY: solve with `lu_decompose` instead of `Foam::solve` (not what GeN-Foam does).
    #[must_use]
    pub fn with_lu_control_solver(mut self) -> Self {
        self.use_lu_control = true;
        self
    }

    /// Select the old-time policy of `Trad` (the control uses [`OldTimePolicy::StartOfStep`]).
    #[must_use]
    pub fn with_old_time_policy(mut self, policy: OldTimePolicy) -> Self {
        self.trad = self.trad.clone().with_policy(policy);
        self
    }

    /// `heatedPin::setInterfacialArea`: `iA = 2 alpha / r_o` in the model's cells.
    pub fn set_interfacial_area(&mut self) {
        for &celli in &self.cell_list {
            self.ia[celli] = 2.0 * self.alpha[celli] / self.regions[self.cell_to_region[celli]].outer_radius;
        }
    }

    /// `heatedPin::updateLocalTemperatureProfile` for one cell.
    fn update_local_temperature_profile(
        &mut self,
        celli: usize,
        ht_sumi: f64,
        h_sumi: f64,
        dt: f64,
        time_index: i64,
    ) -> Result<(), RadialError> {
        let regioni = self.cell_to_region[celli];
        let reg = &self.regions[regioni];
        let n = reg.mesh_size;
        let der = &self.derived[regioni];
        let k = reg.k;
        let q = self.power_density[celli];
        let t_old = self.trad.old_time_of(celli, time_index);
        // recurrent quantities
        let drh = der.dr / 2.0;
        let two_pk_by_dr = 2.0 * PI * k / der.dr;
        let x = reg.rho_cp / dt;
        let mut m = vec![0.0_f64; n * n];
        let mut s = vec![0.0_f64; n];
        {
            // zeroGradient BC at the pin inner surface
            let r = der.r[0];
            let d_a = der.d_a[0];
            let b = two_pk_by_dr * (r + drh);
            let xd_a = x * d_a;
            m[1] = -b;
            m[0] = b + xd_a;
            s[0] = q * d_a + t_old[0] * xd_a;
        }
        for i in 1..(n - 1) {
            let r = der.r[i];
            let d_a = der.d_a[i];
            let b = two_pk_by_dr * (r + drh);
            let c = two_pk_by_dr * (r - drh);
            let xd_a = x * d_a;
            m[i * n + i + 1] = -b;
            m[i * n + i - 1] = -c;
            m[i * n + i] = b + c + xd_a;
            s[i] = q * d_a + t_old[i] * xd_a;
        }
        {
            // convective BC at the pin outer surface
            let i = n - 1;
            let r = der.r[i];
            let d_a = der.d_a[i];
            let c = two_pk_by_dr * (r - drh);
            let d = 2.0 * PI * r;
            let xd_a = x * d_a;
            m[i * n + i - 1] = -c;
            m[i * n + i] = c + d * h_sumi + xd_a;
            s[i] = q * d_a + t_old[i] * xd_a + d * ht_sumi;
        }
        let trad = if self.use_lu_control { lu_control_solve(&m, n, &s)? } else { foam_solve(&m, n, &s)? };
        self.ti[celli] = trad[0];
        self.to[celli] = trad[n - 1];
        let (mut lo, mut hi) = (self.t_min, self.t_max);
        let tavi = update_average_and_extrema(0, n, &self.derived[regioni].r, self.derived[regioni].dr, &trad, &mut lo, &mut hi);
        self.t_min = lo;
        self.t_max = hi;
        self.tav[celli] = tavi;
        self.trad.current[celli] = trad;
        Ok(())
    }

    /// `heatedPin::correct(HT, H)`: `ht_sum`/`h_sum` per mesh cell, `dt` the time step, `time_index` the
    /// current time index (`mesh.time().timeIndex()`), `volumes` the cell volumes.
    ///
    /// # Errors
    /// A singular radial matrix.
    pub fn correct(
        &mut self,
        ht_sum: &[f64],
        h_sum: &[f64],
        volumes: &[f64],
        dt: f64,
        time_index: i64,
    ) -> Result<(), RadialError> {
        // reset min, max
        self.t_max = 0.0;
        self.t_min = 1e69;
        let mut tot_v = 0.0;
        let mut tavav = 0.0;
        for i in 0..self.cell_list.len() {
            let celli = self.cell_list[i];
            self.update_local_temperature_profile(celli, ht_sum[celli], h_sum[celli], dt, time_index)?;
            let dv = volumes[celli];
            tot_v += dv;
            tavav += self.tav[celli] * dv;
        }
        tavav /= tot_v;
        self.t_avav = tavav;
        Ok(())
    }

    /// `heatedPin::correctT`: the pin surface temperature `To` of a cell of the model.
    #[must_use]
    pub fn surface_temperature(&self, cell: usize) -> f64 {
        self.to[cell]
    }

    /// `heatedPin::powerOff`: the power density becomes zero.
    pub fn power_off(&mut self) {
        for q in &mut self.power_density {
            *q *= 0.0;
        }
    }

    /// The model's cells, in `cellList_` order.
    #[must_use]
    pub fn cells(&self) -> &[usize] {
        &self.cell_list
    }

    /// The radial temperature profile `Trad.heatedPin` of a cell.
    #[must_use]
    pub fn trad(&self, cell: usize) -> &[f64] {
        &self.trad.current[cell]
    }

    /// The radial node positions of the region of `cell`.
    #[must_use]
    pub fn radii(&self, cell: usize) -> &[f64] {
        &self.derived[self.cell_to_region[cell]].r
    }

    /// `Ti.heatedPin` (inner pin temperature).
    #[must_use]
    pub fn ti(&self, cell: usize) -> f64 {
        self.ti[cell]
    }

    /// `To.heatedPin` (outer pin temperature).
    #[must_use]
    pub fn to(&self, cell: usize) -> f64 {
        self.to[cell]
    }

    /// `Tav.heatedPin` (local `r dr`-weighted average).
    #[must_use]
    pub fn tav(&self, cell: usize) -> f64 {
        self.tav[cell]
    }

    /// `volumetricArea.powerModel` (`iA`) of a cell.
    #[must_use]
    pub fn interfacial_area(&self, cell: usize) -> f64 {
        self.ia[cell]
    }

    /// Dictionary entry `Tavav` (volume-weighted average of `Tav`).
    #[must_use]
    pub fn t_avav(&self) -> f64 {
        self.t_avav
    }

    /// Dictionary entry `Tmax`.
    #[must_use]
    pub fn t_max(&self) -> f64 {
        self.t_max
    }

    /// Dictionary entry `Tmin`.
    #[must_use]
    pub fn t_min(&self) -> f64 {
        self.t_min
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(n: usize, ri: f64, q: f64) -> HeatedPin {
        let reg = HeatedPinRegion {
            cells: vec![0],
            inner_radius: ri,
            outer_radius: 0.005,
            mesh_size: n,
            rho_cp: 3.9e6,
            k: 20.0,
            initial_temperature: 600.0,
            power_density: q,
            volume_fraction: None,
        };
        HeatedPin::new(1, vec![reg], &[0.4], &[1.0], 0).unwrap()
    }

    /// Analytic check (steady, `dt` huge): solid cylinder, uniform `q`, convective surface. Global energy
    /// balance exact to round-off for any mesh; centre rise `q ro^2/(4k)` converges at second order.
    /// Result 2026-11-27: see the assertions' bounds; measured values are printed with `--nocapture`.
    #[test]
    fn solid_pin_matches_the_closed_form_and_conserves_energy() {
        let (q, h, t_f, ro, k) = (3.0e8, 2.0e4, 600.0, 0.005, 20.0);
        let mut errs = Vec::new();
        for n in [11_usize, 21, 41] {
            let mut p = pin(n, 0.0, q);
            for step in 1..=3 {
                p.correct(&[h * t_f], &[h], &[1.0], 1.0e12, step).unwrap();
            }
            let to = p.to(0);
            // energy: 2 pi ro (H To - HT) = q pi ro^2
            let balance = 2.0 * PI * ro * (h * to - h * t_f) / (q * PI * ro * ro);
            assert!((balance - 1.0).abs() < 1e-12, "n={n} energy balance {balance}");
            let rise_exact = q * ro * ro / (4.0 * k);
            let rise = p.ti(0) - p.to(0);
            let err = (rise - rise_exact).abs() / rise_exact;
            println!("heatedPin steady solid n={n}: centre rise {rise:.6} K vs {rise_exact:.6} K, relative error {err:.3e}");
            errs.push(err);
        }
        // PREDICTION P4(b) MISSED, and the reason is derived, not tuned: the face fluxes use the exact face
        // radius `r +- dr/2` and the sources the exact ring areas, so for T quadratic in r (the solid pin with
        // uniform q) the finite-volume scheme is EXACT; the error is round-off at every mesh size.
        for e in &errs {
            assert!(*e < 1e-11, "solid pin is exact to round-off: {e}");
        }
    }

    /// Hollow pin, adiabatic inner surface (zeroGradient), uniform `q`: `T(r) = To + q(ro^2-r^2)/(4k) +
    /// q ri^2/(2k) ln(r/ro)`. The logarithm is not captured exactly, so here the error is genuinely second
    /// order: halving `dr` cuts the inner-surface error by about 4.
    #[test]
    fn hollow_pin_converges_at_second_order() {
        use petir::mathf::RealMath;
        let (q, h, t_f, ro, ri, k) = (3.0e8, 2.0e4, 600.0, 0.005, 0.001, 20.0);
        let mut errs = Vec::new();
        for n in [11_usize, 21, 41] {
            let mut p = pin(n, ri, q);
            for step in 1..=3 {
                p.correct(&[h * t_f], &[h], &[1.0], 1.0e12, step).unwrap();
            }
            let to = p.to(0);
            let exact_rise = q * (ro * ro - ri * ri) / (4.0 * k) + q * ri * ri / (2.0 * k) * (ri / ro).r_ln();
            let rise = p.ti(0) - to;
            let err = (rise - exact_rise).abs() / exact_rise;
            println!("heatedPin steady hollow n={n}: inner-outer rise {rise:.6} K vs {exact_rise:.6} K, relative error {err:.3e}");
            errs.push(err);
        }
        let (r1, r2) = (errs[0] / errs[1], errs[1] / errs[2]);
        assert!((3.0..=5.0).contains(&r1) && (3.0..=5.0).contains(&r2), "second-order ratios {r1} {r2}");
        assert!(errs[1] < 1e-2);
    }

    /// Controls: a different elimination order (LU) changes the bits; a different old-time policy changes
    /// the first outer iteration of the first step only.
    #[test]
    fn controls_can_fail() {
        let mut a = pin(12, 0.0, 3.0e8);
        let mut b = pin(12, 0.0, 3.0e8).with_lu_control_solver();
        for step in 1..=4 {
            a.correct(&[2.0e4 * 600.0], &[2.0e4], &[1.0], 0.5, step).unwrap();
            b.correct(&[2.0e4 * 600.0], &[2.0e4], &[1.0], 0.5, step).unwrap();
        }
        let d = a.trad(0).iter().zip(b.trad(0)).map(|(x, y)| (x - y).abs() / x.abs()).fold(0.0, f64::max);
        assert!(d < 1e-12, "same physics: {d}");
        let q = pin(12, 0.0, 3.0e8).with_old_time_policy(OldTimePolicy::StartOfStep);
        let mut q = q;
        let mut u = pin(12, 0.0, 3.0e8);
        for _outer in 0..2 {
            u.correct(&[2.0e4 * 600.0], &[2.0e4], &[1.0], 0.5, 1).unwrap();
            q.correct(&[2.0e4 * 600.0], &[2.0e4], &[1.0], 0.5, 1).unwrap();
        }
        assert!(u.trad(0) != q.trad(0), "the old-time quirk is visible with two outer iterations");
        u.power_off();
        assert_eq!(u.cells(), &[0]);
        assert_eq!(u.radii(0).len(), 12);
        assert!(u.interfacial_area(0) > 0.0 && u.tav(0) > 0.0);
        assert!(u.t_max() >= u.t_min() && u.t_avav() > 0.0 && u.surface_temperature(0) == u.to(0));
    }
}
