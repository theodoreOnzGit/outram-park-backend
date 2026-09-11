//! Quick probe: B-10 and H-1 thermal cross sections from this crate's own
//! reconstruction, against the 2200 m/s standards.
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;

const T: f64 = 293.6;

fn main() {
    let load = |name: &str, file: &str| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing {file}"));
        Nuclide::from_endf_file(&p, name, T, 1.0e-3).expect("reconstruct")
    };
    let b10 = load("B10", "n-005_B_010-ENDF8.0.endf");
    let h1 = load("H1", "n-001_H_001-ENDF8.0-Beta6.endf");
    let o16 = load("O16", "n-008_O_016-ENDF8.0.endf");

    println!(
        "{:>10}  {:>12}  {:>12}  {:>12}  {:>12}",
        "E [eV]", "B10 total", "B10 abs", "B10 elastic", "abs*sqrt(E)"
    );
    for &e in &[1.0e-4_f64, 1.0e-3, 0.0253, 0.1, 1.0, 10.0, 100.0] {
        let x = b10.xs_at_energy(e, T);
        println!(
            "{e:>10.2e}  {:>12.4}  {:>12.4}  {:>12.4}  {:>12.4}",
            x.total,
            x.absorption - x.fission,
            x.elastic,
            (x.absorption - x.fission) * (e / 0.0253).sqrt()
        );
    }
    println!(
        "\n  B-10 absorption at 0.0253 eV = {:.2} b (2200 m/s standard: 3835 b)",
        {
            let x = b10.xs_at_energy(0.0253, T);
            x.absorption - x.fission
        }
    );

    println!(
        "\n{:>10}  {:>12}  {:>12}  {:>12}",
        "E [eV]", "H1 free tot", "H1 abs", "O16 abs"
    );
    for &e in &[0.0253_f64, 0.1, 1.0] {
        let x = h1.xs_at_energy(e, T);
        let o = o16.xs_at_energy(e, T);
        println!(
            "{e:>10.4}  {:>12.4}  {:>12.4}  {:>12.6}",
            x.total,
            x.absorption - x.fission,
            o.absorption - o.fission
        );
    }

    // With the bound law attached, as the transport uses it.
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-HinH2O.endf")
            .expect("tape")
            .to_str()
            .unwrap(),
        1,
        T,
        "c_H_in_H2O",
    )
    .expect("sab");
    let h1b = load("H1", "n-001_H_001-ENDF8.0-Beta6.endf").with_thermal_scattering(sab);
    println!(
        "\n{:>10}  {:>14}  {:>14}",
        "E [eV]", "H1 bound tot", "H1 bound scat"
    );
    for &e in &[0.001_f64, 0.0253, 0.1, 1.0, 4.0, 10.0, 20.0] {
        let x = h1b.xs_at_energy(e, T);
        println!("{e:>10.4}  {:>14.4}  {:>14.4}", x.total, x.elastic);
    }
}
