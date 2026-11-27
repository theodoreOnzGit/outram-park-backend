// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The radial-conduction structure power models against GeN-Foam's own classes (gh:#1230, round 35)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506. `upstream_driver/power_model_dump/powerModelDump.C`
//! constructs the real `structure` (hence the real `powerModel` the case selects: `lumpedNuclearStructure`,
//! `heatedPin`, `nuclearFuelPin`, `interpolatedNuclearFuelPin`, from the UNMODIFIED `libFFSSrc2`) on an 8-cell
//! 1-D mesh with three cell zones and one unzoned cell (`scripts/make_power_model_case.sh`), writes
//! `powerDensityNeutronics`, `H` and `HT` from closed-form functions of cell, step and outer iteration that use
//! only `+ - * /` (evaluated below in the same order), calls `structure::correct` and prints every state at 17
//! digits (`scripts/build_and_run_power_model_dump.sh`; 12 steps with one outer iteration, 3 steps with two).
//! Materials and geometry are those of the shipped tutorials' `powerModel` dictionaries (FFTF / one-phase
//! point-kinetics pins; the `lumpedNuclearStructure` usage example), repeated below from the script.
//! The `fvc::laplacian(Tmatrix)` of the lumped model is a mesh operator: the model takes it as input and this
//! test feeds it the value the real binary dumped (`laplacianTmatrix.lumpedNuclearStructure`).
//!
//! **Pass criterion: bit identity** (`to_bits` equal) for every dumped quantity of every step; a residual would
//! need the 1e-9 allowance with an explained cause (none is used). **Controls** (each must fail): the
//! `lu_decompose` solve, the start-of-step old time, the corrected ring areas, a 1e-9 perturbation of `gapH`.
//! Predictions (committed before the first run) and results: README, "Round 35".

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::TimeProfile;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::heated_pin::{HeatedPin, HeatedPinRegion};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::interpolated_nuclear_fuel_pin::{
    InterpolatedNuclearFuelPin, InterpolatedRegion,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::lumped_nuclear_structure::{
    LumpedNuclearStructure, LumpedRegion,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::nuclear_fuel_pin::{
    GapConductance, NuclearFuelPin, NuclearFuelPinRegion,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_power_model::{
    structure_heat_flux, RadialPowerModel, StepInputs,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_solve::OldTimePolicy;
use uom::si::f64::Time;
use uom::si::time::second;

const N_CELLS: usize = 8;
const Q0: f64 = 2.0e8;
const H0: f64 = 1.0e4;
const TF0: f64 = 600.0;
const DT: f64 = 0.5;

fn refs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

/// The parsed reference: `S` (volScalarField over all cells), `R` (profile of one cell), `D` (dictionary scalar).
struct Reference {
    n_steps: i64,
    n_outer: i64,
    s: HashMap<(i64, i64, String), Vec<f64>>,
    r: HashMap<(i64, i64, String, usize), Vec<f64>>,
    d: HashMap<(i64, i64, String), f64>,
}

fn load(model: &str, n_outer: i64) -> Reference {
    let text = String::from_utf8(gunzip(&refs_dir().join(format!("pmd_{model}_o{n_outer}.ref.txt.gz")))).unwrap();
    let mut me = Reference { n_steps: 0, n_outer: 0, s: HashMap::new(), r: HashMap::new(), d: HashMap::new() };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("H") => {
                let w: Vec<&str> = it.collect();
                // H deltaT <dt> nCells <n> nSteps <n> nOuter <n>
                assert_eq!(w[1].parse::<f64>().unwrap(), DT, "deltaT of the reference");
                assert_eq!(w[3].parse::<usize>().unwrap(), N_CELLS);
                me.n_steps = w[5].parse().unwrap();
                me.n_outer = w[7].parse().unwrap();
            }
            Some("S") => {
                let step: i64 = it.next().unwrap().parse().unwrap();
                let outer: i64 = it.next().unwrap().parse().unwrap();
                let name = it.next().unwrap().to_string();
                me.s.insert((step, outer, name), it.map(|x| x.parse().unwrap()).collect());
            }
            Some("R") => {
                let step: i64 = it.next().unwrap().parse().unwrap();
                let outer: i64 = it.next().unwrap().parse().unwrap();
                let name = it.next().unwrap().to_string();
                let cell: usize = it.next().unwrap().parse().unwrap();
                me.r.insert((step, outer, name, cell), it.map(|x| x.parse().unwrap()).collect());
            }
            Some("D") => {
                let step: i64 = it.next().unwrap().parse().unwrap();
                let outer: i64 = it.next().unwrap().parse().unwrap();
                let ty = it.next().unwrap();
                let name = it.next().unwrap();
                me.d.insert((step, outer, format!("{ty}.{name}")), it.next().unwrap().parse().unwrap());
            }
            _ => {}
        }
    }
    assert_eq!(me.n_outer, n_outer);
    me
}

/// Bit-identity tally of one comparison run.
#[derive(Default, Debug)]
struct Tally {
    n: usize,
    exact: usize,
    worst_rel: f64,
    worst_at: String,
    /// worst relative difference excluding `heatFlux` (`H Twall - HT`, a difference of two near-equal numbers that
    /// amplifies a round-off in `Twall` by `Twall/(Twall - Tf)`): the temperatures themselves
    worst_temperature_rel: f64,
    first_diff: Option<String>,
}

impl Tally {
    fn cmp(&mut self, got: f64, want: f64, at: &dyn Fn() -> String) {
        self.n += 1;
        if got.to_bits() == want.to_bits() {
            self.exact += 1;
            return;
        }
        let rel = (got - want).abs() / want.abs().max(f64::MIN_POSITIVE);
        let here = at();
        if !here.contains("heatFlux") && rel > self.worst_temperature_rel {
            self.worst_temperature_rel = rel;
        }
        if rel > self.worst_rel {
            self.worst_rel = rel;
            self.worst_at = here.clone();
        }
        if self.first_diff.is_none() {
            self.first_diff = Some(here);
        }
    }

    fn report(&self, label: &str) {
        println!(
            "{label}: {} values, {} bit-identical, worst relative {:.3e} (temperatures and profiles only: {:.3e}) at [{}]; first differing [{}]",
            self.n,
            self.exact,
            self.worst_rel,
            self.worst_temperature_rel,
            self.worst_at,
            self.first_diff.clone().unwrap_or_default()
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Lumped,
    Heated,
    Fuel,
    Interpolated,
}

impl Kind {
    fn file(self) -> &'static str {
        match self {
            Self::Lumped => "lumpedNuclearStructure",
            Self::Heated => "heatedPin",
            Self::Fuel => "nuclearFuelPin",
            Self::Interpolated => "interpolatedNuclearFuelPin",
        }
    }
}

#[derive(Clone, Copy)]
struct Opts {
    lu: bool,
    old_time: OldTimePolicy,
    corrected_areas: bool,
    gap_scale: f64,
}

const DEFAULT: Opts = Opts { lu: false, old_time: OldTimePolicy::UpstreamQuirk, corrected_areas: false, gap_scale: 1.0 };

/// The structure's `alpha` per cell (the zones' `volumeFraction`s of the case script).
fn structure_alpha(kind: Kind) -> Vec<f64> {
    let mut a = vec![0.0; N_CELLS];
    let zones: &[(&[usize], f64)] = match kind {
        Kind::Lumped => &[(&[0, 1, 2], 0.4), (&[3, 4], 0.3), (&[5, 6], 0.5)],
        Kind::Heated | Kind::Interpolated => &[(&[0, 1, 2], 0.4), (&[3, 4], 0.3)],
        Kind::Fuel => &[(&[0, 1, 2], 0.5539729557), (&[3, 4], 0.45), (&[5, 6], 0.5)],
    };
    for (cells, v) in zones {
        for &c in *cells {
            a[c] = *v;
        }
    }
    a
}

fn build(kind: Kind, o: Opts) -> RadialPowerModel {
    let alpha = structure_alpha(kind);
    let vol = vec![1.0; N_CELLS];
    match kind {
        Kind::Lumped => {
            let regions = vec![
                LumpedRegion {
                    cells: vec![0, 1, 2],
                    volumetric_area: 40.0,
                    fraction_of_power: 1.0,
                    nodes_number: 3,
                    node_fuel: 0,
                    node_clad: 1,
                    node_matrix: 2,
                    kappa_matrix: 12.0,
                    heat_conductances: vec![600000.0, 240000.0, 600000.0, 1200000.0],
                    rho_cp: vec![53576.92, 53576.92, 53576.92],
                    volume_fractions: vec![0.75, 0.125, 0.125],
                    power_fractions: vec![1.0, 0.0, 0.0],
                    t0: 900.0,
                    volume_fraction: None,
                },
                LumpedRegion {
                    cells: vec![3, 4],
                    volumetric_area: 55.0,
                    fraction_of_power: 0.9,
                    nodes_number: 1,
                    node_fuel: 0,
                    node_clad: 0,
                    node_matrix: 0,
                    kappa_matrix: 10.0,
                    heat_conductances: vec![450000.0, 225000.0],
                    rho_cp: vec![60000.0],
                    volume_fractions: vec![1.0],
                    power_fractions: vec![1.0],
                    t0: 850.0,
                    volume_fraction: None,
                },
                LumpedRegion {
                    cells: vec![5, 6],
                    volumetric_area: 30.0,
                    fraction_of_power: 1.0,
                    nodes_number: 2,
                    node_fuel: 0,
                    node_clad: 1,
                    node_matrix: 0,
                    kappa_matrix: 0.0,
                    heat_conductances: vec![500000.0, 300000.0, 700000.0],
                    rho_cp: vec![50000.0, 70000.0],
                    volume_fractions: vec![0.6, 0.4],
                    power_fractions: vec![0.8, 0.2],
                    t0: 700.0,
                    volume_fraction: None,
                },
            ];
            RadialPowerModel::LumpedNuclearStructure(
                LumpedNuclearStructure::new(N_CELLS, regions, &alpha, 0).unwrap().with_old_time_policy(o.old_time),
            )
        }
        Kind::Heated => {
            let regions = vec![
                HeatedPinRegion {
                    cells: vec![0, 1, 2],
                    inner_radius: 0.0,
                    outer_radius: 0.0045,
                    mesh_size: 12,
                    rho_cp: 7800.0 * 500.0,
                    k: 20.0,
                    initial_temperature: 600.0,
                    power_density: 3e8,
                    volume_fraction: None,
                },
                HeatedPinRegion {
                    cells: vec![3, 4],
                    inner_radius: 0.001,
                    outer_radius: 0.005,
                    mesh_size: 9,
                    rho_cp: 3.9e6,
                    k: 15.0,
                    initial_temperature: 650.0,
                    power_density: 1e8,
                    volume_fraction: Some(0.25),
                },
            ];
            let mut m = HeatedPin::new(N_CELLS, regions, &alpha, &vol, 0).unwrap().with_old_time_policy(o.old_time);
            if o.lu {
                m = m.with_lu_control_solver();
            }
            RadialPowerModel::HeatedPin(m)
        }
        Kind::Fuel => {
            let gap_table = TimeProfile::table(
                vec![Time::new::<second>(0.0), Time::new::<second>(795443280.75916)],
                vec![528.4782467867, 3170.8694807204],
            )
            .unwrap();
            let ffft = |cells: Vec<usize>, nf: usize, nc: usize, gap: GapConductance| NuclearFuelPinRegion {
                cells,
                fraction_of_power: 1.0,
                rfi: 0.0,
                rfo: 0.002470,
                rci: 0.002540,
                rco: 0.002921,
                fuel_mesh_size: nf,
                clad_mesh_size: nc,
                rho_cp_fuel: 10480.0 * 250.0,
                rho_cp_clad: 7870.0 * 500.0,
                k_fuel: 3.0,
                k_clad: 20.0,
                fuel_t0: 574.56,
                clad_t0: 574.56,
                gap,
                volume_fraction: Some(0.5539729557),
            };
            let regions = vec![
                ffft(vec![0, 1, 2], 10, 5, GapConductance::Constant(3000.0 * o.gap_scale)),
                NuclearFuelPinRegion {
                    cells: vec![3, 4],
                    fraction_of_power: 0.95,
                    rfi: 0.0012,
                    rfo: 0.004715,
                    rci: 0.004865,
                    rco: 0.005365,
                    fuel_mesh_size: 16,
                    clad_mesh_size: 4,
                    rho_cp_fuel: 2.62e6,
                    rho_cp_clad: 7500.0 * 500.0,
                    k_fuel: 3.0,
                    k_clad: 20.0,
                    fuel_t0: 600.0,
                    clad_t0: 590.0,
                    gap: GapConductance::Constant(3000.0 * o.gap_scale),
                    volume_fraction: None,
                },
                ffft(vec![5, 6], 8, 3, GapConductance::TableOfPowerDensity(gap_table)),
            ];
            let mut m = if o.corrected_areas {
                NuclearFuelPin::with_corrected_ring_areas(N_CELLS, regions, &alpha, &vol, 0).unwrap()
            } else {
                NuclearFuelPin::new(N_CELLS, regions, &alpha, &vol, 0).unwrap()
            };
            m = m.with_old_time_policy(o.old_time);
            if o.lu {
                m = m.with_lu_control_solver();
            }
            RadialPowerModel::NuclearFuelPin(m)
        }
        Kind::Interpolated => {
            let regions = vec![
                InterpolatedRegion { cells: vec![0, 1, 2], fuel_outer_radius: 0.0047, t0: 700.0, volume_fraction: None },
                InterpolatedRegion { cells: vec![3, 4], fuel_outer_radius: 0.0052, t0: 650.0, volume_fraction: None },
            ];
            RadialPowerModel::InterpolatedNuclearFuelPin(InterpolatedNuclearFuelPin::new(N_CELLS, &regions, &alpha).unwrap())
        }
    }
}

/// The closed-form coupling of the driver, in the driver's operation order.
fn coupling(step: i64, outer: i64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = step as f64;
    let kk = outer as f64;
    let (mut q, mut h, mut ht) = (vec![0.0; N_CELLS], vec![0.0; N_CELLS], vec![0.0; N_CELLS]);
    for c in 0..N_CELLS {
        let cc = c as f64;
        q[c] = Q0 * (1.0 + 0.05 * cc) * (1.0 + 0.1 * n);
        let hh = H0 * (1.0 + 0.04 * cc) * (1.0 + 0.02 * n);
        let tf = TF0 + 3.0 * cc + 5.0 * n + 0.5 * kk;
        h[c] = hh;
        ht[c] = hh * tf;
    }
    (q, h, ht)
}

fn run(kind: Kind, n_outer: i64, o: Opts) -> Tally {
    let r = load(kind.file(), n_outer);
    let mut m = build(kind, o);
    let mut t = Tally::default();
    let ty = kind.file();
    let vol = vec![1.0; N_CELLS];
    for step in 1..=r.n_steps {
        for outer in 0..n_outer {
            let (q, h, ht) = coupling(step, outer);
            let mut lap = vec![0.0; N_CELLS];
            if kind == Kind::Lumped {
                lap = r.s[&(step, outer, "laplacianTmatrix.lumpedNuclearStructure".to_string())].clone();
            }
            if let RadialPowerModel::InterpolatedNuclearFuelPin(p) = &mut m {
                let ihf: Vec<f64> = (0..N_CELLS)
                    .map(|c| 1.0e4 * (1.0 + 0.1 * (c as f64)) * (1.0 + 0.05 * (step as f64)))
                    .collect();
                p.set_heat_flux(&ihf);
            }
            let inp = StepInputs { dt: DT, time_index: step, q_neutronics: q, laplacian_t_matrix: lap, volumes: vol.clone() };
            m.correct(&ht, &h, &inp).unwrap();
            let sv = |name: &str| -> &Vec<f64> {
                r.s.get(&(step, outer, name.to_string())).unwrap_or_else(|| panic!("no S {name} at {step} {outer}"))
            };
            let cells: Vec<usize> = m.cells().to_vec();
            let chk = |t: &mut Tally, name: &str, cell: usize, got: f64, want: f64| {
                t.cmp(got, want, &|| format!("{ty} {name} step {step} outer {outer} cell {cell}"));
            };
            for &c in &cells {
                let tact = sv("Tact")[c];
                chk(&mut t, "Tact", c, m.surface_temperature(c), tact);
                chk(&mut t, "Twall", c, m.surface_temperature(c), sv("Twall")[c]);
                chk(&mut t, "heatFlux", c, structure_heat_flux(h[c], m.surface_temperature(c), ht[c]), sv("heatFlux")[c]);
                let (tf, tc) = m.coupling_temperatures(c);
                chk(&mut t, "TFuelAv", c, tf, sv("TFuelAv")[c]);
                chk(&mut t, "TCladAv", c, tc, sv("TCladAv")[c]);
                chk(&mut t, "iA", c, m.interfacial_area(c), sv(&format!("iA.{ty}"))[c]);
            }
            match &m {
                RadialPowerModel::LumpedNuclearStructure(p) => {
                    for &c in &cells {
                        let want = &r.r[&(step, outer, format!("T.{ty}"), c)];
                        assert_eq!(p.nodes(c).len(), want.len());
                        for (j, (g, w)) in p.nodes(c).iter().zip(want).enumerate() {
                            chk(&mut t, &format!("T[{j}]"), c, *g, *w);
                        }
                        chk(&mut t, "Tmax", c, p.tmax(c), sv(&format!("Tmax.{ty}"))[c]);
                        chk(&mut t, "Tsurface", c, p.surface_temperature(c), sv(&format!("Tsurface.{ty}"))[c]);
                    }
                    for c in 0..N_CELLS {
                        chk(&mut t, "Tmatrix", c, p.tmatrix()[c], sv(&format!("Tmatrix.{ty}"))[c]);
                    }
                }
                RadialPowerModel::HeatedPin(p) => {
                    for &c in &cells {
                        let want = &r.r[&(step, outer, format!("Trad.{ty}"), c)];
                        assert_eq!(p.trad(c).len(), want.len());
                        for (j, (g, w)) in p.trad(c).iter().zip(want).enumerate() {
                            chk(&mut t, &format!("Trad[{j}]"), c, *g, *w);
                        }
                        chk(&mut t, "Ti", c, p.ti(c), sv(&format!("Ti.{ty}"))[c]);
                        chk(&mut t, "To", c, p.to(c), sv(&format!("To.{ty}"))[c]);
                        chk(&mut t, "Tav", c, p.tav(c), sv(&format!("Tav.{ty}"))[c]);
                    }
                    chk(&mut t, "Tavav", 0, p.t_avav(), r.d[&(step, outer, format!("{ty}.Tavav"))]);
                    chk(&mut t, "Tmax", 0, p.t_max(), r.d[&(step, outer, format!("{ty}.Tmax"))]);
                    chk(&mut t, "Tmin", 0, p.t_min(), r.d[&(step, outer, format!("{ty}.Tmin"))]);
                }
                RadialPowerModel::NuclearFuelPin(p) => {
                    for &c in &cells {
                        let want = &r.r[&(step, outer, format!("Trad.{ty}"), c)];
                        assert_eq!(p.trad(c).len(), want.len());
                        for (j, (g, w)) in p.trad(c).iter().zip(want).enumerate() {
                            chk(&mut t, &format!("Trad[{j}]"), c, *g, *w);
                        }
                        let (tfi, tfo, tci, tco) = p.boundary_temperatures(c);
                        chk(&mut t, "Tfi", c, tfi, sv(&format!("Tfi.{ty}"))[c]);
                        chk(&mut t, "Tfo", c, tfo, sv(&format!("Tfo.{ty}"))[c]);
                        chk(&mut t, "Tci", c, tci, sv(&format!("Tci.{ty}"))[c]);
                        chk(&mut t, "Tco", c, tco, sv(&format!("Tco.{ty}"))[c]);
                        let (tfav, tcav) = p.averages(c);
                        chk(&mut t, "Tfav", c, tfav, sv(&format!("Tfav.{ty}"))[c]);
                        chk(&mut t, "Tcav", c, tcav, sv(&format!("Tcav.{ty}"))[c]);
                    }
                    let (tfavav, tcavav, tfmax, tfmin, tcmax, tcmin) = p.global_values();
                    for (nm, v) in [("Tfavav", tfavav), ("Tcavav", tcavav), ("Tfmax", tfmax), ("Tfmin", tfmin), ("Tcmax", tcmax), ("Tcmin", tcmin)] {
                        chk(&mut t, nm, 0, v, r.d[&(step, outer, format!("{ty}.{nm}"))]);
                    }
                }
                RadialPowerModel::InterpolatedNuclearFuelPin(p) => {
                    for &c in &cells {
                        chk(&mut t, "T", c, p.surface_temperature(c), sv(&format!("T.{ty}"))[c]);
                        chk(&mut t, "interpolatedPowerDensity", c, p.power_density(c), sv("interpolatedPowerDensity")[c]);
                    }
                }
            }
        }
    }
    t.report(&format!("{} nOuter {} steps {}", kind.file(), n_outer, r.n_steps));
    t
}

fn assert_bit_identical(t: &Tally) {
    assert!(t.n > 100, "compared {} values", t.n);
    assert_eq!(t.exact, t.n, "not bit-identical: worst {:.3e} at [{}], first [{}]", t.worst_rel, t.worst_at, t.first_diff.clone().unwrap_or_default());
}

#[test]
fn lumped_nuclear_structure_is_bit_identical_to_upstream() {
    assert_bit_identical(&run(Kind::Lumped, 1, DEFAULT));
    assert_bit_identical(&run(Kind::Lumped, 2, DEFAULT));
}

#[test]
fn heated_pin_is_bit_identical_to_upstream() {
    assert_bit_identical(&run(Kind::Heated, 1, DEFAULT));
    assert_bit_identical(&run(Kind::Heated, 2, DEFAULT));
}

#[test]
fn nuclear_fuel_pin_is_bit_identical_to_upstream() {
    assert_bit_identical(&run(Kind::Fuel, 1, DEFAULT));
    assert_bit_identical(&run(Kind::Fuel, 2, DEFAULT));
}

#[test]
fn interpolated_nuclear_fuel_pin_is_bit_identical_to_upstream() {
    assert_bit_identical(&run(Kind::Interpolated, 1, DEFAULT));
}

/// Control 1: a different elimination (workspace `lu_decompose`) is NOT bit-identical, though it solves the same system.
#[test]
fn control_lu_solver_is_not_bit_identical() {
    for kind in [Kind::Heated, Kind::Fuel] {
        let t = run(kind, 1, Opts { lu: true, ..DEFAULT });
        assert!(t.exact < t.n, "{kind:?}: the LU solve should differ somewhere");
        assert!(t.worst_rel < 1e-9, "{kind:?}: same physics, round-off only: {:.3e}", t.worst_rel);
    }
}

/// Control 2: the start-of-step old time differs from upstream exactly where `IOFieldField::oldTime()` is
/// quirky: the first cell, in the second outer iteration of the first step; and not with one outer iteration.
#[test]
fn control_old_time_policy_matters_only_with_two_outer_iterations() {
    for kind in [Kind::Heated, Kind::Fuel, Kind::Lumped] {
        let o = Opts { old_time: OldTimePolicy::StartOfStep, ..DEFAULT };
        let one = run(kind, 1, o);
        assert_eq!(one.exact, one.n, "{kind:?}: one outer iteration has no quirk");
        let two = run(kind, 2, o);
        assert!(two.exact < two.n, "{kind:?}: the quirk is visible with two outer iterations");
        let first = two.first_diff.clone().unwrap();
        assert!(first.contains("step 1 outer 1 cell 0"), "{kind:?}: first difference at {first}");
    }
}

/// Control 3: upstream's two ring-area slips change the cladding in a transient.
#[test]
fn control_corrected_ring_areas_differ() {
    let t = run(Kind::Fuel, 1, Opts { corrected_areas: true, ..DEFAULT });
    assert!(t.exact < t.n && t.worst_rel > 1e-7, "worst {:.3e}", t.worst_rel);
}

/// Control 4: a 1e-9 relative change of `gapH` is seen (the comparison can see small input changes).
#[test]
fn control_gap_perturbation_is_detected() {
    let t = run(Kind::Fuel, 1, Opts { gap_scale: 1.0 + 1e-9, ..DEFAULT });
    assert!(t.exact < t.n && t.worst_rel > 1e-12, "worst {:.3e}", t.worst_rel);
}
