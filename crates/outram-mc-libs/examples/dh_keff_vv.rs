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
//! # Results — measured 2026-09-14
//!
//! 7200 histories x [15 inactive + 40 active], single-threaded
//! (`ComputeType::CpuSingleThread`, the default — every arm, so the timings are
//! thread-matched). 25 856 explicit particles in the delta arm. Cross-section
//! reconstruction is **excluded** from the per-treatment timings: it is one-off
//! setup and all five arms share it.
//!
//! ## Machine the timings were taken on
//!
//! Wall-clock seconds mean nothing without it, and the *ratios* are the part
//! that should survive a change of hardware — quote those, not the seconds.
//!
//! | | |
//! |---|---|
//! | CPU | Intel Xeon @ 2.10 GHz, **4 cores / 4 threads** (1 thread per core, no SMT) |
//! | Cache | L1d 192 KiB, L1i 128 KiB, L2 8 MiB (4x2 MiB), **L3 260 MiB shared** |
//! | ISA | x86-64 with AVX-512 (F/DQ/CD/BW/VL, VNNI, BF16), AMX, SHA-NI |
//! | RAM | **15.7 GiB** (16 461 028 kB), no swap |
//! | Virtualisation | KVM, full virtualisation — a cloud container, not bare metal |
//! | Kernel | Linux 6.18.44 |
//! | Toolchain | rustc 1.94.1 (e408947bf, 2026-03-25), `--release` |
//! | Backend | `ComputeType::CpuSingleThread` — **one core in use**, 3 idle |
//!
//! Three things follow that are worth stating rather than leaving to be
//! rediscovered:
//!
//! - **Only one of the four cores does any work.** The default backend is
//!   single-threaded, and these runs did not change it. A multi-threaded run
//!   would be a different measurement — and for CLS and SCLS, a badly
//!   distorted one, because their point query serialises on a mutex (GitHub
//!   issue #205). That is the measurement that has not been made.
//! - **The L3 is 260 MiB and the delta arm's packing is far smaller than that.**
//!   25 856 spheres at a few tens of bytes each is well under a megabyte, so the
//!   explicit geometry is cache-resident here and its lookup cost is close to
//!   best case. On a machine with an ordinary few-MiB L3 the explicit arm would
//!   look relatively worse, which would *widen* the approximations' speedups
//!   rather than narrow them.
//! - **It is a virtualised, shared host.** Run-to-run timing noise of a few per
//!   cent is expected and observed; the 800- and 7200-history runs agreed on
//!   every speed ratio to within about 1 %, which is the reason to trust the
//!   ratios at two significant figures and no further.
//!
//! | Treatment | k | vs delta | sigma | Speed |
//! |---|---|---|---|---|
//! | delta tracking (exact) | 1.38647 +/- 0.00211 | — | — | 1.00x (841.7 s) |
//! | chord-length sampling | 1.35380 +/- 0.00267 | -3267 pcm | 9.6, **resolved** | **2.16x** |
//! | semi-implicit CLS | 1.35282 +/- 0.00228 | -3365 pcm | 10.8, **resolved** | **2.09x** |
//! | naive homogenisation | 1.34345 +/- 0.00234 | **-4302 pcm** | 13.7, **resolved** | 1.75x |
//! | ring-RPT (fitted annulus) | 1.38684 +/- 0.00252 | **+37 pcm** | 0.1, not resolved | **2.13x** |
//!
//! Combined standard error is ~320 pcm, so this run resolves a bias of roughly
//! 1000 pcm. A 9x-smaller run at 800 histories agreed on every speed ratio to
//! within 1 % and on every bias within its (much looser) statistics.
//!
//! ## Ring-RPT reproduces exact tracking; uniform smearing does not
//!
//! **+37 pcm at 0.1 sigma.** Ring-RPT is statistically indistinguishable from
//! the exact delta-tracked pebble, at statistics that would have resolved a
//! 1000 pcm bias comfortably. Filling the same zone uniformly instead costs a
//! **resolved -4302 pcm at 13.7 sigma**. The two differ by ~4340 pcm.
//!
//! That is the entire reason they are separate variants, and until 2026-09-14 a
//! single enum named `RingRpt` did the uniform smear — so this crate reported
//! the second number as the first. See the correction on [`DhTreatment`].
//!
//! **An independent check on the ring-RPT implementation**: the OpenMC deck
//! this geometry comes from measures its own ring-RPT pebble at **-31 pcm
//! (0.34 sigma)** from its own explicit pebble. We measure **+37 pcm (0.1
//! sigma)**. Same magnitude, opposite sign, both unresolved — two codes
//! independently finding the method faithful at the tens-of-pcm level. This
//! comparison is meaningful in a way the absolute one below is not, because
//! both sides are *treatment versus exact within one code*, so material and
//! packing differences cancel.
//!
//! ## CLS and SCLS both carry a real ~3300 pcm bias, and SCLS is not better
//!
//! Both are resolved beyond 9 sigma, so these are measurements rather than
//! bounds. SCLS's -3365 pcm sits within statistics of CLS's -3267 pcm: on this
//! problem the retention window buys **nothing measurable**.
//!
//! Do not read that as a property of SCLS yet. This crate's SCLS-to-k-eff
//! wiring resets its retention window through
//! [`MaterialQuery::begin_history`], and **that reset has not been verified to
//! reproduce standalone SCLS** (bead `op-we7r`). A window that is not being
//! cleared correctly would degrade SCLS towards CLS, which is exactly the
//! result seen. The two candidate explanations are not separated.
//!
//! Independently, `src/stochastic/benchmark.rs` finds CLS *closer* to an RSA
//! reference than SCLS at pf 0.2, with SCLS over-correcting past it — so
//! "SCLS does not win here" is at least consistent with what this crate has
//! measured elsewhere.
//!
//! ## The absolute OpenMC comparison is NOT a code-to-code verification
//!
//! The example prints delta tracking at **+2137 pcm (9.7 sigma)** from OpenMC's
//! published explicit-TRISO `k = 1.36510 +/- 0.00063`. **Do not read that as a
//! discrepancy between the two codes**, because the two calculations are not
//! modelling the same pebble:
//!
//! - **The materials here are illustrative, not the deck's.** They are hand-written
//!   atom densities chosen to represent the material class, as [`materials`]
//!   says; the reference deck derives its number densities from specified mass
//!   densities (graphite at 1.1995 g/cm3, FLiBe from its own `rho(T)`
//!   correlation, and so on). Nothing reconciles the two.
//! - **The packing fraction is off by -3.5 %.** [`DhUniverse::pebble`] requests
//!   pf 0.30 over a cube and keeps only whole particles inside the fuel sphere,
//!   which realises **0.2894** (25 856 particles x 4/3 pi r^3 = 8.314 cm3 in a
//!   28.731 cm3 zone). `examples/fhr_ring_rpt_endf.rs` measures this clip
//!   effect and rescales to hit its target; this constructor does not. Tracked
//!   as a bead.
//! - C-13, Si-29 and Si-30 are neglected here and present in the deck.
//!
//! Every one of those applies identically to all five arms, so none of them can
//! bias the *treatment comparison* this example exists to make — which is why
//! they were acceptable. They make the absolute number meaningless as
//! verification.
//!
//! **For genuine code-to-code against OpenMC on this pebble, use
//! `examples/fhr_ring_rpt_endf.rs`**, which builds the deck's materials and
//! geometry properly. The ring-RPT-vs-OpenMC-ring-RPT line the example prints
//! carries the same caveat, doubled: it inherits this offset *and* the
//! S(alpha,beta) difference described above.
//!
//! ## Correction: the "approximations are slower" finding was a bug in this crate
//!
//! An earlier revision of this file reported every approximate treatment as
//! **slower** than exact delta tracking, and concluded that the geometry-only
//! speedups do not transfer to eigenvalue calculations. **That was an artefact
//! of [`DhUniverse::keff`] bounding its majorant over the whole material
//! table**, including the undiluted kernel that no approximate treatment's
//! geometry can return. Delta tracking accepts a collision with probability
//! `sigma_t / sigma_maj`, so a majorant standing order 25x above anything the
//! smeared geometry contains multiplied the virtual-collision count — and the
//! runtime — by about the same factor.
//!
//! Bounding over reachable materials only moved the four approximate arms from
//! 0.91x / 0.88x / 0.69x / 0.92x to **2.16x / 2.09x / 1.75x / 2.13x**.
//!
//! The control is clean: delta tracking did **not** move, because its majorant
//! was already correct — the kernel really is in its geometry. Only the arms
//! whose majorant changed changed. No eigenvalue was biased by any of it: an
//! over-bound majorant is wasteful, only an under-bound one is wrong.
//!
//! ## What is left unexplained, and the leading hypothesis
//!
//! After the fix the two benchmarks roughly agree, where before they
//! contradicted each other. `examples/dh_tracking_speedup.rs` measures
//! per-history geometry cost at delta 0.52-0.75 us and CLS 0.21-0.23 us,
//! predicting CLS at **2.3-3.6x** delta; the eigenvalue run measures **2.16x**.
//! Slightly under, which is what adding real cross-section work on top of
//! geometry should do.
//!
//! **Naive homogenisation is the outlier**: that benchmark predicts 6.5-10.7x
//! and the eigenvalue run measures 1.75x.
//!
//! The leading hypothesis — consistent with the pattern, **not isolated** — is
//! nuclides evaluated per point query. Roughly 90 % of the explicit fuel zone
//! by volume is single-nuclide carbon, so delta tracking averages ~1.2
//! nuclides per query. CLS, SCLS and ring-RPT put ~30 % of queries in a
//! ~6-nuclide homogenised particle and the rest in single-nuclide matrix,
//! averaging ~2.5. Naive homogenisation puts **every** fuel-zone query in the
//! 6-nuclide smear. Predicted ratios 1.2 / 2.5 / 2.5 / 2.5 / 6.0 reproduce the
//! measured ordering — three arms clustered near 2.1x with naive alone
//! trailing at 1.75x.
//!
//! Separating that from everything else needs a profile or an A/B with matched
//! nuclide counts, and has **not** been run (bead `op-qolc`).
//!
//! ## CLS pays an avoidable cost on top
//!
//! Its sampler is stateful and the k-eff seam wants [`MaterialQuery`] to be
//! [`Sync`], so every point query takes a mutex; SCLS additionally clones its
//! retention window once per history. Both are artefacts of this wiring rather
//! than properties of the methods — GitHub issue #205 / bead `op-ifl3`. The
//! lock is **uncontended** on the single-threaded backend these numbers were
//! taken on, so it is probably a small part of what is left; its real damage
//! shows up when threads are turned on, which has not been run.
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
            .map(|(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    };

    // FLiBe = 2LiF.BeF2 = Li2BeF4. Density correlation rho(T) = (2416 -
    // 0.49072*T)/1000 g/cm^3, the same one the reference deck uses.
    let flibe_rho = (2416.0 - 0.49072 * TEMP_K) / 1000.0;
    let (m_li7, m_li6, m_be9, m_f19) = (7.016_003, 6.015_123, 9.012_183, 18.998_403);
    let li7_per_formula = 2.0 * LI7_PURITY;
    let li6_per_formula = 2.0 * (1.0 - LI7_PURITY);
    let m_formula = li7_per_formula * m_li7 + li6_per_formula * m_li6 + m_be9 + 4.0 * m_f19;
    // Formula units per barn-cm; multiply by the per-formula count for each.
    let n_formula = flibe_rho * N_A_B / m_formula;

    vec![
        // 0 kernel — 19.9 % HALEU UCO. Kernel carbon is not graphite.
        m(
            0,
            "UCO kernel",
            vec![
                (U235, 4.40e-3),
                (U238, 1.77e-2),
                (O16, 2.27e-2),
                (C_FREE, 9.10e-3),
            ],
        ),
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
        m(
            7,
            "FLiBe coolant",
            vec![
                (F19, 4.0 * n_formula),
                (LI7, li7_per_formula * n_formula),
                (LI6, li6_per_formula * n_formula),
                (BE9, n_formula),
            ],
        ),
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
    println!(
        "  reference  : OpenMC explicit TRISO k = {:.5} +/- {:.5}\n",
        OMC_EXPLICIT.0, OMC_EXPLICIT.1
    );

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
                DhTreatment::RingRpt {
                    inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER,
                }
            }
        }
    } else {
        println!(
            "(ring-RPT uses the OpenMC-fitted r_inner = {:.4} cm; set OUTRAM_DH_VV_FIT_RPT=1\n \
             to fit this code's own radius instead — about a dozen extra solves)\n",
            DhTreatment::FHR_REFERENCE_RPT_INNER
        );
        DhTreatment::RingRpt {
            inner_radius: DhTreatment::FHR_REFERENCE_RPT_INNER,
        }
    };

    let all_treatments = [
        DhTreatment::DeltaTracking,
        DhTreatment::ChordLength,
        DhTreatment::Scls,
        DhTreatment::Homogenised,
        rpt_treatment,
    ];
    // OUTRAM_DH_ONLY selects treatments by substring of `name()`, comma
    // separated (e.g. "chord" or "chord,delta"). Unset runs all five, which is
    // the unchanged default. Added so a single treatment can be re-measured
    // without paying for the others -- SCLS in particular is not fixed yet
    // (maintainer, 2026-09-18) and running it wastes the wall clock.
    let treatments: Vec<DhTreatment> = match std::env::var("OUTRAM_DH_ONLY") {
        Ok(filter) => {
            let want: Vec<String> = filter.split(',').map(|w| w.trim().to_lowercase()).collect();
            all_treatments
                .into_iter()
                .filter(|t| {
                    let n = t.name().to_lowercase();
                    want.iter().any(|w| n.contains(w.as_str()))
                })
                .collect()
        }
        Err(_) => all_treatments.to_vec(),
    };
    // OUTRAM_DH_SEEDS: independent seed draws per treatment (default 1).
    // Every DH number in this repo is a SINGLE draw; pooling is gh:#196 /
    // bn:op-awwi. Each draw is timed separately.
    let n_seeds: u64 = std::env::var("OUTRAM_DH_SEEDS")
        .ok().and_then(|v| v.parse().ok()).filter(|&n: &u64| n >= 1).unwrap_or(1);

    let mut rows: Vec<Row> = Vec::new();
    for treatment in treatments {
      let mut draws: Vec<f64> = Vec::with_capacity(n_seeds as usize);
      for draw in 1..=n_seeds {
        let params = PebbleParams::fhr_unit_cell().with_materials(mats.clone());
        let params = PebbleParams { seed: params.seed.wrapping_add(draw - 1) | 1, ..params };
        let settings = KeffSettings { seed: draw, ..settings.clone() };
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
        draws.push((result.k_mean - 1.0) * 0.0 + result.k_mean);
        if n_seeds > 1 {
            println!("      draw {draw}/{n_seeds}: k = {:.5} +/- {:.5}  ({secs:.1} s)",
                     result.k_mean, result.k_std);
        }
        if draw == n_seeds {
            rows.push(Row { treatment, k: result.k_mean, std: result.k_std, secs, particles });
        }
      }
      if n_seeds > 1 && !draws.is_empty() {
          let n = draws.len() as f64;
          let mean = draws.iter().sum::<f64>() / n;
          let sd = if n > 1.0 {
              (draws.iter().map(|k| (k - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
          } else { 0.0 };
          let sem = if n > 1.0 { sd / n.sqrt() } else { 0.0 };
          println!("    POOLED {:<24} k = {mean:.5}  sd = {:.5}  sem = +/-{:.5}  ({} draws)",
                   treatment.name(), sd, sem, draws.len());
      }
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
            if z >= 3.0 {
                "RESOLVED bias"
            } else {
                "not resolved at this n"
            }
        );
    }

    // The absolute comparison against OpenMC lives in the example that actually
    // models the deck. Printing this example's own delta arm against the
    // published number invites it to be quoted as a code-to-code result, which
    // it is not: the materials here are illustrative and the realised packing
    // fraction is 0.2894 rather than 0.30. Both offsets are stated in the doc
    // comment; what is printed is the pointer, not the number.
    println!("\n=== Code-to-code against OpenMC: see examples/fhr_ring_rpt_endf.rs ===");
    println!(
        "  That deck builds the reference materials and geometry properly and measures\n  \
         explicit TRISO, delta-tracked, at k = 1.36547 +/- 0.00229 against OpenMC's\n  \
         {:.5} +/- {:.5} — {:+} pcm ({:.1} sigma). Its ring-RPT arm reads {:+} pcm\n  \
         ({:.1} sigma) against OpenMC's {:.5} +/- {:.5}.",
        OMC_EXPLICIT.0, OMC_EXPLICIT.1, 37, 0.2, -116, 0.5, OMC_RING_RPT.0, OMC_RING_RPT.1
    );
    println!(
        "  Both are SINGLE DRAWS carrying ~230 pcm of statistics; a same-day re-run of\n  \
         the explicit case gave -466 pcm, which is 1.5 sigma of ordinary noise rather\n  \
         than a change. Pooling over seeds is gh:#196 / bn:op-awwi, and is not done.\n  \
         Source: verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md"
    );
    println!(
        "\n  THIS example's arms are NOT comparable to those numbers: illustrative\n  \
         materials, and a realised packing fraction of 0.2894 against a requested\n  \
         0.30. Its delta arm sits {:+.0} pcm from the OpenMC explicit result for those\n  \
         reasons, not because the two codes disagree. Use it for the treatment\n  \
         comparison above, which shares every one of those offsets across all arms.",
        delta(reference.k, reference.std, OMC_EXPLICIT.0, OMC_EXPLICIT.1).0
    );

    println!("\n=== Accuracy bought per unit speed ===");
    for row in &rows {
        let speed = reference.secs / row.secs;
        if row.treatment.is_exact() {
            println!(
                "  {:<28} {:>5.2}x   (reference)",
                row.treatment.name(),
                speed
            );
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
