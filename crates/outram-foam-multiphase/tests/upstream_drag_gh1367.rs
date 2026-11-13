// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Two-fluid interfacial drag against ESI OpenFOAM-v2506 on a frozen field
//! (gh:#1367, row 2, bubbleColumn per force).
//!
//! # Method
//! `verification_and_validation/driver/dragDump.C` evaluates ESI's
//! `phasePair::Re()`, `dragModel::Ki()/K()`, `SchillerNaumann::CdRe()` and
//! `WenYu::CdRe()` expressions, transcribed verbatim, on OpenFOAM
//! `scalarField`/`vectorField` (v2506 field algebra, glibc), with the
//! `twoPhaseEulerFoam/laminar/bubbleColumn` constants. The `phasePair` needs
//! the unbuilt phaseSystem library, so the expression, not the class, is
//! shared (as in row 3). 4096 cells from a splitmix64 generator reproduced here
//! and checked bit for bit. The port is a real `TwoFluidSystem` on a 4096-cell
//! line mesh with the same per-cell `alpha_d`, `U_d`, `U_c`. The forces the
//! port lacks (virtual mass, lift, wall lubrication, turbulent dispersion
//! return `NotImplemented`) are asserted as gaps. Predictions F1-F5 and the
//! results are in `verification_and_validation/upstream_comparison_gh1367.md`.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind, Vector3};
use outram_foam_multiphase::two_fluid::{DragModel, InterfacialForce, Phase, TwoFluidSystem};
use petir::mathf::RealMath;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, Length, MassDensity};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;

const N: usize = 4096;
const RESIDUAL_RE: f64 = 1e-3;

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
}

struct Inputs {
    ad: Vec<f64>,
    ud: Vec<Vector3>,
    uc: Vec<Vector3>,
}

/// Inputs in the order the driver draws them: ad, Ud(3), Uc(3), s.
fn generate_inputs() -> Inputs {
    let mut g = SplitMix(0x9E37_79B9_7F4A_7C15);
    let (mut ad, mut ud, mut uc) = (vec![], vec![], vec![]);
    for _ in 0..N {
        let u0 = g.next();
        let d: Vec<f64> = (0..3).map(|_| g.next()).collect();
        let c: Vec<f64> = (0..3).map(|_| g.next()).collect();
        let s = g.next();
        ad.push(u0);
        ud.push(Vector3::new((2.0 * d[0] - 1.0) * 0.6 * s, (2.0 * d[1] - 1.0) * 0.6 * s, (2.0 * d[2] - 1.0) * 0.6 * s));
        uc.push(Vector3::new((2.0 * c[0] - 1.0) * 0.6 * s, (2.0 * c[1] - 1.0) * 0.6 * s, (2.0 * c[2] - 1.0) * 0.6 * s));
    }
    uc[0] = ud[0];
    ad[1] = 0.0;
    ad[2] = 1.0;
    ad[3] = 1e-9;
    Inputs { ad, ud, uc }
}

fn read_reference() -> BTreeMap<String, Vec<f64>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/reference/drag_v2506.txt.gz");
    let bytes = std::fs::read(path).expect("reference file");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (name, _idx, hex) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        out.entry(name.to_string()).or_default().push(f64::from_bits(u64::from_str_radix(hex, 16).unwrap()));
    }
    out
}

fn line_mesh(n: usize) -> Arc<FvMesh> {
    let vx = |x: f64| Vector3::new(x, 0.0, 0.0);
    let mut owner: Vec<usize> = (0..n - 1).collect();
    owner.push(0);
    owner.push(n - 1);
    let mut areas = vec![vx(1.0); n - 1];
    areas.push(vx(-1.0));
    areas.push(vx(1.0));
    let mut fc: Vec<Vector3> = (1..n).map(|i| vx(i as f64)).collect();
    fc.push(vx(0.0));
    fc.push(vx(n as f64));
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour((1..n).collect())
            .patches(vec![
                BoundaryPatch::new("inlet", n - 1, 1, PatchKind::Patch),
                BoundaryPatch::new("outlet", n, 1, PatchKind::Patch),
            ])
            .cell_volumes(vec![1.0; n])
            .cell_centres((0..n).map(|i| vx(i as f64 + 0.5)).collect())
            .face_area_vectors(areas)
            .face_centres(fc)
            .build()
            .unwrap(),
    )
}

/// Air (dispersed, d = 3e-3) in water (rho 1027, mu 3.645e-4): bubbleColumn.
fn port_system(inp: &Inputs) -> TwoFluidSystem {
    let mesh = line_mesh(N);
    let mut air = Phase::new(
        mesh.clone(),
        "air",
        MassDensity::new::<kilogram_per_cubic_meter>(1.0),
        DynamicViscosity::new::<pascal_second>(1.84e-5),
        Length::new::<meter>(3e-3),
        0.0,
    )
    .unwrap();
    let mut water = Phase::new(
        mesh,
        "water",
        MassDensity::new::<kilogram_per_cubic_meter>(1027.0),
        DynamicViscosity::new::<pascal_second>(3.645e-4),
        Length::new::<meter>(1e-4),
        1.0,
    )
    .unwrap();
    air.alpha_mut().internal.as_mut_slice().copy_from_slice(&inp.ad);
    air.u_mut().internal.as_mut_slice().copy_from_slice(&inp.ud);
    water.u_mut().internal.as_mut_slice().copy_from_slice(&inp.uc);
    TwoFluidSystem::new(air, water).unwrap()
}

/// Count of bit-identical cells and the worst relative difference.
fn compare(port: &[f64], up: &[f64]) -> (usize, f64) {
    assert_eq!(port.len(), up.len());
    let mut same = 0;
    let mut worst: f64 = 0.0;
    for (a, b) in port.iter().zip(up) {
        if a.to_bits() == b.to_bits() {
            same += 1;
        } else {
            worst = worst.max(((a - b) / b).abs());
        }
    }
    (same, worst)
}

fn differing(port: &[f64], up: &[f64]) -> Vec<usize> {
    (0..port.len()).filter(|&i| port[i].to_bits() != up[i].to_bits()).collect()
}

#[test]
fn inputs_regenerate_bit_for_bit() {
    let inp = generate_inputs();
    let up = read_reference();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&inp.ad), bits(&up["ad"]));
    for (k, (nd, nc)) in [("Ud_x", "Uc_x"), ("Ud_y", "Uc_y"), ("Ud_z", "Uc_z")].iter().enumerate() {
        let pick = |v: &[Vector3]| -> Vec<f64> { v.iter().map(|u| [u.x, u.y, u.z][k]).collect() };
        assert_eq!(bits(&pick(&inp.ud)), bits(&up[*nd]), "{nd}");
        assert_eq!(bits(&pick(&inp.uc)), bits(&up[*nc]), "{nc}");
    }
}


/// F1. Slip Reynolds number: 4096 of 4096 bit-identical to ESI
/// `mag(U1-U2)*d/nu` (`sqrt` is correctly rounded on both sides; the port's
/// `nu = mu/rho` round-trips the ESI `nu` exactly for these constants).
#[test]
fn slip_reynolds_number_is_bit_identical() {
    let inp = generate_inputs();
    let up = read_reference();
    let sys = port_system(&inp);
    let re = sys.reynolds_number().unwrap();
    assert_eq!(compare(re.internal.as_slice(), &up["Re"]), (N, 0.0));
    assert_eq!(re.internal.as_slice()[0], 0.0, "Ur = 0 cell");
}

/// F2. SchillerNaumann `K`: 4096 of 4096 bit-identical to ESI v2506 (predicted
/// >= 99 %). `pow(Re, 0.687)` differs between glibc and PETIR in 5 cells, but
/// 4 of them are in the Newton branch (`Re >= 1000`), where the value is not
/// used, and the fifth (cell 2020, Re = 550) is absorbed by the rounding of
/// `1 + 0.15*pow`. Only 750 of 4096 cells are in the `Re < 1000` branch.
#[test]
fn schiller_naumann_k_against_esi_v2506() {
    let inp = generate_inputs();
    let up = read_reference();
    let sys = port_system(&inp);
    let k = DragModel::SchillerNaumann.k_d(&sys).unwrap();
    assert_eq!(compare(k.internal.as_slice(), &up["K_sn"]), (N, 0.0));
    let re = sys.reynolds_number().unwrap();
    let pw: Vec<f64> = re.internal.as_slice().iter().map(|r| r.r_powf(0.687)).collect();
    let d = differing(&pw, &up["powRe0687"]);
    assert_eq!(d, vec![326, 1598, 1968, 2020, 2644]);
    assert_eq!(up["Re"].iter().filter(|r| **r < 1000.0).count(), 750);
    // forced cells: Re = 0, alpha_d = 0, alpha_d = 1e-9 (below the residual)
    for i in [0usize, 1, 3] {
        assert_eq!(k.internal.as_slice()[i].to_bits(), up["K_sn"][i].to_bits(), "cell {i}");
    }
}

/// F3 and F4. Wen-Yu: the dev form (`pow(a2,-2.65)`) is bit-identical to ESI in
/// 2889 of 4096 cells, worst 6.1e-16 (predicted < 50 % identical: **missed**,
/// the two exponent forms round alike more often than expected; bound hit).
/// The ESI form `WenYuEsi` (`pow(a2,-3.65)*max(alpha_c, residual)`) is 4096 of
/// 4096. Both agree in the `alpha_d = 1` cell (cell 2) to 1e-14, the dev form
/// not bit for bit.
#[test]
fn wen_yu_dev_form_and_esi_form_against_esi_v2506() {
    let inp = generate_inputs();
    let up = read_reference();
    let sys = port_system(&inp);
    let dev = DragModel::WenYu.k_d(&sys).unwrap();
    let (same, worst) = compare(dev.internal.as_slice(), &up["K_wy"]);
    assert_eq!(same, 2889);
    assert!(worst < 6.2e-16, "worst {worst}");
    assert_ne!(dev.internal.as_slice()[2].to_bits(), up["K_wy"][2].to_bits());
    assert!(((dev.internal.as_slice()[2] - up["K_wy"][2]) / up["K_wy"][2]).abs() < 1e-14);
    let esi = DragModel::WenYuEsi { residual_re: RESIDUAL_RE }.k_d(&sys).unwrap();
    assert_eq!(compare(esi.internal.as_slice(), &up["K_wy"]), (N, 0.0));
    // 1886 cells have Res < 1000 (the 24(1+0.15 Res^0.687) branch)
    let low = (0..N).filter(|&i| (1.0 - up["ad"][i]).max(1e-6) * up["Re"][i] < 1000.0).count();
    assert_eq!(low, 1886);
}

/// F5. Per-force status. Drag agrees with ESI (F1-F4). The forces ESI's
/// `twoPhaseSystem` sums beside drag are not ported: each returns
/// `NotImplemented`. **Gaps, not defects.** In ESI's order (drag, virtual mass,
/// lift, wall lubrication, turbulent dispersion) the first with no port
/// counterpart is virtual mass.
#[test]
fn missing_forces_are_recorded_as_gaps() {
    let inp = generate_inputs();
    let sys = port_system(&inp);
    for force in [
        InterfacialForce::VirtualMass,
        InterfacialForce::Lift,
        InterfacialForce::WallLubrication,
        InterfacialForce::TurbulentDispersion,
    ] {
        assert!(force.momentum_coefficient(&sys).is_err(), "force unexpectedly implemented");
    }
}
