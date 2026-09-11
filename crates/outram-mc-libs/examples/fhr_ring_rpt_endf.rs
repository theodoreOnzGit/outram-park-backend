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
//! Add `--search-rpt-radius` to additionally solve for *this code's own*
//! RPT inner radius (see "The radius is a fitted parameter" below). That mode
//! costs several more eigenvalue solves, so it is opt-in:
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example fhr_ring_rpt_endf -- --search-rpt-radius
//! ```
//!
//! # What it computes (full 4 cm pebble, 0.1 cm graphite shell, FLiBe, 600 K)
//!
//! - **Explicit-TRISO pebble** — five-layer TRISO particles randomly packed
//!   (30 % by volume) in the r < 1.9 cm graphite fuel zone; delta (Woodcock)
//!   tracking on a reflective sphere of radius 3 — the same boundary the
//!   OpenMC deck uses, so the absolute k is comparable and not only the delta.
//! - **Ring-RPT pebble** — the TRISO material [`homogenise_by_volume`]-dissolved
//!   into a spherical shell (inner radius 1.493359375 cm, volume = total
//!   particle volume). Run through the same reflective sphere as the explicit
//!   case, and also through the real [`fhr_pebble_geometry`] CSG sphere (which
//!   adds the six-factor decomposition and the spectrum).
//! - **Naive homogenisation** — the homogenised fuel filling the whole r < 1.9
//!   zone; the `naive − explicit` gap is the double-heterogeneity error RPT
//!   exists to remove.
//!
//! Reference: OpenMC `ring_rpt − explicit = −31 ± 92 pcm` (`op-mzvp.1`, #156).
//!
//! # Caveats
//!
//! 1. **Graphite S(α,β)** is on (`c_Graphite`, ENDF/B-VIII.0 crystalline
//!    graphite) for the coating / matrix / shell carbon; the fuel-kernel carbon
//!    stays free-gas, as in the OpenMC deck.
//! 2. **Every reported comparison now uses the reflective sphere** r = 3, the
//!    boundary the OpenMC deck used. A reflective *cube* of half-width 3 holds
//!    FLiBe in its corners (3 < r < 3√3) that the sphere does not; the cube rows
//!    are still printed, but only to price that over-count.
//! 3. **The RPT inner radius is a fitted parameter, and 1.4934 cm was fitted
//!    against OpenMC.** Quoting `RPT − explicit` at a radius tuned for another
//!    code measures that code's fit, not this one's method error. Run with
//!    `--search-rpt-radius` to solve for the radius at which *this* code's
//!    ring-RPT pebble reproduces *this* code's explicit-TRISO pebble; the gap
//!    between that radius and 1.4934 cm is the honest statement of how
//!    code-dependent the fit is.
//! 4. Not a validated result — an AI-assisted code-to-code check.

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
    use outram_mc_libs::physics::reactor_physics::{run_keff_reactor_physics, ReactorPhysicsConfig};
    use outram_mc_libs::physics::transport_csg::SourceBox;
    use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
    use outram_mc_libs::pebble_beds::fhr_pebble::{
        fhr_pebble_geometry, homogenise_by_volume, rpt_fuel_outer_radius, ExplicitTrisoPebble,
        TrisoSpec,
    };
    use outram_mc_libs::pebble_beds::crp_packing::pack_spheres_crp;
    use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain};
    use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
    use outram_mc_libs::geometry::triso_particle::TrisoMaterials;
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::BoundaryType;
    use outram_mc_libs::material::thermal::ThermalScattering;
    use std::path::PathBuf;

    /// Full-pebble radii from the OpenMC deck (`openmc_inputs/triso_pebble.py`,
    /// `rpt_pebble.py`): 4 cm pebble, 0.1 cm graphite shell.
    const R_FUEL_ZONE: f64 = 1.9; // 2.0 − shell; TRISO-in-graphite (explicit) / homog fuel (naive)
    const R_PEBBLE: f64 = 2.0; //  graphite shell outer
    const R_ROOT: f64 = 3.0; //   reflective boundary
    const R_RPT_INNER: f64 = 1.493_359_375; // ring-RPT inner graphite ball

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
        pub const C12: usize = 3; // free-gas (fuel kernel + SiC coating)
        pub const C13: usize = 4;
        pub const C12G: usize = 5; // crystalline-graphite S(α,β) — buffer, PyC, matrix, shell
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
        // C12 / C13 (fuel kernel) stay free-gas. C12G / C13G (buffer, PyC,
        // matrix, shell) carry the ENDF/B-VIII.0 crystalline-graphite S(α,β)
        // law — below ~4 eV they scatter off the bound lattice (coherent Bragg
        // + incoherent inelastic) instead of a free carbon atom. This is
        // `c_Graphite` in the OpenMC deck; without it a graphite-moderated
        // spectrum sits ~1700 pcm too high (tests/htr10_graphite_thermal_scatt…).
        let sab = graphite_sab();
        eprintln!("  graphite S(α,β): ENDF/B-VIII.0 tsl-crystalline-graphite (MAT 30) … ok");
        vec![
            load("U235", "n-092_U_235-ENDF8.0.endf"),
            load("U238", "n-092_U_238.endf"),
            load("O16", "n-008_O_016-ENDF8.0.endf"),
            load("C12", "n-006_C_012-ENDF8.0.endf"),
            load("C13", "n-006_C_013-ENDF8.0.endf"),
            load("C12", "n-006_C_012-ENDF8.0.endf").with_thermal_scattering(sab.clone()),
            load("C13", "n-006_C_013-ENDF8.0.endf").with_thermal_scattering(sab),
            load("Si28", "n-014_Si_028-ENDF8.0.endf"),
            load("Si29", "n-014_Si_029-ENDF8.0.endf"),
            load("Si30", "n-014_Si_030-ENDF8.0.endf"),
            load("F19", "n-009_F_019-ENDF8.0.endf"),
            load("Li6", "n-003_Li_006-ENDF8.0.endf"),
            load("Li7", "n-003_Li_007-ENDF8.0.endf"),
            load("Be9", "n-004_Be_009-ENDF8.0.endf"),
        ]
    }

    fn graphite_sab() -> ThermalScattering {
        ThermalScattering::from_endf_file(
            endf_dir()
                .join("tsl-crystalline-graphite.endf")
                .to_str()
                .expect("valid UTF-8 path"),
            30, // MAT 30 — C in crystalline graphite (ENDF/B-VIII.0)
            TEMP_K,
            "c_Graphite",
        )
        .expect("crystalline-graphite S(α,β) from reference-data/endf/")
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
        [(idx12, frac * C12_AB, M_C12), (idx13, frac * C13_AB, M_C13)]
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
        let buffer = Material {
            id: 2,
            name: "buffer".into(),
            temperature: TEMP_K,
            components: graphite_carbon(1.0),
        };
        let pyc1 = Material {
            id: 3,
            name: "PyC1".into(),
            temperature: TEMP_K,
            components: graphite_carbon(1.9),
        };
        let pyc2 = Material {
            id: 4,
            name: "PyC2".into(),
            temperature: TEMP_K,
            components: graphite_carbon(1.87),
        };

        // ── SiC: ρ = 3.2, C 0.5 + Si-nat 0.5. Free-gas C (nx::C12/C13), NOT
        // the bound-graphite law: the OpenMC deck treats SiC as free-gas
        // (`SiC.add_element('C', 0.5)` with no `add_s_alpha_beta`), and a
        // dedicated `tsl-CinSiC`/`tsl-SiinSiC` law is used by neither code.
        // SiC is a 35 µm coating between fuel and moderator, so its thermal
        // treatment is a minor contributor — matched to the deck here. ──
        let sic = Material {
            id: 5,
            name: "SiC".into(),
            temperature: TEMP_K,
            components: number_densities(
                3.2,
                &[
                    (nx::C12, 0.5 * C12_AB, M_C12),
                    (nx::C13, 0.5 * C13_AB, M_C13),
                    (nx::SI28, 0.5 * SI28_AB, M_SI28),
                    (nx::SI29, 0.5 * SI29_AB, M_SI29),
                    (nx::SI30, 0.5 * SI30_AB, M_SI30),
                ],
            ),
        };

        let graphite = Material {
            id: 6,
            name: "graphite matrix/shell".into(),
            temperature: TEMP_K,
            components: graphite_carbon(1.1995),
        };

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

        // ── naive-homogenisation medium: the TRISO material AND the matrix
        //    graphite it is dispersed in, smeared over the whole fuel zone ──
        //
        // `homog` above is the five TRISO layers alone, which is exactly right
        // for the ring-RPT shell because `rpt_fuel_outer_radius` sizes that
        // shell to the total particle volume (pf * r_zone^3). It is NOT right
        // for the naive case, which fills the *entire* r < 1.9 zone: the
        // particles occupy only `pf` of that zone, so filling it with pure
        // particle material gives 1/pf = 3.33x the heavy-metal inventory and
        // removes the 70 %-by-volume graphite matrix that moderates the
        // explicit pebble from the inside. That is not a homogenisation of the
        // explicit pebble, it is a different reactor, and comparing the two
        // measures nothing about double heterogeneity.
        //
        // Mixing at the packing fraction conserves both. `homogenise_by_volume`
        // is linear in atom density, so mixing the already-mixed `homog` with
        // graphite is identical to mixing all six constituents at once.
        let naive_homog = homogenise_by_volume(
            &[
                (&homog, spec.packing_fraction),
                (&graphite, 1.0 - spec.packing_fraction),
            ],
            9,
            "naive-homogenised fuel zone (TRISO + matrix)".into(),
            TEMP_K,
        );

        (
            vec![
                fuel,
                buffer,
                pyc1,
                pyc2,
                sic,
                graphite,
                flibe,
                homog,
                naive_homog,
            ],
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
        pub const FLIBE: usize = 6;
        pub const HOMOG: usize = 7;
        /// TRISO **plus matrix graphite** at the packing fraction — the medium
        /// the naive case must use, so that smearing the fuel zone conserves
        /// the explicit pebble's inventory instead of tripling it.
        pub const NAIVE_HOMOG: usize = 8;
    }

    pub fn run() {
        let search_radius = std::env::args().any(|a| a == "--search-rpt-radius");
        let only = std::env::var("OUTRAM_RINGRPT_ONLY").unwrap_or_default();
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
        // Full-pebble delta domains. The OpenMC deck clips the coolant at a
        // reflective SPHERE r = 3, so `ball` is the matched domain and every
        // absolute comparison against OpenMC uses it.
        //
        // `cube` is kept alongside it deliberately, not as a leftover: its
        // corners (3 < r < 3√3 ≈ 5.196) carry FLiBe that the sphere does not, so
        // the cube-vs-sphere difference on the *same* pebble measures what that
        // extra coolant is worth. Reporting both is what lets a reader separate
        // "our physics differs from OpenMC" from "our domain differed".
        let half = R_ROOT;
        let ball = DeltaDomain::Sphere { radius: R_ROOT };
        let cube = DeltaDomain::Cube { half };
        let seed = 20_260_910;

        // TRISO packed into a cube that fully covers the r < 1.9 fuel sphere, then
        // clipped to that sphere by `ExplicitTrisoPebble::material_at`.
        //
        // The packing fraction has to be corrected for the clip, and the size of
        // the correction is not negligible. `pack_spheres_crp` targets `pf` over
        // the *cube*, but centres are confined to `half − r_particle`, so the
        // interior is denser than nominal and the inscribed fuel sphere inherits
        // that: at face value the r < 1.9 sphere comes out at **pf 0.3072, +2.4 %
        // over the deck's 0.30** — 2.4 % more heavy metal than the model being
        // compared against, which specifies its packing fraction over the fuel
        // *region* (`openmc.model.pack_spheres(..., region=-fuel_sph, pf=0.30)`).
        //
        // The realised in-sphere fraction is linear in the particle count and so
        // in the requested cube `pf`, so one measured rescale lands on target.
        let pack_half = R_FUEL_ZONE + spec.opyc;
        let pack_at = |pf: f64| {
            let spheres =
                pack_spheres_crp(spec.opyc, pack_half, pf, seed).expect("TRISO CRP packing");
            PackedSpheres::from_spheres(spheres, pack_half, spec.opyc)
        };
        let trial = pack_at(spec.packing_fraction);
        let realised = trial.volume_fraction_in_ball(R_FUEL_ZONE, 400_000, 0xC0FFEE);
        let corrected_pf = spec.packing_fraction * spec.packing_fraction / realised;
        let packed = pack_at(corrected_pf);
        let achieved = packed.volume_fraction_in_ball(R_FUEL_ZONE, 400_000, 0xC0FFEE);
        eprintln!(
            "TRISO packing: cube pf {:.5} would give {realised:.5} in r < {R_FUEL_ZONE}; \
             requested {corrected_pf:.5} instead, achieved {achieved:.5} (deck: {:.5})",
            spec.packing_fraction, spec.packing_fraction
        );
        eprintln!(
            "Explicit fuel zone: {} TRISO particles packed in the r < {pack_half} cm cube; \
             ~{} of them lie in the r < {R_FUEL_ZONE} cm fuel sphere, which carries \
             pf {achieved:.4}",
            packed.len(),
            (packed.len() as f64 * (4.0 / 3.0 * std::f64::consts::PI * R_FUEL_ZONE.powi(3))
                / (2.0 * pack_half).powi(3))
            .round() as usize
        );

        let r_rpt_fuel = rpt_fuel_outer_radius(R_RPT_INNER, R_FUEL_ZONE, spec.packing_fraction);
        let majorant = Majorant::bounding(&mats, &nucs, 1.0e-4, 2.0e7, 4096, 32, 0.3);

        // ── Zone lookups (radius from the pebble centre) ──
        let zone_outside_fuel = |r: f64| -> usize {
            if r < R_PEBBLE {
                mi::GRAPHITE // graphite shell
            } else {
                mi::FLIBE // coolant (+ cube corners)
            }
        };

        // `OUTRAM_RINGRPT_ONLY=csg` skips the three delta-tracked pebbles and
        // runs only the surface-tracked CSG case, which is the one that yields
        // the six-factor decomposition. ~5 min instead of ~40.
        if only == "csg" {
            let r_rpt_fuel_csg =
                rpt_fuel_outer_radius(R_RPT_INNER, R_FUEL_ZONE, spec.packing_fraction);
            let pebble_csg = fhr_pebble_geometry(
                R_RPT_INNER,
                r_rpt_fuel_csg,
                R_PEBBLE,
                R_ROOT,
                mi::HOMOG,
                mi::GRAPHITE,
                mi::FLIBE,
                BoundaryType::Reflective,
                TEMP_K,
            );
            let cfg = ReactorPhysicsConfig {
                keff: keff.clone(),
                source_box: SourceBox {
                    lower: Position::new(-r_rpt_fuel_csg, -r_rpt_fuel_csg, -r_rpt_fuel_csg),
                    upper: Position::new(r_rpt_fuel_csg, r_rpt_fuel_csg, r_rpt_fuel_csg),
                },
                ..Default::default()
            };
            let r = run_keff_reactor_physics(&pebble_csg, &mats, &nucs, &cfg)
                .expect("CSG reactor physics");
            eprintln!(
                "  k_eff (ring-RPT, CSG sphere) = {:.5} ± {:.5}",
                r.keff.k_mean, r.keff.k_std
            );
            print_six_factors("ring-RPT CSG pebble", &r);
            let s3 = &r.six_factors;
            let (e2, f2, p2, eps2) = s3.two_group_openmc_convention();
            eprintln!("\n  ── the SAME run in both conventions ──");
            eprintln!(
                "    3-group (this crate): η {:.4} f {:.4} p {:.4} ε {:.4}",
                s3.eta.mean, s3.f.mean, s3.p.mean, s3.epsilon.mean
            );
            eprintln!("    2-group (OpenMC deck): η {e2:.4} f {f2:.4} p {p2:.4} ε {eps2:.4}");
            eprintln!("    OpenMC reference     : η 2.0073 f 0.9216 p 0.4842 ε 1.5043");
            eprintln!("\n    vs OpenMC, 2-group like-for-like:");
            for (name, ours, theirs) in [
                ("η", e2, 2.0073_f64),
                ("f", f2, 0.9216),
                ("p", p2, 0.4842),
                ("ε", eps2, 1.5043),
            ] {
                eprintln!(
                    "      {name}  {ours:.4} vs {theirs:.4}   {:+.2}%",
                    100.0 * (ours / theirs - 1.0)
                );
            }
            return;
        }

        // 1. Explicit-TRISO pebble. `ExplicitTrisoPebble` packages exactly the
        // packed-particle / layer-resolution / matrix-fallback lookup this
        // closure used to hand-assemble (see its rustdoc for why it exists).
        let explicit_pebble = ExplicitTrisoPebble::new(
            packed,
            spec,
            TrisoMaterials {
                kernel: mi::FUEL,
                buffer: mi::BUFFER,
                ipyc: mi::PYC1,
                sic: mi::SIC,
                opyc: mi::PYC2,
                matrix: mi::GRAPHITE,
            },
            mi::GRAPHITE, // shell
            mi::FLIBE,    // coolant (+ cube corners)
            R_FUEL_ZONE,
            R_PEBBLE,
        );
        let explicit_at = |p: Position| -> Option<usize> { explicit_pebble.material_at(p) };
        // Single-case escape hatch for bisecting a change against one number:
        // OUTRAM_RINGRPT_ONLY=explicit-cube runs just the explicit-TRISO cube
        // case and exits. The full deck is seven eigenvalue solves and ~40 min,
        // which is too slow a loop to test a one-line change against.
        let explicit_cube = run_keff_delta_in(cube, &mats, &nucs, &majorant, &explicit_at, &keff);
        if only == "explicit-cube" {
            eprintln!(
                "  k_eff (explicit TRISO pebble, CUBE ONLY) = {:.5} ± {:.5}",
                explicit_cube.k_mean, explicit_cube.k_std
            );
            return;
        }
        let explicit = run_keff_delta_in(ball, &mats, &nucs, &majorant, &explicit_at, &keff);
        eprintln!(
            "  k_eff (explicit TRISO pebble, sphere) = {:.5} ± {:.5}   [cube {:.5} ± {:.5}]",
            explicit.k_mean, explicit.k_std, explicit_cube.k_mean, explicit_cube.k_std
        );

        // 2. Ring-RPT pebble — SAME reflective cube, homogenised fuel as a shell.
        let rpt_at = |p: Position| -> Option<usize> {
            let r = p.norm();
            Some(if r < R_RPT_INNER {
                mi::GRAPHITE // inner graphite ball
            } else if r < r_rpt_fuel {
                mi::HOMOG // homogenised TRISO fuel shell
            } else {
                zone_outside_fuel(r)
            })
        };
        let rpt_cube = run_keff_delta_in(cube, &mats, &nucs, &majorant, &rpt_at, &keff);
        let rpt = run_keff_delta_in(ball, &mats, &nucs, &majorant, &rpt_at, &keff);
        eprintln!(
            "  k_eff (ring-RPT pebble, sphere)       = {:.5} ± {:.5}   [cube {:.5} ± {:.5}]   (fuel shell {R_RPT_INNER:.4}–{r_rpt_fuel:.4} cm)",
            rpt.k_mean, rpt.k_std, rpt_cube.k_mean, rpt_cube.k_std
        );

        // 3. Naive homogenisation — homog fuel fills the whole r < 1.9 zone.
        let naive_at = |p: Position| -> Option<usize> {
            let r = p.norm();
            Some(if r < R_FUEL_ZONE {
                mi::NAIVE_HOMOG
            } else {
                zone_outside_fuel(r)
            })
        };
        let naive_cube = run_keff_delta_in(cube, &mats, &nucs, &majorant, &naive_at, &keff);
        let naive = run_keff_delta_in(ball, &mats, &nucs, &majorant, &naive_at, &keff);
        eprintln!(
            "  k_eff (naive homogenised, sphere)     = {:.5} ± {:.5}   [cube {:.5} ± {:.5}]",
            naive.k_mean, naive.k_std, naive_cube.k_mean, naive_cube.k_std
        );

        // 4. Ring-RPT pebble through the real CSG sphere geometry — this is
        //    directly comparable to the OpenMC RPT number (reflective sphere
        //    r = 3, no cube-corner coolant), and yields the six factors + spectrum.
        let pebble = fhr_pebble_geometry(
            R_RPT_INNER,
            r_rpt_fuel,
            R_PEBBLE,
            R_ROOT,
            mi::HOMOG,
            mi::GRAPHITE,
            mi::FLIBE,
            BoundaryType::Reflective,
            TEMP_K,
        );
        let rp_cfg = ReactorPhysicsConfig {
            keff: keff.clone(),
            source_box: SourceBox {
                lower: Position::new(-r_rpt_fuel, -r_rpt_fuel, -r_rpt_fuel),
                upper: Position::new(r_rpt_fuel, r_rpt_fuel, r_rpt_fuel),
            },
            ..Default::default()
        };
        let rpt_csg = run_keff_reactor_physics(&pebble, &mats, &nucs, &rp_cfg)
            .expect("ring-RPT CSG pebble reactor physics");
        eprintln!(
            "  k_eff (ring-RPT pebble, CSG sphere) = {:.5} ± {:.5}   leak {:.2e}",
            rpt_csg.keff.k_mean, rpt_csg.keff.k_std, rpt_csg.leakage_total.mean
        );
        print_six_factors("ring-RPT CSG pebble", &rpt_csg);

        // ── Summary ──
        let d = |k: f64, s: f64, k0: f64, s0: f64| {
            let dk = (k - k0) * 1e5;
            let cs = (s * s + s0 * s0).sqrt() * 1e5;
            (dk, dk.abs() / cs.max(1.0))
        };
        let (d_rp, z_rp) = d(rpt.k_mean, rpt.k_std, explicit.k_mean, explicit.k_std);
        let (d_nv, z_nv) = d(naive.k_mean, naive.k_std, explicit.k_mean, explicit.k_std);
        let (d_rpc, z_rpc) = d(
            rpt_cube.k_mean,
            rpt_cube.k_std,
            explicit_cube.k_mean,
            explicit_cube.k_std,
        );
        let (d_nvc, z_nvc) = d(
            naive_cube.k_mean,
            naive_cube.k_std,
            explicit_cube.k_mean,
            explicit_cube.k_std,
        );
        // Absolute comparisons against OpenMC — only legitimate on the sphere,
        // which is the boundary the OpenMC deck actually used.
        const OMC_EXPLICIT: (f64, f64) = (1.36510, 0.00063);
        const OMC_RPT: (f64, f64) = (1.36479, 0.00067);
        let (d_ex_omc, z_ex_omc) = d(
            explicit.k_mean,
            explicit.k_std,
            OMC_EXPLICIT.0,
            OMC_EXPLICIT.1,
        );
        let (d_rp_omc, z_rp_omc) = d(rpt.k_mean, rpt.k_std, OMC_RPT.0, OMC_RPT.1);

        eprintln!("\n── outram-mc-libs FHR pebble (ENDF/B-VIII.0, c_Graphite S(α,β)) ──");
        eprintln!("  reflective SPHERE r = {R_ROOT} — the domain OpenMC used, so both the");
        eprintln!("  method delta AND the absolute k are comparable to the reference:");
        eprintln!("    explicit TRISO   : {:.5} ± {:.5}   vs OpenMC = {d_ex_omc:+.0} pcm ({z_ex_omc:.1}σ)", explicit.k_mean, explicit.k_std);
        eprintln!("    ring-RPT         : {:.5} ± {:.5}   Δ(RPT−explicit)   = {d_rp:+.0} pcm  ({z_rp:.1}σ)   vs OpenMC = {d_rp_omc:+.0} pcm ({z_rp_omc:.1}σ)", rpt.k_mean, rpt.k_std);
        eprintln!("    naive homogenised: {:.5} ± {:.5}   Δ(naive−explicit) = {d_nv:+.0} pcm  ({z_nv:.1}σ)", naive.k_mean, naive.k_std);
        eprintln!("  reflective CUBE half-width {half} (corners carry extra FLiBe — NOT");
        eprintln!("  comparable to OpenMC in absolute terms; shown to price that over-count):");
        eprintln!(
            "    explicit TRISO   : {:.5} ± {:.5}   cube−sphere = {:+.0} pcm",
            explicit_cube.k_mean,
            explicit_cube.k_std,
            (explicit_cube.k_mean - explicit.k_mean) * 1e5
        );
        eprintln!("    ring-RPT         : {:.5} ± {:.5}   Δ(RPT−explicit)   = {d_rpc:+.0} pcm  ({z_rpc:.1}σ)", rpt_cube.k_mean, rpt_cube.k_std);
        eprintln!("    naive homogenised: {:.5} ± {:.5}   Δ(naive−explicit) = {d_nvc:+.0} pcm  ({z_nvc:.1}σ)", naive_cube.k_mean, naive_cube.k_std);
        eprintln!("  reflective-sphere CSG (surface-tracked, six factors + spectrum):");
        eprintln!(
            "    ring-RPT         : {:.5} ± {:.5}",
            rpt_csg.keff.k_mean, rpt_csg.keff.k_std
        );

        // ── This code's OWN RPT inner radius (opt-in: --search-rpt-radius) ──
        //
        // R_RPT_INNER = 1.4934 cm is a *fitted* parameter, and it was fitted by
        // the deck author so that OpenMC's ring-RPT pebble reproduced OpenMC's
        // explicit-TRISO pebble. Reporting `RPT − explicit` at that radius here
        // therefore measures how well OpenMC's fit transfers, not how well RPT
        // works in this code. The honest quantity is the radius at which THIS
        // code's ring-RPT reproduces THIS code's explicit-TRISO — so solve for
        // it, with the explicit-TRISO k as the target rather than 1.0.
        //
        // The fuel shell must still fit inside the r = 1.9 cm fuel zone:
        // r_outer³ = r_inner³ + pf·1.9³, so r_inner ≤ (1.9³ − pf·1.9³)^(1/3),
        // which is ≈ 1.687 cm at pf = 0.30. The bracket stays well inside that.
        if search_radius {
            use outram_mc_libs::physics::search::{search_for_keff, SearchMethod, SearchSettings};

            let r_inner_max = (R_FUEL_ZONE.powi(3) * (1.0 - spec.packing_fraction)).cbrt();
            eprintln!("\n── RPT inner-radius search (target = this code's explicit-TRISO k) ──");
            eprintln!(
                "  target k = {:.5} ± {:.5}   geometric ceiling on r_inner = {r_inner_max:.4} cm",
                explicit.k_mean, explicit.k_std
            );

            let k_of_r = |r_inner: f64| {
                let r_outer = rpt_fuel_outer_radius(r_inner, R_FUEL_ZONE, spec.packing_fraction);
                let at = move |p: Position| -> Option<usize> {
                    let r = p.norm();
                    Some(if r < r_inner {
                        mi::GRAPHITE
                    } else if r < r_outer {
                        mi::HOMOG
                    } else {
                        zone_outside_fuel(r)
                    })
                };
                let k = run_keff_delta_in(ball, &mats, &nucs, &majorant, at, &keff);
                eprintln!(
                    "    r_inner {r_inner:.4} cm (shell {r_inner:.4}–{r_outer:.4}) -> k = {:.5} ± {:.5}",
                    k.k_mean, k.k_std
                );
                k
            };

            let settings = SearchSettings {
                target: explicit.k_mean,
                // Parameter tolerance in cm. Deliberately coarser than the
                // radius change worth one k standard error — past that point
                // the residual's SIGN is Monte Carlo noise and bisection is
                // just walking randomly.
                tol: 0.01,
                // Residual tolerance ~ the combined 1σ of the two eigenvalues
                // being compared: stop as soon as the ring-RPT pebble is
                // statistically indistinguishable from the explicit one.
                k_tol: (explicit.k_std.powi(2) + explicit.k_std.powi(2)).sqrt(),
                max_iterations: 8,
                method: SearchMethod::Bisect,
            };

            match search_for_keff(k_of_r, (1.10, 1.64), &settings) {
                Ok(res) => {
                    eprintln!(
                        "  converged={}  r_inner* = {:.4} cm   k = {:.5} ± {:.5}  ({} solves)",
                        res.converged,
                        res.parameter,
                        res.keff,
                        res.keff_std,
                        res.iterations.len()
                    );
                    eprintln!(
                        "  deck author's OpenMC-fitted radius = {R_RPT_INNER:.4} cm   \
                         Δ = {:+.4} cm ({:+.1} %)",
                        res.parameter - R_RPT_INNER,
                        100.0 * (res.parameter - R_RPT_INNER) / R_RPT_INNER
                    );
                    eprintln!(
                        "  Read this as: the RPT radius is code-dependent by that much on this \
                         pebble.\n  It is NOT a defect in either code — RPT is a fitted \
                         equivalence, and what it\n  is fitted against is part of its definition."
                    );
                }
                Err(e) => {
                    eprintln!("  search could not start: {e:?}");
                    eprintln!(
                        "  (a bracket that does not straddle means k_rpt(1.10) and k_rpt(1.64) \
                         sit on the\n   same side of the explicit-TRISO k — widen it, or check \
                         the monotonicity assumption)"
                    );
                }
            }
        } else {
            eprintln!(
                "\n(the RPT inner radius 1.4934 cm was fitted against OpenMC; pass \
                 --search-rpt-radius\n to solve for this code's own equivalent radius)"
            );
        }

        // ── OpenMC reference (op-mzvp.1, GH #156) ──
        eprintln!("\n── OpenMC full-pebble reference (op-mzvp.1, ENDF/B-VIII.0, c_Graphite) ──");
        eprintln!("  explicit TRISO pebble : k = 1.36510 ± 0.00063");
        eprintln!(
            "  ring-RPT pebble       : k = 1.36479 ± 0.00067   Δ(RPT−explicit) = −31 pcm (0.34σ)"
        );
        eprintln!("  η/f/p/ε (explicit)    : 2.0158 / 0.9172 / 0.4837 / 1.5064");
        eprintln!("  η/f/p/ε (ring-RPT)    : 2.0073 / 0.9216 / 0.4842 / 1.5043");
        eprintln!(
            "\nCAVEATS (see verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md):\n\
             - the delta domain is now the same reflective SPHERE r = 3 the OpenMC\n\
               deck used, so the cube-corner FLiBe over-count no longer affects the\n\
               reported comparison; the cube rows are kept only to price it.\n\
             - explicit-TRISO delta resolves the 5 coating layers by nearest-centre\n\
               + radius, not exact CSG.\n\
             - the RPT inner radius (1.4934 cm) was tuned by the deck author for\n\
               OpenMC, so it is not optimal here.\n\
             - Li-6(n,t) now counted (op-mzvp.2.10 fixed); tiny at 99.995 % Li-7.\n\
             - AI-assisted code-to-code check, not a validated result."
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
