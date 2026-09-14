//! WIMSR golden validation against NJOY2016 on ENDF/B-VIII.0 U-238: the
//! crate's WIMS-D library from NJOY's own GENDF against the library NJOY
//! wrote from it (`reference-data/wimsr/`, wave-6 deck: RECONR 0.001 →
//! BROADR 293.6 K → THERMR free-gas `MT=221` → GROUPR 29 groups, `iwt=3`,
//! `lord=1`, six sigma-zeros → WIMSR `iverw=4 igroup=9 (29 15 8 15)`,
//! `ires=1 mti=221 ip1opt=0 isof=1`, eight unit Goldstein lambdas).
//!
//! **Tier 1** (NJOY's GENDF in, so only `wimsr.f90`'s arithmetic and
//! formatting differ). Prediction: the library **byte-identical** — every
//! quantity is a sum or ratio of GENDF words printed with 9 significant
//! figures, and the Fortran `1PE15.8` edit is reproduced by
//! [`njoy_outram_park_fork::wimsr::wimout::e15_8`]. Where a line differs
//! the test reports it and falls back to a numeric comparison at 1e-8
//! (the printing floor), so a genuine arithmetic-order difference in the
//! last digit is reported, not hidden.
//!
//! The listing's intermediate stage prints (`iprint = 2`) are pinned
//! too: the transport-corrected total, absorption, ν σ_f, σ_f, potential
//! and slowing-down power (4 figures), the resonance-integral tables (6
//! figures) and the P1 rows (4 figures).

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::wimsr::{run_gendf, WimsrInput};

const LABEL: &str = "wimsr-u238";
const GENDF: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-thermal221-for-wimsr.gendf";
const WIMS: &str = "u238-ENDF8.0-293.6K-29g-6sigz-wimsd.wims";

fn input() -> WimsrInput {
    WimsrInput {
        iprint: 2,
        iverw: 4,
        ngnd: 29,
        nfg: 15,
        nrg: 8,
        igref: 15,
        mat: 9237,
        rdfid: 9238.0,
        iburn: 0,
        ntemp: 0,
        nsigz: 0,
        sgref: 1.0e10,
        ires: 1,
        sigp: 0.0,
        mti: 221,
        mtc: 0,
        ip1opt: 0,
        inorf: 0,
        isof: 1,
        ifprod: 0,
        p1flx_user: Vec::new(),
        efiss: 0.0,
        burn: Vec::new(),
        glam: vec![1.0; 8],
    }
}

fn numbers(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

#[test]
fn library_matches_njoy_wimsd_tape() {
    let Some(gendf) = reference_file_or_skip("wimsr", GENDF, LABEL) else {
        return;
    };
    let Some(wims) = reference_file_or_skip("wimsr", WIMS, LABEL) else {
        return;
    };
    let tape = Tape::read_file(&gendf).expect("GENDF parses");
    let want = std::fs::read_to_string(&wims).expect("library reads");
    let want_lines: Vec<&str> = want.lines().collect();
    let out = run_gendf(&tape, &input()).expect("wimsr runs");
    for m in &out.xsecs.messages {
        println!("[{LABEL}] message: {m}");
    }
    println!(
        "[{LABEL}] {} lines (njoy {}), ifiss {}, ntemp {}, nfiss {}",
        out.lines.len(),
        want_lines.len(),
        out.xsecs.ifiss,
        out.xsecs.ntemp,
        out.xsecs.nfiss
    );
    let mut identical = 0usize;
    let mut differing: Vec<(usize, String, String)> = Vec::new();
    let mut worst = 0.0f64;
    for (i, (a, b)) in out.lines.iter().zip(&want_lines).enumerate() {
        if a == b {
            identical += 1;
            continue;
        }
        let (na, nb) = (numbers(a), numbers(b));
        if na.len() == nb.len() && !na.is_empty() {
            for (x, y) in na.iter().zip(&nb) {
                let rel = (x - y).abs() / y.abs().max(x.abs()).max(1e-300);
                if rel > worst {
                    worst = rel;
                }
            }
        } else {
            worst = 1.0;
        }
        if differing.len() < 12 {
            differing.push((i + 1, a.clone(), b.to_string()));
        }
    }
    for (n, a, b) in &differing {
        println!("[{LABEL}] line {n}\n  crate {a}\n  njoy  {b}");
    }
    println!(
        "[{LABEL}] identical lines {identical}/{}, differing {}, worst numeric {:.3e}",
        want_lines.len(),
        want_lines.len() - identical,
        worst
    );
    assert_eq!(out.lines.len(), want_lines.len(), "{LABEL}: line count");
    assert!(worst < 1e-8, "{LABEL}: numeric deviation {worst:.3e}");
    assert_eq!(identical, want_lines.len(), "{LABEL}: library not byte-identical");
}

/// The `iprint = 2` listing values (`reference-data/wimsr/*.njoy-listing`).
#[test]
fn listing_stage_values() {
    let Some(gendf) = reference_file_or_skip("wimsr", GENDF, LABEL) else {
        return;
    };
    let tape = Tape::read_file(&gendf).expect("GENDF parses");
    let out = run_gendf(&tape, &input()).expect("wimsr runs");
    let ti = &out.xsecs.temp_independent;
    // record = spot(16..23), sdp(16..23), xtr(1..23), ab0(1..23), 8 zeros, glam(8)
    let spot = &ti.record[0..8];
    let sdp = &ti.record[8..16];
    let xtr = &ti.record[16..39];
    let ab0 = &ti.record[39..62];
    let check = |name: &str, got: &[f64], want: &[f64], tol: f64| {
        let mut w = 0.0f64;
        for (i, (a, b)) in got.iter().zip(want).enumerate() {
            let rel = (a - b).abs() / b.abs().max(1e-300);
            assert!(rel < tol, "{LABEL}: {name}[{}] got {a:e} want {b:e}", i + 1);
            w = w.max(rel);
        }
        println!("[{LABEL}] {name}: worst {w:.2e} over {} values", want.len());
    };
    check(
        "sigma potential (16-23)",
        spot,
        &[9.3358e0, 1.4437e1, 6.8948e0, 9.0852e0, 1.3952e1, 5.9629e0, 8.7257e0, 1.0140e1],
        6e-5,
    );
    check(
        "scattering power per unit lethargy (16-23)",
        sdp,
        &[4.9393e-2, 7.5613e-1, 6.7407e-3, 4.9319e-1, 1.8469e-3, 2.2979e-1, 1.9735e-1, 4.1883e-1],
        6e-5,
    );
    check(
        "transport corrected total (1-23)",
        xtr,
        &[
            1.4252e0, 1.2969e0, 1.4168e0, 1.8916e0, 3.4126e0, 7.2230e0, 1.1206e1, 1.3190e1,
            1.2588e1, 1.2742e1, 1.3253e1, 1.2478e1, 1.3867e1, 1.7859e1, 2.9135e1, 2.6949e1,
            1.4852e1, 1.4905e2, 9.4911e0, 3.0837e2, 7.4089e0, 9.2011e0, 1.0970e1,
        ],
        6e-5,
    );
    check(
        "absorption (1-23)",
        ab0,
        &[
            1.1662e0, 8.3999e-1, 5.6522e-1, 3.3245e-1, 1.2268e-1, 1.2946e-1, 2.3304e-1,
            4.2596e-1, 6.0475e-1, 7.7597e-1, 1.1841e0, 1.9054e0, 3.4378e0, 5.4271e0, 1.8950e1,
            1.7659e1, 3.6651e-1, 1.4221e2, 3.9342e-1, 2.9414e2, 1.5263e0, 4.8840e-1, 8.2753e-1,
        ],
        6e-5,
    );
    check(
        "neutron current spectrum (1-23)",
        &ti.current_spectrum,
        &[
            1.9876e0, 1.6320e0, 1.6329e0, 1.4763e0, 1.2638e0, 1.7588e0, 5.2633e-1, 5.8185e-1,
            5.7208e-1, 5.8513e-1, 8.7797e-1, 9.0731e-1, 8.0604e-1, 6.9433e-1, 9.9561e-1,
            8.7388e-1, 1.2550e-1, 4.8983e-1, 2.2945e-1, 8.5720e-2, 5.5567e-1, 5.4069e-1,
            2.1534e-1,
        ],
        6e-5,
    );
    let td = &out.xsecs.per_temp[0];
    check(
        "transport corrected total at 294 K (24-29)",
        &td.record[0..6],
        &[6.2137e2, 1.3991e1, 9.2968e0, 9.6177e0, 1.0066e1, 4.2003e1],
        6e-5,
    );
    check(
        "absorption at 294 K (24-29)",
        &td.record[6..12],
        &[6.0727e2, 6.8039e0, 4.9044e-1, 5.3861e-1, 9.0950e-1, 2.8932e1],
        6e-5,
    );
    // resonance integrals: group 16 and 20 absorption, group 20 fission yield
    let ri = out.resint.as_ref().expect("ires = 1");
    check(
        "resonance integral absorption group 16",
        &ri.groups[0].absorption[0],
        &[7.87028e-1, 9.79770e-1, 2.21060e0, 6.62746e0, 1.42090e1, 1.76593e1],
        6e-6,
    );
    check(
        "resonance integral absorption group 20",
        &ri.groups[4].absorption[0],
        &[4.69350e0, 6.29521e0, 1.59550e1, 6.00922e1, 1.87050e2, 2.94139e2],
        6e-6,
    );
    check(
        "resonance integral fission yield group 20",
        &ri.groups[4].nu_fission.as_ref().unwrap()[0],
        &[2.64616e-5, 3.56895e-5, 9.18682e-5, 3.50292e-4, 1.09452e-3, 1.72233e-3],
        6e-6,
    );
    check(
        "flux per unit lethargy group 17",
        &ri.groups[1].flux_per_lethargy[0],
        &[7.69683e-1, 9.38730e-1, 1.03657e0, 1.02279e0, 1.00583e0, 1.00000e0],
        6e-6,
    );
    // p1 rows: group 1 (l1=1, l2=2) and group 26 (l1=25, l2=29)
    let p1 = out.p1.as_ref().expect("ip1opt = 0");
    let r1 = &p1[0].rows[0];
    assert_eq!((r1.0, r1.1, r1.2), (1, 1, 2));
    check("p1 row 1", &r1.3, &[2.6671e0, 1.3632e-3], 6e-5);
    let r26 = &p1[0].rows[25];
    assert_eq!((r26.0, r26.1, r26.2), (26, 25, 29));
    check(
        "p1 row 26",
        &r26.3,
        &[9.6829e-4, 4.6523e-2, -2.0189e-2, -2.5003e-5, -6.5396e-7],
        6e-5,
    );
    // fission spectrum: 23 groups, first three
    assert_eq!(out.xsecs.nfiss, 23);
    check("fission spectrum (1-3)", &out.xsecs.uff[0..3], &[2.8546e-3, 6.0297e-2, 3.3594e-1], 6e-5);
}
