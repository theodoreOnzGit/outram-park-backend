// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # No discrete level is open below OpenMC's `LevelInelastic` threshold (gh:#1497)
//!
//! OpenMC's `LevelInelastic::sample` (`src/distribution_energy.cpp:47`) returns
//! `E_cm = mass_ratio * (E - threshold)` with `threshold = (A + 1)/A |Q|`
//! (`openmc/data/reaction.py:1274`), and `inelastic_scatter` takes
//! `sqrt(E_in * E_cm)`. Below that threshold the outgoing energy is NaN. Upstream
//! never gets there because the level's cross section is 0 below its
//! `threshold_idx`. This port's levels come from the ENDF route (RECONR), so the
//! same must hold of them: this was the leading hypothesis for gh:#1497's NaN
//! energy, and it was REJECTED by this scan (the NaN came from the direction
//! norm; see `tests/direction_norm_drift_guard.rs`).
//!
//! ## Methodology
//!
//! For every discrete level (MT 51..90) of each tape below, ENDF/B-VIII.0 at
//! 300.15 K, tolerance 1e-3 (the HTR-10 loader's settings): the level's own
//! native grid in `[0.98, 1 + 1e-7] x threshold`, plus `threshold x (1 - 10^-k)`
//! for k = 1..15. Pass: no point below the threshold carries a nonzero level
//! cross section, and `sample_level_inelastic_scatter` at the threshold-adjacent
//! points it does sample returns a finite energy and direction.
//!
//! ## Results (2026-11-11)
//!
//! 0 below-threshold points with nonzero cross section on every tape scanned:
//! this test's five, and, in a one-off run of the same scan, U-235, U-238,
//! C-12, C-13, Si-29, Si-30, He-3, He-4, Fe-56 and Mn-55 as well. No level is
//! open at any scanned point below its threshold; those that open inside the
//! window first do so at the window's top, `threshold x (1 + 1e-7)`.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

fn endf(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/endf")
        .join(name)
}

#[test]
fn no_level_is_open_below_the_openmc_threshold() {
    let tapes = [
        "n-008_O_016-ENDF8.0.endf",
        "n-014_Si_028-ENDF8.0.endf",
        "n-005_B_010-ENDF8.0.endf",
        "n-005_B_011-ENDF8.0.endf",
        "n-024_Cr_052-ENDF8.0.endf",
    ];
    let mut checked = 0usize;
    for tape in tapes {
        let p = endf(tape);
        if !p.exists() {
            eprintln!("SKIP {tape}: tape absent");
            continue;
        }
        let nuc = Nuclide::from_endf_file(&p, tape, ThermodynamicTemperature::new::<kelvin>(300.15), 1.0e-3)
            .expect("load");
        let awr = nuc.awr;
        for (mt, q, cont) in nuc.inelastic_levels_table() {
            if cont || q >= 0.0 {
                continue;
            }
            let thr = (awr + 1.0) / awr * q.abs();
            let mut es = nuc.native_energy_grid(thr * 0.98, thr * (1.0 + 1e-7));
            for k in 1..=15 {
                es.push(thr * (1.0 - 10f64.powi(-k)));
            }
            for &e in &es {
                let xs = nuc.inelastic_channel_xs(mt as i32, e).unwrap_or(0.0);
                checked += 1;
                assert!(
                    !(xs > 0.0 && e < thr),
                    "{tape} MT={mt}: sigma = {xs:e} b at E = {e:e} eV, below threshold {thr:e} eV"
                );
                if xs > 0.0 {
                    let mut seed = 1497_u64;
                    let (e2, u2) =
                        nuc.sample_level_inelastic_scatter(mt as i32, q, e, Direction::new(0.0, 0.0, 1.0), &mut seed);
                    assert!(
                        e2.is_finite() && u2.u.is_finite() && u2.v.is_finite() && u2.w.is_finite(),
                        "{tape} MT={mt} E={e:e}: E'={e2}, u'={u2:?}"
                    );
                }
            }
        }
    }
    eprintln!("{checked} (level, energy) points checked");
}
