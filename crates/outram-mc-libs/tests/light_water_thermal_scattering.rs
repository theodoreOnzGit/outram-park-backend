//! V&V: light-water S(α,β) thermal scattering, njoy LEAPR → `outram-mc-libs`.
//!
//! # What this validates
//!
//! The **whole** light-water bound-scattering path, end to end, with no external
//! data file anywhere in it:
//!
//! ```text
//!   tsl-HinH2O.leapr (12 kB, compiled into njoy)
//!     -> leapr::generate  (contin -> trans -> discre -> endout)
//!     -> ENDF MF=7 tape (in the njoy cache)
//!     -> thermr::scattering::IncoherentInelasticScattering
//!     -> ThermalScattering::from_leapr        <- this crate
//!     -> Nuclide::with_thermal_scattering     <- the transport hot loop
//! ```
//!
//! Every other thermal-scattering test in this crate needs a multi-megabyte
//! `tsl-*.endf` at a hardcoded absolute path and **skips** when it is absent,
//! which is always in CI and in a fresh container. This one runs anywhere, so
//! the moderator physics an LWR depends on is actually covered rather than
//! nominally covered.
//!
//! ## Data / provenance
//!
//! - **Deck:** `njoy-outram-park-fork/src/leapr/decks/tsl-HinH2O.leapr` — the
//!   ENDF/B-VIII.0 LEAPR job for H in light water, `MAT = 1`. Open-source
//!   (ENDF/B-VIII.0, 2018), per `DATA_POLICY.md`.
//! - **Reference values.** The bound thermal scattering cross section of light
//!   water at 2200 m/s (0.0253 eV) is ≈ **103 b/molecule** (public literature
//!   value; dominated by the two bound H). The free-atom limit is
//!   σ_free = B(1)/natom = **20.436 b/H**. The generation side is compared
//!   against the published evaluation in the njoy crate's
//!   `tests/leapr_h2o_secondary_scatterer.rs`; this test checks what the MC
//!   consumer surface does with it.
//!
//! # Methodology and measured results (2026-08-14, 293.6 K)
//!
//! | Check | Pass criterion | Measured |
//! |---|---|---|
//! | Selected temperature | 293.6 K | **293.6 K** |
//! | Elastic channel | `None` (a liquid has no lattice) | **`None`** |
//! | σ_s(0.0253 eV) | 90–115 b/molecule | **104.889 b/molecule** (52.444 b/H) |
//! | Free-atom limit near cutoff | within 5 % of 20.436 b/H | **21.087 b/H (+3.19 %)** |
//! | Monotone decrease 0.1 → 1 → 3.9 eV | strictly decreasing | **33.044 → 21.950 → 21.087 b** |
//! | Bound rise at low E | σ(1 meV) > 2 × σ_free | **117.968 b/H (5.77 ×)** |
//! | Up-scatter from 1 meV | > 50 % of samples gain energy | **78.3 %** (15 653 / 20 000) |
//! | Mean E' from 1 meV | between E_in and ~kT | **0.0108 eV** (E_in = 0.001, kT = 0.0253) |
//!
//! # Interpretation
//!
//! The thermal value lands **+1.8 %** against the 103 b/molecule literature
//! figure, and σ(E) relaxes onto the free-atom limit by the 4 eV cutoff, so the
//! join to the free-gas treatment above the cutoff is smooth. The up-scatter
//! result is the one that matters for reactor physics and the one a weaker test
//! would miss: a neutron entering at 1 meV — far below the 293.6 K Maxwellian —
//! must mostly *gain* energy, and its mean outgoing energy must sit between the
//! incident energy and roughly `kT`. That is the mechanism which establishes the
//! thermal spectrum in a moderator, and it exercises the S(α,β) kernel in both
//! β directions rather than just reading a cross section off a curve.
//!
//! **Validation standing.** The underlying law is *regenerated*, not the
//! published tape, and light water has **not** been compared point-by-point
//! against a reference tape — see `SabRequest::validation` and bead `op-rvrg`.
//! Treat these as verification of the wiring and of limiting physics, not as a
//! validated water library.

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use outram_mc_libs::material::thermal::{ThermalElastic, ThermalScattering};

/// Moderator temperature for every check below \[K\].
const T_K: f64 = 293.6;
/// Free-atom cross section per H, `B(1)/natom` \[barn\].
const SIGMA_FREE_PER_H: f64 = 20.436;
/// Hydrogen atoms per H₂O molecule.
const H_PER_MOLECULE: f64 = 2.0;

/// Build the light-water thermal table straight from the LEAPR deck.
///
/// This is the call under test: no path, no `mat` number to get wrong, no file
/// that has to exist.
fn light_water() -> ThermalScattering {
    ThermalScattering::from_leapr(SabMaterial::HInH2O, T_K, "H in H2O")
        .expect("regenerate and bake H-in-H2O thermal tables")
}

/// The table is built at the requested temperature and carries no elastic
/// channel.
///
/// **Pass criterion:** selected temperature within 0.1 K of 293.6 K, and
/// [`ThermalElastic::None`] — light water is a liquid, so it has neither a
/// lattice to diffract from (coherent elastic) nor a tabulated bound-proton
/// incoherent-elastic law. Getting a spurious elastic channel here would inflate
/// the moderator cross section.
///
/// **Result (2026-08-14):** 293.6 K, `None`.
#[test]
fn builds_at_temperature_with_no_elastic_channel() {
    let th = light_water();
    let sel = th.selected_temperature_k();
    assert!(
        (sel - T_K).abs() < 0.1,
        "generated at the requested temperature, got {sel} K"
    );
    assert!(
        matches!(th.elastic(), ThermalElastic::None),
        "light water has no MF=7/MT=2 elastic law, got {}",
        th.elastic().kind_name()
    );
    // With no elastic channel the total must be exactly the inelastic part.
    let (a, b) = (th.total_xs(0.0253), th.inelastic_xs(0.0253));
    assert!(
        (a - b).abs() < 1e-12,
        "total {a} must equal inelastic {b} when there is no elastic channel"
    );
}

/// The bound thermal cross section matches the literature value for light water.
///
/// **Methodology.** Evaluate σ_inel at 0.0253 eV (2200 m/s) per H and scale by
/// the two H per molecule. Compare against the ≈ 103 b/molecule bound
/// scattering cross section of light water.
///
/// **Pass criterion:** 90–115 b/molecule. The band is wide because the
/// literature figure is itself quoted loosely and the underlying law is
/// regenerated rather than the published tape.
///
/// **Result (2026-08-14):** 52.444 b/H → **104.889 b/molecule**, +1.8 % against
/// 103 b. Note this is ~5.1 × the free-atom 20.436 b/H — chemical binding, which
/// is the entire reason this treatment exists.
#[test]
fn thermal_cross_section_matches_bound_light_water() {
    let th = light_water();
    let per_h = th.inelastic_xs(0.0253);
    let per_molecule = H_PER_MOLECULE * per_h;
    assert!(
        (90.0..=115.0).contains(&per_molecule),
        "sigma_s(H2O, 0.0253 eV) = {per_molecule} b/molecule (per-H {per_h}); expect ~103 b"
    );
    assert!(
        per_h > 2.0 * SIGMA_FREE_PER_H,
        "the bound cross section must far exceed the free-atom {SIGMA_FREE_PER_H} b, got {per_h}"
    );
}

/// σ(E) relaxes onto the free-atom limit by the thermal cutoff, so the join to
/// the free-gas treatment above it is smooth.
///
/// **Methodology.** Evaluate σ_inel across 0.1 → 1 → 3.9 eV (the top of the grid
/// sits at the 4 eV cutoff) and compare the highest point against
/// σ_free = 20.436 b/H.
///
/// **Pass criterion:** strictly decreasing, and within 5 % of σ_free just below
/// the cutoff. A discontinuity here would show up in a pin-cell spectrum as an
/// artefact at 4 eV.
///
/// **Result (2026-08-14):** 33.044 → 21.950 → 21.087 b/H; the 3.9 eV value is
/// **+3.19 %** of σ_free.
#[test]
fn approaches_the_free_atom_limit_at_the_cutoff() {
    let th = light_water();
    assert!(
        (th.cutoff_ev() - 4.0).abs() < 1e-9,
        "the default thermal cutoff is 4 eV, got {}",
        th.cutoff_ev()
    );

    let (s_lo, s_mid, s_hi) = (
        th.inelastic_xs(0.1),
        th.inelastic_xs(1.0),
        th.inelastic_xs(3.9),
    );
    assert!(
        s_lo > s_mid && s_mid > s_hi,
        "sigma must decrease toward the free-atom limit: {s_lo} {s_mid} {s_hi}"
    );
    let rel = (s_hi - SIGMA_FREE_PER_H).abs() / SIGMA_FREE_PER_H;
    assert!(
        rel < 0.05,
        "sigma(3.9 eV) = {s_hi} b within 5 % of sigma_free = {SIGMA_FREE_PER_H} b \
         (got {:.2} %)",
        rel * 100.0
    );

    // And the bound rise at the bottom of the grid — the other end of the curve.
    let s_1mev = th.inelastic_xs(0.001);
    assert!(
        s_1mev > 2.0 * SIGMA_FREE_PER_H,
        "sigma(1 meV) = {s_1mev} b must show the bound-atom rise above \
         {SIGMA_FREE_PER_H} b"
    );
}

/// A cold neutron up-scatters off the thermal motion of the molecule.
///
/// This is the physics that makes a moderator a moderator, and it is what the
/// free-gas treatment gets wrong. It also exercises the S(α,β) kernel in the
/// negative-β (energy-gain) direction, which a cross-section-only check never
/// touches.
///
/// **Methodology.** Sample 20 000 scatters at E = 0.001 eV — well below the
/// 293.6 K Maxwellian mean of ≈ 0.038 eV — and count how many leave with
/// `E' > E`. Also check every sampled cosine is a valid direction and that the
/// mean outgoing energy is bounded by roughly `kT`.
///
/// **Pass criterion:** every sample returns a value (the energy is below the
/// cutoff, so the bound law must apply); > 50 % gain energy; every μ in
/// \[−1, 1\]; mean E' strictly above E_in and below 0.0253 eV.
///
/// **Result (2026-08-14):** 20 000 / 20 000 sampled; **78.3 %** up-scatter
/// (15 653); mean E' = **0.010786 eV** against E_in = 0.001 eV.
#[test]
fn cold_neutrons_upscatter_toward_the_maxwellian() {
    let th = light_water();
    let e_in = 0.001;
    let kt_ev = 8.617_333_262e-5 * T_K;

    let mut seed = 0x5EED_1234_u64;
    let (mut sampled, mut upscattered) = (0usize, 0usize);
    let mut e_sum = 0.0;
    for _ in 0..20_000 {
        let (e_out, mu) = th
            .sample(e_in, &mut seed)
            .expect("E below the cutoff must always sample the bound law");
        sampled += 1;
        e_sum += e_out;
        assert!(
            (-1.0..=1.0).contains(&mu),
            "cosine {mu} outside [-1, 1] at E' = {e_out}"
        );
        assert!(e_out > 0.0, "outgoing energy {e_out} must be positive");
        if e_out > e_in {
            upscattered += 1;
        }
    }

    let frac = upscattered as f64 / sampled as f64;
    assert!(
        frac > 0.5,
        "a 1 meV neutron in a 293.6 K moderator must mostly gain energy, got {:.1} %",
        frac * 100.0
    );
    let mean = e_sum / sampled as f64;
    assert!(
        mean > e_in && mean < kt_ev,
        "mean outgoing energy {mean} eV must lie between E_in = {e_in} and kT = {kt_ev}"
    );
}

/// The table attaches to a nuclide and takes over below the cutoff.
///
/// **Methodology.** Attach the water table to an H-1 nuclide built from the
/// crate's embedded CORE data with [`Nuclide::with_thermal_scattering`], and
/// compare the elastic cross section reported below and above the cutoff against
/// the same nuclide without it. This is the seam the transport loop crosses.
///
/// **Pass criterion:** below the cutoff the bound value replaces the free-gas
/// one and is at least 1.5 × larger; above the cutoff the two agree exactly, and
/// `sample_thermal` declines there while applying below.
///
/// **Result (2026-08-14):** at 0.0253 eV the bound path gives **52.444 b**
/// against the free-gas nuclide's **30.082 b** (1.74 ×); at 10 eV, above the
/// 4 eV cutoff, both give **20.461 b** — identical to machine precision.
///
/// The free-gas 30.082 b at 0.0253 eV is H-1's own Doppler-broadened elastic
/// cross section, not the 20.436 b free-atom asymptote — which is why the
/// criterion is 1.5 × rather than the 2 × used against σ_free elsewhere.
#[test]
fn attaches_to_a_nuclide_and_overrides_below_the_cutoff() {
    use outram_mc_libs::material::nuclide::Nuclide;

    let free_gas = Nuclide::from_core("H1").expect("H1 is in the embedded CORE data");
    let bound = Nuclide::from_core("H1")
        .expect("H1 is in the embedded CORE data")
        .with_thermal_scattering(light_water());

    let below = 0.0253;
    let (xs_free, xs_bound) = (
        free_gas.xs_at_energy(below, T_K).elastic,
        bound.xs_at_energy(below, T_K).elastic,
    );
    assert!(
        xs_bound > 1.5 * xs_free,
        "below the cutoff the bound law must replace free gas: {xs_bound} vs {xs_free}"
    );

    let above = 10.0;
    let (xs_free_hi, xs_bound_hi) = (
        free_gas.xs_at_energy(above, T_K).elastic,
        bound.xs_at_energy(above, T_K).elastic,
    );
    assert!(
        (xs_free_hi - xs_bound_hi).abs() < 1e-12,
        "above the cutoff the thermal table must not apply: {xs_bound_hi} vs {xs_free_hi}"
    );
    assert!(
        bound.sample_thermal(above, &mut 1u64).is_none(),
        "sample_thermal must decline above the cutoff"
    );
    assert!(
        bound.sample_thermal(below, &mut 1u64).is_some(),
        "sample_thermal must apply below the cutoff"
    );
}
