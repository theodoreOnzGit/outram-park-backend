// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerModels/nuclearSteadyStatePebble/nuclearSteadyStatePebble.{C,H}
//             (constructor, setInterfacialArea, correct, correctT)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `nuclearSteadyStatePebble` as the structure's power model (gh:#1876, README round 87)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! A statement-for-statement translation of upstream's class as `structure::correct` runs it, for the coupled `onePhase`
//! region: the constructor evaluates the chain once from `pebbleSurfaceTemperatureGuess` (conductivities and densities at
//! the guess), `correct(HT, H)` evaluates it from `TpS = (surfaceHeatFlux + HT)/H` with the conductivities of the PREVIOUS
//! call's temperatures (`kfuel(Tfav)`, `kbuffer/kSiC/kPyC((TfS + Tmav)/2)`, `kgraphite(Tmav)`), and `correctT` hands `TpS`
//! to the structure. `iA = 3 alpha / pebbleShellRadius`.
//!
//! The arithmetic keeps upstream's expression order (C++ left-to-right association), `pow(x, 3)` and `pow(x, 5)` through
//! PETIR's `r_powf` (`std::pow`), and `pow(x, 2)` as `x * x` (GCC folds `pow(x, 2.0)` to the exact product). The physics
//! reading of the same chain is [`super::PebbleGeometry`] (round 2df1, verified on the gFHR data to the 6-digit write
//! precision); this module exists because that one reorders the arithmetic, takes the conductivities as values and requires
//! a central graphite core (`3D_HTR-10` has `pebbleCoreRadius 0`).
//!
//! Not ported: `fractionOfPowerFromNeutronics` is read and kept, and upstream never uses it either (its use in `correct` is
//! commented out). The model's own `READ_IF_PRESENT` fields (`Tfav.nuclearSteadyStatePebble`, ...) are overwritten by the
//! constructor in every cell of the model and read nowhere else, so they are not read.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use core::f64::consts::PI;
use petir::mathf::RealMath;

/// The eight coefficients of an OpenFOAM `Polynomial<8>` (no log term: the dictionary form `(a0 ... a7)` has none).
pub type Poly8 = [f64; 8];

/// `Polynomial<8>::value(x)` (`Polynomial.C:152-170`): `a0`, then `a_i x^i` with `x^i` built by repeated products.
#[must_use]
pub fn poly_value(c: &Poly8, x: f64) -> f64 {
    let mut val = c[0];
    let mut pow_x = x;
    for &ci in &c[1..] {
        val += ci * pow_x;
        pow_x *= x;
    }
    val
}

/// One region (cellZone) of the model: the dictionary entries upstream reads (`get` = required, `lookupOrDefault` = optional).
#[derive(Debug, Clone, PartialEq)]
pub struct PebbleRegion {
    /// The zone's cells in cellZone order.
    pub cells: Vec<usize>,
    /// The powerModel `volumeFraction` override, if given (`powerModel.C`).
    pub volume_fraction: Option<f64>,
    /// `fractionOfPowerFromNeutronics` (default 1; read, unused upstream).
    pub fraction_of_power_from_neutronics: f64,
    /// `pebbleSurfaceTemperatureGuess` (default 900 K).
    pub tps_guess: f64,
    /// `pebbleCoreRadius`, `pebbleMatrixRadius`, `pebbleShellRadius` [m].
    pub r_core: f64,
    /// See [`Self::r_core`].
    pub r_matrix: f64,
    /// See [`Self::r_core`].
    pub r_shell: f64,
    /// `trisoFuelRadius`, `trisoBufferRadius`, `trisoInnerPyCRadius`, `trisoSiCRadius`, `trisoOuterPyCRadius` [m].
    pub r_fuel: f64,
    /// See [`Self::r_fuel`].
    pub r_buffer: f64,
    /// See [`Self::r_fuel`].
    pub r_pyc_in: f64,
    /// See [`Self::r_fuel`].
    pub r_sic: f64,
    /// See [`Self::r_fuel`].
    pub r_pyc_out: f64,
    /// `nTRISO` (a `label`).
    pub n_triso: i64,
    /// `trisoFuelKCoeffs`, `trisoBufferKCoeffs`, `trisoSiCKCoeffs`, `trisoPyCKCoeffs`, `pebbleGraphiteKCoeffs`.
    pub k_fuel: Poly8,
    /// See [`Self::k_fuel`].
    pub k_buffer: Poly8,
    /// See [`Self::k_fuel`].
    pub k_sic: Poly8,
    /// See [`Self::k_fuel`].
    pub k_pyc: Poly8,
    /// See [`Self::k_fuel`].
    pub k_graphite: Poly8,
    /// `trisoFuelDensityCoeffs`, `trisoBufferDensityCoeffs`, `trisoSiCDensityCoeffs`, `trisoPyCDensityCoeffs`,
    /// `pebbleGraphiteDensityCoeffs`, `coolantDensityCoeffs`.
    pub rho_fuel: Poly8,
    /// See [`Self::rho_fuel`].
    pub rho_buffer: Poly8,
    /// See [`Self::rho_fuel`].
    pub rho_sic: Poly8,
    /// See [`Self::rho_fuel`].
    pub rho_pyc: Poly8,
    /// See [`Self::rho_fuel`].
    pub rho_graphite: Poly8,
    /// See [`Self::rho_fuel`].
    pub rho_coolant: Poly8,
}

/// The volume-weighted average, minimum and maximum of one temperature over the model's cells, as `correct` prints them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvgMinMax {
    /// `sum(T dV)/sum(dV)`.
    pub avg: f64,
    /// Minimum (`1e69` before any cell).
    pub min: f64,
    /// Maximum (`0` before any cell).
    pub max: f64,
}

/// The six `T.nuclearSteadyStatePebble.<X> (avg min max)` lines of the last `correct`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleReport {
    /// `TpS`.
    pub tps: AvgMinMax,
    /// `Tmout`.
    pub tmout: AvgMinMax,
    /// `Tmav`.
    pub tmav: AvgMinMax,
    /// `TfS`.
    pub tfs: AvgMinMax,
    /// `Tfav`.
    pub tfav: AvgMinMax,
    /// `Tfmax`.
    pub tfmax: AvgMinMax,
}

/// The per-cell chain fields of the model (upstream's `volScalarField`s, internal values).
#[derive(Debug, Clone, PartialEq)]
pub struct PebbleFields {
    /// `TpS`, `Tmout`, `Tmav`, `TfS`, `Tfav`, `Tfmax` [K].
    pub tps: Vec<f64>,
    /// See [`Self::tps`].
    pub tmout: Vec<f64>,
    /// See [`Self::tps`].
    pub tmav: Vec<f64>,
    /// See [`Self::tps`].
    pub tfs: Vec<f64>,
    /// See [`Self::tps`].
    pub tfav: Vec<f64>,
    /// See [`Self::tps`].
    pub tfmax: Vec<f64>,
    /// `keffmatrix` [W/(m K)].
    pub keff_matrix: Vec<f64>,
    /// `surfaceHeatFlux` [W/m^2].
    pub surface_heat_flux: Vec<f64>,
    /// `rhofuel`, `rhobuffer`, `rhoSiC`, `rhoPyC`, `rhographite`, `rhocoolant` [kg/m^3].
    pub rho_fuel: Vec<f64>,
    /// See [`Self::rho_fuel`].
    pub rho_buffer: Vec<f64>,
    /// See [`Self::rho_fuel`].
    pub rho_sic: Vec<f64>,
    /// See [`Self::rho_fuel`].
    pub rho_pyc: Vec<f64>,
    /// See [`Self::rho_fuel`].
    pub rho_graphite: Vec<f64>,
    /// See [`Self::rho_fuel`].
    pub rho_coolant: Vec<f64>,
}

/// The `nuclearSteadyStatePebble` power model over a whole mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclearSteadyStatePebble {
    cell_list: Vec<usize>,
    cell_to_region: Vec<usize>,
    regions: Vec<PebbleRegion>,
    alpha: Vec<f64>,
    ia: Vec<f64>,
    fields: PebbleFields,
    report: Option<PebbleReport>,
}

/// One evaluation of the chain in one cell: `(TpS, Tmout, Tmav, TfS, Tfav, Tfmax, keffmatrix, surfaceHeatFlux)`.
struct Chain {
    tps: f64,
    tmout: f64,
    tmav: f64,
    tfs: f64,
    tfav: f64,
    tfmax: f64,
    keff: f64,
    shf: f64,
}

/// Conductivities of one cell's evaluation.
struct Ks {
    fuel: f64,
    buffer: f64,
    sic: f64,
    pyc: f64,
    graphite: f64,
}

/// `pow(x, 3)` of upstream (`std::pow`).
fn pow3(x: f64) -> f64 {
    x.r_powf(3.0)
}

/// `pow(x, 5)` of upstream (`std::pow`).
fn pow5(x: f64) -> f64 {
    x.r_powf(5.0)
}

/// `pow(x, 2)` of upstream: GCC folds `pow(x, 2.0)` to `x * x`.
fn pow2(x: f64) -> f64 {
    x * x
}

/// The chain of `nuclearSteadyStatePebble.C:404-436` (constructor) and `:530-560` (`correct`), in upstream's order. `tps` is
/// `None` in the constructor (`TpS = guess`, passed as `guess`) and `Some((HT, H))` in `correct`.
fn chain(r: &PebbleRegion, k: &Ks, q: f64, dv: f64, alpha: f64, guess: f64, ht_h: Option<(f64, f64)>) -> Chain {
    let (rcore, rmatrix, rshell) = (r.r_core, r.r_matrix, r.r_shell);
    let (rfuel, rbuffer, rpycin, rsic, rpycout) = (r.r_fuel, r.r_buffer, r.r_pyc_in, r.r_sic, r.r_pyc_out);
    let ntriso = r.n_triso as f64;
    let q_cell = q * dv;
    let q_pebble_tot = if alpha == 0.0 { 0.0 } else { q_cell * ((4.0 / 3.0 * PI * pow3(rshell)) / dv) * (1.0 / alpha) };
    let q_matrix = q_pebble_tot / (4.0 / 3.0 * PI * (pow3(rmatrix) - pow3(rcore)));
    let q_triso = q_pebble_tot / ntriso;
    let q_fuel = q_triso / (4.0 / 3.0 * PI * pow3(rfuel));
    let shf = q_pebble_tot / (4.0 * PI * pow2(rshell));
    let ktriso = (1.0 / rfuel - 1.0 / rpycout)
        / ((1.0 / k.buffer) * (1.0 / rfuel - 1.0 / rbuffer)
            + (1.0 / k.pyc) * (1.0 / rbuffer - 1.0 / rpycin)
            + (1.0 / k.sic) * (1.0 / rpycin - 1.0 / rsic)
            + 1.0 / k.pyc * (1.0 / rsic - 1.0 / rpycout));
    let pf = (4.0 / 3.0 * PI * pow3(rpycout)) * ntriso / (4.0 / 3.0 * PI * (pow3(rmatrix) - pow3(rcore)));
    let kappa_coef = ktriso / k.graphite;
    let beta_coef = (kappa_coef - 1.0) / (kappa_coef + 2.0);
    let keff = k.graphite * ((1.0 + 2.0 * beta_coef * pf) / (1.0 - beta_coef * pf));
    let tps = match ht_h {
        Some((ht, h)) => (shf + ht) / h,
        None => guess,
    };
    let tmout = tps + (q_pebble_tot / (4.0 * PI * k.graphite)) * (1.0 / rmatrix - 1.0 / rshell);
    let tmav = tmout
        + (q_matrix / (6.0 * keff)) * (pow2(rmatrix) - 3.0 * (pow5(rmatrix) - pow5(rcore)) / (5.0 * (pow3(rmatrix) - pow3(rcore))))
        + (pow3(rcore) * q_matrix) / (3.0 * keff) * (1.0 / rmatrix - 3.0 * (pow2(rmatrix) - pow2(rcore)) / (2.0 * (pow3(rmatrix) - pow3(rcore))));
    let tfs = tmav
        + q_triso / (4.0 * PI)
            * (1.0 / k.buffer * (1.0 / rfuel - 1.0 / rbuffer)
                + 1.0 / k.pyc * (1.0 / rbuffer - 1.0 / rpycin)
                + 1.0 / k.sic * (1.0 / rpycin - 1.0 / rsic)
                + 1.0 / k.pyc * (1.0 / rsic - 1.0 / rpycout));
    let tfav = tfs + q_fuel * pow2(rfuel) / (15.0 * k.fuel);
    let tfmax = tfs + q_fuel * pow2(rfuel) / (6.0 * k.fuel);
    Chain { tps, tmout, tmav, tfs, tfav, tfmax, keff, shf }
}

/// `updateLocalAvgGlobalMinMaxT`.
fn min_max(t: f64, s: &mut AvgMinMax) {
    if t > s.max {
        s.max = t;
    }
    if t < s.min {
        s.min = t;
    }
}

impl NuclearSteadyStatePebble {
    /// The constructor (`nuclearSteadyStatePebble.C:327-484`): `alpha` from each region's `volumeFraction` or the structure's,
    /// the chain at the surface-temperature guess with `q = powerDensityNeutronics`, then `setInterfacialArea`.
    #[must_use]
    pub fn new(regions: Vec<PebbleRegion>, structure_alpha: &[f64], q_neutronics: &[f64], volumes: &[f64]) -> Self {
        let n = structure_alpha.len();
        let z = vec![0.0; n];
        let mut me = Self {
            cell_list: Vec::new(),
            cell_to_region: vec![0; n],
            regions,
            alpha: vec![0.0; n],
            ia: vec![0.0; n],
            fields: PebbleFields {
                tps: z.clone(),
                tmout: z.clone(),
                tmav: z.clone(),
                tfs: z.clone(),
                tfav: z.clone(),
                tfmax: z.clone(),
                keff_matrix: z.clone(),
                surface_heat_flux: z.clone(),
                rho_fuel: z.clone(),
                rho_buffer: z.clone(),
                rho_sic: z.clone(),
                rho_pyc: z.clone(),
                rho_graphite: z.clone(),
                rho_coolant: z,
            },
            report: None,
        };
        for (ri, r) in me.regions.iter().enumerate() {
            for &c in &r.cells {
                me.cell_list.push(c);
                me.cell_to_region[c] = ri;
                me.alpha[c] = match r.volume_fraction {
                    Some(v) => v,
                    None => structure_alpha[c],
                };
            }
        }
        for i in 0..me.cell_list.len() {
            let c = me.cell_list[i];
            let r = &me.regions[me.cell_to_region[c]];
            let g = r.tps_guess;
            let k = Ks { fuel: poly_value(&r.k_fuel, g), buffer: poly_value(&r.k_buffer, g), sic: poly_value(&r.k_sic, g), pyc: poly_value(&r.k_pyc, g), graphite: poly_value(&r.k_graphite, g) };
            let ch = chain(r, &k, q_neutronics[c], volumes[c], me.alpha[c], g, None);
            let f = &mut me.fields;
            f.surface_heat_flux[c] = ch.shf;
            f.tps[c] = ch.tps;
            f.tmout[c] = ch.tmout;
            f.tmav[c] = ch.tmav;
            f.tfs[c] = ch.tfs;
            f.tfav[c] = ch.tfav;
            f.tfmax[c] = ch.tfmax;
            f.keff_matrix[c] = ch.keff;
            f.rho_fuel[c] = poly_value(&r.rho_fuel, g);
            f.rho_buffer[c] = poly_value(&r.rho_buffer, g);
            f.rho_sic[c] = poly_value(&r.rho_sic, g);
            f.rho_pyc[c] = poly_value(&r.rho_pyc, g);
            f.rho_graphite[c] = poly_value(&r.rho_graphite, g);
            f.rho_coolant[c] = poly_value(&r.rho_coolant, g);
        }
        // setInterfacialArea: iA = 3 alpha / rshell
        for i in 0..me.cell_list.len() {
            let c = me.cell_list[i];
            me.ia[c] = 3.0 * me.alpha[c] / me.regions[me.cell_to_region[c]].r_shell;
        }
        me
    }

    /// `correct(HTSum, HSum)` (`nuclearSteadyStatePebble.C:501-660`): the chain from `TpS = (surfaceHeatFlux + HT)/H`, the
    /// densities, and the volume-weighted report.
    pub fn correct(&mut self, ht_sum: &[f64], h_sum: &[f64], q_neutronics: &[f64], volumes: &[f64]) {
        let fresh = AvgMinMax { avg: 0.0, min: 1e69, max: 0.0 };
        let mut rep = PebbleReport { tps: fresh, tmout: fresh, tmav: fresh, tfs: fresh, tfav: fresh, tfmax: fresh };
        let mut tot_v = 0.0;
        for i in 0..self.cell_list.len() {
            let c = self.cell_list[i];
            let r = &self.regions[self.cell_to_region[c]];
            let f = &mut self.fields;
            let mid = (f.tfs[c] + f.tmav[c]) / 2.0;
            let k = Ks {
                fuel: poly_value(&r.k_fuel, f.tfav[c]),
                buffer: poly_value(&r.k_buffer, mid),
                sic: poly_value(&r.k_sic, mid),
                pyc: poly_value(&r.k_pyc, mid),
                graphite: poly_value(&r.k_graphite, f.tmav[c]),
            };
            let dv = volumes[c];
            let ch = chain(r, &k, q_neutronics[c], dv, self.alpha[c], 0.0, Some((ht_sum[c], h_sum[c])));
            f.surface_heat_flux[c] = ch.shf;
            f.tps[c] = ch.tps;
            f.tmout[c] = ch.tmout;
            f.tmav[c] = ch.tmav;
            f.tfs[c] = ch.tfs;
            f.tfav[c] = ch.tfav;
            f.tfmax[c] = ch.tfmax;
            f.keff_matrix[c] = ch.keff;
            let mid = (f.tfs[c] + f.tmav[c]) / 2.0;
            f.rho_fuel[c] = poly_value(&r.rho_fuel, f.tfav[c]);
            f.rho_buffer[c] = poly_value(&r.rho_buffer, mid);
            f.rho_sic[c] = poly_value(&r.rho_sic, mid);
            f.rho_pyc[c] = poly_value(&r.rho_pyc, mid);
            f.rho_graphite[c] = poly_value(&r.rho_graphite, f.tmav[c]);
            f.rho_coolant[c] = poly_value(&r.rho_coolant, ht_sum[c] / h_sum[c]);
            tot_v += dv;
            rep.tfav.avg += f.tfav[c] * dv;
            rep.tfmax.avg += f.tfmax[c] * dv;
            rep.tmav.avg += f.tmav[c] * dv;
            rep.tps.avg += f.tps[c] * dv;
            rep.tfs.avg += f.tfs[c] * dv;
            rep.tmout.avg += f.tmout[c] * dv;
            min_max(f.tfav[c], &mut rep.tfav);
            min_max(f.tfmax[c], &mut rep.tfmax);
            min_max(f.tmout[c], &mut rep.tmout);
            min_max(f.tmav[c], &mut rep.tmav);
            min_max(f.tps[c], &mut rep.tps);
            min_max(f.tfs[c], &mut rep.tfs);
        }
        for s in [&mut rep.tfav, &mut rep.tfmax, &mut rep.tmav, &mut rep.tps, &mut rep.tmout, &mut rep.tfs] {
            s.avg /= tot_v;
        }
        self.report = Some(rep);
    }

    /// `correctT`: the pebble surface temperature `TpS` of a cell of the model.
    #[must_use]
    pub fn surface_temperature(&self, cell: usize) -> f64 {
        self.fields.tps[cell]
    }

    /// `iA` of a cell.
    #[must_use]
    pub fn interfacial_area(&self, cell: usize) -> f64 {
        self.ia[cell]
    }

    /// The model's cells in `cellList_` order.
    #[must_use]
    pub fn cells(&self) -> &[usize] {
        &self.cell_list
    }

    /// The chain fields.
    #[must_use]
    pub fn fields(&self) -> &PebbleFields {
        &self.fields
    }

    /// The report of the last `correct` (`None` before the first).
    #[must_use]
    pub fn report(&self) -> Option<PebbleReport> {
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(v: f64) -> Poly8 {
        [v, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    }

    /// `Polynomial::value` by hand on a cubic.
    #[test]
    fn poly_value_by_hand() {
        let c = [1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(poly_value(&c, 2.0), 1.0 + 4.0 + 12.0 + 32.0);
    }

    /// The gFHR pebble (round 2df1's geometry): the same chain as [`super::super::PebbleGeometry::steady_temperatures`] to
    /// round-off, from the coupled form's surface temperature `(q'' + HT)/H`, and `iA = 3 alpha / r_shell`.
    #[test]
    fn coupled_chain_agrees_with_the_physics_reading_on_the_gfhr_pebble() {
        let reg = PebbleRegion {
            cells: vec![0],
            volume_fraction: None,
            fraction_of_power_from_neutronics: 1.0,
            tps_guess: 900.0,
            r_core: 1.38e-2,
            r_matrix: 1.8e-2,
            r_shell: 2.0e-2,
            r_fuel: 212.5e-6,
            r_buffer: 312.5e-6,
            r_pyc_in: 352.5e-6,
            r_sic: 387.5e-6,
            r_pyc_out: 427.5e-6,
            n_triso: 9022,
            k_fuel: constant(3.3073),
            k_buffer: constant(0.5),
            k_sic: constant(90.3),
            k_pyc: constant(4.0),
            k_graphite: constant(41.68),
            rho_fuel: constant(10500.0),
            rho_buffer: constant(1000.0),
            rho_sic: constant(3216.0),
            rho_pyc: constant(1900.0),
            rho_graphite: constant(1632.0),
            rho_coolant: constant(1900.0),
        };
        let (q, dv, alpha) = (2.0e7, 1.0e-3, 0.61);
        let mut m = NuclearSteadyStatePebble::new(vec![reg], &[alpha], &[q], &[dv]);
        assert!((m.interfacial_area(0) - 3.0 * 0.61 / 2.0e-2).abs() < 1e-12);
        assert_eq!(m.surface_temperature(0), 900.0);
        // H = 1e5, HT = H * 850 - q'': TpS = 850 exactly in exact arithmetic
        let shf = m.fields().surface_heat_flux[0];
        m.correct(&[1.0e5 * 850.0 - shf], &[1.0e5], &[q], &[dv]);
        let tps = m.surface_temperature(0);
        assert!((tps - 850.0).abs() < 1e-9, "TpS {tps}");
        let geo = super::super::PebbleGeometry {
            core_radius: 1.38e-2,
            matrix_radius: 1.8e-2,
            shell_radius: 2.0e-2,
            fuel_radius: 212.5e-6,
            buffer_radius: 312.5e-6,
            inner_pyc_radius: 352.5e-6,
            silicon_carbide_radius: 387.5e-6,
            outer_pyc_radius: 427.5e-6,
            triso_count: 9022.0,
        };
        let k = super::super::PebbleConductivities { fuel: 3.3073, buffer: 0.5, pyrolytic_carbon: 4.0, silicon_carbide: 90.3, graphite: 41.68 };
        let want = geo.steady_temperatures(tps, q, alpha, &k);
        let f = m.fields();
        for (got, w) in [(f.tmout[0], want.matrix_outer), (f.tmav[0], want.matrix_average), (f.tfs[0], want.particle_surface), (f.tfav[0], want.fuel_average), (f.tfmax[0], want.fuel_max), (f.keff_matrix[0], want.matrix_effective_conductivity)] {
            assert!((got - w).abs() <= 1e-12 * w.abs(), "{got} vs {w}");
        }
        let rep = m.report().expect("report");
        assert_eq!(rep.tfmax.max, f.tfmax[0]);
        assert_eq!(rep.tfmax.avg, f.tfmax[0]);
    }

    /// `pebbleCoreRadius 0` (HTR-10): the core terms vanish and the chain stays finite; `alpha = 0` gives no power.
    #[test]
    fn a_coreless_pebble_and_an_empty_cell() {
        let mut reg = PebbleRegion {
            cells: vec![0, 1],
            volume_fraction: None,
            fraction_of_power_from_neutronics: 1.0,
            tps_guess: 523.15,
            r_core: 0.0,
            r_matrix: 2.5e-2,
            r_shell: 3e-2,
            r_fuel: 2.5e-4,
            r_buffer: 3.4e-4,
            r_pyc_in: 3.8e-4,
            r_sic: 4.15e-4,
            r_pyc_out: 4.55e-4,
            n_triso: 8335,
            k_fuel: constant(3.3073),
            k_buffer: constant(0.5),
            k_sic: constant(98.0),
            k_pyc: constant(4.0),
            k_graphite: constant(42.0),
            rho_fuel: constant(11031.432),
            rho_buffer: constant(1000.0),
            rho_sic: constant(3216.0),
            rho_pyc: constant(1900.0),
            rho_graphite: constant(1632.0),
            rho_coolant: constant(8.5),
        };
        reg.volume_fraction = None;
        let m = NuclearSteadyStatePebble::new(vec![reg], &[0.6, 0.0], &[2.0e6, 2.0e6], &[1e-6, 1e-6]);
        let f = m.fields();
        assert!(f.tfmax[0].is_finite() && f.tfmax[0] > 523.15);
        // matrix average of a solid uniformly heated sphere of radius rm: Tm(rm) + q rm^2/(15 k)
        let q_peb = 2.0e6 * (4.0 / 3.0 * PI * 3e-2f64.powi(3)) / 0.6;
        let qm = q_peb / (4.0 / 3.0 * PI * 2.5e-2f64.powi(3));
        let want = f.tmout[0] + qm * 2.5e-2f64.powi(2) / (15.0 * f.keff_matrix[0]);
        assert!((f.tmav[0] - want).abs() < 1e-9 * want);
        assert_eq!(f.tfmax[1], 523.15);
        assert_eq!(m.interfacial_area(1), 0.0);
        assert_eq!(m.cells(), &[0, 1]);
    }
}
