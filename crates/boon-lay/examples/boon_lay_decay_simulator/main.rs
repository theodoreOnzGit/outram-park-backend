/// the boon lay decay simulator
///
/// this is powered by egui
///
///
///
fn main() {

    println!("Starting Boon Lay Decay Simulator...");
    decay_simulator_v1::decay_simulator_v1().unwrap();
}

pub mod decay_simulator_v1;
