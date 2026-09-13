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
//! Two environment switches cut the cost when only one number is wanted:
//!
//! ```text
//! OUTRAM_RINGRPT_ONLY=csg                 one surface-tracked ring-RPT solve
//!                                         (~5 min instead of ~40) plus the
//!                                         six-factor decomposition
//! OUTRAM_RINGRPT_FREE_GAS_GRAPHITE=1      run with the graphite S(α,β) law
//!                                         REMOVED — every bound carbon moved to
//!                                         free gas. Differencing against the
//!                                         normal run BOUNDS what the whole
//!                                         thermal law is worth on this pebble.
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
//! 4. **The graphite S(α,β) law is priced, and it is small.** Running the
//!    surface-tracked ring-RPT case with the law REMOVED (every bound carbon
//!    moved to free gas, `OUTRAM_RINGRPT_FREE_GAS_GRAPHITE=1`) against the
//!    normal run, 2026-09-12, same seed and statistics:
//!
//!    ```text
//!      graphite S(a,b)   k_eff (ring-RPT CSG)   p (2-group)   vs OpenMC p
//!      on  (the deck)    1.40546 +/- 0.00234    0.5256        +8.54 %
//!      REMOVED           1.40419 +/- 0.00243    0.5238        +8.18 %
//!      difference        -127 +/- 337 pcm       -0.0018       -0.36 points
//!    ```
//!
//!    Deleting the entire bound thermal law -- every defect in it, known and
//!    unknown, together -- is worth **-127 +/- 337 pcm (0.38 sigma)** here and
//!    moves the resonance-escape error by a third of a percentage point out of
//!    8.5. So no defect in that law can carry the +4000 pcm residual, and the
//!    residual's signature (p too high by 8.5 %, epsilon too low by 5.0 %)
//!    survives with no bound law present at all. The moderation in this pebble
//!    is dominated by FLiBe, which is free-gas in both codes; the bound graphite
//!    sits only in the inner core and the 0.1 cm shell.
//!
//!    This is why the mode exists: an *accuracy* statement about a mechanism
//!    ("the law is within 0.05 % of THERMR") cannot bound a residual, because it
//!    says nothing about how much the mechanism is worth. Turning the mechanism
//!    off does.
//!
//! 5. **Anisotropic elastic scattering is priced too, and it is also small.**
//!    `OUTRAM_RINGRPT_ISOTROPIC_ELASTIC=1` drops every nuclide's ENDF MF=4
//!    angular distribution, so elastic scattering is isotropic in CM at every
//!    energy. Same case, same seed, 2026-09-12:
//!
//!    ```text
//!      elastic angle     k_eff (ring-RPT CSG)   p (2-group)   eps (2-group)
//!      MF=4 (the data)   1.40546 +/- 0.00234    0.5256        1.4292
//!      ISOTROPIC-CM      1.40465 +/- 0.00213    0.5253        1.4296
//!      difference        -81 +/- 317 pcm        -0.0003       +0.0004
//!    ```
//!
//!    **-81 +/- 317 pcm (0.26 sigma)**, and neither p nor epsilon moves. This was
//!    the best remaining candidate on an argument from coverage: the
//!    slowing-down verification that covers anisotropy (`xi/xi_0 = 1.000`, eight
//!    nuclides) spans 4 eV to 10 keV, and CM scattering is isotropic throughout
//!    that band, so the MeV anisotropy that sets the fast spectrum was unchecked.
//!    It is checked now (`tests/elastic_anisotropy_vs_endf_mf4.rs`: the sampler
//!    reproduces its own MF=4 mean cosine to 0.0073 worst, and the moderators run
//!    from isotropic at 1 keV to mu-bar +0.60...+0.73 at 14 MeV), and priced now,
//!    and it is not the residual either.
//!
//!    The pricing is only readable *because* of that test. A null result from
//!    switching a mechanism off means "the mechanism is worth nothing" only if
//!    the mechanism was there; had MF=4 silently failed to parse, this run would
//!    have measured the same -81 pcm and meant the opposite.
//!
//! 6. **The pricing harness has a positive control, and it passes.** A table of
//!    null results is only worth reading if the instrument that produced it can
//!    produce a non-null one. `OUTRAM_RINGRPT_TARGET_AT_REST=1` zeroes the
//!    transport temperature, which for a pointwise nuclide changes the kinematics
//!    only (the cross sections were already broadened to 600 K at construction),
//!    so what goes away is free-gas target motion -- the defect bead `op-50vu`
//!    recorded and fixed. Same case, same seed, 2026-09-12:
//!
//!    ```text
//!      target motion     k_eff (ring-RPT CSG)   p vs OpenMC   eps vs OpenMC
//!      sampled (correct) 1.40546 +/- 0.00234    +8.54 %       -4.99 %
//!      AT REST (broken)  1.38304 +/- 0.00223    +6.03 %       -3.69 %
//!      difference        -2242 +/- 323 pcm      -2.51 points  +1.30 points
//!    ```
//!
//!    **-2242 +/- 323 pcm at 6.9 sigma**, on the same 4000 x [30 + 80] statistics
//!    that returned 0.38 and 0.26 sigma for the two mechanisms above. So the
//!    harness resolves a two-thousand-pcm effect comfortably, and those two zeros
//!    are measurements rather than failures to measure.
//!
//! 7. **Caveats 4-6 were measured BEFORE the residual was closed.** They are
//!    still correct as *worth* measurements — that is what they are for, and
//!    their numbers do not depend on what else was wrong at the time — but read
//!    them as the elimination trail that led to GitHub #193 rather than as live
//!    statements about an open gap. The trail was: graphite S(α,β) priced at
//!    -127 +/- 337 pcm, the MF=4 angular law at -81 +/- 317, a deliberately
//!    broken positive control at -2242 +/- 323 to show the harness could resolve
//!    something, the inelastic channel at -4190 +/- 335, and F-19's inelastic
//!    **alone** at -4033 +/- 333. The last row localised it to one nuclide's
//!    inelastic channel, and reading that channel closely found the defect.
//!
//!    The lesson worth keeping is the method. Every earlier exclusion in this
//!    study was an *accuracy* statement — "within 0.05 % of THERMR", "+/-0.04 %
//!    vs NJOY", "resonance integrals +0.00 %" — and an accuracy statement cannot
//!    bound a residual, because it says nothing about how much the mechanism is
//!    worth. The defect that was actually there is not visible as an accuracy
//!    error at all: F-19's inelastic cross section is faithful to its tape to
//!    1e-7 everywhere it exists. It was the 0.0224 b where it should not have
//!    existed that mattered.
//!
//! 8. Not a validated result — an AI-assisted code-to-code check.

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
        // ── Sensitivity switch: how much is anisotropic elastic scattering WORTH? ──
        //
        // `OUTRAM_RINGRPT_ISOTROPIC_ELASTIC=1` drops every nuclide's ENDF MF=4
        // elastic angular distribution, so elastic scattering becomes isotropic in
        // CM at every energy. See `Nuclide::with_isotropic_elastic` for why this
        // is the mechanism most likely to be mispriced: the slowing-down check
        // that covers anisotropy spans 4 eV to 10 keV, where CM scattering is
        // isotropic anyway, so the MeV anisotropy that actually sets the
        // slowing-down power above the resonances is unverified.
        // ── Sensitivity switch: how much is INELASTIC energy loss worth? ──
        //
        // `OUTRAM_RINGRPT_NO_INELASTIC=1` drops every nuclide's resolved inelastic
        // levels, so an inelastic collision scatters elastically instead. The
        // collision rate, absorption and fission are untouched; what goes away is
        // the excitation energy the neutron leaves in the residual nucleus, which
        // is inelastic scattering's whole contribution to moderation.
        //
        // It is the last large un-priced energy-loss channel above ~100 keV, it
        // is what sets how quickly neutrons fall below U-238's fast-fission
        // threshold, and nothing in this crate verifies it: the slowing-down
        // check that covers elastic kinematics spans 4 eV to 10 keV, far below
        // every inelastic threshold here (F-19 110 keV, U-238 45 keV, Be-9
        // 1.7 MeV, C-12 4.4 MeV, O-16 6.0 MeV).
        // Accepts `all`, or a comma-separated list of nuclide names as they appear
        // in `nuclides()` (e.g. `F19`, `U238,U235`), so the channel can be priced
        // per nuclide rather than only wholesale. The pebble's inelastic
        // scattering is not dominated by the same nuclide its fission is.
        let no_inelastic = std::env::var("OUTRAM_RINGRPT_NO_INELASTIC").unwrap_or_default();
        let isotropic_elastic = std::env::var("OUTRAM_RINGRPT_ISOTROPIC_ELASTIC").is_ok();
        let nucs = if !no_inelastic.is_empty() {
            let all = no_inelastic.eq_ignore_ascii_case("all");
            let wanted: Vec<&str> = no_inelastic.split(',').map(str::trim).collect();
            eprintln!(
                "  !! SENSITIVITY MODE: INELASTIC levels dropped for [{no_inelastic}] — those\n\
                 \x20    collisions scatter elastically. Wrong physics on purpose."
            );
            nuclides()
                .into_iter()
                .map(|n| {
                    if all || wanted.iter().any(|w| w.eq_ignore_ascii_case(&n.name)) {
                        n.without_inelastic()
                    } else {
                        n
                    }
                })
                .collect::<Vec<_>>()
        } else if isotropic_elastic {
            eprintln!(
                "  !! SENSITIVITY MODE: elastic scattering forced ISOTROPIC-CM — every\n\
                 \x20    MF=4 angular distribution dropped. Wrong physics on purpose."
            );
            nuclides()
                .into_iter()
                .map(Nuclide::with_isotropic_elastic)
                .collect::<Vec<_>>()
        } else {
            nuclides()
        };
        let (mats, spec) = build_materials();
        // ── Sensitivity switch: how much is the graphite S(α,β) law WORTH here? ──
        //
        // `OUTRAM_RINGRPT_FREE_GAS_GRAPHITE=1` moves every bound-graphite carbon
        // component onto the free-gas carbon nuclide of the same isotope, so the
        // pebble runs with no bound thermal law at all. Differencing the two runs
        // BOUNDS everything the graphite law can be worth on this pebble — every
        // defect in it, known and unknown, together. That bound is what says
        // whether a few-percent error in the law can carry a 4000 pcm residual or
        // cannot, which no accuracy statement about the law itself can settle.
        //
        // It is a diagnostic, not a model: the deck being compared against uses
        // `c_Graphite`, so the free-gas run is deliberately the WRONG model.
        let free_gas_graphite = std::env::var("OUTRAM_RINGRPT_FREE_GAS_GRAPHITE").is_ok();
        let mats = if free_gas_graphite {
            eprintln!(
                "  !! SENSITIVITY MODE: graphite S(α,β) REMOVED — every bound carbon \n\
                 \x20    moved to free gas. This is not the deck's model; it prices the law."
            );
            mats.into_iter().map(free_gas_carbon).collect::<Vec<_>>()
        } else {
            mats
        };
        eprintln!();

        let compute = ComputeType::CpuMultiThread(Default::default());
        // ── Positive control: can the pricing harness SEE a big effect at all? ──
        //
        // `OUTRAM_RINGRPT_TARGET_AT_REST=1` zeroes the transport temperature, which
        // for a HIGH-tier (pointwise) nuclide changes the *kinematics only*: the
        // cross sections were Doppler-broadened to 600 K at construction and are
        // temperature-independent at lookup, so the collision rate is untouched and
        // what goes away is free-gas target motion. That is exactly the defect bead
        // `op-50vu` recorded — a target held at rest can only take energy away, so
        // there is no thermal equilibrium at all — and it was worth ~1700 pcm.
        //
        // A table of null results is only worth reading if the instrument that
        // produced it can produce a non-null one. This row is that demonstration.
        let target_at_rest = std::env::var("OUTRAM_RINGRPT_TARGET_AT_REST").is_ok();
        if target_at_rest {
            eprintln!(
                "  !! POSITIVE CONTROL: free-gas target motion REMOVED — transport\n\
                 \x20    temperature zeroed. Known-broken on purpose."
            );
        }
        let keff = KeffSettings {
            n_particles: 4000,
            n_inactive: 30,
            n_active: 80,
            temperature_k: if target_at_rest { 0.0 } else { TEMP_K },
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
        check_majorant_bounds(&majorant, &mats, &nucs);

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

        vv_gate(
            (explicit.k_mean, explicit.k_std),
            (rpt.k_mean, rpt.k_std),
            (naive.k_mean, naive.k_std),
            (rpt_csg.keff.k_mean, rpt_csg.keff.k_std),
            OMC_EXPLICIT,
            OMC_RPT,
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

    /// Delta tracking is unbiased only while `Σ_maj ≥ Σ_t` **everywhere**; an
    /// under-bound is a silent bias, not a crash, and it loses collisions exactly
    /// where `Σ_t` spikes — the U-238 resonance peaks. That would show up as too
    /// little resonance absorption, `p` too high and `k` too high, which is the
    /// shape of this study's residual, so it is measured rather than assumed.
    ///
    /// [`Majorant::bounding`] lays 4096 log bins over 11 decades (0.64 % wide)
    /// and sub-samples each 32 times, so the sampling pitch is ~0.021 % in energy.
    /// A Doppler width `Δ = √(4EkT/A)` is 1.1 % of E at the 6.674 eV resonance but
    /// only ~0.02 % by 20 keV — comparable to the pitch — so the high-keV
    /// resonances are where a peak can slip between sub-samples.
    ///
    /// The check therefore uses the **nuclides' own reconstructed grids**
    /// ([`Nuclide::native_energy_grid`]), not another log grid: those are the
    /// points σ(E) is actually tabulated on, so every resonance peak is a node.
    /// A log grid would miss the same peaks the majorant does and agree with it
    /// for the wrong reason.
    fn check_majorant_bounds(majorant: &Majorant, mats: &[Material], nucs: &[Nuclide]) {
        let t0 = std::time::Instant::now();
        let mut worst = (0.0_f64, 0.0_f64, 0usize); // ratio, energy, material
        let mut n_pts = 0usize;
        for nuc in nucs {
            for e in nuc.native_energy_grid(1.0e-4, 2.0e7) {
                n_pts += 1;
                let maj = majorant.at(e);
                if !(maj > 0.0) {
                    continue;
                }
                for (m, mat) in mats.iter().enumerate() {
                    let ratio = mat.macro_xs_total(e, nucs) / maj;
                    if ratio > worst.0 {
                        worst = (ratio, e, m);
                    }
                }
            }
        }
        let (ratio, e, m) = worst;
        eprintln!(
            "Majorant check: worst Σ_t/Σ_maj = {ratio:.6} at {e:.6e} eV (material {m}, \
             {}) over {n_pts} reconstructed grid points x {} materials, {:.1?}",
            mats[m].name,
            mats.len(),
            t0.elapsed()
        );
        assert!(
            ratio <= 1.0,
            "delta-tracking majorant UNDER-BOUNDS by {:.2} % at {e:.6e} eV in {} -- \
             the flight is biased toward too few collisions exactly where Σ_t spikes, \
             which under-counts resonance absorption silently",
            100.0 * (ratio - 1.0),
            mats[m].name
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
        // The OpenMC deck reports a TWO-group decomposition at the same 0.625 eV
        // cadmium cutoff this crate uses, so k, eta and f are already
        // like-for-like -- but p and epsilon are not: the three-group form
        // carries fast absorption in epsilon. Print the converted pair too, so
        // the V&V record can quote a comparison that means something.
        let (eta2, f2, p2, eps2) = s.two_group_openmc_convention();
        eprintln!(
            "  [{tag}] two-group (OpenMC convention, 0.625 eV): η {eta2:.4} f {f2:.4} \
             p {p2:.4} ε {eps2:.4}"
        );
        print_spectrum(tag, r);
    }

    /// The lethargy-normalised flux, decade by decade.
    ///
    /// Recorded because the residual is now known to be **one scalar** — the
    /// fraction of neutrons crossing 0.625 eV — and the spectrum is the thing
    /// that scalar is a moment of. Two features are worth reading off it: the
    /// slowing-down plateau (`ψ(u)` should be roughly flat from ~1 keV to
    /// ~100 keV, where `ψ = q/ξΣ_s` and little is absorbed), and the depth of the
    /// resonance dips relative to it.
    ///
    /// `ψ` is normalised so `Σ ψ_i Δu_i = 1` over the whole range, so it is a
    /// *shape*: comparable between runs and between codes, not an absolute flux.
    fn print_spectrum(
        tag: &str,
        r: &outram_mc_libs::physics::reactor_physics::ReactorPhysicsReport,
    ) {
        let sp = &r.spectrum;
        let edges = &sp.energy_edges_ev;
        eprintln!("  [{tag}] flux per unit lethargy (Σψ·Δu = 1 over the whole range):");
        // Decade bands, plus the bands that matter for resonance escape.
        const BANDS: &[(f64, f64, &str)] = &[
            (1.0e-4, 1.0e-2, "1e-4 – 1e-2 eV"),
            (1.0e-2, 0.1, "1e-2 – 0.1  eV  (Maxwellian peak)"),
            (0.1, 0.625, "0.1  – 0.625 eV"),
            (0.625, 10.0, "0.625 – 10   eV  (6.674 eV)"),
            (10.0, 1.0e2, "10   – 100  eV"),
            (1.0e2, 1.0e3, "100  – 1e3  eV"),
            (1.0e3, 2.0e4, "1e3  – 2e4  eV  (plateau)"),
            (2.0e4, 1.0e5, "2e4  – 1e5  eV"),
            (1.0e5, 1.0e6, "1e5  – 1e6  eV"),
            (1.0e6, 2.0e7, "1e6  – 2e7  eV"),
        ];
        for &(lo, hi, label) in BANDS {
            let (mut num, mut du) = (0.0_f64, 0.0_f64);
            for i in 0..sp.flux_per_lethargy.len() {
                let (e0, e1) = (edges[i], edges[i + 1]);
                if e1 <= lo || e0 >= hi {
                    continue;
                }
                let (a, b) = (e0.max(lo), e1.min(hi));
                if b <= a {
                    continue;
                }
                let w = (b / a).ln();
                num += sp.flux_per_lethargy[i].mean * w;
                du += w;
            }
            if du > 0.0 {
                eprintln!("      {label:<34} ψ̄ = {:.5}   (Δu = {du:.3})", num / du);
            }
        }
    }

    /// V&V gate for the FHR ring-RPT pebble against the OpenMC reference.
    ///
    /// # This case DISAGREES with its reference, and the gate is built around that
    ///
    /// Everything here is a **characterisation** gate, not a pass/fail on physics
    /// this crate has got right. The pebble sits about **+4000 pcm** above OpenMC
    /// and the cause is an open defect (GitHub #188 — the H-in-H2O scattering
    /// kernel is too narrow; see `examples/h2o_kernel_vs_njoy_thermr.rs`). Pinning
    /// the disagreement is the whole point: it is the one number the whole
    /// investigation moves, and if it drifts silently nobody can tell a fix from a
    /// regression.
    ///
    /// **Do not widen these envelopes.** When #188 lands, the explicit-TRISO offset
    /// must *fall*, and the right response is to record the new value and tighten.
    ///
    /// # The claims, in order of how much they are worth
    ///
    /// 1. **`RPT - explicit` is small.** This is the only claim here that is a
    ///    statement about a method working, and it is independent of the absolute
    ///    offset: both sides carry the same data defect, so it cancels. Recorded
    ///    `+226 +/- 316 pcm (0.71 sigma)` against the reference's own
    ///    `-31 +/- 92 pcm`.
    /// 2. **`naive - explicit` is large and negative.** Naive homogenisation throws
    ///    away the double heterogeneity, so it MUST be badly wrong. Recorded
    ///    `-3191 pcm (10.7 sigma)`. Without this claim, a code that had quietly
    ///    stopped modelling the TRISO structure at all would pass claim 1 trivially
    ///    — RPT and explicit would agree because both had become the naive case.
    /// 3. **The CSG and delta-tracking ring-RPT results agree.** Two different
    ///    transport methods, different initial source distributions, same answer:
    ///    recorded 18 pcm apart (0.05 sigma). This separates a tracking bug from a
    ///    data one.
    /// 4. **The absolute offset against OpenMC.** Until 2026-09-12 this was the
    ///    loosest claim here and a characterisation of an open defect: +4004 pcm
    ///    on the explicit pebble, gated only against a +/-1500 pcm drift. GitHub
    ///    #193 closed it to **-469 pcm (2.2 sigma)** and the ring-RPT pebble to
    ///    **-85 pcm (0.4 sigma)**, so it is now a real absolute-k agreement and a
    ///    drift in it is a regression rather than a change in a known-wrong
    ///    number. The gate is tightened to +/-1000 pcm accordingly, which is 3.3x
    ///    the ~305 pcm standard error of a run-to-run difference; going tighter
    ///    means buying histories, not shrinking the constant.
    ///
    /// # Results (recorded in `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`)
    ///
    /// Reflective sphere r = R_ROOT, the domain OpenMC used, so the absolute k is
    /// comparable and not only the method delta:
    ///
    /// ```text
    ///   method                        this crate            vs explicit       vs OpenMC
    ///   explicit TRISO          1.40514 +/- 0.00204              —             +4004 pcm
    ///   ring-RPT                1.40739 +/- 0.00241   +226 +/- 316 (0.71 s)    +4260 pcm
    ///   naive homogenised       1.37323 +/- 0.00219   -3191      (10.7 s)          —
    ///   ring-RPT (CSG)          1.40757 +/- 0.00224    +18        (0.05 s)     +4278 pcm
    ///
    ///   OpenMC reference:  explicit 1.36510 +/- 0.00063,  ring-RPT 1.36479 +/- 0.00067
    /// ```
    ///
    /// Re-measured 2026-09-11, first run with this gate in place:
    ///
    /// ```text
    ///   explicit TRISO          1.40514 +/- 0.00204              —             +4004 pcm
    ///   ring-RPT                1.40527 +/- 0.00213    +14 +/- 295 (0.05 s)    +4048 pcm
    ///   naive homogenised       1.37187 +/- 0.00232   -3327      (10.8 s)          —
    ///   ring-RPT (CSG)          1.40745 +/- 0.00214   +218 +/- 302 (0.72 s)        —
    /// ```
    ///
    /// # 2026-09-12 — THE +4000 pcm OFFSET IS CLOSED (GitHub #193)
    ///
    /// Threshold reactions had a non-zero cross section **below** their
    /// threshold: `eval_mt` clamped an MF=3 section to its endpoint value outside
    /// the tabulated grid, which is right at the top and wrong at the bottom of a
    /// threshold section. ENDF/B-VIII.0 F-19 opens MT=51 at
    /// `(115 840 eV, 0.018129 b)`, so F-19 carried a constant 0.0224 b of
    /// inelastic scattering at every energy below 115 keV — and because
    /// `two_body_scatter` clamps a negative outgoing CM energy to zero, each of
    /// those collisions dropped the neutron to `E/(A+1)²`, a factor of 394. About
    /// one F-19 collision in 170, through the whole resonance region, was
    /// teleporting the neutron past the U-238 resonances six lethargy units at a
    /// time. That is a resonance-escape error by construction, which is what this
    /// study had localised the residual to.
    ///
    /// Full re-measurement, same 4000 x [30 + 80] statistics, reflective sphere:
    ///
    /// ```text
    ///   method                    this crate            vs explicit         vs OpenMC
    ///   explicit TRISO      1.36041 +/- 0.00207             —           -469 pcm (2.2 s)
    ///   ring-RPT            1.36394 +/- 0.00204   +353 +/- 291 (1.2 s)   -85 pcm (0.4 s)
    ///   naive homogenised   1.32759 +/- 0.00201   -3282      (11.4 s)        —
    ///   ring-RPT (CSG)      1.36140 +/- 0.00229   -254 +/- 307 vs delta       —
    ///
    ///   OpenMC reference:  explicit 1.36510 +/- 0.00063,  ring-RPT 1.36479 +/- 0.00067
    /// ```
    ///
    /// **+4004 -> -469 pcm** on the explicit pebble and **+4260 -> -85 pcm** on
    /// ring-RPT.
    ///
    /// # 2026-09-13 — the continuous thermal kernel took the last 500 pcm
    ///
    /// `ThermalScattering::sample` stopped reading back one of N discrete
    /// equiprobable outgoing energies and started interpolating the quantile
    /// function they tabulate. Re-measured on the same statistics:
    ///
    /// ```text
    ///   method                    this crate            vs explicit          vs OpenMC
    ///   explicit TRISO      1.36547 +/- 0.00229             —            +37 pcm (0.2 s)
    ///   ring-RPT            1.36363 +/- 0.00216  -184 +/- 315 (0.58 s)  -116 pcm (0.5 s)
    ///   naive homogenised   1.32971 +/- 0.00237  -3575      (10.9 s)         —
    ///   ring-RPT (CSG)      1.36810 +/- 0.00213  +331 vs OpenMC  (1.5 s)
    /// ```
    ///
    /// The explicit pebble is now **+37 pcm** from a reference carrying
    /// +/-63 pcm of its own — 0.2 sigma, which is agreement rather than a
    /// residual. `RPT - explicit` is -184 +/- 315 pcm against the reference's
    /// own -31 +/- 92, and `naive - explicit` stays a real -3575 pcm at
    /// 10.9 sigma. The six factors, from the CSG case, land on the reference in all
    /// four: eta -0.00 %, f -0.01 %, p +0.04 %, epsilon -0.08 %, against
    /// +0.11 / -0.17 / +8.54 / -4.99 % before. The `naive - explicit`
    /// double-heterogeneity claim is unchanged in character (-3282 pcm at 11.4
    /// sigma, against -3191 recorded), which is the check that the fix did not
    /// simply flatten the physics.
    ///
    /// Superseded, kept for the record — re-measured 2026-09-12, CSG case only
    /// (`OUTRAM_RINGRPT_ONLY=csg`), after the emission-table resize (`5916b917`,
    /// `f540b6ac`, GitHub #190) made the graphite and water thermal kernels finer
    /// in both dimensions:
    ///
    /// ```text
    ///   ring-RPT (CSG)          1.40546 +/- 0.00234   (was 1.40757 +/- 0.00224)
    /// ```
    ///
    /// **-211 pcm, 0.65 sigma** — consistent with the -199 +/- 317 pcm that
    /// change's own author measured on this case, and with the -371 +/- 194 pcm
    /// it is worth on a dedicated graphite k_inf medium. The three delta-tracked
    /// rows above have NOT been re-run at this commit and are left as recorded;
    /// the drift gate below is +/-1500 pcm, so a move of this size does not fire
    /// it, and quoting a number that was not measured would be worse than
    /// quoting a slightly stale one that was.
    ///
    /// The explicit-TRISO case reproduces **exactly** (same seed, deterministic
    /// packing). The three method deltas move by a fraction of their own sigma
    /// between runs — `RPT - explicit` from +226 to +14, `CSG - delta` from +18
    /// to +218 — which is what a ~300 pcm statistical error looks like and is why
    /// those claims are gated at 4 sigma rather than pinned to a value. The
    /// `naive - explicit` claim, being a real ~3300 pcm effect, barely moves.
    ///
    /// # What was REFUTED, and is recorded here so it is not re-derived
    ///
    /// This residual was attributed to self-shielded U-238 resonance absorption, and
    /// a quantitative prediction (+3200 pcm predicted against +2950 measured) even
    /// appeared to confirm it. **That was a coincidence and the reading is refuted.**
    /// ICSBEP LEU-COMP-THERM-008 ships several independently critical cases sharing
    /// one lattice; running three gives `dk` of +2950, +2271 and +1713 pcm — a
    /// spread of 1237 +/- 86 pcm, **14 sigma**. An error in resonance escape `p` is
    /// a property of the lattice, so it would give the *same* offset in all three.
    /// It does not. See `examples/lct008_keff.rs`.
    fn vv_gate(
        explicit: (f64, f64),
        rpt: (f64, f64),
        naive: (f64, f64),
        rpt_csg: (f64, f64),
        omc_explicit: (f64, f64),
        omc_rpt: (f64, f64),
    ) {
        /// Recorded `explicit TRISO - OpenMC explicit`, pcm.
        ///
        /// **Was +4004. Re-measured 2026-09-12 after GitHub #193** — threshold
        /// reactions had a non-zero cross section below their threshold, and
        /// F-19's spurious sub-threshold inelastic channel was teleporting one
        /// collision in ~170 past the U-238 resonances, six lethargy units at a
        /// time. That was the whole residual. That run read **+37 pcm**.
        ///
        /// **Re-measured again 2026-09-13** after MT=91/MT=16 moved from the
        /// Weisskopf evaporation stand-in to the evaluation's own ENDF **MF=6
        /// LAW=1** law (gh:#192 item 1). Both arms moved **down**, the same
        /// direction as Godiva, where the switch was priced at
        /// **−105 ± 32 pcm (3.3 sigma)** over 64 seeds per arm
        /// (`examples/godiva_mf6_continuum_ensemble.rs`).
        ///
        /// **A caution that applies to both constants below.** Each is a
        /// **single run**, and the explicit arm alone carries ~230 pcm of
        /// statistics, so two runs of unchanged code differ by ~325 pcm of
        /// standard error. The −503 pcm drift recorded here is therefore
        /// **1.5 sigma** of run-to-run noise: consistent with Godiva's measured
        /// effect in sign and plausible in size, but **not resolved by this run**.
        /// Pooling these two cases over seeds the way Godiva was pooled is the
        /// honest fix and has not been done — see gh:#196 / `bn:op-awwi`, which
        /// is about exactly this class of single-draw baseline.
        const RECORDED_EXPLICIT_VS_OMC_PCM: f64 = -466.0;
        /// Recorded `ring-RPT - OpenMC ring-RPT`, pcm. Was +4260, then −116;
        /// see [`RECORDED_EXPLICIT_VS_OMC_PCM`] for the 2026-09-13 re-measurement
        /// and the single-draw caution that applies here too (this arm drifted
        /// −120 pcm, well inside its own noise).
        const RECORDED_RPT_VS_OMC_PCM: f64 = -236.0;
        /// How far the recorded absolute offsets may move before this gate fires.
        ///
        /// **Tightened 5x, from 1500 pcm, when #193 closed the offset.** It cannot
        /// go much below this and still be a gate on physics rather than on the
        /// harness: each side carries ~215 pcm of statistics, so the difference
        /// between the recorded run and a fresh one has a standard error near
        /// 305 pcm and 1000 pcm is only 3.3 of those. Buying a tighter pin means
        /// buying more histories, not a smaller number here.
        const ABSOLUTE_DRIFT_GATE_PCM: f64 = 1000.0;

        let pcm = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0) * 1.0e5;
        let sigma =
            |a: (f64, f64), b: (f64, f64)| ((a.1 * a.1 + b.1 * b.1).sqrt() * 1.0e5).max(1.0);

        eprintln!("\n=== V&V gate: ring-RPT pebble vs the OpenMC reference ===");

        // 1. The method delta. The only claim here about something working.
        let d_rp = pcm(rpt, explicit);
        let s_rp = sigma(rpt, explicit);
        eprintln!(
            "  RPT - explicit  = {d_rp:+.0} +/- {s_rp:.0} pcm ({:.2} sigma)  \
             [recorded +226 +/- 316, reference -31 +/- 92]",
            d_rp.abs() / s_rp
        );
        assert!(
            d_rp.abs() <= 4.0 * s_rp,
            "ring-RPT and explicit TRISO now differ by {d_rp:+.0} pcm, {:.1} sigma of \
             this run's own statistics ({s_rp:.0} pcm). Both carry the same data, so \
             the ~+4000 pcm offset against OpenMC cancels in this difference and what \
             is left is the RPT approximation itself. Recorded: +226 +/- 316 pcm \
             (0.71 sigma), against the reference deck's own -31 +/- 92 pcm.",
            d_rp.abs() / s_rp,
        );

        // 2. Naive homogenisation MUST be badly wrong. Without this, a code that had
        //    stopped modelling the TRISO structure at all would pass claim 1.
        let d_nv = pcm(naive, explicit);
        let s_nv = sigma(naive, explicit);
        eprintln!(
            "  naive - explicit = {d_nv:+.0} +/- {s_nv:.0} pcm ({:.1} sigma)  [recorded -3191, 10.7 sigma]",
            d_nv.abs() / s_nv
        );
        assert!(
            d_nv < -1000.0 && d_nv.abs() > 5.0 * s_nv,
            "naive homogenisation is only {d_nv:+.0} pcm from explicit TRISO \
             ({:.1} sigma). It must be badly and NEGATIVELY wrong — smearing the fuel \
             through the matrix destroys the resonance self-shielding the TRISO \
             kernels provide, which is the entire double-heterogeneity effect \
             (recorded -3191 pcm, 10.7 sigma).\n\
             If naive now agrees with explicit, the TRISO structure is not being \
             modelled at all — and the RPT-vs-explicit agreement asserted above \
             becomes trivially true and meaningless.",
            d_nv.abs() / s_nv,
        );

        // 3. Two transport methods, one answer: separates tracking from data.
        let d_csg = pcm(rpt_csg, rpt);
        let s_csg = sigma(rpt_csg, rpt);
        eprintln!(
            "  ring-RPT CSG - ring-RPT delta-tracked = {d_csg:+.0} +/- {s_csg:.0} pcm \
             ({:.2} sigma)  [recorded +18, 0.05 sigma]",
            d_csg.abs() / s_csg
        );
        assert!(
            d_csg.abs() <= 4.0 * s_csg,
            "surface-tracked CSG and delta-tracked ring-RPT now differ by {d_csg:+.0} \
             pcm ({:.1} sigma). They are the same geometry and the same data through \
             two different transport methods and two different initial source \
             distributions; recorded 18 pcm apart (0.05 sigma). A departure here is a \
             TRACKING defect, which is a different search from the data one (#188).",
            d_csg.abs() / s_csg,
        );

        // 4. The absolute offset. No longer a characterisation of an open
        //    defect — see the doc comment.
        for (label, ours, omc, recorded) in [
            (
                "explicit TRISO",
                explicit,
                omc_explicit,
                RECORDED_EXPLICIT_VS_OMC_PCM,
            ),
            ("ring-RPT", rpt, omc_rpt, RECORDED_RPT_VS_OMC_PCM),
        ] {
            let d = pcm(ours, omc);
            eprintln!(
                "  {label:<15} vs OpenMC = {d:+.0} pcm  [recorded {recorded:+.0}, \
                 drift gate +/-{ABSOLUTE_DRIFT_GATE_PCM:.0}]"
            );
            assert!(
                (d - recorded).abs() <= ABSOLUTE_DRIFT_GATE_PCM,
                "{label} is now {d:+.0} pcm from OpenMC, against the {recorded:+.0} pcm \
                 recorded in verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md \
                 — a drift of {:+.0} pcm.\n\
                 The +4000 pcm era ended with GitHub #193; both offsets are now \
                 within a couple of sigma of zero, so this is no longer a \
                 characterisation pin on an open defect but a real absolute-k \
                 agreement, and a drift here is a REGRESSION. Record the new value \
                 and re-tighten only if something was genuinely fixed; otherwise \
                 find what moved. Never widen the tolerance.",
                d - recorded,
            );
        }
    }
}
