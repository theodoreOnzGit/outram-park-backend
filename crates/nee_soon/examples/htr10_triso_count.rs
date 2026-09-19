//! How many TRISO particles does the assembled pebble actually contain?
use outram_mc_libs::pebble_beds::sphere_packing::cubic_pitch_for_count;

fn main() {
    let r_zone = 2.5_f64;
    let r_part = 0.0455_f64;
    let (pitch, want) = cubic_pitch_for_count(r_part, r_zone, 8335, [0.5, 0.5, 0.0]);
    let n = ((2.0 * r_zone / pitch).floor() as usize).max(1);
    let half = 0.5 * n as f64 * pitch;
    let r_keep = r_zone - r_part;
    let mut kept = 0usize;
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let c = |m: usize| -half + (m as f64 + 0.5) * pitch;
                let (x, y, z) = (c(i), c(j), c(k));
                if x * x + y * y + z * z <= r_keep * r_keep {
                    kept += 1;
                }
            }
        }
    }
    let v_part = (4.0 / 3.0) * std::f64::consts::PI * r_part.powi(3);
    let v_zone = (4.0 / 3.0) * std::f64::consts::PI * r_zone.powi(3);
    println!("cubic_array_in_ball target      : 8335, achievable {want}");
    println!("lattice n per axis              : {n}  (half-width {half:.4} cm vs zone {r_zone})");
    println!("tiles assigned a PARTICLE       : {kept}");
    println!(
        "packing realised                : {:.6}",
        kept as f64 * v_part / v_zone
    );
    println!("packing intended (TrisoSpec)    : 0.050248");
    println!(
        "ratio                           : {:.3}x",
        (kept as f64 * v_part / v_zone) / 0.050248
    );
}
