//! **V&V: `Majorant::bounding` bounds `Sigma_t` on ENDF/B-VIII.0 data**
//! (GitHub #585).
//!
//! # Why this exists
//!
//! Delta tracking accepts a collision with probability `Sigma_t / Sigma_maj`.
//! Where `Sigma_t > Sigma_maj` the excess collisions are never sampled. The run
//! completes and nothing errors, so an under-bound majorant is a **silent**
//! bias. On 2026-10-05 (#528) the old bin-sampled `Majorant::bounding`
//! (4096 log bins x 32 samples, margin 0.1) left the true `Sigma_t` of the
//! HTR-10 UO2 kernel at **1.18x** the majorant at 1.689 MeV, on ENDF/B-VIII.0.
//! #585 rebuilt `bounding` on the union of every nuclide's own breakpoints.
//! That makes it a bound by construction on pointwise and S(alpha,beta) data,
//! because those are linear or convex between their nodes.
//!
//! # Methodology
//!
//! Each case builds `Majorant::bounding` exactly as its caller does, then runs
//! `Majorant::audit`. The audit scans 2 000 001 log-spaced energies over
//! 1e-5 eV to 20 MeV, every breakpoint of every nuclide in use with its
//! one-ulp neighbours, and the midpoint of every interval between
//! breakpoints. Each energy is checked against each material's
//! `macro_xs_total_upper_bound`, which takes the largest URR band.
//!
//! **Pass:** worst `Sigma_t / Sigma_maj <= 1` for the new construction. The
//! HTR-10 case also audits the old construction
//! (`Majorant::bounding_without_breakpoints`) and requires it to **fail**.
//! That second check shows this instrument can see the defect it guards
//! against.
//!
//! - **HTR-10 fuel zone:** the `examples/htr10_fuel_zone_kinf.rs` kernel and
//!   matrix (IAEA-TECDOC-1382 Table 4-38 densities, as that example cites),
//!   ENDF/B-VIII.0 through this workspace's NJOY port at 293.15 K, tolerance
//!   1e-3. URR tables and DBRC are on by the constructor. Bound graphite
//!   S(alpha,beta) (`tsl-reactor-graphite-30P`, MAT 32) is on C-12 and C-13.
//!   Margin 0.1, `[1e-4, 2e7]` eV, 4096 x 32.
//! - **Godiva HEU metal:** `examples/godiva_kinf_vs_openmc.rs`'s U-234/235/238
//!   at 293.6 K. Margin 0.1, `[1e-5, 2e7]` eV, 4000 x 24.
//!
//! # Results (2026-10-05, ENDF/B-VIII.0, release build)
//!
//! Measured with `--test-threads=2` on cores 2-3 of a 2.1 GHz Xeon (KVM).
//!
//! | case | construction | nodes | worst `Sigma_t/Sigma_maj` | at | energies |
//! |---|---|---|---|---|---|
//! | HTR-10 kernel + matrix | `bounding` (union grid) | 363 755 | **0.9092** | 9.863e-5 eV, kernel | 2 947 800 |
//! | HTR-10 kernel + matrix | ablation, pre-#585 | 4 097 | **1.1839** | 1.68934 MeV, kernel | 2 947 800 |
//! | Godiva HEU | `bounding` (union grid) | 346 988 | **0.9091** | 20 MeV | 3 019 956 |
//! | Godiva HEU | ablation, pre-#585 | 4 001 | 0.9678 | 17.02 keV (URR) | 3 019 956 |
//! | TRISO notebook (LOW) | `bounding` (union grid) | 133 808 | 0.9436 | 15.05 keV | 427 332 |
//!
//! **Interpretation.** The new construction bounds both ENDF sets. Its worst
//! ratio is `1/1.1 = 0.9091`, the margin alone: `Sigma_t` touches the
//! tabulated value at a node and is nowhere above it. The ablation
//! reproduces the #528 defect, 18.4 % short at 1.689 MeV. Godiva was
//! **bounded by the old construction too** (0.9678). Its pointwise structure
//! at 4000 x 24 happens to stay inside the 10 % margin, so the Godiva
//! records were not measured on an under-bound majorant. The never-below
//! check passes at 400 001 energies, so the new majorant is at least the old
//! one everywhere.
//!
//! Cost: 391 s for the file, dominated by the U-235/U-238 reconstructions with
//! URR tables (~70 s each) and the 3 M-energy audits. The two ENDF tests are
//! therefore behind `long-tests`.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::{Majorant, MajorantAudit};
use outram_mc_libs::pebble_beds::htr10::{
    C12_ATOM_FRACTION_OF_NATURAL_C, C13_ATOM_FRACTION_OF_NATURAL_C,
};

/// The audit's scan: 2 000 000 log intervals over the full transport range.
const AUDIT_LO: f64 = 1.0e-5;
const AUDIT_HI: f64 = 2.0e7;
const AUDIT_N: usize = 2_000_000;

fn load(file: &str, name: &str, temp_k: f64) -> Option<Nuclide> {
    let p = reference_endf(file)?;
    Some(
        Nuclide::from_endf_file(&p, name, temp_k, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({file}): {e}")),
    )
}

fn mat(id: i32, name: &str, temp: f64, comps: &[(usize, f64)]) -> Material {
    Material {
        id,
        name: name.into(),
        temperature: temp,
        components: comps
            .iter()
            .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect(),
    }
}

fn report(label: &str, m: &Majorant, a: &MajorantAudit, mats: &[Material]) {
    println!(
        "{label}: {} nodes; worst Sigma_t/Sigma_maj = {:.4} at {:.5e} eV in '{}' \
         ({} energies)",
        m.len(),
        a.worst_ratio,
        a.energy_ev,
        mats[a.material].name,
        a.energies_checked
    );
}

/// HTR-10 UO2 kernel + graphite matrix on ENDF/B-VIII.0 with 30P graphite
/// S(alpha,beta): the case that measured 1.18 at 1.689 MeV.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs 7 ENDF nuclides + audits 3M energies (~6.5 min on 2 cores); skipped under --no-default-features"
)]
fn htr10_kernel_and_matrix_are_bounded_on_endf() {
    let t = 293.15;
    let Some(u235) = load("n-092_U_235-ENDF8.0.endf", "U235", t) else {
        eprintln!("SKIP: reference-data/endf/ not present");
        return;
    };
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-reactor-graphite-30P.endf")
            .expect("tsl-reactor-graphite-30P.endf")
            .to_str()
            .expect("utf-8 path"),
        32,
        t,
        "c_Graphite",
    )
    .expect("graphite S(alpha,beta)");
    let nuclides = vec![
        u235,
        load("n-092_U_238.endf", "U238", t).expect("U-238 tape"),
        load("n-008_O_016-ENDF8.0.endf", "O16", t).expect("O-16 tape"),
        load("n-006_C_012-ENDF8.0.endf", "C12", t)
            .expect("C-12 tape")
            .with_thermal_scattering(sab.clone()),
        load("n-006_C_013-ENDF8.0.endf", "C13", t)
            .expect("C-13 tape")
            .with_thermal_scattering(sab),
        load("n-005_B_010-ENDF8.0.endf", "B10", t).expect("B-10 tape"),
        load("n-005_B_011-ENDF8.0.endf", "B11", t).expect("B-11 tape"),
    ];
    // Densities [atoms/b-cm]: examples/htr10_fuel_zone_kinf.rs.
    let mc = 8.674169e-2;
    let mats = vec![
        mat(
            1,
            "HTR-10 UO2 kernel",
            t,
            &[
                (0, 3.992067e-3),
                (1, 1.924449e-2),
                (2, 4.647329e-2),
                (5, 1.849637e-8),
                (6, 7.445022e-8),
            ],
        ),
        mat(
            2,
            "HTR-10 graphite matrix",
            t,
            &[
                (3, mc * C12_ATOM_FRACTION_OF_NATURAL_C),
                (4, mc * C13_ATOM_FRACTION_OF_NATURAL_C),
                (5, 2.244010e-8),
                (6, 9.032424e-8),
            ],
        ),
    ];

    let new = Majorant::bounding(&mats, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let a_new = new.audit(&mats, &nuclides, AUDIT_LO, AUDIT_HI, AUDIT_N);
    report("HTR-10 bounding (union grid)", &new, &a_new, &mats);

    let old = Majorant::bounding_without_breakpoints(&mats, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let a_old = old.audit(&mats, &nuclides, AUDIT_LO, AUDIT_HI, AUDIT_N);
    report("HTR-10 ABLATION bounding_without_breakpoints", &old, &a_old, &mats);

    assert!(
        a_new.worst_ratio <= 1.0,
        "Majorant::bounding UNDER-BOUNDS Sigma_t by {:.3} % at {:.5e} eV in '{}'",
        (a_new.worst_ratio - 1.0) * 100.0,
        a_new.energy_ev,
        mats[a_new.material].name
    );
    assert!(
        a_old.worst_ratio > 1.0,
        "the audit no longer sees the pre-#585 under-bound ({:.4}); either the \
         data changed or the instrument went blind",
        a_old.worst_ratio
    );
}

/// Godiva HEU metal (U-234/235/238) on ENDF/B-VIII.0, built exactly as
/// `godiva_kinf_vs_openmc` builds it: a second, fast-spectrum material set
/// with URR tables on every uranium isotope.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs U-234/235/238 from ENDF + audits 3M energies (~6 min); skipped under --no-default-features"
)]
fn godiva_heu_is_bounded_on_endf() {
    let t = 293.6;
    let Some(u234) = load("n-092_U_234-ENDF8.0.endf", "U234", t) else {
        eprintln!("SKIP: reference-data/endf/ not present");
        return;
    };
    let nuclides = vec![
        u234,
        load("n-092_U_235-ENDF8.0.endf", "U235", t).expect("U-235 tape"),
        load("n-092_U_238.endf", "U238", t).expect("U-238 tape"),
    ];
    let mats = vec![mat(
        1,
        "Godiva HEU",
        t,
        &[(0, 4.9184e-4), (1, 4.4994e-2), (2, 2.4984e-3)],
    )];
    let new = Majorant::bounding(&mats, &nuclides, 1.0e-5, 2.0e7, 4000, 24, 0.10);
    let a_new = new.audit(&mats, &nuclides, AUDIT_LO, AUDIT_HI, AUDIT_N);
    report("Godiva bounding (union grid)", &new, &a_new, &mats);
    let old = Majorant::bounding_without_breakpoints(&mats, &nuclides, 1.0e-5, 2.0e7, 4000, 24, 0.10);
    let a_old = old.audit(&mats, &nuclides, AUDIT_LO, AUDIT_HI, AUDIT_N);
    report("Godiva ABLATION bounding_without_breakpoints", &old, &a_old, &mats);
    assert!(
        a_new.worst_ratio <= 1.0,
        "Majorant::bounding UNDER-BOUNDS Godiva Sigma_t by {:.3} % at {:.5e} eV",
        (a_new.worst_ratio - 1.0) * 100.0,
        a_new.energy_ev
    );
}

/// The new construction is never below the old one: every energy the old
/// majorant bounded, the new one bounds too. LOW tier, so it is fast and
/// runs without reference data. These are the openmc-notebook TRISO materials
/// (`tests/openmc_notebooks/triso.rs`).
#[test]
fn union_grid_majorant_is_never_below_the_sampled_one() {
    let nuclides: Vec<Nuclide> = ["U234", "U235", "U238", "H1"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("CORE nuclide"))
        .collect();
    let mats = vec![
        mat(
            1,
            "HEU kernel",
            293.6,
            &[(0, 4.9184e-4), (1, 4.4994e-2), (2, 2.4984e-3)],
        ),
        mat(2, "H matrix", 293.6, &[(3, 4.0e-2)]),
    ];
    let new = Majorant::bounding(&mats, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let old = Majorant::bounding_without_breakpoints(&mats, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let n = 400_000;
    for i in 0..=n {
        let e = 1.0e-6 * (2.0e13_f64).powf(i as f64 / n as f64);
        assert!(
            new.at(e) >= old.at(e),
            "new majorant {} below old {} at {e:e} eV",
            new.at(e),
            old.at(e)
        );
    }
    let a = new.audit(&mats, &nuclides, 1.0e-4, 2.0e7, 400_000);
    report("TRISO notebook (LOW tier) bounding", &new, &a, &mats);
    assert!(a.worst_ratio <= 1.0, "LOW-tier under-bound {:.4} at {:e} eV", a.worst_ratio, a.energy_ev);
}
