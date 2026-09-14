//! **DH treatment V&V** — the eigenvalue each [`DhTreatment`] produces on one
//! identical FHR unit cell, so the speedups in
//! `examples/dh_tracking_speedup.rs` can be judged on accuracy rather than on
//! cost alone.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example dh_keff_vv
//! ```
//!
//! # What this establishes, and what it does not
//!
//! **Establishes:** how far each approximate treatment moves k away from an
//! exact delta-tracked answer on the same geometry, same materials, same
//! settings, same seed — and, because the geometry is now the published one,
//! how far the exact arm itself sits from OpenMC. The first is the accuracy
//! cost of the corresponding speedup, which is what a user choosing a treatment
//! needs to know; the second is what says the exact arm is trustworthy enough
//! to be the reference.
//!
//! **Does not establish:** agreement with an experiment. OpenMC is another
//! code, not a measurement. This is **verification**, not validation.
//!
//! # Geometry — the FHR reference UNIT CELL, not a bare pebble
//!
//! 1.9 cm fuel zone inside a 2.0 cm pebble, wrapped in FLiBe out to a
//! **reflective sphere at r = 3.0 cm**, at **600 K**
//! ([`PebbleParams::fhr_unit_cell`]). That is the system
//! `examples/fhr_ring_rpt_endf.rs` and the OpenMC deck behind it both use, so
//! the numbers here are directly comparable to the published
//! `k = 1.36510 ± 0.00063` (explicit TRISO) and `k = 1.36479 ± 0.00067`
//! (ring-RPT).
//!
//! **An earlier revision used [`PebbleParams::fhr_reference`] — the pebble
//! alone, reflective at its own surface, no coolant.** That is a legitimate
//! quantity but a *different reactor*: stripping 1 cm of FLiBe moderator off a
//! pebble whose resonance escape probability is only ~0.484 cost over 13 000
//! pcm, and made every number here non-comparable to the published reference
//! without saying so. Fixed 2026-09-14.
//!
//! # Data — HIGH tier, ENDF/B-VIII.0
//!
//! Continuous-energy cross sections reconstructed from the ENDF/B-VIII.0 tapes
//! in `reference-data/endf/` (RECONR + BROADR on device at 600 K, 1e-3
//! tolerance), plus the **ENDF/B-VIII.0 crystalline-graphite S(alpha,beta)**
//! law on the moderator carbon.
//!
//! The bound-carbon law is not optional here. A pebble is graphite-moderated
//! and the spectrum is thermal, so below ~4 eV neutrons scatter off the bound
//! lattice — coherent Bragg plus incoherent inelastic — not off a free carbon
//! atom. This crate's own measurement is that omitting it puts a
//! graphite-moderated spectrum **~1700 pcm too high**
//! (`tests/htr10_graphite_thermal_scattering`).
//!
//! An earlier revision also used the LOW embedded tier (`Nuclide::from_core`:
//! coarse group data above the WMP range, Watt fission-birth stand-in, no
//! S(alpha,beta)). That is fine for a *timing* comparison but is not defensible
//! as V&V evidence. It also **flattered the approximations**: LOW's coarse
//! group data has already smeared the U-238 resonances whose spatial
//! self-shielding homogenisation destroys, so there was less left to lose and
//! the measured penalties came out ~700-800 pcm too small. Testing an
//! approximation on data that has already made the same approximation
//! understates its cost.
//!
//! # One caveat on the ring-RPT comparison
//!
//! Our ring-RPT annulus keeps the graphite S(alpha,beta) law on its
//! buffer/PyC carbon, because `homogenise_by_volume` preserves nuclide
//! indices. The OpenMC deck's ring-RPT pebble does **not** — `rpt_pebble.py`
//! builds the mixed material with `add_element('C', ...)` and no
//! `add_s_alpha_beta`, so 83 % of its mixed carbon reverts to free gas. Keeping
//! the bound law is the better homogenisation and is consistent across every
//! arm here, which is what isolates the *geometric* approximation — but it
//! means our ring-RPT arm is **not** bit-comparable to the published ring-RPT
//! number, while our delta arm **is** comparable to the published explicit one.
//! `examples/fhr_ring_rpt_endf.rs` matches the deck instead, deliberately.
//!
//! # Results
//!
//! **PENDING — this example has not yet been re-run on the unit cell.** The
//! geometry, the treatment set and the ring-RPT implementation all changed on
//! 2026-09-14; every previously recorded number here was taken on the bare
//! pebble with a `RingRpt` variant that actually did naive full-zone
//! homogenisation, so none of them transfers. Run the example to produce the
//! table; do not cite a number from this file until it is filled in.
//!
//! For the record, superseded and **not to be cited**: on the bare pebble at
//! 293.6 K, delta tracking gave `k = 1.23050 ± 0.00608` in 28.5 s, chord-length
//! sampling `1.18328 ± 0.00732` in 56.0 s (-4722 pcm), and the naive smear —
//! then mislabelled ring-RPT — `1.17899 ± 0.00615` in 133.9 s (-5151 pcm).
//! The finding worth carrying forward from that run is the *ranking*: both
//! approximations came out **slower** than exact delta tracking in a real
//! continuous-energy eigenvalue calculation, inverting the 2.5-3x and 8x
//! speedups the geometry-only benchmark reports. Whether that survives on the
//! unit cell with a real ring-RPT arm is exactly what the re-run settles.
//!
//! The likely mechanism, consistent with those numbers but **not isolated**:
//! homogenisation moves cost out of geometry and into cross-section evaluation.
//! Most of the fuel zone by volume is single-nuclide graphite — buffer, IPyC,
//! OPyC, matrix — so a delta-tracking point query there sums **one** nuclide,
//! while a smeared point carries the union of all of them. The geometry lookup
//! traded away was already O(1) through the packing grid. Separating material
//! cost from geometry cost needs a profile or an A/B with matched nuclide
//! counts, and that has **not** been run.
//!
//! Chord-length sampling pays a second, avoidable cost: its sampler is stateful
//! and the k-eff seam wants [`MaterialQuery`] to be [`Sync`], so every point
//! query takes a mutex. That is an artefact of this wiring, not a property of
//! CLS, and it is the first thing to attack if CLS is wanted for production.
//!
//! # Settings
//!
//! Default 800 histories x [15 inactive + 40 active], single-threaded. At these
//! settings the combined standard error is of order 900 pcm, so a bias smaller
//! than roughly 2000 pcm will read as "not resolved" — which means
//! **unmeasured**, not zero. Raise it with `OUTRAM_DH_VV_HISTORIES`.
//!
//! `OUTRAM_DH_VV_FIT_RPT=1` additionally fits this code's own ring-RPT inner
//! radius via [`fit_ring_rpt_inner_radius`] instead of borrowing the deck
//! author's OpenMC-fitted 1.4934 cm, at a cost of about a dozen extra
//! eigenvalue solves.

use std::time::Instant;

use outram_mc_libs::prelude::*;
use outram_mc_libs::material::material::NuclideComponent;
use outram_mc_libs::material::thermal::ThermalScattering;
use njoy_outram_park_fork::reference_data::reference_endf;

/// Material / data temperature \[K\] — the FHR deck's operating temperature.
const TEMP_K: f64 = 600.0;

/// Li-7 enrichment of the FLiBe, as the reference deck specifies it. Li-6 is a
/// strong absorber, so the residual 50 pcm of it is not negligible bookkeeping.
const LI7_PURITY: f64 = 0.99995;

/// Avogadro's number / 1e24, so `rho * N_A_B / M` lands directly in atoms/b-cm.
const N_A_B: f64 = 0.602_214_076;

/// Published OpenMC reference on **this** geometry (op-mzvp.1, GH #156):
/// explicit TRISO, ENDF/B-VIII.0, `c_Graphite`, reflective sphere r = 3 cm.
const OMC_EXPLICIT: (f64, f64) = (1.36510, 0.00063);
/// The same deck's ring-RPT pebble — see the caveat in the doc comment about
/// why our ring-RPT arm is not bit-comparable to this one.
const OMC_RING_RPT: (f64, f64) = (1.36479, 0.00067);

// Nuclide indices into the vector returned by `nuclides()`.
const U235: usize = 0;
const U238: usize = 1;
const O16: usize = 2;
const C_FREE: usize = 3;
const C_GRAPHITE: usize = 4;
const SI28: usize = 5;
const LI6: usize = 6;
const LI7: usize = 7;
const BE9: usize = 8;
const F19: usize = 9;

/// Representative HALEU UCO TRISO compositions \[atoms/b-cm\] plus FLiBe.
///
/// Illustrative of the material class, **not** a benchmark specification. This
/// example measures the *difference between treatments*, which is insensitive
/// to the exact densities so long as every arm shares them.
///
/// # Carbon bookkeeping
///
/// Two carbon entries, deliberately:
///
/// - **`C_FREE`** — the kernel's oxycarbide carbon and the SiC carbon. Neither
///   sits in a graphite lattice, and the reference deck treats both as free gas.
/// - **`C_GRAPHITE`** — buffer, IPyC, OPyC, fuel-zone matrix and the outer
///   shell, all graphite, all carrying the S(alpha,beta) law.
///
/// C-13 (1.1 % of natural carbon) is neglected, as are Si-29/30. Both apply
/// identically to every arm, so neither can bias the comparison this example
/// exists to make — though they would matter for an absolute k.
fn materials() -> Vec<Material> {
    let m = |id: i32, name: &str, comps: Vec<(usize, f64)>| Material {
        id,
        name: name.into(),
        temperature: TEMP_K,
        components: comps
            .into_iter()
            .map(|(nuclide_idx, atom_density)| NuclideComponent { nuclide_idx, atom_density })
            .collect(),
    };

    // FLiBe = 2LiF.BeF2 = Li2BeF4. Density correlation rho(T) = (2416 -
    // 0.49072*T)/1000 g/cm^3, the same one the reference deck uses.
    let flibe_rho = (2416.0 - 0.49072 * TEMP_K) / 1000.0;
    let (m_li7, m_li6, m_be9, m_f19) = (7.016_003, 6.015_123, 9.012_183, 18.998_403);
    let li7_per_formula = 2.0 * LI7_PURITY;
    let li6_per_formula = 2.0 * (1.0 - LI7_PURITY);
    let m_formula =
        li7_per_formula * m_li7 + li6_per_formula * m_li6 + m_be9 + 4.0 * m_f19;
    // Formula units per barn-cm; multiply by the per-formula count for each.
    let n_formula = flibe_rho * N_A_B / m_formula;

    vec![
        // 0 kernel — 19.9 % HALEU UCO. Kernel carbon is not graphite.
        m(0, "UCO kernel", vec![
            (U235, 4.40e-3), (U238, 1.77e-2), (O16, 2.27e-2), (C_FREE, 9.10e-3),
        ]),
        // 1 buffer — porous graphite
        m(1, "buffer", vec![(C_GRAPHITE, 5.02e-2)]),
        // 2 IPyC — pyrolytic graphite
        m(2, "IPyC", vec![(C_GRAPHITE, 9.53e-2)]),
        // 3 SiC — carbon here is silicon-bound, not graphite
        m(3, "SiC", vec![(SI28, 4.79e-2), (C_FREE, 4.79e-2)]),
        // 4 OPyC — pyrolytic graphite
        m(4, "OPyC", vec![(C_GRAPHITE, 9.53e-2)]),
        // 5 matrix — graphite binder in the fuel zone
        m(5, "matrix graphite", vec![(C_GRAPHITE, 8.53e-2)]),
        // 6 shell — fuel-free graphite outer shell
        m(6, "shell graphite", vec![(C_GRAPHITE, 8.78e-2)]),
        // 7 coolant — FLiBe, free-gas in both codes
        m(7, "FLiBe coolant", vec![
            (F19, 4.0 * n_formula),
            (LI7, li7_per_formula * n_formula),
            (LI6, li6_per_formula * n_formula),
            (BE9, n_formula),
        ]),
    ]
}

/// Reconstruct the ten nuclides from the ENDF/B-VIII.0 reference tapes at
/// [`TEMP_K`].
///
/// Index order must match the constants above.
fn nuclides() -> Vec<Nuclide> {
    let load = |name: &str, file: &str| -> Nuclide {
        let path = reference_endf(file)
            .unwrap_or_else(|| panic!("missing reference tape {file} in reference-data/endf/"));
        eprint!("  reconstructing {name:<6} from {file} … ");
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", path.display()));
        eprintln!("{:.1?}", t0.elapsed());
        n
    };

    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-crystalline-graphite.endf")
            .expect("missing tsl-crystalline-graphite.endf")
            .to_str()
            .expect("valid UTF-8 path"),
        30, // MAT 30 — C in crystalline graphite (ENDF/B-VIII.0)
        TEMP_K,
        "c_Graphite",
    )
    .expect("crystalline-graphite S(alpha,beta)");
    eprintln!("  graphite S(alpha,beta): ENDF/B-VIII.0 tsl-crystalline-graphite @ {TEMP_K} K … ok");

    vec![
        load("U235", "n-092_U_235-ENDF8.0.endf"),
        load("U238", "n-092_U_238.endf"),
        load("O16", "n-008_O_016-ENDF8.0.endf"),
        // free-gas carbon: kernel oxycarbide and SiC
        load("C12", "n-006_C_012-ENDF8.0.endf"),
        // graphite-bound carbon: buffer, PyC, matrix, shell
        load("C12", "n-006_C_012-ENDF8.0.endf").with_thermal_scattering(sab),
        load("Si28", "n-014_Si_028-ENDF8.0.endf"),
        load("Li6", "n-003_Li_006-ENDF8.0.endf"),
        load("Li7", "n-003_Li_007-ENDF8.0.endf"),
        load("Be9", "n-004_Be_009-ENDF8.0.endf"),
        load("F19", "n-009_F_019-ENDF8.0.endf"),
    ]
}

/// `(delta_pcm, sigma_distance)` between two eigenvalues with independent
/// statistics.
fn delta(k: f64, s: f64, k0: f64, s0: f64) -> (f64, f64) {
    let dk = (k - k0) * 1.0e5;
    let combined = (s * s + s0 * s0).sqrt() * 1.0e5;
    (dk, dk.abs() / combined.max(1.0))
}

struct Row {
    treatment: DhTreatment,
    k: f64,
    std: f64,
    secs: f64,
    particles: usize,
}

fn main() {
    let n_particles: usize = std::env::var("OUTRAM_DH_VV_HISTORIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(800);
    let fit_rpt = std::env::var("OUTRAM_DH_VV_FIT_RPT").is_ok();

    println!("Reconstructing ENDF/B-VIII.0 cross sections at {TEMP_K} K (setup, not transport):");
    let t_data = Instant::now();
    let nucs = nuclides();
    let data_secs = t_data.elapsed().as_secs_f64();
    println!("  data ready in {data_secs:.1} s\n");
    let mats = materials();

    let settings = KeffSettings {
        n_particles,
        n_inactive: 15,
        n_active: 40,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };

    println!("DH treatment V&V — FHR reference UNIT CELL, every treatment");
    println!("===========================================================");
    println!(
        "  histories  : {} x [{} inactive + {} active]",
        settings.n_particles, settings.n_inactive, settings.n_active
    );
    println!("  data       : HIGH tier, ENDF/B-VIII.0 + crystalline-graphite S(a,b), {TEMP_K} K");
    println!("  geometry   : 1.9 cm fuel zone / 2.0 cm pebble / FLiBe to r = 3.0 cm, reflective");
    println!("  reference  : OpenMC explicit TRISO k = {:.5} +/- {:.5}\n", OMC_EXPLICIT.0, OMC_EXPLICIT.1);

    // Optionally fit the ring-RPT radius for THIS code and THIS data rather
    // than borrowing the deck author's OpenMC-fitted value. Off by default
    // because it costs about a dozen extra eigenvalue solves.
    let rpt_treatment = if fit_rpt {
        println!("Fitting the ring-RPT inner radius (OUTRAM_DH_VV_FIT_RPT set) …");
        let params = PebbleParams::fhr_unit_cell().with_materials(mats.clone());
        match fit_ring_rpt_inner_radius(&params, &nucs, &settings, None) {
            Ok(fit) => {
                println!(
                    "  fitted r_inner = {:.4} cm   k = {:.5} (target {:.5} +/- {:.5}, \
                     residual {:+.0} pcm, {} solves, converged={})",
                    fit.inner_radius,
                    fit.fitted_k,
                    fit.target_k,
                    fit.target_std,
                    fit.residual_pcm,
                    fit.evaluations,
                    fit.converged()
                );
                println!(
                    "  deck author's OpenMC-fitted radius = {:.4} cm   delta = {:+.4} cm\n",
                    DhTreatment::FHR_REFERENCE_RPT_INNER,
                    fit.inner_radius - DhTreatment::FHR_REFERENCE_RPT_INNER
                );
                fit.treatment()
            }
            Err(e) => {
                println!("  fit failed ({e}); falling back to the reference radius\n");
                DhTreatment::RingRpt { inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER }
            }
        }
    } else {
        println!(
            "(ring-RPT uses the OpenMC-fitted r_inner = {:.4} cm; set OUTRAM_DH_VV_FIT_RPT=1\n \
             to fit this code's own radius instead — about a dozen extra solves)\n",
            DhTreatment::FHR_REFERENCE_RPT_INNER
        );
        DhTreatment::RingRpt { inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER }
    };

    let treatments = [
        DhTreatment::DeltaTracking,
        DhTreatment::ChordLength,
        DhTreatment::Scls,
        DhTreatment::Homogenised,
        rpt_treatment,
    ];

    let mut rows: Vec<Row> = Vec::new();
    for treatment in treatments {
        let params = PebbleParams::fhr_unit_cell().with_materials(mats.clone());
        let universe = match DhUniverse::pebble(params, treatment) {
            Ok(u) => u,
            Err(e) => {
                println!("  {:<28} BUILD FAILED: {e}", treatment.name());
                continue;
            }
        };
        let particles = universe.particle_count();
        let t0 = Instant::now();
        let result = universe.keff(&nucs, &settings);
        let secs = t0.elapsed().as_secs_f64();
        println!(
            "  {:<28} k = {:.5} +/- {:.5}   {:>7.1} s   {} particles stored",
            treatment.name(),
            result.k_mean,
            result.k_std,
            secs,
            particles
        );
        rows.push(Row { treatment, k: result.k_mean, std: result.k_std, secs, particles });
    }

    let Some(reference) = rows.iter().find(|r| r.treatment.is_exact()) else {
        println!("\nNo exact arm completed — nothing to compare against.");
        return;
    };
    let _ = reference.particles;

    println!("\n=== Bias vs the exact (delta-tracked) treatment ===");
    println!(
        "  Reference: k = {:.5} +/- {:.5}  (delta tracking is exact by construction)\n",
        reference.k, reference.std
    );
    println!("  treatment                      dk [pcm]  combined sd     sigma      verdict");
    println!("  ------------------------------------------------------------------------------");
    for row in rows.iter().filter(|r| !r.treatment.is_exact()) {
        let (dk, z) = delta(row.k, row.std, reference.k, reference.std);
        let combined = (row.std * row.std + reference.std * reference.std).sqrt() * 1.0e5;
        println!(
            "  {:<28} {:>8.0}  {:>11.0}  {:>8.1}  {}",
            row.treatment.name(),
            dk,
            combined,
            z,
            if z >= 3.0 { "RESOLVED bias" } else { "not resolved at this n" }
        );
    }

    println!("\n=== Against the published OpenMC reference ===");
    let (dk_ex, z_ex) = delta(reference.k, reference.std, OMC_EXPLICIT.0, OMC_EXPLICIT.1);
    println!(
        "  delta tracking vs OpenMC explicit TRISO : {:+.0} pcm ({:.1} sigma)",
        dk_ex, z_ex
    );
    if let Some(rpt) = rows.iter().find(|r| r.treatment.needs_fitting()) {
        let (dk_rp, z_rp) = delta(rpt.k, rpt.std, OMC_RING_RPT.0, OMC_RING_RPT.1);
        println!(
            "  ring-RPT       vs OpenMC ring-RPT       : {:+.0} pcm ({:.1} sigma)   \
             [see the S(a,b) caveat]",
            dk_rp, z_rp
        );
    }

    println!("\n=== Accuracy bought per unit speed ===");
    for row in &rows {
        let speed = reference.secs / row.secs;
        if row.treatment.is_exact() {
            println!("  {:<28} {:>5.2}x   (reference)", row.treatment.name(), speed);
        } else {
            let (dk, _) = delta(row.k, row.std, reference.k, reference.std);
            println!(
                "  {:<28} {:>5.2}x   costs {:+.0} pcm",
                row.treatment.name(),
                speed,
                dk
            );
        }
    }

    println!(
        "\n  A treatment is worth taking only if its bias is small against the\n  \
         tolerance of the question being asked. A bias that is 'not resolved'\n  \
         here means this run could not measure it -- NOT that it is zero."
    );
}
