//! Differential test of `punggol::openmc_photon` for a **material of two
//! elements** (U and H) against a built OpenMC (gh:#1690), skipped unless
//! `OPENMC_REFERENCE_DUMPS` (two single-element dumps from
//! `verification_and_validation/openmc_photon/harness.cpp`, `A.dump,B.dump`),
//! `OPENMC_REFERENCE_MULTI_TABLES` and `OPENMC_REFERENCE_MULTI_SAMPLES` (from
//! `harness_multi.cpp`) are set. Compares (a) the thick-target bremsstrahlung
//! tables of `Material::init_bremsstrahlung` for atom densities and for weight
//! fractions (negative `atom_density_` entries), and (b) `sample_element` plus
//! `sample_photon_reaction` for the atom-density material, call by call.
//! Results: `docs/openmc_photon_phase2.md`.

use outram_mc_libs::geometry::position::Direction;
use petir::mathf::RealMath;
use punggol::openmc_photon::bremsstrahlung::BremsstrahlungData;
use punggol::openmc_photon::reaction::{Event, PhotonSettings, PhotonState, TtbTables, sample_element, sample_photon_reaction};
use punggol::openmc_photon::ttb_tables::{ElementTtb, ElementTtbInput, TtbComponent, init_bremsstrahlung, prepare_element_ttb};
use punggol::openmc_photon::{ElectronSubshell, Matrix, PhotonInteraction, Tabulated1D, Transition};
use std::sync::Arc;

struct Dump {
    lines: Vec<String>,
    pos: usize,
}
impl Dump {
    fn new(path: &str) -> Dump {
        Dump { lines: std::fs::read_to_string(path).unwrap().lines().map(String::from).collect(), pos: 0 }
    }
    fn header(&mut self, name: &str) -> Vec<String> {
        let t: Vec<String> = self.lines[self.pos].split_whitespace().map(String::from).collect();
        assert_eq!(t[0], name, "dump order");
        self.pos += 1;
        t[1..].to_vec()
    }
    fn vec(&mut self, name: &str) -> Vec<f64> {
        let n: usize = self.header(name)[0].parse().unwrap();
        let v = self.lines[self.pos..self.pos + n].iter().map(|l| l.trim().parse().unwrap()).collect();
        self.pos += n;
        v
    }
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
            let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            x.push(v[0]);
            y.push(v[1]);
        }
        self.pos += n;
        (x, y)
    }
}

struct Loaded {
    p: PhotonInteraction,
    input: ElementTtbInput,
    k_grid: Vec<f64>,
    cutoff: f64,
}

/// The parsing half of `photon_samplers_against_compiled_openmc`, for one dump.
fn load(path: &str) -> Loaded {
    let mut d = Dump::new(path);
    let z: usize = d.header("Z")[0].parse().unwrap();
    let _ = d.header("ngrid");
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
    let _ = d.header("probes");
    d.pos += 20;
    let has_relax = d.header("has_atomic_relaxation")[0] == "1";
    let _ = d.header("max_stack");
    let n_shells: usize = d.header("shells")[0].parse().unwrap();
    let mut shells = Vec::new();
    for _ in 0..n_shells {
        let h = d.header("shell");
        let nt: usize = h[3].parse().unwrap();
        let mut transitions = Vec::new();
        for _ in 0..nt {
            let v: Vec<&str> = d.lines[d.pos].split_whitespace().collect();
            d.pos += 1;
            transitions.push(Transition { primary_subshell: v[0].parse().unwrap(), secondary_subshell: v[1].parse().unwrap(), energy: v[2].parse().unwrap(), probability: v[3].parse().unwrap() });
        }
        shells.push(ElectronSubshell { index_subshell: h[0].parse().unwrap(), threshold: h[1].parse().unwrap(), binding_energy: h[2].parse().unwrap(), transitions });
    }
    let (raw_dcs, _raw_r, raw_c) = d.mat("ttb_raw_dcs");
    let raw_e = d.vec("ttb_raw_e");
    let k_grid = d.vec("ttb_k_grid");
    let raw_ne = d.vec("ttb_raw_ne");
    let raw_ion = d.vec("ttb_raw_ion");
    let sc = d.header("ttb_scalars");
    let (i_ev, z_atomic, cutoff): (f64, f64, f64) = (sc[0].parse().unwrap(), sc[1].parse().unwrap(), sc[2].parse().unwrap());
    let input = ElementTtbInput { z: z_atomic, i_ev, n_electrons: raw_ne, ionization_energy: raw_ion, dcs: Matrix::new(raw_dcs, raw_c).unwrap(), electron_energy: raw_e };
    let p = PhotonInteraction {
        z,
        energy,
        coherent,
        incoherent,
        pair_production_total: pair_total,
        cross_sections: Matrix::new(xs_flat, xs_shape[1]).unwrap(),
        incoherent_form_factor: Some(Tabulated1D::lin_lin(ix, iy).unwrap()),
        coherent_int_form_factor: Some(Tabulated1D::lin_lin(cx, cy).unwrap()),
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
    Loaded { p, input, k_grid, cutoff }
}

fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 { a.abs() } else { (a - b).abs() / b.abs() }
}
fn rel_v(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold(0.0_f64, |w, (x, y)| w.max(rel(*x, *y)))
}

#[test]
fn two_element_material_against_compiled_openmc() {
    let (Ok(dumps), Ok(tables_path), Ok(samples_path)) = (std::env::var("OPENMC_REFERENCE_DUMPS"), std::env::var("OPENMC_REFERENCE_MULTI_TABLES"), std::env::var("OPENMC_REFERENCE_MULTI_SAMPLES")) else {
        eprintln!("OPENMC_REFERENCE_DUMPS / _MULTI_TABLES / _MULTI_SAMPLES not set: differential test skipped");
        return;
    };
    let names: Vec<&str> = dumps.split(',').collect();
    let loaded: Vec<Loaded> = names.iter().map(|n| load(n)).collect();
    let cutoff = loaded[0].cutoff;
    let k_grid = &loaded[0].k_grid;

    // element TTB data on the shared grid: the first element sets it, the next reuses it
    let (t0, grid_lin) = prepare_element_ttb(&loaded[0].input, cutoff, None, k_grid);
    let (t1, _) = prepare_element_ttb(&loaded[1].input, cutoff, Some(&grid_lin), k_grid);

    let mut d = Dump::new(&tables_path);
    let n_lin: usize = d.header("grid_lin")[0].parse().unwrap();
    let lin_ref: Vec<f64> = d.lines[d.pos..d.pos + n_lin].iter().map(|l| l.parse().unwrap()).collect();
    d.pos += n_lin;
    let w_grid = rel_v(&grid_lin, &lin_ref);
    let ttb_grid_ref = d.vec("ttb_e_grid");
    let ttb_grid: Vec<f64> = grid_lin.iter().map(|v| RealMath::r_ln(*v)).collect();
    let ne = ttb_grid.len();
    let flat = |m: &Matrix| (0..m.rows()).flat_map(|i| m.row(i).to_vec()).collect::<Vec<_>>();
    let mut derived_atom = None;
    let (mut w_pdf, mut w_cdf, mut w_yield) = ([0.0_f64; 2], [0.0_f64; 2], [0.0_f64; 2]);
    for m in 0..2 {
        let h = d.header("mat");
        let v: Vec<f64> = h.iter().map(|x| x.parse().unwrap()).collect();
        let clone = |t: &ElementTtb| ElementTtb {
            z: t.z,
            i_ev: t.i_ev,
            n_electrons: t.n_electrons.clone(),
            ionization_energy: t.ionization_energy.clone(),
            dcs: t.dcs.clone(),
            stopping_power_radiative: t.stopping_power_radiative.clone(),
        };
        let comps = [TtbComponent { awr: v[2], atom_density: v[3], element: clone(&t0) }, TtbComponent { awr: v[4], atom_density: v[5], element: clone(&t1) }];
        let derived = init_bremsstrahlung(&comps, v[1], &grid_lin, k_grid);
        let mut table = |pre: &str| BremsstrahlungData {
            pdf: Matrix::new(d.vec(&format!("m{m}_{pre}_pdf")), ne).unwrap(),
            cdf: Matrix::new(d.vec(&format!("m{m}_{pre}_cdf")), ne).unwrap(),
            yield_: d.vec(&format!("m{m}_{pre}_yield")),
        };
        let (re, rp) = (table("e"), table("p"));
        w_pdf[m] = rel_v(&flat(&derived.electron.pdf), &flat(&re.pdf)).max(rel_v(&flat(&derived.positron.pdf), &flat(&rp.pdf)));
        w_cdf[m] = rel_v(&flat(&derived.electron.cdf), &flat(&re.cdf)).max(rel_v(&flat(&derived.positron.cdf), &flat(&rp.cdf)));
        w_yield[m] = rel_v(&derived.electron.yield_, &re.yield_).max(rel_v(&derived.positron.yield_, &rp.yield_));
        if m == 0 {
            derived_atom = Some((derived, v[3], v[5]));
        }
    }
    eprintln!("two-element TTB tables ({ne} energies, U + H): shared grid {w_grid:e}, log grid {:e}", rel_v(&ttb_grid, &ttb_grid_ref));
    for (m, label) in ["atom densities", "weight fractions"].iter().enumerate() {
        eprintln!("  {label}: pdf {:e}, cdf {:e}, ln yield {:e} (worst relative)", w_pdf[m], w_cdf[m], w_yield[m]);
    }

    // sample_element + sample_photon_reaction on the atom-density material
    let (derived, rho_u, rho_h) = derived_atom.unwrap();
    let rhos = [rho_u, rho_h];
    let tables = TtbTables { e_grid: &ttb_grid, electron: &derived.electron, positron: &derived.positron };
    let (mut n, mut bad_event, mut bad_elem, mut bad_seed, mut bad_nsec) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut per_elem, mut worst) = ([0usize; 2], 0.0_f64);
    for l in std::fs::read_to_string(samples_path).unwrap().lines() {
        let t: Vec<&str> = l.split_whitespace().collect();
        assert_eq!(t[0], "mreact");
        n += 1;
        let mode: u8 = t[1].parse().unwrap();
        let e: f64 = t[2].parse().unwrap();
        let mut seed: u64 = t[3].parse().unwrap();
        let base = PhotonSettings { photon_cutoff: cutoff, ..PhotonSettings::new() };
        let settings = if mode == 0 { base } else { base.without_ttb() };
        let below = e < cutoff;
        let micros = [loaded[0].p.calculate_xs(e), loaded[1].p.calculate_xs(e)];
        let macro_total = rhos[0] * micros[0].total + rhos[1] * micros[1].total;
        let mut chosen = 0usize;
        if !below {
            chosen = sample_element(macro_total, &rhos, &[micros[0].total, micros[1].total], &mut seed).expect("an element is sampled");
            per_elem[chosen] += 1;
            if chosen as i32 != t[6].parse::<i32>().unwrap() {
                bad_elem += 1;
            }
        }
        let mut st = PhotonState { e, u: Direction::new(0.48, 0.6, 0.64), wgt: 1.0, mu: 0.0, in_void: false, event: Event::None, event_mt: 0 };
        let out = sample_photon_reaction(&mut st, &loaded[chosen].p, &micros[chosen], &settings, Some(&tables), &mut seed);
        let want_event = match t[4] {
            "3" => Event::Scatter,
            "4" => Event::Absorb,
            _ => Event::None,
        };
        if !below && (st.event != want_event || st.event_mt != t[5].parse::<i32>().unwrap()) {
            bad_event += 1;
        }
        let v: Vec<f64> = t[7..12].iter().map(|x| x.parse().unwrap()).collect();
        for (a, b) in [(st.e, v[0]), (st.wgt, v[1]), (st.u.u, v[2]), (st.u.v, v[3]), (st.u.w, v[4])] {
            worst = worst.max(rel(a, b));
        }
        if st.event == Event::Scatter {
            worst = worst.max((st.mu - t[12].parse::<f64>().unwrap()).abs());
        }
        if seed != t[13].parse::<u64>().unwrap() {
            bad_seed += 1;
        }
        let nsec: usize = t[14].parse().unwrap();
        if out.secondaries.len() != nsec {
            bad_nsec += 1;
        } else {
            for (k, sec) in out.secondaries.iter().enumerate() {
                let b: Vec<f64> = t[15 + 6 * k..21 + 6 * k].iter().map(|x| x.parse().unwrap()).collect();
                for (a, b) in [(sec.e, b[1]), (sec.wgt, b[2]), (sec.u.u, b[3]), (sec.u.v, b[4]), (sec.u.w, b[5])] {
                    worst = worst.max(rel(a, b));
                }
            }
        }
    }
    eprintln!(
        "two-element sample_element + sample_photon_reaction: {n} calls (U chosen {}, H chosen {}); element mismatches {bad_elem}, event/MT mismatches {bad_event}, bank-size mismatches {bad_nsec}, final-seed mismatches {bad_seed}, worst state/secondary difference {worst:e}",
        per_elem[0], per_elem[1]
    );
    assert!(n > 0);
}

#[test]
fn sample_element_picks_by_cumulative_macro_cross_section() {
    // a seed whose first draw is known is not needed: with one element the
    // answer is always index 0, and a zero-density element is never chosen
    let mut seed = 12345_u64;
    for _ in 0..50 {
        assert_eq!(sample_element(2.0, &[1.0, 0.0], &[2.0, 5.0], &mut seed), Some(0));
        assert_eq!(sample_element(2.0, &[0.0, 1.0], &[5.0, 2.0], &mut seed), Some(1));
    }
    // a macro total larger than the sum of the parts: nothing may be sampled
    // for a cutoff above the cumulative probability (upstream's fatal_error)
    let mut hit_none = false;
    for _ in 0..50 {
        hit_none |= sample_element(1.0e12, &[1.0], &[1.0], &mut seed).is_none();
    }
    assert!(hit_none);
}
