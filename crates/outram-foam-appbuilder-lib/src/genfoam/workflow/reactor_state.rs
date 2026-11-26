// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/neutronics.C (reactorState_: `keff`, `pTarget`, read with
//                    lookupOrDefault 1.0 and written AUTO_WRITE to <time>/uniform/reactorState)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version. See <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! `<time>/uniform/reactorState`: the two numbers an eigenvalue stage hands the next (gh:#1814).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Upstream reads `keff` and `pTarget` with `lookupOrDefault(.., 1.0)` and writes the dictionary at every
//! write time. [`ReactorState::parse`] and [`ReactorState::to_foam_file`] do the same, so a port stage and an
//! upstream stage can restart each other; the file helpers are `std` only.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::WorkflowError;

/// `keff` and `pTarget` of `uniform/reactorState`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactorState {
    /// `keff_`.
    pub k_eff: f64,
    /// `pTarget_` [W].
    pub p_target: f64,
}

impl ReactorState {
    /// Parse the text of a `reactorState` file. A missing entry is `1.0`, as upstream's `lookupOrDefault`.
    /// `//` and `/* */` comments are skipped.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::ReactorState`] if an entry is not a number.
    pub fn parse(text: &str) -> Result<Self, WorkflowError> {
        let mut without_block = String::new();
        let mut rest = text;
        while let Some(i) = rest.find("/*") {
            without_block.push_str(&rest[..i]);
            match rest[i..].find("*/") {
                Some(j) => rest = &rest[i + j + 2..],
                None => {
                    rest = "";
                    break;
                }
            }
        }
        without_block.push_str(rest);
        let (mut k_eff, mut p_target) = (1.0, 1.0);
        for raw in without_block.lines() {
            let line = raw.split("//").next().unwrap_or("").trim();
            let line = line.trim_end_matches(';').trim();
            let mut it = line.split_whitespace();
            let (Some(key), Some(value), None) = (it.next(), it.next(), it.next()) else { continue };
            if key == "keff" || key == "pTarget" {
                let v: f64 = value
                    .parse()
                    .map_err(|_| WorkflowError::ReactorState(format!("`{key}` is not a number: `{value}`")))?;
                if key == "keff" {
                    k_eff = v;
                } else {
                    p_target = v;
                }
            }
        }
        Ok(Self { k_eff, p_target })
    }

    /// The file as upstream writes it (full-precision shortest round-trip numbers).
    #[must_use]
    pub fn to_foam_file(&self) -> String {
        format!(
            "FoamFile\n{{\n    version     2.0;\n    format      ascii;\n    class       dictionary;\n    object      reactorState;\n}}\n\nkeff            {};\npTarget         {};\n",
            self.k_eff, self.p_target
        )
    }

    /// Read `<time dir>/uniform/reactorState`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::ReactorState`] if the file cannot be read or parsed.
    #[cfg(feature = "std")]
    pub fn read(time_dir: &std::path::Path) -> Result<Self, WorkflowError> {
        let p = time_dir.join("uniform/reactorState");
        let text = std::fs::read_to_string(&p).map_err(|e| WorkflowError::ReactorState(format!("{}: {e}", p.display())))?;
        Self::parse(&text)
    }

    /// Write `<time dir>/uniform/reactorState`, creating `uniform/`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::ReactorState`] if the file cannot be written.
    #[cfg(feature = "std")]
    pub fn write(&self, time_dir: &std::path::Path) -> Result<(), WorkflowError> {
        let dir = time_dir.join("uniform");
        std::fs::create_dir_all(&dir).map_err(|e| WorkflowError::ReactorState(format!("{}: {e}", dir.display())))?;
        std::fs::write(dir.join("reactorState"), self.to_foam_file())
            .map_err(|e| WorkflowError::ReactorState(format!("{e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_upstream_writes_and_round_trips() {
        let text = "FoamFile\n{\n    class dictionary;\n}\n// c\nkeff            0.96031194083006322;\npTarget         2e+07; // W\n/* x\nkeff 5; */\n";
        let r = ReactorState::parse(text).expect("parse");
        assert_eq!(r.k_eff, 0.960_311_940_830_063_2);
        assert_eq!(r.p_target, 2e7);
        let back = ReactorState::parse(&r.to_foam_file()).expect("round trip");
        assert_eq!(back, r);
    }

    #[test]
    fn missing_entries_default_to_one_and_garbage_is_refused() {
        assert_eq!(ReactorState::parse("x 1;").expect("defaults"), ReactorState { k_eff: 1.0, p_target: 1.0 });
        assert!(ReactorState::parse("keff abc;").is_err());
    }

    #[cfg(feature = "std")]
    #[test]
    fn writes_and_reads_a_time_directory() {
        let dir = std::env::temp_dir().join("outram_reactor_state_test");
        let _ = std::fs::remove_dir_all(&dir);
        let r = ReactorState { k_eff: 0.93, p_target: 1.5e6 };
        r.write(&dir).expect("write");
        assert_eq!(ReactorState::read(&dir).expect("read"), r);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
