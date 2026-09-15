// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! Unit tests for the `nuclearData` reader, on synthetic dictionaries.
//!
//! The reader is exercised against *real* upstream GeN-Foam tutorial files in
//! `tests/genfoam_tutorial_nuclear_data.rs`, which skips when the upstream
//! clone is absent. The tests here use small hand-written dictionaries so they
//! run everywhere and pin the schema, the defaults, and the error messages.

use super::*;
use crate::genfoam::neutronics::xs::CrossSectionData;

/// A minimal but complete two-group / two-precursor / one-zone dictionary in
/// the exact shape upstream writes.
fn minimal() -> &'static str {
    r#"
FoamFile { version 2.0; format ascii; class dictionary; object nuclearData; }

fastNeutrons    true;
energyGroups    2;
precGroups      2;

xsVariables
{
    TFuel   log;
    rhoCool lin;
}

states
(
    reference
    {
        TFuel   900;
        rhoCool 4125;
        zones
        (
            core
            {
                fuelFraction 1.0;
                IV nonuniform List<scalar> 2 (1.0e-07 5.0e-06 );
                D nonuniform List<scalar> 2 (2.0e-02 1.0e-02 );
                nuSigmaEff nonuniform List<scalar> 2 (5.0e-01 1.5e+00 );
                sigmaPow nonuniform List<scalar> 2 (6.0e-12 2.0e-11 );
                scatteringMatrixP0 2 2 (
                    ( 1.0e+01 5.0e+00 )
                    ( 0.0e+00 2.0e+01 )
                );
                sigmaRemoval nonuniform List<scalar> 2 (6.0e+00 3.5e+00 );
                chiPrompt nonuniform List<scalar> 2 (0.9 0.1 );
                chiDelayed nonuniform List<scalar> 2 (0.5 0.5 );
                Beta nonuniform List<scalar> 2 (2.0e-04 4.0e-04 );
                lambda nonuniform List<scalar> 2 (1.2e-02 3.0e-01 );
                discFactor nonuniform List<scalar> 2 (1 1 );
                integralFlux nonuniform List<scalar> 2 (1 1 );
            }
        );
    }
)
;
"#
}

/// Write `text` to a scratch file and read it back through the public entry
/// point, so the include-resolving path is the one under test.
fn read_str(text: &str) -> Result<NuclearDataInput, AppBuilderError> {
    let dir = std::env::temp_dir().join(format!(
        "op_nuclear_data_test_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("nuclearData");
    std::fs::write(&path, text).unwrap();
    let out = read_nuclear_data(&path);
    let _ = std::fs::remove_dir_all(&dir);
    out
}

#[test]
fn the_minimal_dictionary_round_trips_into_cross_section_data() {
    let input = read_str(minimal()).expect("parse");

    assert_eq!(input.energy_groups, 2);
    assert_eq!(input.prec_groups, 2);
    assert_eq!(input.legendre_moments, 1, "only P0 is declared");
    assert_eq!(input.poly_spline_mode, 1, "upstream default");
    assert!(input.fast_neutrons);
    assert_eq!(input.xs_variables.len(), 2);
    assert_eq!(input.xs_variables[0].name, "TFuel");
    assert_eq!(input.xs_variables[0].law, VariableLaw::Log);
    assert_eq!(input.xs_variables[1].law, VariableLaw::Linear);
    assert!(input.do_not_parametrize.is_empty());

    assert_eq!(input.states.len(), 1);
    let s = &input.states[0];
    assert_eq!(s.name, "reference");
    assert_eq!(s.parameters["TFuel"], 900.0);
    assert_eq!(s.parameters["rhoCool"], 4125.0);
    assert_eq!(s.zones.len(), 1);

    let z = &s.zones[0];
    assert_eq!(z.name, "core");
    assert_eq!(z.d, vec![2.0e-2, 1.0e-2]);
    assert_eq!(z.nu_sigma_eff, vec![5.0e-1, 1.5]);
    assert_eq!(z.scattering.len(), 1);
    // `m[from][to]`: the file's first row is group 0's outgoing scattering.
    assert_eq!(z.scattering[0], vec![vec![1.0e1, 5.0], vec![0.0, 2.0e1]]);

    let c = z
        .constants
        .as_ref()
        .expect("reference zone carries constants");
    assert_eq!(c.fuel_fraction, 1.0);
    assert_eq!(c.secondary_power_volume_fraction, 1.0, "upstream default");
    assert_eq!(c.fraction_to_secondary_power, 0.0, "upstream default");
    assert!(c.df_adjust, "upstream default");
    assert_eq!(c.beta, vec![2.0e-4, 4.0e-4]);
    assert_eq!(c.lambda, vec![1.2e-2, 3.0e-1]);

    // The point of the reader: the result must satisfy the consumer.
    let xs = CrossSectionData::from_input(&input).expect("build CrossSectionData");
    assert_eq!(xs.energy_groups(), 2);
    assert_eq!(xs.prec_groups(), 2);
    assert_eq!(xs.zone_count(), 1);
    assert_eq!(xs.zone_index("core"), Some(0));
}

#[test]
fn a_perturbed_state_inherits_the_reference_parameters_it_omits() {
    // Upstream's ESFR perturbed states restate only the variable they perturb.
    let text = minimal().replace(
        "\n)\n;",
        r#"
    TFuel1200K
    {
        TFuel 1200;
        zones
        (
            core
            {
                D nonuniform List<scalar> 2 (2.1e-02 1.1e-02 );
                nuSigmaEff nonuniform List<scalar> 2 (5.1e-01 1.6e+00 );
                sigmaPow nonuniform List<scalar> 2 (6.1e-12 2.1e-11 );
                scatteringMatrixP0 2 2 (
                    ( 1.1e+01 5.1e+00 )
                    ( 0.0e+00 2.1e+01 )
                );
                sigmaRemoval nonuniform List<scalar> 2 (6.1e+00 3.6e+00 );
                chiPrompt nonuniform List<scalar> 2 (0.9 0.1 );
                chiDelayed nonuniform List<scalar> 2 (0.5 0.5 );
            }
        );
    }
)
;"#,
    );
    let input = read_str(&text).expect("parse");
    assert_eq!(input.states.len(), 2);
    let p = &input.states[1];
    assert_eq!(p.name, "TFuel1200K");
    assert_eq!(p.parameters["TFuel"], 1200.0, "restated");
    assert_eq!(
        p.parameters["rhoCool"], 4125.0,
        "inherited from the reference state, which is why upstream may omit it"
    );
    assert!(
        p.zones[0].constants.is_none(),
        "feedback-independent constants are read from the reference state only"
    );
}

#[test]
fn an_include_is_spliced_relative_to_the_including_file() {
    let dir = std::env::temp_dir().join(format!("op_nuclear_data_inc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let zone = r#"
            core
            {
                fuelFraction 1.0;
                IV nonuniform List<scalar> 2 (1.0e-07 5.0e-06 );
                D nonuniform List<scalar> 2 (2.0e-02 1.0e-02 );
                nuSigmaEff nonuniform List<scalar> 2 (5.0e-01 1.5e+00 );
                sigmaPow nonuniform List<scalar> 2 (6.0e-12 2.0e-11 );
                scatteringMatrixP0 2 2 (
                    ( 1.0e+01 5.0e+00 )
                    ( 0.0e+00 2.0e+01 )
                );
                sigmaRemoval nonuniform List<scalar> 2 (6.0e+00 3.5e+00 );
                chiPrompt nonuniform List<scalar> 2 (0.9 0.1 );
                chiDelayed nonuniform List<scalar> 2 (0.5 0.5 );
                Beta nonuniform List<scalar> 2 (2.0e-04 4.0e-04 );
                lambda nonuniform List<scalar> 2 (1.2e-02 3.0e-01 );
                discFactor nonuniform List<scalar> 2 (1 1 );
                integralFlux nonuniform List<scalar> 2 (1 1 );
            }
"#;
    std::fs::write(dir.join("XSref"), zone).unwrap();
    // Replace the inline zone body with an include of the file just written.
    let start = minimal().find("            core").unwrap();
    let end = minimal().find("        );\n    }\n)").unwrap();
    let text = format!(
        "{}            #include \"XSref\"\n{}",
        &minimal()[..start],
        &minimal()[end..]
    );
    std::fs::write(dir.join("nuclearData"), &text).unwrap();

    let input = read_nuclear_data(&dir.join("nuclearData")).expect("parse with include");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(input.states[0].zones.len(), 1);
    assert_eq!(input.states[0].zones[0].name, "core");
    assert_eq!(input.states[0].zones[0].d, vec![2.0e-2, 1.0e-2]);
}

#[test]
fn a_wrong_array_length_is_rejected_where_the_file_name_is_still_known() {
    // `D` given 3 values in a 2-group case. Left unchecked this surfaces much
    // later as a shape mismatch inside the cross-section layer, with no file.
    let text = minimal().replace(
        "D nonuniform List<scalar> 2 (2.0e-02 1.0e-02 );",
        "D nonuniform List<scalar> 3 (2.0e-02 1.0e-02 3.0e-02 );",
    );
    let err = read_str(&text).expect_err("must reject");
    let msg = err.to_string();
    assert!(msg.contains("nuclearData"), "names the file: {msg}");
    assert!(msg.contains("zone `core`"), "names the zone: {msg}");
    assert!(msg.contains('D'), "names the key: {msg}");
    assert!(
        msg.contains("expected 2 values, found 3"),
        "states both counts: {msg}"
    );
}

#[test]
fn a_first_state_that_is_not_reference_is_rejected() {
    let text = minimal().replace("    reference\n", "    hotFullPower\n");
    let err = read_str(&text).expect_err("must reject");
    assert!(
        err.to_string().contains("must be named `reference`"),
        "{err}"
    );
}

#[test]
fn an_unknown_variable_law_is_rejected_by_name() {
    let text = minimal().replace("TFuel   log;", "TFuel   cubic;");
    let err = read_str(&text).expect_err("must reject");
    let msg = err.to_string();
    assert!(msg.contains("cubic"), "names the offending law: {msg}");
    assert!(
        msg.contains("lin, log or sqrt"),
        "names the valid set: {msg}"
    );
}
