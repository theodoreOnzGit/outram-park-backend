// GUI (egui/eframe) example — out of scope for Android, which has no windowing
// stack and no C toolchain for the transitive `ring` dep. On Android the real
// entry point and its egui-using module are gated out and replaced by an empty
// `main`, so the example target still builds (to a no-op) there.
#[cfg(not(target_os = "android"))]
fn main() {
    println!("Starting Boon Lay Decay Simulator...");
    decay_simulator_v1::decay_simulator_v1().unwrap();
}

#[cfg(not(target_os = "android"))]
pub mod decay_simulator_v1;

#[cfg(target_os = "android")]
fn main() {}
