use uom::si::f64::*;
use uom::si::thermodynamic_temperature::degree_celsius;

#[path = "../fhr_sim_v2/app/thermal_hydraulics_backend/components.rs"]
mod components;

fn main() {
    println!("building initial temperature...");
    let initial_temperature = ThermodynamicTemperature::new::<degree_celsius>(500.0);
    println!("calling new_reactor_vessel_pipe_1...");
    let _pipe = components::new_reactor_vessel_pipe_1(initial_temperature);
    println!("new_reactor_vessel_pipe_1 ok");
}
