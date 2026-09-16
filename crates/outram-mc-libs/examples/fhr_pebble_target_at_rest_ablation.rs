//! **Price free-gas target motion on the FHR pebble** — a paired-seed ablation
//! of [`Nuclide::with_target_at_rest`] on the ring-RPT pebble, the case gh:#193
//! priced it on.
//!
//! Run 2 of job 4 in `docs/handoff-heavy-neutronics-runs.md`, and the one that
//! matters. Its companion, `examples/godiva_target_at_rest_ablation.rs`, is a
//! harness check that exists so a null here could not be blamed on a dead
//! switch.
//!
//! # What is being ablated, and what this is really testing
//!
//! Two arms over the **same seeds**, identical in every other respect —
//! geometry, materials, temperature, histories, generation split, and every
//! cross section. The ablation is one call,
//! `.map(Nuclide::with_target_at_rest)`, applied per nuclide **in process**.
//!
//! The question is **not** "is free-gas target motion worth something on a
//! thermal pebble" — that is already known to be worth about −2242 pcm. It is
//! whether the **new in-process hook reproduces the old whole-material
//! switch**. gh:#193's pricing table got its −2242 pcm by setting
//! `OUTRAM_RINGRPT_TARGET_AT_REST=1` in `examples/fhr_ring_rpt_endf.rs`, which
//! zeroes `KeffSettings::temperature_k` for the **whole run** — including the
//! S(α,β) moderators, whose bound-atom law is a different code path from the
//! free-gas kernel this hook reaches.
//!
//! So a materially different number is a **real finding, not a failure**: it
//! would mean the per-nuclide hook reaches a different set of nuclides than
//! zeroing the temperature did, which is plausible and worth knowing.
//!
//! # Prediction on record, made before any measurement
//!
//! **−2242 pcm**, because that is what the environment-variable route measured
//! in gh:#193's pricing table on this case:
//!
//! ```text
//!   target motion     k_eff (ring-RPT CSG)   p vs OpenMC   eps vs OpenMC
//!   sampled (correct) 1.40546 +/- 0.00234    +8.54 %       -4.99 %
//!   AT REST (broken)  1.38304 +/- 0.00223    +6.03 %       -3.69 %
//!   difference        -2242 +/- 323 pcm      -2.51 points  +1.30 points
//! ```
//!
//! Note what that row's `± 323` is and is not: it is the two runs' **own
//! within-run** `k_std` added in quadrature, from **one seed a side**. This
//! program instead runs an ensemble of seeds and quotes the seed-to-seed
//! standard error, which is the statistic the difference actually needs.
//!
//! # Statistics
//!
//! This hook **does not preserve the RNG stream** — the free-gas kernel draws a
//! target velocity the target-at-rest kernel never draws, so the arms diverge
//! at the first thermal collision, which on a moderated pebble is immediately.
//! The difference is attributable **statistically over an ensemble of seeds
//! only**, never history by history. A single paired run measures nothing here.
//! Both the paired and unpaired figures are printed and the run says which is
//! tighter, measured rather than assumed.
//!
//! # The case
//!
//! The **ring-RPT CSG pebble** — the surface-tracked case that yields the
//! six-factor decomposition, and the one gh:#193's table is quoted on. Its
//! geometry, materials and nuclide set are reproduced from
//! `examples/fhr_ring_rpt_endf.rs` unchanged; the delta-tracked explicit-TRISO
//! and naive-homogenised cases in that example are not run, because the
//! reference row is the CSG one.
//!
//! **Not a validated result** — an AI-assisted code-to-code ablation.
//!
//! ```text
//! OUTRAM_FHR_SEEDS=8 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example fhr_pebble_target_at_rest_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    eprintln!(
        "fhr_pebble_target_at_rest_ablation: desktop-only (reads multi-MB ENDF tapes from the \
         repo's reference-data/, which a device build does not carry)"
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
    use outram_mc_libs::pebble_beds::fhr_pebble::{
        fhr_pebble_geometry, homogenise_by_volume, rpt_fuel_outer_radius, TrisoSpec,
    };
    use outram_mc_libs::geometry::position::Position;
    use outram_mc_libs::geometry::surface::BoundaryType;
    use outram_mc_libs::material::thermal::ThermalScattering;
    use std::path::PathBuf;
    use std::time::Instant;

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
    /// Move every bound-graphite carbon component onto the free-gas carbon
    /// nuclide of the same isotope, merging with any already there.
    ///
    /// Needed only to match `rpt_pebble.py`, whose homogenised TRISO material is
    /// built with no `add_s_alpha_beta` call. Nothing physical recommends it —
    /// the buffer and PyC layers are graphite — but a code-to-code comparison
    /// has to reproduce the model it is compared against, not improve on it.
    fn free_gas_carbon(mut m: Material) -> Material {
        let mut acc: Vec<NuclideComponent> = Vec::new();
        for c in m.components.drain(..) {
            let idx = match c.nuclide_idx {
                nx::C12G => nx::C12,
                nx::C13G => nx::C13,
                other => other,
            };
            match acc.iter_mut().find(|a| a.nuclide_idx == idx) {
                Some(a) => a.atom_density += c.atom_density,
                None => acc.push(NuclideComponent {
                    nuclide_idx: idx,
                    atom_density: c.atom_density,
                }),
            }
        }
        m.components = acc;
        m
    }

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
        //
        // Then **all** of its carbon is moved onto the free-gas nuclides, to
        // match the deck. `rpt_pebble.py::get_mixed_triso_fuel_material` builds
        // the mixed material with `add_element('C', ...)` and **no**
        // `add_s_alpha_beta` call — so in the reference's ring-RPT pebble the
        // buffer / PyC1 / PyC2 carbon, which *does* carry `c_Graphite` in the
        // explicit pebble, is free-gas once homogenised. That is 83 % of the
        // mixed material's carbon and ~66 % of all its atoms, so it is not a
        // detail. `homogenise_by_volume` preserves nuclide indices, which is the
        // right default — it is this deck that drops the bound treatment, and
        // matching it is the point of the comparison.
        let vols = spec.layer_volumes();
        let homog = free_gas_carbon(homogenise_by_volume(
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
        ));

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

    /// Indices into the `Vec<Material>` `build_materials` returns. Only
    /// `HOMOG`, `GRAPHITE` and `FLIBE` are named by the ring-RPT CSG geometry;
    /// the rest are kept because they document that vector's ORDER, which the
    /// three that are used depend on.
    #[allow(dead_code)]
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

    /// Seeds per arm. Each seed is a full 4000 × [30 + 80] power iteration.
    fn n_seeds() -> usize {
        std::env::var("OUTRAM_FHR_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8)
    }

    /// Mean, sample standard deviation and standard error of a sample.
    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        let sd = var.sqrt();
        (mean, sd, sd / n.sqrt())
    }

    /// One arm's per-seed results: `k_eff`, and the 2-group `p` and `ε` in the
    /// OpenMC deck's convention, so the run reproduces gh:#193's three columns
    /// rather than only its headline `k`.
    struct Arm {
        k: Vec<f64>,
        p: Vec<f64>,
        eps: Vec<f64>,
    }

    /// Run one arm over `seeds` on the ring-RPT CSG pebble.
    ///
    /// Transport is parallel **inside** a seed (`CpuMultiThread`), seeds are
    /// looped serially — the same backend gh:#193's row was taken on, so the
    /// two are comparable without arguing about RNG-stream layout.
    fn run_arm(
        nucs: &[Nuclide],
        mats: &[Material],
        r_fuel: f64,
        seeds: &[u64],
        label: &str,
    ) -> Arm {
        let mut arm = Arm {
            k: Vec::new(),
            p: Vec::new(),
            eps: Vec::new(),
        };
        let pebble = fhr_pebble_geometry(
            R_RPT_INNER,
            r_fuel,
            R_PEBBLE,
            R_ROOT,
            mi::HOMOG,
            mi::GRAPHITE,
            mi::FLIBE,
            BoundaryType::Reflective,
            TEMP_K,
        );
        for &seed in seeds {
            let t = Instant::now();
            let cfg = ReactorPhysicsConfig {
                keff: KeffSettings {
                    n_particles: 4000,
                    n_inactive: 30,
                    n_active: 80,
                    temperature_k: TEMP_K,
                    seed,
                    compute: ComputeType::CpuMultiThread(Default::default()),
                    ..KeffSettings::default()
                },
                source_box: SourceBox {
                    lower: Position::new(-r_fuel, -r_fuel, -r_fuel),
                    upper: Position::new(r_fuel, r_fuel, r_fuel),
                },
                ..Default::default()
            };
            let r = run_keff_reactor_physics(&pebble, mats, nucs, &cfg)
                .expect("CSG reactor physics");
            let (_e2, _f2, p2, eps2) = r.six_factors.two_group_openmc_convention();
            eprintln!(
                "    {label} seed {seed:>3}: k = {:.5} ± {:.5}, p = {p2:.4}, eps = {eps2:.4}  \
                 [{:.1} s]",
                r.keff.k_mean,
                r.keff.k_std,
                t.elapsed().as_secs_f64()
            );
            arm.k.push(r.keff.k_mean);
            arm.p.push(p2);
            arm.eps.push(eps2);
        }
        arm
    }

    pub fn run() {
        eprintln!(
            "=== FHR ring-RPT pebble: free-gas target motion, priced with the in-process hook ==="
        );
        eprintln!();
        let t0 = Instant::now();
        eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
        let moving = nuclides();
        let at_rest: Vec<Nuclide> = moving
            .iter()
            .cloned()
            .map(Nuclide::with_target_at_rest)
            .collect();

        // ── Controls, before spending hours of transport ──
        //
        // "There was something to remove" is the assertion usually skipped and
        // the one that matters most: a hook that removes nothing reports "no
        // difference", and that reads as "this physics does not matter". The
        // mechanism here is the elastic kinematics temperature, so the control
        // is that it was non-zero on every nuclide and is now zero on every
        // nuclide.
        assert!(
            moving.iter().all(|n| !n.is_target_at_rest()),
            "the unablated arm already holds its targets at rest; any difference below would be \
             an artefact"
        );
        assert!(
            at_rest.iter().all(|n| n.is_target_at_rest()),
            "with_target_at_rest did not take on every nuclide -- a partial no-op, and the \
             number below would not be this mechanism's worth"
        );
        for (m, r) in moving.iter().zip(&at_rest) {
            assert!(
                m.free_gas_kt(TEMP_K) > 0.0,
                "{}: kinematics kT is already zero before the ablation",
                m.name
            );
            assert_eq!(
                r.free_gas_kt(TEMP_K),
                0.0,
                "{}: kinematics kT survived the ablation",
                r.name
            );
        }
        eprintln!(
            "  control: kinematics kT {:.6e} -> 0 eV on all {} nuclides (S(alpha,beta) \
             moderators included)",
            moving[0].free_gas_kt(TEMP_K),
            moving.len()
        );

        let (mats, spec) = build_materials();
        let r_fuel = rpt_fuel_outer_radius(R_RPT_INNER, R_FUEL_ZONE, spec.packing_fraction);
        eprintln!(
            "  ring-RPT fuel shell: {R_RPT_INNER:.6} < r < {r_fuel:.6} cm; pebble {R_PEBBLE}, \
             reflective root {R_ROOT}"
        );
        eprintln!("Nuclear data + materials ready in {:.1} s.\n", t0.elapsed().as_secs_f64());

        let n = n_seeds();
        let seeds: Vec<u64> = (1..=n as u64).collect();
        eprintln!("{n} seeds per arm, 4000 histories × [30 inactive + 80 active]:");
        let t = Instant::now();
        let a = run_arm(&moving, &mats, r_fuel, &seeds, "MOVING ");
        eprintln!("  MOVING  arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let b = run_arm(&at_rest, &mats, r_fuel, &seeds, "AT-REST");
        eprintln!("  AT-REST arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        let (ka, ska, eka) = stats(&a.k);
        let (kb, skb, ekb) = stats(&b.k);
        // pcm on the eigenvalue itself, the convention gh:#193's row uses.
        let diff = (kb - ka) * 1.0e5;
        let ediff = ((eka * eka) + (ekb * ekb)).sqrt() * 1.0e5;
        let d: Vec<f64> = b
            .k
            .iter()
            .zip(&a.k)
            .map(|(x, y)| (x - y) * 1.0e5)
            .collect();
        let (md, sdp, edp) = stats(&d);

        println!("FHR ring-RPT CSG pebble, free-gas target motion (in-process hook)");
        println!("  arm        n      k_eff        sd        sem");
        println!(
            "  MOVING   {:>3}   {ka:.5}   {ska:.5}   {eka:.5}",
            a.k.len()
        );
        println!(
            "  AT-REST  {:>3}   {kb:.5}   {skb:.5}   {ekb:.5}",
            b.k.len()
        );
        println!();
        for (label, x, y) in [("p  (2-group)", &a.p, &b.p), ("eps(2-group)", &a.eps, &b.eps)] {
            let (mx, _, ex) = stats(x);
            let (my, _, ey) = stats(y);
            println!(
                "  {label}: MOVING {mx:.4} ± {ex:.4}   AT-REST {my:.4} ± {ey:.4}   \
                 difference {:+.4} ± {:.4}",
                my - mx,
                (ex * ex + ey * ey).sqrt()
            );
        }
        println!();
        println!(
            "  difference (AT-REST − MOVING), unpaired = {:+.0} ± {:.0} pcm  ({:.1} sigma)",
            diff,
            ediff,
            (diff / ediff).abs()
        );
        println!(
            "  difference (AT-REST − MOVING), paired   = {:+.0} ± {:.0} pcm  ({:.1} sigma), \
             paired sd {:.0}",
            md,
            edp,
            (md / edp).abs(),
            sdp
        );
        let (quote, qerr) = if sdp > (ska * 1.0e5).max(skb * 1.0e5) {
            println!(
                "  -> paired sd ({sdp:.0}) EXCEEDS either arm's ({:.0}); the seeds do not pair, \
                 so quote the UNPAIRED figure.",
                (ska * 1.0e5).max(skb * 1.0e5)
            );
            (diff, ediff)
        } else {
            println!(
                "  -> paired sd ({sdp:.0}) is below either arm's ({:.0}); quote the PAIRED \
                 figure.",
                (ska * 1.0e5).max(skb * 1.0e5)
            );
            (md, edp)
        };

        const REFERENCE_PCM: f64 = -2242.0;
        println!();
        println!("  Prediction on record, made before any measurement: {REFERENCE_PCM:+.0} pcm,");
        println!("  because that is what the environment-variable route (which zeroes the whole");
        println!("  run's transport temperature, S(alpha,beta) moderators included) measured on");
        println!("  this same case in gh:#193's pricing table. The question is whether the new");
        println!("  PER-NUCLIDE hook reaches the same set of nuclides that did.");
        println!();
        println!("  VERDICT:");
        let gap = quote - REFERENCE_PCM;
        if qerr > 0.0 && gap.abs() < 3.0 * qerr {
            println!(
                "    HELD. {quote:+.0} ± {qerr:.0} pcm is {:+.0} pcm from the recorded \
                 {REFERENCE_PCM:+.0} pcm,",
                gap
            );
            println!(
                "    which is {:.1} sigma of this run's own statistics. The in-process hook \
                 reproduces",
                (gap / qerr).abs()
            );
            println!("    the whole-material switch on this case.");
        } else {
            println!(
                "    FAILED as stated: {quote:+.0} ± {qerr:.0} pcm is {gap:+.0} pcm from the \
                 recorded"
            );
            println!(
                "    {REFERENCE_PCM:+.0} pcm ({:.1} sigma). Per the hand-off this is a REAL \
                 FINDING, not a",
                (gap / qerr).abs()
            );
            println!(
                "    failure: the per-nuclide hook is reaching a different set of nuclides than"
            );
            println!(
                "    zeroing the temperature did -- most plausibly the S(alpha,beta) moderators,"
            );
            println!(
                "    whose bound-atom law is a different code path from the free-gas kernel."
            );
            println!(
                "    Note also that the {REFERENCE_PCM:+.0} pcm reference carries a ONE-SEED \
                 within-run"
            );
            println!(
                "    error bar (±323 pcm), not an ensemble one; part of any gap may be that."
            );
        }
    }
}
