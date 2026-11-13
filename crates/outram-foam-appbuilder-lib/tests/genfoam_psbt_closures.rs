//! Code-to-code verification of the PSBT water-cooled case's wall-friction
//! closure, against a **run of upstream GeN-Foam** (bead `op-2df1`).
//!
//! # Why this case is verified at the closure level
//!
//! `Tutorials/featureCases/1D_PSBT_SC` is the OECD/NRC PSBT subchannel
//! benchmark, and it sets `regionSolvers { fluidRegion twoPhase; }`. ~~This crate
//! has no two-phase Euler-Euler porous solver~~ (**CORRECTED 2026-11-04,
//! gh:#1218:** `solver::two_phase` now exists, but it does not port PSBT's
//! `multiRegimeBoilingTRACE`, `twoParameters` regime map or `byRegime` areas,
//! so the statement below about PSBT's exit void still holds) — `genfoam::thermal_hydraulics::
//! solver` has `one_phase` only — so the port **cannot** compute the case's exit
//! void fraction, and no amount of comparison here changes that. What it *can*
//! do is the closures, and PSBT's own `phaseProperties` names them:
//! `ReynoldsPower` fluid-structure drag, `BestionTRACE` interfacial drag,
//! `LockhartMartinelli` two-phase multiplier, `multiRegimeBoiling` heat
//! transfer.
//!
//! This test takes the first of those — the one the port implements as
//! [`FsWallFriction::ReynoldsPower`] — and checks it against upstream's **own
//! computed drag field**, cell by cell, on upstream's own Reynolds numbers.
//!
//! # The reference run
//!
//! Upstream GeN-Foam `652b3da` on OpenFOAM v2506 (ESI), both built from source
//! here, run on `PhaseII_Ex1_01_5215`. It reproduced the tutorial's own
//! `Alltest` reference **exactly** — `alpha.vapour = 0.123835` and
//! `T.fixedPower = 620.178` at the channel exit — which is what qualifies it as
//! a reference. Its `t = 5 s` fields are committed as
//! `tests/data/genfoam_psbt_fs_drag.csv`.
//!
//! # How the comparison is closed
//!
//! Upstream assembles the drag as (`FSDragFactor.C`, isotropic branch)
//!
//! ```text
//!   Kd = 0.5/Dh * (1 - alpha_structure) * rho * max(|U|, minMagU) * f(Re)
//! ```
//!
//! where `f` is the friction factor the selected model returns — for
//! `ReynoldsPower`, `coeff*Re^exp + const`, which PSBT sets to Blasius
//! (`0.316*Re^-0.25`). Rearranged, and with `rho|U|` recovered from upstream's
//! own `alphaRhoMagU.liquid / alpha.liquid`:
//!
//! ```text
//!   Dh = f_port(Re) / ( 2 Kd / [(1 - alpha_structure) rho|U|] )
//! ```
//!
//! So evaluating the **port's** friction factor against **upstream's** drag must
//! reproduce the hydraulic diameter — and the case declares exactly three of
//! them, per subchannel zone. That makes this falsifiable in a way a formula
//! comparison is not: if the port's `f` were wrong by any factor, the recovered
//! `Dh` would not land on a declared value at all.
//!
//! **Single-phase cells only.** PSBT applies a `LockhartMartinelli` two-phase
//! multiplier to `Kd` wherever vapour is present, which breaks the relation
//! above. The test uses the cells with `alpha.vapour == 0`; including the
//! two-phase cells (334 of them) raises the worst deviation from 6.2e-6 to 0.53,
//! which is the
//! multiplier showing up exactly where it should and is itself a check that the
//! filter is the right one.
//!
//! # Results (measured 2026-09-15)
//!
//! | | |
//! |---|---|
//! | cells compared | **836** of 1170 (those with `alpha.vapour == 0`) |
//! | Reynolds range | 2.01e5 to 6.12e5 |
//! | recovered `Dh` | lands on one of the three declared values in every cell |
//! | worst relative deviation | **6.15e-06** |
//!
//! 6 parts per million is the ASCII write precision of the fields themselves —
//! OpenFOAM writes 6 significant figures by default — not a physics difference.

use std::path::Path;

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::units::ReynoldsNumber;
use uom::si::ratio::ratio;

/// The three hydraulic diameters `PhaseII_Ex1_01_5215`'s `phaseProperties`
/// declares, one per subchannel zone (m).
const DECLARED_DH: [f64; 3] = [0.0063460691, 0.0081255374, 0.0117778432];

/// One row of upstream's `t = 5 s` fluid-region fields.
struct Cell {
    re: f64,
    kd_xx: f64,
    alpha_structure: f64,
    alpha_liquid: f64,
    alpha_rho_mag_u_liquid: f64,
    alpha_vapour: f64,
}

fn load() -> Vec<Cell> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_psbt_fs_drag.csv");
    let text = std::fs::read_to_string(&path).expect("read the PSBT reference fixture");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("Re,"))
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            Cell {
                re: v[0],
                kd_xx: v[1],
                alpha_structure: v[2],
                alpha_liquid: v[3],
                alpha_rho_mag_u_liquid: v[4],
                alpha_vapour: v[5],
            }
        })
        .collect()
}

/// **V&V — the ported `ReynoldsPower` wall friction reproduces upstream's own
/// PSBT drag field.**
///
/// Methodology, results and the reason only single-phase cells are used are in
/// the module documentation above.
#[test]
fn the_ported_wall_friction_reproduces_upstreams_psbt_drag() {
    let cells = load();
    assert_eq!(cells.len(), 1170, "fixture should carry every fluid cell");

    // Exactly what PSBT's phaseProperties declares for "liquid.structure".
    let blasius = FsWallFriction::ReynoldsPower {
        coeff: 0.316,
        exp: -0.25,
        constant: 0.0,
    };

    let mut compared = 0usize;
    let mut worst = 0.0_f64;
    let mut re_min = f64::INFINITY;
    let mut re_max = 0.0_f64;
    let mut worst_re = 0.0;

    for c in &cells {
        // The Lockhart-Martinelli two-phase multiplier scales Kd wherever vapour
        // is present, which the single-phase relation does not model.
        if c.alpha_vapour != 0.0 {
            continue;
        }
        let rho_mag_u = c.alpha_rho_mag_u_liquid / c.alpha_liquid;
        // G = 2 Kd / [(1 - alpha_s) rho|U|] = f / Dh
        let g = 2.0 * c.kd_xx / ((1.0 - c.alpha_structure) * rho_mag_u);
        let f = blasius
            .friction_factor(ReynoldsNumber::new::<ratio>(c.re))
            .get::<ratio>();
        let recovered_dh = f / g;

        let nearest = DECLARED_DH
            .iter()
            .copied()
            .min_by(|a, b| {
                (a - recovered_dh)
                    .abs()
                    .partial_cmp(&(b - recovered_dh).abs())
                    .unwrap()
            })
            .unwrap();
        let rel = (recovered_dh - nearest).abs() / nearest;
        if rel > worst {
            worst = rel;
            worst_re = c.re;
        }
        re_min = re_min.min(c.re);
        re_max = re_max.max(c.re);
        compared += 1;
    }

    println!(
        "\n=== PSBT wall friction: port vs upstream GeN-Foam run ===\n\
         \tcells compared      {compared} of {} (alpha.vapour == 0)\n\
         \tReynolds range      {re_min:.4e} .. {re_max:.4e}\n\
         \tworst |Dh_recovered - Dh_declared| / Dh_declared = {worst:.3e}  (at Re = {worst_re:.4e})",
        cells.len()
    );

    assert!(
        compared > 800,
        "expected most cells to be single-phase, got {compared}"
    );
    assert!(
        worst < 1.0e-5,
        "the port's friction factor does not reproduce upstream's drag: worst \
         recovered-Dh deviation {worst:.3e} at Re = {worst_re:.4e}"
    );
}

/// The two-phase cells must NOT satisfy the single-phase relation — upstream
/// applies a `LockhartMartinelli` multiplier there.
///
/// This is the control for the test above: it shows the 6e-6 agreement comes
/// from the friction closure being right, not from the relation being so loose
/// that anything satisfies it. Measured worst deviation across the two-phase
/// cells: **0.53**, five orders of magnitude larger.
#[test]
fn the_two_phase_cells_show_the_multiplier_the_single_phase_relation_omits() {
    let cells = load();
    let blasius = FsWallFriction::ReynoldsPower {
        coeff: 0.316,
        exp: -0.25,
        constant: 0.0,
    };
    let mut worst = 0.0_f64;
    let mut n = 0usize;
    for c in &cells {
        if c.alpha_vapour == 0.0 {
            continue;
        }
        let rho_mag_u = c.alpha_rho_mag_u_liquid / c.alpha_liquid;
        let g = 2.0 * c.kd_xx / ((1.0 - c.alpha_structure) * rho_mag_u);
        let f = blasius
            .friction_factor(ReynoldsNumber::new::<ratio>(c.re))
            .get::<ratio>();
        let dh = f / g;
        let nearest = DECLARED_DH
            .iter()
            .copied()
            .min_by(|a, b| (a - dh).abs().partial_cmp(&(b - dh).abs()).unwrap())
            .unwrap();
        worst = worst.max((dh - nearest).abs() / nearest);
        n += 1;
    }
    println!("two-phase cells: {n}, worst recovered-Dh deviation {worst:.3e}");
    assert!(n > 0, "the case should have two-phase cells at t = 5 s");
    assert!(
        worst > 1.0e-3,
        "the two-phase cells satisfy the single-phase relation to {worst:.3e}, \
         so the Lockhart-Martinelli multiplier is not present where expected — \
         which would mean the single-phase agreement proves nothing"
    );
}

// ---------------------------------------------------------------------------
// gh:#1361 round 3: the same comparison against a FRESH run of the real binary
// at writePrecision 17 (the committed CSV above carries 6 significant digits).
// Method, prediction Q1 (written before the run) and results:
// `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 3".
// ---------------------------------------------------------------------------

/// `name -> column` of a `COL` reference written by `scripts/extract_psbt_reference.sh`.
fn fresh_columns(time: u32) -> std::collections::HashMap<String, Vec<f64>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "verification_and_validation/genfoam_vs_upstream_gh1361/refs/psbt_psbt_ex1_t{time}.ref.txt.gz"
    ));
    let bytes = std::fs::read(&path).expect("reference present");
    // gzip -n: 10-byte header, 8-byte trailer.
    let text = String::from_utf8(
        miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate"),
    )
    .expect("utf8");
    let mut out = std::collections::HashMap::new();
    for line in text.lines() {
        let mut w = line.split_whitespace();
        if w.next() == Some("COL") {
            let name = w.next().expect("name").to_string();
            out.insert(name, w.map(|x| x.parse::<f64>().expect("f64")).collect());
        }
    }
    out
}

/// Distance in representable doubles.
fn ulp_distance(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        0
    } else if (a < 0.0) != (b < 0.0) {
        u64::MAX
    } else {
        a.to_bits().abs_diff(b.to_bits())
    }
}

/// Result of [`friction_check`].
struct FrictionCheck {
    cells: usize,
    single_phase: usize,
    bit_identical: usize,
    worst_rel: f64,
    worst_ulp: u64,
}

/// The recovered hydraulic diameter against the declared one over the single-phase cells of one
/// time of the fresh run.
fn friction_check(time: u32) -> FrictionCheck {
    let c = fresh_columns(time);
    let blasius = FsWallFriction::ReynoldsPower { coeff: 0.316, exp: -0.25, constant: 0.0 };
    let n = c["alpha.liquid"].len();
    for (k, v) in &c {
        assert_eq!(v.len(), n, "column {k}");
    }
    let mut out = FrictionCheck { cells: n, single_phase: 0, bit_identical: 0, worst_rel: 0.0, worst_ulp: 0 };
    for i in 0..n {
        if c["alpha.vapour"][i] != 0.0 {
            continue;
        }
        let rho_mag_u = c["alphaRhoMagU.liquid"][i] / c["alpha.liquid"][i];
        let g = 2.0 * c["Kd.liquid.structure"][i] / ((1.0 - c["alpha.structure"][i]) * rho_mag_u);
        let f = blasius
            .friction_factor(ReynoldsNumber::new::<ratio>(c["Re.liquid.structure"][i]))
            .get::<ratio>();
        let dh = f / g;
        let nearest = DECLARED_DH
            .iter()
            .copied()
            .min_by(|a, b| (a - dh).abs().partial_cmp(&(b - dh).abs()).unwrap())
            .unwrap();
        out.worst_rel = out.worst_rel.max((dh - nearest).abs() / nearest);
        let u = ulp_distance(dh, nearest);
        out.worst_ulp = out.worst_ulp.max(u);
        if u == 0 {
            out.bit_identical += 1;
        }
        out.single_phase += 1;
    }
    out
}

/// **V&V, round 3 — the ported `ReynoldsPower` friction against a fresh 17-digit run.**
#[test]
fn the_ported_wall_friction_reproduces_a_fresh_17_digit_psbt_run() {
    for time in [1_u32, 5] {
        let r = friction_check(time);
        println!(
            "t = {time} s: {} of {} cells single-phase; worst recovered-Dh deviation {:.3e}; \
             bit-identical to the declared Dh in {} cells; worst {} ulp",
            r.single_phase, r.cells, r.worst_rel, r.bit_identical, r.worst_ulp
        );
        assert_eq!(r.cells, 1170);
        assert!(r.single_phase > 800, "expected most cells single-phase, got {}", r.single_phase);
        // Prediction Q1 (< 1e-13) MISSED. t = 5 s (steady): 1.2e-11, i.e. the 6.15e-6 of the 6-digit CSV
        // above WAS its write precision. t = 1 s (transient): 8.0e-6, a different thing: `Kd` and
        // `Re` are evaluated at the start of an outer iteration, `alphaRhoMagU` at its end, and
        // the PIMPLE residualControl is 1e-5, so the three fields are not from one state
        // (README, Round 3). Pinned at the measurements.
        let bound = if time == 5 { 3e-11 } else { 1.5e-5 };
        assert!(r.worst_rel < bound, "t = {time} s: worst deviation {:.3e}", r.worst_rel);
    }
}
