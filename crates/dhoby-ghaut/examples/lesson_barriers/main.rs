//! **The data of the Fission products chapter 4, "TRISO layer migration and
//! particle failure"** (lessons 29-38; maintainer direction 2026-10-22,
//! which merged the planned "Barriers" and "Fuel failure" chapters and
//! added the diffusion-through-layers portion). Writes `barrier-data.js`
//! and `failure-data.js` in `docs/site/science/fission-products/`.
//!
//! ```bash
//! cargo run --release -p dhoby-ghaut --example lesson_barriers [out_dir]
//! ```
//!
//! - `layers.rs`: per-layer D for Ag-110m, Cs-137, Sr-90 and Kr-85 from
//!   boon-lay's walker, TRISO-ATOPS's graphite and SiC-Ag correlations, and
//!   the HTR-10 TRISO thicknesses (`TrisoSpec::HTR10_LI2014`).
//! - `walk.rs`: boon-lay's multilayer Walk-on-Spheres walker on the HTR-10
//!   particle as a cost probe, and the exact mean exit time that checks
//!   lesson 32's walk.
//! - `failure.rs`: boon-lay fuel failure on the HTR-10 particle (an
//!   extrapolation), and TRISO-ATOPS's transient Booth for the release from
//!   failed particles.
//!
//! **Predictions, written 2026-10-20 (P1-P3, with the parked generator) and
//! 2026-10-22 (P4-P12), before the first run of each:**
//!
//! - P1. For Ag, Cs and Sr the SiC layer's crossing time l^2/D exceeds the
//!   IPyC layer's by at least a factor 10 at every temperature from 800 to
//!   1600 °C (carbon leaks metals; SiC holds them better).
//! - P2. Kr's IPyC crossing time exceeds a year at 1000 °C.
//! - P3. Kr in the buffer has no coefficient (a `todo!()`; read, not called).
//! - P4. In boon-lay's whole-particle walk, the median release time is set by
//!   the slowest layer: within a factor 10 either way of the largest of the
//!   per-layer l^2/D (the kernel counted with its radius). So Cs and Ag are
//!   SiC-limited and Sr is KERNEL-limited (Sr barely moves in UO2).
//! - P5. The page's lattice walk (barrier-physics.js) gives a median within
//!   a factor 2 of boon-lay's walker at the same element and temperature
//!   (checked in check-fission.js).
//! - P6. The `fuel_failure::htr10` docs' 200 h table (T_B 776 °C) re-measures
//!   to 3 significant figures: sigma 50/178/370/536 MPa and phi_1
//!   4.53e-9/3.10e-5/4.79e-3/6.07e-2 at 1200/1600/2000/2200 °C; phi_2
//!   3.4e-15 at 1600 and 0.977 at 2200 °C. Its end-of-irradiation table
//!   re-measures too: phi_1 2.8e-15, 1.2e-12, 5.0e-9, 1.6e-6 at T_B 700,
//!   776, 900, 1000 °C.
//! - P7. phi_1 at sigma = sigma_o is 0.5 exactly (the ln 2), to 1e-12; and
//!   the pressure split p_gas + p_CO equals the driver's `pressure_at` to
//!   1e-12 relative.
//! - P8. Grain-boundary attack ON raises phi_1 after 500 h at 1600 °C by at
//!   least a factor 3 over OFF.
//! - P9. phi_2 overtakes phi_1 between 2000 and 2200 °C in a 200 h hold.
//! - P10. On the illustrative heat-up that cools again, phi_1 and phi_2 never
//!   fall (phi_1 is accumulated from positive increments; phi_2 is read off
//!   the monotone zeta).
//! - P11. Cs-137's kernel release by transient Booth reaches at least 0.99
//!   in a 200 h hold at 1600 °C, so the released share of the whole
//!   population is phi_total within 1 %; Sr-90's is between 0.05 and 0.15.
//! - P12. Raising burnup from 0.04 to 0.12 FIMA raises the 1600 °C, 200 h
//!   pressure by more than the factor 3 of burnup alone (more gas also
//!   gets out of a hotter, longer-burned kernel), but less than 6.
//!
//! **Written 2026-10-22 after a QUICK run (8 walks, 2e6 steps each) showed
//! that boon-lay's walker reached no release at all, and BEFORE the runs
//! that test them:**
//!
//! - P13. The exact mean exit time (`walk::exact_mean_exit`) is dominated
//!   (more than half) by one layer: SiC for Cs and Ag, the kernel for Sr, at
//!   every temperature from 1200 to 1800 °C. (This replaces P4, which needs
//!   the walker's release times.)
//! - P14. The page's lattice walk gives a mean exit time within 15 % of the
//!   exact mean for Cs, Ag and Sr at 1200, 1400, 1600 and 1800 °C (checked
//!   in check-fission.js; replaces P5).
//! - P15. In 2e7 steps at 1600 °C, boon-lay's walker never reaches the SiC
//!   for Cs, and spends more than 90 % of its steps in the buffer.
//!
//! **Results, 2026-10-22 (predictions above unedited):**
//!
//! - P1 HIT: smallest SiC/IPyC crossing-time ratio 16.7 (Cs, 800 °C).
//! - P2 HIT: Kr IPyC crossing at 1000 °C 4.79e10 s (about 1500 years).
//! - P3 HIT: Kr/buffer is the only walker gap among the four layers (read
//!   from the source; never called).
//! - P4, P5 NOT TESTABLE: a quick run gave no release from boon-lay's walker
//!   at all, so there was no median to compare. Replaced by P13-P15.
//! - P6 HIT: sigma 50.2/178.5/370.3/536.5 MPa; phi_1 4.529e-9/3.103e-5/
//!   4.794e-3/6.069e-2; phi_2 3.442e-15 and 0.9770; end of irradiation
//!   2.776e-15, 1.214e-12, 4.988e-9, 1.577e-6.
//! - P7 HIT: phi_1(sigma_o) = 0.5 exactly; pressure split 2.7e-16.
//! - P8 HIT: grain-boundary attack raises phi_1 at 1600 °C, 500 h by 62.8.
//! - P9 HIT: at 200 h phi_2 < phi_1 at 2000 °C (2.78e-4 vs 4.79e-3), > at
//!   2200 °C (0.977 vs 6.07e-2).
//! - P10 HIT: neither falls on any of the four heat-ups.
//! - P11 HIT: Cs Booth release 0.9999996, Sr 0.0827 (1600 °C, 200 h).
//! - P12 MISSED: the ratio is 3.000 exactly. In boon-lay fuel failure neither
//!   F_d (D_S of a UO2 kernel does not depend on burnup) nor the UO2 OPF
//!   depends on burnup, so the pressure is linear in it.
//! - P13 HIT: the dominant layer carries 93.3-100 % of the mean exit time
//!   (SiC for Cs and Ag, kernel for Sr, 1000-1800 °C).
//! - P14 HIT: the page's lattice walk is within 15 % of the exact mean in all
//!   12 cases (worst Ag at 1200 °C, -14.2 %, s.e. 3.6 %; node, 400 atoms).
//! - P15 HIT: in 2e7 steps (2.7 s) at 1600 °C, boon-lay's walker spent
//!   99.95-100 % of its steps in the buffer, entered the IPyC 8 (Cs), 60
//!   (Ag) and 1558 (Sr) times, and never reached the SiC.
//!
//! The run's numbers are gated in `check-fission.js` (checks 36-44).
//!
//! Driver glue (`main`, `native::run`, formatting): writes what the three
//! modules compute; its output is checked by `check-fission.js` and the
//! modules are reached by `tests/lesson_barriers.rs`. Research, education
//! and V&V only (`RESPONSIBLE_USE.md`).

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "layers.rs"]
mod layers;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "walk.rs"]
mod walk;
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
#[path = "failure.rs"]
mod failure;

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use super::failure as ff;
    use super::layers::*;
    use super::walk;
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial;
    use boon_lay::prelude::{diffusion_coefficient, diffusion_coefficient_sic_ag, find_nuclide};
    use boon_lay::Nuclide;
    use std::fmt::Write as _;
    use uom::si::diffusion_coefficient::square_meter_per_second;
    use uom::si::f64::ThermodynamicTemperature;
    use uom::si::thermodynamic_temperature::degree_celsius;
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
        let quick = std::env::var("LESSON_BARRIERS_QUICK").is_ok();
        let barrier = barrier_js(quick);
        let failure = failure_js();
        for (name, js) in [("barrier-data.js", barrier), ("failure-data.js", failure)] {
            let path = std::path::Path::new(&out_dir).join(name);
            std::fs::write(&path, js).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("wrote {}", path.display());
        }
        Ok(())
    }

    fn header(js: &mut String, what: &str) {
        writeln!(js, "// The data of the Fission products chapter 4 \"TRISO layer migration and particle failure\": {what}.").unwrap();
        writeln!(js, "// GENERATED by crates/dhoby-ghaut/examples/lesson_barriers/main.rs:").unwrap();
        writeln!(js, "//   cargo run --release -p dhoby-ghaut --example lesson_barriers").unwrap();
        writeln!(js, "// Do not edit; checked by check-fission.js. SI units (m, s, Pa, m^2/s); temperatures in °C.").unwrap();
    }

    fn barrier_js(quick: bool) -> String {
        let t_c: Vec<f64> = (0..=24).map(|i| 600.0 + 50.0 * i as f64).collect();
        let mut js = String::new();
        header(&mut js, "per-layer diffusion and the whole-particle walk");
        writeln!(js, "window.BARRIER_DATA = {{").unwrap();
        writeln!(js, "  tC: {},", arr(t_c.iter().copied())).unwrap();
        write!(js, "  thickness: {{").unwrap();
        for (name, l) in htr10_thicknesses() {
            write!(js, " \"{name}\": {},", e(l)).unwrap();
        }
        writeln!(js, " }},").unwrap();
        writeln!(js, "  radii: {},", arr(htr10_radii())).unwrap();
        writeln!(js, "  walker: [").unwrap();
        for (name, n) in walker_elements() {
            for (lname, layer) in layers() {
                match has_walker_coefficient(layer, n) {
                    Ok(()) => {
                        let d: Vec<Option<f64>> = t_c.iter().map(|&t| walker_d(layer, n, t)).collect();
                        writeln!(js, "    {{ el: \"{name}\", layer: \"{lname}\", d: {} }},", opt(d)).unwrap();
                    }
                    Err(why) => {
                        println!("  {name} in {lname}: no coefficient ({why})");
                        writeln!(js, "    {{ el: \"{name}\", layer: \"{lname}\", d: null, gap: \"{why}\" }},").unwrap();
                    }
                }
            }
        }
        writeln!(js, "  ],").unwrap();
        // The graphite rows the walker cannot give, said once per element.
        for (name, n) in walker_elements() {
            if let Err(why) = has_walker_coefficient(TrisoPebbleLayerMaterial::MatrixGraphite, n) {
                println!("  {name} in matrix graphite: {why}");
            }
        }
        let temp = |c: f64| ThermodynamicTemperature::new::<degree_celsius>(c);
        writeln!(js, "  atops: [").unwrap();
        for (name, z) in [("Ag", 47u32), ("Cs", 55), ("Sr", 38), ("Kr", 36)] {
            let g: Vec<f64> = t_c.iter().map(|&t| diffusion_coefficient(z, temp(t), temp(t)).graphite.get::<square_meter_per_second>()).collect();
            let k: Vec<f64> = t_c.iter().map(|&t| diffusion_coefficient(z, temp(t), temp(t)).kernel.get::<square_meter_per_second>()).collect();
            writeln!(js, "    {{ el: \"{name}\", layer: \"graphite\", d: {} }},", arr(g)).unwrap();
            writeln!(js, "    {{ el: \"{name}\", layer: \"kernel\", d: {} }},", arr(k)).unwrap();
        }
        let s: Vec<f64> = t_c.iter().map(|&t| diffusion_coefficient_sic_ag(temp(t)).get::<square_meter_per_second>()).collect();
        writeln!(js, "    {{ el: \"Ag\", layer: \"SiC\", d: {} }},", arr(s)).unwrap();
        writeln!(js, "  ],").unwrap();
        write!(js, "  halfLife: {{").unwrap();
        for (k, n) in [("Cs", "Cs-137"), ("Sr", "Sr-90"), ("Ag", "Ag-110m"), ("Kr", "Kr-85")] {
            let t = find_nuclide(n).expect("in the TRISO-ATOPS table").half_life.get::<second>();
            write!(js, " {k}: {},", e(t)).unwrap();
        }
        writeln!(js, " }},").unwrap();

        // P1, P2 printed for the record.
        let th = htr10_thicknesses();
        let (ipyc, sic) = (th[2].1, th[3].1);
        for (name, n) in walker_elements() {
            for t in [800.0, 1000.0, 1200.0, 1400.0, 1600.0] {
                let tp = walker_d(TrisoPebbleLayerMaterial::PyC, n, t).map(|d| crossing_time(ipyc, d));
                let ts = walker_d(TrisoPebbleLayerMaterial::SiC, n, t).map(|d| crossing_time(sic, d));
                if let (Some(a), Some(b)) = (tp, ts) {
                    println!("  {name} {t} C: IPyC {a:.3e} s, SiC {b:.3e} s, ratio {:.1}", b / a);
                }
            }
        }

        // The whole-particle walk. boon-lay's own walker as a cost probe
        // (P15), and the exact mean exit time (P13) that checks the page's
        // lattice walk (P14, in check-fission.js).
        let budget: u64 = if quick { 2_000_000 } else { 20_000_000 };
        writeln!(js, "  wos: {{ captureNm: {}, probeSteps: {budget}, probes: [", walk::capture_eps_nm()).unwrap();
        for (name, nuc) in [("Cs", Nuclide::Cs137), ("Ag", Nuclide::Ag110m), ("Sr", Nuclide::Sr90)] {
            let t0 = std::time::Instant::now();
            let (rmax, per, entries, released) = walk::probe(nuc, 1600.0, 7, budget);
            let tot: u64 = per.iter().sum();
            println!(
                "  walker probe {name} 1600 C, {budget} steps ({:.1} s): max radius {:.2} um, steps per layer {:?} ({:.1} % buffer), IPyC entries {entries}, released {released}",
                t0.elapsed().as_secs_f64(), rmax * 1e6, per, 100.0 * per[1] as f64 / tot.max(1) as f64
            );
            writeln!(js, "    {{ el: \"{name}\", tC: 1600, rMax: {}, perLayer: [{}], ipycEntries: {entries}, released: {released}, seconds: {:.3} }},", e(rmax), per.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","), t0.elapsed().as_secs_f64()).unwrap();
        }
        writeln!(js, "  ], exact: [").unwrap();
        for (name, nuc) in [("Cs", Nuclide::Cs137), ("Ag", Nuclide::Ag110m), ("Sr", Nuclide::Sr90)] {
            for t in [1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0, 1600.0, 1700.0, 1800.0] {
                let (m, terms) = walk::exact_mean_exit(nuc, t).expect("Cs, Ag, Sr have every layer");
                let (k, big) = terms.iter().enumerate().fold((0, 0.0f64), |a, (k, &v)| if v > a.1 { (k, v) } else { a });
                println!("  exact mean exit {name} {t} C: {m:.4e} s, largest term layer {k} ({:.1} %)", 100.0 * big / m);
                writeln!(js, "    {{ el: \"{name}\", tC: {t}, mean: {}, terms: {} }},", e(m), arr(terms)).unwrap();
            }
        }
        writeln!(js, "  ] }}").unwrap();
        writeln!(js, "}};").unwrap();
        js
    }

    fn failure_js() -> String {
        let mut js = String::new();
        header(&mut js, "boon-lay fuel failure on the HTR-10 particle (an EXTRAPOLATION)");
        writeln!(js, "window.FAILURE_DATA = {{").unwrap();
        let (ri, ra, rm, d0, vk, vf) = ff::geometry();
        writeln!(js, "  tB: {},", ff::T_B_C).unwrap();
        writeln!(js, "  geometry: {{ ri: {}, ra: {}, rMean: {}, d0: {}, vk: {}, vf: {}, kernelUm: {} }},", e(ri), e(ra), e(rm), e(d0), e(vk), e(vf), ff::kernel_radius_um()).unwrap();
        let p = ff::particle(false);
        let p0 = ff::phi1_start();
        writeln!(js, "  phi1Start: {},", e(p0)).unwrap();
        writeln!(js, "  asManufacturedTarget: {},", e(ff::as_manufactured_target())).unwrap();
        let (s0, m0) = ff::strength_at(boon_lay::fuel_failure::htr10::STAND_IN_FLUENCE_E25_PER_M2);
        writeln!(
            js,
            "  strength: {{ unirradiatedMPa: {}, unirradiatedM: {}, fluence: {}, s0: {}, m: {}, burnup: {}, residenceDays: {} }},",
            boon_lay::fuel_failure::htr10::STAND_IN_STRENGTH_MPA,
            boon_lay::fuel_failure::htr10::STAND_IN_WEIBULL_MODULUS,
            boon_lay::fuel_failure::htr10::STAND_IN_FLUENCE_E25_PER_M2,
            e(s0),
            e(m0),
            boon_lay::fuel_failure::htr10::BURNUP_FIMA,
            boon_lay::fuel_failure::htr10::RESIDENCE_FULL_POWER_DAYS
        )
        .unwrap();
        let gammas: Vec<f64> = (0..=12).map(|i| 0.25 * i as f64).collect();
        let sg: Vec<(f64, f64)> = gammas.iter().map(|&g| ff::strength_at(g)).collect();
        writeln!(js, "  strengthVsFluence: {{ gamma: {}, s0: {}, m: {} }},", arr(gammas.iter().copied()), arr(sg.iter().map(|x| x.0)), arr(sg.iter().map(|x| x.1))).unwrap();

        // Pressure pieces on a grid (lesson 33).
        let tcs: Vec<f64> = (0..=12).map(|i| 1000.0 + 100.0 * i as f64).collect();
        let hrs = [0.0, 50.0, 200.0, 500.0];
        writeln!(js, "  pressure: {{ tC: {}, hours: {}, rows: [", arr(tcs.iter().copied()), arr(hrs)).unwrap();
        let mut worst_split: f64 = 0.0;
        for &h in &hrs {
            let parts: Vec<_> = tcs.iter().map(|&t| ff::pressure_parts(&p, t, h)).collect();
            for (i, &t) in tcs.iter().enumerate() {
                let drv = ff::pressure_at(&p, t, h);
                worst_split = worst_split.max(((parts[i].3 + parts[i].4) - drv).abs() / drv);
            }
            writeln!(
                js,
                "    {{ fd: {}, opf: {}, p: {}, pGas: {}, pCo: {}, sigma: {} }},",
                arr(parts.iter().map(|x| x.0)),
                arr(parts.iter().map(|x| x.1)),
                arr(parts.iter().map(|x| x.2)),
                arr(parts.iter().map(|x| x.3)),
                arr(parts.iter().map(|x| x.4)),
                arr(parts.iter().map(|x| ff::stress(x.2, 1.0)))
            )
            .unwrap();
        }
        writeln!(js, "  ] }},").unwrap();
        println!("  P7: worst |p_gas + p_CO - pressure_at| / p = {worst_split:.2e}");
        let bu: Vec<(f64, f64)> = [0.02, 0.04, 0.06, 0.0851, 0.10, 0.12]
            .iter()
            .map(|&f| (f, ff::pressure_parts(&ff::particle_at_burnup(f), 1600.0, 200.0).2))
            .collect();
        writeln!(js, "  burnup: {{ fima: {}, p1600at200h: {} }},", arr(bu.iter().map(|x| x.0)), arr(bu.iter().map(|x| x.1))).unwrap();
        let p04 = bu[1].1;
        let p12 = bu[5].1;
        println!("  P12: p(0.12) / p(0.04) at 1600 C, 200 h = {:.3}", p12 / p04);

        // Weibull check points (lesson 34).
        let mut wc = Vec::new();
        for (s, s0, m) in [(s0, s0, m0), (0.5 * s0, s0, m0), (0.25 * s0, s0, m0), (1.5 * s0, s0, m0), (2e8, 6e8, 8.0), (2e8, 6e8, 4.0), (1e8, 4e8, 2.0)] {
            wc.push(format!("[{},{},{},{}]", e(s), e(s0), e(m), e(ff::weibull(s, s0, m))));
        }
        println!("  P7: phi_1(sigma_o) - 0.5 = {:.2e}", ff::weibull(s0, s0, m0) - 0.5);
        writeln!(js, "  weibullCheck: [{}],", wc.join(",")).unwrap();

        // Thinning (lesson 35).
        let hh: Vec<f64> = (0..=20).map(|i| 25.0 * i as f64).collect();
        writeln!(js, "  thinning: {{ hours: {}, rows: [", arr(hh.iter().copied())).unwrap();
        for t in [1400.0, 1600.0, 1800.0, 2000.0, 2200.0] {
            writeln!(js, "    {{ tC: {t}, fkor: {} }},", arr(hh.iter().map(|&h| ff::thinning(t, h, 20)))).unwrap();
        }
        writeln!(js, "  ] }},").unwrap();
        let ct: Vec<f64> = (0..=16).map(|i| 1000.0 + 100.0 * i as f64).collect();
        let cr: Vec<(f64, f64)> = ct.iter().map(|&t| ff::corrosion(t)).collect();
        let (pf, pp) = ff::prefactors();
        writeln!(js, "  corrosion: {{ tC: {}, figure: {}, printed: {}, figurePrefactor: {}, printedPrefactor: {} }},", arr(ct.iter().copied()), arr(cr.iter().map(|x| x.0)), arr(cr.iter().map(|x| x.1)), e(pf), e(pp)).unwrap();
        let dt: Vec<f64> = (0..=16).map(|i| 1600.0 + 50.0 * i as f64).collect();
        let dd: Vec<_> = dt.iter().map(|&t| ff::decomposition(t, 200.0)).collect();
        writeln!(js, "  decomposition: {{ tC: {}, k: {}, zeta200: {}, sphere200: {}, loose200: {} }},", arr(dt.iter().copied()), arr(dd.iter().map(|x| x.0)), arr(dd.iter().map(|x| x.1)), arr(dd.iter().map(|x| x.2)), arr(dd.iter().map(|x| x.3))).unwrap();
        let gg: Vec<_> = ct.iter().map(|&t| ff::grain_boundary(t, 200.0, m0)).collect();
        writeln!(js, "  grainBoundary: {{ tC: {}, rate: {}, m200: {}, m0: {} }},", arr(ct.iter().copied()), arr(gg.iter().map(|x| x.0)), arr(gg.iter().map(|x| x.1)), e(m0)).unwrap();

        // Accidents (lessons 36-37): isothermal holds, GB off and on, 500 h
        // in 5 h steps; and the illustrative heat-ups.
        let row = |w: &ff::Row| format!("[{},{},{},{},{},{},{},{},{}]", w.hours, w.t_c, e(w.fkor), e(w.zeta), e(w.p_pa), e(w.sigma_pa), e(w.phi1), e(w.phi2), e(w.total));
        writeln!(js, "  rowFields: [\"hours\",\"tC\",\"fkor\",\"zeta\",\"p\",\"sigma\",\"phi1\",\"phi2\",\"total\"],").unwrap();
        writeln!(js, "  holds: [").unwrap();
        for t in [1200.0, 1400.0, 1600.0, 1800.0, 2000.0, 2200.0] {
            for gb in [false, true] {
                let rows = ff::run_history(&ff::particle(gb), &ff::hold(t, 500.0, 100));
                let w200 = rows.iter().find(|w| (w.hours - 200.0).abs() < 1e-9).copied().expect("a 200 h row");
                let w500 = *rows.last().unwrap();
                println!(
                    "  hold {t} C gb={gb}: 200 h sigma {:.1} MPa phi1 {:.3e} phi2 {:.3e} FKOR {:.4}; 500 h phi1 {:.3e}",
                    w200.sigma_pa / 1e6, w200.phi1, w200.phi2, w200.fkor, w500.phi1
                );
                let rel = if gb {
                    String::new()
                } else {
                    let rel = |z| arr(ff::kernel_release(z, &rows));
                    format!(", release: {{ Cs: {}, Sr: {}, Kr: {} }}", rel(55), rel(38), rel(36))
                };
                writeln!(js, "    {{ tC: {t}, gb: {gb}, rows: [{}]{rel} }},", rows.iter().map(row).collect::<Vec<_>>().join(",")).unwrap();
            }
        }
        writeln!(js, "  ],").unwrap();
        writeln!(js, "  ramps: [").unwrap();
        for peak in [1400.0, 1600.0, 1800.0, 2000.0] {
            let rows = ff::run_history(&p, &ff::ramp(peak));
            let mono = rows.windows(2).all(|w| w[1].phi1 >= w[0].phi1 && w[1].phi2 >= w[0].phi2);
            println!("  ramp to {peak} C: final phi1 {:.3e} phi2 {:.3e}, never falls: {mono}", rows.last().unwrap().phi1, rows.last().unwrap().phi2);
            let rel = |z| arr(ff::kernel_release(z, &rows));
            writeln!(js, "    {{ peak: {peak}, rows: [{}], release: {{ Cs: {}, Sr: {}, Kr: {} }} }},", rows.iter().map(row).collect::<Vec<_>>().join(","), rel(55), rel(38), rel(36)).unwrap();
        }
        writeln!(js, "  ],").unwrap();
        let eoi: Vec<(f64, f64)> = [700.0, 776.0, 900.0, 1000.0].iter().map(|&t| (t, ff::eoi(t))).collect();
        for (t, v) in &eoi {
            println!("  end of irradiation T_B {t} C: phi1 {v:.3e}");
        }
        writeln!(js, "  eoi: {{ tB: {}, phi1: {} }}", arr(eoi.iter().map(|x| x.0)), arr(eoi.iter().map(|x| x.1))).unwrap();
        writeln!(js, "}};").unwrap();
        js
    }
}
