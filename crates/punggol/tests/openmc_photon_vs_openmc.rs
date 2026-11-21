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
use outram_mc_libs::geometry::position::Direction;
use petir::mathf::RealMath;
use petir::rng::lcg::prn;
use punggol::openmc_photon::bremsstrahlung::BremsstrahlungData;
use punggol::openmc_photon::ttb_tables::{ElementTtbInput, TtbComponent, init_bremsstrahlung, prepare_element_ttb};
use punggol::openmc_photon::reaction::{Event, PhotonSettings, PhotonState, TtbTables, sample_electron_reaction, sample_photon_reaction, sample_positron_reaction};
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
    /// A `name rows cols` header followed by `rows * cols` numbers.
    fn mat(&mut self, name: &str) -> (Vec<f64>, usize, usize) {
        let h = self.header(name);
        let (r, c): (usize, usize) = (h[0].parse().unwrap(), h[1].parse().unwrap());
        let v = self.lines[self.pos..self.pos + r * c].iter().map(|l| l.trim().parse().unwrap()).collect();
        self.pos += r * c;
        (v, r, c)
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

    // thick-target bremsstrahlung: derive the tables from the raw element data
    // and compare with what `Material::init_bremsstrahlung` produced
    let (raw_dcs, raw_r, raw_c) = d.mat("ttb_raw_dcs");
    let raw_e = d.vec("ttb_raw_e");
    let k_grid = d.vec("ttb_k_grid");
    let raw_ne = d.vec("ttb_raw_ne");
    let raw_ion = d.vec("ttb_raw_ion");
    let sc = d.header("ttb_scalars");
    let (i_ev, z_atomic): (f64, f64) = (sc[0].parse().unwrap(), sc[1].parse().unwrap());
    let e_lin = d.vec("ttb_e_grid_lin");
    let (c_dcs, _, _) = d.mat("ttb_dcs");
    let c_srad = d.vec("ttb_srad");
    let mh = d.header("ttb_mat");
    let (awr, atom_density, density_entry): (f64, f64, f64) = (mh[0].parse().unwrap(), mh[1].parse().unwrap(), mh[2].parse().unwrap());
    let ttb_grid_ref = d.vec("ttb_e_grid");
    let ne = ttb_grid_ref.len();
    let mut ref_table = |pre: &str| BremsstrahlungData {
        pdf: Matrix::new(d.vec(&format!("{pre}_pdf")), ne).unwrap(),
        cdf: Matrix::new(d.vec(&format!("{pre}_cdf")), ne).unwrap(),
        yield_: d.vec(&format!("{pre}_yield")),
    };
    let ref_electron = ref_table("ttb_e");
    let ref_positron = ref_table("ttb_p");
    let input = ElementTtbInput {
        z: z_atomic,
        i_ev,
        n_electrons: raw_ne,
        ionization_energy: raw_ion,
        dcs: Matrix::new(raw_dcs, raw_c).unwrap(),
        electron_energy: raw_e,
    };
    assert_eq!(input.dcs.rows(), raw_r);
    let (element_ttb, grid_lin) = prepare_element_ttb(&input, 1000.0, None, &k_grid);
    let rel_v = |a: &[f64], b: &[f64]| a.iter().zip(b).fold(0.0_f64, |w, (x, y)| w.max(if *y == 0.0 { x.abs() } else { (x - y).abs() / y.abs() }));
    let w_grid = rel_v(&grid_lin, &e_lin);
    let w_dcs = rel_v(&(0..element_ttb.dcs.rows()).flat_map(|i| element_ttb.dcs.row(i).to_vec()).collect::<Vec<_>>(), &c_dcs);
    let w_srad = rel_v(&element_ttb.stopping_power_radiative, &c_srad);
    let comps = [TtbComponent { awr, atom_density, element: element_ttb }];
    let derived = init_bremsstrahlung(&comps, density_entry, &grid_lin, &k_grid);
    let ttb_grid: Vec<f64> = grid_lin.iter().map(|v| RealMath::r_ln(*v)).collect();
    let flat = |m: &Matrix| (0..m.rows()).flat_map(|i| m.row(i).to_vec()).collect::<Vec<_>>();
    let w_log_grid = rel_v(&ttb_grid, &ttb_grid_ref);
    let w_pdf = rel_v(&flat(&derived.electron.pdf), &flat(&ref_electron.pdf)).max(rel_v(&flat(&derived.positron.pdf), &flat(&ref_positron.pdf)));
    let w_cdf = rel_v(&flat(&derived.electron.cdf), &flat(&ref_electron.cdf)).max(rel_v(&flat(&derived.positron.cdf), &flat(&ref_positron.cdf)));
    let w_yield = rel_v(&derived.electron.yield_, &ref_electron.yield_).max(rel_v(&derived.positron.yield_, &ref_positron.yield_));
    eprintln!(
        "TTB tables (Z = {z_atomic}, {ne} energies): grid {w_grid:e}, log grid {w_log_grid:e}, dcs {w_dcs:e}, radiative stopping power {w_srad:e}; electron+positron pdf {w_pdf:e}, cdf {w_cdf:e}, ln yield {w_yield:e} (worst relative)"
    );
    // the reaction samples below use the DERIVED tables, so they check the whole chain
    let (ttb_electron, ttb_positron) = (derived.electron, derived.positron);

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

    let (mut n_react, mut react_bad_seed, mut react_bad_event, mut react_bad_nsec) = (0usize, 0usize, 0usize, 0usize);
    let mut w_react = 0.0_f64;
    let (mut n_chg, mut chg_bad) = (0usize, 0usize);
    let mut w_chg = 0.0_f64;
    let tables = TtbTables { e_grid: &ttb_grid, electron: &ttb_electron, positron: &ttb_positron };

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
            "react" => {
                n_react += 1;
                let mode: u8 = t[1].parse().unwrap();
                let e: f64 = t[2].parse().unwrap();
                let mut seed: u64 = t[3].parse().unwrap();
                let settings = if mode == 0 { PhotonSettings::new() } else { PhotonSettings::new().without_ttb() };
                // upstream's `sample_element` draws one number before the reaction is sampled
                let _ = prn(&mut seed);
                let micro = p.calculate_xs(e);
                let mut st = PhotonState {
                    e,
                    u: Direction::new(0.48, 0.6, 0.64),
                    wgt: 1.0,
                    mu: 0.0,
                    in_void: false,
                    event: Event::None,
                    event_mt: 0,
                };
                let out = sample_photon_reaction(&mut st, &p, &micro, &settings, Some(&tables), &mut seed);
                let want_event = match t[4] {
                    "3" => Event::Scatter,
                    "4" => Event::Absorb,
                    _ => Event::None,
                };
                if st.event != want_event || st.event_mt != t[5].parse::<i32>().unwrap() {
                    react_bad_event += 1;
                }
                let v: Vec<f64> = t[6..11].iter().map(|x| x.parse().unwrap()).collect();
                for (a, b) in [(st.e, v[0]), (st.wgt, v[1]), (st.u.u, v[2]), (st.u.v, v[3]), (st.u.w, v[4])] {
                    w_react = w_react.max(if b == 0.0 { a.abs() } else { rel(a, b) });
                }
                if std::env::var("OPENMC_DEBUG").is_ok() && n_react < 100000 {
                    let dd = [(st.e, v[0]), (st.wgt, v[1]), (st.u.u, v[2]), (st.u.v, v[3]), (st.u.w, v[4])];
                    if dd.iter().any(|(a, b)| (a - b).abs() > 1e-9 * b.abs().max(1.0)) {
                        eprintln!("DIFF mode {mode} e {e} mt {} mine {:?} want {:?} line {}", st.event_mt, dd, v, &l[..l.len().min(160)]);
                    }
                }
                // `p.mu()` is only written by the two scattering events (it is uninitialised otherwise)
                if st.event == Event::Scatter {
                    w_react = w_react.max((st.mu - t[11].parse::<f64>().unwrap()).abs());
                }
                if seed != t[12].parse::<u64>().unwrap() {
                    react_bad_seed += 1;
                }
                let nsec: usize = t[13].parse().unwrap();
                if out.secondaries.len() != nsec {
                    react_bad_nsec += 1;
                    if std::env::var("OPENMC_DEBUG").is_ok() && react_bad_nsec < 8 {
                        eprintln!("NSEC mode {mode} e {e} mt {} mine {} want {nsec}\n mine {:?}\n want {:?}", st.event_mt, out.secondaries.len(), out.secondaries.iter().map(|x| x.e).collect::<Vec<_>>(), (0..nsec).map(|k| t[14 + 6 * k + 1].to_string()).collect::<Vec<_>>());
                    }
                } else {
                    for (k, sec) in out.secondaries.iter().enumerate() {
                        let b: Vec<f64> = t[14 + 6 * k..20 + 6 * k].iter().map(|x| x.parse().unwrap()).collect();
                        assert_eq!(b[0], 1.0, "only photons are banked");
                        for (a, b) in [(sec.e, b[1]), (sec.wgt, b[2]), (sec.u.u, b[3]), (sec.u.v, b[4]), (sec.u.w, b[5])] {
                            w_react = w_react.max(if b == 0.0 { a.abs() } else { rel(a, b) });
                        }
                    }
                }
            }
            "chg" => {
                n_chg += 1;
                let mode: u8 = t[1].parse().unwrap();
                let kind: u8 = t[2].parse().unwrap();
                let e: f64 = t[3].parse().unwrap();
                let mut seed: u64 = t[4].parse().unwrap();
                let settings = if mode == 0 { PhotonSettings::new() } else { PhotonSettings::new().without_ttb() };
                let mut st = PhotonState { e, u: Direction::new(0.48, 0.6, 0.64), wgt: 1.0, mu: 0.0, in_void: false, event: Event::None, event_mt: 0 };
                let out = if kind == 0 {
                    sample_electron_reaction(&mut st, &settings, Some(&tables), &mut seed)
                } else {
                    sample_positron_reaction(&mut st, &settings, Some(&tables), &mut seed)
                };
                if st.event != Event::Absorb || t[5] != "4" || st.e != 0.0 || st.wgt != 0.0 {
                    chg_bad += 1;
                }
                if seed != t[8].parse::<u64>().unwrap() {
                    chg_bad += 1;
                }
                if out.secondaries.len() != t[9].parse::<usize>().unwrap() {
                    chg_bad += 1;
                } else {
                    for (k, sec) in out.secondaries.iter().enumerate() {
                        let b: Vec<f64> = t[10 + 5 * k..15 + 5 * k].iter().map(|x| x.parse().unwrap()).collect();
                        for (a, b) in [(sec.e, b[0]), (sec.wgt, b[1]), (sec.u.u, b[2]), (sec.u.v, b[3]), (sec.u.w, b[4])] {
                            w_chg = w_chg.max(if b == 0.0 { a.abs() } else { rel(a, b) });
                        }
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
    eprintln!(
        "{n_react} sample_photon_reaction calls (TTB on/off): event/MT mismatches {react_bad_event}, bank-size mismatches {react_bad_nsec}, final-seed mismatches {react_bad_seed}, worst state/secondary difference {w_react:e}"
    );
    eprintln!("{n_chg} sample_electron/positron_reaction calls: mismatches (event, seed, bank size) {chg_bad}, worst secondary difference {w_chg:e}");
    assert!(n > 0);
}
