// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Interfacial heat transfer `K` against upstream OpenFOAM on a frozen field
//! (gh:#1367, row 3).
//!
//! # Method
//! `verification_and_validation/driver/heatTransferDump.C` evaluates upstream's
//! `K()` expressions, transcribed verbatim, on OpenFOAM `scalarField`s built
//! from the ESI v2506 libraries (ESI `RanzMarshall` and `sphericalHeatTransfer`;
//! Foundation `sphericalHeatTransfer` and `Gunn`, because ESI v2506 has no
//! Gunn). 4096 cells, inputs from a splitmix64 generator that this test
//! reproduces and checks bit for bit against the committed input columns. The
//! port is then evaluated on the same inputs and compared bit pattern by bit
//! pattern. The expression, not `K()` itself, is shared: the `phasePair`
//! needs the unbuilt phaseSystem library. See
//! `verification_and_validation/upstream_comparison_gh1367.md` for predictions
//! and results.
//!
//! # Results
//! Pinned below as `PIN_*`; the record explains each count.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer;
use std::collections::BTreeMap;
use std::path::PathBuf;

const N: usize = 4096;
const RESIDUAL: f64 = 1e-6;

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

/// Inputs in the order the driver draws them: ad, kc, kd, d, Re, Pr.
fn generate_inputs() -> BTreeMap<&'static str, Vec<f64>> {
    let mut g = SplitMix(0x9E37_79B9_7F4A_7C15);
    let mut cols: BTreeMap<&'static str, Vec<f64>> = BTreeMap::new();
    for _ in 0..N {
        let u: Vec<f64> = (0..6).map(|_| g.next()).collect();
        cols.entry("ad").or_default().push(0.0 + (1.0 - 0.0) * u[0]);
        cols.entry("kc").or_default().push(0.02 + (0.7 - 0.02) * u[1]);
        cols.entry("kd").or_default().push(0.01 + (0.1 - 0.01) * u[2]);
        cols.entry("d").or_default().push(1e-4 + (1e-2 - 1e-4) * u[3]);
        cols.entry("Re").or_default().push(0.0 + (5e4 - 0.0) * u[4]);
        cols.entry("Pr").or_default().push(0.5 + (12.0 - 0.5) * u[5]);
    }
    cols.get_mut("ad").unwrap()[0] = 0.0;
    cols.get_mut("ad").unwrap()[1] = 1.0;
    cols.get_mut("Re").unwrap()[2] = 0.0;
    cols.get_mut("ad").unwrap()[3] = 1e-9;
    cols
}

fn read_reference() -> BTreeMap<String, Vec<f64>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/reference/heat_transfer_v2506.txt.gz");
    let bytes = std::fs::read(path).expect("reference file");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (name, _idx, hex) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        out.entry(name.to_string())
            .or_default()
            .push(f64::from_bits(u64::from_str_radix(hex, 16).unwrap()));
    }
    out
}

/// Count of bit-identical cells and the worst relative difference.
fn compare(port: &[f64], up: &[f64]) -> (usize, f64) {
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

fn port_k(model: InterfacialHeatTransfer, c: &BTreeMap<&'static str, Vec<f64>>) -> Vec<f64> {
    (0..N)
        .map(|i| {
            model
                .volumetric_coefficient_si(
                    c["ad"][i],
                    1.0 - c["ad"][i],
                    c["kc"][i],
                    c["kd"][i],
                    c["d"][i],
                    c["Re"][i],
                    c["Pr"][i],
                    RESIDUAL,
                )
                .unwrap()
        })
        .collect()
}

#[test]
fn inputs_regenerate_bit_for_bit() {
    let mine = generate_inputs();
    let up = read_reference();
    for name in ["ad", "kc", "kd", "d", "Re", "Pr"] {
        let a: Vec<u64> = mine[name].iter().map(|v| v.to_bits()).collect();
        let b: Vec<u64> = up[name].iter().map(|v| v.to_bits()).collect();
        assert_eq!(a, b, "input column {name}");
    }
}

fn differing(port: &[f64], up: &[f64]) -> Vec<usize> {
    (0..N).filter(|&i| port[i].to_bits() != up[i].to_bits()).collect()
}

/// Port transcendental intermediates, as the port's `Nu` evaluates them.
struct Intermediates {
    cbrt_pr: Vec<f64>,
    pow02: Vec<f64>,
    pow07: Vec<f64>,
}

fn port_intermediates(c: &BTreeMap<&'static str, Vec<f64>>) -> Intermediates {
    use petir::mathf::RealMath;
    Intermediates {
        cbrt_pr: c["Pr"].iter().map(|v| v.r_cbrt()).collect(),
        pow02: c["Re"].iter().map(|v| v.r_powf(0.2)).collect(),
        pow07: c["Re"].iter().map(|v| v.r_powf(0.7)).collect(),
    }
}

/// First divergent operation: `sqrt` is identical in 4096 of 4096 cells;
/// `cbrt` differs (PETIR's is correctly rounded in 4096 of 4096, checked
/// against a 60-digit Newton iteration; glibc's in 2159); `pow(Re, 0.2)` and
/// `pow(Re, 0.7)` differ in 6 and 3 cells.
#[test]
fn transcendental_intermediates_against_glibc() {
    let c = generate_inputs();
    let up = read_reference();
    let sq: Vec<f64> = c["Re"].iter().map(|v| v.sqrt()).collect();
    let m = port_intermediates(&c);
    assert_eq!(compare(&sq, &up["sqrtRe"]), (4096, 0.0));
    let cb = compare(&m.cbrt_pr, &up["cbrtPr"]);
    assert_eq!(cb.0, 2159, "cbrt(Pr) bit-identical count");
    assert!(cb.1 < 4.4e-16, "cbrt(Pr) worst {}", cb.1);
    let p2 = compare(&m.pow02, &up["powRe02"]);
    assert_eq!(p2.0, 4090, "pow(Re,0.2) bit-identical count");
    assert!(p2.1 < 2.3e-16);
    let p7 = compare(&m.pow07, &up["powRe07"]);
    assert_eq!(p7.0, 4093, "pow(Re,0.7) bit-identical count");
    assert!(p7.1 < 1.4e-16);
}

/// RanzMarshall: 2601 of 4096 bit-identical to ESI v2506, worst 6.2e-16. Every
/// cell that differs is a cell where glibc's `cbrt(Pr)` differs from PETIR's.
#[test]
fn ranz_marshall_against_esi_v2506() {
    let c = generate_inputs();
    let up = read_reference();
    let k = port_k(InterfacialHeatTransfer::RanzMarshall, &c);
    let (same, worst) = compare(&k, &up["K_rm_esi"]);
    assert_eq!(same, 2601);
    assert!(worst < 6.3e-16, "worst {worst}");
    let m = port_intermediates(&c);
    let cbrt_differs = differing(&m.cbrt_pr, &up["cbrtPr"]);
    for i in differing(&k, &up["K_rm_esi"]) {
        assert!(cbrt_differs.contains(&i), "cell {i} differs without a cbrt difference");
    }
    for i in [0usize, 3] {
        assert_eq!(k[i].to_bits(), up["K_rm_esi"][i].to_bits(), "residual-branch cell {i}");
    }
}

/// Spherical: the port reads `kappa_d` (Foundation). ESI v2506 reads
/// `continuous().kappa()`: 0 of 4096 cells agree, and the ratio is
/// `kappa_d / kappa_c`. Against Foundation's expression all 4096 agree.
#[test]
fn spherical_follows_foundation_not_esi() {
    let c = generate_inputs();
    let up = read_reference();
    let k = port_k(InterfacialHeatTransfer::Spherical, &c);
    assert_eq!(compare(&k, &up["K_sph_found"]), (4096, 0.0));
    assert_eq!(compare(&k, &up["K_sph_esi"]).0, 0);
    let mut worst: f64 = 0.0;
    for i in 0..N {
        let ratio = k[i] / up["K_sph_esi"][i];
        let expect = c["kd"][i] / c["kc"][i];
        worst = worst.max(((ratio - expect) / expect).abs());
    }
    println!("spherical: worst |K_port/K_esi / (kd/kc) - 1| = {worst:e}");
    assert!(worst < 4.3e-16, "worst {worst}");
}

/// Gunn (Foundation only; ESI v2506 has none): 2639 of 4096 bit-identical,
/// worst 6.7e-16; every differing cell has a differing glibc `cbrt` or `pow`.
#[test]
fn gunn_against_foundation_expression() {
    let c = generate_inputs();
    let up = read_reference();
    let k = port_k(InterfacialHeatTransfer::Gunn, &c);
    let (same, worst) = compare(&k, &up["K_gunn_found"]);
    assert_eq!(same, 2639);
    assert!(worst < 6.8e-16, "worst {worst}");
    let m = port_intermediates(&c);
    let (a, b, d) = (
        differing(&m.cbrt_pr, &up["cbrtPr"]),
        differing(&m.pow02, &up["powRe02"]),
        differing(&m.pow07, &up["powRe07"]),
    );
    for i in differing(&k, &up["K_gunn_found"]) {
        assert!(a.contains(&i) || b.contains(&i) || d.contains(&i), "cell {i}");
    }
    // Cell 0 (alpha_d = 0) is identical; cell 3 (alpha_d = 1e-9) differs by
    // 1 ulp through glibc's cbrt/pow, so the residual cells are not both exact.
    assert_eq!(k[0].to_bits(), up["K_gunn_found"][0].to_bits());
    assert_ne!(k[3].to_bits(), up["K_gunn_found"][3].to_bits());
}

