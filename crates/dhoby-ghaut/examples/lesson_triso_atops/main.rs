//! **The data of the Fission products chapter 5, "Integrating it in
//! TRISO-ATOPS"** (lessons 39-46; maintainer direction 2026-10-22: the
//! planned "Inside the codes" Part A, recast as the chapter that brings
//! chapters 3 and 4 together inside TRISO-ATOPS). Writes `atops-data.js` in
//! `docs/site/science/fission-products/`.
//!
//! ```bash
//! cargo run --release -p dhoby-ghaut --example lesson_triso_atops [out_dir]
//! ```
//!
//! `atops.rs` calls boon-lay's TRISO-ATOPS port (`triso_atops_fork`, a port
//! of INL's TRISO-ATOPS at upstream de374c8) with the MHTGR Case A constants
//! of the TRISO-ATOPS paper (Stoyer, Raichart & Petti, Nucl. Technol. 2026,
//! Tables 2-3, printed p. 7, as `constants.csv` records them), and bridges
//! to chapter 4 through `FailureFractions::with_fuel_failure_incremental`.
//!
//! **Predictions, written 2026-10-22 before the first run:**
//!
//! - Q1. Over the 84 nuclides, `ElementGroup` gives 5 groups, and chapter 3's
//!   diffusion classes nest inside them: every noble gas, I, Te and Se is in
//!   the {Se, Kr, Te, I, Xe} class; every special metal is in {Rb, Cs} or
//!   {Sr, Ba, Eu}; Ag and Pd are {Pd, Ag}; Other is the nominal 1e-19 class.
//!   The only exceptions are halogens with no correlation of their own (Br,
//!   if present), which sit in the Halogen group but the nominal class.
//! - Q2. The clamps, read from the output alone on a 25 °C grid from 300 °C:
//!   Cs kernel 700 and graphite 550; Sr kernel 700 and graphite 800; Ag
//!   graphite 475-500 (the code says 490); none for the Kr kernel, the Ag
//!   kernel or Ag in SiC.
//! - Q3. `booth_longlived` tends to 1.216e-4, not 0, as D t / a^2 -> 0
//!   (gh:#385): at 1e-16 it returns 1.21598e-4 within 1e-3; `booth_transient`
//!   returns the same (its 1e-6 snap does not reach it).
//! - Q4. The port's Booth radius sqrt(2 a_grain r) is 6.53e-5 m for Case A.
//! - Q5. Doubling all four normal-operation failure fractions doubles the
//!   node's release rate (to 1e-12 relative) for Kr-88, I-131, Cs-137 and
//!   Sr-90, and leaves Ag-110m's unchanged.
//! - Q6. Replacing f_inc by boon-lay fuel failure's end-of-irradiation phi_1
//!   (1.2e-12 at T_B 776 °C, chapter 4) multiplies Kr-88's release rate by
//!   (f_HM + phi_1) / (f_HM + f_inc) = 0.465 (noble gases use f_HM + f_inc).
//! - Q7. boon-lay's live pools (not a port), stepped from empty with no
//!   leak over Case A's 40-year run, reproduce the node's circulating pool
//!   for Cs-137 to 1e-9 relative.
//! - Q8. On a heat-up from 900 to 1600 °C, the venting fraction at the end of
//!   the hold is 1 - T0/T_peak = 0.374 within 2 %, and it is undefined at
//!   every cooling sample.
//! - Q9. Cs-137's kernel release fraction on that heat-up reaches at least
//!   0.99 by the end of the hold.
//!
//! **Results, 2026-10-22 (predictions above unedited):**
//!
//! - Q1 HIT: 84 nuclides, 5 groups, and no exception at all (the table has no
//!   Br). Y-90 is in Other, with the nominal D, although the paper says Y
//!   uses Sr's diffusivities (printed p. 9): a paper-versus-code difference
//!   the port inherits from upstream.
//! - Q2 HIT: Cs kernel 700, graphite 550; Sr 700, 800; Ag graphite 475 on
//!   the 25 °C grid (490 in the code); no clamp on the Kr kernel, the Ag
//!   kernel or Ag in SiC.
//! - Q3 HIT: both 1.215976e-4 at 1e-16.
//! - Q4 HIT: 6.5269e-5 m.
//! - Q5 HIT: ratio 2.000000000000 for Kr-88, I-131, Cs-137, Sr-90; Ag-110m 1.
//! - Q6 HIT: 0.465116 for Kr-88 (and I-131); 0.871508 for Cs-137 and Sr-90,
//!   which use all four fractions.
//! - Q7 HIT for Cs-137 (relative difference 0); I-131, Sr-90, Ag-110m also
//!   0. The first run gave Kr-88 a plate-out rate, which upstream's routing
//!   does not (my input error, corrected; the prediction named only Cs-137).
//! - Q8 MISSED: the venting fraction at the end of the hold is 0.356, not
//!   0.374 within 2 %. Upstream's `mean_temperature_rate` sets dT/dt = 0 at
//!   the first sample, so the trapezoid drops half of the first 5 h step;
//!   it is undefined at every cooling sample, as predicted.
//! - Q9 HIT: Cs-137 kernel release 0.9979 at the end of the hold.
//!
//! Gated in `check-fission.js` (checks 45-52).
//!
//! Driver glue (`main`, `native::run`, formatting): writes what `atops.rs`
//! computes; its output is checked by `check-fission.js` and the functions
//! are reached by `tests/lesson_triso_atops.rs`. Research, education and V&V
//! only (`RESPONSIBLE_USE.md`).

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "atops.rs"]
mod atops;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::atops::*;
    use boon_lay::fuel_failure::history::FailureProgress;
    use boon_lay::fuel_failure::htr10;
    use petir::mathf::RealMath;
    use std::fmt::Write as _;
    use uom::si::frequency::hertz;
    use uom::si::ratio::ratio;
    use uom::si::time::second;

    fn e(x: f64) -> String {
        format!("{x:e}")
    }
    fn arr(v: impl IntoIterator<Item = f64>) -> String {
        format!("[{}]", v.into_iter().map(e).collect::<Vec<_>>().join(","))
    }
    fn opt(v: impl IntoIterator<Item = Option<f64>>) -> String {
        format!("[{}]", v.into_iter().map(|x| x.map_or("null".into(), e)).collect::<Vec<_>>().join(","))
    }

    pub fn run() -> Result<(), String> {
        let out_dir = std::env::args().nth(1).unwrap_or_else(|| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
        });
        let mut js = String::new();
        writeln!(js, "// The data of the Fission products chapter 5 \"Integrating it in TRISO-ATOPS\".").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_triso_atops/main.rs:").unwrap();
        writeln!(js, "//   cargo run --release -p dhoby-ghaut --example lesson_triso_atops").unwrap();
        writeln!(js, "// Do not edit; checked by check-fission.js. boon-lay's TRISO-ATOPS port with the MHTGR Case A").unwrap();
        writeln!(js, "// constants of Stoyer et al. (2026), Tables 2-3, as constants.csv records them. SI units; °C.").unwrap();
        writeln!(js, "window.ATOPS_DATA = {{").unwrap();
        let p = case_a();
        writeln!(js, "  caseA: {{ kPlate: {}, kClean: {}, aGraph: {}, aGrain: {}, aSiC: {}, r: {}, runTime: {}, tIrrad: {}, aBooth: {}, fHM: 2e-5, fSiC: 1e-4, fInc: 2.3e-5, fIncSiC: 3.6e-5, fIncAcc: 6.6e-5, fIncSiCAcc: 1.7e-4 }},",
            e(p.k_plate.get::<hertz>()), e(p.k_clean.get::<hertz>()), e(p.graphite_thickness.get::<uom::si::length::meter>()), e(p.grain_size.get::<uom::si::length::meter>()),
            e(p.sic_thickness.get::<uom::si::length::meter>()), e(p.kernel_radius.get::<uom::si::length::meter>()), e(p.run_time.get::<second>()), e(p.irradiation_time.get::<second>()), e(a_booth())).unwrap();
        println!("  Q4: a_booth = {:.4e} m", a_booth());

        // Nuclides, groups and chapter 3's diffusion classes (Q1).
        let grid: Vec<f64> = (0..=84).map(|i| 300.0 + 25.0 * i as f64).collect();
        let sig = |z: u32| -> Vec<(f64, f64)> { grid.iter().map(|&t| d(z, t)).collect() };
        let reps = [(36u32, "Kr-like"), (55, "Cs-like"), (38, "Sr-like"), (47, "Ag-like"), (40, "nominal")];
        let class_of = |z: u32| -> usize { let s = sig(z); reps.iter().position(|(r, _)| sig(*r) == s).unwrap_or(99) };
        let nucs = nuclides();
        write!(js, "  nuclides: [").unwrap();
        let mut groups = std::collections::BTreeSet::new();
        for (n, sl) in &nucs {
            let g = group_name(n.element_group());
            groups.insert(g);
            write!(js, "{{ name: \"{}\", z: {}, a: {}, t12: {}, group: \"{g}\", short: {sl}, dClass: {} }},", n.name, n.z, n.a, e(n.half_life.get::<second>()), class_of(n.z)).unwrap();
        }
        writeln!(js, "],").unwrap();
        writeln!(js, "  dClassNames: [{}],", reps.iter().map(|r| format!("\"{}\"", r.1)).collect::<Vec<_>>().join(",")).unwrap();
        println!("  Q1: {} nuclides, groups {:?}", nucs.len(), groups);
        for (n, _) in &nucs {
            let g = n.element_group();
            let c = class_of(n.z);
            let expect = match g {
                boon_lay::triso_atops_fork::nuclide_model::ElementGroup::NobleGas | boon_lay::triso_atops_fork::nuclide_model::ElementGroup::Halogen => 0,
                boon_lay::triso_atops_fork::nuclide_model::ElementGroup::SpecialMetal => if c == 1 { 1 } else { 2 },
                boon_lay::triso_atops_fork::nuclide_model::ElementGroup::Silver => 3,
                boon_lay::triso_atops_fork::nuclide_model::ElementGroup::Other => 4,
            };
            if c != expect {
                println!("  Q1 exception: {} (Z {}) group {} but class {}", n.name, n.z, group_name(g), c);
            }
        }

        // Diffusion with clamps (Q2).
        writeln!(js, "  dGrid: {},", arr(grid.iter().copied())).unwrap();
        writeln!(js, "  dCurves: [").unwrap();
        for (z, name) in reps {
            let s = sig(z);
            let ck = flat_below(&grid, |t| d(z, t).0);
            let cg = flat_below(&grid, |t| d(z, t).1);
            println!("  Q2: {name} (Z {z}): kernel flat below {ck:?}, graphite flat below {cg:?}");
            writeln!(js, "    {{ z: {z}, name: \"{name}\", kernel: {}, graphite: {}, clampKernel: {}, clampGraphite: {} }},", arr(s.iter().map(|x| x.0)), arr(s.iter().map(|x| x.1)), ck.map_or("null".into(), e), cg.map_or("null".into(), e)).unwrap();
        }
        writeln!(js, "  ],").unwrap();
        let cs = flat_below(&grid, d_sic_ag);
        println!("  Q2: Ag in SiC flat below {cs:?}");
        writeln!(js, "  dSicAg: {},", arr(grid.iter().map(|&t| d_sic_ag(t)))).unwrap();

        // Release models (lesson 42).
        let tr: Vec<f64> = (0..=24).map(|i| 600.0 + 50.0 * i as f64).collect();
        writeln!(js, "  rbT: {},", arr(tr.iter().copied())).unwrap();
        writeln!(js, "  rb: [").unwrap();
        for name in ["Kr-88", "Xe-133", "I-131", "Cs-137", "Cs-134", "Sr-90", "Sr-89", "Ag-110m", "Ag-111", "Zr-95"] {
            if let Some((n, sl)) = nucs.iter().find(|x| x.0.name == name) {
                writeln!(js, "    {{ name: \"{name}\", group: \"{}\", short: {sl}, rb: {} }},", group_name(n.element_group()), arr(tr.iter().map(|&t| rb(n, *sl, t)))).unwrap();
            } else {
                println!("  {name}: not in the TRISO-ATOPS table, left out");
            }
        }
        writeln!(js, "  ],").unwrap();
        let xs: Vec<f64> = (0..=72).map(|i| 10f64.r_powf(-16.0 + 17.0 * i as f64 / 72.0)).collect();
        writeln!(js, "  booth: {{ terms: {}, x: {}, long: {}, transient: {} }},", booth_terms(), arr(xs.iter().copied()), arr(xs.iter().map(|&x| booth_long(x))), arr(xs.iter().map(|&x| booth_trans(x)))).unwrap();
        println!("  Q3: booth_longlived(1e-16) = {:.6e}, booth_transient(1e-16) = {:.6e}", booth_long(1e-16), booth_trans(1e-16));
        writeln!(js, "  breakthrough: {},", arr(tr.iter().map(|&t| breakthrough(t)))).unwrap();
        let t3 = years(3.0);
        writeln!(js, "  attenuation: {{ Cs: {}, Sr: {}, Ag: {} }},", arr(tr.iter().map(|&t| attenuation(55, t, t3))), arr(tr.iter().map(|&t| attenuation(38, t, t3))), arr(tr.iter().map(|&t| attenuation(47, t, t3)))).unwrap();

        // Normal operation per node (lesson 43) and the coolant (44).
        let f = fractions_a();
        let doubled = boon_lay::triso_atops_fork::activities::source_terms::FailureFractions { heavy_metal: 2.0 * f.heavy_metal, sic: 2.0 * f.sic, incremental: 2.0 * f.incremental, incremental_sic: 2.0 * f.incremental_sic };
        let phi1 = htr10::end_of_irradiation_failure(temp(776.0)).get::<ratio>();
        let bridged = f.with_fuel_failure_incremental(FailureProgress::at_start(uom::si::f64::Ratio::new::<ratio>(phi1), uom::si::f64::Ratio::new::<ratio>(0.0)));
        writeln!(js, "  phi1Eoi776: {},", e(phi1)).unwrap();
        let tn: Vec<f64> = (0..=12).map(|i| 600.0 + 50.0 * i as f64).collect();
        writeln!(js, "  nodeT: {},", arr(tn.iter().copied())).unwrap();
        writeln!(js, "  nodes: [").unwrap();
        for name in ["Kr-88", "I-131", "Cs-137", "Sr-90", "Ag-110m"] {
            let (n, sl) = nucs.iter().find(|x| x.0.name == name).expect("in the table").clone();
            let base = node(&n, sl, 900.0, f, false);
            let dbl = node(&n, sl, 900.0, doubled, false);
            let br = node(&n, sl, 900.0, bridged, false);
            let hps = node(&n, sl, 900.0, f, true);
            println!("  Q5/Q6 {name}: R {:.4e}, doubled ratio {:.12}, bridged ratio {:.6}", base.release_rate, dbl.release_rate / base.release_rate, br.release_rate / base.release_rate);
            let pools = |a: &boon_lay::triso_atops_fork::normal_operation::NodalActivities| format!("[{},{},{},{},{},{}]", e(a.release_rate), e(a.source_rate), e(a.graphite_activity), e(a.circulating_activity), e(a.plate_out_activity), e(a.clean_up_activity));
            let rt: Vec<f64> = tn.iter().map(|&t| node(&n, sl, t, f, false).release_rate).collect();
            let lam = n.decay_constant().get::<hertz>();
            // Upstream's routing: noble gases do not plate out (k_plate = 0).
            let kp = if matches!(n.element_group(), boon_lay::triso_atops_fork::nuclide_model::ElementGroup::NobleGas) { 0.0 } else { p.k_plate.get::<hertz>() };
            let (lc, lp, _, _) = live(base.source_rate, lam, kp, 0.0, 0.0, p.run_time.get::<second>(), 400);
            let (lcl, _, _, lleak) = live(base.source_rate, lam, kp, 0.0, 1e-6, p.run_time.get::<second>(), 400);
            println!("  Q7 {name}: live circulating {lc:.6e} vs node {:.6e} (rel {:.2e}); with a 1e-6/s leak {lcl:.4e}, leaked {lleak:.4e}", base.circulating_activity, (lc - base.circulating_activity).abs() / base.circulating_activity.abs().max(1e-300));
            writeln!(js, "    {{ name: \"{name}\", group: \"{}\", short: {sl}, lambda: {}, base: {}, doubled: {}, bridged: {}, hps: {}, releaseVsT: {}, live: [{},{}], liveLeak: [{},{}] }},",
                group_name(n.element_group()), e(lam), pools(&base), pools(&dbl), pools(&br), pools(&hps), arr(rt), e(lc), e(lp), e(lcl), e(lleak)).unwrap();
        }
        writeln!(js, "  ],").unwrap();
        writeln!(js, "  poolFields: [\"release\",\"source\",\"graphite\",\"circulating\",\"plateOut\",\"cleanUp\"],").unwrap();

        // Accidents (lesson 45).
        writeln!(js, "  accidents: [").unwrap();
        for peak in [1400.0, 1600.0, 1800.0] {
            let (h, t) = heatup(900.0, peak);
            writeln!(js, "    {{ peak: {peak}, t0: 900, hours: {}, tC: {}, nuclides: [", arr(h.iter().copied()), arr(t.iter().copied())).unwrap();
            for name in ["Cs-137", "Sr-90", "Kr-85", "I-131", "Ag-110m"] {
                let (n, sl) = nucs.iter().find(|x| x.0.name == name).expect("in the table").clone();
                let a = accident(&n, &h, &t);
                let base = node(&n, sl, 900.0, f, false);
                let inv = 3.7e10 / n.decay_constant().get::<hertz>();
                let rel: Vec<f64> = a.rf_kernel.iter().map(|&rf| released_atoms(&n, &base, rf) / inv).collect();
                if peak == 1600.0 {
                    let end_hold = 20usize;
                    println!("  Q8/Q9 {name} 1600 C: RF kernel at end of hold {:.6}, graphite {:.4e}, vent {:?}, vent defined while cooling: {}", a.rf_kernel[end_hold], a.rf_graphite[end_hold], a.vent[end_hold], a.vent[end_hold + 1..].iter().any(|v| v.is_some()));
                }
                writeln!(js, "      {{ name: \"{name}\", group: \"{}\", rfKernel: {}, rfGraphite: {}, vent: {}, releasedPerInventory: {} }},", group_name(n.element_group()), arr(a.rf_kernel.iter().copied()), arr(a.rf_graphite.iter().copied()), opt(a.vent.iter().copied()), arr(rel)).unwrap();
            }
            writeln!(js, "    ] }},").unwrap();
        }
        writeln!(js, "  ]").unwrap();
        writeln!(js, "}};").unwrap();
        let path = std::path::Path::new(&out_dir).join("atops-data.js");
        std::fs::write(&path, js).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", path.display());
        Ok(())
    }
}
