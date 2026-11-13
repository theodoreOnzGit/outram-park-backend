// Reference driver: runs shared.rs against UNMODIFIED RustBCA v3.0.0 (crate `libRustBCA`).
// Build: a scratch crate whose Cargo.toml is driver_Cargo.toml (here), depending on the
// upstream checkout by path; src/main.rs is this file; shared.rs sits beside it.
#[path = "shared.rs"]
mod shared;

use libRustBCA as rb;
use rand::rngs::ChaCha8Rng;
use rand::SeedableRng;

pub type Rng8 = ChaCha8Rng;

pub fn make_rng(seed: u64, stream: u64) -> Rng8 {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    r.set_stream(stream);
    r
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    for l in shared::all_lines() {
        println!("{l}");
    }
}
