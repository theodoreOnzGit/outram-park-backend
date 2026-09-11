//! **HEU-SOL-THERM-009 case 1** — a water-reflected sphere of uranium
//! oxyfluoride solution, run on the same HIGH data path and CSG driver as the
//! FHR pebble.
//!
//! # Why this exists
//!
//! `godiva_keff_endf_local.rs` settled the *fast* half of the machinery against
//! an experiment: ν̄, χ, fast σ, inelastic levels, (n,2n) and the eigenvalue
//! driver reproduce ICSBEP **HEU-MET-FAST-001** to +57 ± 173 pcm. Nothing had
//! settled the *thermal* half, and the FHR pebble study's residual
//! (`op-mzvp.2.12`, +4004 pcm, entirely the fraction of neutrons crossing
//! 0.625 eV) lives there. Every mechanism on this side has been excluded against
//! its own oracle, so the remaining question is no longer "which of our
//! components is wrong" but **"is this code right on a thermal system at all?"**
//! — and that needs a thermal case whose answer does not come from the deck
//! under test.
//!
//! This is that case, and it is deliberately the *complement* of the pebble:
//!
//! | | FHR pebble | HST-009 |
//! |---|---|---|
//! | moderator | graphite (`c_Graphite`) + FLiBe | water (`c_H_in_H2O`) |
//! | fuel | 19.9 % UCO in TRISO grains | 90 % HEU in solution |
//! | U-238 | 80 % of the heavy metal | **5 %** |
//! | geometry | doubly heterogeneous | homogeneous |
//! | reference | one unreproduced deck | a critical experiment |
//!
//! So it splits the search once more. If `k ≈ 1.00` here, the thermal machinery
//! — the S(α,β) path, the free-gas kernel, thermal fission and capture, the
//! eigenvalue driver on a moderated system — is sound, and whatever ails the
//! pebble is specific to **U-238 resonance escape** or to the reference itself.
//! If `k` is ~4 % high here too, the pebble's residual is reproduced on a
//! benchmark with a known answer, and the hunt has a target that does not depend
//! on anyone's deck.
//!
//! # Model, and what is approximated
//!
//! Specification from the ICSBEP model in `mit-crpg/benchmarks`
//! (`icsbep/heu-sol-therm-009/openmc/case-1`, Paul Romano, 2013-07-09) —
//! three concentric spheres:
//!
//! | region | outer radius \[cm\] | material |
//! |---|---|---|
//! | uranium oxyfluoride solution | 11.5177 | U-234/235/236/238, F-19, O, H + `c_H_in_H2O` |
//! | 1100 aluminium | 11.6764 | Al-27, Si, Cu, Zn, Mn-55 |
//! | water reflector | 35.0 (vacuum) | H, O + `c_H_in_H2O` |
//!
//! Three approximations, each stated because each is a real departure:
//!
//! - **U-236 omitted** (8.8837e-6, 0.5 % of the uranium). A weak 1/v absorber at
//!   0.5 % of a strongly absorbing mix; worth tens of pcm.
//! - **O-17 folded into O-16** (0.038 % of the oxygen). Worth single pcm.
//! - **Cu and Zn omitted from the 1100-aluminium tank** (7.6e-5 /b·cm in a
//!   **1.6 mm** shell). `Σ_a ≈ 2.2e-4 cm⁻¹` over 0.16 cm is an absorption
//!   probability of 3.5e-5 per traversal, against Al-27's own `0.0138 cm⁻¹`
//!   which is 60× larger and itself small. Worth a few pcm.
//!
//! None of the three is in the fuel, and together they are two orders of
//! magnitude below the effect being tested.
//!
//! # The reference value
//!
//! **`k_eff = 1.0000`**, because this is a *critical* configuration: ICSBEP
//! benchmark models are experiments measured at delayed critical, corrected to a
//! benchmark model whose `k_eff` is 1.0000 to within the evaluated experimental
//! uncertainty — typically 0.1–0.6 % for a solution system. The case-specific
//! uncertainty is not in the repository and the handbook is not reachable from
//! this environment, so **treat the reference as `1.0000 ± 0.006`**, the
//! pessimistic end of that band. That is ample: the quantity under test is a
//! **4 %** discrepancy, six times the worst-case reference uncertainty.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example hst009_keff
//! ```

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::BoundaryType;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::fhr_pebble::fhr_pebble_geometry;
use outram_mc_libs::physics::compute::ComputeType;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::vv::assert_reproduces_keff;
use std::time::Instant;

/// Room temperature. The benchmark states the reflector at 25 °C; 293.6 K is the
/// tabulated temperature on the `H(H2O)` tape, so no thermal interpolation is
/// involved and the 4.4 K difference is worth single pcm here.
const TEMP_K: f64 = 293.6;

const R_SOLUTION: f64 = 11.5177;
const R_TANK: f64 = 11.6764;
const R_REFLECTOR: f64 = 35.0;

/// Nuclide slots.
mod nx {
    pub const U234: usize = 0;
    pub const U235: usize = 1;
    pub const U238: usize = 2;
    pub const F19: usize = 3;
    pub const O16: usize = 4;
    pub const H1: usize = 5;
    pub const AL27: usize = 6;
    pub const SI28: usize = 7;
    pub const SI29: usize = 8;
    pub const SI30: usize = 9;
    pub const MN55: usize = 10;
}

fn main() {
    eprintln!("HEU-SOL-THERM-009 case 1 — water-reflected sphere of uranium oxyfluoride");
    eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
    let t0 = Instant::now();
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-HinH2O.endf")
            .expect("H(H2O) tape")
            .to_str()
            .expect("path"),
        1, // MAT 1 — H in H2O, ENDF/B-VIII.0
        TEMP_K,
        "c_H_in_H2O",
    )
    .expect("H(H2O) S(a,b)");
    eprintln!("  H(H2O) S(α,β): ENDF/B-VIII.0 tsl-HinH2O (MAT 1) … ok");

    let nuclides: Vec<Nuclide> = vec![
        load("U234", "n-092_U_234-ENDF8.0.endf"),
        load("U235", "n-092_U_235-ENDF8.0.endf"),
        load("U238", "n-092_U_238.endf"),
        load("F19", "n-009_F_019-ENDF8.0.endf"),
        load("O16", "n-008_O_016-ENDF8.0.endf"),
        load("H1", "n-001_H_001-ENDF8.0-Beta6.endf").with_thermal_scattering(sab),
        load("Al27", "n-013_Al_027-ENDF8.0.endf"),
        load("Si28", "n-014_Si_028-ENDF8.0.endf"),
        load("Si29", "n-014_Si_029-ENDF8.0.endf"),
        load("Si30", "n-014_Si_030-ENDF8.0.endf"),
        load("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    ];
    eprintln!(
        "Nuclear data ready in {:.1} s.\n",
        t0.elapsed().as_secs_f64()
    );

    // Atom densities [atoms/barn·cm] exactly as the benchmark model gives them,
    // except where the module docs say otherwise.
    let comp = |v: &[(usize, f64)]| -> Vec<NuclideComponent> {
        v.iter()
            .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect()
    };
    let materials = vec![
        Material {
            id: 1,
            name: "uranium oxyfluoride solution".into(),
            temperature: TEMP_K,
            components: comp(&[
                (nx::U234, 1.7561e-05),
                (nx::U235, 1.6626e-03),
                // U-236 (8.8837e-06) omitted — no tape here; see the module docs.
                (nx::U238, 9.4079e-05),
                (nx::F19, 3.5663e-03),
                // O-17 (1.264344e-05) folded into O-16.
                (nx::O16, 3.334735656e-02 + 1.264344e-05),
                (nx::H1, 5.9587e-02),
            ]),
        },
        Material {
            id: 2,
            name: "1100 aluminium tank".into(),
            temperature: TEMP_K,
            components: comp(&[
                (nx::AL27, 5.9699e-02),
                (nx::SI28, 5.09126279536e-04),
                (nx::SI29, 2.5851979832e-05),
                (nx::SI30, 1.7041740632e-05),
                (nx::MN55, 1.4853e-05),
                // Cu (5.14e-5) and Zn (2.5e-5) omitted; see the module docs.
            ]),
        },
        Material {
            id: 3,
            name: "water reflector".into(),
            temperature: TEMP_K,
            components: comp(&[
                (nx::H1, 6.6659e-02),
                (nx::O16, 3.3316368309e-02 + 1.2631691e-05),
            ]),
        },
    ];

    // Three concentric spheres. `fhr_pebble_geometry` is named for the pebble but
    // is a general concentric-sphere builder: passing `r_inner = 0` drops the
    // inner ball and leaves exactly the three regions this benchmark needs.
    let geom = fhr_pebble_geometry(
        0.0,
        R_SOLUTION,
        R_TANK,
        R_REFLECTOR,
        0, // solution fills r < R_SOLUTION
        1, // aluminium tank
        2, // water reflector, vacuum at R_REFLECTOR
        BoundaryType::Vacuum,
        TEMP_K,
    );

    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 120,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(Default::default()),
        ..KeffSettings::default()
    };
    // Start inside the solution; `only_fissionable` is implicit in the driver.
    let src = SourceBox {
        lower: Position::new(-R_SOLUTION, -R_SOLUTION, -R_SOLUTION),
        upper: Position::new(R_SOLUTION, R_SOLUTION, R_SOLUTION),
    };

    eprintln!(
        "solution r < {R_SOLUTION} cm, 1100-Al tank to {R_TANK} cm, water to {R_REFLECTOR} cm \
         (vacuum)"
    );
    eprintln!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t = Instant::now();
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);
    eprintln!("  transport: {:.1} s", t.elapsed().as_secs_f64());
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP HEU-SOL-THERM-009 case 1 (critical) = 1.0000 ± ~0.006");
    let pcm = (result.k_mean - 1.0) * 1.0e5;
    println!(
        "  Δk from the benchmark = {pcm:+.0} ± {:.0} pcm",
        result.k_std * 1.0e5
    );
    println!(
        "\n  For scale, the FHR pebble sits +4004 pcm above its reference.\n  \
         (The U-238 resonance-escape reading of that residual was later REFUTED:\n  \
         see examples/lct008_keff.rs and GitHub #188. Three LEU-COMP-THERM-008\n  \
         cases sharing one lattice give +2950/+2271/+1713 pcm, a 14-sigma spread\n  \
         that an error in `p` cannot produce.)"
    );

    // ── V&V gate ──────────────────────────────────────────────────────────────
    //
    // `recorded_pcm` is deliberately None: see the note in
    // examples/godiva_keff_endf_local.rs. Fill it in from a real run, with the
    // date, and the gate starts checking drift as well as agreement.
    println!("\n=== V&V gate: ICSBEP HEU-SOL-THERM-009 case 1 ===");
    assert_reproduces_keff(
        "HEU-SOL-THERM-009 case 1 (critical HEU solution)",
        result.k_mean,
        result.k_std,
        ICSBEP_HST009_K,
        ICSBEP_HST009_BAND,
        None,
    );
}

/// ICSBEP **HEU-SOL-THERM-009 case 1** benchmark `k_eff`.
///
/// A critical HEU solution — so the benchmark value is exactly 1.0000 by
/// construction. The band is the evaluation's own stated uncertainty, which is
/// six times Godiva's because a solution assembly's composition and geometry are
/// far harder to pin than a machined metal sphere's.
///
/// This is the **thermal** benchmark of this crate's ICSBEP set, and it is
/// *homogeneous* — which matters for what it can and cannot exclude. The
/// H-in-H2O scattering law (GitHub #188) controls the spatial distribution of
/// the thermal flux, and in a homogeneous assembly there is no spatial
/// distribution for it to get wrong. So reproducing this case does **not**
/// clear the water law; it clears everything else about the thermal path.
const ICSBEP_HST009_K: f64 = 1.0000;

/// The ICSBEP-stated uncertainty on [`ICSBEP_HST009_K`].
const ICSBEP_HST009_BAND: f64 = 0.006;

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
    eprint!("  reconstructing {name:<6} … ");
    let t0 = Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}
