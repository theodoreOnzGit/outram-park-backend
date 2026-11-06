//! Draws the assembled containment network of Units 1, 2 and 3 (gh:#1110)
//! into `verification_and_validation/bwr_mark1/geometry/`, one SVG each.
//!
//! ```text
//! cargo run --release -p sembawang --example containment_network_geometry
//! ```
//!
//! CLI glue only (a test of the drawing itself is
//! `containment_network::drawing::tests::the_drawing_shows_what_the_network_holds`).

use sembawang::bwr_mark1::containment_network::{ContainmentNetwork, NetworkConfig, NetworkMechanisms};
use sembawang::bwr_mark1::params::Params;

fn main() -> Result<(), String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/bwr_mark1/geometry");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cases = [
        ("unit1.svg", "Unit 1 containment network (default)", Params::default(), NetworkConfig::unit1()),
        ("unit1_gh1097.svg", "Unit 1 containment network (gh:#1097 configuration)", Params::default(), NetworkConfig::unit1_gh1097()),
        ("unit2.svg", "Unit 2 containment network", Params::unit2(), NetworkConfig::unit2()),
        ("unit3.svg", "Unit 3 containment network", Params::unit3(), NetworkConfig::unit3()),
    ];
    for (file, title, prm, cfg) in cases {
        let n = ContainmentNetwork::with_config(&prm, cfg, NetworkMechanisms::default()).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(file), n.geometry_svg(title)).map_err(|e| e.to_string())?;
        for v in n.geometry() {
            println!("{file}: {} {:.3}..{:.3} m, {:.1} m3, widest {:.2} m2, pool {:.3} m, {} layers, ports {:?}", v.name, v.bottom_m, v.top_m, v.total_m3, v.max_area_m2, v.pool_level_m, v.layer_tops_m.len(), v.ports);
        }
    }
    Ok(())
}
