//! V&V: the bound-atom thermal **elastic** channel, end to end into transport.
//!
//! # What this validates
//!
//! That `outram-mc-libs` now carries *both* bound thermal channels — the
//! incoherent-inelastic S(α,β) law it already had, and the elastic law it did
//! not (bead `op-nhoa`) — and that the elastic channel is correct in cross
//! section, in sampling, and in how it substitutes for free-gas elastic inside
//! [`Nuclide::xs_at_energy`].
//!
//! The motivating case is **HTR-10**: crystalline graphite is the moderator of a
//! pebble-bed HTGR, and coherent elastic (Bragg) scattering is ~90 % of its
//! thermal scattering. Before this change a graphite nuclide carrying an S(α,β)
//! table had its free-gas elastic channel *replaced* by σ_inel alone — dropping
//! the dominant channel and leaving graphite with less thermal scattering than
//! the free gas it replaced.
//!
//! ## Data / provenance
//!
//! - **Evaluations:** ENDF/B-VIII.0 thermal scattering sublibrary (2018; open,
//!   per `DATA_POLICY.md`) — `tsl-crystalline-graphite.endf` (MAT 30),
//!   `tsl-HinH2O.endf` (MAT 1), `tsl-HinZrH.endf` (MAT 7).
//! - **Kernel:** the njoy consumer surface
//!   `njoy_outram_park_fork::thermr::scattering`, itself a port of NJOY2016
//!   (release 2016.79, commit `ac5adf5f`) `thermr.f90`.
//! - **Sampling reference:** OpenMC C++ `src/thermal.cpp:296-302`
//!   (`ThermalData::sample_dist`, the σ_el/σ_total channel split) and
//!   `src/secondary_thermal.cpp:34-54` (`CoherentElasticAE::sample`, the
//!   cumulative-structure-factor Bragg-edge draw).
//! - The tapes are **not** checked in. Tests read them from `TSL_DIR` (env
//!   override) or the default directory below, and **skip** — print a note,
//!   pass — when absent, so CI stays green.
//!
//! # Methodology and measured results (2026-08-11, ENDF/B-VIII.0, release mode)
//!
//! Every number below was produced by running these tests on the real tapes on
//! 2026-08-11 on this machine; each assert holds to the stated tolerance.
//!
//! 1. **Channel detection is from the evaluation, not the caller.** Measured:
//!    graphite MAT 30 → `coherent elastic`; `tsl-HinH2O` MAT 1 → `none`;
//!    `tsl-HinZrH` MAT 7 → `incoherent elastic`. PASS.
//!
//! 2. **Coherent-elastic σ is stored exactly, not resampled.** The sawtooth is
//!    kept as its Bragg-edge table, so this crate's σ_el(E) must equal the njoy
//!    surface's σ_coh_el(E) to floating-point round-off — not merely to a grid
//!    tolerance. Scanned 1e-4 → 4 eV in 0.05 % steps (21 199 points) against
//!    the 221 tabulated Bragg edges: **worst relative deviation 7.67e-16**
//!    (≈ 3 ulp). PASS at 1e-12. The whole table is 221 (E, σ) pairs ≈ 3.5 kB,
//!    so storing it exactly costs less than the log resample it replaces —
//!    and a resample would misplace every one of the 221 discontinuities.
//!
//! 3. **Bragg cutoff and the sub-cutoff zero.** Measured cutoff
//!    1.822327e-3 eV — graphite's (002) plane, matching the njoy-side V&V value
//!    1.8223 meV. σ_el is *exactly* 0.0 at 1.0e-4, 1.0e-3 and 1.8e-3 eV, and
//!    4.7542 b just above at 5.0e-3 eV. PASS.
//!
//! 4. **The elastic channel dominates at thermal energies** (the whole point).
//!    Measured at 296 K, barn per carbon atom:
//!
//!    | E \[eV\] | σ_inel | σ_el | σ_total | elastic fraction |
//!    |---|---|---|---|---|
//!    | 1.0e-3 | 0.3017 | 0.0000 | 0.3017 | 0.0000 |
//!    | 5.0e-3 | 0.2208 | 4.7542 | 4.9750 | 0.9556 |
//!    | 0.0253 | 0.4863 | 4.5514 | 5.0378 | **0.9035** |
//!    | 0.1    | 1.5937 | 3.2013 | 4.7949 | 0.6676 |
//!    | 1.0    | 4.2620 | 0.4788 | 4.7408 | 0.1010 |
//!    | 3.9    | 4.6095 | 0.1228 | 4.7323 | 0.0259 |
//!
//!    σ_coh_el(0.0253 eV, 296 K) = 4.5514 b reproduces the njoy-side V&V value
//!    (`op-1y4y`) to 4 decimals, and the 0.9035 elastic fraction confirms the
//!    ~90 % figure `op-nhoa` was filed on. The 1/E fall of the sawtooth against
//!    the rise of σ_inel toward the 4.739 b free-atom limit is what makes
//!    σ_total nearly flat (4.73–5.04 b) across two decades — the physical
//!    signature the old inelastic-only path destroyed.
//!
//! 5. **Nuclide-level substitution.** C-nat (`C0`, LOW/WMP tier) with the
//!    graphite table attached, versus the same nuclide without it. Measured
//!    `MicroXS::elastic` \[barn\]:
//!
//!    | E \[eV\] | free gas (WMP) | bound (new) | bound (old, σ_inel only) | new/free |
//!    |---|---|---|---|---|
//!    | 1.0e-3 | 8.9938 | 0.3017 | 0.3017 | 0.034 |
//!    | 0.0253 | 4.9398 | 5.0378 | 0.4863 | **1.020** |
//!    | 0.1    | 4.7898 | 4.7949 | 1.5937 | 1.001 |
//!
//!    The "old" column is the pre-fix behaviour, computed in the same run from
//!    `inelastic_xs`. At 0.0253 eV it gave graphite **9.8 %** of its free-gas
//!    scattering; the fixed path gives 102.0 %, the physically right
//!    neighbourhood (the free-gas WMP value already sits near the 4.74 b
//!    free-atom limit, and binding lifts it slightly at thermal energies).
//!    Below the Bragg cutoff the bound value is legitimately far *below* free
//!    gas — coherent elastic is closed there and only the small σ_inel
//!    remains, which is real physics, not a defect. The test also asserts that
//!    absorption is untouched by the substitution and that `MicroXS::total` is
//!    rebuilt consistently around the swapped channel.
//!
//! 6. **Sampling: energy conservation and the channel split.** 200 000 draws at
//!    E = 0.0253 eV, RNG seed 1. Every returned `(E_out, μ)` has |μ| ≤ 1 and
//!    E_out > 0. Measured elastic fraction (draws with E_out == E_in exactly)
//!    **0.90340**, against the cross-section prediction σ_el/σ_total =
//!    **0.90346**; difference 6.0e-5 = **0.09 σ** on a 1-σ binomial error of
//!    6.6e-4. PASS (gate: 5 σ). This is the direct check that the split
//!    mirrors OpenMC's `prn(seed) < thermal_elastic/thermal`.
//!
//! 7. **Sampling: the Bragg cosine law.** The mean cosine of the sampled
//!    elastic scatters must equal the analytic Σᵢ pᵢ μᵢ over the open
//!    reflections, which the test computes independently from the njoy
//!    surface's `bragg_reflections` (whose probabilities are separately
//!    asserted to sum to 1). At 0.0253 eV, 17 reflections are open. Measured:
//!    sampled **μ̄ = -0.02357** vs analytic **μ̄ = -0.02497**; difference
//!    1.40e-3 = **1.17 σ** on a standard error of 1.20e-3. PASS (gate: 5 σ).
//!    The near-zero mean is not isotropy — the distribution is a comb of 17
//!    discrete Bragg cosines that happens to be nearly balanced here.
//!
//! 8. **HTR-10 temperature points**, σ per carbon atom at 0.0253 eV. Measured:
//!
//!    | Requested T \[K\] | Resolved T \[K\] | σ_el | σ_inel |
//!    |---|---|---|---|
//!    | 293.15 (20 °C, B1) | 296.00 (tolerance snap) | 4.5514 | 0.4863 |
//!    | 393.15 (120 °C, B2/B3) | 393.15 (interpolated) | 4.3849 | 0.6779 |
//!    | 523.15 (250 °C, B4) | 523.15 (interpolated) | 4.1672 | 0.9445 |
//!    | 1073.15 (hot operation) | 1073.15 (interpolated) | 3.3592 | 2.0373 |
//!
//!    σ_el falls monotonically with T (Debye-Waller) while σ_inel rises, and
//!    every non-tabulated request is interpolated rather than snapped — the
//!    `op-1y4y` / `op-cjw.22` fixes reaching the transport surface. 293.15 K is
//!    within NJOY's `T/1000 + 5` K tolerance of the tabulated 296 K and is
//!    correctly reported as resolved *to* 296 K.
//!
//! 9. **Light-water regression — no behaviour change.** `tsl-HinH2O` MAT 1 at
//!    293.6 K: channel `none`, σ_el ≡ 0, and σ_total == σ_inel == 52.1420 b at
//!    0.0253 eV, bit-identical. The existing thermal pincell case is therefore
//!    unaffected by this change. PASS.
//!
//! 10. **Incoherent elastic (ZrH), the other law.** `tsl-HinZrH` MAT 7 at
//!     296 K: channel `incoherent elastic`, σ_el(0.0253 eV) = **55.0151 b**
//!     per H against σ_inel = **2.8804 b**, so this channel dominates too.
//!     Unlike the coherent sawtooth this σ(E) *is* resampled onto a log grid,
//!     so the gate is interpolation error, not round-off: worst relative
//!     deviation from the njoy surface over 1e-4 → 4 eV is **2.61e-4** at the
//!     chosen 400 points (it was 1.05e-3 at 200, the expected 4× for a halved
//!     step — the grid was sized by this measurement). Sampling returns
//!     E_out == E_in with |μ| ≤ 1 and a **forward-peaked** mean cosine:
//!     measured μ̄ = **0.13817** over 47 532 elastic draws against the njoy
//!     equiprobable-cosine mean **0.14142**, a 1.26 σ agreement on
//!     SEM = 2.59e-3. PASS.
//!
//!     This channel is **not** on the HTR-10 path (graphite has no incoherent
//!     elastic) and is verified only against this workspace's own njoy
//!     surface — no published ZrH reference value is compared against, and no
//!     claim is made about the absolute correctness of the ZrH physics.
//!
//! # What this does NOT establish
//!
//! - **No k_eff has been computed with the elastic channel in place.** These
//!   are cross-section, sampling-law and substitution checks. Wiring graphite
//!   S(α,β) into `pebble_beds` material construction and producing an HTR-10
//!   thermal-spectrum k_eff is bead `op-hc2o`, still open.
//! - **No comparison against NJOY/OpenMC-processed ACE.** The reference for
//!   items 2 and 7 is this workspace's own njoy port, so these are *internal
//!   consistency* gates, not an independent oracle. The njoy port's own
//!   graphite numbers were oracle-checked separately (`op-1y4y`).
//! - **AI-assisted draft, not human-reviewed** (RESPONSIBLE_USE.md).

use outram_mc_libs::material::thermal::{ThermalElastic, ThermalScattering};
use outram_mc_libs::prelude::Nuclide;

/// Default location of the ENDF/B-VIII.0 thermal-scattering sublibrary on this
/// machine; override with the `TSL_DIR` environment variable.
const DEFAULT_TSL_DIR: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt";

/// Absolute path of a `tsl-*` tape, honouring the `TSL_DIR` override.
fn tsl(file: &str) -> String {
    let dir = std::env::var("TSL_DIR").unwrap_or_else(|_| DEFAULT_TSL_DIR.to_string());
    format!("{dir}/{file}")
}

/// `Some(path)` when the tape is present, else `None` after printing a skip note
/// (data-gated tests pass rather than fail when the public tapes are absent).
fn tape_or_skip(file: &str, label: &str) -> Option<String> {
    let p = tsl(file);
    if std::path::Path::new(&p).exists() {
        Some(p)
    } else {
        println!("[{label}] SKIP: {p} not found (set TSL_DIR to the ENDF/B-VIII.0 thermal_scatt directory)");
        None
    }
}

/// Item 1 + 2 + 3: channel detection, exactness of the stored sawtooth, and the
/// Bragg cutoff. See the module header for the measured values.
#[test]
fn graphite_coherent_channel_is_detected_and_stored_exactly() {
    use njoy_outram_park_fork::thermr::scattering::CoherentElasticScattering;
    use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
    use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

    let Some(path) = tape_or_skip("tsl-crystalline-graphite.endf", "graphite coherent") else {
        return;
    };
    let g = ThermalScattering::from_endf_file(&path, 30, 296.0, "C in graphite").unwrap();

    // (1) The evaluation, not the caller, chooses the channel.
    assert_eq!(g.elastic().kind_name(), "coherent elastic");
    let ThermalElastic::Coherent(table) = g.elastic() else {
        panic!("graphite MAT 30 must present a coherent-elastic channel");
    };

    // (3) Bragg cutoff = graphite (002), 1.8223 meV; σ_el exactly 0 below it.
    let cutoff = table.bragg_cutoff_ev();
    assert!(
        (cutoff - 1.822327e-3).abs() < 1.0e-8,
        "Bragg cutoff {cutoff:e} eV != graphite (002) 1.822327e-3 eV"
    );
    for e in [1.0e-4, 1.0e-3, 1.8e-3] {
        assert_eq!(
            g.elastic_xs(e),
            0.0,
            "σ_el must be exactly zero below the Bragg cutoff"
        );
    }
    assert!(
        g.elastic_xs(5.0e-3) > 4.0,
        "σ_el must open above the cutoff"
    );

    // (2) Exactness against the njoy surface across the whole thermal range.
    let ce =
        CoherentElasticScattering::from_endf_file(&path, 30, Temperature::new::<kelvin>(296.0))
            .unwrap();
    let mut worst: f64 = 0.0;
    let mut e = 1.0e-4;
    let mut n = 0usize;
    while e < 4.0 {
        let reference = ce
            .cross_section(NeutronEnergy::new::<electronvolt>(e))
            .get::<barn>();
        let ours = g.elastic_xs(e);
        worst = worst.max((reference - ours).abs() / reference.max(1.0e-30));
        n += 1;
        e *= 1.0005;
    }
    println!(
        "[graphite coherent] {} Bragg edges tabulated; {n} points scanned, \
         worst relative deviation {worst:.3e}",
        ce.bragg_edge_table().len()
    );
    assert!(
        worst < 1.0e-12,
        "stored Bragg table must reproduce the njoy sawtooth to round-off, got {worst:.3e}"
    );
}

/// Item 4: the elastic channel dominates bound thermal scattering in graphite.
#[test]
fn graphite_elastic_dominates_thermal_scattering() {
    let Some(path) = tape_or_skip("tsl-crystalline-graphite.endf", "graphite dominance") else {
        return;
    };
    let g = ThermalScattering::from_endf_file(&path, 30, 296.0, "C in graphite").unwrap();

    // Anchor: the value op-nhoa and op-1y4y were filed on, per carbon atom.
    let el = g.elastic_xs(0.0253);
    let inel = g.inelastic_xs(0.0253);
    assert!(
        (el - 4.5514).abs() < 5.0e-4,
        "σ_coh_el(0.0253 eV, 296 K) = {el:.4} b, expected 4.5514"
    );
    assert!(
        (inel - 0.4863).abs() < 5.0e-4,
        "σ_inel(0.0253 eV, 296 K) = {inel:.4} b, expected 0.4863"
    );
    assert!((g.total_xs(0.0253) - (el + inel)).abs() < 1.0e-12);

    let fraction = el / (el + inel);
    assert!(
        (0.90..0.91).contains(&fraction),
        "elastic fraction at 0.0253 eV = {fraction:.4}, expected ≈ 0.9035"
    );

    // σ_total is nearly flat over two decades — the signature of the sawtooth's
    // 1/E fall balancing σ_inel's rise to the free-atom limit.
    for e in [5.0e-3, 0.0253, 0.1, 1.0, 3.9] {
        let t = g.total_xs(e);
        assert!(
            (4.7..5.1).contains(&t),
            "σ_total({e} eV) = {t:.4} b outside the expected 4.7–5.1 b band"
        );
    }

    // Above the cutoff the S(α,β) treatment switches off entirely.
    assert_eq!(g.elastic_xs(4.0), 0.0);
    assert_eq!(g.total_xs(4.0), 0.0);
}

/// Item 5: `Nuclide::xs_at_energy` substitutes σ_inel + σ_el, not σ_inel alone.
#[test]
fn bound_graphite_nuclide_keeps_its_dominant_channel() {
    let Some(path) = tape_or_skip("tsl-crystalline-graphite.endf", "graphite nuclide") else {
        return;
    };
    // A second, identical handle kept outside the nuclide so the expected
    // values are read from the table itself rather than re-derived.
    let reference = ThermalScattering::from_endf_file(&path, 30, 296.0, "C in graphite").unwrap();
    let attached = ThermalScattering::from_endf_file(&path, 30, 296.0, "C in graphite").unwrap();

    let free = Nuclide::from_core("C0").expect("C-nat is in the embedded CORE library");
    let bound = Nuclide::from_core("C0")
        .unwrap()
        .with_thermal_scattering(attached);

    for &e in &[1.0e-3, 0.0253, 0.1] {
        let a = free.xs_at_energy(e, 296.0);
        let b = bound.xs_at_energy(e, 296.0);
        println!(
            "[graphite nuclide] E={e:.4e} free={:.4} bound={:.4} old(inel-only)={:.4}",
            a.elastic,
            b.elastic,
            reference.inelastic_xs(e)
        );
        // The substituted channel is the bound TOTAL (σ_inel + σ_el), and the
        // total XS is rebuilt consistently around it.
        assert!(
            (b.elastic - reference.total_xs(e)).abs() < 1.0e-12,
            "the substituted channel must be σ_inel + σ_el, not σ_inel alone"
        );
        assert!((b.total - (b.absorption + b.inelastic + b.n2n + b.elastic)).abs() < 1.0e-9);
        // Absorption is untouched by the thermal substitution.
        assert_eq!(a.absorption, b.absorption);
    }

    // At 0.0253 eV the fix is the whole point: inelastic-only gave graphite
    // ~10 % of its free-gas scattering; the bound total is slightly ABOVE it.
    let a = free.xs_at_energy(0.0253, 296.0);
    let b = bound.xs_at_energy(0.0253, 296.0);
    assert!(
        reference.inelastic_xs(0.0253) / a.elastic < 0.15,
        "sanity: the pre-fix inelastic-only substitution really was ~10 % of free gas"
    );
    assert!(
        (1.0..1.2).contains(&(b.elastic / a.elastic)),
        "bound/free elastic ratio at 0.0253 eV = {:.3}, expected ≈ 1.06",
        b.elastic / a.elastic
    );
}

/// Items 6 + 7: sampling conserves energy on the elastic branch, splits the two
/// channels in proportion to their cross sections, and reproduces the analytic
/// Bragg mean cosine.
#[test]
fn graphite_sampling_matches_the_cross_section_split_and_bragg_law() {
    use njoy_outram_park_fork::thermr::scattering::CoherentElasticScattering;
    use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
    use uom::si::{energy::electronvolt, thermodynamic_temperature::kelvin};

    let Some(path) = tape_or_skip("tsl-crystalline-graphite.endf", "graphite sampling") else {
        return;
    };
    let g = ThermalScattering::from_endf_file(&path, 30, 296.0, "C in graphite").unwrap();

    let e_in = 0.0253;
    let predicted_elastic_fraction = g.elastic_xs(e_in) / g.total_xs(e_in);

    // Analytic Bragg mean cosine, computed independently from the njoy surface.
    let ce =
        CoherentElasticScattering::from_endf_file(&path, 30, Temperature::new::<kelvin>(296.0))
            .unwrap();
    let reflections = ce.bragg_reflections(NeutronEnergy::new::<electronvolt>(e_in));
    let p_sum: f64 = reflections.iter().map(|&(_, p)| p).sum();
    assert!(
        (p_sum - 1.0).abs() < 1.0e-12,
        "reflection probabilities must sum to 1"
    );
    let analytic_mu: f64 = reflections.iter().map(|&(mu, p)| mu * p).sum();
    println!(
        "[graphite sampling] {} open Bragg reflections at {e_in} eV",
        reflections.len()
    );

    const N: usize = 200_000;
    let mut seed = 1u64;
    let (mut n_elastic, mut mu_sum, mut mu_sq_sum) = (0usize, 0.0f64, 0.0f64);
    for _ in 0..N {
        let (e_out, mu) = g
            .sample(e_in, &mut seed)
            .expect("thermal sampling below the cutoff");
        assert!((-1.0..=1.0).contains(&mu), "cosine {mu} out of range");
        assert!(e_out > 0.0, "outgoing energy must be positive");
        if e_out == e_in {
            n_elastic += 1;
            mu_sum += mu;
            mu_sq_sum += mu * mu;
        }
    }

    // (6) Channel split vs the cross-section prediction, at 5 binomial σ.
    let observed = n_elastic as f64 / N as f64;
    let sigma_binomial =
        (predicted_elastic_fraction * (1.0 - predicted_elastic_fraction) / N as f64).sqrt();
    println!(
        "[graphite sampling] elastic fraction observed {observed:.5} predicted \
         {predicted_elastic_fraction:.5} (1σ = {sigma_binomial:.5})"
    );
    assert!(
        (observed - predicted_elastic_fraction).abs() < 5.0 * sigma_binomial,
        "sampled elastic fraction {observed:.5} is more than 5σ from the \
         cross-section prediction {predicted_elastic_fraction:.5}"
    );

    // (7) Bragg mean cosine vs the analytic reflection-weighted mean, at 5 σ.
    let sampled_mu = mu_sum / n_elastic as f64;
    let var = (mu_sq_sum / n_elastic as f64 - sampled_mu * sampled_mu).max(0.0);
    let sem = (var / n_elastic as f64).sqrt();
    println!(
        "[graphite sampling] mean Bragg cosine sampled {sampled_mu:.5} analytic \
         {analytic_mu:.5} (SEM = {sem:.5})"
    );
    assert!(
        (sampled_mu - analytic_mu).abs() < 5.0 * sem,
        "sampled mean Bragg cosine {sampled_mu:.5} is more than 5σ from the \
         analytic {analytic_mu:.5}"
    );
}

/// Item 8: the HTR-10 benchmark temperatures — interpolated, never snapped, and
/// Debye-Waller-suppressed with rising T.
#[test]
fn graphite_htr10_temperature_points() {
    let Some(path) = tape_or_skip("tsl-crystalline-graphite.endf", "graphite temperatures") else {
        return;
    };
    // (requested K, expected resolved K, expected σ_el, expected σ_inel) at 0.0253 eV.
    let cases = [
        (293.15, 296.00, 4.5514, 0.4863),
        (393.15, 393.15, 4.3849, 0.6779),
        (523.15, 523.15, 4.1672, 0.9445),
        (1073.15, 1073.15, 3.3592, 2.0373),
    ];
    let mut last_el = f64::INFINITY;
    let mut last_inel = 0.0f64;
    for (requested, resolved, el, inel) in cases {
        let g = ThermalScattering::from_endf_file(&path, 30, requested, "C in graphite").unwrap();
        assert!(
            (g.selected_temperature_k() - resolved).abs() < 1.0e-6,
            "T = {requested} K resolved to {} K, expected {resolved} K",
            g.selected_temperature_k()
        );
        let (got_el, got_inel) = (g.elastic_xs(0.0253), g.inelastic_xs(0.0253));
        assert!(
            (got_el - el).abs() < 5.0e-4,
            "σ_el({requested} K) = {got_el:.4}, expected {el}"
        );
        assert!(
            (got_inel - inel).abs() < 5.0e-4,
            "σ_inel({requested} K) = {got_inel:.4}, expected {inel}"
        );
        // Debye-Waller: elastic falls, inelastic rises, monotonically in T.
        assert!(got_el < last_el, "σ_el must decrease with temperature");
        assert!(
            got_inel > last_inel,
            "σ_inel must increase with temperature"
        );
        last_el = got_el;
        last_inel = got_inel;
    }

    // Outside the tabulated 296–2000 K range: a hard refusal, never a snap.
    assert!(
        ThermalScattering::from_endf_file(&path, 30, 100.0, "C in graphite").is_err(),
        "100 K is below the graphite tabulation and must be refused"
    );
    assert!(
        ThermalScattering::from_endf_file(&path, 30, 2200.0, "C in graphite").is_err(),
        "2200 K is above the graphite tabulation and must be refused"
    );
}

/// Item 9: light water is untouched — the regression gate for the existing
/// thermal pincell case.
#[test]
fn light_water_has_no_elastic_channel_and_is_unchanged() {
    let Some(path) = tape_or_skip("tsl-HinH2O.endf", "H2O regression") else {
        return;
    };
    let w = ThermalScattering::from_endf_file(&path, 1, 293.6, "H in H2O").unwrap();
    assert_eq!(w.elastic().kind_name(), "none");
    assert!(matches!(w.elastic(), ThermalElastic::None));

    for e in [1.0e-4, 1.0e-3, 0.0253, 0.1, 1.0, 3.9] {
        assert_eq!(
            w.elastic_xs(e),
            0.0,
            "water has no thermal elastic at any energy"
        );
        // Bit-identical to the pre-change behaviour: total == inelastic.
        assert_eq!(w.total_xs(e), w.inelastic_xs(e));
    }
    let anchor = w.inelastic_xs(0.0253);
    assert!(
        (anchor - 52.1420).abs() < 5.0e-4,
        "σ_inel(0.0253 eV, 293.6 K) for H in H2O = {anchor:.4} b, expected 52.1420"
    );

    // Sampling must never return E_out == E_in from a channel that does not
    // exist; every draw comes from the inelastic emission tables.
    let mut seed = 7u64;
    for _ in 0..2000 {
        let (e_out, mu) = w.sample(0.0253, &mut seed).unwrap();
        assert!((-1.0..=1.0).contains(&mu));
        assert!(e_out > 0.0);
    }
}

/// Item 10: the incoherent-elastic law (ZrH) — structural verification only.
#[test]
fn zrh_incoherent_elastic_channel_is_forward_peaked() {
    let Some(path) = tape_or_skip("tsl-HinZrH.endf", "ZrH incoherent elastic") else {
        return;
    };
    let z = ThermalScattering::from_endf_file(&path, 7, 296.0, "H in ZrH").unwrap();
    assert_eq!(z.elastic().kind_name(), "incoherent elastic");
    assert!(matches!(z.elastic(), ThermalElastic::Incoherent(_)));

    use njoy_outram_park_fork::thermr::scattering::IncoherentElasticScattering;
    use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
    use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

    let el = z.elastic_xs(0.0253);
    let inel = z.inelastic_xs(0.0253);
    println!("[ZrH] σ_el = {el:.4} b/H, σ_inel = {inel:.4} b/H at 0.0253 eV");
    assert!(
        (el - 55.0151).abs() < 5.0e-3,
        "σ_inc_el(0.0253 eV, 296 K) = {el:.4} b, expected 55.0151"
    );
    assert!(
        (inel - 2.8804).abs() < 5.0e-3,
        "σ_inel(0.0253 eV, 296 K) = {inel:.4} b, expected 2.8804"
    );
    assert!(
        el > inel,
        "incoherent elastic dominates H in ZrH at thermal energies"
    );

    // The pre-tabulated σ must track the njoy surface. Unlike the coherent
    // sawtooth this channel IS resampled onto a 200-point log grid, so the
    // gate is a grid-interpolation tolerance, not round-off.
    let ie =
        IncoherentElasticScattering::from_endf_file(&path, 7, Temperature::new::<kelvin>(296.0))
            .unwrap();
    let mut worst: f64 = 0.0;
    let mut e = 1.0e-4;
    while e < 4.0 {
        let reference = ie
            .cross_section(NeutronEnergy::new::<electronvolt>(e))
            .get::<barn>();
        worst = worst.max((reference - z.elastic_xs(e)).abs() / reference.max(1.0e-30));
        e *= 1.0005;
    }
    println!("[ZrH] worst relative σ deviation vs the njoy surface: {worst:.3e}");
    assert!(
        worst < 5.0e-4,
        "log-grid resample error {worst:.3e} exceeds 0.05 %"
    );

    // Reference mean cosine: the mean of the njoy surface's equally-probable
    // cosines at this energy — computed here, not hard-coded, so the assert
    // cannot drift from the underlying angular law.
    let mus = ie.equiprobable_cosines(NeutronEnergy::new::<electronvolt>(0.0253), 16);
    let reference_mu: f64 = mus.iter().sum::<f64>() / mus.len() as f64;

    // Elastic scatters keep their energy and are forward-peaked.
    let mut seed = 3u64;
    let (mut n_el, mut mu_sum, mut mu_sq_sum) = (0usize, 0.0f64, 0.0f64);
    for _ in 0..50_000 {
        let (e_out, mu) = z.sample(0.0253, &mut seed).unwrap();
        assert!((-1.0..=1.0).contains(&mu));
        if e_out == 0.0253 {
            n_el += 1;
            mu_sum += mu;
            mu_sq_sum += mu * mu;
        }
    }
    let mean_mu = mu_sum / n_el as f64;
    let var = (mu_sq_sum / n_el as f64 - mean_mu * mean_mu).max(0.0);
    let sem = (var / n_el as f64).sqrt();
    println!(
        "[ZrH] {n_el} elastic draws, mean cosine {mean_mu:.5} vs reference \
         {reference_mu:.5} (SEM = {sem:.5})"
    );
    assert!(
        mean_mu > 0.0,
        "the incoherent-elastic law p(μ) ∝ exp(−2EW'(1−μ)) is forward-peaked, \
         so μ̄ must be positive; got {mean_mu:.5}"
    );
    assert!(
        (mean_mu - reference_mu).abs() < 5.0 * sem,
        "sampled mean cosine {mean_mu:.5} is more than 5σ from the njoy \
         equiprobable-cosine mean {reference_mu:.5}"
    );
}
