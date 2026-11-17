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
//! thermal pebble" -- that is already known. It is whether the **new in-process
//! hook reproduces the old whole-material switch**. gh:#193's pricing table got
//! its number by setting `OUTRAM_RINGRPT_TARGET_AT_REST=1` in
//! `examples/fhr_ring_rpt_endf.rs`, which at that commit (`75615b678`) set
//! `KeffSettings::temperature_k = 0.0` for the **whole run** and edited no
//! nuclide.
//!
//! **That switch no longer exists** (gh:#1617). gh:#313 (`0414bc827`,
//! 2026-09-30) made the CSG kernel stop reading `KeffSettings::temperature_k`
//! (`tests/temperature_precedence.rs` pins it), and moved the env var onto
//! `Nuclide::with_target_at_rest`. So on today's build there is no second route
//! to compare against; the like-for-like number below was measured by
//! replaying the old switch on the last build that has it.
//!
//! # Prediction on record
//!
//! ~~**-2242 pcm**, k_MOVING 1.40546 +/- 0.00234 (gh:#193's table, 2026-09-12,
//! one seed a side, +/- 323 pcm within-run)~~ **CORRECTED 2026-11-17 (gh:#1617)**
//! -- stale, not a like-for-like number: both figures predate the F-19
//! sub-threshold inelastic fix `45f8d72d` (k_MOVING 1.40546 before it, 1.36140
//! after; tests/inelastic_channel_vs_endf.rs, gh:#193's closing comment). The
//! F-19 defect made p +8.54 % over OpenMC; the ablation acts on that same
//! slowing-down balance, so the -2242 was measured on a broken model.
//!
//! **Re-baselined: -556 +/- 87 pcm** (paired, 8 seeds, 4000 x [30 + 80]),
//! k_MOVING = 1.36625, k_AT-REST = 1.36069, produced 2026-11-17 by replaying
//! the whole-run switch (`KeffSettings::temperature_k = 0.0`, nuclides
//! untouched) on commit `60f6bb261` (post-F-19, pre-#313; the last kind of
//! build that has the switch), next to the per-nuclide hook on the same build.
//! The prediction for that replay was committed first
//! (`verification_and_validation/sab_per_material_1581/README.md`, "gh:#1617")
//! and held on every count: the switch arm is **bit-identical to the hook arm on
//! 8 of 8 seeds** (k, p and eps), MOVING seed 1 reproduced 1.36662 +/- 0.00210,
//! and the difference equals the 2026-09-17 record of -556 +/- 87 pcm. So the
//! two routes are the same arithmetic, and **-556 is both the whole-material
//! and the per-nuclide value on that build**. It was not tuned: it is what the
//! replay returned. Raw output: `ensembles_1617/`.
//!
//! It is the pass criterion below because it is the only like-for-like
//! whole-material number there is, with the caveat that **it was measured on
//! `60f6bb261`, not on the build running this driver**: anything that moved
//! between that commit and now (gh:#313's free-gas kT, `81ec018c3`'s URR/DBRC/
//! equiprobable S(a,b) sampling, gh:#1581's per-material S(a,b)) can move this
//! driver's number away from it without the hook being wrong. The first
//! post-1617 measurement here, 2026-11-17 (`7b5dd858d`, 8 seeds), was
//! -153 +/- 43 pcm paired, 4.2 sigma from -556 with k_MOVING 1.36307; ~~the cause
//! is not isolated (the MOVING arm moved -318 pcm and the AT-REST arm +85 pcm)~~
//! **CORRECTED 2026-11-17 (gh:#1636): isolated.** `60f6bb261`'s `Nuclide` carried
//! no resonance elastic scattering table (`git grep -i dbrc 60f6bb261 --
//! crates/outram-mc-libs/src` hits only `physics/scatter.rs`); `f8dbb4951`
//! (2026-09-20, "correct physics is the DEFAULT") attached DBRC by default and
//! `457d4ca97` (gh:#1311) made upstream's RVS the method. It acts only at target
//! kT > 0, so it is in the MOVING arm and absent from the AT-REST arm, which is
//! exactly the signature (MOVING -318 +/- 50 pcm, AT-REST +85 +/- 105, 0.8 sigma).
//! Removing it (`OUTRAM_FHR_NO_RESONANCE_SCATTERING=1`, `Nuclide::without_dbrc`)
//! gives, 8 seeds, same session build: MOVING 1.36570 (+263 +/- 74 pcm over the
//! default, 3.6 sigma), AT-REST bit-identical seed for seed, worth **-416 +/- 79
//! pcm**, 1.2 sigma from -556 +/- 87. The legacy scatter kernels (gh:#1386) move
//! MOVING by -32 +/- 47 pcm (0.7 sigma): not the cause. So the default-build
//! worth is **-153 +/- 43 pcm** (`SESSION_PCM`), and -556 is the worth of a model
//! without resonance scattering. Correct physics (default on, maintainer-decided),
//! so this is a re-baseline, not a revert. Raw outputs, analysis and the
//! predictions written before the runs:
//! `verification_and_validation/sab_per_material_1581/` (`ensembles_1636/`,
//! README "gh:#1636"). Not done: building `f8dbb4951` and its parent to separate
//! DBRC-on from the RVS method switch (the ablation removes both together).
//!
//! Note what the +/- is: a seed-to-seed standard error over an ensemble, not
//! one run's within-run `k_std` as in gh:#193's single-seed row.
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
    use petir::mathf::RealMath;
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
    use outram_mc_libs::material::material_sab::{assign_material_sab, MaterialSab};
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
        let n = Nuclide::from_endf_file(&p, name, uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(TEMP_K), 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
        eprintln!("{:.1?}", t0.elapsed());
        n
    }

    fn nuclides() -> (Vec<Nuclide>, ThermalScattering) {
        // C12 / C13 (fuel kernel) stay free-gas. C12G / C13G (buffer, PyC,
        // matrix, shell) carry the ENDF/B-VIII.0 crystalline-graphite S(α,β)
        // law — below ~4 eV they scatter off the bound lattice (coherent Bragg
        // + incoherent inelastic) instead of a free carbon atom. This is
        // `c_Graphite` in the OpenMC deck; without it a graphite-moderated
        // spectrum sits ~1700 pcm too high (tests/htr10_graphite_thermal_scatt…).
        let sab = graphite_sab();
        eprintln!("  graphite S(α,β): ENDF/B-VIII.0 tsl-crystalline-graphite (MAT 30) … ok");
        let nuclides = vec![
            load("U235", "n-092_U_235-ENDF8.0.endf"),
            load("U238", "n-092_U_238.endf"),
            load("O16", "n-008_O_016-ENDF8.0.endf"),
            load("C12", "n-006_C_012-ENDF8.0.endf"),
            load("C13", "n-006_C_013-ENDF8.0.endf"),
            // Loaded free; c_Graphite is attached per material in `run`
            // (gh:#1581).
            load("C12", "n-006_C_012-ENDF8.0.endf"),
            load("C13", "n-006_C_013-ENDF8.0.endf"),
            load("Si28", "n-014_Si_028-ENDF8.0.endf"),
            load("Si29", "n-014_Si_029-ENDF8.0.endf"),
            load("Si30", "n-014_Si_030-ENDF8.0.endf"),
            load("F19", "n-009_F_019-ENDF8.0.endf"),
            load("Li6", "n-003_Li_006-ENDF8.0.endf"),
            load("Li7", "n-003_Li_007-ENDF8.0.endf"),
            load("Be9", "n-004_Be_009-ENDF8.0.endf"),
        ];
        (nuclides, sab)
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
        let var = x.iter().map(|v| (v - mean).r_powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
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
            let r =
                run_keff_reactor_physics(&pebble, mats, nucs, &cfg).expect("CSG reactor physics");
            let (_e2, _f2, p2, eps2) = r.six_factors.two_group_openmc_convention();
            eprintln!(
                "    {label} seed {seed:>3}: kfull = {:.15} k = {:.5} ± {:.5}, p = {p2:.4}, \
                 eps = {eps2:.4}  [{:.1} s]",
                r.keff.k_mean,
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
        let (free_nuclides, sab) = nuclides();
        let (mats, spec) = build_materials();
        // S(a,b) per material, as OpenMC does (gh:#462, #1581): every material
        // that references C12G / C13G (buffer, PyC1, PyC2, graphite and the
        // naive-homogenised zone, which holds the matrix graphite) gets
        // c_Graphite; the kernel, SiC and the free-gas homogenised TRISO carbon
        // (C12 / C13) stay free. The bound copies are appended to the array.
        let mut sab_entries = MaterialSab::for_materials_containing(&mats, nx::C12G, 0, &[]);
        sab_entries.extend(MaterialSab::for_materials_containing(&mats, nx::C13G, 0, &[]));
        let (mats, moving) = assign_material_sab(&mats, &free_nuclides, &[sab], &sab_entries)
            .expect("S(a,b) per material");
        // gh:#1636: `OUTRAM_FHR_NO_RESONANCE_SCATTERING=1` removes the default-on
        // resonance elastic scattering (RVS/DBRC table) from BOTH arms, to price it.
        // Off by default: the ordinary run keeps the physics (CLAUDE.md, correct
        // physics is the default).
        let no_res = std::env::var("OUTRAM_FHR_NO_RESONANCE_SCATTERING")
            .map(|v| !v.is_empty() && v != "0")
            .unwrap_or(false);
        let moving: Vec<Nuclide> = if no_res {
            moving.into_iter().map(Nuclide::without_dbrc).collect()
        } else {
            moving
        };
        eprintln!(
            "  resonance elastic scattering (DBRC/RVS table) on {} of {} nuclides{}",
            moving.iter().filter(|n| n.has_dbrc()).count(),
            moving.len(),
            if no_res { " (ABLATED, gh:#1636)" } else { "" }
        );
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

        let r_fuel = rpt_fuel_outer_radius(R_RPT_INNER, R_FUEL_ZONE, spec.packing_fraction);
        eprintln!(
            "  ring-RPT fuel shell: {R_RPT_INNER:.6} < r < {r_fuel:.6} cm; pebble {R_PEBBLE}, \
             reflective root {R_ROOT}"
        );
        eprintln!(
            "Nuclear data + materials ready in {:.1} s.\n",
            t0.elapsed().as_secs_f64()
        );

        let n = n_seeds();
        let seeds: Vec<u64> = (1..=n as u64).collect();
        eprintln!("{n} seeds per arm, 4000 histories × [30 inactive + 80 active]:");
        // `OUTRAM_FHR_ARMS=at-rest` runs only the ablated arm (so the two arms can
        // run as two processes); the per-seed lines are merged by hand afterwards.
        if std::env::var("OUTRAM_FHR_ARMS").as_deref() == Ok("at-rest") {
            let b = run_arm(&at_rest, &mats, r_fuel, &seeds, "AT-REST");
            let (kb, skb, ekb) = stats(&b.k);
            println!(
                "AT-REST-only: n = {} k_AT-REST = {kb:.5}, sd = {:.0} pcm, sem = {:.0} pcm",
                b.k.len(),
                skb * 1.0e5,
                ekb * 1.0e5
            );
            return;
        }
        let t = Instant::now();
        let a = run_arm(&moving, &mats, r_fuel, &seeds, "MOVING ");
        eprintln!("  MOVING  arm done in {:.1} s", t.elapsed().as_secs_f64());
        // gh:#1636: `OUTRAM_FHR_ARMS=moving` screens the unablated arm alone (a
        // legacy-knob run needs only that arm; the at-rest arm is half the cost).
        if std::env::var("OUTRAM_FHR_ARMS").as_deref() == Ok("moving") {
            let (ka, ska, eka) = stats(&a.k);
            println!(
                "MOVING-only: n = {} k_MOVING = {ka:.5}, sd = {:.0} pcm, sem = {:.0} pcm",
                a.k.len(),
                ska * 1.0e5,
                eka * 1.0e5
            );
            return;
        }
        let t = Instant::now();
        let b = run_arm(&at_rest, &mats, r_fuel, &seeds, "AT-REST");
        eprintln!("  AT-REST arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        let (ka, ska, eka) = stats(&a.k);
        let (kb, skb, ekb) = stats(&b.k);
        // pcm on the eigenvalue itself, the convention gh:#193's row uses.
        let diff = (kb - ka) * 1.0e5;
        let ediff = ((eka * eka) + (ekb * ekb)).sqrt() * 1.0e5;
        let d: Vec<f64> = b.k.iter().zip(&a.k).map(|(x, y)| (x - y) * 1.0e5).collect();
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
        for (label, x, y) in [
            ("p  (2-group)", &a.p, &b.p),
            ("eps(2-group)", &a.eps, &b.eps),
        ] {
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

        // gh:#1636: the -556 record was measured on a build with NO resonance elastic
        // scattering; the default build carries it, which acts on the MOVING arm only.
        // So each configuration is judged against its own like-for-like record.
        let (r_pcm, r_err, r_k, r_build, r_what) = if no_res {
            (
                REFERENCE_PCM,
                REFERENCE_ERR_PCM,
                REFERENCE_K_MOVING,
                REFERENCE_BUILD,
                "the whole-run switch replayed on that commit (gh:#1617), a build without \
                 resonance scattering; here with it ablated (gh:#1636 measured -416 +/- 79)",
            )
        } else {
            (
                SESSION_PCM,
                SESSION_ERR_PCM,
                SESSION_K_MOVING,
                SESSION_BUILD,
                "this driver on the default build, 8 seeds (gh:#1636, 2026-11-17); the -556 \
                 of 60f6bb261 is reproduced only with resonance scattering ablated",
            )
        };
        let verdict = judge_against(r_pcm, r_err, r_k, quote, qerr, ka, eka);
        println!();
        println!("  Prediction on record: {r_pcm:+.0} +/- {r_err:.0} pcm, {r_what}; build {r_build}.");
        println!(
            "  (-2242 pcm / k 1.40546 predates the F-19 fix 45f8d72d; -556 / 60f6bb261 predates \
             default resonance scattering; both struck through in the doc comment.)"
        );
        println!(
            "  Unablated arm: k_MOVING {ka:.5} vs the reference build's {r_k:.5}: \
             {:+.0} pcm ({:+.1} sigma of this run's own sem).",
            verdict.moving_shift_pcm, verdict.moving_shift_sigma
        );
        println!();
        println!("  VERDICT:");
        println!(
            "    gap {:+.0} pcm from the reference, {:.1} sigma of (this run + reference) errors \
             (gh:#1636 task gate abs(z) <= 2; HELD below is the older 3 sigma).",
            verdict.gap_pcm, verdict.gap_sigma
        );
        if verdict.held {
            println!("    HELD. The in-process hook reproduces the recorded like-for-like value.");
        } else {
            println!("    FAILED as stated. At 60f6bb261 the hook and the switch were BIT-IDENTICAL");
            println!("    on 8 of 8 seeds, so this is not a hook-versus-switch disagreement; compare");
            println!("    the unablated-arm line above and re-baseline only with a mechanism, as");
            println!("    gh:#1636 did for resonance scattering.");
        }
    }

    /// Outcome of comparing a measured difference with the re-baselined value.
    pub struct Verdict {
        pub gap_pcm: f64,
        pub gap_sigma: f64,
        pub held: bool,
        pub moving_shift_pcm: f64,
        pub moving_shift_sigma: f64,
    }

    /// Re-baselined like-for-like prediction (gh:#1617, 2026-11-17): the
    /// whole-run switch replayed on `REFERENCE_BUILD`, 8 seeds paired. Fixed
    /// from that replay; never retuned to a result of this driver.
    pub const REFERENCE_PCM: f64 = -556.0;
    /// Seed-to-seed standard error of [`REFERENCE_PCM`] (paired, 8 seeds).
    pub const REFERENCE_ERR_PCM: f64 = 87.0;
    /// k_MOVING of the replay's unablated arm (8-seed mean).
    pub const REFERENCE_K_MOVING: f64 = 1.36625;
    /// The commit the reference was measured on.
    pub const REFERENCE_BUILD: &str = "60f6bb261";

    /// Compare `quote +/- qerr` (pcm) with the re-baselined reference, in
    /// quadrature with the reference's own error; HELD within 3 sigma.
    /// `k_moving` (+/- `k_moving_sem`) is this run's unablated mean, reported against the
    /// reference build's so a shifted baseline is visible.
    pub fn judge(quote: f64, qerr: f64, k_moving: f64, k_moving_sem: f64) -> Verdict {
        judge_against(
            REFERENCE_PCM,
            REFERENCE_ERR_PCM,
            REFERENCE_K_MOVING,
            quote,
            qerr,
            k_moving,
            k_moving_sem,
        )
    }

    /// The default build's own record (gh:#1636, 2026-11-17): this driver, session
    /// build, 8 seeds, resonance scattering on. Measured, with its mechanism
    /// established by ablation (`REFERENCE_PCM` comes back when it is removed);
    /// not tuned.
    pub const SESSION_PCM: f64 = -153.0;
    /// Paired seed-to-seed standard error of [`SESSION_PCM`].
    pub const SESSION_ERR_PCM: f64 = 43.0;
    /// k_MOVING of that run.
    pub const SESSION_K_MOVING: f64 = 1.36307;
    /// Commit the first measurement was taken on (gh:#1583), reproduced seed for
    /// seed by the gh:#1636 step-0 determinism check.
    pub const SESSION_BUILD: &str = "7b5dd858d";

    /// [`judge`] against an explicit reference.
    pub fn judge_against(
        ref_pcm: f64,
        ref_err: f64,
        ref_k_moving: f64,
        quote: f64,
        qerr: f64,
        k_moving: f64,
        k_moving_sem: f64,
    ) -> Verdict {
        let gap = quote - ref_pcm;
        let err = (qerr * qerr + ref_err * ref_err).sqrt();
        let shift = (k_moving - ref_k_moving) * 1.0e5;
        Verdict {
            gap_pcm: gap,
            gap_sigma: gap.abs() / err,
            held: gap.abs() < 3.0 * err,
            moving_shift_pcm: shift,
            moving_shift_sigma: shift / (k_moving_sem * 1.0e5),
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        /// gh:#1636: the default build holds against its own record; the
        /// resonance-scattering-ablated measurement (-416 +/- 79 pcm) is within
        /// 2 sigma of the 60f6bb261 record (-556 +/- 87), the default one is not.
        #[test]
        fn judge_against_session_and_ablated_records() {
            let own = judge_against(
                SESSION_PCM,
                SESSION_ERR_PCM,
                SESSION_K_MOVING,
                SESSION_PCM,
                43.0,
                SESSION_K_MOVING,
                0.0004,
            );
            assert!(own.held && own.gap_sigma < 1e-9);
            let ablated = judge(-416.0, 79.0, 1.36570, 0.0006);
            assert!(ablated.held && ablated.gap_sigma < 2.0, "{}", ablated.gap_sigma);
            let default_vs_old = judge(SESSION_PCM, SESSION_ERR_PCM, SESSION_K_MOVING, 0.0004);
            assert!(default_vs_old.gap_sigma > 2.0);
        }

        /// The verdict logic is pinned on three cases: the reference itself
        /// holds; the stale -2242 does not; and the 2026-11-17 first
        /// measurement (-153 +/- 43 pcm, k_MOVING 1.36307 +/- 0.00041) fails at
        /// about 4 sigma with the baseline visibly shifted by about -318 pcm.
        #[test]
        fn judge_reference_holds_stale_and_shifted_do_not() {
            let v = judge(REFERENCE_PCM, 87.0, REFERENCE_K_MOVING, 0.0004);
            assert!(v.held && v.gap_sigma < 1e-9);
            let stale = judge(-2242.0, 87.0, REFERENCE_K_MOVING, 0.0004);
            assert!(!stale.held && stale.gap_sigma > 10.0);
            let today = judge(-153.0, 43.0, 1.36307, 0.00041);
            assert!(!today.held);
            assert!((today.gap_sigma - 4.15).abs() < 0.05, "{}", today.gap_sigma);
            assert!((today.moving_shift_pcm + 318.0).abs() < 1.0);
        }
    }
}
