//! Code-to-code verification of the ported `nuclearSteadyStatePebble` model
//! against a **run of upstream GeN-Foam** (bead `op-2df1`).
//!
//! # What is compared
//!
//! Upstream's `3D_gFHR` tutorial writes the pebble model's **entire internal
//! chain** as fields — `TpS`, `Tmout`, `Tmav`, `TfS`, `Tfav`, `Tfmax` and
//! `keffmatrix` — which map one-to-one onto what
//! [`PebbleGeometry::steady_temperatures`] returns. So the comparison needs no
//! reconstruction and no inference: upstream's own pebble **surface**
//! temperature goes in, and every stage of the port's chain is checked against
//! upstream's own field for that stage, cell by cell.
//!
//! This supersedes the bound in
//! `the_gfhr_pebble_rise_fits_inside_upstreams_reported_temperatures`, which
//! could only check the pebble rise against the headroom between upstream's
//! `Tfmax_min` and its coolant inlet. That test remains valid and is still the
//! one that runs without an upstream clone; this one is the direct comparison.
//!
//! # The reference run
//!
//! Upstream GeN-Foam `652b3da` on OpenFOAM v2506 (ESI), both built from source
//! here. The run reproduced the tutorial's own `Alltest` reference —
//! `Tfmax (avg min max) = 981.812 / 900.664 / 1062.96 K` against the expected
//! `981.808 / 900.664 / 1062.87`, the **minimum exact** — which is what
//! qualifies it as a reference.
//!
//! Its `t = 50 s` fields are committed as
//! `tests/data/genfoam_gfhr_pebble_chain.csv`: 2005 cells sampled evenly from
//! 292 500, with the `TpS` extremes forced in so the endpoints are covered.
//!
//! # Results (measured 2026-09-15, over all 292 500 cells of the run)
//!
//! | stage | worst relative difference |
//! |---|---|
//! | `keffmatrix` (Maxwell mixture) | **9.44e-07** |
//! | `Tmout` (unheated shell) | **3.82e-06** |
//! | `Tmav` (heated annulus, to volume average) | **5.07e-06** |
//! | `TfS` (TRISO coatings) | **7.21e-06** |
//! | `Tfav` (kernel volume average) | **4.50e-06** |
//! | `Tfmax` (kernel centre) | **5.10e-06** |
//!
//! Every stage agrees to about 5 parts per million, which is the ASCII write
//! precision of the fields themselves — OpenFOAM writes 6 significant figures by
//! default — not a physics difference. The committed 2005-cell sample reproduces
//! the same bounds.
//!
//! **This verifies the whole model, not one number:** the shell conduction, the
//! adiabatic-inner-face annulus integral to its *volume average*, the series
//! coating resistance, the Maxwell effective conductivity, the kernel's 1/15 and
//! 1/6 factors, and upstream's `alpha` power bookkeeping.

use std::path::Path;

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::{
    PebbleConductivities, PebbleGeometry,
};

/// Uniform across the Core zone, from the case's own `phaseProperties`.
const ALPHA_STRUCTURE: f64 = 0.61;
const POWER_DENSITY: f64 = 19_965_700.0;
/// `powerDensityNeutronics` as the tutorial's `phaseProperties` states it (`19.96567386e6`): the fresh
/// run's input. The constant above is the older record's rounded value (19 965 700), 1.3e-6 higher.
const FRESH_POWER_DENSITY: f64 = 19.965_673_86e6;
/// Upstream's own computed Maxwell effective matrix conductivity, W/(m K).
const UPSTREAM_KEFFMATRIX: f64 = 29.587;

fn gfhr() -> PebbleGeometry {
    PebbleGeometry {
        core_radius: 1.38e-2,
        matrix_radius: 1.8e-2,
        shell_radius: 2.0e-2,
        fuel_radius: 212.5e-6,
        buffer_radius: 312.5e-6,
        inner_pyc_radius: 352.5e-6,
        silicon_carbide_radius: 387.5e-6,
        outer_pyc_radius: 427.5e-6,
        triso_count: 9022.0,
    }
}

fn gfhr_k() -> PebbleConductivities {
    PebbleConductivities {
        fuel: 3.3073,
        buffer: 0.50,
        pyrolytic_carbon: 4.00,
        silicon_carbide: 90.3,
        graphite: 41.68,
    }
}

struct Row {
    t_ps: f64,
    t_mout: f64,
    t_mav: f64,
    t_fs: f64,
    t_fav: f64,
    t_fmax: f64,
}

fn load() -> Vec<Row> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_gfhr_pebble_chain.csv");
    let text = std::fs::read_to_string(&path).expect("read the gFHR pebble-chain fixture");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("TpS,") && !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            Row {
                t_ps: v[0],
                t_mout: v[1],
                t_mav: v[2],
                t_fs: v[3],
                t_fav: v[4],
                t_fmax: v[5],
            }
        })
        .collect()
}

/// `COL`/`UNIFORM` lines of `refs/gfhr_chain.ref.txt.gz` (gh:#1361 round 3): a fresh run of the
/// real binary at writePrecision 17, third step (t = 0.358511 s), every 100th pebble-bed cell
/// plus the `TpS` extremes. Returns `(columns, uniform values)`.
fn fresh_chain() -> (std::collections::HashMap<String, Vec<f64>>, std::collections::HashMap<String, f64>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs/gfhr_chain.ref.txt.gz");
    let bytes = std::fs::read(&path).expect("reference present");
    // gzip -n: 10-byte header, 8-byte trailer.
    let text = String::from_utf8(
        miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate"),
    )
    .expect("utf8");
    let (mut cols, mut uniform) = (std::collections::HashMap::new(), std::collections::HashMap::new());
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("COL") => {
                let name = w.next().expect("name").to_string();
                cols.insert(name, w.map(|x| x.parse::<f64>().expect("f64")).collect());
            }
            Some("UNIFORM") => {
                let name = w.next().expect("name").to_string();
                uniform.insert(name, w.next().expect("value").parse::<f64>().expect("f64"));
            }
            _ => {}
        }
    }
    (cols, uniform)
}

/// **V&V, round 3 — every stage of the pebble chain against a FRESH 17-digit run.**
///
/// Prediction Q2 (written before the port run): each stage within 1e-12 relative, the 5e-6 of
/// the record above being the 6-digit ASCII floor of the committed CSV.
#[test]
fn the_ported_pebble_chain_reproduces_a_fresh_17_digit_gfhr_run() {
    let (cols, uniform) = fresh_chain();
    let g = gfhr();
    let k = gfhr_k();
    let t_ps = &cols["TpS"];
    assert!(t_ps.len() > 2000, "sample of {}", t_ps.len());

    let keff = g.effective_matrix_conductivity(&k);
    let keff_up = uniform["keffmatrix"];
    let keff_rel = (keff - keff_up).abs() / keff_up;

    const NAMES: [&str; 5] = ["Tmout", "Tmav", "TfS", "Tfav", "Tfmax"];
    let mut worst = [0.0_f64; 5];
    let mut worst_ulp = [0_u64; 5];
    for (i, &ts) in t_ps.iter().enumerate() {
        let t = g.steady_temperatures(ts, FRESH_POWER_DENSITY, ALPHA_STRUCTURE, &k);
        let ours = [t.matrix_outer, t.matrix_average, t.particle_surface, t.fuel_average, t.fuel_max];
        for (s, name) in NAMES.iter().enumerate() {
            let theirs = cols[*name][i];
            worst[s] = worst[s].max((ours[s] - theirs).abs() / theirs);
            let ulp = if ours[s].to_bits() == theirs.to_bits() { 0 } else { ours[s].to_bits().abs_diff(theirs.to_bits()) };
            worst_ulp[s] = worst_ulp[s].max(ulp);
        }
    }
    println!(
        "gFHR fresh 17-digit run: {} sampled cells; keffmatrix port {keff:.17} upstream {keff_up:.17} rel {keff_rel:.3e}",
        t_ps.len()
    );
    for (s, name) in NAMES.iter().enumerate() {
        println!("\t{name:8} worst relative difference {:.3e}  (worst {} ulp)", worst[s], worst_ulp[s]);
    }
    assert!(keff_rel < 1e-12, "keffmatrix {keff_rel:.3e}");
    for (s, name) in NAMES.iter().enumerate() {
        assert!(worst[s] < 1e-12, "{name} differs from upstream by {:.3e}", worst[s]);
    }
}

/// **V&V — every stage of the ported pebble model matches upstream's own fields.**
///
/// Methodology and measured results are in the module documentation above.
#[test]
fn the_ported_pebble_chain_reproduces_upstreams_gfhr_fields() {
    let rows = load();
    assert!(rows.len() > 1000, "fixture should carry a real sample");
    let g = gfhr();
    let k = gfhr_k();

    // The Maxwell mixture rule, against upstream's own computed value.
    let keff = g.effective_matrix_conductivity(&k);
    let keff_rel = (keff - UPSTREAM_KEFFMATRIX).abs() / UPSTREAM_KEFFMATRIX;

    let mut worst = [0.0_f64; 5];
    const NAMES: [&str; 5] = ["Tmout", "Tmav", "TfS", "Tfav", "Tfmax"];
    let mut t_ps_min = f64::INFINITY;
    let mut t_ps_max = 0.0_f64;

    for r in &rows {
        // Upstream's own pebble surface temperature is the input.
        let t = g.steady_temperatures(r.t_ps, POWER_DENSITY, ALPHA_STRUCTURE, &k);
        let pairs = [
            (t.matrix_outer, r.t_mout),
            (t.matrix_average, r.t_mav),
            (t.particle_surface, r.t_fs),
            (t.fuel_average, r.t_fav),
            (t.fuel_max, r.t_fmax),
        ];
        for (i, (ours, theirs)) in pairs.iter().enumerate() {
            worst[i] = worst[i].max((ours - theirs).abs() / theirs);
        }
        t_ps_min = t_ps_min.min(r.t_ps);
        t_ps_max = t_ps_max.max(r.t_ps);
    }

    println!(
        "\n=== gFHR pebble chain: port vs upstream GeN-Foam run ===\n\
         \tcells compared   {}\n\
         \tTpS range        {t_ps_min:.3} .. {t_ps_max:.3} K\n\
         \tkeffmatrix       port {keff:.6}  upstream {UPSTREAM_KEFFMATRIX}  \
         rel {keff_rel:.3e}",
        rows.len()
    );
    for (i, name) in NAMES.iter().enumerate() {
        println!("\t{name:8} worst relative difference {:.3e}", worst[i]);
    }

    assert!(
        keff_rel < 1.0e-5,
        "the Maxwell effective conductivity differs from upstream's by {keff_rel:.3e}"
    );
    for (i, name) in NAMES.iter().enumerate() {
        assert!(
            worst[i] < 1.0e-5,
            "{name} differs from upstream by {:.3e}, beyond the fields' own \
             6-significant-figure write precision",
            worst[i]
        );
    }
}
