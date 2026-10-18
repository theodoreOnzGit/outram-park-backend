//! **Tallies score the URR band transport followed (GitHub #953).**
//!
//! # Methodology
//!
//! Before this change every tally (track-length under surface tracking,
//! tentative-collision and real-collision under delta tracking) multiplied by
//! `Material::macro_xs`, the infinitely dilute cross sections, while the
//! flight and the collision used the sampled probability-table band
//! (`macro_xs_total_urr`, `xs_at_energy_urr`, band variate `urr_xi`).
//! OpenMC scores the cached sampled micro cross sections
//! (`src/nuclide.cpp:826-865`; `get_nuclide_xs` in
//! `src/tallies/tally_scoring.cpp`).
//!
//! Fixture: ENDF/B-VIII.0 U-238 at 293.6 K with cheap PURR settings (table
//! values are not asserted, only consistency), a pure U-238 material, probed at
//! 50 keV inside the 20-149 keV unresolved range over 400 URR stream seeds.
//!
//! Assertions: (1) for every seed `Material::macro_xs_urr` has total equal to
//! the flight's `macro_xs_total_urr` and absorption/fission equal to the
//! collision's `xs_at_energy_urr` band values; (2) the dilute `macro_xs` total
//! differs from the flight total for at least one seed, i.e. the pre-fix defect
//! was observable; (3) outside the range, and with tables ablated,
//! `macro_xs_urr` equals `macro_xs` bit for bit (k and every non-URR tally are
//! unchanged).
//!
//! # Results
//!
//! Printed by the test; recorded in
//! `verification_and_validation/urr_tally_scoring_953/README.md`.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::material::material::{urr_xi, Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;

const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
const N_DENS: f64 = 0.0222;

fn fixture() -> Option<(Nuclide, Nuclide)> {
    let p = reference_endf_or_skip("n-092_U_238.endf", "U-238 (URR tally scoring #953)")?;
    let tape = Tape::read_file(&p).expect("tape parses");
    let t = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
        TEMP_K,
    );
    let dilute = Nuclide::from_endf_file(&p, "U238", t, 1.0e-3)
        .expect("reconstructs")
        .without_urr_probability_tables();
    let with = dilute
        .clone()
        .with_urr_probability_tables(&tape, MAT, TEMP_K, 20, 4, 500)
        .expect("tables");
    Some((dilute, with))
}

fn mat() -> Material {
    Material {
        id: 1,
        name: "U238".into(),
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: N_DENS,
        }],
        temperature: TEMP_K,
    }
}

#[test]
fn tally_macro_xs_follows_the_sampled_urr_band() {
    let Some((dilute, with)) = fixture() else {
        return;
    };
    assert!(with.has_urr_probability_tables());
    let e = 5.0e4;
    let m = mat();
    let nucs = vec![with.clone()];
    let dil = m.macro_xs(e, &nucs);
    let (mut n_diff, mut sum_abs) = (0usize, 0.0);
    let n = 400u64;
    for k in 0..n {
        let seed = 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(k + 1) | 1;
        let t = m.macro_xs_urr(e, &nucs, seed);
        let flight = m.macro_xs_total_urr(e, &nucs, seed);
        assert_eq!(
            t.total, flight,
            "tally total must be the flight total (seed {k})"
        );
        let band = with.xs_at_energy_urr(e, TEMP_K, urr_xi(0, seed));
        assert_eq!(t.absorption, N_DENS * band.absorption);
        assert_eq!(t.fission, N_DENS * band.fission);
        if (dil.total - flight).abs() > 1e-12 * dil.total {
            n_diff += 1;
        }
        sum_abs += t.absorption;
    }
    assert!(
        n_diff > 0,
        "dilute total never differed from the flight total: no defect to fix"
    );
    println!(
        "URR 953 @50 keV, {n} seeds: dilute Sa={:.6e}; band-mean Sa={:.6e} (ratio {:.5}); seeds with dilute!=flight total: {n_diff}",
        dil.absorption,
        sum_abs / n as f64,
        sum_abs / n as f64 / dil.absorption
    );

    // Bit-identical outside the range and when ablated.
    let ab = vec![dilute];
    for (nu, en) in [(&nucs, 1.0e3), (&nucs, 2.0e6), (&ab, e)] {
        let a = m.macro_xs(en, nu);
        let b = m.macro_xs_urr(en, nu, 12345);
        assert_eq!(a.total.to_bits(), b.total.to_bits());
        assert_eq!(a.absorption.to_bits(), b.absorption.to_bits());
        assert_eq!(a.elastic.to_bits(), b.elastic.to_bits());
    }
}
