//! **Ring-RPT code-to-code verification** — the explicit-TRISO vs ring-RPT FHR
//! pebble, run through `outram-mc-libs` on HIGH-fidelity ENDF/B-VIII.0 data and
//! compared against the OpenMC reference (`op-mzvp`, GitHub #156).
//!
//! Gated behind the `endf-pebble-cases` feature (it needs the reference ENDF
//! tapes in `reference-data/endf/` and reconstructs ~13 nuclides, so it is not
//! part of the default `cargo test --examples`):
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example fhr_ring_rpt_endf
//! ```
//!
//! # What it computes
//!
//! - **Explicit fuel-zone k∞** — five-layer TRISO particles randomly packed
//!   (30 % by volume) in a graphite matrix cube, delta (Woodcock) tracking.
//! - **Homogenised fuel-zone k∞** — the same TRISO material dissolved into one
//!   medium ([`homogenise_by_volume`]) filling the same cube.
//! - **Ring-RPT pebble** — the homogenised fuel as a spherical shell (inner
//!   radius 1.493359375 cm, volume = total particle volume), graphite + FLiBe,
//!   reflective; six-factor decomposition + lethargy spectrum.
//!
//! The RPT-equivalence quantity is the **fuel-zone k∞ difference**
//! `homogenised − explicit`; RPT works if it is small. That difference is
//! compared against OpenMC's `ring_rpt − explicit` = −31 ± 92 pcm.
//!
//! # Caveats (this run)
//!
//! 1. **No graphite S(α,β) yet** — the matrix/coating/shell carbon is free-gas
//!    here, bound `c_Graphite` in the OpenMC deck. `tests/htr10_graphite_thermal
//!    _scattering_pebble_bed.rs` measures ~1700 pcm for that difference in a
//!    graphite pebble bed, so the *absolute* k values will sit well above
//!    OpenMC's. The RPT *difference* is far less sensitive (common-mode).
//!    Adding S(α,β) is `op-mzvp.2.8` follow-up.
//! 2. **Charged-particle absorption** — Li-6(n,t) is under-counted (`op-mzvp.2.10`);
//!    small at 99.995 % Li-7.
//! 3. Not a validated result — an AI-assisted code-to-code check.

#[cfg(target_os = "android")]
fn main() {
    eprintln!(
        "fhr_ring_rpt_endf: desktop-only (reads multi-MB ENDF tapes from the repo's \
         reference-data/, which a device build does not carry)"
    );
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::compute::ComputeType;
    use outram_mc_libs::physics::keff::KeffSettings;
    use outram_mc_libs::physics::reactor_physics::{
        run_keff_reactor_physics, ReactorPhysicsConfig,
    };
    use outram_mc_libs::physics::transport_csg::SourceBox;
    use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
    use outram_mc_libs::pebble_beds::fhr_pebble::{
        homogeneous_cube, homogenise_by_volume, triso_layer_at, TrisoLayer, TrisoSpec,
    };
    use outram_mc_libs::pebble_beds::crp_packing::pack_spheres_crp;
    use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
    use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
    use outram_mc_libs::geometry::position::Position;
    use std::path::PathBuf;

    const AVOGADRO_1E24: f64 = 0.602_214_076; // N_A / 1e24 [atoms·mol⁻¹ / 1e24]
    const TEMP_K: f64 = 600.0;
    const ENRICH: f64 = 0.199; // U-235 atom fraction of uranium
    const LI7_PURITY: f64 = 0.99995;

    // Atomic masses [u].
    const M_U235: f64 = 235.0439;
    const M_U238: f64 = 238.0508;
    const M_O16: f64 = 15.9949;
    const M_C12: f64 = 12.0000;
    const M_C13: f64 = 13.0034;
    const M_SI28: f64 = 27.9769;
    const M_SI29: f64 = 28.9765;
    const M_SI30: f64 = 29.9738;
    const M_F19: f64 = 18.9984;
    const M_LI6: f64 = 6.0151;
    const M_LI7: f64 = 7.0160;
    const M_BE9: f64 = 9.0122;

    // Natural isotopic abundances.
    const C12_AB: f64 = 0.9893;
    const C13_AB: f64 = 0.0107;
    const SI28_AB: f64 = 0.92223;
    const SI29_AB: f64 = 0.04685;
    const SI30_AB: f64 = 0.03092;

    /// Nuclide-array indices.
    mod nx {
        pub const U235: usize = 0;
        pub const U238: usize = 1;
        pub const O16: usize = 2;
        pub const C12: usize = 3; // free-gas (fuel kernel)
        pub const C13: usize = 4;
        pub const C12G: usize = 5; // bound graphite (coatings, matrix, shell) — S(α,β) TODO
        pub const C13G: usize = 6;
        pub const SI28: usize = 7;
        pub const SI29: usize = 8;
        pub const SI30: usize = 9;
        pub const F19: usize = 10;
        pub const LI6: usize = 11;
        pub const LI7: usize = 12;
        pub const BE9: usize = 13;
    }

    fn endf_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("reference-data")
            .join("endf")
    }

    fn load(name: &str, file: &str) -> Nuclide {
        let p = endf_dir().join(file);
        eprint!("  reconstructing {name:<6} … ");
        let t0 = std::time::Instant::now();
        let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
        eprintln!("{:.1?}", t0.elapsed());
        n
    }

    fn nuclides() -> Vec<Nuclide> {
        // C12G / C13G are the *same* reconstruction as C12 / C13 for now — the
        // bound-graphite S(α,β) overlay is op-mzvp.2.8 follow-up.
        vec![
            load("U235", "n-092_U_235-ENDF8.0.endf"),
            load("U238", "n-092_U_238.endf"),
            load("O16", "n-008_O_016-ENDF8.0.endf"),
            load("C12", "n-006_C_012-ENDF8.0.endf"),
            load("C13", "n-006_C_013-ENDF8.0.endf"),
            load("C12", "n-006_C_012-ENDF8.0.endf"),
            load("C13", "n-006_C_013-ENDF8.0.endf"),
            load("Si28", "n-014_Si_028-ENDF8.0.endf"),
            load("Si29", "n-014_Si_029-ENDF8.0.endf"),
            load("Si30", "n-014_Si_030-ENDF8.0.endf"),
            load("F19", "n-009_F_019-ENDF8.0.endf"),
            load("Li6", "n-003_Li_006-ENDF8.0.endf"),
            load("Li7", "n-003_Li_007-ENDF8.0.endf"),
            load("Be9", "n-004_Be_009-ENDF8.0.endf"),
        ]
    }

    /// Atom-density components \[atoms/barn·cm\] for a compound of mass density
    /// `rho` \[g/cm³\] with per-nuclide `(index, atom_fraction, mass_u)`.
    fn number_densities(rho: f64, fracs: &[(usize, f64, f64)]) -> Vec<NuclideComponent> {
        let m_bar: f64 = fracs.iter().map(|&(_, f, m)| f * m).sum();
        let n_tot = rho * AVOGADRO_1E24 / m_bar; // atoms/barn·cm
        fracs
            .iter()
            .map(|&(nuclide_idx, f, _)| NuclideComponent {
                nuclide_idx,
                atom_density: f * n_tot,
            })
            .collect()
    }

    fn carbon(idx12: usize, idx13: usize, frac: f64) -> [(usize, f64, f64); 2] {
        [
            (idx12, frac * C12_AB, M_C12),
            (idx13, frac * C13_AB, M_C13),
        ]
    }

    fn build_materials() -> (Vec<Material>, TrisoSpec) {
        let spec = TrisoSpec::FHR_HALEU_UCO;

        // ── fuel: UCO kernel, ρ = 10.5, atom fractions O16 0.5, C 1/6, U rest ──
        let u_frac = 1.0 - 0.5 - 1.0 / 6.0;
        let c = carbon(nx::C12, nx::C13, 1.0 / 6.0);
        let fuel = Material {
            id: 1,
            name: "UCO fuel kernel".into(),
            temperature: TEMP_K,
            components: number_densities(
                10.5,
                &[
                    (nx::U235, ENRICH * u_frac, M_U235),
                    (nx::U238, (1.0 - ENRICH) * u_frac, M_U238),
                    (nx::O16, 0.5, M_O16),
                    c[0],
                    c[1],
                ],
            ),
        };

        let graphite_carbon = |rho: f64| {
            let c = carbon(nx::C12G, nx::C13G, 1.0);
            number_densities(rho, &c)
        };
        let buffer = Material { id: 2, name: "buffer".into(), temperature: TEMP_K, components: graphite_carbon(1.0) };
        let pyc1 = Material { id: 3, name: "PyC1".into(), temperature: TEMP_K, components: graphite_carbon(1.9) };
        let pyc2 = Material { id: 4, name: "PyC2".into(), temperature: TEMP_K, components: graphite_carbon(1.87) };

        // ── SiC: ρ = 3.2, C 0.5 + Si-nat 0.5 ──
        let sic = Material {
            id: 5,
            name: "SiC".into(),
            temperature: TEMP_K,
            components: number_densities(
                3.2,
                &[
                    (nx::C12G, 0.5 * C12_AB, M_C12),
                    (nx::C13G, 0.5 * C13_AB, M_C13),
                    (nx::SI28, 0.5 * SI28_AB, M_SI28),
                    (nx::SI29, 0.5 * SI29_AB, M_SI29),
                    (nx::SI30, 0.5 * SI30_AB, M_SI30),
                ],
            ),
        };

        let graphite = Material { id: 6, name: "graphite matrix/shell".into(), temperature: TEMP_K, components: graphite_carbon(1.1995) };

        // ── FLiBe: 2LiF·BeF2, ρ(T) = (2416 − 0.49072·T)/1000 ──
        let flibe_rho = (2416.0 - 0.49072 * TEMP_K) / 1000.0;
        let li7 = 2.0 * LI7_PURITY;
        let li6 = 2.0 * (1.0 - LI7_PURITY);
        let s = 4.0 + li7 + li6 + 1.0;
        let flibe = Material {
            id: 7,
            name: "FLiBe".into(),
            temperature: TEMP_K,
            components: number_densities(
                flibe_rho,
                &[
                    (nx::F19, 4.0 / s, M_F19),
                    (nx::LI7, li7 / s, M_LI7),
                    (nx::LI6, li6 / s, M_LI6),
                    (nx::BE9, 1.0 / s, M_BE9),
                ],
            ),
        };

        // ── homogenised TRISO fuel: volume-weighted mix of the 5 layers ──
        let vols = spec.layer_volumes();
        let homog = homogenise_by_volume(
            &[
                (&fuel, vols[0]),
                (&buffer, vols[1]),
                (&pyc1, vols[2]),
                (&sic, vols[3]),
                (&pyc2, vols[4]),
            ],
            8,
            "homogenised TRISO fuel".into(),
            TEMP_K,
        );

        (
            vec![fuel, buffer, pyc1, pyc2, sic, graphite, flibe, homog],
            spec,
        )
    }

    mod mi {
        pub const FUEL: usize = 0;
        pub const BUFFER: usize = 1;
        pub const PYC1: usize = 2;
        pub const PYC2: usize = 3;
        pub const SIC: usize = 4;
        pub const GRAPHITE: usize = 5;
        pub const HOMOG: usize = 7;
    }

    fn layer_material(layer: TrisoLayer) -> usize {
        match layer {
            TrisoLayer::Kernel => mi::FUEL,
            TrisoLayer::Buffer => mi::BUFFER,
            TrisoLayer::Ipyc => mi::PYC1,
            TrisoLayer::Sic => mi::SIC,
            TrisoLayer::Opyc => mi::PYC2,
        }
    }

    pub fn run() {
        eprintln!("=== FHR ring-RPT vs explicit-TRISO — outram-mc-libs on ENDF/B-VIII.0 ===\n");
        eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
        let nucs = nuclides();
        let (mats, spec) = build_materials();
        eprintln!();

        let compute = ComputeType::CpuMultiThread(Default::default());
        let keff = KeffSettings {
            n_particles: 4000,
            n_inactive: 30,
            n_active: 80,
            temperature_k: TEMP_K,
            compute,
            ..KeffSettings::default()
        };

        // ── 1. Explicit fuel-zone k∞ (delta tracking) ──
        let h = 0.35_f64; // 0.7 cm cube — ~320 TRISO particles at pf 0.3
        let seed = 20_260_910;
        // Collective-rearrangement packing reaches 0.30 (RSA saturates below it).
        let spheres = pack_spheres_crp(spec.opyc, h, spec.packing_fraction, seed)
            .expect("TRISO CRP packing");
        let packed = PackedSpheres::from_spheres(spheres, h, spec.opyc);
        eprintln!(
            "Explicit fuel zone: {} TRISO particles, realized pf {:.4}, cube half-width {h} cm",
            packed.len(),
            packed.packing_fraction()
        );
        // The delta run sees the full material array; `material_at` returns the
        // `mi::*` index directly, so kernel..OPyC + graphite matrix are all in
        // scope and the majorant bounds every one of them.
        let majorant = Majorant::bounding(&mats, &nucs, 1.0e-4, 2.0e7, 4096, 32, 0.1);
        let material_at = |p: Position| -> Option<usize> {
            match packed.containing_center(p) {
                Some(c) => triso_layer_at((p - c).norm(), &spec).map(layer_material),
                None => Some(mi::GRAPHITE),
            }
        };
        let explicit = run_keff_delta(h, &mats, &nucs, &majorant, material_at, &keff);
        eprintln!(
            "  k∞ (explicit TRISO)     = {:.5} ± {:.5}\n",
            explicit.k_mean, explicit.k_std
        );

        // ── 2. Homogenised fuel-zone k∞ (same cube, one medium) ──
        let homog_only = vec![mats[mi::HOMOG].clone()];
        let cube = homogeneous_cube(h, 0, TEMP_K);
        let src = SourceBox {
            lower: Position::new(-h, -h, -h),
            upper: Position::new(h, h, h),
        };
        let rp_cfg = ReactorPhysicsConfig {
            keff: keff.clone(),
            source_box: src,
            ..Default::default()
        };
        let homog_cube = run_keff_reactor_physics(&cube, &homog_only, &nucs, &rp_cfg)
            .expect("homogenised cube reactor physics");
        eprintln!(
            "  k∞ (homogenised)        = {:.5} ± {:.5}",
            homog_cube.keff.k_mean, homog_cube.keff.k_std
        );

        let d_pcm = (homog_cube.keff.k_mean - explicit.k_mean) * 1.0e5;
        let sigma_pcm =
            (explicit.k_std.powi(2) + homog_cube.keff.k_std.powi(2)).sqrt() * 1.0e5;
        eprintln!(
            "  RPT equivalence: homogenised − explicit = {:+.0} pcm  (combined σ {:.0} pcm, {:.2}σ)",
            d_pcm,
            sigma_pcm,
            d_pcm.abs() / sigma_pcm.max(1.0)
        );
        print_six_factors("homogenised fuel zone", &homog_cube);

        // ── 3. Ring-RPT shell fuel-zone k∞ (delta tracking, radius-classified) ──
        // The homogenised fuel occupies a spherical shell r ∈ (r_inner, r_shell)
        // sized so its volume = the total particle volume, graphite elsewhere.
        // Same reflective cube domain as (1) and (2). Solved with delta tracking
        // to avoid `op-mzvp.2.11` (run_keff_csg leaks on concentric spheres).
        let r_inner_fz = 0.20_f64; // fits inside the h = 0.35 cube
        let r_shell_fz = (r_inner_fz.powi(3) + spec.packing_fraction * (0.85_f64).powi(3)).cbrt();
        let rpt_mats = vec![mats[mi::HOMOG].clone(), mats[mi::GRAPHITE].clone()];
        let rpt_majorant = Majorant::bounding(&rpt_mats, &nucs, 1.0e-4, 2.0e7, 4096, 32, 0.1);
        let rpt_at = move |p: Position| -> Option<usize> {
            let r = p.norm();
            if r > r_inner_fz && r < r_shell_fz {
                Some(0) // homogenised fuel shell
            } else {
                Some(1) // graphite
            }
        };
        let rpt = run_keff_delta(h, &rpt_mats, &nucs, &rpt_majorant, rpt_at, &keff);
        eprintln!(
            "\n  k∞ (ring-RPT shell)     = {:.5} ± {:.5}   (fuel shell {r_inner_fz:.3}–{r_shell_fz:.3} cm)",
            rpt.k_mean, rpt.k_std
        );

        // ── Summary ──
        let delta = |k: f64, s: f64| {
            let dk = (k - explicit.k_mean) * 1e5;
            let cs = (s.powi(2) + explicit.k_std.powi(2)).sqrt() * 1e5;
            (dk, dk.abs() / cs.max(1.0))
        };
        let (d_nh, z_nh) = delta(homog_cube.keff.k_mean, homog_cube.keff.k_std);
        let (d_rp, z_rp) = delta(rpt.k_mean, rpt.k_std);
        eprintln!("\n── outram-mc-libs fuel-zone k∞ (ENDF/B-VIII.0, free-gas C) ──");
        eprintln!("  explicit TRISO        : {:.5} ± {:.5}", explicit.k_mean, explicit.k_std);
        eprintln!(
            "  naive homogenised     : {:.5} ± {:.5}   Δ(naive−explicit) = {d_nh:+.0} pcm  ({z_nh:.1}σ)",
            homog_cube.keff.k_mean, homog_cube.keff.k_std
        );
        eprintln!(
            "  ring-RPT shell        : {:.5} ± {:.5}   Δ(RPT−explicit)   = {d_rp:+.0} pcm  ({z_rp:.1}σ)",
            rpt.k_mean, rpt.k_std
        );

        // ── OpenMC reference (op-mzvp.1, GH #156) ──
        eprintln!("\n── OpenMC full-pebble reference (op-mzvp.1, ENDF/B-VIII.0, c_Graphite) ──");
        eprintln!("  explicit TRISO pebble : k = 1.36510 ± 0.00063");
        eprintln!("  ring-RPT pebble       : k = 1.36479 ± 0.00067   Δ(RPT−explicit) = −31 pcm (0.34σ)");
        eprintln!("  η/f/p/ε (explicit)    : 2.0158 / 0.9172 / 0.4837 / 1.5064");
        eprintln!(
            "\nCAVEATS:\n  • free-gas carbon (no c_Graphite S(α,β)) — op-mzvp.2.8; absolute k not \
             comparable to OpenMC.\n  • fuel-zone k∞ in a 0.7 cm reflective cube, NOT the full \
             graphite+FLiBe pebble — op-mzvp.2.11 blocks the CSG pebble geometry.\n  • Li-6(n,t) \
             absorption under-counted — op-mzvp.2.10 (tiny at 99.995 % Li-7).\n  • AI-assisted \
             code-to-code check, not a validated result."
        );
    }

    fn print_six_factors(
        tag: &str,
        r: &outram_mc_libs::physics::reactor_physics::ReactorPhysicsReport,
    ) {
        let s = &r.six_factors;
        eprintln!(
            "  [{tag}] η {:.4} f {:.4} p {:.4} ε {:.4} P_FNL {:.4} P_TNL {:.4} | k_4f {:.5} \
             leak {:.4} gap {:+.2}% consistent={}",
            s.eta.mean,
            s.f.mean,
            s.p.mean,
            s.epsilon.mean,
            s.p_fnl.mean,
            s.p_tnl.mean,
            s.k_from_factors.mean,
            r.leakage_total.mean,
            r.consistency_gap * 100.0,
            r.consistent
        );
    }
}
