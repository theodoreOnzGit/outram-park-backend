//! Genuine PFLOTRAN regression decks against the shipped `.regression.gold`
//! files (gh:#1537, rows 3-5 of gh:#1422).
//!
//! # Methodology
//!
//! * Deck: `regression_tests/default/infiltration1d/infiltration1d.in` at
//!   PFLOTRAN 669fbce (verbatim copy under
//!   `verification_and_validation/upstream_differential_gh1422/regression_decks/`,
//!   LGPL-3.0, see `PROVENANCE.md` there).
//! * Reference: `infiltration1d.regression.gold`, which is real PFLOTRAN output
//!   (and was reproduced bit for bit from a binary built from 669fbce in round 1,
//!   see `../verification_and_validation/upstream_differential_gh1422/README.md`).
//! * Predictions P17-P24 were committed BEFORE the first port run
//!   (commit `ac0228be4`); the bounds asserted here are those, unchanged.
//! * Pass criterion: the predicted step sequence and Newton count, and the
//!   per-quantity bounds of P22/P23.
//!
//! Results are recorded in the README of the differential directory.

use outram_park_fork_pflotran::deck::regression::{parse_regression_deck, RegressionFlowValue};
use outram_park_fork_pflotran::flow::regression_run::run_regression_deck;

const DECK: &str = include_str!(
    "../verification_and_validation/upstream_differential_gh1422/regression_decks/default/infiltration1d/infiltration1d.in"
);
const GOLD: &str = include_str!(
    "../verification_and_validation/upstream_differential_gh1422/regression_decks/default/infiltration1d/infiltration1d.regression.gold"
);

/// One `-- NAME --` section of a `.regression.gold` file.
struct GoldSection {
    name: String,
    entries: Vec<(String, f64)>,
}

fn parse_gold(text: &str) -> Vec<GoldSection> {
    let mut out: Vec<GoldSection> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("--") && t.ends_with("--") {
            out.push(GoldSection {
                name: t.trim_matches('-').trim().to_string(),
                entries: Vec::new(),
            });
        } else if let Some((k, v)) = t.split_once(':') {
            if let (Some(sec), Ok(val)) = (out.last_mut(), v.trim().replace('E', "e").parse::<f64>()) {
                sec.entries.push((k.trim().to_string(), val));
            }
        }
    }
    out
}

fn gold_value(g: &[GoldSection], section: &str, key: &str) -> f64 {
    g.iter()
        .find(|s| s.name.starts_with(section))
        .and_then(|s| s.entries.iter().find(|(k, _)| k == key))
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("gold has no {section} / {key}"))
}

#[test]
fn p17_deck_reader_reads_the_whole_infiltration1d_deck() {
    let d = parse_regression_deck(DECK).expect("infiltration1d.in must parse");
    assert_eq!((d.grid.nx, d.grid.ny, d.grid.nz), (40, 1, 1));
    assert!(d.grid.dx.iter().all(|w| *w == 0.005));
    assert_eq!(d.regions.len(), 4);
    assert_eq!(d.flow_conditions.len(), 2);
    assert_eq!(d.regression_cell_ids, vec![5, 15, 25, 35]);
    assert_eq!(d.output_times_s, vec![0.0, 864.0, 5184.0, 9504.0]);
    assert_eq!(d.time.final_time_s, 9504.0);
    assert_eq!(d.time.max_dt_s, Some(1000.0));
    assert_eq!(d.time.initial_dt_s, None);
    assert_eq!(d.eos_water.density, Some(997.16));
    assert_eq!(d.eos_water.viscosity, Some(1.0e-3));
    assert_eq!(d.initial_conditions.len(), 1);
    assert_eq!(d.boundary_conditions.len(), 1);
    assert_eq!(
        d.flow_conditions[1].value,
        RegressionFlowValue::LiquidSaturation(0.4444)
    );
    assert_eq!(d.strata[0].material, "soil");
}

#[test]
fn deck_reader_rejects_unsupported_cards_and_honours_skip() {
    let bad = DECK.replace(
        "MODE RICHARDS",
        "MODE RICHARDS\n      OPTIONS\n        STEADY_STATE\n      /",
    );
    assert!(parse_regression_deck(&bad).is_err(), "OPTIONS must be refused");
    let bad = DECK.replace("TORTUOSITY 1.0d0", "TORTUOSITY 1.0d0\n  ROCK_DENSITY 870.");
    assert!(parse_regression_deck(&bad).is_err());
    // SKIP ... NOSKIP drops the enclosed (otherwise unsupported) block.
    let skipped = DECK.replace(
        "OUTPUT\n",
        "SKIP\nBOGUS_BLOCK\n  FOO 1\nEND\nNOSKIP\nOUTPUT\n",
    );
    assert!(parse_regression_deck(&skipped).is_ok());
    assert!(parse_regression_deck("SIMULATION\nEND\n").is_err());
}

#[test]
fn infiltration1d_against_gold() {
    let deck = parse_regression_deck(DECK).unwrap();
    let gold = parse_gold(GOLD);
    let r = run_regression_deck(&deck).expect("infiltration1d must run");

    // P19: initial pressure.
    println!("initial pressure {:.4} Pa", r.initial_pressure);
    // P19 as committed said 93151.6 +- 0.1 Pa. That MISSED: the hand
    // arithmetic rounded Se to 0.1666 (exact 0.16660004167). The formula was
    // right; the closed form below (evaluated independently with python,
    // 93151.84660865767) is what is asserted, and the miss is recorded in the README.
    assert!((r.initial_pressure - 93151.846_608_657_67).abs() < 1.0e-9, "P19");
    assert!(gold_value(&gold, "PRESSURE", "Min") > r.initial_pressure, "P19 ordering");

    // P20: dt sequence and step count.
    println!("dt history {:?}", r.dt_history);
    println!("newton per step {:?}", r.newton_per_step);
    let expect_dt = [
        1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 353.0, 512.0, 1000.0, 1000.0, 1000.0,
        808.0, 1000.0, 1000.0, 1000.0, 1000.0, 320.0,
    ];
    assert_eq!(r.dt_history.len(), expect_dt.len(), "P20 step count");
    for (i, (a, b)) in r.dt_history.iter().zip(expect_dt.iter()).enumerate() {
        assert!((a - b).abs() < 1.0e-9, "P20 dt[{i}] = {a}, predicted {b}");
    }
    assert_eq!(r.final_time, 9504.0);
    assert_eq!(r.time_steps as f64, gold_value(&gold, "SOLUTION", "Time Steps"));
    assert_eq!(r.time_step_cuts as f64, gold_value(&gold, "SOLUTION", "Time Step Cuts"));

    // P21: Newton iterations.
    println!(
        "newton iterations {} (gold {})",
        r.newton_iterations,
        gold_value(&gold, "SOLUTION", "Newton Iterations")
    );
    assert_eq!(
        r.newton_iterations as f64,
        gold_value(&gold, "SOLUTION", "Newton Iterations"),
        "P21"
    );

    // P22 / P23 per-quantity table.
    let stats = |v: &[f64]| {
        let max = v.iter().cloned().fold(f64::MIN, f64::max);
        let min = v.iter().cloned().fold(f64::MAX, f64::min);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        (max, min, mean)
    };
    let (pmax, pmin, pmean) = stats(&r.pressure);
    let (smax, smin, smean) = stats(&r.saturation);
    let mut rows: Vec<(String, f64, f64, f64)> = vec![
        ("P Max".into(), pmax, gold_value(&gold, "PRESSURE", "Max"), 1.0e-3),
        ("P Min".into(), pmin, gold_value(&gold, "PRESSURE", "Min"), 1.0e-3),
        ("P Mean".into(), pmean, gold_value(&gold, "PRESSURE", "Mean"), 1.0e-3),
        ("S Max".into(), smax, gold_value(&gold, "SATURATION", "Max"), 7.0e-8),
        ("S Min".into(), smin, gold_value(&gold, "SATURATION", "Min"), 7.0e-8),
        ("S Mean".into(), smean, gold_value(&gold, "SATURATION", "Mean"), 7.0e-8),
    ];
    for id in &deck.regression_cell_ids {
        let key = id.to_string();
        rows.push((
            format!("P cell {id}"),
            r.pressure[id - 1],
            gold_value(&gold, "PRESSURE", &key),
            1.0e-3,
        ));
        rows.push((
            format!("S cell {id}"),
            r.saturation[id - 1],
            gold_value(&gold, "SATURATION", &key),
            7.0e-8,
        ));
    }
    let norm = r.pressure.iter().map(|p| p * p).sum::<f64>().sqrt();
    rows.push((
        "Solution 2-Norm".into(),
        norm,
        gold_value(&gold, "SOLUTION", "Solution 2-Norm"),
        6.4e-3,
    ));
    println!("{:<18}{:>22}{:>22}{:>12}{:>12}", "quantity", "port", "gold", "diff", "bound");
    for (name, port, g, bound) in &rows {
        println!("{name:<18}{port:>22.13e}{g:>22.13e}{:>12.3e}{bound:>12.1e}", port - g);
    }
    for (name, port, g, bound) in &rows {
        assert!((port - g).abs() <= *bound, "{name}: port {port} gold {g} bound {bound}");
    }
}

/// A small saturated-to-unsaturated van Genuchten deck with pressure
/// conditions: exercises the VAN_GENUCHTEN/MUALEM_VG_LIQ reader paths, the
/// BOTTOM/TOP/NORTH face mapping and a run that needs no saturation conversion.
const VG_DECK: &str = "
SIMULATION
  SIMULATION_TYPE SUBSURFACE
  PROCESS_MODELS
    SUBSURFACE_FLOW flow
      MODE RICHARDS
    /
  /
END
SUBSURFACE
GRID
  TYPE structured
  NXYZ 10 1 1
  BOUNDS
    0.d0 0.d0 0.d0
    1.d0 1.d0 1.d0
  /
END
MATERIAL_PROPERTY soil
  ID 1
  POROSITY 0.4
  PERMEABILITY
    PERM_ISO 1.d-12
  /
  CHARACTERISTIC_CURVES cc
END
CHARACTERISTIC_CURVES cc
  SATURATION_FUNCTION VAN_GENUCHTEN
    M 0.5
    ALPHA 1.d-4
    LIQUID_RESIDUAL_SATURATION 0.1
    MAX_CAPILLARY_PRESSURE 1.d8
  /
  PERMEABILITY_FUNCTION MUALEM_VG_LIQ
    M 0.5
    LIQUID_RESIDUAL_SATURATION 0.1
  /
END
EOS WATER
  DENSITY CONSTANT 997.16d0
  VISCOSITY CONSTANT 1.d-3
END
OUTPUT
  FORMAT TECPLOT POINT
  TIMES s 50
END
TIME
  FINAL_TIME 100 s
  INITIAL_TIMESTEP_SIZE 1.d0 s
  MAXIMUM_TIMESTEP_SIZE 20 s
END
REGION all
  COORDINATES
    0.d0 0.d0 0.d0
    1.d0 1.d0 1.d0
  /
END
REGION west
  FACE WEST
  COORDINATES
    0.d0 0.d0 0.d0
    0.d0 1.d0 1.d0
  /
END
FLOW_CONDITION wet
  TYPE
    LIQUID_PRESSURE DIRICHLET
  /
  LIQUID_PRESSURE 111325.d0
END
FLOW_CONDITION dry
  TYPE
    LIQUID_PRESSURE DIRICHLET
  /
  LIQUID_PRESSURE 91325.d0
END
INITIAL_CONDITION
  FLOW_CONDITION dry
  REGION all
END
BOUNDARY_CONDITION inlet
  FLOW_CONDITION wet
  REGION west
END
STRATA
  REGION all
  MATERIAL soil
END
END_SUBSURFACE
";

#[test]
fn van_genuchten_pressure_deck_runs_and_unsupported_decks_are_refused() {
    let deck = parse_regression_deck(VG_DECK).unwrap();
    assert_eq!(deck.boundary_conditions[0].name.as_deref(), Some("inlet"));
    let r = run_regression_deck(&deck).unwrap();
    assert_eq!(r.final_time, 100.0);
    assert!(r.time_steps > 5 && r.newton_iterations >= r.time_steps);
    // wetting front enters from the west: pressure decreases monotonically eastwards
    assert!(r.pressure[0] > r.pressure[9]);
    assert!(r.pressure.windows(2).all(|w| w[0] >= w[1] - 1.0e-6));
    // no EOS block: upstream's default IFC67 density is not translated
    let no_eos = VG_DECK.replace("DENSITY CONSTANT 997.16d0", "");
    assert!(run_regression_deck(&parse_regression_deck(&no_eos).unwrap()).is_err());
    // saturation condition with a non-LINEAR saturation function
    let sat_ic = VG_DECK.replace("LIQUID_PRESSURE DIRICHLET\n  /\n  LIQUID_PRESSURE 91325.d0", "LIQUID_SATURATION DIRICHLET\n  /\n  LIQUID_SATURATION 0.5");
    assert!(run_regression_deck(&parse_regression_deck(&sat_ic).unwrap()).is_err());
    // non-uniform widths
    let mut skew = parse_regression_deck(VG_DECK).unwrap();
    skew.grid.dx[0] *= 2.0;
    assert!(run_regression_deck(&skew).is_err());
    // two strata
    let mut two = parse_regression_deck(VG_DECK).unwrap();
    two.strata.push(two.strata[0].clone());
    assert!(run_regression_deck(&two).is_err());
}

#[test]
fn linear_model_validation_and_slopes() {
    assert!(Linear::new(0.0, 9807.0, 0.3).is_err());
    assert!(Linear::new(1.0e30, 1.0e-31, 0.3).is_err());
    assert!(Linear::new(1.0e30, 9807.0, 1.0).is_err());
    let c = CharacteristicCurves::Linear(Linear::new(1.0e30, 9807.0, 0.3333333).unwrap());
    let half = Ratio::new::<ratio>(0.5);
    assert_eq!(c.d_kr_d_se(half), 1.0);
    assert_eq!(c.d_kr_d_se(Ratio::new::<ratio>(1.0)), 0.0);
    let slope = c.d_se_d_pc(Pressure::new::<pascal>(100.0));
    assert!((slope + 1.0 / 9807.0).abs() < 1.0e-18);
    assert_eq!(c.d_se_d_pc(Pressure::new::<pascal>(-1.0)), 0.0);
    // saturation at/below Sr gives pcmax, at/above 1 gives 0
    let l = Linear::new(1.0e30, 9807.0, 0.3333333).unwrap();
    assert_eq!(l.capillary_pressure(0.3), 9807.0);
    assert_eq!(l.capillary_pressure(1.0), 0.0);
}

// ---------------------------------------------------------------------------
// P18: LINEAR saturation function and Burdine-linear kr against the verbatim
// upstream kernel (verification_and_validation/.../linear_kernel.F90, compiled
// with gfortran -O2 -mno-fma; check_verbatim_linear.sh: 39/39 statements are
// byte-for-byte in characteristic_curves_common.F90 at 669fbce).
// ---------------------------------------------------------------------------

use outram_park_fork_pflotran::properties::{CharacteristicCurves, Linear};
use uom::si::f64::{Pressure, Ratio};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

const LINEAR_OUTPUT: &str = include_str!(
    "../verification_and_validation/upstream_differential_gh1422/linear_output.txt"
);

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex double"))
}

#[test]
fn p18_linear_curves_bit_identical_to_upstream_kernel() {
    let mut set: Option<(f64, f64, f64)> = None;
    let mut compared = 0usize;
    let mut identical = 0usize;
    let mut saturated_mismatch = 0usize;
    let mut cap_n = 0usize;
    let mut cap_identical = 0usize;
    for line in LINEAR_OUTPUT.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[0] == "SET" {
            set = Some((hex(f[1]), hex(f[2]), hex(f[3])));
            continue;
        }
        let (sr, alpha, pcmax) = set.expect("SET precedes data");
        let lin = Linear::new(alpha, pcmax, sr).unwrap();
        let curves = CharacteristicCurves::Linear(lin);
        let a = hex(f[1]);
        match f[0] {
            // RichardsAuxVarCompute truncates pc at pcmax, never evaluates beyond.
            "SAT" | "CHN" if a > pcmax => {}
            "SAT" => {
                let se = curves
                    .effective_saturation(Pressure::new::<pascal>(a))
                    .get::<ratio>();
                let sl_port = sr + (1.0 - sr) * se;
                // `+ 0.0` turns a signed zero into +0 (upstream sets the literal 0.d0 there).
                let dsat_port = (1.0 - sr) * curves.d_se_d_pc(Pressure::new::<pascal>(a)) * (-1.0) + 0.0;
                let (sl_up, dsat_up) = (hex(f[2]), hex(f[3]));
                compared += 2;
                if sl_port.to_bits() == sl_up.to_bits() {
                    identical += 1;
                } else if a <= 0.0 {
                    saturated_mismatch += 1;
                    println!("saturated pc {a}: port {sl_port:e} upstream {sl_up:e}");
                } else {
                    panic!("Sl differs at pc {a}: {sl_port:e} vs {sl_up:e}");
                }
                assert_eq!(dsat_port.to_bits(), dsat_up.to_bits(), "dsat at pc {a}");
                identical += 1;
            }
            "CHN" => {
                let se = curves
                    .effective_saturation(Pressure::new::<pascal>(a))
                    .get::<ratio>();
                let kr_port = curves.relative_permeability(Ratio::new::<ratio>(se)).get::<ratio>();
                compared += 1;
                assert_eq!(kr_port.to_bits(), hex(f[2]).to_bits(), "kr at pc {a}");
                identical += 1;
            }
            "CAP" => {
                cap_n += 1;
                assert_eq!(lin.capillary_pressure(a).to_bits(), hex(f[2]).to_bits(), "pc at Sl {a}");
                cap_identical += 1;
            }
            "KR" => {}
            other => panic!("unknown record {other}"),
        }
    }
    println!(
        "P18: Sl/dSl/kr {identical}/{compared} bit-identical ({saturated_mismatch} saturated-branch Sl differ), capillary pressure {cap_identical}/{cap_n}"
    );
    assert!(compared > 300 && cap_n > 100);
    assert_eq!(cap_identical, cap_n);
    assert_eq!(identical + saturated_mismatch, compared);
    assert_eq!(saturated_mismatch, 0, "P18: bit-identical on every point");
}
