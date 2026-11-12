// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Drift-flux relative-velocity model against ESI OpenFOAM-v2506's own
//! `driftFluxFoam` classes on a frozen field (gh:#1367, row 1).
//!
//! # Method
//! `verification_and_validation/driver/driftFluxDump.C`, built by
//! `scripts/run_drift_flux.sh` from ESI's UNMODIFIED
//! `incompressibleTwoPhaseInteractingMixture`, `mixtureViscosityModels` and
//! `relativeVelocityModels/{relativeVelocityModel,simple,general}` sources,
//! reads the `RAS/dahl` tutorial mesh (8000 cells) and dictionaries, imposes a
//! frozen `alpha.sludge` from a splitmix64 generator (reproduced here and
//! checked bit for bit), calls the real `correct()`, `rho()` and `tauDm()`,
//! and dumps IEEE-754 bit patterns. The port is evaluated on the same alphas
//! and compared bit pattern by bit pattern. MULES is not ported, so alpha
//! transport is NOT compared (see the record). Predictions D1-D5 and the
//! results are in `verification_and_validation/upstream_comparison_gh1367.md`.
//!
//! # Results
//! Pinned below; the record explains each count.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_basic_lib::prelude::{
    BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind, Vector3, VolScalarField, VolVectorField,
};
use outram_foam_multiphase::drift_flux::{tau_dm_openfoam, DriftFluxMixture, SlipModel};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, MassDensity};
use uom::si::mass_density::kilogram_per_cubic_meter;

const N: usize = 8000;

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

/// Frozen alpha, as the driver draws it (one draw per cell, then forced cells).
fn generate_alpha() -> Vec<f64> {
    let mut g = SplitMix(0x9E37_79B9_7F4A_7C15);
    let mut a: Vec<f64> = (0..N).map(|_| 0.0 + 0.5 * g.next()).collect();
    a[0] = 0.0;
    a[1] = -1e-3;
    a[2] = 1e-9;
    a[3] = 0.5;
    a[4] = 0.999999;
    a[5] = 1e-3;
    a
}

fn read_reference() -> BTreeMap<String, Vec<f64>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/reference/drift_flux_v2506.txt.gz");
    let bytes = std::fs::read(path).expect("reference file");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, hex) = if f[0] == "const" { (format!("const_{}", f[1]), f[2]) } else { (f[0].to_string(), f[2]) };
        out.entry(name).or_default().push(f64::from_bits(u64::from_str_radix(hex, 16).unwrap()));
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

/// Count of bit-identical entries and the worst relative difference.
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

fn alpha_field(mesh: &Arc<FvMesh>, a: &[f64]) -> VolScalarField {
    let mut f = VolScalarField::uniform("alpha", mesh.clone(), 0.0);
    f.internal.as_mut_slice().copy_from_slice(a);
    f
}

fn components(v: &[Vector3]) -> [Vec<f64>; 3] {
    [v.iter().map(|u| u.x).collect(), v.iter().map(|u| u.y).collect(), v.iter().map(|u| u.z).collect()]
}

fn port_udm(model: &SlipModel, a: &[f64]) -> Vec<Vector3> {
    let mesh = line_mesh(N);
    let u = VolVectorField::zero("U", mesh.clone());
    model.drift_velocity(&u, &alpha_field(&mesh, a)).unwrap()
}

fn compare_vec(port: &[Vector3], up: &BTreeMap<String, Vec<f64>>, prefix: &str) -> Vec<(usize, f64)> {
    let c = components(port);
    ["x", "y", "z"].iter().enumerate().map(|(i, s)| compare(&c[i], &up[&format!("{prefix}_{s}")])).collect()
}

const V0: Vector3 = Vector3 { x: 0.0, y: -0.002198, z: 0.0 };

#[test]
fn inputs_regenerate_bit_for_bit() {
    let up = read_reference();
    let a: Vec<u64> = generate_alpha().iter().map(|v| v.to_bits()).collect();
    let b: Vec<u64> = up["alpha"].iter().map(|v| v.to_bits()).collect();
    assert_eq!(a, b, "alpha column");
    assert_eq!(up["const_rhod"][0], 1996.0);
    assert_eq!(up["const_rhoc"][0], 996.0);
    // alpha2 is 1 - alpha1 bit for bit in upstream
    let a2: Vec<f64> = generate_alpha().iter().map(|v| 1.0 - v).collect();
    assert_eq!(compare(&a2, &up["alpha2"]), (N, 0.0));
}


fn differing(port: &[f64], up: &[f64]) -> Vec<usize> {
    (0..port.len()).filter(|&i| port[i].to_bits() != up[i].to_bits()).collect()
}

/// D1. The dev form `TerminalVelocity{u_t = V0}` is NOT ESI `simple`: ESI
/// multiplies by `rhoc/rho(alpha)` per cell. Only cell 0 (`alpha = 0`, so
/// `rho = rhoc` exactly) is bit-identical (1 of 8000; the prediction also
/// counted cell 1 at `alpha = -1e-3`, but `rho(-1e-3) != rhoc`, so it
/// differs: part-miss). `Udm_port / Udm_ESI = rho(alpha)/rhoc` to 6e-16.
#[test]
fn dev_terminal_velocity_is_not_esi_simple() {
    let up = read_reference();
    let a = generate_alpha();
    let dev = SlipModel::TerminalVelocity { u_t: V0, hindrance_exp: 285.84 };
    let u = port_udm(&dev, &a);
    let r = compare_vec(&u, &up, "Udm_simple");
    assert_eq!(r[1].0, 1, "y bit-identical count");
    assert_eq!((r[0].0, r[2].0), (N, N), "x and z are zero on both sides");
    let mut worst: f64 = 0.0;
    for i in 0..N {
        let up_y = up["Udm_simple_y"][i];
        if up_y == 0.0 {
            continue;
        }
        let ratio = u[i].y / up_y;
        let expect = (a[i] * 1996.0 + (1.0 - a[i]) * 996.0) / 996.0;
        worst = worst.max(((ratio - expect) / expect).abs());
    }
    println!("D1: worst |ratio/(rho/rhoc) - 1| = {worst:e}");
    assert!(worst < 6e-16, "worst {worst}");
}

/// D2. `OpenFoamSimple`: 7993 of 8000 y components bit-identical to ESI's real
/// `simple::correct()`, worst 2.6e-16 (1 ulp). Every differing cell is a cell
/// where `pow(10, -a*max(alpha,0))` differs between glibc and PETIR.
#[test]
fn open_foam_simple_against_esi_v2506() {
    let up = read_reference();
    let a = generate_alpha();
    let m = SlipModel::OpenFoamSimple { rho_c: 996.0, rho_d: 1996.0, v0: V0, a: 285.84 };
    let u = port_udm(&m, &a);
    let r = compare_vec(&u, &up, "Udm_simple");
    assert_eq!((r[0], r[2]), ((N, 0.0), (N, 0.0)));
    assert_eq!(r[1].0, 7993);
    assert!(r[1].1 < 2.7e-16, "worst {}", r[1].1);
    use petir::mathf::RealMath;
    let pw: Vec<f64> = a.iter().map(|&x| 10.0_f64.r_powf(-285.84 * x.max(0.0))).collect();
    let pw_differs = differing(&pw, &up["pow10_a"]);
    let y: Vec<f64> = u.iter().map(|v| v.y).collect();
    for i in differing(&y, &up["Udm_simple_y"]) {
        assert!(pw_differs.contains(&i), "cell {i} differs without a pow difference");
    }
    // the first divergent operation: pow(10, x), glibc against PETIR
    assert_eq!(pw_differs.len(), 7);
}

/// D3. `OpenFoamGeneral`: 7995 of 8000 (residualAlpha 0, as dahl) and 7992 of
/// 8000 (residualAlpha 1e-3) y components bit-identical, worst 1.5e-16 and
/// 1.4e-16; every differing cell has a differing `exp`.
#[test]
fn open_foam_general_against_esi_v2506() {
    use petir::mathf::RealMath;
    let up = read_reference();
    let a = generate_alpha();
    for (res, name, count, bound, ea, ea1) in [
        (0.0, "Udm_general", 7995usize, 1.6e-16, "exp_a_r0", "exp_a1_r0"),
        (1e-3, "Udm_general_res", 7992usize, 1.5e-16, "exp_a_r1", "exp_a1_r1"),
    ] {
        let m = SlipModel::OpenFoamGeneral {
            rho_c: 996.0,
            rho_d: 1996.0,
            v0: V0,
            a: 285.84,
            a1: 0.1,
            residual_alpha: res,
        };
        let u = port_udm(&m, &a);
        let r = compare_vec(&u, &up, name);
        assert_eq!((r[0], r[2]), ((N, 0.0), (N, 0.0)));
        assert_eq!(r[1].0, count, "{name}");
        assert!(r[1].1 < bound, "{name} worst {}", r[1].1);
        let x: Vec<f64> = a.iter().map(|&v| (v - res).max(0.0)).collect();
        let e1: Vec<f64> = x.iter().map(|&v| (-285.84 * v).r_exp()).collect();
        let e2: Vec<f64> = x.iter().map(|&v| (-0.1 * v).r_exp()).collect();
        let (d1, d2) = (differing(&e1, &up[ea]), differing(&e2, &up[ea1]));
        let y: Vec<f64> = u.iter().map(|v| v.y).collect();
        for i in differing(&y, &up[&format!("{name}_y")]) {
            assert!(d1.contains(&i) || d2.contains(&i), "{name}: cell {i} differs without an exp difference");
        }
    }
}

/// D4 and D5. Mixture density (`rho_mixture`) and `tauDm` are bit-identical to
/// ESI in 8000 of 8000 cells (D5 fed ESI's own `Udm`, so only the tensor
/// algebra is under test).
#[test]
fn mixture_density_and_tau_dm_bit_identical() {
    let up = read_reference();
    let a = generate_alpha();
    let mesh = line_mesh(N);
    let mut mix = DriftFluxMixture::new(
        mesh,
        MassDensity::new::<kilogram_per_cubic_meter>(1996.0),
        MassDensity::new::<kilogram_per_cubic_meter>(996.0),
        DynamicViscosity::new::<pascal_second>(1e-3),
        DynamicViscosity::new::<pascal_second>(1e-3),
        0.0,
    )
    .unwrap();
    mix.alpha_mut().internal.as_mut_slice().copy_from_slice(&a);
    assert_eq!(compare(mix.rho_mixture().internal.as_slice(), &up["rho_mix"]), (N, 0.0));
    assert_eq!(compare(mix.rho_mixture().internal.as_slice(), &up["rho_rvm"]), (N, 0.0));
    let udm: Vec<Vector3> = (0..N)
        .map(|i| Vector3::new(up["Udm_simple_x"][i], up["Udm_simple_y"][i], up["Udm_simple_z"][i]))
        .collect();
    let tau = tau_dm_openfoam(&a, &udm, 996.0, 1996.0).unwrap();
    for (k, s) in ["xx", "xy", "xz", "yy", "yz", "zz"].iter().enumerate() {
        let p: Vec<f64> = tau.iter().map(|t| t[k]).collect();
        // -0.0 and +0.0 compare by bits: upstream's signed zeros must match too
        assert_eq!(compare(&p, &up[&format!("tauDm_simple_{s}")]), (N, 0.0), "tauDm_{s}");
    }
    assert!(tau_dm_openfoam(&a, &udm[..3], 996.0, 1996.0).is_err());
}
