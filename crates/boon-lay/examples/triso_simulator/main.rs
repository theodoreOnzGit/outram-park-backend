/// the boon lay decay simulator
///
/// this is powered by egui
///
///
///
fn main() {

    println!("Starting TRISO Diffusion and Decay Simulator by Boon Lay...");
    triso_simulator_v1::triso_decay_diffusion_simulator_v1().unwrap();
}

pub mod triso_simulator_v1;
