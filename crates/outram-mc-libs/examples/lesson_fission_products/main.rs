//! **The data of the Fission products lessons** (track started 2026-10-20,
//! maintainer design): every number the pages in
//! `docs/site/science/fission-products/` use, read here and written to that
//! folder's data files.
//!
//! ```bash
//! RUSTUP_TOOLCHAIN=1.95 cargo run --release -p outram-mc-libs --example lesson_fission_products [out_dir]
//! ```
//!
//! - `fission-data.js` (lesson 1, one fission step by step): for U-235 and
//!   U-238 (the ENDF/B-VIII.0 tapes `reference-data/endf/` holds), the
//!   MF=1/MT=458 energy release, ν̄ (MT=452, 456, 455), the delayed-neutron
//!   groups (MT=455 decay constants, MF=5/MT=455 group spectra), the
//!   MF=5/MT=18 prompt spectrum at the yield energies, the resolved
//!   resonances' widths (MF=2), the capture Q (MF=3/MT=102), and the
//!   independent fission yields; with the NNDC mass excesses and
//!   ground-state decays for every nuclide from Z = 20 to 72.
//! - `yields-data.js` (lesson 2, split atoms): the ENDF/B-VIII.0 independent
//!   yields of U-235, U-238, Pu-239 and U-233 at every incident energy
//!   fission-yields-data tabulates, by (Z, A) and summed to Y(A); the
//!   peak-to-valley ratio of each Y(A); and Y(A) as the crate interpolates it
//!   at energies between the tabulated ones, so the page's own interpolation
//!   can be checked against the crate's.
//! - `spectrum-data.js` (lesson 3, the prompt fission neutron spectrum): for
//!   U-235 and U-238, MF=5/MT=18's flags (read with `acer::energy`'s
//!   `parse_mf5_section`), its incident-energy grid, and on a slider grid of
//!   incident energies: a histogram of outgoing energies drawn through the
//!   Monte Carlo's own sampler, the exact bin probabilities of that sampling
//!   law, and its mean and most probable outgoing energies (`spectrum.rs`).
//!
//! **Predictions, written 2026-10-20 before the first run of the lesson 2
//! and 3 parts** (the lesson 1 file had already printed three MF=5 means,
//! 2.0002, 2.0103 and 2.0181 MeV for U-235 at 1e-5 eV, 500 keV and 14 MeV, so
//! the spectrum prediction is not blind at those three points):
//!
//! - Peak-to-valley of Y(A) falls with incident energy for every parent:
//!   hundreds or more at thermal, of order ten at 14 MeV.
//! - The mean prompt-neutron energy rises slowly with incident energy, by a
//!   few per cent over the whole grid, except where multi-chance fission
//!   opens (above about 6 MeV), where the pre-fission neutrons could pull it
//!   down. The most probable energy sits well below the mean (the spectrum
//!   has a long high-energy tail), below 1 MeV.
//!
//! What the runs gave is in the page text and in `check-fission.js`.
//!
//! Nothing here is fitted or compared with a target; the comparisons are in
//! `docs/site/science/fission-products/check-fission.js`. The readers are in
//! `data.rs` and `spectrum.rs`, reached by `tests/lesson_fission_products.rs`.
//!
//! Driver glue (`main`, `write_*`, `arr`): formats what `data.rs` and
//! `spectrum.rs` compute; its output is checked by `check-fission.js`.

#[path = "data.rs"]
mod data;
#[path = "spectrum.rs"]
mod spectrum;

use data::*;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::delayed::{DelayedChi, DelayedNuBar};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::acer::nu::NuRepr;
use njoy_outram_park_fork::reference_data::reference_endf;
use std::fmt::Write as _;

fn arr(v: impl IntoIterator<Item = f64>, prec: usize) -> String {
    let s: Vec<String> = v.into_iter().map(|x| format!("{x:.prec$e}")).collect();
    format!("[{}]", s.join(","))
}

fn main() -> Result<(), String> {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/fission-products").to_string()
    });
    let src = fission_yields_data_src()?;
    let masses = mass_excess_table(&std::fs::read_to_string(src.join("nndc_nudat_z_n_and_mass_excess.csv")).map_err(|e| e.to_string())?);
    let decays = decay_table(&std::fs::read_to_string(src.join("half_life_branching_ratio_nndc_nudat_data_export.csv")).map_err(|e| e.to_string())?);
    println!("NNDC tables from {}: {} masses, {} ground-state decays", src.display(), masses.len(), decays.len());
    let products = product_nuclides();
    println!("{} product nuclide names in the yield enum (Z 20-72, A 60-180, with isomers)", products.len());

    let mut header = String::new();
    let mut parents_js = Vec::new();
    for (label, z, a, nuc, tape_file) in parents() {
        let Some(tape_file) = tape_file else { continue };
        let path = reference_endf(tape_file).ok_or_else(|| format!("{tape_file} not in reference-data/endf"))?;
        let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mat = tape.materials()[0];
        let awr = tape.section(mat, 3, 18).map(|s| s.rows[0][1]).ok_or("no MF=3/MT=18")?;
        let sn = capture_q(&tape, mat).ok_or("no MT=102")?;
        let er = read_mt458(&tape, mat)?;
        let nu = NuBar::from_endf(&tape, mat).map_err(|e| e.to_string())?.ok_or("no MT=452")?;
        let nup = match NuRepr::from_endf(&tape, mat, 456).map_err(|e| e.to_string())? {
            Some(NuRepr::Tabulated { e_mev, nu, .. }) => e_mev.iter().map(|e| e * 1e6).zip(nu).collect::<Vec<_>>(),
            _ => return Err("MT=456 is not tabulated".into()),
        };
        let dnu = DelayedNuBar::from_endf(&tape, mat).map_err(|e| e.to_string())?.ok_or("no MT=455")?;
        let dchi = DelayedChi::from_endf(&tape, mat).map_err(|e| e.to_string())?.ok_or("no MF=5/MT=455")?;
        let chi = match FissionSpectrum::from_endf_mf5(&tape, mat).map_err(|e| e.to_string())? {
            Some(FissionSpectrum::ContinuousTabular(t)) => t,
            _ => return Err("MF=5/MT=18 is not LF=1".into()),
        };
        let res = resonances(&tape, mat)?;
        let near = res.list.iter().min_by(|x, y| (x.0 - 0.0253).abs().total_cmp(&(y.0 - 0.0253).abs())).copied().ok_or("no resonances")?;
        let mean_gamma = res.list.iter().map(|r| r.4).sum::<f64>() / res.list.len() as f64;
        let radius = channel_radius_fm(res.awri);

        let _ = writeln!(header, "//   {label} ({tape_file}, MAT {mat}): AWR {awr:.4}; MT=102 QM (S_n) {:.4} MeV; MT=458 LFC={} NPLY={}", sn / 1e6, er.lfc, er.nply);
        let comps: Vec<String> = (0..9).map(|k| format!("{} {:.4}±{:.2}", MT458_NAMES[k], er.thermal[k].0 / 1e6, er.thermal[k].1 / 1e6)).collect();
        let _ = writeln!(header, "//     MT=458 thermal point [MeV]: {}", comps.join(", "));
        let tabs: Vec<String> = er.tabulated.iter().map(|(k, t)| format!("{} {} points, {:.4} at {:.3e} eV to {:.4} at {:.3e} eV", MT458_NAMES[(*k - 1) as usize], t.len(), t[0].1 / 1e6, t[0].0, t[t.len() - 1].1 / 1e6, t[t.len() - 1].0)).collect();
        let _ = writeln!(header, "//     MT=458 tabulated (LFC=1) [MeV]: {}", tabs.join("; "));
        let _ = writeln!(header, "//     nu-bar at 0.0253 eV: total (MT=452) {:.5}, prompt (MT=456) {:.5}, delayed (MT=455) {:.5}", nu.at(0.0253), linlin(&nup, 0.0253), dnu.nu_delayed_at(0.0253));
        let _ = writeln!(header, "//     MT=455 decay constants [1/s]: {:?}; group fractions {:?}", dnu.lambda, dchi.groups.iter().map(|g| g.fraction[0].1).collect::<Vec<_>>());
        let _ = writeln!(header, "//     MF=2 Reich-Moore range {:.3e}-{:.3e} eV, {} resonances, AWRI {:.4}; nearest 0.0253 eV: E_r {:.4} eV, Gamma {:.4e} eV (n {:.3e}, gamma {:.3e}, f {:.3e}); mean Gamma {:.4e} eV; ENDF-102 channel radius {radius:.4} fm", res.el, res.eh, res.list.len(), res.awri, near.0, near.4, near.1, near.2, near.3, mean_gamma);
        println!("{label}: MAT {mat}, S_n {:.4} MeV, EFR {:.3} MeV, nu_p(thermal) {:.4}, nearest resonance {:.4} eV Gamma {:.4e} eV", sn / 1e6, er.thermal[0].0 / 1e6, linlin(&nup, 0.0253), near.0, near.4);

        // Yields at each tabulated energy, and the prompt spectrum there.
        let mut yields_js = Vec::new();
        for e in tabulated_energies(nuc) {
            let y = independent_yields(nuc, e, &products);
            let sum = yield_sum(&y);
            println!("  yields at {e:.4e} eV: {} (Z,A) with Y > 0, sum {sum:.6}", y.len());
            let _ = writeln!(header, "//     independent yields at {e:.4e} eV: {} (Z,A) summed over isomers, sum {sum:.6}{}", y.len(), if sum < 1.0 { " -- NOT EVALUATED, left out" } else { "" });
            if sum < 1.0 {
                continue;
            }
            // The MF=5/MT=18 table at the incident-grid point at or below e.
            let k = chi.incident.iter().rposition(|&x| x <= e).unwrap_or(0);
            let t = &chi.tables[k];
            let (se, sp) = thin(&t.e_out, &t.pdf, (t.e_out.len() / 160).max(1));
            let mean = mean_energy(&t.e_out, &t.pdf, t.linlin);
            let _ = writeln!(header, "//       MF=5/MT=18 table used: incident {:.4e} eV, {} points, mean outgoing {:.4} MeV", chi.incident[k], t.e_out.len(), mean / 1e6);
            let zs: Vec<String> = y.keys().map(|k| k.0.to_string()).collect();
            let as_: Vec<String> = y.keys().map(|k| k.1.to_string()).collect();
            yields_js.push(format!(
                "{{\"e\":{e:.6e},\"z\":[{}],\"a\":[{}],\"y\":{},\"sum\":{sum:.8},\"chi\":{{\"incident\":{:.6e},\"mean\":{mean:.6e},\"linlin\":{},\"e\":{},\"p\":{}}},\"efr\":{:.6e},\"enp\":{:.6e},\"nup\":{:.6e},\"nud\":{:.6e}}}",
                zs.join(","), as_.join(","), arr(y.values().copied(), 4), chi.incident[k], t.linlin, arr(se, 5), arr(sp, 5),
                er.at(1, e), er.at(2, e), linlin(&nup, e), dnu.nu_delayed_at(e)
            ));
        }
        let groups: Vec<String> = dchi.groups.iter().map(|g| {
            let e: Vec<f64> = g.spectrum.iter().map(|p| p.0).collect();
            let p: Vec<f64> = g.spectrum.iter().map(|p| p.1).collect();
            // Every bin: the group spectra are histograms (INT=1), and a
            // thinned histogram would be a different spectrum.
            format!("{{\"frac\":{:.6e},\"lf\":{},\"e\":{},\"p\":{}}}", g.fraction[0].1, g.lf, arr(e, 4), arr(p, 4))
        }).collect();
        let thermal: Vec<String> = er.thermal.iter().map(|c| format!("[{:.6e},{:.6e}]", c.0, c.1)).collect();
        let tab: Vec<String> = er.tabulated.iter().map(|(k, t)| format!("\"{}\":[{},{}]", k, arr(t.iter().map(|p| p.0), 5), arr(t.iter().map(|p| p.1), 6))).collect();
        parents_js.push(format!(
            "\"{label}\":{{\"z\":{z},\"a\":{a},\"mat\":{mat},\"awr\":{awr:.6},\"sn\":{sn:.6e},\"mt458\":{{\"lfc\":{},\"names\":[{}],\"thermal\":[{}],\"tab\":{{{}}}}},\"lambda\":{},\"groups\":[{}],\"res\":{{\"el\":{:.6e},\"eh\":{:.6e},\"n\":{},\"near\":{},\"mean\":{mean_gamma:.6e},\"radius_fm\":{radius:.6}}},\"yields\":[{}]}}",
            er.lfc, MT458_NAMES.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(","), thermal.join(","), tab.join(","),
            arr(dnu.lambda.iter().copied(), 6), groups.join(","), res.el, res.eh, res.list.len(),
            arr([near.0, near.1, near.2, near.3, near.4], 6), yields_js.join(",")
        ));
    }

    // Masses and decays, Z = 0 (neutron), 2 (He-4), 20-72, and the parents'
    // compound nuclei.
    let mut mz = Vec::new();
    let mut zs: Vec<u32> = vec![0, 2];
    zs.extend(20..=72);
    zs.extend([92, 94]);
    for z in zs {
        let row: Vec<(u32, f64)> = masses.iter().filter(|(&(zz, _), _)| zz == z).map(|(&(_, a), &d)| (a, d)).collect();
        if row.is_empty() {
            continue;
        }
        let a0 = row[0].0;
        let mut v = Vec::new();
        let mut a_prev = a0 - 1;
        for (a, d) in row {
            for _ in a_prev + 1..a {
                v.push("null".to_string());
            }
            v.push(format!("{d:.0}"));
            a_prev = a;
        }
        mz.push(format!("\"{z}\":[{a0},{}]", v.join(",")));
    }
    let mut dz = Vec::new();
    for (&(z, a), d) in &decays {
        if !(20..=72).contains(&z) {
            continue;
        }
        if let Some(t) = d.half_life_s {
            dz.push(format!("\"{z},{a}\":[{t:.5e},{},{}]", d.beta_minus, d.beta_n));
        }
    }
    let file = format!(
        "// The data of Fission products lesson 1 (one fission, step by step).\n\
         // GENERATED by crates/outram-mc-libs/examples/lesson_fission_products/main.rs:\n\
         //   RUSTUP_TOOLCHAIN=1.95 cargo run --release -p outram-mc-libs --example lesson_fission_products\n\
         // Do not edit; checked by check-fission.js. Sources: ENDF/B-VIII.0 tapes in\n\
         // reference-data/endf/ read through njoy-outram-park-fork; independent fission\n\
         // yields (ENDF/B-VIII.0) and the NNDC NuDat mass and decay tables shipped in\n\
         // fission-yields-data 0.1.5. Energies in eV, mass excesses in keV (NuDat export,\n\
         // rounded there to 3 significant figures), half-lives in s, branchings in %.\n\
         // masses: \"Z\": [A of the first entry, mass excess per A ...]; decays:\n\
         // \"Z,A\": [half-life, beta-minus %, beta-delayed-neutron %] (ground states;\n\
         // a nuclide absent from it is stable in the NuDat table). Read from the tapes:\n\
         {header}\
         window.FISSION_DATA = {{\"masses\":{{{}}},\"decays\":{{{}}},\"parents\":{{{}}}}};\n",
        mz.join(","), dz.join(","), parents_js.join(",")
    );
    let out = format!("{out_dir}/fission-data.js");
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    write_yields(&out_dir, &products)?;
    write_spectrum(&out_dir)?;
    Ok(())
}

/// `yields-data.js`: lesson 2's yields, peak-to-valley ratios and the
/// crate's own interpolation at energies between the tabulated ones.
fn write_yields(out_dir: &str, products: &[(fission_yields_data::prelude::Nuclide, u32, u32)]) -> Result<(), String> {
    // Energies between the tabulated ones at which the crate's interpolated
    // Y(A) is written, for check-fission.js to compare the page against.
    const BETWEEN: [f64; 4] = [1.0e3, 1.0e5, 1.0e6, 5.0e6];
    let mut header = String::new();
    let mut js = Vec::new();
    for (label, z, a, nuc, _) in parents() {
        let acn = a + 1;
        let mut tables = Vec::new();
        for e in tabulated_energies(nuc) {
            let y = independent_yields(nuc, e, products);
            let sum = yield_sum(&y);
            if sum < 1.0 {
                let _ = writeln!(header, "//   {label} at {e:.4e} eV: yields sum to {sum:.6} -- not evaluated, left out");
                continue;
            }
            let m = mass_yields(&y);
            let pv = peak_valley(&m, acn as f64 / 2.0).ok_or("no peaks")?;
            let _ = writeln!(header, "//   {label} at {e:.4e} eV: {} (Z,A), sum {sum:.6}; peaks A={} ({:.4e}) and A={} ({:.4e}), valley A={} ({:.4e}); peak-to-valley {:.2}",
                y.len(), pv.light_peak.0, pv.light_peak.1, pv.heavy_peak.0, pv.heavy_peak.1, pv.valley.0, pv.valley.1, pv.ratio);
            println!("{label} {e:.4e} eV: peak-to-valley {:.2}", pv.ratio);
            // By element: "Z": [A of the first entry, Y(Z, A) per A ...].
            let mut by_z: std::collections::BTreeMap<u32, Vec<(u32, f64)>> = Default::default();
            for (&(zz, aa), &v) in &y {
                by_z.entry(zz).or_default().push((aa, v));
            }
            let zs: Vec<String> = by_z.iter().map(|(zz, row)| {
                let a0 = row[0].0;
                let mut v = vec![0.0; (row[row.len() - 1].0 - a0 + 1) as usize];
                for &(aa, yy) in row { v[(aa - a0) as usize] = yy; }
                format!("\"{zz}\":[{a0},{}]", v.iter().map(|x| if *x == 0.0 { "0".to_string() } else { format!("{x:.4e}") }).collect::<Vec<_>>().join(","))
            }).collect();
            let a0 = *m.keys().next().ok_or("empty")?;
            let a1 = *m.keys().last().ok_or("empty")?;
            let mass: Vec<String> = (a0..=a1).map(|aa| format!("{:.5e}", m.get(&aa).copied().unwrap_or(0.0))).collect();
            tables.push(format!(
                "{{\"e\":{e:.6e},\"sum\":{sum:.8},\"zy\":{{{}}},\"mass\":[{a0},{}],\"pv\":{{\"light\":[{},{:.5e}],\"heavy\":[{},{:.5e}],\"valley\":[{},{:.5e}],\"ratio\":{:.6e}}}}}",
                zs.join(","), mass.join(","), pv.light_peak.0, pv.light_peak.1, pv.heavy_peak.0, pv.heavy_peak.1, pv.valley.0, pv.valley.1, pv.ratio
            ));
        }
        let e_lo = tabulated_energies(nuc).into_iter().find(|&e| yield_sum(&independent_yields(nuc, e, products)) >= 1.0).unwrap_or(0.0253);
        let between: Vec<String> = BETWEEN.iter().filter(|&&e| e > e_lo).map(|&e| {
            let m = mass_yields(&independent_yields(nuc, e, products));
            let a0 = m.keys().next().copied().unwrap_or(0);
            let a1 = m.keys().last().copied().unwrap_or(0);
            format!("{{\"e\":{e:.6e},\"mass\":[{a0},{}]}}", (a0..=a1).map(|aa| format!("{:.5e}", m.get(&aa).copied().unwrap_or(0.0))).collect::<Vec<_>>().join(","))
        }).collect();
        js.push(format!("\"{label}\":{{\"z\":{z},\"a\":{a},\"tables\":[{}],\"between\":[{}]}}", tables.join(","), between.join(",")));
    }
    let file = format!(
        "// The data of Fission products lesson 2 (split atoms).\n\
         // GENERATED by crates/outram-mc-libs/examples/lesson_fission_products/main.rs:\n\
         //   RUSTUP_TOOLCHAIN=1.95 cargo run --release -p outram-mc-libs --example lesson_fission_products\n\
         // Do not edit; checked by check-fission.js. ENDF/B-VIII.0 independent fission\n\
         // yields (MT=454) through fission-yields-data 0.1.5, summed over isomeric states.\n\
         // Per parent: \"tables\", one per incident energy [eV] the crate tabulates, with\n\
         // zy: \"Z\": [A of the first entry, Y(Z,A) ...], mass: [A of the first entry, Y(A) ...],\n\
         // and pv, the peak-to-valley numbers of Y(A) ([A, Y] of each peak and of the\n\
         // valley, and the ratio of the mean of the peaks to the valley); \"between\": Y(A)\n\
         // as the crate interpolates it at energies between the tabulated ones.\n\
         {header}\
         window.YIELDS_DATA = {{{}}};\n",
        js.join(",")
    );
    let out = format!("{out_dir}/yields-data.js");
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}

/// `spectrum-data.js`: lesson 3's prompt spectra, sampled through the MC's
/// sampler and computed exactly, on a slider grid of incident energies.
fn write_spectrum(out_dir: &str) -> Result<(), String> {
    use njoy_outram_park_fork::acer::energy::{parse_mf5_section, Mf5Law};
    const WIDTH: f64 = 1.25e5; // bin width [eV]
    const NBINS: usize = 120; // 0 to 15 MeV
    const SAMPLES: usize = 100_000;
    const PER_DECADE: usize = 6;
    let mut header = String::new();
    let mut js = Vec::new();
    for (label, _, _, _, tape_file) in parents() {
        let Some(tape_file) = tape_file else { continue };
        let path = reference_endf(tape_file).ok_or_else(|| format!("{tape_file} not in reference-data/endf"))?;
        let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mat = tape.materials()[0];
        let sec = tape.section(mat, 5, 18).ok_or("no MF=5/MT=18")?;
        let subs = parse_mf5_section(sec).map_err(|e| e.to_string())?;
        let lfs: Vec<String> = subs.iter().map(|s| match &s.law { Mf5Law::Tabulated(_) => "1".to_string(), Mf5Law::Evaporation { ace_law, .. } => ace_law.to_string(), Mf5Law::Watt { .. } => "11".to_string() }).collect();
        let mf3_chances: Vec<i32> = [19, 20, 21, 38].into_iter().filter(|&mt| tape.section(mat, 3, mt).is_some()).collect();
        let spectrum = FissionSpectrum::from_endf_mf5(&tape, mat).map_err(|e| e.to_string())?.ok_or("no MF=5/MT=18")?;
        let FissionSpectrum::ContinuousTabular(chi) = &spectrum else { return Err(format!("{label}: MF=5/MT=18 is not one LF=1 law")) };
        let _ = writeln!(header, "//   {label} ({tape_file}, MAT {mat}): MF=5/MT=18 NK={} LF={}; incident grid {} energies, {:.4e} to {:.4e} eV, TAB2 interpolation {:?}; MF=3 partial-chance fission MTs present: {:?}",
            subs.len(), lfs.join(","), chi.incident.len(), chi.incident[0], chi.incident[chi.incident.len() - 1], chi.incident_interp, mf3_chances);
        // The slider grid: PER_DECADE log steps from the lowest tabulated
        // incident energy to 20 MeV, plus every tabulated energy in range and
        // 0.0253 eV, lesson 2's thermal energy.
        let (lo, hi) = (chi.incident[0], 2.0e7f64.min(chi.incident[chi.incident.len() - 1]));
        let nd = ((hi / lo).log10() * PER_DECADE as f64).ceil() as usize;
        let mut grid: Vec<f64> = (0..=nd).map(|k| lo * 10f64.powf(k as f64 / PER_DECADE as f64)).filter(|&e| e <= hi * 1.000001).collect();
        grid.extend(chi.incident.iter().copied().filter(|&e| e >= lo && e <= hi));
        grid.push(0.0253);
        grid.sort_by(f64::total_cmp);
        grid.dedup_by(|a, b| (*a / *b - 1.0).abs() < 1e-9);
        let mut rows = Vec::new();
        for (k, &e) in grid.iter().enumerate() {
            let tab = chi.incident.iter().any(|&t| (t / e - 1.0).abs() < 1e-9);
            let (i, r) = spectrum::bracket(chi, e);
            let (counts, over, sum) = spectrum::sample_histogram(&spectrum, e, SAMPLES, (k * SAMPLES) as u64, WIDTH, NBINS);
            let (prob, pover) = spectrum::bin_probabilities(chi, e, WIDTH, NBINS);
            let mean = spectrum::law_mean(chi, e);
            let mode = spectrum::law_mode(chi, e);
            if tab {
                let _ = writeln!(header, "//     tabulated {e:.4e} eV: mean {:.4} MeV (sampled {:.4}), most probable {:.4} MeV", mean / 1e6, sum / SAMPLES as f64 / 1e6, mode / 1e6);
            }
            rows.push(format!(
                "{{\"e\":{e:.6e},\"tab\":{tab},\"i\":{i},\"r\":{r:.6e},\"mean\":{mean:.6e},\"mode\":{mode:.6e},\"sum\":{sum:.6e},\"n\":{SAMPLES},\"over\":{over},\"pover\":{pover:.4e},\"counts\":[{}],\"prob\":[{}]}}",
                counts.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
                prob.iter().map(|p| format!("{p:.4e}")).collect::<Vec<_>>().join(",")
            ));
        }
        println!("{label}: MF=5/MT=18 NK={} LF={}, {} tabulated incident energies, {} slider energies", subs.len(), lfs.join(","), chi.incident.len(), grid.len());
        js.push(format!(
            "\"{label}\":{{\"nk\":{},\"lf\":[{}],\"incident\":{},\"interp\":{:?},\"chances\":{:?},\"width\":{WIDTH:e},\"nbins\":{NBINS},\"grid\":[{}]}}",
            subs.len(), lfs.join(","), arr(chi.incident.iter().copied(), 6), chi.incident_interp.iter().map(|&(n, i)| [n, i]).collect::<Vec<_>>(), mf3_chances, rows.join(",")
        ));
    }
    let file = format!(
        "// The data of Fission products lesson 3 (the prompt fission neutron spectrum).\n\
         // GENERATED by crates/outram-mc-libs/examples/lesson_fission_products/main.rs:\n\
         //   RUSTUP_TOOLCHAIN=1.95 cargo run --release -p outram-mc-libs --example lesson_fission_products\n\
         // Do not edit; checked by check-fission.js. ENDF/B-VIII.0 MF=5/MT=18 read by\n\
         // njoy-outram-park-fork; energies in eV. Per slider energy e: the bracket (i, r)\n\
         // the sampler uses, the exact mean and most probable outgoing energy of the\n\
         // sampling law, the sum of n draws through outram-mc-libs' sampler, their counts\n\
         // in bins of `width` from 0 (and the overflow), and each bin's exact probability.\n\
         {header}\
         window.SPECTRUM_DATA = {{{}}};\n",
        js.join(",")
    );
    let out = format!("{out_dir}/spectrum-data.js");
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
