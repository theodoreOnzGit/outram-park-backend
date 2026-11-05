//! **The data of three Fission products lessons** (gh:#1207, maintainer
//! direction 2026-11-04: fold the TRISO-ATOPS mdBook's rung 6, "Air and
//! steam", and rung 8, "Into the source term", into the Learn-the-science
//! chapter): `chem-air.html`, `chem-steam.html` and `atops-source-term.html`.
//! Writes `chemhand-data.js` in `docs/site/science/fission-products/`.
//!
//! ```bash
//! cargo run --release -p dhoby-ghaut --example lesson_chemistry_handoff [out_dir]
//! ```
//!
//! `chem.rs` calls `boon_lay::chemistry` (three cited rate laws, each a
//! transcription, not a port) on temperature grids at a set of partial
//! pressures, and re-exports the dispersion demo's recorded source term
//! (`examples/dispersion_web/recorded.rs`, the sembawang example
//! `htr10_air_ingress_kora_bound` re-run 2026-10-04) without recomputing it.
//!
//! **Inputs, and why.** Temperatures 400-1600 °C every 10 °C (the measured
//! ranges, 597-694 °C for air and 850-1100 °C for steam, and the core
//! temperatures of an accident, up to 1600 °C). O₂ 0.21-21.28 kPa (21.28 kPa
//! is the measurement's own 21 % of 1 atm, `REFERENCE_O2_PA`). Steam 0.1-300
//! kPa: 0.5-20 kPa is Wang & Sun's box, 2.8 Pa-1.05 kPa the hydrolysis fit's,
//! and hundreds of kPa the water-ingress level gh:#418 names. The supply
//! limit is drawn with boon-lay's own illustrative check values (5000 kg of
//! graphite, 1 mol/s of O₂), labelled as such: no core air-ingress rate is
//! published for HTR-10 (gh:#420).
//!
//! **Predictions, written 2026-11-04 before the first run** (by hand from the
//! printed formulas and the module constants):
//!
//! - C1. On the 10 °C grid the air law is flagged inside its measured range at
//!   exactly 600-690 °C (10 points; the code's box is 870.15-967.15 K).
//! - C2. The kinetic air rate at 900 °C in air is 1.39e-3 /s within 5 %
//!   (Arrhenius ratio exp(E_a/R (1/923.15 - 1/1173.15)) ≈ 201 over the
//!   6.8877e-6 /s of 650 °C).
//! - C3. With 5000 kg and 1 mol/s the oxygen supply starts to bind at about
//!   612 °C in air (inside the measured range) and about 731 °C at 1 kPa
//!   O₂, each within 10 °C.
//! - C4. At 900 °C in air the kinetic carbon rate is about 580 times the
//!   supply (within 5 %).
//! - C5. Graphite-steam at 1050 °C, 5 kPa, no H₂: 2.005e-6 /s (boon-lay's
//!   transcription check), flagged inside; 1 kPa of H₂ cuts it to 0.45 of
//!   that, within 3 %.
//! - C6. The steam law is flagged inside only at 850-1100 °C and only for
//!   0.5, 1, 2, 5, 10 and 20 kPa; never at 0.1, 100 or 300 kPa.
//! - C7. Kernel hydrolysis at 1000 °C reaches the whole inventory (the clamp)
//!   at 5.0 kPa within 10 %; at 770 °C at 2.4 kPa within 10 % (TECDOC-978
//!   itself says "near 2 kPa at 770 °C").
//! - C8. At 300 kPa the hydrolysis fraction is clamped to 1 from 400 °C up to
//!   somewhere between 1150 and 1250 °C, and is BELOW 1 above that (about
//!   3.5e-3 at 1600 °C): the fit's pressure exponent -4.353 + 6503/T turns
//!   negative above 1494 K, so far outside the fit more steam releases less.
//! - S1. The recorded source term's released fraction of the inventory is
//!   1.91e-4 for Xe-133, I-131 and I-133 alike (3 figures), 1.02e-3 for
//!   Cs-137 and 2.49e-2 for Ag-110m, as the sembawang example's doc records.
//! - S2. The six recorded nuclides carry 65 % of the recorded total
//!   (1.103e13 of 1.689e13 Bq), within 1 %; ordered by released Bq,
//!   I-133 > Xe-133 > I-131 > Cs-137 > Cs-134 > Ag-110m.
//!
//! **Results, first run 2026-11-04 (predictions above unedited): all ten
//! held.**
//!
//! - C1: 600-690 °C, 10 points, at every O₂ pressure.
//! - C2: 1.3843e-3 /s (650 °C: 6.88773e-6 /s, boon-lay's check 6.8877e-6).
//! - C3: the supply binds above 612.5 °C in air and 730.9 °C at 1 kPa (also
//!   804.4 °C at 0.21 kPa, 701.3 at 2, 664.9 at 5, 639.1 at 10).
//! - C4: 576.3.
//! - C5: 2.004984e-6 /s, flagged inside; with 1 kPa H₂, 0.4536 of it.
//! - C6: inside at 850-1100 °C (26 points) for 0.5-20 kPa; none at 0.1, 100
//!   and 300 kPa.
//! - C7: 4.998 kPa at 1000 °C, 2.365 kPa at 770 °C.
//! - C8: at 300 kPa clamped from 400 to 1160 °C, 3.505e-3 at 1600 °C. Not
//!   predicted: the same sign change makes LOW pressure release more at high
//!   temperature, e.g. 0.1 kPa is clamped from 1360 °C up, while 0.5 and 1 kPa
//!   are never clamped on the grid (0.984 and 0.534 at 1600 °C).
//! - S1: 1.909e-4, 1.908e-4, 1.909e-4 (Xe-133, I-131, I-133), 1.021e-3
//!   (Cs-137), 2.494e-2 (Ag-110m); also Cs-134 1.491e-3.
//! - S2: 1.1031e13 of 1.6890e13 Bq = 0.6531, in the predicted order.
//!
//! Driver glue (`main`, `native::run`, formatting): writes what `chem.rs`
//! computes; its output is checked by `check-chemhand.js` and the functions
//! are reached by `tests/lesson_chemistry_handoff.rs`. Research, education
//! and V&V only (`RESPONSIBLE_USE.md`): kinetic fits outside their ranges and
//! a deliberately pessimistic bounding release, never a dose to a person.

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "chem.rs"]
mod chem;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::chem::*;
    use std::fmt::Write as _;

    fn e(x: f64) -> String {
        format!("{x:e}")
    }
    fn arr(v: impl IntoIterator<Item = f64>) -> String {
        format!("[{}]", v.into_iter().map(e).collect::<Vec<_>>().join(","))
    }
    fn flags(v: impl IntoIterator<Item = u8>) -> String {
        format!("[{}]", v.into_iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))
    }
    fn opt(x: Option<f64>) -> String {
        x.map_or("null".into(), e)
    }

    pub const O2_KPA: [f64; 6] = [0.21, 1.0, 2.0, 5.0, 10.0, 21.27825];
    pub const STEAM_KPA: [f64; 9] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 100.0, 300.0];
    pub const H2_KPA: [f64; 2] = [0.0, 1.0];

    pub fn run() -> Result<(), String> {
        let out_dir = std::env::args().nth(1).unwrap_or_else(|| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
        });
        let grid: Vec<f64> = (0..=120).map(|i| 400.0 + 10.0 * i as f64).collect();
        let mut js = String::new();
        writeln!(js, "// The data of the Fission products lessons chem-air, chem-steam and atops-source-term.").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_chemistry_handoff/main.rs:").unwrap();
        writeln!(js, "//   cargo run --release -p dhoby-ghaut --example lesson_chemistry_handoff").unwrap();
        writeln!(js, "// Do not edit; checked by check-chemhand.js. boon_lay::chemistry (graphite_air, graphite_steam,").unwrap();
        writeln!(js, "// kernel_hydrolysis) and the dispersion demo's recorded source term. SI units except °C and kPa.").unwrap();
        writeln!(js, "window.CHEMHAND_DATA = {{").unwrap();
        writeln!(js, "  tC: {},", arr(grid.iter().copied())).unwrap();

        // Air (lesson chem-air).
        writeln!(js, "  air: {{ massKg: {}, supplyMolPerS: {}, refO2kPa: 21.27825, co2kJPerMol: {}, curves: [", ILLUSTRATIVE_GRAPHITE_KG, ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S, boon_lay::chemistry::graphite_air::CO2_REACTION_ENTHALPY_J_PER_MOL / 1e3).unwrap();
        for p in O2_KPA {
            let c = air_curve(p, &grid);
            let kin: Vec<f64> = grid.iter().map(|&t| air_kinetic_mol(t, p)).collect();
            let gas: Vec<(f64, bool)> = grid.iter().map(|&t| air_gasified(t, p)).collect();
            let x = air_crossover_c(p, 300.0, 1600.0);
            let inside: Vec<f64> = grid.iter().zip(&c).filter(|(_, r)| r.1).map(|(t, _)| *t).collect();
            println!("  air {p} kPa: inside {:?}..{:?} ({} points); supply binds above {x:?} degC", inside.first(), inside.last(), inside.len());
            writeln!(js, "    {{ o2kPa: {p}, rate: {}, inside: {}, kineticMol: {}, gasifiedMol: {}, supplyBinds: {}, crossoverC: {} }},",
                arr(c.iter().map(|r| r.0)), flags(c.iter().map(|r| r.1 as u8)), arr(kin.iter().copied()), arr(gas.iter().map(|g| g.0)), flags(gas.iter().map(|g| g.1 as u8)), opt(x)).unwrap();
        }
        writeln!(js, "  ] }},").unwrap();
        let r900 = air_curve(21.27825, &[900.0])[0].0;
        println!("  C2: air rate at 900 degC in air {r900:.4e} /s; C4: kinetic/supply {:.1}", air_kinetic_mol(900.0, 21.27825) / ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S);
        let r650 = air_curve(21.27825, &[650.0])[0].0;
        println!("  air rate at 650 degC in air {r650:.5e} /s (boon-lay check 6.8877e-6)");
        writeln!(js, "  airAt: {{ r650: {}, r900: {}, ratio900: {} }},", e(r650), e(r900), e(air_kinetic_mol(900.0, 21.27825) / ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S)).unwrap();

        // Steam (lesson chem-steam).
        writeln!(js, "  steam: [").unwrap();
        for p in STEAM_KPA {
            let h: Vec<(f64, u8)> = hydrolysis_curve(p, &grid);
            write!(js, "    {{ kPa: {p}, hydrolysis: {}, hydrolysisFlag: {}, graphite: [", arr(h.iter().map(|x| x.0)), flags(h.iter().map(|x| x.1))).unwrap();
            for h2 in H2_KPA {
                let g = steam_curve(p, h2, &grid);
                write!(js, "{{ h2kPa: {h2}, rate: {}, inside: {} }},", arr(g.iter().map(|r| r.0)), flags(g.iter().map(|r| r.1 as u8))).unwrap();
                if h2 == 0.0 {
                    let inside: Vec<f64> = grid.iter().zip(&g).filter(|(_, r)| r.1).map(|(t, _)| *t).collect();
                    println!("  C6 steam {p} kPa: inside {:?}..{:?} ({} points)", inside.first(), inside.last(), inside.len());
                }
            }
            writeln!(js, "] }},").unwrap();
            let clamped: Vec<f64> = grid.iter().zip(&h).filter(|(_, x)| x.1 == 2).map(|(t, _)| *t).collect();
            println!("  C8 hydrolysis {p} kPa: clamped at {:?}..{:?} ({} points); f(1600) = {:.3e}", clamped.first(), clamped.last(), clamped.len(), h.last().unwrap().0);
        }
        writeln!(js, "  ],").unwrap();
        let s1050 = steam_curve(5.0, 0.0, &[1050.0])[0];
        let s1050h = steam_curve(5.0, 1.0, &[1050.0])[0];
        println!("  C5: steam 1050 degC 5 kPa {:.6e} /s inside {}; with 1 kPa H2 ratio {:.4}", s1050.0, s1050.1, s1050h.0 / s1050.0);
        writeln!(js, "  steamAt: {{ r1050: {}, r1050H2: {} }},", e(s1050.0), e(s1050h.0)).unwrap();
        let wt: Vec<f64> = (0..=13).map(|i| 700.0 + 50.0 * i as f64).collect();
        let wp: Vec<Option<f64>> = wt.iter().map(|&t| hydrolysis_whole_inventory_kpa(t)).collect();
        println!("  C7: whole inventory at 1000 degC {:?} kPa, at 770 degC {:?} kPa", hydrolysis_whole_inventory_kpa(1000.0), hydrolysis_whole_inventory_kpa(770.0));
        writeln!(js, "  wholeInventory: {{ tC: {}, kPa: [{}], at770: {}, at1000: {} }},", arr(wt.iter().copied()), wp.iter().map(|x| opt(*x)).collect::<Vec<_>>().join(","), opt(hydrolysis_whole_inventory_kpa(770.0)), opt(hydrolysis_whole_inventory_kpa(1000.0))).unwrap();

        // The source term (lesson atops-source-term), recorded, not recomputed.
        let rel = released();
        let sum: f64 = rel.iter().map(|r| r.released_bq).sum();
        for r in &rel {
            println!("  S1 {}: released {:.4e} Bq of {:.4e} = {:.3e}", r.name, r.released_bq, r.inventory_bq, r.fraction);
        }
        println!("  S2: six nuclides {sum:.4e} of {:.4e} Bq = {:.4}", recorded::RELEASE_TOTAL_BQ, sum / recorded::RELEASE_TOTAL_BQ);
        writeln!(js, "  source: {{ provenance: {:?}, totalBq: {}, sixBq: {}, chiOverQ400mF: {}, nuclides: [", recorded::SOURCE, e(recorded::RELEASE_TOTAL_BQ), e(sum), e(recorded::CHI_OVER_Q_400M_F_1MS)).unwrap();
        for r in &rel {
            writeln!(js, "    {{ name: \"{}\", releasedBq: {}, inventoryBq: {}, fraction: {} }},", r.name, e(r.released_bq), e(r.inventory_bq), e(r.fraction)).unwrap();
        }
        writeln!(js, "  ] }}").unwrap();
        writeln!(js, "}};").unwrap();
        let path = std::path::Path::new(&out_dir).join("chemhand-data.js");
        std::fs::write(&path, js).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", path.display());
        Ok(())
    }
}
