// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Writes the input format of GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/XS.{C,H} (the `states`/`zones`
//                    nuclearData dictionary schema) and the layout of the
//                    tutorials' `constant/neutroRegion/nuclearData` files
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # Writing a GeN-Foam `nuclearData` dictionary
//!
//! The inverse of [`super::read_nuclear_data`]: a
//! [`NuclearDataInput`] written as the text a GeN-Foam case reads, in the
//! layout of upstream's tutorial files (`key nonuniform List<scalar> N ( … );`
//! per group array, `scatteringMatrixP<m> G G ( ( … ) … );` per moment).
//! Values are written as given (MKSA, per metre): nothing is converted.
//!
//! The reference state's zones carry the constants block
//! ([`ZoneConstantsInput`]); perturbed states carry the cross sections only,
//! because upstream reads the constants from the reference state alone.
//!
//! Verified by round trip: `write -> read_nuclear_data` returns the same
//! input (`tests::written_nuclear_data_reads_back_identically`).

use std::fmt::Write as _;

use crate::genfoam::neutronics::xs::input::{NuclearDataInput, ZoneConstantsInput, ZoneStateInput};

fn list(out: &mut String, key: &str, v: &[f64]) {
    let _ = write!(
        out,
        "                {key} nonuniform List<scalar> {} (",
        v.len()
    );
    for x in v {
        let _ = write!(out, " {x:.9e}");
    }
    out.push_str(" );\n");
}

fn zone(out: &mut String, z: &ZoneStateInput, constants: Option<&ZoneConstantsInput>) {
    let _ = writeln!(out, "            {}\n            {{", z.name);
    if let Some(c) = constants {
        let _ = writeln!(out, "                fuelFraction {:.9e};", c.fuel_fraction);
        let _ = writeln!(
            out,
            "                secondaryPowerVolumeFraction {:.9e};",
            c.secondary_power_volume_fraction
        );
        let _ = writeln!(
            out,
            "                fractionToSecondaryPower {:.9e};",
            c.fraction_to_secondary_power
        );
        let _ = writeln!(out, "                dfAdjust {};", c.df_adjust);
        list(out, "IV", &c.iv);
    }
    list(out, "D", &z.d);
    list(out, "nuSigmaEff", &z.nu_sigma_eff);
    list(out, "sigmaPow", &z.sigma_pow);
    for (m, mat) in z.scattering.iter().enumerate() {
        let g = mat.len();
        let _ = writeln!(out, "                scatteringMatrixP{m} {g} {g} (");
        for row in mat {
            out.push_str("                    (");
            for x in row {
                let _ = write!(out, " {x:.9e}");
            }
            out.push_str(" )\n");
        }
        out.push_str("                );\n");
    }
    list(out, "sigmaRemoval", &z.sigma_removal);
    list(out, "chiPrompt", &z.chi_prompt);
    list(out, "chiDelayed", &z.chi_delayed);
    if let Some(c) = constants {
        list(out, "Beta", &c.beta);
        list(out, "lambda", &c.lambda);
        list(out, "discFactor", &c.disc_factor);
        list(out, "integralFlux", &c.integral_flux);
    }
    out.push_str("            }\n");
}

/// The `nuclearData` dictionary text for `input`, with `header` written as
/// a `//` comment block under the `FoamFile` banner (provenance, caveats).
#[must_use]
pub fn write_nuclear_data(input: &NuclearDataInput, header: &str) -> String {
    let mut out = String::new();
    out.push_str("FoamFile\n{\n    version 2.0;\n    format ascii;\n    class dictionary;\n    location \"constant/neutroRegion\";\n    object nuclearData;\n}\n");
    for line in header.lines() {
        let _ = writeln!(out, "// {line}");
    }
    out.push('\n');
    let _ = writeln!(out, "fastNeutrons {};", input.fast_neutrons);
    let _ = writeln!(out, "energyGroups {};", input.energy_groups);
    let _ = writeln!(out, "precGroups {};", input.prec_groups);
    let _ = writeln!(out, "polyharmonicSplineMode {};", input.poly_spline_mode);
    if !input.do_not_parametrize.is_empty() {
        out.push_str("doNotParametrize (");
        for g in &input.do_not_parametrize {
            let _ = write!(out, " {g}");
        }
        out.push_str(" );\n");
    }
    if !input.xs_variables.is_empty() {
        out.push_str("xsVariables\n{\n");
        for v in &input.xs_variables {
            let _ = writeln!(out, "    {} {};", v.name, v.law.keyword());
        }
        out.push_str("}\n");
    }
    out.push_str("\nstates\n(\n");
    // The reference state's constants, by zone name, written in the reference
    // state only.
    for (si, s) in input.states.iter().enumerate() {
        let _ = writeln!(out, "    {}\n    {{", s.name);
        for (k, v) in &s.parameters {
            let _ = writeln!(out, "        {k} {v:.9e};");
        }
        out.push_str("        zones\n        (\n");
        for z in &s.zones {
            let c = if si == 0 { z.constants.as_ref() } else { None };
            zone(&mut out, z, c);
        }
        out.push_str("        );\n    }\n");
    }
    out.push_str(
        ");\n\n// ************************************************************************* //\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genfoam::neutronics::xs::input::StateInput;
    use crate::genfoam::neutronics::xs::variables::{VariableLaw, XsVariable};
    use std::collections::BTreeMap;

    fn zone_at(name: &str, f: f64, constants: bool) -> ZoneStateInput {
        ZoneStateInput {
            name: name.into(),
            d: vec![0.012 * f, 0.009 * f],
            nu_sigma_eff: vec![0.31 * f, 4.1 * f],
            sigma_pow: vec![1.2e-12 * f, 5.0e-11 * f],
            sigma_removal: vec![2.5 * f, 9.75 * f],
            chi_prompt: vec![1.0, 0.0],
            chi_delayed: vec![1.0, 0.0],
            scattering: vec![vec![vec![30.0 * f, 2.0 * f], vec![0.01 * f, 40.0 * f]]],
            constants: constants.then(|| ZoneConstantsInput {
                iv: vec![5.0e-8, 2.0e-4],
                disc_factor: vec![1.0, 1.0],
                integral_flux: vec![0.7, 0.3],
                beta: vec![],
                lambda: vec![],
                ..ZoneConstantsInput::default()
            }),
        }
    }

    /// Two states (ln T), two zones, no delayed groups: written, read back
    /// with the upstream-schema reader, identical to 1e-9 relative.
    #[test]
    fn written_nuclear_data_reads_back_identically() {
        let mut p0 = BTreeMap::new();
        p0.insert("TFuel".to_string(), 300.15);
        let mut p1 = BTreeMap::new();
        p1.insert("TFuel".to_string(), 600.0);
        let input = NuclearDataInput {
            energy_groups: 2,
            prec_groups: 0,
            legendre_moments: 1,
            poly_spline_mode: 1,
            fast_neutrons: false,
            xs_variables: vec![XsVariable::new("TFuel", VariableLaw::Log)],
            do_not_parametrize: vec![],
            states: vec![
                StateInput {
                    name: "reference".into(),
                    parameters: p0,
                    zones: vec![zone_at("bed", 1.0, true), zone_at("refl", 0.5, true)],
                },
                StateInput {
                    name: "T600".into(),
                    parameters: p1,
                    zones: vec![zone_at("bed", 1.1, false), zone_at("refl", 0.55, false)],
                },
            ],
        };
        let text = write_nuclear_data(&input, "round-trip test");
        let dir = std::env::temp_dir().join(format!("op_nd_writer_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nuclearData");
        std::fs::write(&path, &text).unwrap();
        let back = super::super::read_nuclear_data(&path).unwrap_or_else(|e| panic!("{e}\n{text}"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back.energy_groups, 2);
        assert_eq!(back.prec_groups, 0);
        assert_eq!(back.states.len(), 2);
        assert_eq!(back.states[1].parameters["TFuel"], 600.0);
        assert_eq!(back.xs_variables[0].law, VariableLaw::Log);
        let close = |a: &[f64], b: &[f64]| {
            a.iter()
                .zip(b)
                .all(|(x, y)| (x - y).abs() <= 1e-9 * x.abs().max(1e-30))
        };
        for (s, t) in input.states.iter().zip(&back.states) {
            for (z, w) in s.zones.iter().zip(&t.zones) {
                assert_eq!(z.name, w.name);
                assert!(close(&z.d, &w.d) && close(&z.nu_sigma_eff, &w.nu_sigma_eff));
                assert!(
                    close(&z.sigma_removal, &w.sigma_removal) && close(&z.sigma_pow, &w.sigma_pow)
                );
                assert!(
                    close(&z.scattering[0][0], &w.scattering[0][0])
                        && close(&z.scattering[0][1], &w.scattering[0][1])
                );
            }
        }
        let c = back.states[0].zones[0]
            .constants
            .as_ref()
            .expect("reference constants");
        assert!(close(&c.iv, &[5.0e-8, 2.0e-4]));
        // And the parsed input builds the solver's cross-section data.
        crate::genfoam::neutronics::xs::CrossSectionData::from_input(&back)
            .expect("CrossSectionData");
    }
}
