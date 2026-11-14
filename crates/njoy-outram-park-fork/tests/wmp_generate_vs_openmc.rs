// SPDX-License-Identifier: AGPL-3.0-only

//! **Windowed-multipole generation against OpenMC's own generator** (gh:#1390).
//!
//! # Methodology
//!
//! Reference: OpenMC `8f79d9d` (2026-11-10) `WindowedMultipole.from_endf` on
//! ENDF/B-VIII.0 Mg-26 (`reference-data/endf/n-012_Mg_026-ENDF8.0.endf`,
//! MLBW, resolved range to 450 keV), with NJOY2016.80 as its `njoy`,
//! numpy 2.4.6 / scipy 1.17.1 (OpenBLAS) under Python 3.11. The run dumped
//! its intermediates (script and recipe:
//! `verification_and_validation/wmp_generate/README.md`) into
//! `verification_and_validation/wmp_generate/mg26_openmc_8f79d9d.txt`,
//! floats as shortest round-trip decimals, so nothing is lost in transport.
//!
//! Each stage of the port is run on **the reference's own input to that
//! stage**, so a difference is attributed to the stage that made it:
//!
//! | stage | port input | compared | gate as predicted before running |
//! |---|---|---|---|
//! | front end | the ENDF tape | grid, elastic, MT=27, AWR, E range | bit-identical |
//! | one `vectfit` call | OpenMC's first call's `f`, `s`, poles, weights | poles, residues | < 1e-8 relative |
//! | `_vectfit_xs` | OpenMC's pointwise input | pole count; sigma on the 10x test grid | count identical; every point within `rtol = 1e-3` or `atol = 1e-5` b (upstream's own "ideal" test) |
//! | `_windowing` (100 windows, order 5) and `from_multipole` search | OpenMC's multipole data | windows, spacing, used poles; curve fit | identical; < 1e-8 of each coefficient vector's largest entry |
//! | whole chain | the ENDF tape | sigma(E, T) of the port's library against OpenMC's | within `rtol` / `atol` |
//!
//! The 1e-8 gates and "every point within tolerance" are the prediction's;
//! they failed on the first run and are kept, unchanged, in the ignored test
//! `mg26_coefficients_within_code_to_code_allowance`. The binding test holds
//! the identities, the pole count, and the cross sections (`rtol`/`atol`,
//! and "no worse than OpenMC" on upstream's own test, because OpenMC's own
//! fit leaves 730 points outside).
//!
//! # Results (2026-11-14)
//!
//! | stage | result |
//! |---|---|
//! | front end | 2115 grid points, elastic and MT=27 values **bit-identical**; AWR, name bit-identical |
//! | one `vectfit` call | poles 7.3e-8, residues 8.1e-4 relative (OpenMC against itself, input +-1 ulp: 6e-9, 2e-5) |
//! | `_vectfit_xs` | 8 poles, as OpenMC; poles 2.1e-3, residues 9.4e-3 relative (OpenMC against itself: 2.5e-4, 9.3e-4); 730 of 42 282 test points outside `rtol`/`atol` for both, worst 4.963e-3 for both |
//! | `_windowing` 100 x order 5 | windows, spacing, used poles **identical**; curve fit 1.6e-7 of scale (OpenMC against itself: 3.2e-7); sigma 5.2e-9 relative |
//! | `from_multipole` search | 110 windows, order 2, 4 poles, **identical**; curve fit 2.1e-9 of scale |
//! | whole chain from the tape | same shape as OpenMC's library; sigma(E, T) at 0, 293.6, 1200 K within `atol` at all 2400 values |
//!
//! Causes and the self-sensitivity instrument:
//! `verification_and_validation/wmp_generate/README.md`. Runtime ~7.5 min.
//!
//! Skips (passes, printing `SKIP`) when the ENDF tape is absent.

use std::collections::HashMap;
use std::path::PathBuf;

use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::wmp::generate::vectfit::{evaluate, vectfit};
use njoy_outram_park_fork::wmp::generate::{
    pointwise_from_endf, search_windowing, vectfit_xs, windowing, MultipoleData, SearchOptions, VectfitOptions,
    WindowingOptions, WmpGenerated,
};
use njoy_outram_park_fork::wmp::Cf64;

struct Fixture(HashMap<String, Vec<String>>);

impl Fixture {
    fn load() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("verification_and_validation/wmp_generate/mg26_openmc_8f79d9d.txt");
        let text = std::fs::read_to_string(&p).expect("fixture");
        let mut m = HashMap::new();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let mut it = line.split_whitespace();
            let key = it.next().unwrap().to_string();
            let n: usize = it.next().unwrap().parse().unwrap();
            let v: Vec<String> = it.map(str::to_string).collect();
            assert_eq!(v.len(), n, "{key}");
            m.insert(key, v);
        }
        Fixture(m)
    }
    fn f(&self, k: &str) -> Vec<f64> {
        self.0[k].iter().map(|s| s.parse().unwrap()).collect()
    }
    fn c(&self, k: &str) -> Vec<Cf64> {
        self.f(&format!("{k}.re")).into_iter().zip(self.f(&format!("{k}.im"))).map(|(r, i)| Cf64::new(r, i)).collect()
    }
    fn rows(&self, k: &str, n: usize) -> Vec<Vec<f64>> {
        self.f(k).chunks(n).map(<[f64]>::to_vec).collect()
    }
    fn crows(&self, k: &str, n: usize) -> Vec<Vec<Cf64>> {
        self.c(k).chunks(n).map(<[Cf64]>::to_vec).collect()
    }
}

fn cabs(z: Cf64) -> f64 {
    z.re.hypot(z.im)
}

/// Worst `|a - b| / |b|` after pairing each reference pole with its nearest.
fn match_poles(ours: &[Cf64], theirs: &[Cf64]) -> (Vec<usize>, f64) {
    let mut map = Vec::new();
    let mut worst = 0.0_f64;
    for t in theirs {
        let (k, d) = ours
            .iter()
            .enumerate()
            .map(|(k, p)| (k, cabs(p.sub(*t))))
            .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
        map.push(k);
        worst = worst.max(d / cabs(*t));
    }
    (map, worst)
}

fn wmp_from_fixture(fx: &Fixture, tag: &str, name: &str) -> WmpGenerated {
    let data = fx.c(&format!("{tag}.data"));
    let shape = fx.f(&format!("{tag}.data.shape"));
    let ncol = shape[1] as usize;
    let w = fx.f(&format!("{tag}.windows"));
    let cs = fx.f(&format!("{tag}.curvefit_shape"));
    let (nw, nc, nr) = (cs[0] as usize, cs[1] as usize, cs[2] as usize);
    let cf = fx.f(&format!("{tag}.curvefit"));
    WmpGenerated {
        name: name.into(),
        spacing: fx.f(&format!("{tag}.spacing"))[0],
        sqrt_awr: fx.f("awr")[0].sqrt(),
        e_min: fx.f("e_min")[0],
        e_max: fx.f("e_max")[0],
        poles: data.chunks(ncol).map(|r| r[0]).collect(),
        residues: data.chunks(ncol).map(|r| r[1..].to_vec()).collect(),
        windows: w.chunks(2).map(|p| [p[0] as i64, p[1] as i64]).collect(),
        curvefit: (0..nw).map(|i| (0..nc).map(|j| (0..nr).map(|k| cf[(i * nc + j) * nr + k]).collect()).collect()).collect(),
        broaden_poly: vec![true; nw],
    }
}

/// Windows, spacing and used poles identical; curve fit within `tol` of each
/// coefficient vector's largest entry. Returns the worst curve-fit difference.
fn compare_wmp(label: &str, ours: &WmpGenerated, theirs: &WmpGenerated) -> f64 {
    assert_eq!(ours.windows, theirs.windows, "{label}: windows");
    assert_eq!(ours.spacing.to_bits(), theirs.spacing.to_bits(), "{label}: spacing");
    assert_eq!(ours.poles, theirs.poles, "{label}: used poles");
    assert_eq!(ours.residues, theirs.residues, "{label}: used residues");
    let mut worst = 0.0_f64;
    let mut identical = 0usize;
    let mut count = 0usize;
    for (wo, wt) in ours.curvefit.iter().zip(&theirs.curvefit) {
        assert_eq!(wo.len(), wt.len(), "{label}: curve-fit order");
        let nr = wt[0].len();
        for r in 0..nr {
            let scale = wt.iter().map(|c| c[r].abs()).fold(0.0, f64::max);
            for (co, ct) in wo.iter().zip(wt) {
                count += 1;
                if co[r].to_bits() == ct[r].to_bits() {
                    identical += 1;
                }
                if scale > 0.0 {
                    worst = worst.max((co[r] - ct[r]).abs() / scale);
                }
            }
        }
    }
    println!("{label}: {} windows identical, curve fit {identical}/{count} bit-identical, worst {worst:.3e} of scale", ours.n_windows());
    worst
}

/// What [`run_stages`] measured; the two tests below gate different parts.
struct Measured {
    /// One `vectfit` call: worst pole and residue relative difference.
    vf1_pole: f64,
    vf1_residue: f64,
    /// `_vectfit_xs` pole counts, port and OpenMC.
    n_poles: (usize, usize),
    /// Test-grid points outside `rtol`/`atol`: OpenMC's poles, the port's.
    bad_openmc: usize,
    bad_ours: usize,
    /// Worst curve-fit coefficient difference (of scale): fixed, searched.
    cf_fixed: f64,
    cf_search: f64,
    /// Windowed sigma of the two fixed libraries, worst relative.
    sigma_fixed: f64,
    /// Whole-chain sigma(E, T) values outside `rtol`/`atol`.
    bad_chain: usize,
}

/// Runs every stage, prints every measurement and asserts the identities
/// (front end bits, window and used-pole identity). `None` when the tape is
/// absent.
fn run_stages() -> Option<Measured> {
    let endf = reference_endf_or_skip("n-012_Mg_026-ENDF8.0.endf", "wmp_generate_vs_openmc")?;
    let fx = Fixture::load();
    let energy = fx.f("energy");
    let ne = energy.len();
    let ce = fx.rows("ce_xs", ne);
    let (rtol, atol) = (1e-3, 1e-5);

    // ---- 1. front end: bit-identical
    let pw = pointwise_from_endf(&endf, 5e-4).expect("front end");
    assert_eq!(pw.name, fx.0["name"][0]);
    assert_eq!(pw.awr.to_bits(), fx.f("awr")[0].to_bits(), "AWR");
    assert_eq!(pw.energy.len(), ne, "grid length");
    let bits = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() == y.to_bits()).count();
    let (ge, gs, ga) = (bits(&pw.energy, &energy), bits(&pw.elastic, &ce[0]), bits(&pw.absorption, &ce[1]));
    println!("front end: grid {ge}/{ne}, elastic {gs}/{ne}, MT=27 {ga}/{ne} bit-identical; fission {:?}", pw.fission.is_some());
    assert_eq!((ge, gs, ga), (ne, ne, ne), "front end must be bit-identical");
    assert!(pw.fission.is_none());

    // ---- 2. one vectfit call on OpenMC's own first input
    let f = fx.rows("vf1.f", ne);
    let wts = fx.rows("vf1.weight", ne);
    let s: Vec<f64> = energy.iter().map(|e| e.sqrt()).collect();
    let p_in = fx.c("vf1.poles_in");
    let ref_poles = fx.c("vf1.poles_out");
    let ref_res = fx.crows("vf1.residues", ref_poles.len());
    let r = vectfit(&f, &s, &p_in, &wts, 0, false, false).expect("vectfit");
    assert_eq!(r.poles.len(), ref_poles.len());
    let (map, worst_p) = match_poles(&r.poles, &ref_poles);
    let mut worst_r = 0.0_f64;
    for v in 0..ref_res.len() {
        for (k, t) in ref_res[v].iter().enumerate() {
            let o = r.residues[v][map[k]];
            if cabs(*t) > 0.0 {
                worst_r = worst_r.max(cabs(o.sub(*t)) / cabs(*t));
            }
        }
    }
    let same = r.poles.iter().zip(&ref_poles).filter(|(a, b)| a == b).count();
    println!(
        "vectfit (first call): {} poles, {same} bit-identical in place; worst pole {worst_p:.3e}, residue {worst_r:.3e} relative",
        ref_poles.len()
    );

    // ---- 3. _vectfit_xs on OpenMC's pointwise input
    let fit = vectfit_xs(&energy, &ce, &VectfitOptions::default()).expect("vectfit_xs");
    let ref_mp = fx.c("mp.poles");
    let ref_mpr = fx.crows("mp.residues", ref_mp.len());
    println!("vectfit_xs: {} poles (OpenMC {})", fit.poles.len(), ref_mp.len());
    if fit.poles.len() == ref_mp.len() {
        let (map, wp) = match_poles(&fit.poles, &ref_mp);
        let mut wr = 0.0_f64;
        for v in 0..ref_mpr.len() {
            for (k, t) in ref_mpr[v].iter().enumerate() {
                wr = wr.max(cabs(fit.residues[v][map[k]].sub(*t)) / cabs(*t));
            }
        }
        println!("vectfit_xs: worst pole {wp:.3e}, residue {wr:.3e} relative");
    }
    // sigma on upstream's 10x test grid, upstream's own acceptance test
    let check_sigma = |label: &str, poles: &[Cf64], res: &[Vec<Cf64>]| -> (usize, usize, f64) {
        let n_test = (ne - 1) * 10 + 1;
        let xt: Vec<f64> = (0..n_test).map(|i| i as f64).collect();
        let mut te = Vec::with_capacity(n_test);
        for &x in &xt {
            let j = ((x as usize) / 10).min(ne - 2);
            let t = x - (j * 10) as f64;
            te.push(if t == 0.0 { energy[j] } else { (energy[j + 1] - energy[j]) / 10.0 * t + energy[j] });
        }
        te[n_test - 1] = energy[ne - 1];
        let ts: Vec<f64> = te.iter().map(|e| e.sqrt()).collect();
        let vfres: Vec<Vec<Cf64>> = res.iter().map(|r| r.iter().map(|z| z.mul(Cf64::new(0.0, 1.0))).collect()).collect();
        let ev = evaluate(&ts, poles, &vfres, &[]);
        let mut bad = 0usize;
        let mut worst = 0.0_f64;
        for (m, row) in ev.iter().enumerate() {
            let refx: Vec<f64> = (0..n_test)
                .map(|i| {
                    let j = energy.partition_point(|&x| x <= te[i]).saturating_sub(1).min(ne - 2);
                    if te[i] == energy[j] { ce[m][j] } else if te[i] >= energy[ne - 1] { ce[m][ne - 1] } else {
                        (ce[m][j + 1] - ce[m][j]) / (energy[j + 1] - energy[j]) * (te[i] - energy[j]) + ce[m][j]
                    }
                })
                .collect();
            for i in 0..n_test {
                let x = row[i] / te[i];
                let ab = (x - refx[i]).abs();
                if !(ab / refx[i] < rtol || ab < atol) {
                    bad += 1;
                }
                if ab > atol {
                    worst = worst.max(ab / refx[i]);
                }
            }
        }
        println!("{label}: {bad} of {} test points outside rtol/atol; worst relative {worst:.3e}", 2 * n_test);
        (bad, 2 * n_test, worst)
    };
    let (bad_openmc, _, _) = check_sigma("sigma, OpenMC's poles", &ref_mp, &ref_mpr);
    let (bad_ours, _, _) = check_sigma("sigma, port's poles", &fit.poles, &fit.residues);

    // ---- 4. windowing on OpenMC's multipole data
    let mp = MultipoleData {
        name: fx.0["name"][0].clone(),
        awr: fx.f("awr")[0],
        e_min: fx.f("e_min")[0],
        e_max: fx.f("e_max")[0],
        mts: vec![2, 27],
        poles: vec![ref_mp.clone()],
        residues: vec![ref_mpr.clone()],
    };
    let fixed = windowing(&mp, &WindowingOptions { n_win: Some(100), ..WindowingOptions::new(5) }).expect("windowing");
    let w_fixed = compare_wmp("windowing n_win=100 n_cf=5", &fixed, &wmp_from_fixture(&fx, "wmp_fixed", &mp.name));
    let searched = search_windowing(&mp, &SearchOptions::default()).expect("search");
    let ref_search = wmp_from_fixture(&fx, "wmp", &mp.name);
    let w_search = compare_wmp("from_multipole search", &searched, &ref_search);

    // ---- 5. whole chain sigma(E, T): port's library vs OpenMC's
    let ours_mp = MultipoleData { poles: vec![fit.poles.clone()], residues: vec![fit.residues.clone()], ..mp.clone() };
    let ours_lib = search_windowing(&ours_mp, &SearchOptions::default()).expect("search on port's fit").to_windowed_multipole();
    let ref_lib = ref_search.to_windowed_multipole();
    println!(
        "whole chain: port {} poles / {} windows / order {}, OpenMC {} / {} / {}",
        ours_lib.poles.len(), ours_lib.windows.len(), ours_lib.fit_order, ref_lib.poles.len(), ref_lib.windows.len(), ref_lib.fit_order
    );
    let mut worst_chain = 0.0_f64;
    let mut bad_chain = 0usize;
    for t in [0.0, 293.6, 1200.0] {
        for k in 0..400 {
            let e = (ref_lib.e_min.sqrt() + (ref_lib.e_max.sqrt() - ref_lib.e_min.sqrt()) * (k as f64 + 0.5) / 400.0).powi(2);
            let (a, b) = (ours_lib.evaluate(e, t), ref_lib.evaluate(e, t));
            for (x, y) in [(a.scatter, b.scatter), (a.absorption, b.absorption)] {
                let ab = (x - y).abs();
                if ab > atol {
                    worst_chain = worst_chain.max(ab / y.abs());
                }
                if !(ab / y.abs() < rtol || ab < atol) {
                    bad_chain += 1;
                }
            }
        }
    }
    println!("whole chain sigma(E,T), 2400 values at 0/293.6/1200 K: {bad_chain} outside rtol/atol, worst {worst_chain:.3e}");

    // windowed sigma of the port's windowing of OpenMC's multipole data
    // against OpenMC's windowing of the same data (n_win = 100, n_cf = 5)
    let lf = fixed.to_windowed_multipole();
    let rf = wmp_from_fixture(&fx, "wmp_fixed", &mp.name).to_windowed_multipole();
    let mut sigma_fixed = 0.0_f64;
    for k in 0..2000 {
        let e = (lf.e_min.sqrt() + (lf.e_max.sqrt() - lf.e_min.sqrt()) * (k as f64 + 0.5) / 2000.0).powi(2);
        let (a, b) = (lf.evaluate(e, 0.0), rf.evaluate(e, 0.0));
        for (x, y) in [(a.scatter, b.scatter), (a.absorption, b.absorption)] {
            sigma_fixed = sigma_fixed.max((x - y).abs() / y.abs().max(atol));
        }
    }
    println!("windowing n_win=100 n_cf=5: sigma(E, 0 K) of the two libraries, worst {sigma_fixed:.3e} relative");

    Some(Measured {
        vf1_pole: worst_p,
        vf1_residue: worst_r,
        n_poles: (fit.poles.len(), ref_mp.len()),
        bad_openmc,
        bad_ours,
        cf_fixed: w_fixed,
        cf_search: w_search,
        sigma_fixed,
        bad_chain,
    })
}

/// The binding gates: identities, the pole count, and the cross sections
/// (the second gate the gh:#1390 brief asks for when coefficients cannot
/// reach the code-to-code allowance).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "~7.5 min: a full vector-fitting order search and three windowings; runs by default")]
fn mg26_generation_matches_openmc_stage_by_stage() {
    let Some(m) = run_stages() else { return };
    assert_eq!(m.n_poles.0, m.n_poles.1, "vectfit_xs pole count");
    // Upstream's own acceptance test, on the 10x test grid against the
    // pointwise data. Planned as "zero points outside"; OpenMC's own poles
    // leave 730 of 42 282 outside (its order search ends without an "ideal"
    // fit), so that premise was false and the port is held to "no worse than
    // OpenMC". Recorded on gh:#1390; the original form is asserted, and
    // fails, in the ignored test below.
    assert!(m.bad_ours <= m.bad_openmc, "port misses {} points, OpenMC {}", m.bad_ours, m.bad_openmc);
    assert!(m.sigma_fixed < 1e-3, "windowed sigma {:.3e}", m.sigma_fixed);
    assert_eq!(m.bad_chain, 0, "whole chain within rtol/atol of OpenMC's library");
}

/// The coefficient gates of the prediction posted on gh:#1390 before the
/// first run, kept as written. They FAIL (measured 2026-11-14: one `vectfit`
/// call residue 8.1e-4, curve fit 1.6e-7 of scale; OpenMC's own fit misses
/// 730 test points). The cause is measured, not assumed: OpenMC moves itself
/// by the same order when its input is perturbed by one ulp (see
/// `verification_and_validation/wmp_generate/README.md`). Whether to accept
/// that is the maintainer's call, so the gates stay here unchanged rather
/// than loosened.
#[test]
#[ignore = "fails: coefficient differences exceed 1e-8 (LAPACK path, ill-conditioned fit); maintainer decision on gh:#1390"]
fn mg26_coefficients_within_code_to_code_allowance() {
    let Some(m) = run_stages() else { return };
    assert!(m.vf1_pole < 1e-8 && m.vf1_residue < 1e-8, "one vectfit call: pole {:.3e}, residue {:.3e}", m.vf1_pole, m.vf1_residue);
    assert_eq!(m.bad_openmc, 0, "OpenMC's own fit meets its ideal test");
    assert_eq!(m.bad_ours, 0, "port's fit meets upstream's ideal test");
    assert!(m.cf_fixed < 1e-8 && m.cf_search < 1e-8, "curve fit: {:.3e}, {:.3e}", m.cf_fixed, m.cf_search);
}
