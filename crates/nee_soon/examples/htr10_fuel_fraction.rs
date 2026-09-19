//! **What fuel fraction does the assembled bed ACTUALLY have?**
//!
//! Formulas for how much of a cylinder a hex lattice tiles have now been wrong
//! twice in this model (`(n_rings+0.5)*pitch` put the cylinder outside the
//! tiled region entirely; `(n_rings-0.5)*pitch` overstates the covered area).
//! So this measures it instead, through `Geometry::locate` on the real
//! assembled core -- the same code path transport uses.
//!
//! Samples uniformly in the bed cylinder and reports the volume fraction that
//! resolves to each material. The kernel fraction is the number that sets
//! reactivity; the paper implies 0.61 * (2.5/3.0)^3 * 0.57 of the bed is
//! fuel-zone, and the kernel is `TrisoSpec`'s packing of that again.
use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, HTR10_CORE_RADIUS_CM};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::cell::SurfaceToken;

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

fn main() {
    let rings = env_usize("OUTRAM_HTR10_RINGS", 14);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 25);
    let core = assemble_explicit_triso(rings, layers, usize::MAX);
    let geom = &core.geometry;

    // Bed extent. Sample inside the cylinder only.
    let r_max = HTR10_CORE_RADIUS_CM;
    // Sample the whole fuelled envelope, bed PLUS conus. Sampling only the bed
    // cylinder cannot see conus fuel at all, so it would report "no change"
    // however much was added.
    let z_hi = core.bed_half_height;
    let z_lo = core.conus_floor;
    let n = env_usize("OUTRAM_HTR10_SAMPLES", 400_000);

    let mut seed = 12345_u64;
    let mut prn = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };

    let mut counts = [0usize; 9]; // 0..7 materials, 8 = void/none
                                  // Radial bins: where does the tiling actually stop?
    const NB: usize = 18;
    let (mut bin_tot, mut bin_lost) = ([0usize; NB], [0usize; NB]);
    let u = Direction::new(0.0, 0.0, 1.0);
    for _ in 0..n {
        // Uniform in the cylinder: r = R sqrt(xi).
        let r = r_max * prn().sqrt();
        let th = 2.0 * std::f64::consts::PI * prn();
        let z = z_lo + (z_hi - z_lo) * prn();
        let p = Position::new(r * th.cos(), r * th.sin(), z);
        let b = ((r / r_max) * NB as f64) as usize;
        let b = b.min(NB - 1);
        bin_tot[b] += 1;
        match geom
            .locate(p, u, SurfaceToken::NONE)
            .and_then(|g| g.material)
        {
            Some(m) => counts[m.min(7)] += 1,
            None => {
                counts[8] += 1;
                bin_lost[b] += 1;
            }
        }
    }

    let names = [
        "kernel",
        "buffer",
        "IPyC",
        "SiC",
        "OPyC",
        "graphite",
        "helium",
        "reflector",
        "LOST/void",
    ];
    println!(
        "envelope r <= {:.2} cm, z in [{:.2}, {:.2}] cm ({:.2} cm tall), {} samples",
        r_max,
        z_lo,
        z_hi,
        z_hi - z_lo,
        n
    );
    println!(
        "  (bed {:.2} cm + conus {:.2} cm below it)",
        2.0 * core.bed_half_height,
        -core.conus_floor - core.bed_half_height
    );
    println!("{:<12} {:>9} {:>10}", "material", "count", "fraction");
    for (i, nm) in names.iter().enumerate() {
        if counts[i] > 0 {
            println!(
                "{nm:<12} {:>9} {:>10.6}",
                counts[i],
                counts[i] as f64 / n as f64
            );
        }
    }
    println!();
    println!("radial profile of UNTILED fraction (needs OUTRAM_HTR10_NO_OUTER=1):");
    for b in 0..NB {
        if bin_tot[b] == 0 {
            continue;
        }
        let f = bin_lost[b] as f64 / bin_tot[b] as f64;
        let lo = r_max * b as f64 / NB as f64;
        let hi = r_max * (b + 1) as f64 / NB as f64;
        println!(
            "  r {lo:6.1}-{hi:6.1} cm : {f:6.3}  {}",
            "#".repeat((f * 40.0) as usize)
        );
    }
    let fuel_zone: usize = counts[0] + counts[1] + counts[2] + counts[3] + counts[4];
    println!();
    println!(
        "fuel-zone (all TRISO layers + matrix is separate): {:.6}",
        fuel_zone as f64 / n as f64
    );
    println!(
        "kernel volume fraction of bed                    : {:.6}",
        counts[0] as f64 / n as f64
    );
    // Paper-implied KERNEL fraction.
    //
    // CORRECTED: this previously stopped at the PARTICLE fraction (0.010111)
    // and printed it as the kernel fraction, making the ratio read 0.1587 when
    // the model was actually within a few percent. The kernel is only
    // (0.025/0.0455)^3 = 0.16585 of a particle's volume.
    let particle_frac = 0.61 * (2.5_f64 / 3.0).powi(3) * 0.57 * 0.050248;
    let implied = particle_frac * (0.025_f64 / 0.0455).powi(3);
    println!("paper-implied PARTICLE fraction                  : {particle_frac:.6}");
    println!("paper-implied KERNEL fraction                    : {implied:.6}");
    println!(
        "ratio (ours / paper)                             : {:.4}",
        (counts[0] as f64 / n as f64) / implied
    );
}
