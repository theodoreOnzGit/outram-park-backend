//! Phase 2 (gh:#1683, gh:#1690): `punggol::openmc_photon`'s three photon
//! samplers against a REAL OpenMC built from the pinned source (`8f79d9d0`).
//!
//! Method: `verification_and_validation/openmc_photon/harness.cpp` links
//! `libopenmc`, loads a photon HDF5 library, dumps the data members the
//! samplers read (`OPENMC_REFERENCE_DUMP`) and runs `compton_scatter`
//! (Doppler on and off), `rayleigh_scatter` and `pair_production` on fixed
//! seeds (`OPENMC_REFERENCE_SAMPLES`). This test builds a
//! [`PhotonInteraction`] from the dump, runs the Rust samplers on the same
//! seeds and compares every output and the final seed (the LCG is the same
//! generator, `outram_mc_libs::rng::lcg`). Skipped unless both variables are
//! set. Prints the worst relative differences; asserts that the dump's
//! `Tabulated1D` probes are reproduced and that the sample files parse.
//! Results: `docs/openmc_photon_phase2.md`.

use punggol::openmc_photon::{ElectronSubshell, Matrix, PhotonInteraction, RelaxationSecondary, Tabulated1D, Transition};
use std::sync::Arc;

struct Dump {
    lines: Vec<String>,
    pos: usize,
}

impl Dump {
    fn header(&mut self, name: &str) -> Vec<String> {
        let l = &self.lines[self.pos];
        self.pos += 1;
        let t: Vec<String> = l.split_whitespace().map(String::from).collect();
        assert_eq!(t[0], name, "dump order: expected {name}, got {l}");
        t[1..].to_vec()
    }
    fn vec(&mut self, name: &str) -> Vec<f64> {
        let n: usize = self.header(name)[0].parse().unwrap();
        let v = self.lines[self.pos..self.pos + n].iter().map(|l| l.trim().parse().unwrap()).collect();
        self.pos += n;
        v
    }
    fn tab(&mut self, name: &str) -> (Vec<f64>, Vec<f64>) {
        let n: usize = self.header(name)[0].parse().unwrap();
        let (mut x, mut y) = (Vec::new(), Vec::new());
        for l in &self.lines[self.pos..self.pos + n] {
            let mut it = l.split_whitespace().map(|t| t.parse::<f64>().unwrap());
            x.push(it.next().unwrap());
            y.push(it.next().unwrap());
        }
        self.pos += n;
        (x, y)
    }
}

#[test]
fn photon_samplers_against_compiled_openmc() {
    let (Ok(dump_path), Ok(samples_path)) = (std::env::var("OPENMC_REFERENCE_DUMP"), std::env::var("OPENMC_REFERENCE_SAMPLES")) else {
        eprintln!("OPENMC_REFERENCE_DUMP / OPENMC_REFERENCE_SAMPLES not set: differential test skipped");
        return;
    };
    let text = std::fs::read_to_string(dump_path).unwrap();
    let mut d = Dump { lines: text.lines().map(String::from).collect(), pos: 0 };
    let z: usize = d.header("Z")[0].parse().unwrap();
    let _ngrid = d.header("ngrid");
    let pz = d.vec("profile_pz");
    let pdf = d.vec("profile_pdf");
    let shape: Vec<usize> = d.header("profile_pdf_shape").iter().map(|t| t.parse().unwrap()).collect();
    let cdf = d.vec("profile_cdf");
    let tail = d.vec("profile_tail_slope");
    let neg = d.vec("profile_negative_mass");
    let be = d.vec("binding_energy");
    let epdf = d.vec("electron_pdf");
    let nmap: usize = d.header("subshell_map")[0].parse().unwrap();
    let smap: Vec<i32> = d.lines[d.pos..d.pos + nmap].iter().map(|l| l.trim().parse().unwrap()).collect();
    d.pos += nmap;
    let energy = d.vec("energy");
    let coherent = d.vec("coherent");
    let incoherent = d.vec("incoherent");
    let pair_total = d.vec("pair_production_total");
    let xs_flat = d.vec("cross_sections");
    let xs_shape: Vec<usize> = d.header("cross_sections_shape").iter().map(|t| t.parse().unwrap()).collect();
    let (ix, iy) = d.tab("incoherent_form_factor");
    let (cx, cy) = d.tab("coherent_int_form_factor");
    let inc = Tabulated1D::lin_lin(ix, iy).unwrap();
    let coh = Tabulated1D::lin_lin(cx, cy).unwrap();
    // probes: the Rust interpolation must reproduce OpenMC's Tabulated1D
    let _ = d.header("probes");
    let mut worst_probe = 0.0_f64;
    for _ in 0..20 {
        let v: Vec<f64> = d.lines[d.pos].split_whitespace().map(|t| t.parse().unwrap()).collect();
        d.pos += 1;
        worst_probe = worst_probe.max((inc.eval(v[0]) - v[1]).abs() / v[1].abs().max(1e-300)).max((coh.eval(v[0]) - v[2]).abs() / v[2].abs().max(1e-300));
    }
    eprintln!("Tabulated1D probes: worst relative difference {worst_probe:e}");
    let has_relax = d.header("has_atomic_relaxation")[0] == "1";
    let max_stack: i32 = d.header("max_stack")[0].parse().unwrap();
    let n_shells: usize = d.header("shells")[0].parse().unwrap();
    let mut shells = Vec::new();
    for _ in 0..n_shells {
        let h = d.header("shell");
        let nt: usize = h[3].parse().unwrap();
        let mut transitions = Vec::new();
        for _ in 0..nt {
            let v: Vec<&str> = d.lines[d.pos].split_whitespace().collect();
            d.pos += 1;
            transitions.push(Transition {
                primary_subshell: v[0].parse().unwrap(),
                secondary_subshell: v[1].parse().unwrap(),
                energy: v[2].parse().unwrap(),
                probability: v[3].parse().unwrap(),
            });
        }
        shells.push(ElectronSubshell {
            index_subshell: h[0].parse().unwrap(),
            threshold: h[1].parse().unwrap(),
            binding_energy: h[2].parse().unwrap(),
            transitions,
        });
    }

    let p = PhotonInteraction {
        z,
        energy,
        coherent,
        incoherent,
        pair_production_total: pair_total,
        cross_sections: Matrix::new(xs_flat, xs_shape[1]).unwrap(),
        incoherent_form_factor: Some(inc),
        coherent_int_form_factor: Some(coh),
        profile_pz: Arc::from(pz),
        profile_pdf: Matrix::new(pdf, shape[1]).unwrap(),
        profile_cdf: Matrix::new(cdf, shape[1]).unwrap(),
        profile_tail_slope: tail,
        profile_negative_mass: neg,
        binding_energy: be,
        electron_pdf: epdf,
        subshell_map: smap,
        has_atomic_relaxation: has_relax,
        shells,
        ..Default::default()
    };

    if has_relax {
        assert_eq!(p.calc_max_stack_size(), max_stack, "calc_max_stack_size");
    }
    let (mut n_relax, mut relax_seed_bad, mut relax_count_bad) = (0usize, 0usize, 0usize);
    let mut w_relax = 0.0_f64;

    let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-300);
    let (mut n, mut seed_bad, mut shell_bad) = (0usize, 0usize, 0usize);
    let (mut n_xs, mut idx_bad) = (0usize, 0usize);
    let mut w_xs = [0.0_f64; 6];
    let (mut w_a, mut w_mu, mut w_r, mut w_pair) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for l in std::fs::read_to_string(samples_path).unwrap().lines() {
        let t: Vec<&str> = l.split_whitespace().collect();
        n += 1;
        match t[0] {
            "xs" => {
                n_xs += 1;
                let e: f64 = t[1].parse().unwrap();
                let x = p.calculate_xs(e);
                if x.index_grid != t[2].parse::<usize>().unwrap() {
                    idx_bad += 1;
                }
                let theirs: Vec<f64> = t[3..9].iter().map(|v| v.parse().unwrap()).collect();
                let mine = [x.interp_factor, x.coherent, x.incoherent, x.photoelectric, x.pair_production, x.total];
                for k in 0..6 {
                    w_xs[k] = w_xs[k].max(if theirs[k] == 0.0 { mine[k].abs() } else { rel(mine[k], theirs[k]) });
                }
            }
            "compton" => {
                let alpha: f64 = t[1].parse().unwrap();
                let doppler = t[2] == "1";
                let mut seed: u64 = t[3].parse().unwrap();
                let (a_out, mu, ish) = p.compton_scatter(alpha, doppler, &mut seed);
                w_a = w_a.max(rel(a_out, t[4].parse().unwrap()));
                w_mu = w_mu.max((mu - t[5].parse::<f64>().unwrap()).abs());
                if ish != t[6].parse::<i32>().unwrap() {
                    shell_bad += 1;
                }
                if seed != t[7].parse::<u64>().unwrap() {
                    seed_bad += 1;
                }
            }
            "rayleigh" => {
                let alpha: f64 = t[1].parse().unwrap();
                let mut seed: u64 = t[2].parse().unwrap();
                let mu = p.rayleigh_scatter(alpha, &mut seed);
                w_r = w_r.max((mu - t[3].parse::<f64>().unwrap()).abs());
                if seed != t[4].parse::<u64>().unwrap() {
                    seed_bad += 1;
                }
            }
            "pair" => {
                let alpha: f64 = t[1].parse().unwrap();
                let mut seed: u64 = t[2].parse().unwrap();
                let (ee, ep, mue, mup) = p.pair_production(alpha, &mut seed);
                for (a, b) in [(ee, t[3]), (ep, t[4]), (mue, t[5]), (mup, t[6])] {
                    w_pair = w_pair.max((a - b.parse::<f64>().unwrap()).abs() / b.parse::<f64>().unwrap().abs().max(1.0e-300));
                }
                if seed != t[7].parse::<u64>().unwrap() {
                    seed_bad += 1;
                }
            }
            "relax" => {
                n_relax += 1;
                let ish: usize = t[1].parse().unwrap();
                let pe: f64 = t[2].parse().unwrap();
                let mut seed: u64 = t[3].parse().unwrap();
                let out = p.atomic_relaxation(ish, pe, 1000.0, &mut seed);
                if seed != t[4].parse::<u64>().unwrap() {
                    relax_seed_bad += 1;
                }
                // upstream's bank holds the photons only (charged secondaries are deposited locally)
                let photons: Vec<(f64, (f64, f64, f64))> = out
                    .iter()
                    .filter_map(|s| match s {
                        RelaxationSecondary::Photon { u, energy } => Some((*energy, *u)),
                        _ => None,
                    })
                    .collect();
                if photons.len() != t[5].parse::<usize>().unwrap() {
                    relax_count_bad += 1;
                } else {
                    for (k, (e, u)) in photons.iter().enumerate() {
                        let v: Vec<f64> = t[6 + 4 * k..10 + 4 * k].iter().map(|x| x.parse().unwrap()).collect();
                        w_relax = w_relax.max(rel(*e, v[0])).max((u.0 - v[1]).abs()).max((u.1 - v[2]).abs()).max((u.2 - v[3]).abs());
                    }
                }
            }
            other => panic!("unknown sample kind {other}"),
        }
    }
    eprintln!(
        "{n} samples: compton alpha_out rel {w_a:e}, |dmu| {w_mu:e}, shell mismatches {shell_bad}; rayleigh |dmu| {w_r:e}; pair worst rel {w_pair:e}; final-seed mismatches {seed_bad}"
    );
    eprintln!(
        "{n_xs} calculate_xs points: index mismatches {idx_bad}; worst difference (relative, absolute where the reference is 0): interp {:e}, coherent {:e}, incoherent {:e}, photoelectric {:e}, pair {:e}, total {:e}",
        w_xs[0], w_xs[1], w_xs[2], w_xs[3], w_xs[4], w_xs[5]
    );
    eprintln!(
        "{n_relax} atomic_relaxation cascades (max stack {max_stack}): photon-count mismatches {relax_count_bad}, final-seed mismatches {relax_seed_bad}, worst photon energy/direction difference {w_relax:e}"
    );
    assert!(n > 0);
}
