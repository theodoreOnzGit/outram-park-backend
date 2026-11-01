// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Heat structures coupled to HEM pipes: PipeWithHeatStructure, CoreChannel
//! and a wall heat exchanger (gh:#1077; RELAP-7 Theory Manual §4.1 and
//! §6.2–6.4, cited, not quoted).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # What this is
//!
//! [`HeatStructure`] is transient 1-D conduction through a wall, a cylinder
//! (a fuel rod: fuel, gap, cladding) or a slab, at every axial cell of a pipe:
//! `ρc ∂T/∂t = ∇·(k∇T) + q'''`, finite volume across the thickness, backward
//! Euler in time, one tridiagonal solve per axial cell
//! ([`crate::multiphase_1d::thomas_solve`]). Zones carry their own `k`, `ρc`
//! and volumetric source; a gap is a zone of gas conductivity (radiation
//! across it is not modelled).
//!
//! [`ThermalCoupler`] couples heat structures to the pipes of a
//! [`HemNetwork`]. Each step the heat to the fluid, `h A (T_s − T_f)` with
//! the surface temperature eliminated through the half-cell resistance, is
//! put into the pipe cells with [`HemPipe::set_cell_heat_input`]; the network
//! steps; and each structure then advances the **same** `dt` with that same
//! heat leaving it. The exchange is frozen over the step, so energy is
//! conserved exactly, and explicit in the coupling; the acoustic step of the
//! fluid keeps `h dt/(ρc Δr)` small.
//!
//! **What exists and why this is new (searched first):** TUAS's solid arrays
//! (`SolidColumn`) are axial with the wall lumped radially; `bedok`'s
//! `fuelrodheattime_1dcylnd` is a transient rod solver, but a port of the
//! MATLAB code with its own geometry and sparse types, and `tampines` does
//! not depend on `bedok`; OFFBEAT has no thermal sub-solver. This module
//! reuses `thomas_solve` and, for MATPRO properties, expects the caller to
//! tabulate `outram-park-fork-offbeat`'s `Relap7HeatStructure` (gh:#1070)
//! into zone constants.
//!
//! # Limits
//!
//! - Properties are constant per zone (no `k(T)` yet).
//! - The heat-transfer coefficient is the caller's (a constant per link);
//!   wiring the RELAP5 dispatcher ([`crate::heat_transfer`], gh:#1063) needs
//!   fluid conductivity and heat capacity, which the HEM state does not carry.
//! - No axial conduction in the structure.
//! - [`ThermalCoupler::step`] **replaces** each coupled pipe's cell heat
//!   input every step; a reactor power source acting on the same pipe must be
//!   given to the structure (its zone source), not to the pipe.

use std::f64::consts::PI;

use petir::mathf::RealMath;

use tampines_steam_tables::hem::{HemEquationOfState, HemNetwork, HemPipe};
use uom::si::f64::{
    HeatTransfer, Length, Power, ThermalConductivity, ThermodynamicTemperature, Time,
    VolumetricHeatCapacity, VolumetricPowerDensity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use crate::multiphase_1d::thomas_solve;
use crate::TampinesError;

/// The shape of a heat structure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructureShape {
    /// A cylinder (or annulus) from `inner_radius` outward; `multiplicity`
    /// identical rods per axial cell.
    Cylinder {
        /// Inner radius (0 for a solid rod, whose inner side is then the
        /// symmetry axis and must be adiabatic).
        inner_radius: Length,
        /// Number of identical rods.
        multiplicity: f64,
    },
    /// A plane wall of this width (the wall area per axial cell is
    /// `width · Δz`).
    Slab {
        /// Width normal to the axis and the thickness.
        width: Length,
    },
}

/// One material layer across the thickness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConductionZone {
    /// Layer thickness.
    pub thickness: Length,
    /// Number of finite-volume cells across it (≥ 1).
    pub n_cells: usize,
    /// Thermal conductivity.
    pub conductivity: ThermalConductivity,
    /// Volumetric heat capacity `ρc`.
    pub volumetric_heat_capacity: VolumetricHeatCapacity,
}

/// A boundary condition on one side of a structure, for
/// [`HeatStructure::step`].
#[derive(Debug, Clone, PartialEq)]
pub enum StructureBoundary {
    /// No heat crosses (also the symmetry axis of a solid rod).
    Adiabatic,
    /// Surface temperature held.
    Temperature(ThermodynamicTemperature),
    /// `h (T_s − T_∞)` leaves the surface.
    Convective {
        /// Heat-transfer coefficient.
        h: HeatTransfer,
        /// Ambient temperature.
        ambient: ThermodynamicTemperature,
    },
    /// A prescribed heat **into** the structure per axial cell, W (negative
    /// out); frozen over the step.
    HeatInflow(Vec<f64>),
}

/// Which side of a structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureSide {
    /// The inner surface (smaller radius, or `x = 0` of a slab).
    Inner,
    /// The outer surface.
    Outer,
}

/// Transient 1-D conduction across a wall at every axial cell.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatStructure {
    shape: StructureShape,
    /// Face positions across the thickness, m (radius or `x`), `n + 1`.
    faces: Vec<f64>,
    k: Vec<f64>,
    rho_c: Vec<f64>,
    n_axial: usize,
    dz: f64,
    /// Temperatures, axial-major: `t[j * n + i]`.
    t: Vec<f64>,
    /// Volumetric source per cell, W/m³, axial-major.
    q: Vec<f64>,
}

impl HeatStructure {
    /// A structure of `zones` (from the inner side outward) at `n_axial`
    /// cells of length `dz`, uniformly at `initial`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] on an empty or non-positive zone, a
    /// non-positive `dz`, multiplicity or width, or a negative inner radius.
    pub fn new(
        shape: StructureShape,
        zones: &[ConductionZone],
        n_axial: usize,
        dz: Length,
        initial: ThermodynamicTemperature,
    ) -> Result<Self, TampinesError> {
        let bad = |m: String| Err(TampinesError::InvalidInput(m));
        let dz = dz.get::<meter>();
        if zones.is_empty() || n_axial == 0 || !(dz.is_finite() && dz > 0.0) {
            return bad("a heat structure needs zones, axial cells and dz > 0".into());
        }
        let x0 = match shape {
            StructureShape::Cylinder { inner_radius, multiplicity } => {
                let r0 = inner_radius.get::<meter>();
                if !(r0.is_finite() && r0 >= 0.0 && multiplicity.is_finite() && multiplicity > 0.0) {
                    return bad(format!("cylinder inner radius {r0} m, multiplicity {multiplicity}"));
                }
                r0
            }
            StructureShape::Slab { width } => {
                let w = width.get::<meter>();
                if !(w.is_finite() && w > 0.0) {
                    return bad(format!("slab width {w} m"));
                }
                0.0
            }
        };
        let (mut faces, mut k, mut rho_c) = (vec![x0], Vec::new(), Vec::new());
        for z in zones {
            let (th, kk, rc) = (
                z.thickness.get::<meter>(),
                z.conductivity.get::<watt_per_meter_kelvin>(),
                z.volumetric_heat_capacity.get::<joule_per_cubic_meter_kelvin>(),
            );
            if z.n_cells == 0 || ![th, kk, rc].iter().all(|v| v.is_finite() && *v > 0.0) {
                return bad(format!("invalid zone {z:?}"));
            }
            let start = *faces.last().unwrap_or(&x0);
            for c in 1..=z.n_cells {
                faces.push(start + th * c as f64 / z.n_cells as f64);
                k.push(kk);
                rho_c.push(rc);
            }
        }
        let t0 = initial.get::<kelvin>();
        if !(t0.is_finite() && t0 > 0.0) {
            return bad(format!("initial temperature {t0} K"));
        }
        let n = k.len();
        Ok(Self { shape, faces, k, rho_c, n_axial, dz, t: vec![t0; n * n_axial], q: vec![0.0; n * n_axial] })
    }

    /// Cells across the thickness.
    pub fn n_cells(&self) -> usize {
        self.k.len()
    }

    /// Axial cells.
    pub fn n_axial(&self) -> usize {
        self.n_axial
    }

    /// Temperature of cell `i` (across) at axial cell `j`.
    pub fn temperature(&self, j: usize, i: usize) -> Option<ThermodynamicTemperature> {
        (j < self.n_axial && i < self.n_cells())
            .then(|| ThermodynamicTemperature::new::<kelvin>(self.t[j * self.n_cells() + i]))
    }

    /// Set a uniform volumetric source in the cells of zone-range
    /// `cells` (indices across the thickness) at axial cell `j`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for an out-of-range index or a
    /// non-finite source.
    pub fn set_source(
        &mut self,
        j: usize,
        cells: core::ops::Range<usize>,
        q: VolumetricPowerDensity,
    ) -> Result<(), TampinesError> {
        let v = q.get::<watt_per_cubic_meter>();
        let n = self.n_cells();
        if j >= self.n_axial || cells.end > n || !v.is_finite() {
            return Err(TampinesError::InvalidInput(format!("source at axial {j}, cells {cells:?}, {v} W/m3")));
        }
        for i in cells {
            self.q[j * n + i] = v;
        }
        Ok(())
    }

    /// Set the sources of cells `cells` at axial cell `j` so they hold the
    /// total power `p` (uniform density over their volume).
    ///
    /// # Errors
    ///
    /// As [`Self::set_source`].
    pub fn set_power(&mut self, j: usize, cells: core::ops::Range<usize>, p: Power) -> Result<(), TampinesError> {
        let vol: f64 = cells.clone().map(|i| self.volume(i)).sum();
        if !(vol > 0.0) {
            return Err(TampinesError::InvalidInput(format!("no volume in cells {cells:?}")));
        }
        self.set_source(j, cells, VolumetricPowerDensity::new::<watt_per_cubic_meter>(p.get::<watt>() / vol))
    }

    /// Volume of cell `i` at one axial cell, m³ (all rods).
    fn volume(&self, i: usize) -> f64 {
        let (a, b) = (self.faces[i], self.faces[i + 1]);
        match self.shape {
            StructureShape::Cylinder { multiplicity, .. } => multiplicity * PI * (b * b - a * a) * self.dz,
            StructureShape::Slab { width } => (b - a) * width.get::<meter>() * self.dz,
        }
    }

    /// Area of face `f` at one axial cell, m² (all rods).
    fn face_area(&self, f: usize) -> f64 {
        match self.shape {
            StructureShape::Cylinder { multiplicity, .. } => multiplicity * 2.0 * PI * self.faces[f] * self.dz,
            StructureShape::Slab { width } => width.get::<meter>() * self.dz,
        }
    }

    /// Area of a surface at one axial cell.
    pub fn surface_area(&self, side: StructureSide) -> uom::si::f64::Area {
        let f = match side {
            StructureSide::Inner => 0,
            StructureSide::Outer => self.n_cells(),
        };
        uom::si::f64::Area::new::<uom::si::area::square_meter>(self.face_area(f))
    }

    /// Thermal resistance from the centre of cell `i` to face `f` (its own
    /// inner face `i` or outer face `i + 1`), K/W, all rods.
    fn half_resistance(&self, i: usize, f: usize) -> f64 {
        let (a, b) = (self.faces[i], self.faces[i + 1]);
        match self.shape {
            StructureShape::Cylinder { multiplicity, .. } => {
                // The cell's area-weighted centre, `r_c² = (a² + b²)/2`, so a
                // solid centre cell (a = 0) has a finite outer resistance.
                let rc = (0.5 * (a * a + b * b)).sqrt();
                let rf = self.faces[f];
                (rf / rc).r_ln().abs() / (2.0 * PI * self.k[i] * self.dz * multiplicity)
            }
            StructureShape::Slab { width } => 0.5 * (b - a) / (self.k[i] * width.get::<meter>() * self.dz),
        }
    }

    /// Stored energy `Σ ρc V T`, J (relative to 0 K).
    pub fn stored_energy(&self) -> f64 {
        let n = self.n_cells();
        (0..self.n_axial)
            .map(|j| (0..n).map(|i| self.rho_c[i] * self.volume(i) * self.t[j * n + i]).sum::<f64>())
            .sum()
    }

    /// Total source power, W.
    pub fn source_power(&self) -> f64 {
        let n = self.n_cells();
        (0..self.n_axial)
            .map(|j| (0..n).map(|i| self.q[j * n + i] * self.volume(i)).sum::<f64>())
            .sum()
    }

    /// Heat leaving `side` at axial cell `j` into a fluid at `t_fluid` with
    /// coefficient `h`, W: `(T_c − T_f)/(R_half + 1/(h A))` with `T_c` the
    /// surface cell's temperature.
    pub fn heat_to_fluid(&self, side: StructureSide, j: usize, t_fluid: f64, h: f64) -> f64 {
        let n = self.n_cells();
        let (i, f) = match side {
            StructureSide::Inner => (0, 0),
            StructureSide::Outer => (n - 1, n),
        };
        let a = self.face_area(f);
        if a <= 0.0 || h <= 0.0 {
            return 0.0;
        }
        (self.t[j * n + i] - t_fluid) / (self.half_resistance(i, f) + 1.0 / (h * a))
    }

    /// Advance `dt` by backward Euler with these boundary conditions.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt`, a
    /// `HeatInflow` of the wrong length, or a non-adiabatic inner side on a
    /// solid rod; the tridiagonal solver's errors.
    pub fn step(
        &mut self,
        dt: Time,
        inner: &StructureBoundary,
        outer: &StructureBoundary,
    ) -> Result<(), TampinesError> {
        let dt = dt.get::<second>();
        if !(dt.is_finite() && dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!("dt {dt} s")));
        }
        let solid_rod = matches!(self.shape, StructureShape::Cylinder { .. }) && self.faces[0] == 0.0;
        if solid_rod && *inner != StructureBoundary::Adiabatic {
            return Err(TampinesError::InvalidInput("a solid rod's axis is adiabatic".into()));
        }
        for b in [inner, outer] {
            if let StructureBoundary::HeatInflow(v) = b {
                if v.len() != self.n_axial {
                    return Err(TampinesError::InvalidInput(format!("{} heat inflows for {} cells", v.len(), self.n_axial)));
                }
            }
        }
        let n = self.n_cells();
        // Inter-cell conductances, W/K.
        let g: Vec<f64> = (0..n.saturating_sub(1))
            .map(|i| 1.0 / (self.half_resistance(i, i + 1) + self.half_resistance(i + 1, i + 1)))
            .collect();
        let cap: Vec<f64> = (0..n).map(|i| self.rho_c[i] * self.volume(i) / dt).collect();
        // A boundary as (conductance to a fixed temperature, that
        // temperature, prescribed inflow), for cell `i` through face `f`.
        let bc = |b: &StructureBoundary, i: usize, f: usize, j: usize| -> (f64, f64, f64) {
            match b {
                StructureBoundary::Adiabatic => (0.0, 0.0, 0.0),
                StructureBoundary::Temperature(t) => (1.0 / self.half_resistance(i, f), t.get::<kelvin>(), 0.0),
                StructureBoundary::Convective { h, ambient } => {
                    let a = self.face_area(f);
                    let hh = h.get::<watt_per_square_meter_kelvin>();
                    if a <= 0.0 || hh <= 0.0 {
                        (0.0, 0.0, 0.0)
                    } else {
                        (1.0 / (self.half_resistance(i, f) + 1.0 / (hh * a)), ambient.get::<kelvin>(), 0.0)
                    }
                }
                StructureBoundary::HeatInflow(v) => (0.0, 0.0, v[j]),
            }
        };
        let mut new_t = self.t.clone();
        for j in 0..self.n_axial {
            let (gi, ti, qi) = bc(inner, 0, 0, j);
            let (go, to, qo) = bc(outer, n - 1, n, j);
            let (mut a, mut b, mut c, mut d) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
            for i in 0..n {
                b[i] = cap[i];
                d[i] = cap[i] * self.t[j * n + i] + self.q[j * n + i] * self.volume(i);
                if i > 0 {
                    a[i] = -g[i - 1];
                    b[i] += g[i - 1];
                }
                if i + 1 < n {
                    c[i] = -g[i];
                    b[i] += g[i];
                }
            }
            b[0] += gi;
            d[0] += gi * ti + qi;
            b[n - 1] += go;
            d[n - 1] += go * to + qo;
            let x = thomas_solve(&a, &b, &c, &d)?;
            new_t[j * n..(j + 1) * n].copy_from_slice(&x);
        }
        self.t = new_t;
        Ok(())
    }
}

/// What one side of a coupled structure sees.
#[derive(Debug, Clone, PartialEq)]
pub enum SideCondition {
    /// A boundary independent of the network.
    Boundary(StructureBoundary),
    /// The fluid of network pipe `pipe`, cell for cell, with coefficient `h`.
    Pipe {
        /// Pipe index in the network.
        pipe: usize,
        /// Heat-transfer coefficient (constant; see the module limits).
        h: HeatTransfer,
    },
}

/// A heat structure and what each side of it sees: a PipeWithHeatStructure
/// (one side on a pipe), a CoreChannel (a fuel rod with a source, its outer
/// side on the coolant), or a heat-exchanger wall (each side on a pipe).
#[derive(Debug, Clone, PartialEq)]
pub struct HeatStructureLink {
    /// The structure.
    pub structure: HeatStructure,
    /// Its inner side.
    pub inner: SideCondition,
    /// Its outer side.
    pub outer: SideCondition,
}

/// Couples heat structures to the pipes of a [`HemNetwork`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThermalCoupler {
    links: Vec<HeatStructureLink>,
    /// Heat given to the fluid since construction, J.
    heat_to_fluid_j: f64,
}

impl ThermalCoupler {
    /// An empty coupler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a link; its pipes must have as many cells as the structure has
    /// axial cells.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a missing pipe or a cell-count
    /// mismatch.
    pub fn add<E: HemEquationOfState>(
        &mut self,
        net: &HemNetwork<E>,
        link: HeatStructureLink,
    ) -> Result<usize, TampinesError> {
        for side in [&link.inner, &link.outer] {
            if let SideCondition::Pipe { pipe, .. } = side {
                let p = net
                    .pipe(*pipe)
                    .ok_or_else(|| TampinesError::InvalidInput(format!("no pipe {pipe}")))?;
                if p.geometry().n_cells() != link.structure.n_axial() {
                    return Err(TampinesError::InvalidInput(format!(
                        "pipe {pipe} has {} cells, the structure {}",
                        p.geometry().n_cells(),
                        link.structure.n_axial()
                    )));
                }
            }
        }
        self.links.push(link);
        Ok(self.links.len() - 1)
    }

    /// The links.
    pub fn link(&self, index: usize) -> Option<&HeatStructureLink> {
        self.links.get(index)
    }

    /// The links, mutably (to set sources between steps).
    pub fn link_mut(&mut self, index: usize) -> Option<&mut HeatStructureLink> {
        self.links.get_mut(index)
    }

    /// Heat given to the fluid since construction.
    pub fn heat_to_fluid(&self) -> uom::si::f64::Energy {
        uom::si::f64::Energy::new::<uom::si::energy::joule>(self.heat_to_fluid_j)
    }

    /// Stored energy of every structure, J.
    pub fn stored_energy(&self) -> f64 {
        self.links.iter().map(|l| l.structure.stored_energy()).sum()
    }

    /// One coupled step: set the pipes' heat input from the walls, step the
    /// network, then advance every structure the same `dt` with that heat
    /// leaving it. Returns `dt`.
    ///
    /// # Errors
    ///
    /// The network's and the structures' errors.
    pub fn step<E: HemEquationOfState>(&mut self, net: &mut HemNetwork<E>) -> Result<Time, TampinesError> {
        // 1. The heat each pipe side receives, per axial cell.
        let mut per_pipe: Vec<(usize, Vec<f64>)> = Vec::new();
        let mut side_heat: Vec<[Option<Vec<f64>>; 2]> = Vec::with_capacity(self.links.len());
        for l in &self.links {
            let mut sides: [Option<Vec<f64>>; 2] = [None, None];
            for (k, (cond, side)) in [(&l.inner, StructureSide::Inner), (&l.outer, StructureSide::Outer)]
                .into_iter()
                .enumerate()
            {
                if let SideCondition::Pipe { pipe, h } = cond {
                    let p: &HemPipe<E> = net
                        .pipe(*pipe)
                        .ok_or_else(|| TampinesError::InvalidInput(format!("no pipe {pipe}")))?;
                    let hh = h.get::<watt_per_square_meter_kelvin>();
                    let mut q = Vec::with_capacity(l.structure.n_axial());
                    for j in 0..l.structure.n_axial() {
                        let tf = p.cell_state(j)?.temperature.get::<kelvin>();
                        q.push(l.structure.heat_to_fluid(side, j, tf, hh));
                    }
                    match per_pipe.iter_mut().find(|(i, _)| i == pipe) {
                        Some((_, acc)) => acc.iter_mut().zip(&q).for_each(|(a, b)| *a += b),
                        None => per_pipe.push((*pipe, q.clone())),
                    }
                    sides[k] = Some(q);
                }
            }
            side_heat.push(sides);
        }
        for (pipe, q) in &per_pipe {
            let powers: Vec<Power> = q.iter().map(|w| Power::new::<watt>(*w)).collect();
            if let Some(p) = net.pipe_mut(*pipe) {
                p.set_cell_heat_input(&powers)?;
            }
        }
        // 2. The network.
        let dt = net.step()?;
        // 3. The structures, with the same heat leaving them.
        for (l, sides) in self.links.iter_mut().zip(side_heat) {
            let bc = |cond: &SideCondition, q: &Option<Vec<f64>>| match (cond, q) {
                (SideCondition::Boundary(b), _) => b.clone(),
                (SideCondition::Pipe { .. }, Some(q)) => StructureBoundary::HeatInflow(q.iter().map(|w| -w).collect()),
                (SideCondition::Pipe { .. }, None) => StructureBoundary::Adiabatic,
            };
            let (bi, bo) = (bc(&l.inner, &sides[0]), bc(&l.outer, &sides[1]));
            l.structure.step(dt, &bi, &bo)?;
            for q in sides.iter().flatten() {
                self.heat_to_fluid_j += q.iter().sum::<f64>() * dt.get::<second>();
            }
        }
        Ok(dt)
    }
}
