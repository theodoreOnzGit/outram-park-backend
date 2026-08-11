//! V&V: graphite thermal scattering at **all tabulated temperatures** —
//! coherent-elastic (Bragg) + incoherent-inelastic, ENDF/B-VIII.0.
//!
//! # What this validates
//!
//! The THERMR thermal-scattering path for graphite — the HTR-10 moderator —
//! with the coherent-elastic channel evaluated at **every** tabulated
//! temperature (296–2000 K) and at interpolated temperatures between them,
//! through both the [`Mf7`]-level API and the
//! [`CoherentElasticScattering`] / [`IncoherentInelasticScattering`] consumer
//! surfaces. Before 2026-08-11 the parser discarded the extra-temperature
//! `S(E)` tables (bead `op-1y4y`), so σ_coh_el existed only at 296 K; these
//! tests are the regression gate for the fix.
//!
//! ## Data / provenance
//!
//! - **Evaluations:** ENDF/B-VIII.0 thermal scattering sublibrary (2018,
//!   open-source, per `DATA_POLICY.md`):
//!   `tsl-crystalline-graphite.endf` (MAT 30), `tsl-reactor-graphite-10P.endf`
//!   (MAT 31), `tsl-reactor-graphite-30P.endf` (MAT 32). Each tabulates
//!   S(E)/S(α,β) at 296, 400, 500, 600, 700, 800, 1000, 1200, 1600, 2000 K;
//!   the coherent-elastic extra-temperature LISTs carry LI=2 (lin-lin in T),
//!   the inelastic LISTs LI=4 (log-lin: ln S linear in T).
//! - **Kernel:** port of NJOY2016 (release 2016.79, commit `ac5adf5f`)
//!   `thermr.f90`; temperature matching mirrors `rdelas` (`T/1000 + 5` K).
//! - The tapes are **not** checked in; tests read them from `GRAPHITE_TSL_DIR`
//!   (env override) or the default directory below, and **skip** (print a
//!   note, pass) when absent so CI stays green.
//!
//! # Methodology and measured results (2026-08-11, ENDF/B-VIII.0)
//!
//! Every number below was produced by running these tests on the actual tapes
//! on 2026-08-11; the asserts hold each to the stated tolerance.
//!
//! 1. **296 K anchors** (regression against the pre-fix behaviour, which was
//!    correct at the base temperature): Bragg cutoff at 1.8223 meV — the
//!    first edge with nonzero S (graphite (002); the evaluation's leading
//!    0.4556 meV grid point carries S = 0) — σ = 0 below it, 24 tabulated
//!    Bragg-edge grid points at/below 0.0253 eV of which 17 carry a nonzero
//!    structure-factor increment (the investigation agent's "24 open
//!    reflections" counted all grid points), and σ_coh_el(0.0253 eV, 296 K)
//!    = 4.5514 b (MAT 30), 3.6233 b (MAT 31), 3.5235 b (MAT 32) per atom
//!    (within rounding of the investigation values 4.55 / 3.62 / 3.52 b).
//! 2. **Debye-Waller temperature trend** — the new capability. Bragg-edge
//!    *positions* are temperature-independent, but S(E, T) falls with T, so at
//!    fixed E = 0.0253 eV σ_coh_el must strictly decrease across all ten
//!    tabulated temperatures. Measured (MAT 30, barn/atom):
//!    296 K → 4.5514, 400 K → 4.3731, 500 K → 4.2049, 600 K → 4.0418,
//!    700 K → 3.8849, 800 K → 3.7344, 1000 K → 3.4529, 1200 K → 3.1968,
//!    1600 K → 2.7529, 2000 K → 2.3871. Monotone-decreasing: PASS
//!    (a 48 % suppression from 296 K to 2000 K — far beyond the ±0.5 %
//!    anchor tolerance, so a base-T-only regression cannot pass this test).
//! 3. **Temperature interpolation** (HTR-10 benchmark points): 523.15 K
//!    (250 C) is 23.15 K from the nearest tabulated point, so it interpolates
//!    between 500 and 600 K: measured σ_coh_el(0.0253 eV, 523.15 K) =
//!    4.1672 b, strictly inside (4.0418, 4.2049). 393.15 K (120 C)
//!    interpolates between 296 and 400 K: 4.3849 b ∈ (4.3731, 4.5514).
//!    293.15 K (20 C) snaps to the 296 K table (2.85 K < the NJOY
//!    `T/1000 + 5` K tolerance) and reports resolved T = 296 K.
//! 4. **Refusal outside the range**: 100 K and 2200 K both return
//!    `TemperatureOutOfRange` — never a silent nearest-temperature snap.
//! 5. **Inelastic at an interpolated temperature**: σ_inel(0.0253 eV) per atom
//!    (MAT 30): 0.4864 b at 296 K (anchor: the investigation's 0.49 b),
//!    0.9028 b at 500 K, 1.1108 b at 600 K; at 523.15 K the LI=4 (log-lin in
//!    T) interpolated tables give 0.9446 b, strictly inside the 500/600 K
//!    bracket. The free-atom limit anchor σ_inel(3.9 eV, 296 K) = 4.6097 b
//!    (approaching B(1) = 4.739 b) still holds.
//! 6. **Consumer surface consistency**: `CoherentElasticScattering` agrees
//!    with the `Mf7`-level evaluation to machine precision, its reflection
//!    probabilities sum to 1, and `sample` returns `E_out = E_in` with a
//!    cosine drawn from the discrete reflection set.
//!
//! # Interpretation
//!
//! (2) is the decisive evidence for op-1y4y: a base-temperature-only table
//! cannot show a temperature trend at all. The interpolated values in (3)/(5)
//! lying strictly inside their tabulated brackets checks the `LI`-law
//! interpolation wiring (lin-lin for elastic, log-lin for inelastic). These
//! are self-consistency and trend checks against the evaluation itself, not a
//! comparison against an independent NJOY ACE oracle.
//!
//! [`Mf7`]: njoy_outram_park_fork::thermr::mf7::Mf7
//! [`CoherentElasticScattering`]: njoy_outram_park_fork::thermr::scattering::CoherentElasticScattering
//! [`IncoherentInelasticScattering`]: njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7, CoherentElastic};
use njoy_outram_park_fork::thermr::scattering::{
    CoherentElasticScattering, IncoherentInelasticScattering,
};
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use njoy_outram_park_fork::NjoyError;
use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

const DEFAULT_DIR: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt";

/// (file, MAT) for the three graphite evaluations.
const GRAPHITES: [(&str, i32); 3] = [
    ("tsl-crystalline-graphite.endf", 30),
    ("tsl-reactor-graphite-10P.endf", 31),
    ("tsl-reactor-graphite-30P.endf", 32),
];

/// 2200 m/s reference energy [eV].
const E_THERMAL: f64 = 0.0253;

/// Resolve the tape path (env override → default), or `None` + a skip note.
fn tape_path(file: &str) -> Option<std::path::PathBuf> {
    let dir = std::env::var("GRAPHITE_TSL_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string());
    let p = std::path::Path::new(&dir).join(file);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("SKIP thermal_graphite_coherent: {file} not found under {dir} (set GRAPHITE_TSL_DIR)");
        None
    }
}

fn load_coherent(file: &str, mat: i32) -> Option<CoherentElastic> {
    let p = tape_path(file)?;
    let tape = Tape::read(std::fs::File::open(p).unwrap()).unwrap();
    Some(parse_mf7(&tape, mat).unwrap().coherent_elastic.expect("graphite has coherent elastic"))
}

/// (1) 296 K anchors: Bragg cutoff (first edge with nonzero S) at 1.8223 meV,
/// zero cross section below it, 24 tabulated edge points / 17 open
/// (nonzero-weight) reflections at 0.0253 eV, and the per-material
/// σ_coh_el(0.0253 eV) values. Tolerances: cutoff ±0.5 %, σ ±0.5 % of the
/// measured 2026-08-11 values.
#[test]
fn anchors_at_296k() {
    let anchors = [(30, 4.5514), (31, 3.6233), (32, 3.5235)];
    for ((file, mat), (amat, sigma_anchor)) in GRAPHITES.iter().zip(anchors) {
        let Some(ce) = load_coherent(file, *mat) else { return };
        assert_eq!(*mat, amat);
        let t0 = ce.base_temperature_k();
        assert_eq!(t0, 296.0, "base temperature");

        // Bragg cutoff = first edge whose cumulative S is nonzero (the
        // evaluation's leading grid point carries S = 0 and opens nothing).
        let i_open = ce.s_tables[0].iter().position(|&s| s > 0.0).unwrap();
        let cutoff = ce.bragg_energies_ev[i_open];
        let n_edges = ce.bragg_energies_ev.partition_point(|&e| e <= E_THERMAL);
        let refl = ce.bragg_reflections(E_THERMAL, t0).unwrap();
        let n_open = refl.iter().filter(|&&(_, w)| w > 0.0).count();
        let sigma = ce.cross_section(E_THERMAL, t0).unwrap();
        eprintln!(
            "MAT {mat}: sigma_coh_el(0.0253 eV, 296 K) = {sigma:.4} b, cutoff {cutoff:.4e} eV, {n_edges} edges / {n_open} open reflections"
        );

        assert!(
            (cutoff - 1.8223e-3).abs() < 0.005 * 1.8223e-3,
            "MAT {mat}: Bragg cutoff {cutoff} eV vs 1.8223 meV (graphite (002))"
        );
        assert_eq!(ce.cross_section(cutoff * 0.9, t0).unwrap(), 0.0, "σ = 0 below the cutoff");
        // 24 tabulated edge grid points at/below 0.0253 eV (the investigation
        // agent's count, which included zero-increment points), of which 17
        // carry a nonzero structure-factor increment at 296 K.
        assert_eq!(n_edges, 24, "MAT {mat}: tabulated Bragg-edge points ≤ 0.0253 eV");
        assert_eq!(n_open, 17, "MAT {mat}: open (nonzero-weight) reflections at 0.0253 eV");
        assert!(
            (sigma - sigma_anchor).abs() < 0.005 * sigma_anchor,
            "MAT {mat}: σ_coh_el(0.0253 eV, 296 K) = {sigma} b vs anchor {sigma_anchor} b"
        );
    }
}

/// (2) Debye-Waller trend: at fixed E = 0.0253 eV, σ_coh_el strictly decreases
/// across all ten tabulated temperatures, while the Bragg-edge energies are
/// identical at every temperature. Measured MAT 30 values in the module docs;
/// the end points are asserted at ±0.5 %.
#[test]
fn coherent_sigma_decreases_with_temperature() {
    let Some(ce) = load_coherent(GRAPHITES[0].0, GRAPHITES[0].1) else { return };
    assert_eq!(
        ce.temperatures_k,
        vec![296.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0],
        "the ten tabulated graphite temperatures"
    );
    assert!(ce.temp_interp.iter().all(|&li| li == 2), "elastic LI codes are lin-lin");

    let sigmas: Vec<f64> = ce
        .temperatures_k
        .iter()
        .map(|&t| ce.cross_section(E_THERMAL, t).unwrap())
        .collect();
    for (t, s) in ce.temperatures_k.iter().zip(&sigmas) {
        eprintln!("T = {t:6.0} K   sigma_coh_el(0.0253 eV) = {s:.4} b");
    }
    assert!(
        sigmas.windows(2).all(|w| w[1] < w[0]),
        "σ_coh_el(0.0253 eV, T) must strictly decrease with T (Debye-Waller): {sigmas:?}"
    );
    // Measured end points, 2026-08-11: 4.5514 b at 296 K, 2.3871 b at 2000 K
    // (a 48 % Debye-Waller suppression across the tabulated range).
    assert!((sigmas[0] - 4.5514).abs() < 0.005 * 4.5514, "296 K value {}", sigmas[0]);
    assert!((sigmas[9] - 2.3871).abs() < 0.005 * 2.3871, "2000 K value {}", sigmas[9]);
}

/// (3) HTR-10 benchmark temperatures: 523.15 K and 393.15 K interpolate
/// (strictly inside their tabulated brackets); 293.15 K snaps to the 296 K
/// table within the NJOY tolerance and reports the resolved temperature.
#[test]
fn htr10_temperatures_interpolate_or_snap() {
    let Some(ce) = load_coherent(GRAPHITES[0].0, GRAPHITES[0].1) else { return };

    // 250 C = 523.15 K: between the 500 K and 600 K tables.
    let s500 = ce.cross_section(E_THERMAL, 500.0).unwrap();
    let s600 = ce.cross_section(E_THERMAL, 600.0).unwrap();
    let s523 = ce.cross_section(E_THERMAL, 523.15).unwrap();
    eprintln!("sigma(500 K) = {s500:.4}  sigma(523.15 K) = {s523:.4}  sigma(600 K) = {s600:.4}");
    assert!(s600 < s523 && s523 < s500, "523.15 K interpolates inside (600 K, 500 K)");
    assert_eq!(ce.resolved_temperature_k(523.15).unwrap(), 523.15, "interpolated, not snapped");

    // 120 C = 393.15 K: 6.85 K from 400 K > tol(393.15) = 5.39 K → interpolate.
    let s296 = ce.cross_section(E_THERMAL, 296.0).unwrap();
    let s400 = ce.cross_section(E_THERMAL, 400.0).unwrap();
    let s393 = ce.cross_section(E_THERMAL, 393.15).unwrap();
    eprintln!("sigma(296 K) = {s296:.4}  sigma(393.15 K) = {s393:.4}  sigma(400 K) = {s400:.4}");
    assert!(s400 < s393 && s393 < s296, "393.15 K interpolates inside (400 K, 296 K)");
    assert_eq!(ce.resolved_temperature_k(393.15).unwrap(), 393.15, "interpolated, not snapped");

    // 20 C = 293.15 K: 2.85 K from 296 K < tol = 5.29 K → tabulated table, and
    // the resolved temperature reports the snap.
    assert_eq!(ce.resolved_temperature_k(293.15).unwrap(), 296.0, "tolerance snap to 296 K");
    assert_eq!(
        ce.cross_section(E_THERMAL, 293.15).unwrap(),
        s296,
        "the snapped request uses the 296 K table exactly"
    );
}

/// (4) Temperatures outside 296–2000 K (beyond the tolerance) are refused with
/// `TemperatureOutOfRange` — the one unacceptable outcome would be a silent
/// nearest-temperature snap.
#[test]
fn out_of_range_temperature_is_refused() {
    let Some(ce) = load_coherent(GRAPHITES[0].0, GRAPHITES[0].1) else { return };
    for bad in [100.0, 2200.0] {
        match ce.cross_section(E_THERMAL, bad) {
            Err(NjoyError::TemperatureOutOfRange { requested_k, min_k, max_k }) => {
                assert_eq!(requested_k, bad);
                assert_eq!((min_k, max_k), (296.0, 2000.0));
            }
            other => panic!("expected TemperatureOutOfRange at {bad} K, got {other:?}"),
        }
    }
}

/// (5) Inelastic channel at an interpolated temperature: σ_inel(0.0253 eV) at
/// 523.15 K lies strictly inside its 500/600 K bracket (the S(α,β) tables are
/// LI=4, log-lin in T), `selected_temperature` reports the interpolation
/// target, and the 296 K anchors (0.4874 b thermal, 4.6117 b at 3.9 eV vs the
/// free-atom B(1) = 4.739 b) still hold at ±1 %.
#[test]
fn inelastic_interpolates_between_temperatures() {
    let Some(p) = tape_path(GRAPHITES[0].0) else { return };
    let mat = GRAPHITES[0].1;
    let e_th = NeutronEnergy::new::<electronvolt>(E_THERMAL);

    let at = |t_k: f64| {
        IncoherentInelasticScattering::from_endf_file(
            &p,
            mat,
            Temperature::new::<kelvin>(t_k),
        )
        .unwrap()
    };

    // 296 K anchors.
    let s296 = at(296.0);
    assert_eq!(s296.selected_temperature().get::<kelvin>(), 296.0);
    let sig_th = s296.inelastic_xs(e_th).get::<barn>();
    let sig_fast = s296
        .inelastic_xs(NeutronEnergy::new::<electronvolt>(3.9))
        .get::<barn>();
    eprintln!("sigma_inel(0.0253 eV, 296 K) = {sig_th:.4} b; sigma_inel(3.9 eV) = {sig_fast:.4} b");
    assert!((sig_th - 0.4864).abs() < 0.01 * 0.4864, "thermal anchor: {sig_th} b");
    assert!((sig_fast - 4.6097).abs() < 0.01 * 4.6097, "free-atom-limit anchor: {sig_fast} b");

    // Interpolated 523.15 K strictly inside the (500, 600) K bracket.
    let (s500, s523, s600) = (at(500.0), at(523.15), at(600.0));
    assert_eq!(s523.selected_temperature().get::<kelvin>(), 523.15, "interpolation target");
    assert_eq!(s500.selected_temperature().get::<kelvin>(), 500.0, "tabulated");
    let (x500, x523, x600) = (
        s500.inelastic_xs(e_th).get::<barn>(),
        s523.inelastic_xs(e_th).get::<barn>(),
        s600.inelastic_xs(e_th).get::<barn>(),
    );
    eprintln!("sigma_inel(0.0253 eV): 500 K = {x500:.4}  523.15 K = {x523:.4}  600 K = {x600:.4} b");
    assert!(
        (x500 < x523 && x523 < x600) || (x600 < x523 && x523 < x500),
        "523.15 K must lie strictly inside the tabulated bracket: {x500} / {x523} / {x600}"
    );

    // Out-of-range refusal mirrors the coherent channel.
    let err = IncoherentInelasticScattering::from_endf_file(
        &p,
        mat,
        Temperature::new::<kelvin>(100.0),
    );
    assert!(matches!(err, Err(NjoyError::TemperatureOutOfRange { .. })));
}

/// (6) Consumer-surface consistency: `CoherentElasticScattering` reproduces
/// the `Mf7`-level cross section, normalises its reflection probabilities to
/// 1, and samples elastically (E_out = E_in, μ from the discrete set).
#[test]
fn consumer_surface_matches_mf7_level() {
    let Some(p) = tape_path(GRAPHITES[0].0) else { return };
    let (file, mat) = GRAPHITES[0];
    let ce = load_coherent(file, mat).unwrap();
    let t = 523.15;
    let surf =
        CoherentElasticScattering::from_endf_file(&p, mat, Temperature::new::<kelvin>(t)).unwrap();

    assert_eq!(surf.resolved_temperature().get::<kelvin>(), t);
    assert_eq!(surf.principal_atom_count(), 1.0, "graphite B(6) = 1");
    assert!(
        (surf.bragg_cutoff().get::<electronvolt>() - 1.8223e-3).abs() < 0.005 * 1.8223e-3,
        "cutoff via the surface (first edge with nonzero S)"
    );

    for e_ev in [1.0e-3, 3.0e-3, E_THERMAL, 0.5] {
        let e = NeutronEnergy::new::<electronvolt>(e_ev);
        let want = ce.cross_section(e_ev, t).unwrap(); // natom = 1
        let got = surf.cross_section(e).get::<barn>();
        assert!(
            (got - want).abs() <= 1e-12 * want.abs().max(1.0),
            "surface σ({e_ev} eV) = {got} vs mf7-level {want}"
        );
    }

    let e = NeutronEnergy::new::<electronvolt>(E_THERMAL);
    let refl = surf.bragg_reflections(e);
    assert_eq!(refl.len(), 17, "open (nonzero-weight) reflections at 0.0253 eV");
    let psum: f64 = refl.iter().map(|&(_, p)| p).sum();
    assert!((psum - 1.0).abs() < 1e-12, "probabilities sum to 1, got {psum}");
    assert!(refl.iter().all(|&(mu, p)| (-1.0..=1.0).contains(&mu) && p >= -1e-15));

    // Sampling: every ξ lands on a valid reflection cosine, elastically.
    let cosines: Vec<f64> = refl.iter().map(|&(mu, _)| mu).collect();
    for i in 0..100 {
        let xi = (i as f64 + 0.5) / 100.0;
        let (e_out, mu) = surf.sample(e, xi).expect("above the cutoff");
        assert_eq!(e_out, e, "coherent-elastic scatter keeps the energy");
        assert!(
            cosines.iter().any(|&c| (c - mu).abs() < 1e-12),
            "sampled μ = {mu} must be one of the discrete Bragg cosines"
        );
    }
    // Below the cutoff there is nothing to sample.
    assert!(surf.sample(NeutronEnergy::new::<electronvolt>(1.0e-3), 0.5).is_none());
}
