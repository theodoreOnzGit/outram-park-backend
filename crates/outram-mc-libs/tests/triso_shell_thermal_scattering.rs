//! V&V: bound-atom S(alpha, beta) thermal scattering for the **TRISO shell
//! stack** — the carbon in the PyC coatings and the graphite matrix, and both
//! species in the SiC layer (kopi-beans `op-6tz.35.1`).
//!
//! # Summary of what this establishes
//!
//! - **The PyC coatings and the graphite matrix are DONE.** The carbon law
//!   regenerates from the embedded deck, binds to the nuclide, and is
//!   elastic-dominated as it must be. Ready to use.
//! - **The SiC layer is USABLE, with a stated bias.** Its coherent-elastic
//!   channel used to be unobtainable from the embedded decks — the law had
//!   ~2.7 % of the layer's true thermal scattering, a data-generation gap.
//!   That gap **closed on 2026-08-19** when the generalized coherent-elastic
//!   formulation landed in `njoy-outram-park-fork::leapr::coher::general`
//!   (bead `op-jw4a`, GitHub issue #24). Both SiC laws now carry a real
//!   MF=7/MT=2, measuring −2.96 % against the ENDF/B-VIII.0 oracle tape at
//!   293.6 K.
//!   **That residual is characterised, not validated.** GitHub issue #28
//!   traces it to the published evaluation rather than to this port: the
//!   tape's MF=7/MT=2 matches, edge by edge, an atomic basis that is not a
//!   valid crystallographic centring, while this port uses the true
//!   zinc-blende structure of 3C-SiC. See
//!   `njoy-outram-park-fork/docs/leapr-sic-coherent-elastic-vv.md`. Whether a
//!   ~3 % elastic bias is acceptable for a shell-resolved TRISO transport case
//!   is a judgement call for whoever picks that up — but it can now be an
//!   informed one. Do **not** describe the SiC layer as validated, and do not
//!   substitute the graphite law for it.
//!
//! # How the bead's premise changed
//!
//! `op-6tz.35.1` was filed on 2026-08-12 saying `geometry/triso_particle.rs`
//! "assembles the 5-region TRISO CSG but assigns plain material ids", blocked
//! because it "needs thermal-scattering data not present on this host" and
//! depended on a download path (`op-6tz.28`).
//!
//! **The data-availability half of that premise no longer holds.** As of
//! 2026-08-14 all 33 ENDF/B-VIII.0 LEAPR decks are embedded in
//! `njoy-outram-park-fork` (`leapr::decks::SabMaterial`), so the decks
//! regenerate offline with no download and no host-specific path:
//!
//! | TRISO region | Scatterer | Deck | MAT | Usable? |
//! |---|---|---|---|---|
//! | Buffer / IPyC / OPyC / matrix | C in graphite | `tsl-crystalline-graphite` | 30 | **Yes** |
//! | SiC layer | C in SiC | `tsl-CinSiC` | 44 | **Yes, −2.96 % elastic bias** |
//! | SiC layer | Si in SiC | `tsl-SiinSiC` | 43 | **Yes, −2.96 % elastic bias** |
//!
//! Both SiC materials share **one** MT=2 section — coherent elastic is a Bragg
//! property of the 3C-SiC compound lattice, not of either sublattice. A caller
//! that puts both laws in one SiC region must count MT=2 **once** or it
//! double-counts Bragg scattering.
//!
//! # What "wiring it in" means here (design note)
//!
//! It is **not** a change to `triso_particle.rs`. That module assigns opaque
//! material *indices* by design — its doc comment is explicit that "the
//! material ids are opaque indices the caller maps to real materials". A
//! thermal-scattering law binds to a [`Nuclide`], not to a material index, via
//! `Nuclide::with_thermal_scattering`. So the wiring is **caller-side
//! composition**: build the shell nuclides with the right law attached, then
//! hand the resulting materials to the geometry builder. Putting deck-loading
//! inside the geometry module would drag ENDF parsing into a CSG module and
//! break the crate's data/transport boundary (`outram-mc-libs` parses no
//! ENDF — it consumes the njoy surface). This test is therefore the
//! deliverable: an executable, checked demonstration of the composition.
//!
//! # Methodology
//!
//! For each law: regenerate the MF=7 law at 293.6 K from the embedded deck via
//! `leapr::generate::generate_tape` with `ElasticChannel::Generate`, build the
//! [`ThermalScattering`] consumer, attach it to the matching CORE nuclide, and
//! compare the microscopic elastic cross section at 0.0253 eV with and without
//! it — the same before/after comparison `tests/thermal_graphite_elastic.rs`
//! uses at nuclide level.
//!
//! Nuclide naming: carbon is **`"C0"`** — natural carbon, ENDF/B-VII.1 ZA
//! 006000, which is how the embedded CORE windowed-multipole library names
//! elemental carbon (there is no `"C12"` entry; see
//! `njoy-outram-park-fork/docs/wmp-nuclide-manifest.md`). Silicon is `"Si28"`.
//!
//! # Results (measured 2026-08-14, this environment, release mode)
//!
//! Channel decomposition at 0.0253 eV, barns per principal atom:
//!
//! | Law | Elastic | Inelastic | Total |
//! |---|---|---|---|
//! | C in graphite | 4.55555 | 0.48162 | 5.03717 |
//! | C in SiC | **0.00000** | 0.13291 | 0.13291 |
//! | Si in SiC | **0.00000** | 0.06367 | 0.06367 |
//!
//! Against the free-gas cross section each law replaces:
//!
//! | Law | Free-gas | Bound | Change |
//! |---|---|---|---|
//! | C in graphite | 4.9382 b | 5.0372 b | **+2.01%** |
//! | C in SiC | 4.9382 b | 0.1329 b | **-97.31%** |
//! | Si in SiC | 1.9914 b | 0.0637 b | **-96.80%** |
//!
//! The graphite figure reproduces the 5.0378 b recorded independently under
//! `op-nhoa` against the official ENDF/B-VIII.0 tape, so the regeneration path
//! agrees with the tape path.
//!
//! ## Why the SiC numbers are a gap and not a result
//!
//! A bound crystalline solid scattering **97% less** than the free gas it
//! replaces is not physical — it is the exact signature `op-nhoa` named: an
//! inelastic-only law with the dominant elastic channel missing. Root cause,
//! confirmed from the decks themselves rather than inferred:
//!
//! - **Card 4 `iel` (the built-in-lattice selector) is `1` for
//!   `tsl-crystalline-graphite` and `0` for the SiC decks.** `iel = 0` means
//!   stock LEAPR generates no coherent-elastic channel.
//! - The SiC decks' own comment cards say the coherent elastic was produced by
//!   **modified LEAPR source**, citing Y. Zhu and A. Hawari, "Implementation of
//!   a Generalized Coherent Elastic Scattering Formulation for Thermal Neutron
//!   Scattering". Stock NJOY LEAPR carries built-in lattices only for graphite,
//!   Be and BeO; a general crystal structure factor is exactly the modification
//!   that paper adds.
//!
//! So this is **not a defect in this workspace's LEAPR port** — the port
//! faithfully reproduces stock LEAPR, which cannot make this data. It is a
//! genuine capability gap, and the SiC layer's thermal treatment is blocked on
//! it. Do **not** substitute the graphite law for the SiC layer: carbon bound
//! in silicon carbide sits in a different lattice, and the substitution would
//! produce a plausible wrong number rather than a failure.
//!
//! The two SiC tests below therefore assert the **gap**, so that whenever it is
//! closed they fail and force this documentation to be updated, rather than
//! silently continuing to pass on inelastic-only data.
//!
//! # What this does NOT establish
//!
//! No transport-level TRISO result and no benchmark comparison. The
//! doubly-heterogeneous k-eigenvalue demonstration for graphite lives in
//! `tests/htr10_graphite_thermal_scattering_pebble_bed.rs`; a shell-resolved
//! TRISO transport case is `op-6tz.35`. The reference throughout is this
//! workspace's own njoy port, so these are internal-consistency gates, not an
//! independent NJOY/OpenMC-ACE oracle. AI-assisted draft, no human review.

use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::units::Temperature;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use uom::si::thermodynamic_temperature::kelvin;

const TEMPERATURE_K: f64 = 293.6;
/// The conventional thermal reference energy \[eV\].
const E_THERMAL: f64 = 0.0253;

/// Coherent-elastic cross section of 3C-SiC at [`E_THERMAL`] \[barn per
/// principal atom\] read from the ENDF/B-VIII.0 oracle tapes
/// `reference-data/endf/tsl-SiinSiC.endf` and `tsl-CinSiC.endf`, whose
/// MF=7/MT=2 sections are byte-identical apart from the header.
const SIC_ELASTIC_ORACLE_BARN: f64 = 2.94078;

/// Pass band for the SiC elastic channel against
/// [`SIC_ELASTIC_ORACLE_BARN`].
///
/// **This brackets a known, characterised bias, not a validation.** The
/// measured residual is −3.03 %, and GitHub issue #28 traces it to the
/// published evaluation rather than to this port: edge-by-edge, the tape's
/// MF=7/MT=2 matches an atomic basis that is not a valid crystallographic
/// centring, while this port uses the true zinc-blende structure of 3C-SiC
/// (see `njoy-outram-park-fork/docs/leapr-sic-coherent-elastic-vv.md`). The
/// residual is therefore not expected to close, and tightening this band would
/// amount to reproducing the evaluation's own error.
const SIC_ELASTIC_BAND: f64 = 0.05;

/// Regenerate one embedded LEAPR deck's MF=7 law and build the `outram-mc-libs`
/// consumer from it.
///
/// Bridges the two crates via a temp-file round trip, as
/// `tests/htr10_graphite_thermal_scattering_pebble_bed.rs` does. The filename
/// carries a per-call atomic counter as well as the pid: cargo runs the tests
/// in this file as threads of ONE process, so a pid-only name would let
/// concurrent callers race on the same path (observed there as
/// `EndfParse("unexpected end of section data")`).
fn regenerated_law(material: SabMaterial, label: &str) -> ThermalScattering {
    let located = locate_deck(material).unwrap_or_else(|e| {
        panic!("deck for {label} is embedded in njoy-outram-park-fork (2026-08-14): {e:?}")
    });
    let deck = LeaprDeck::parse(&located.text).expect("embedded deck parses");
    let temperature = Temperature::new::<kelvin>(TEMPERATURE_K);
    let tape = generate_tape(&deck, temperature, ElasticChannel::Generate)
        .unwrap_or_else(|e| panic!("{label} regenerates from its embedded deck: {e:?}"));

    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!(
        "op_triso_sab_{label}_{}_{seq}.endf",
        std::process::id()
    ));
    {
        let file = std::fs::File::create(&tmp).expect("create temp file for regenerated tape");
        tape.write(file).expect("write the regenerated MF=7 tape");
    }
    let result = ThermalScattering::from_endf_file(
        tmp.to_str().expect("temp path is valid UTF-8"),
        material.mat(),
        TEMPERATURE_K,
        label,
    )
    .unwrap_or_else(|e| panic!("ThermalScattering builds for {label}: {e:?}"));
    let _ = std::fs::remove_file(&tmp);
    result
}

/// The free-gas and bound elastic cross sections \[barn\] at 0.0253 eV for one
/// nuclide, with and without the law attached.
fn free_and_bound(law: ThermalScattering, nuclide_name: &str) -> (f64, f64) {
    let free = Nuclide::from_core(nuclide_name)
        .unwrap_or_else(|e| panic!("{nuclide_name} is in the embedded CORE library: {e:?}"));
    let bound = Nuclide::from_core(nuclide_name)
        .expect("second construction succeeds")
        .with_thermal_scattering(law);
    (
        free.xs_at_energy(E_THERMAL, TEMPERATURE_K).elastic,
        bound.xs_at_energy(E_THERMAL, TEMPERATURE_K).elastic,
    )
}

/// LIVE: carbon in **graphite** — the law the buffer, IPyC and OPyC coatings
/// and the surrounding matrix/pebble binder all bind.
///
/// This is the usable half of `op-6tz.35.1`. Asserts the physics is right, not
/// merely that something changed: the elastic (Bragg) channel must dominate at
/// 0.0253 eV, and the bound cross section must exceed the free gas it replaces.
#[test]
fn triso_carbon_coatings_bind_graphite_sab() {
    let law = regenerated_law(SabMaterial::CrystallineGraphite, "C-in-graphite");
    let cutoff = law.cutoff_ev();
    let (el, inel) = (law.elastic_xs(E_THERMAL), law.inelastic_xs(E_THERMAL));

    assert!(
        cutoff > E_THERMAL,
        "cutoff {cutoff} eV must exceed 0.0253 eV"
    );
    assert!(
        el > inel,
        "coherent-elastic must dominate graphite at 0.0253 eV (measured elsewhere ~4.55 b vs \
         ~0.49 b), got elastic {el} vs inelastic {inel}"
    );
    assert_eq!(law.total_xs(cutoff), 0.0, "law must vanish at its cutoff");

    let (free, bound) = free_and_bound(law, "C0");
    assert!(
        bound > free,
        "bound graphite should scatter MORE than the free gas it replaces (elastic channel is \
         ~90% of the total), got free {free:.4} b vs bound {bound:.4} b"
    );
    eprintln!(
        "[op-6tz.35.1] buffer/IPyC/OPyC/matrix, C in graphite @ {E_THERMAL} eV: \
         elastic {el:.5} b + inelastic {inel:.5} b; free-gas {free:.4} b -> bound {bound:.4} b \
         ({:+.2} %)",
        100.0 * (bound - free) / free
    );
}

/// LIVE: carbon in **silicon carbide** binds a usable S(alpha,beta) law with a
/// real coherent-elastic channel.
///
/// # History
///
/// This used to be a GAP assertion — `iel = 0` on card 4 of `tsl-CinSiC.leapr`
/// meant stock LEAPR, and therefore this port, produced no MF=7/MT=2 at all,
/// so the SiC layer was unusable. The deck's own README says its
/// coherent-elastic section came from an in-house routine implementing Zhu and
/// Hawari's generalized coherent-elastic formulation. That formulation landed
/// in `leapr::coher::general` on 2026-08-19 (bead `op-jw4a`, GitHub issue
/// #24), which closed the gap.
///
/// # Methodology
///
/// Regenerate the embedded deck, take the elastic and inelastic cross sections
/// at 0.0253 eV, and compare the elastic channel against the ENDF/B-VIII.0
/// oracle tape (`reference-data/endf/tsl-CinSiC.endf`, elastic 2.94078 b).
/// The pass band is **±5 %**, which is the tolerance the root-cause analysis
/// in GitHub issue #28 justifies rather than a round number: the residual is
/// −3.03 %, it is *not* an error in this port, and it is not expected to
/// close. `njoy-outram-park-fork`'s
/// `tests/leapr_sic_coherent_elastic_oracle.rs` shows edge-by-edge that the
/// published tape's MF=7/MT=2 matches an atomic basis which is not a valid
/// crystallographic centring, while this port uses the true zinc-blende
/// structure of 3C-SiC; see `docs/leapr-sic-coherent-elastic-vv.md` in that
/// crate. So the band brackets a **known, characterised bias against the
/// evaluation**, and tightening it would amount to reproducing the
/// evaluation's own error.
///
/// # Results (measured 2026-08-21, release mode)
///
/// At this file's 293.6 K: elastic 2.85382 b (−2.96 % vs the 2.94078 b
/// oracle), inelastic 0.13291 b, bound total 2.9867 b against 4.9382 b of
/// free-gas carbon (ratio 0.605; the oracle implies 0.622). The elastic value
/// is identical to the silicon side to within 1e-9, as it must be — coherent
/// elastic is a property of the 3C-SiC compound lattice, not of either
/// sublattice.
///
/// (The −3.03 % quoted in issue #28 and in
/// `njoy-outram-park-fork/tests/leapr_sic_coherent_elastic_oracle.rs` is the
/// same comparison at 296 K, where the elastic is 2.85169 b. The oracle tape's
/// own base temperature is 300 K.)
///
/// **Not validated**: the ±5 % band is a characterisation of a known bias, not
/// a validation of the SiC elastic channel. Do not describe the SiC layer as
/// validated for a shell-resolved TRISO transport case on the strength of this
/// test.
#[test]
fn sic_carbon_binds_a_coherent_elastic_channel_within_the_characterised_band() {
    let law = regenerated_law(SabMaterial::CInSiC, "C-in-SiC");
    let (el, inel) = (law.elastic_xs(E_THERMAL), law.inelastic_xs(E_THERMAL));
    let (free, bound) = free_and_bound(law, "C0");

    eprintln!(
        "[op-6tz.35.1] SiC layer, C in SiC @ {E_THERMAL} eV: elastic {el:.5} b + inelastic \
         {inel:.5} b ({:+.2} % vs oracle {SIC_ELASTIC_ORACLE_BARN} b); free-gas {free:.4} b -> \
         bound {bound:.4} b ({:+.2} %)",
        100.0 * (el - SIC_ELASTIC_ORACLE_BARN) / SIC_ELASTIC_ORACLE_BARN,
        100.0 * (bound - free) / free
    );

    assert!(
        inel > 0.0,
        "the inelastic channel must be generated (got {inel})"
    );
    assert!(
        el > 0.0,
        "the coherent-elastic channel must now be generated -- if this is 0 the generalized \
         coherent-elastic path has regressed, see njoy-outram-park-fork::leapr::coher::general"
    );
    let rel = (el - SIC_ELASTIC_ORACLE_BARN).abs() / SIC_ELASTIC_ORACLE_BARN;
    assert!(
        rel < SIC_ELASTIC_BAND,
        "C-in-SiC elastic {el:.5} b vs ENDF/B-VIII.0 oracle {SIC_ELASTIC_ORACLE_BARN} b is \
         {:.2} % off, outside the characterised {:.0} % band. The known residual is -3.03 % and \
         is traced to the evaluation, not to this port (GitHub issue #28); a move outside the \
         band means something else changed -- investigate rather than widening it.",
        100.0 * rel,
        100.0 * SIC_ELASTIC_BAND
    );
    // Unlike graphite, SiC's bound law stays BELOW the free gas at 0.0253 eV,
    // and the oracle tape says the same: 2.94078 + 0.13291 = 3.0737 b against
    // the same 4.9382 b free-gas carbon, a ratio of 0.622 to this port's 0.605.
    // The compound Bragg channel is shared between two sublattices and is
    // Debye-Waller suppressed at the thermal point, so it does not make up the
    // full free-atom value the way graphite's does. What matters for usability
    // is that the channel is present and dominant -- before it existed the
    // bound law was under a tenth of the free gas.
    assert!(
        el > inel,
        "coherent elastic must dominate the SiC law at {E_THERMAL} eV, got elastic {el} vs \
         inelastic {inel}"
    );
    assert!(
        bound > 0.5 * free,
        "with the elastic channel present the bound SiC law must be a substantial fraction of \
         the free gas it replaces (oracle implies 0.62); got {bound:.4} b vs free {free:.4} b, \
         ratio {:.3}. Below 0.5 suggests the elastic channel has gone missing again.",
        bound / free
    );
}

/// LIVE: silicon in **silicon carbide**, the same closure as the carbon side.
/// See [`sic_carbon_binds_a_coherent_elastic_channel_within_the_characterised_band`]
/// for the methodology, the provenance of the ±5 % band, and why the −3.03 %
/// residual is a property of the published evaluation rather than of this port.
///
/// # Results (measured 2026-08-21, release mode)
///
/// At this file's 293.6 K: elastic 2.85382 b (−2.96 % vs the 2.94078 b
/// oracle), inelastic 0.06367 b.
/// The elastic value is identical to the carbon side, which this test asserts
/// directly: coherent elastic is a Bragg property of the 3C-SiC compound
/// lattice, so a caller that puts **both** SiC laws in one region must count
/// MT=2 **once** or it double-counts Bragg scattering. That is exactly the
/// trap this pair of tests exists to keep visible; see
/// `reference-data/endf/README.md`.
#[test]
fn sic_silicon_binds_the_same_lattice_elastic_channel_as_the_carbon_side() {
    let law = regenerated_law(SabMaterial::SiInSiC, "Si-in-SiC");
    let (el, inel) = (law.elastic_xs(E_THERMAL), law.inelastic_xs(E_THERMAL));
    let (free, bound) = free_and_bound(law, "Si28");

    eprintln!(
        "[op-6tz.35.1] SiC layer, Si in SiC @ {E_THERMAL} eV: elastic {el:.5} b + inelastic \
         {inel:.5} b ({:+.2} % vs oracle {SIC_ELASTIC_ORACLE_BARN} b); free-gas {free:.4} b -> \
         bound {bound:.4} b ({:+.2} %)",
        100.0 * (el - SIC_ELASTIC_ORACLE_BARN) / SIC_ELASTIC_ORACLE_BARN,
        100.0 * (bound - free) / free
    );

    assert!(
        inel > 0.0,
        "the inelastic channel must be generated (got {inel})"
    );
    let rel = (el - SIC_ELASTIC_ORACLE_BARN).abs() / SIC_ELASTIC_ORACLE_BARN;
    assert!(
        rel < SIC_ELASTIC_BAND,
        "Si-in-SiC elastic {el:.5} b vs ENDF/B-VIII.0 oracle {SIC_ELASTIC_ORACLE_BARN} b is \
         {:.2} % off, outside the characterised {:.0} % band -- see the carbon-side test.",
        100.0 * rel,
        100.0 * SIC_ELASTIC_BAND
    );

    let carbon_el = regenerated_law(SabMaterial::CInSiC, "C-in-SiC").elastic_xs(E_THERMAL);
    assert!(
        (el - carbon_el).abs() < 1.0e-9,
        "coherent elastic is a property of the 3C-SiC compound lattice and must be identical for \
         both materials, got Si-in-SiC {el:.9} b vs C-in-SiC {carbon_el:.9} b -- a caller summing \
         both materials' elastic channels for one SiC region double-counts Bragg scattering"
    );
}

/// LIVE: the SiC carbon law and the graphite carbon law are **different laws**
/// on the same nuclide — the reason the shell stack needs per-layer data.
///
/// Carbon bound in silicon carbide sits in a different lattice from carbon
/// bound in graphite, so substituting one for the other is a silent modelling
/// error: it yields a plausible number rather than an error. This test pins
/// that they are distinguishable, so a future refactor cannot quietly collapse
/// the two onto one deck.
///
/// Note the measured difference is currently inflated by the missing SiC
/// elastic channel documented above; it would remain nonzero, but smaller, once
/// that gap closes. The assertion is deliberately a loose "distinguishable"
/// bound rather than a pinned value, so closing the gap does not break it.
#[test]
fn sic_carbon_and_graphite_carbon_are_different_scatterers() {
    let graphite = regenerated_law(SabMaterial::CrystallineGraphite, "C-in-graphite-cmp");
    let in_sic = regenerated_law(SabMaterial::CInSiC, "C-in-SiC-cmp");

    let g = graphite.total_xs(E_THERMAL);
    let s = in_sic.total_xs(E_THERMAL);
    assert!(
        g > 0.0 && s > 0.0,
        "both laws nonzero (graphite {g}, SiC {s})"
    );

    let rel = (g - s).abs() / g.max(s);
    eprintln!(
        "[op-6tz.35.1] C in graphite {g:.4} b vs C in SiC {s:.4} b at {E_THERMAL} eV \
         -- relative difference {:.2} %",
        100.0 * rel
    );
    assert!(
        rel > 1.0e-3,
        "carbon-in-graphite and carbon-in-SiC must be distinguishable bound laws, but they agree \
         to {:.6} % -- check that locate_deck is not returning the same deck for both",
        100.0 * rel
    );
}

/// DIAGNOSTIC (asserts nothing): per-deck channel decomposition at 0.0253 eV.
///
/// The table it prints is what distinguishes "the law is missing a channel"
/// from "the law is fine but small", which is precisely the distinction the
/// SiC gap above turned on. Run with:
///
/// ```text
/// cargo test --release -p outram-mc-libs --test triso_shell_thermal_scattering \
///     -- --ignored --nocapture diagnose_channels
/// ```
#[test]
#[ignore = "diagnostic sweep, asserts nothing -- run explicitly with --ignored"]
fn diagnose_channels() {
    for (m, label) in [
        (SabMaterial::CrystallineGraphite, "C-in-graphite"),
        (SabMaterial::CInSiC, "C-in-SiC"),
        (SabMaterial::SiInSiC, "Si-in-SiC"),
    ] {
        let law = regenerated_law(m, label);
        eprintln!(
            "{label:16} cutoff {:7.4} eV | elastic {:9.5} b | inelastic {:9.5} b | total {:9.5} b",
            law.cutoff_ev(),
            law.elastic_xs(E_THERMAL),
            law.inelastic_xs(E_THERMAL),
            law.total_xs(E_THERMAL),
        );
    }
}
