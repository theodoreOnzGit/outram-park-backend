//! **HTR-10 core k-eff against the RMC benchmark** — `bn:op-867c`, gh #214.
//!
//! The epic's acceptance criterion. Explicit TRISO, hybrid delta/surface
//! tracking, reflector from IAEA-TECDOC-1382 Table 4-3.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_rmc_keff
//! OUTRAM_HTR10_HISTORIES=20000 OUTRAM_HTR10_RINGS=14 cargo run --release ...
//! ```
//!
//! # STATUS
//!
//! ~~**2026-09-17: THIS DOES NOT WORK YET.** First run (8 rings x 12 layers,
//! 1500 histories) returned `k_eff = 0.000000 +/- 0.000000` with 2,820,163,146
//! virtual collisions and zero entropy. Two findings: the majorant cost was as
//! predicted (~22 rejections per real collision) and NOT the failure; `k = 0`
//! was a separate, un-isolated bug, suspected to be the source box, the empty
//! helium material, or `material_at` returning `None`. **Do not treat this
//! example as a result** — it computes no eigenvalue.~~
//!
//! **CORRECTED 2026-09-18 — it works, and every hypothesis quoted above was
//! wrong.** `k = 0` was none of those three. The cause was a **port defect in
//! `HexLattice::distance`**: its axial branch compared a lattice-frame `z`
//! against a tile-local bound, returning NEGATIVE distances so neutrons stepped
//! backwards and oscillated until the event budget killed them. A
//! budget-exhausted history is scored as a *leak*, so the neutron balance
//! closed and `k` reported no error at all. The majorant was never implicated.
//!
//! Three silent geometry defects followed it, all costing fuel rather than
//! histories: the lattice axial centre, the ring count, and the bed cylinder
//! being circumscribed about the tiled hexagon instead of inscribed in it. The
//! largest single reactivity term turned out to be a missing **void** — 98.758
//! cm of helium core cavity above the bed that had been modelled as graphite.
//!
//! **Current result** (14 rings x 25 layers, 10000 histories x [40 inactive +
//! 120 active], surface tracking, ENDF/B-VIII.0):
//!
//! ```text
//! k_eff = 0.995200 +/- 0.001082     RMC 1.004288     -909 +/- 108 pcm
//! lost locate = 0   stuck events = 0   negative distances = 0
//! ```
//!
//! That is inside the 500-1000 pcm gate. **It is the gate being met, not a
//! validated model** — see the qualifications below, and note in particular
//! that this is a SINGLE SEED (`seed: 20260917`), as is every ablation in the
//! V&V record. Pooled multi-seed re-measurement is gh:#196 / `bn:op-awwi` and
//! has NOT been done, so quote this as one draw, not as a mean.
//!
//! # UPDATE 2026-09-18 — the conus is now modelled, and it OVERSHOOTS
//!
//! The 0.995200 above was measured with a **flat-bottomed** bed, omitting the
//! 36.946 cm conus of pebbles beneath it. Modelling the conus (`op-5n34`,
//! `HTR10_CONUS_HEIGHT_CM`) adds 14.8 % kernel volume and is worth
//! **+4578 +/- 158 pcm** (29 sigma), taking the same settings to
//!
//! ```text
//! k_eff = 1.040984 +/- 0.001148     RMC 1.004288     +3670 +/- 115 pcm
//! ```
//!
//! The predicted SIGN was confirmed — adding fuel raised `k`. The magnitude
//! **overshoots the 909 pcm it was meant to close by a factor of five.**
//!
//! ~~**So the -909 pcm above was agreeing for the wrong reason.** A model
//! missing 13.6 % of its fuel volume cannot be 909 pcm low by accident;
//! something else is over-reactive by a few thousand pcm.~~
//!
//! # CORRECTED 2026-09-18 (later) — the conus contents were wrong
//!
//! The overshoot was not a masked error elsewhere. **The conus was filled with
//! FUEL pebbles, and it holds only dummy ones.** Terry et al. (2005) §2, in
//! this repo's own derived geometry
//! (`kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md:256`):
//!
//! > *"the conus and discharge tube contained only **dummy** pebbles"*
//!
//! The conus is part of the bed hex lattice, and `bed_tile_levels` applied the
//! core's 57:43 fuel:dummy split to every level. (Since 2026-09-25 the
//! explicit-TRISO bed assigns fuel per BALL through `bed::TwoBallBed`, with
//! the conus all-dummy by construction.) Extending the lattice to the
//! conus floor therefore filled it with fuel. The geometry was right; the
//! contents were not. Correcting it is worth **-5177 +/- 420 pcm (12 sigma)**.
//!
//! The same sentence covers the DISCHARGE TUBE, which was solid reflector
//! graphite (over-reflecting the conus tip) — now pebble graphite at the
//! bed's 0.61 filling fraction, between that bound and the pure-helium one.
//!
//! **The "two offsetting errors" reading is withdrawn.** The flat-bottomed
//! model was not missing fuel; it was missing the conus's *dummy* pebbles and
//! had reflector graphite there instead, worth only about -680 pcm.
//!
//! Current physical model, 3000 histories x [20 + 60]:
//!
//! ```text
//! single seed : k_eff = 0.991372 +/- 0.003002   ->  -1292 +/- 300 pcm
//! 8 seeds     : pooled dk = -1592 pcm, sem +/-63, seed-to-seed sd 179
//! ```
//!
//! **Quote the pooled number.** The single draw sits 1.7 sd off it. At
//! `sd = 179 pcm` one run of this case re-randomises by that much, so a
//! single-seed residual is not quotable to better than a few hundred pcm --
//! `OUTRAM_BENCH_SEEDS=n` runs the ensemble (gh:#196 / `bn:op-awwi`).
//!
//! Full ablation chain, methodology and results:
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`.
//!
//! # Read this before quoting any number it prints
//!
//! - ~~**ENDF/B-VIII.0**; RMC, MCNP, Serpent and HCP all used **VII.0**. On a
//!   graphite-moderated LEU system that difference alone is worth hundreds of
//!   pcm, so a disagreement CANNOT be attributed to transport.~~
//!   **MEASURED 2026-09-18 — it is worth `+1644 +/- 438 pcm` (3.75 sigma), and
//!   it was essentially the whole residual.** Run `OUTRAM_HTR10_ENDF7=1` for
//!   the reference's own library: `k = 1.004525 +/- 0.003096`, i.e.
//!   **`+385 +/- 310 pcm` from RMC height-matched, 1.24 sigma** — agreement,
//!   against `-1259 pcm` on VIII.0. The warning was right that a disagreement
//!   could not be attributed to transport; it understated the size by 5x.
//!   Single seed — pool before quoting.
//! - The reference quotes **no uncertainty** on any of its twelve values.
//! - The reflector densities are **R-Z homogenised**; TECDOC says a 3-D model
//!   must correct them for the boring geometries. Unadjusted, they smear the
//!   control-rod and helium-flow channels uniformly.
//! - **No control rods or absorber balls** are modelled.
//! - The realised TRISO count is **8340**, not 8335 — unattainable, see
//!   `cubic_array_in_ball`.

use std::time::Instant;

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, mat};
use nee_soon::htr10_rmc::reflector::zone_composition;
use outram_mc_libs::material::nuclide::Nuclide;
use njoy_outram_park_fork::leapr::decks::SabMaterial;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::run_diagnostics::{DataSource, RunDiagnostics};
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::htr10::{BoronReading, Htr10Nuclides};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::tally::mesh::RegularMesh;

const TEMP_K: f64 = 300.15;
/// RMC's value at the **123.576 cm** loading height.
///
/// Kept as the historical comparison point, but **do not compare against it
/// blind** -- see [`rmc_at_height`]. The bed this example builds is
/// `n_axial x 4.899` cm tall (`2 * bed_half_height`; ~~`lat_height *
/// n_axial`~~, which stopped being true on 2026-09-25 when the tile became the
/// two-ball 9.798 cm prism), which at the default layer count is NOT
/// 123.576 cm, and RMC's own curve is steep enough (~270 pcm/cm near this
/// point) that the mismatch is a real systematic rather than a rounding
/// detail.
const RMC_KEFF: f64 = 1.004288; // 123.576 cm loading height

/// RMC's `k_eff` interpolated to an arbitrary fuel-loading height \[cm\], from
/// the paper's own twelve-point curve.
///
/// # Why this exists
///
/// The example compared every result against the single 123.576 cm point while
/// building a bed of `n_axial x 4.899` cm. At the default 25 layers that
/// bed is **122.474 cm**, and RMC's curve interpolates there to **1.000676**
/// rather than 1.004288 -- so **+361 pcm of the reported disagreement was the
/// comparison point, not the model**. The curve rises ~270 pcm/cm through this
/// region, so a 1.1 cm mismatch is worth more than several of the physics terms
/// the V&V record ablates.
///
/// Returns `None` outside the tabulated range \[94.182, 201.960\] cm rather
/// than extrapolating: past the ends the curve flattens and a linear
/// extension would invent reactivity.
fn rmc_at_height(h_cm: f64) -> Option<f64> {
    let c = nee_soon::htr10_rmc::RMC_KEFF_VS_HEIGHT;
    if h_cm < c[0].0 || h_cm > c[c.len() - 1].0 {
        return None;
    }
    for w in c.windows(2) {
        let ((h0, k0), (h1, k1)) = (w[0], w[1]);
        if (h0..=h1).contains(&h_cm) {
            return Some(k0 + (h_cm - h0) / (h1 - h0) * (k1 - k0));
        }
    }
    None
}
const NUC: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,
    c_graphite: 4,
    si28: 5,
    b10: 6,
    // Appended rather than inserted: slots 0..6 keep their indices so no
    // existing material silently repoints at a different nuclide.
    c_sic: 7,
    si29: 8,
    si30: 9,
    // 10: B-11, the rest of natural boron (gh:#311).
    b11: 10,
};

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Ablation knobs over the NUCLEAR DATA rather than the geometry.
///
/// Both exist because the V&V record names ENDF/B-VIII.0-vs-VII.0 as a known,
/// uncorrected systematic "worth hundreds of pcm" that had never actually been
/// priced. No VII.0 tape is available locally, so the library term cannot be
/// reproduced exactly; what CAN be done is to bound library sensitivity on the
/// nuclide that carries most of it.
///
/// - `OUTRAM_HTR10_U238_JENDL=1` swaps U-238 to the JENDL-3.3 evaluation.
///   **This is not the VII.0 offset** and must never be quoted as one. It is a
///   different-library bound on the dominant absorber.
/// - `OUTRAM_HTR10_NO_SAB=1` drops the crystalline-graphite S(alpha,beta) and
///   leaves carbon as a free gas. Primarily a HARNESS check: in a
///   graphite-moderated system this must be worth a large, resolved amount. If
///   it came back near zero, the thermal scattering law would not be engaged
///   at all, and every thermal result here would be resting on nothing.
fn nuclides(diag: &mut RunDiagnostics) -> Option<Vec<Nuclide>> {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let u238_file = if std::env::var("OUTRAM_HTR10_U238_JENDL").is_ok() {
        eprintln!("  ABLATION: U-238 from JENDL-3.3 (NOT the VII.0 offset -- a library bound)");
        "n-092_U_238-JENDL3.3.endf"
    } else {
        "n-092_U_238.endf"
    };
    let no_sab = std::env::var("OUTRAM_HTR10_NO_SAB").is_ok();
    if no_sab {
        eprintln!("  ABLATION: graphite S(alpha,beta) DISABLED -- carbon as free gas");
    }
    // `OUTRAM_HTR10_ENDF7=1` runs the WHOLE nuclide set from ENDF/B-VII.0 --
    // the library RMC, MCNP, Serpent and HCP all used. This is the offset the
    // V&V record has named as "worth hundreds of pcm" and never priced.
    //
    // Downloaded 2026-09-18 from the IAEA NDS `download-endf` tree
    // (https://www-nds.iaea.org/public/download-endf/ENDF-B-VII.0/), which is
    // the same pinned host `njoy-outram-park-fork::acquire` uses. Open,
    // publicly released evaluated nuclear data.
    //
    // **One genuine evaluation difference, not a version relabel:** VII.0's
    // carbon is ELEMENTAL natural carbon (`6-C-0`, MAT 600), where VIII.0 ships
    // C-12 separately. So the VII.0 arm carries the 1.1 % C-13 in its carbon and
    // the VIII.0 arm does not. That is part of what "the library difference"
    // physically IS here, and it is not separable without a third arm.
    let endf7 = std::env::var("OUTRAM_HTR10_ENDF7").is_ok();
    if endf7 {
        eprintln!("  ABLATION: ENDF/B-VII.0 for ALL nuclides (the library the references used)");
        eprintln!("            note: VII.0 carbon is elemental C-nat, not C-12");
    }
    // Every tape is recorded, with its path, whether or not it loaded. A
    // thermal law that fails to load falls back to free gas and the
    // eigenvalue simply comes out somewhere else -- the diagnostics file is
    // what turns that from an invisible substitution into a line of text.
    macro_rules! load {
        ($diag:expr, $n:expr, $f:expr) => {{
            let p = base.join($f);
            eprint!("  {:<6} ", $n);
            let t = Instant::now();
            let r = $diag.time_data(
                format!("{} cross sections", $n),
                DataSource::File(p.clone()),
                format!("{:.2} K, tol 1.0e-3", TEMP_K),
                || {
                    p.exists().then_some(())?;
                    Nuclide::from_endf_file(&p, $n, TEMP_K, 1.0e-3).ok()
                },
            );
            eprintln!("{:.1?}", t.elapsed());
            r
        }};
    }
    // Si-29 and Si-30 from the SAME library as Si-28. Until 2026-09-25 they
    // were hardcoded to the VIII.0 tapes, so the VII.0 arm carried 7.7 % of
    // its silicon from the other evaluation. VII.0 does evaluate both (IAEA
    // NDS n_1428_14-Si-29, n_1431_14-Si-30; the Si-28 from that tree is
    // byte-identical to the committed VII.0 tape).
    let (f_si29, f_si30) = if endf7 {
        ("n-014_Si_029-ENDF7.0.endf", "n-014_Si_030-ENDF7.0.endf")
    } else {
        ("n-014_Si_029-ENDF8.0.endf", "n-014_Si_030-ENDF8.0.endf")
    };
    // B-11 from the selected library too (gh:#311). VII.0 tape: IAEA NDS
    // n_0528_5-B-11; the B-10 from that tree is byte-identical to the committed
    // VII.0 tape.
    let f_b11 = if endf7 {
        "n-005_B_011-ENDF7.0.endf"
    } else {
        "n-005_B_011-ENDF8.0.endf"
    };
    let (f_u235, f_u238, f_o16, f_c, f_si28, f_b10, f_tsl) = if endf7 {
        (
            "n-092_U_235-ENDF7.0.endf",
            "n-092_U_238-ENDF7.0.endf",
            "n-008_O_016-ENDF7.0.endf",
            "n-006_C_000-ENDF7.0.endf",
            "n-014_Si_028-ENDF7.0.endf",
            "n-005_B_010-ENDF7.0.endf",
            "tsl-graphite-ENDF7.0.endf",
        )
    } else {
        (
            "n-092_U_235-ENDF8.0.endf",
            u238_file,
            "n-008_O_016-ENDF8.0.endf",
            "n-006_C_012-ENDF8.0.endf",
            "n-014_Si_028-ENDF8.0.endf",
            "n-005_B_010-ENDF8.0.endf",
            "tsl-crystalline-graphite.endf",
        )
    };
    // The graphite thermal tape's MAT differs between releases: VIII.0's
    // crystalline graphite is MAT 30 (ZA 130), VII.0's is MAT 31 (ZA 131).
    // Passing the wrong one makes `from_endf_file` return Err and the whole
    // nuclide set silently become `None`, which surfaces as the misleading
    // "reference-data/endf/ not in this checkout" -- so it is selected here
    // rather than hardcoded.
    let tsl_mat = if endf7 { 31 } else { 30 };
    let sab = diag.time_data(
        "graphite S(a,b)",
        DataSource::File(base.join(f_tsl)),
        format!("MAT {tsl_mat}, {TEMP_K:.2} K, c_Graphite"),
        || {
            ThermalScattering::from_endf_file(
                base.join(f_tsl).to_str()?,
                tsl_mat,
                TEMP_K,
                "c_Graphite",
            )
            .map_err(|e| eprintln!("  thermal scattering load FAILED (mat {tsl_mat}): {e}"))
            .ok()
        },
    )?;
    // SiC HAS ITS OWN BOUND THERMAL LAWS, and until 2026-09-23 the model used
    // neither: its carbon was free gas and its silicon was bare Si-28.
    // ENDF/B-VIII.0 ships `tsl-CinSiC` (MAT 44) and `tsl-SiinSiC` (MAT 43)
    // precisely so a SiC coating need not be approximated as a gas.
    //
    // These are NOT loaded for the ENDF/B-VII.0 arm: VII.0 has no SiC
    // thermal evaluation, so that arm keeps free-gas SiC. That is a real
    // difference between the two libraries rather than an inconsistency, and
    // it is one more term bundled into the "library" number -- see the
    // carbon-evaluation note above.
    let sic_sab = |diag: &mut RunDiagnostics, mat: i32, name: &'static str| {
        let f = if mat == 44 {
            "tsl-CinSiC.endf"
        } else {
            "tsl-SiinSiC.endf"
        };
        if endf7 || no_sab {
            diag.note(format!(
                "{name} S(a,b) deliberately NOT applied ({}) -- SiC carbon and \
                 silicon are free gas in this arm",
                if endf7 {
                    "ENDF/B-VII.0 has no SiC thermal evaluation"
                } else {
                    "NO_SAB ablation"
                }
            ));
            return None;
        }
        let p = base.join(f);
        diag.time_data(
            format!("{name} S(a,b)"),
            DataSource::File(p.clone()),
            format!("MAT {mat}, {TEMP_K:.2} K"),
            || {
                if !p.exists() {
                    eprintln!("  {name}: {f} not in this checkout -- falling back to free gas");
                    return None;
                }
                ThermalScattering::from_endf_file(p.to_str()?, mat, TEMP_K, name)
                    .map_err(|e| eprintln!("  {name} S(a,b) load FAILED (mat {mat}): {e}"))
                    .ok()
            },
        )
    };
    let c_in_sic = sic_sab(diag, 44, "c_SiC");
    let si_in_sic = sic_sab(diag, 43, "Si_SiC");

    // UO2 HAS BOUND THERMAL LAWS TOO, and the kernel had none at all -- the
    // fuel was scattering as a free gas, in the one place the thermal flux
    // and the absorption actually meet. No UO2 tape ships in
    // `reference-data/endf/`, but both LEAPR decks are committed in
    // `njoy-outram-park-fork`, so these are GENERATED rather than downloaded:
    // reproducible from a deck that can be read, with no new binary tapes.
    //
    // Generation is not free. That is exactly why this run now separates
    // nuclear-data time from transport time.
    //
    // APPLIED TO BOTH LIBRARY ARMS, unlike the SiC laws above. Until
    // 2026-09-24 these were withheld from the `OUTRAM_HTR10_ENDF7=1` arm
    // alongside SiC, but the two cases are not alike: SiC is withheld because
    // ENDF/B-VII.0 ships no SiC thermal evaluation, whereas these are
    // GENERATED from LEAPR decks that do not depend on the library version at
    // all. Withholding them therefore put a difference into the measured
    // "library term" that is not a library difference -- an artefact of which
    // arm the code chose to run them in. Both arms now carry them, so the
    // term prices evaluation differences and the genuinely-absent SiC law,
    // and nothing else.
    // `OUTRAM_HTR10_UO2_TAPE_DIR=<dir>` reads the UO2 laws from TABULATED
    // tapes in <dir> (`tsl-UinUO2.endf`, `tsl-OinUO2.endf`) instead of
    // generating them from the committed decks. The decks are the VIII.0
    // evaluation, so this is how the VII.0 arm gets its OWN UO2 laws (VII.0
    // ships them: U-in-UO2 MAT 76, O-in-UO2 MAT 75). A tape that fails to load
    // aborts the run rather than falling back to free gas.
    let uo2_tape_dir = std::env::var("OUTRAM_HTR10_UO2_TAPE_DIR").ok();
    let uo2_sab = |diag: &mut RunDiagnostics, material: SabMaterial, name: &'static str| {
        if no_sab {
            diag.note(format!(
                "{name} S(a,b) deliberately NOT applied (NO_SAB ablation)"
            ));
            return None;
        }
        if let Some(dir) = &uo2_tape_dir {
            let (f, mat) = match material {
                SabMaterial::UInUO2 => ("tsl-UinUO2.endf", 76),
                SabMaterial::OInUO2 => ("tsl-OinUO2.endf", 75),
                _ => unreachable!("only the two UO2 laws come through here"),
            };
            let p = std::path::Path::new(dir).join(f);
            eprint!("  {name:<8} TAPE  ");
            let t = Instant::now();
            let out = diag.time_data(
                format!("{name} S(a,b)"),
                DataSource::File(p.clone()),
                format!("MAT {mat}, {TEMP_K:.2} K, tabulated tape"),
                || {
                    Some(
                        ThermalScattering::from_endf_file(p.to_str()?, mat, TEMP_K, name)
                            .unwrap_or_else(|e| {
                                panic!("{name} tape {} (MAT {mat}) FAILED: {e}", p.display())
                            }),
                    )
                },
            );
            eprintln!("{:.1?}", t.elapsed());
            return Some(out.expect("UO2 tape path is not valid UTF-8"));
        }
        eprint!("  {name:<8} LEAPR ");
        let t = Instant::now();
        let out = diag.time_data(
            format!("{name} S(a,b)"),
            DataSource::GeneratedFromLeaprDeck(material.base().to_string()),
            format!(
                "MAT {}, {TEMP_K:.2} K, generated in-process",
                material.mat()
            ),
            || {
                ThermalScattering::from_leapr(material, TEMP_K, name)
                    .map_err(|e| eprintln!("  {name} LEAPR generation FAILED: {e}"))
                    .ok()
            },
        );
        eprintln!("{:.1?}", t.elapsed());
        out
    };
    let u_in_uo2 = uo2_sab(diag, SabMaterial::UInUO2, "U_UO2");
    let o_in_uo2 = uo2_sab(diag, SabMaterial::OInUO2, "O_UO2");
    let bind = |n: Nuclide, sab: &Option<ThermalScattering>| match sab {
        Some(s) => n.with_thermal_scattering(s.clone()),
        None => n,
    };

    Some(vec![
        bind(load!(diag, "U235", f_u235)?, &u_in_uo2),
        bind(load!(diag, "U238", f_u238)?, &u_in_uo2),
        bind(load!(diag, "O16", f_o16)?, &o_in_uo2),
        // 3: free-gas carbon, retained for the NO_SAB ablation arm only.
        load!(diag, "C12", f_c)?,
        // 4: graphite-bound carbon.
        if no_sab {
            load!(diag, "C12", f_c)?
        } else {
            load!(diag, "C12", f_c)?.with_thermal_scattering(sab)
        },
        // 5, 8, 9: silicon, split over its three natural isotopes and bound
        // in SiC. The atom density was always built from silicon's natural
        // molar mass, so this splits a correct total rather than changing it.
        bind(load!(diag, "Si28", f_si28)?, &si_in_sic),
        load!(diag, "B10", f_b10)?,
        // 7: carbon bound in SiC.
        bind(load!(diag, "C12", f_c)?, &c_in_sic),
        bind(load!(diag, "Si29", f_si29)?, &si_in_sic),
        bind(load!(diag, "Si30", f_si30)?, &si_in_sic),
        // 10: B-11 (gh:#311). No thermal law: a trace scatterer in graphite.
        load!(diag, "B11", f_b11)?,
    ])
}

fn main() {
    let histories = env_usize("OUTRAM_HTR10_HISTORIES", 2000);
    let rings = env_usize("OUTRAM_HTR10_RINGS", 8);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 12);

    println!("HTR-10 core k-eff vs Li, Yu & Wei (2014), RMC {RMC_KEFF}");
    println!("=========================================================");
    // The library actually in use, not a hardcoded one. Until 2026-09-24 this
    // line printed "ENDF/B-VIII.0" unconditionally, so an `OUTRAM_HTR10_ENDF7=1`
    // run announced itself as VIII.0 at the top of its own transcript -- the
    // one place a reader looks to find out which arm a saved log came from.
    if std::env::var("OUTRAM_HTR10_ENDF7").is_ok() {
        println!("  ENDF/B-VII.0 (the library RMC, MCNP, Serpent and HCP used)");
    } else {
        println!("  ENDF/B-VIII.0 (references used VII.0 -- offset NOT corrected)");
    }
    println!("  explicit TRISO, hybrid delta/surface tracking, TECDOC reflector\n");

    eprintln!("Reconstructing cross sections:");
    // The run's own diagnostic record. Nuclear-data processing and transport
    // are timed SEPARATELY: they scale with completely different things --
    // data prep with the nuclide count and the thermal laws asked for,
    // transport with histories x cycles -- and one combined number makes a
    // run impossible to reason about.
    let mut diag = RunDiagnostics::new("htr10-rmc-keff");
    diag.note(format!(
        "{histories} histories x [{} inactive + {} active], {rings} rings x {layers} layers",
        env_usize("OUTRAM_HTR10_INACTIVE", 30),
        env_usize("OUTRAM_HTR10_ACTIVE", 70)
    ));
    let Some(nucs) = nuclides(&mut diag) else {
        println!("SKIP: reference-data/endf/ not in this checkout.");
        return;
    };

    // Pebble materials, slots 0..5 in DhUniverse::pebble order.
    // OUTRAM_HTR10_NOBORON=1 drops the Table 2 boron impurity rows. Not
    // physical -- real nuclear graphite carries it -- but it bounds how much of
    // the k deficit the boron treatment could possibly account for.
    let boron = if std::env::var("OUTRAM_HTR10_NOBORON").is_ok() {
        BoronReading::None
    } else {
        BoronReading::Natural
    };
    // The material set now lives in `htr10_rmc::materials` so that this
    // example and `htr10_geometry_export` cannot drift apart -- the exporter
    // writes the manuscript's material table and must describe the model this
    // eigenvalue is actually computed with. The ENVIRONMENT KNOBS stay here;
    // the module takes an explicit config.
    let zone_id = std::env::var("OUTRAM_HTR10_REFL_ZONE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(22usize);
    let refl_scale: f64 = std::env::var("OUTRAM_HTR10_REFL_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let z_report = zone_composition(zone_id).expect("zone is listed");
    println!(
        "  reflector zone: {zone_id} (C {:.4e} x{refl_scale:.3}, natural B {:.4e})",
        z_report.carbon, z_report.natural_boron
    );
    let mats = nee_soon::htr10_rmc::materials::htr10_material_set(
        NUC,
        nee_soon::htr10_rmc::materials::Htr10MaterialConfig {
            temperature_k: TEMP_K,
            boron,
            reflector_zone: zone_id,
            reflector_carbon_scale: refl_scale,
        },
    );

    // OUTRAM_HTR10_SURFACE=1 runs the SAME geometry with surface tracking only.
    let surface_only = std::env::var("OUTRAM_HTR10_SURFACE").is_ok();
    // OUTRAM_HTR10_HOMOG=1 drops the nested TRISO lattice for a homogenised
    // fuel zone. Not physical (the zone becomes pure kernel material) but it
    // DISCRIMINATES: if this fissions and the explicit model does not, the TRISO
    // lattice is the culprit.
    let homog = std::env::var("OUTRAM_HTR10_HOMOG").is_ok();
    let maj_idx = if surface_only { usize::MAX } else { 0 };
    let core = if homog {
        nee_soon::htr10_rmc::core_model::assemble(rings, layers, maj_idx)
    } else {
        assemble_explicit_triso(rings, layers, maj_idx)
    };
    println!(
        "  fuel zone: {}",
        if homog {
            "HOMOGENISED"
        } else {
            "explicit TRISO lattice"
        }
    );
    println!(
        "  tracking: {}",
        if surface_only {
            "SURFACE ONLY"
        } else {
            "hybrid (delta bed)"
        }
    );
    println!(
        "  geometry: {} tiles, {} cells, {} universes",
        core.tiles, core.cells, core.universes
    );

    // Region-local majorant: the BED's materials only. The reflector is
    // surface-tracked, so it must NOT raise the bed's tracking cost.
    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-4_f64.ln() + (2.0e7_f64.ln() - 1.0e-4_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let bed_mats: Vec<usize> = (0..=mat::HELIUM).collect();
    let maj = Majorant::over_indices(&mats, &bed_mats, &nucs, &grid, 0.3);

    let settings = KeffSettings {
        n_particles: histories,
        // Tunable so source convergence can be MEASURED rather than assumed.
        // A loosely-coupled 1.8 m pebble core is exactly where a thin inactive
        // stage biases k, and the reference paper's own 5 inactive cycles are
        // not a model to copy.
        n_inactive: env_usize("OUTRAM_HTR10_INACTIVE", 30),
        n_active: env_usize("OUTRAM_HTR10_ACTIVE", 70),
        temperature_k: TEMP_K,
        seed: 20260917,
        // `OUTRAM_HTR10_THREADS=n` pins the thread count; default stays Auto.
        //
        // This exists because thread-count independence is **tested for
        // `run_keff` and merely assumed for `run_keff_csg_hybrid`**, which is
        // the driver this case actually uses.
        // `physics::keff::tests::cpu_multi_is_reproducible` asserts bit-identity
        // between 1 and 4 threads -- on the simple sphere path, through
        // `run_keff`. Nothing covers the hybrid CSG path, and with `Auto` the
        // thread count follows machine load, so two runs of this example on a
        // busy box need not use the same count. If the hybrid driver is
        // order-dependent, every single-seed number in the HTR-10 V&V record is
        // irreproducible and the ablation chain built from their differences is
        // unsound. Pinning the count is what makes that testable.
        compute: ComputeType::CpuMultiThread(
            match std::env::var("OUTRAM_HTR10_THREADS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
            {
                Some(n) => ThreadCount::Fixed(n),
                None => ThreadCount::Auto,
            },
        ),
        ..KeffSettings::default()
    };
    let r = core.tiles as f64;
    let _ = r;
    // The source box and entropy mesh must span the WHOLE fissile region.
    //
    // Both were [-50,50]^3 / [-60,60]^3, fixed numbers that predate the conus
    // and the corrected bed extent. The bed now runs from `conus_floor`
    // (-98.2 cm at 25 layers) to `+bed_half_height`, so the old box missed the
    // entire conus and the top of the bed. A starting source that misses fuel
    // is recoverable given enough inactive generations; an entropy mesh that
    // is blind to part of the core is NOT -- it reports convergence of the
    // region it can see, which is exactly the diagnostic one must not trust.
    let zl = core.conus_floor;
    let zu = core.bed_half_height;
    let rb = core.bed_radius;
    let src = SourceBox {
        lower: Position::new(-rb, -rb, zl),
        upper: Position::new(rb, rb, zu),
    };
    let entropy_mesh = RegularMesh {
        lower_left: [-rb, -rb, zl],
        upper_right: [rb, rb, zu],
        dimension: [4, 4, 4],
    };

    println!(
        "  {histories} histories x [{} inactive + {} active]\n",
        settings.n_inactive, settings.n_active
    );
    println!(
        "  nuclear data processed in {:.1} s ({} items)",
        diag.data_seconds(),
        diag.data_item_count()
    );
    let t = Instant::now();
    let res = diag.time_phase("transport (k-eigenvalue)", || {
        run_keff_csg_hybrid(
            &core.geometry,
            &mats,
            &nucs,
            if surface_only {
                &[]
            } else {
                std::slice::from_ref(&maj)
            },
            Some(&entropy_mesh),
            src,
            &settings,
            None,
        )
    });
    let secs = t.elapsed().as_secs_f64();

    // SEED ENSEMBLE (OUTRAM_BENCH_SEEDS, default 1 -- single-seed behaviour and
    // runtime unchanged unless asked for).
    //
    // This case scatters seed-to-seed by far more than most of the effects
    // being argued about, so a single pair CANNOT resolve anything much below
    // ~300 pcm. Anything smaller must be quoted as a pooled mean with its sem,
    // or not quoted at all. `run_keff_csg_hybrid` is already internally
    // multi-threaded, so seeds run sequentially and each uses every core.
    let n_seeds = outram_mc_libs::vv::bench_seeds();
    if n_seeds > 1 {
        let mut ens: Vec<f64> = vec![(res.k_mean - RMC_KEFF) * 1.0e5];
        for seed in 2..=n_seeds as u64 {
            let sset = KeffSettings {
                seed: settings.seed + seed,
                ..settings.clone()
            };
            let r2 = run_keff_csg_hybrid(
                &core.geometry,
                &mats,
                &nucs,
                if surface_only {
                    &[]
                } else {
                    std::slice::from_ref(&maj)
                },
                Some(&entropy_mesh),
                src,
                &sset,
                None,
            );
            eprintln!("    seed {seed}: k = {:.6} +/- {:.6}", r2.k_mean, r2.k_std);
            ens.push((r2.k_mean - RMC_KEFF) * 1.0e5);
        }
        let (mean, sd, sem) = outram_mc_libs::vv::pooled(&ens);
        println!("\n  ENSEMBLE HTR-10 vs RMC: {n_seeds} seeds");
        println!("    pooled dk    = {mean:+.0} pcm");
        println!("    seed-to-seed sd  = {sd:.0} pcm   (what ONE run scatters by)");
        println!("    uncertainty  sem = +/-{sem:.0} pcm   (on the pooled mean)");
    }

    // Height-matched comparison. The bed is `2 * bed_half_height` = `n_axial x
    // 4.899` cm tall (not `lat_height * n_axial` since the two-ball tile); RMC's
    // curve is sampled at ITS heights, so comparing against a point the model
    // does not occupy imports a systematic worth ~270 pcm per cm of mismatch.
    let bed_height_cm = core.bed_half_height * 2.0;
    let rmc_here = rmc_at_height(bed_height_cm);
    match rmc_here {
        Some(k) => println!(
            "\n  HEIGHT-MATCHED: bed is {bed_height_cm:.3} cm -> RMC(interp) = {k:.6}\n  \
             (the {RMC_KEFF:.6} headline is RMC at 123.576 cm; difference {:+.0} pcm \
             is comparison point, NOT model)",
            (RMC_KEFF - k) * 1.0e5
        ),
        None => println!(
            "\n  HEIGHT-MATCHED: bed is {bed_height_cm:.3} cm -- OUTSIDE the tabulated \
             RMC range [94.182, 201.960] cm, so no height-matched reference exists \
             and the {RMC_KEFF:.6} comparison below is NOT like-for-like."
        ),
    }

    let pcm = (res.k_mean - RMC_KEFF) * 1.0e5;
    if let Some(k) = rmc_here {
        println!(
            "  dk height-matched = {:+.0} pcm   (against {:.6}, not {RMC_KEFF:.6})",
            (res.k_mean - k) * 1.0e5,
            k
        );
    }
    let sigma = res.k_std * 1.0e5;
    println!("  k_eff        = {:.6} +/- {:.6}", res.k_mean, res.k_std);
    println!("  RMC          = {RMC_KEFF:.6}");
    println!("  difference   = {pcm:+.0} pcm   (our sigma {sigma:.0} pcm)");
    println!("  virtual coll = {}", res.virtual_collisions);
    // Histories transported = n_particles x every generation, active or not.
    let n_hist = res.histories.max(1) as f64;
    println!(
        "  histories    = {} (planned {})",
        res.histories,
        settings.n_particles * (settings.n_inactive + settings.n_active)
    );
    println!(
        "  collisions   = {} ({:.2} per history)",
        res.collisions,
        res.collisions as f64 / n_hist
    );
    println!(
        "  lost locate  = {} ({:.3} %)",
        res.lost_locate,
        100.0 * res.lost_locate as f64 / n_hist
    );
    println!(
        "  stuck events = {} ({:.3} %)",
        res.stuck_events,
        100.0 * res.stuck_events as f64 / n_hist
    );
    if res.stuck_events > 0 {
        println!(
            "  stuck path   = {:.4} cm mean, last E = {:.4e} eV",
            res.stuck_path_cm / res.stuck_events as f64,
            res.stuck_last_e
        );
    }
    println!(
        "  neg distance = {} (worst {:.4e} cm, level {})",
        res.neg_dist, res.neg_worst, res.neg_level
    );
    println!(
        "      from lattice = {}, from surface = {}",
        res.neg_from_lattice, res.neg_from_surface
    );
    println!(
        "  leak vacuum  = {} ({:.3} %)",
        res.leak_vacuum,
        100.0 * res.leak_vacuum as f64 / n_hist
    );
    println!(
        "  leak infinity= {} ({:.3} %)",
        res.leak_infinity,
        100.0 * res.leak_infinity as f64 / n_hist
    );
    println!("  wall clock   = {secs:.1} s");
    println!("  generations reported: {}", res.k_by_generation.len());
    let nz = res.k_by_generation.iter().filter(|k| **k > 0.0).count();
    println!("  generations with k > 0: {nz}");
    for (i, k) in res.k_by_generation.iter().take(5).enumerate() {
        println!("    gen {i}: k = {k:.6}");
    }
    // Full entropy trace: a still-rising trace means the source has NOT
    // converged and every active generation before it is biased.
    if !res.entropy.is_empty() {
        let step = (res.entropy.len() / 12).max(1);
        print!("  entropy trace:");
        for (i, h) in res.entropy.iter().enumerate() {
            if i % step == 0 || i + 1 == res.entropy.len() {
                print!(" {h:.3}");
            }
        }
        println!();
    }
    if let (Some(first), Some(last)) = (res.entropy.first(), res.entropy.last()) {
        println!(
            "  entropy      = {first:.4} -> {last:.4} bits (ceiling {:.4})",
            (entropy_mesh.n_bins() as f64).log2()
        );
    }
    println!("\n  Gate is 500-1000 pcm. This is a REDUCED core ({rings} rings x {layers} layers),");
    println!("  not the 123.576 cm loading, and carries the VIII.0-vs-VII.0 offset.");

    // Data-processing time and transport time, reported separately, and the
    // full provenance record written to a file. If any data item failed to
    // load, `print_summary` says so loudly -- a run that silently fell back
    // to free gas must not be read as if it had not.
    diag.note(format!("k_eff = {:.6} +/- {:.6}", res.k_mean, res.k_std));
    diag.print_summary();
    diag.write_and_report();
}
