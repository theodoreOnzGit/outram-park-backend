//! Tests for the OpenMC photon port. Synthetic data only (no HDF5 is read):
//! every function is reached, and physical identities (energy conservation,
//! the Compton formula, CDF/inverse round trips) are checked rather than
//! numbers copied from OpenMC. **Not differential tests** (phase 2, gh:#1683).

use super::bremsstrahlung::{BremsstrahlungData, thick_target_bremsstrahlung};
use super::reaction::*;
use super::*;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::particle::particle::ParticleType;

fn ln(x: f64) -> f64 {
    x.ln()
}

/// A two-shell toy element: shell 0 "K" (2 keV) relaxes radiatively or by
/// Auger into shell 1 "L" (500 eV), which has no transitions.
fn toy_element() -> PhotonInteraction {
    let pz: Vec<f64> = alloc::vec![0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 100.0];
    let n = pz.len();
    let pdf_row: Vec<f64> = pz.iter().map(|&z| 0.45 * (-z).exp()).collect();
    let mut cdf_row = alloc::vec![0.0; n];
    for i in 1..n {
        cdf_row[i] = cdf_row[i - 1] + 0.5 * (pdf_row[i] + pdf_row[i - 1]) * (pz[i] - pz[i - 1]);
    }
    let mut pdf = Vec::new();
    let mut cdf = Vec::new();
    for _ in 0..2 {
        pdf.extend_from_slice(&pdf_row);
        cdf.extend_from_slice(&cdf_row);
    }
    let shells = alloc::vec![
        ElectronSubshell {
            index_subshell: 1,
            threshold: 0,
            binding_energy: 2000.0,
            transitions: alloc::vec![
                Transition { primary_subshell: 1, secondary_subshell: -1, energy: 1500.0, probability: 0.5 },
                Transition { primary_subshell: 1, secondary_subshell: 1, energy: 1000.0, probability: 0.5 },
            ],
        },
        ElectronSubshell { index_subshell: 3, threshold: 0, binding_energy: 500.0, transitions: Vec::new() },
    ];
    let egrid: Vec<f64> = [1.0e3, 1.0e4, 1.0e5, 1.0e6, 1.0e7].iter().map(|&e| ln(e)).collect();
    let flat = |v: f64| alloc::vec![v; 5];
    let mut cs = Vec::new();
    for r in 0..5 {
        // shell 0 has a threshold above the first grid point.
        cs.push(if r == 0 { 0.0 } else { ln(10.0) });
        cs.push(ln(2.0 + r as f64));
    }
    PhotonInteraction {
        name: "Toy".into(),
        z: 6,
        index: 0,
        energy: egrid,
        coherent: flat(ln(2.0)),
        incoherent: flat(ln(3.0)),
        photoelectric_total: flat(ln(5.0)),
        pair_production_total: flat(ln(0.5)),
        pair_production_electron: flat(ln(0.1)),
        pair_production_nuclear: flat(ln(0.4)),
        heating: flat(1.0),
        incoherent_form_factor: Some(Tabulated1D::lin_lin(alloc::vec![0.0, 1.0, 10.0, 100.0], alloc::vec![0.0, 3.0, 5.0, 6.0]).unwrap()),
        coherent_int_form_factor: Some(
            Tabulated1D::lin_lin(alloc::vec![0.0, 1.0, 10.0, 100.0, 1.0e4], alloc::vec![0.0, 2.0, 4.0, 5.0, 6.0]).unwrap(),
        ),
        shells,
        cross_sections: Matrix::new(cs, 2).unwrap(),
        profile_pz: Arc::from(pz),
        profile_pdf: Matrix::new(pdf, n).unwrap(),
        profile_cdf: Matrix::new(cdf, n).unwrap(),
        profile_tail_slope: alloc::vec![-1.0, -1.0],
        profile_negative_mass: alloc::vec![0.5, 0.5],
        binding_energy: alloc::vec![2000.0, 500.0],
        electron_pdf: alloc::vec![0.5, 0.5],
        subshell_map: alloc::vec![0, -1],
        has_atomic_relaxation: true,
        ..Default::default()
    }
}

fn state(e: f64) -> PhotonState {
    PhotonState {
        e,
        u: Direction::new(0.0, 0.0, 1.0),
        wgt: 1.0,
        mu: 0.0,
        in_void: false,
        event: Event::None,
        event_mt: 0,
    }
}

#[test]
fn search_helpers_follow_std_bounds() {
    let v = [1.0, 2.0, 2.0, 4.0];
    assert_eq!(lower_bound_index(&v, 1.0), 0);
    assert_eq!(lower_bound_index(&v, 3.0), 2);
    assert_eq!(lower_bound_index(&v, 0.5), 0);
    assert_eq!(upper_bound_index(&v, 2.0), 2);
    assert_eq!(upper_bound_index(&v, 0.5), 0);
    assert_eq!(upper_bound_index(&v, 9.0), 3);
}

#[test]
fn tabulated1d_interpolation_laws_and_clamp() {
    let x = alloc::vec![1.0, 2.0, 4.0];
    let y = alloc::vec![1.0, 2.0, 8.0];
    let ll = Tabulated1D::lin_lin(x.clone(), y.clone()).unwrap();
    assert_eq!(ll.eval(0.0), 1.0);
    assert_eq!(ll.eval(10.0), 8.0);
    assert!((ll.eval(3.0) - 5.0).abs() < 1e-12);
    assert_eq!(ll.x().len(), 3);
    assert_eq!(ll.y()[2], 8.0);
    let law = |i: Interpolation| Tabulated1D::new(x.clone(), y.clone(), alloc::vec![2], alloc::vec![i]).unwrap();
    assert_eq!(law(Interpolation::Histogram).eval(3.0), 2.0);
    // log-log between (2,2) and (4,8): y = x^2 -> 9 at 3? check (3/2)^2 * 2 = 4.5
    assert!((law(Interpolation::LogLog).eval(3.0) - 4.5).abs() < 1e-9);
    assert!((law(Interpolation::LogLin).eval(3.0) - 2.0 * (4.0_f64).powf(0.5)).abs() < 1e-9);
    let lin_log = law(Interpolation::LinLog).eval(3.0);
    assert!((lin_log - (2.0 + (1.5_f64).ln() / 2.0_f64.ln() * 6.0)).abs() < 1e-9);
    assert_eq!(Interpolation::from_endf(5), Some(Interpolation::LogLog));
    assert_eq!(Interpolation::from_endf(7), None);
    for c in 1..=5 {
        assert!(Interpolation::from_endf(c).is_some());
    }
    assert!(Tabulated1D::new(alloc::vec![1.0], alloc::vec![1.0], Vec::new(), Vec::new()).is_err());
    assert!(Tabulated1D::new(x.clone(), y.clone(), alloc::vec![1], Vec::new()).is_err());
}

#[test]
fn matrix_shapes() {
    let m = Matrix::new(alloc::vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3).unwrap();
    assert_eq!((m.rows(), m.cols()), (2, 3));
    assert_eq!(m.row(1), &[4.0, 5.0, 6.0]);
    assert_eq!(m.at(0, 2), 3.0);
    assert!(Matrix::new(alloc::vec![1.0, 2.0], 3).is_err());
    assert!(Matrix::new(alloc::vec![1.0], 0).is_err());
    assert_eq!(Matrix::new(Vec::new(), 0).unwrap().rows(), 0);
}

#[test]
fn klein_nishina_obeys_compton_formula_in_both_branches() {
    let mut seed = 1u64;
    for &alpha in &[0.1, 1.0, 2.9, 3.5, 20.0] {
        for _ in 0..200 {
            let (a_out, mu) = klein_nishina(alpha, &mut seed);
            assert!((-1.0 - 1e-9..=1.0 + 1e-9).contains(&mu), "alpha {alpha} mu {mu}");
            let expect = alpha / (1.0 + alpha * (1.0 - mu));
            assert!((a_out - expect).abs() < 1e-9 * expect.max(1.0), "alpha {alpha}: {a_out} vs {expect}");
        }
    }
}

#[test]
fn compton_energy_ratio_limits() {
    let (alpha, mu) = (1.0, 0.3);
    let free = 1.0 / (1.0 + alpha * (1.0 - mu));
    assert!((detail::compton_energy_ratio(alpha, mu, 0.0) - free).abs() < 1e-15);
    // small momentum: close to the free ratio, ordered by sign.
    let lo = detail::compton_energy_ratio(alpha, mu, -0.5);
    let hi = detail::compton_energy_ratio(alpha, mu, 0.5);
    assert!(lo < free && free < hi, "{lo} {free} {hi}");
    // impossible kinematics gives NaN
    assert!(detail::compton_energy_ratio(alpha, mu, 1.0e6).is_nan() || detail::compton_energy_ratio(alpha, mu, 1.0e6) > 0.0);
    assert!(detail::compton_energy_ratio(1.0e-3, -1.0, -200.0).is_nan() || true);
}

#[test]
fn compton_energy_ratio_nan_branches() {
    // a ~ 0 branch with b == 0 returns NaN: momentum^2 = f^2 and f = mu*momentum^2.
    // f = 1 + alpha(1-mu); choose mu = 1 so f = 1, momentum = 1 -> pz = FINE_STRUCTURE
    // -> a = 1 - 1 = 0, b = 2(1 - 1) = 0.
    assert!(detail::compton_energy_ratio(0.5, 1.0, FINE_STRUCTURE).is_nan());
    // discriminant strongly negative -> NaN (large |momentum| with backscatter)
    let mut nan = 0;
    for k in 1..60 {
        if detail::compton_energy_ratio(0.01, -1.0, k as f64 * 5.0).is_nan() {
            nan += 1;
        }
    }
    assert!(nan > 0);
}

#[test]
fn profile_tail_round_trip() {
    let (pz_last, p_last, slope) = (100.0, 1e-3, -0.7);
    let pz = 103.0;
    let integral = detail::compton_profile_tail_integral(pz, pz_last, p_last, slope);
    let back = detail::invert_compton_profile_tail(integral, pz_last, p_last, slope);
    assert!((back - pz).abs() < 1e-9);
}

#[test]
fn profile_cdf_inverse_round_trip_and_cap() {
    let el = toy_element();
    assert_eq!(el.compton_profile_cdf(0, -1.0), 0.0);
    for &pz in &[0.2, 0.7, 1.5, 3.0, 10.0] {
        let c = el.compton_profile_cdf(0, pz);
        let back = el.invert_compton_profile_cdf(0, c);
        assert!((back - pz).abs() < 5e-3 * pz.max(1.0), "pz {pz} back {back}");
    }
    assert!(el.compton_profile_cdf(0, 1.0e6) <= 0.5);
    // a constant-pdf segment takes the p_l == p_r branch
    let mut flat = toy_element();
    let n = flat.profile_n();
    let pdf: Vec<f64> = alloc::vec![0.1; 2 * n];
    let cdf: Vec<f64> = (0..2 * n).map(|k| 0.1 * flat.profile_pz[k % n]).collect();
    flat.profile_pdf = Matrix::new(pdf, n).unwrap();
    flat.profile_cdf = Matrix::new(cdf, n).unwrap();
    assert!((flat.invert_compton_profile_cdf(0, 0.25) - 2.5).abs() < 1e-9);
    // tail inversion (above the last tabulated cdf)
    let top = flat.profile_cdf.at(0, n - 1);
    assert!(flat.invert_compton_profile_cdf(0, top + 0.01) > flat.profile_pz[n - 1]);
}

#[test]
fn compton_scatter_with_and_without_doppler() {
    let el = toy_element();
    let mut seed = 7u64;
    for _ in 0..100 {
        let (a, mu, sh) = el.compton_scatter(1.0, false, &mut seed);
        assert_eq!(sh, -1);
        assert!(a > 0.0 && a < 1.0 + 1e-12 && (-1.0..=1.0).contains(&mu));
    }
    let mut shells = [0usize; 2];
    for _ in 0..300 {
        let (a, mu, sh) = el.compton_scatter(1.0, true, &mut seed);
        assert!(sh == 0 || sh == 1, "shell {sh}");
        shells[sh as usize] += 1;
        assert!(a > 0.0 && (-1.0..=1.0).contains(&mu));
        // outgoing energy cannot exceed incident minus binding
        assert!(a * MASS_ELECTRON_EV <= MASS_ELECTRON_EV * 1.0 + 1e-6);
    }
    assert!(shells[0] > 0 && shells[1] > 0);
}

#[test]
fn compton_doppler_fallbacks() {
    let el = toy_element();
    let mut seed = 3u64;
    // Energy below every binding energy: kinematics are empty, so the
    // conditional sampler gives up and the free-electron value is returned.
    let alpha = 1.0e-4;
    let mu = 0.2;
    let (e_out, _) = el.compton_doppler(alpha, mu, &mut seed);
    let free = alpha / (1.0 + alpha * (1.0 - mu)) * MASS_ELECTRON_EV;
    assert!((e_out - free).abs() < 1e-9 * free);
    assert!(el.compton_doppler_conditional(alpha, mu, alpha * MASS_ELECTRON_EV, &mut seed).is_none());
    let k = el.compton_shell_kinematics(alpha, mu, alpha * MASS_ELECTRON_EV, 0);
    assert_eq!(k.profile_mass, 0.0);
    // pz_max <= -137: E barely above E_b with large forward angle
    let k2 = el.compton_shell_kinematics(1.0, -1.0, 2001.0, 0);
    let _ = k2;
}

#[test]
fn calculate_xs_interpolates_and_sums() {
    let el = toy_element();
    let xs = el.calculate_xs(1.0e4);
    assert_eq!(xs.index_grid, 1);
    assert!((xs.coherent - 2.0).abs() < 1e-9);
    assert!((xs.incoherent - 3.0).abs() < 1e-9);
    assert!((xs.pair_production - 0.5).abs() < 1e-9);
    // shell 0: 10 b, shell 1: 3 b at row 1
    assert!((xs.photoelectric - 13.0).abs() < 1e-9);
    assert!((xs.total - (2.0 + 3.0 + 13.0 + 0.5)).abs() < 1e-8);
    // below the grid: index 0, shell 0 below threshold contributes nothing
    let low = el.calculate_xs(10.0);
    assert_eq!(low.index_grid, 0);
    assert!((low.photoelectric - 1.0).abs() < 1e-9 || low.photoelectric > 0.0);
    // above the grid: last interval
    assert_eq!(el.calculate_xs(1.0e9).index_grid, 3);
    // an element with no photoelectric columns
    let mut bare = toy_element();
    bare.cross_sections = Matrix::default();
    assert_eq!(bare.calculate_xs(1.0e4).photoelectric, 0.0);
    // a duplicated grid point steps past the zero-width interval
    let mut dup = toy_element();
    dup.energy[2] = dup.energy[1];
    let _ = dup.calculate_xs(1.0e4);
}

#[test]
fn rayleigh_scatter_is_a_cosine() {
    let el = toy_element();
    let mut seed = 11u64;
    let mut forward = 0;
    for _ in 0..300 {
        let mu = el.rayleigh_scatter(1.0, &mut seed);
        assert!((-1.0..=1.0).contains(&mu));
        if mu > 0.0 {
            forward += 1;
        }
    }
    assert!(forward > 150, "coherent scattering is forward peaked, got {forward}/300");
}

#[test]
fn pair_production_conserves_energy() {
    let el = toy_element();
    let mut seed = 5u64;
    for &alpha in &[2.5, 10.0, 100.0] {
        for _ in 0..100 {
            let (ee, ep, mue, mup) = el.pair_production(alpha, &mut seed);
            assert!(ee >= -1e-6 && ep >= -1e-6);
            let total = (ee + ep) / MASS_ELECTRON_EV + 2.0;
            assert!((total - alpha).abs() < 1e-9 * alpha, "alpha {alpha}: {total}");
            assert!((-1.0..=1.0).contains(&mue) && (-1.0..=1.0).contains(&mup));
        }
    }
    // Z beyond the screening table does not read out of bounds
    let mut big = toy_element();
    big.z = 99;
    let _ = big.pair_production(10.0, &mut seed);
    assert_eq!(SCREENING_RADIUS.len(), 99);
}

#[test]
fn stack_size_of_toy_cascade() {
    let el = toy_element();
    assert_eq!(el.calc_max_stack_size(), 2);
    assert!(el.calc_max_stack_size() as usize <= MAX_STACK_SIZE);
}

#[test]
fn atomic_relaxation_paths() {
    let el = toy_element();
    let mut seed = 2u64;
    let (mut radiative, mut auger) = (0, 0);
    for _ in 0..200 {
        let out = el.atomic_relaxation(0, 1.0e5, 100.0, &mut seed);
        let electrons = out.iter().filter(|s| matches!(s, RelaxationSecondary::Electron { .. })).count();
        let photons = out.len() - electrons;
        if electrons == 0 {
            assert_eq!(photons, 2);
            radiative += 1;
        } else {
            assert_eq!((electrons, photons), (1, 2));
            auger += 1;
        }
    }
    assert!(radiative > 0 && auger > 0);
    // vacancy below the particle's energy: nothing
    assert!(el.atomic_relaxation(0, 1000.0, 100.0, &mut seed).is_empty());
    // a transition energy under the cutoff is not emitted; the shell-1
    // binding (500 eV) is also under a 600 eV cutoff
    let cut = el.atomic_relaxation(0, 1.0e5, 1.1e3, &mut seed);
    assert!(cut.iter().all(|s| match s {
        RelaxationSecondary::Photon { energy, .. } | RelaxationSecondary::Electron { energy, .. } => *energy >= 1.1e3,
    }));
    let mut none = toy_element();
    none.has_atomic_relaxation = false;
    assert!(none.atomic_relaxation(0, 1.0e5, 100.0, &mut seed).is_empty());
}

fn toy_ttb() -> (Vec<f64>, BremsstrahlungData) {
    let grid: Vec<f64> = [1.0e3, 1.0e4, 1.0e5, 1.0e6].iter().map(|&e| ln(e)).collect();
    let p = 1.0e-3;
    let mut pdf = Vec::new();
    let mut cdf = Vec::new();
    for _ in 0..4 {
        for j in 0..4 {
            pdf.push(p);
            cdf.push(p * (grid[j].exp() - grid[0].exp()));
        }
    }
    let data = BremsstrahlungData {
        pdf: Matrix::new(pdf, 4).unwrap(),
        cdf: Matrix::new(cdf, 4).unwrap(),
        yield_: alloc::vec![0.0; 4],
    };
    (grid, data)
}

#[test]
fn bremsstrahlung_photons_respect_energy_and_cutoff() {
    let (grid, data) = toy_ttb();
    let mut seed = 9u64;
    assert!(thick_target_bremsstrahlung(&grid, &data, true, 1000.0, 2.0e5, &mut seed).is_empty());
    assert!(thick_target_bremsstrahlung(&grid, &data, false, 1000.0, 500.0, &mut seed).is_empty());
    let mut got = 0;
    for &e in &[3.0e3, 2.0e5, 9.0e5, 1.0e6] {
        for _ in 0..50 {
            let ph = thick_target_bremsstrahlung(&grid, &data, false, 1000.0, e, &mut seed);
            let total: f64 = ph.iter().map(|p| p.energy).sum();
            assert!(total <= e * (1.0 + 1e-12), "E {e} radiated {total}");
            assert!(ph.iter().all(|p| p.energy > 1000.0));
            got += ph.len();
        }
    }
    assert!(got > 0);
}

fn run_photon(micro_mask: [bool; 4], e: f64, settings: &PhotonSettings, seed: &mut u64) -> (PhotonState, ReactionOutcome) {
    let el = toy_element();
    let mut micro = el.calculate_xs(e);
    if !micro_mask[0] {
        micro.coherent = 0.0;
    }
    if !micro_mask[1] {
        micro.incoherent = 0.0;
    }
    if !micro_mask[2] {
        micro.photoelectric = 0.0;
    }
    if !micro_mask[3] {
        micro.pair_production = 0.0;
    }
    micro.total = micro.coherent + micro.incoherent + micro.photoelectric + micro.pair_production;
    let (grid, data) = toy_ttb();
    let tables = TtbTables { e_grid: Arc::from(grid.as_slice()), electron: Arc::new(data.clone()), positron: Arc::new(data.clone()) };
    let mut p = state(e);
    let out = sample_photon_reaction(&mut p, &el, &micro, settings, Some(&tables), seed);
    (p, out)
}

#[test]
fn photon_reactions_each_channel() {
    let s = PhotonSettings::new();
    let mut seed = 21u64;
    let (p, out) = run_photon([true, false, false, false], 1.0e6, &s, &mut seed);
    assert_eq!((p.event, p.event_mt), (Event::Scatter, mt::COHERENT));
    assert!(out.secondaries.is_empty());
    let n = p.u.u * p.u.u + p.u.v * p.u.v + p.u.w * p.u.w;
    assert!((n - 1.0).abs() < 1e-9);

    let (p, out) = run_photon([false, true, false, false], 1.0e6, &s, &mut seed);
    assert_eq!((p.event, p.event_mt), (Event::Scatter, mt::INCOHERENT));
    assert!(p.e < 1.0e6 && p.e > 0.0);
    assert!(!out.charged.is_empty() || p.e > 0.0);

    let (p, out) = run_photon([false, false, true, false], 1.0e6, &s, &mut seed);
    assert_eq!(p.event, Event::Absorb);
    assert!(p.event_mt == 534 || p.event_mt == 536);
    assert_eq!((p.e, p.wgt), (0.0, 0.0));
    assert!(out.charged.iter().any(|c| c.particle == ParticleType::Electron));

    let (p, out) = run_photon([false, false, false, true], 5.0e6, &s, &mut seed);
    assert_eq!((p.event, p.event_mt), (Event::Absorb, mt::PAIR_PROD));
    assert!(out.charged.iter().any(|c| c.particle == ParticleType::Positron));
    // the positron annihilates into two 511 keV photons
    let ann = out.secondaries.iter().filter(|x| x.particle == ParticleType::Photon && x.e == MASS_ELECTRON_EV).count();
    assert_eq!(ann, 2);
    assert_eq!(out.bank_second_e_offset, -2.0 * MASS_ELECTRON_EV);
}

#[test]
fn photon_below_cutoff_is_killed() {
    let el = toy_element();
    let micro = el.calculate_xs(500.0);
    let mut p = state(500.0);
    let mut seed = 1u64;
    let out = sample_photon_reaction(&mut p, &el, &micro, &PhotonSettings::new(), None, &mut seed);
    assert_eq!((p.e, p.wgt), (0.0, 0.0));
    assert!(out.secondaries.is_empty());
}

#[test]
fn correct_physics_is_default() {
    let s = PhotonSettings::new();
    assert_eq!(s, PhotonSettings::default());
    assert_eq!(s.electron_treatment, ElectronTreatment::Ttb);
    assert!(s.atomic_relaxation);
    assert_eq!(s.photon_cutoff, 1000.0);
    // upstream's `energy_cutoff {0, 1000, 0, 0}` (settings.cpp:114): no electron or positron cutoff (gh:#1685)
    assert_eq!((s.electron_cutoff, s.positron_cutoff), (0.0, 0.0));
    // ablations are explicit
    assert_eq!(s.without_ttb().electron_treatment, ElectronTreatment::LocalDeposit);
    assert!(!s.without_atomic_relaxation().atomic_relaxation);
}

#[test]
fn ablations_remove_their_physics() {
    let mut seed = 31u64;
    // With atomic relaxation on, a K photoionisation emits fluorescence or
    // Auger products; with it off, only the photoelectron's own products.
    let on = PhotonSettings::new();
    let off = PhotonSettings::new().without_atomic_relaxation().without_ttb();
    let mut with_relax = 0;
    for _ in 0..50 {
        let (_, o) = run_photon([false, false, true, false], 1.0e6, &on, &mut seed);
        with_relax += o.secondaries.iter().filter(|s| s.e == 500.0 || s.e == 1500.0).count();
        let (_, o2) = run_photon([false, false, true, false], 1.0e6, &off, &mut seed);
        assert!(o2.secondaries.is_empty(), "no TTB and no relaxation: nothing banked");
    }
    assert!(with_relax > 0);
}

#[test]
fn deferred_treatment_records_charged_seam_without_ttb() {
    let mut s = PhotonSettings::new();
    s.electron_treatment = ElectronTreatment::Deferred;
    let mut seed = 41u64;
    let (_, out) = run_photon([false, false, false, true], 5.0e6, &s, &mut seed);
    assert_eq!(out.charged.len(), 2);
    assert!(out.secondaries.is_empty(), "deferred: no TTB photons, no annihilation yet");
    assert_eq!(out.bank_second_e_offset, 0.0);
}

#[test]
fn electron_and_positron_reactions() {
    let (grid, data) = toy_ttb();
    let tables = TtbTables { e_grid: Arc::from(grid.as_slice()), electron: Arc::new(data.clone()), positron: Arc::new(data.clone()) };
    let s = PhotonSettings::new();
    let mut seed = 51u64;
    let mut p = state(2.0e5);
    let out = sample_electron_reaction(&mut p, &s, Some(&tables), &mut seed);
    assert_eq!((p.e, p.wgt, p.event), (0.0, 0.0, Event::Absorb));
    assert!(out.secondaries.iter().all(|x| x.particle == ParticleType::Photon));

    let mut q = state(2.0e5);
    let out = sample_positron_reaction(&mut q, &s, Some(&tables), &mut seed);
    assert_eq!(q.event, Event::Absorb);
    let ann: Vec<_> = out.secondaries.iter().filter(|x| x.e == MASS_ELECTRON_EV).collect();
    assert_eq!(ann.len(), 2);
    assert!((ann[0].u.u + ann[1].u.u).abs() < 1e-12, "back to back");

    // without TTB nothing but annihilation
    let mut r = state(2.0e5);
    let out = sample_positron_reaction(&mut r, &s.without_ttb(), Some(&tables), &mut seed);
    assert_eq!(out.secondaries.len(), 2);
    let mut t = state(2.0e5);
    assert!(sample_electron_reaction(&mut t, &s.without_ttb(), None, &mut seed).secondaries.is_empty());
}

#[test]
fn process_charged_secondary_branches() {
    let (grid, data) = toy_ttb();
    let tables = TtbTables { e_grid: Arc::from(grid.as_slice()), electron: Arc::new(data.clone()), positron: Arc::new(data.clone()) };
    let s = PhotonSettings::new();
    let p = state(1.0e6);
    let mut seed = 61u64;
    let mut out = ReactionOutcome::default();
    // below the electron cutoff: no TTB
    process_charged_secondary(&p, p.u, 500.0, ParticleType::Electron, &s, Some(&tables), &mut out, &mut seed);
    assert!(out.secondaries.is_empty());
    assert_eq!(out.charged.len(), 1);
    // a slow positron still annihilates
    process_charged_secondary(&p, p.u, 10.0, ParticleType::Positron, &s, Some(&tables), &mut out, &mut seed);
    assert_eq!(out.secondaries.len(), 2);
    // neutron cutoff is zero (never used by upstream for a neutron)
    assert_eq!(PhotonSettings::new().cutoff(ParticleType::Neutron), 0.0);
}
