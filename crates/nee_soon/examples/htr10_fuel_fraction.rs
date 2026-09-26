//! **What fuel fraction does the assembled bed ACTUALLY have?**
//!
//! Formulas for how much of a cylinder a hex lattice tiles have now been wrong
//! twice in this model (`(n_rings+0.5)*pitch` put the cylinder outside the
//! tiled region entirely; `(n_rings-0.5)*pitch` overstates the covered area).
//! So this measures it instead, through `Geometry::locate` on the real
//! assembled core -- the same code path transport uses.
//!
//! ```bash
//! OUTRAM_HTR10_SAMPLES=400000000 cargo run --release -p nee_soon --example htr10_fuel_fraction
//! ```
//!
//! Samples uniformly in the fuelled envelope (the cylinder `r <= 90 cm` from
//! the conus floor to the bed top) and reports the volume fraction that
//! resolves to each material, each with its binomial standard error, beside
//! the value the paper implies over the SAME envelope:
//!
//! - bed slab (`|z| <= bed half-height`): pebbles at the paper's 0.61, 57 % of
//!   them fuelled; a fuel pebble is a 2.5 cm fuel zone holding the TRISO
//!   (`8340` built, not the paper's 8335, see `cubic_array_in_ball`) in a
//!   3.0 cm ball of graphite.
//! - conus band (bed floor to conus floor): only the frustum inside the cone
//!   is bed, holding DUMMY pebbles at 0.61 (Terry 2005 s2); the rest of the
//!   band is reflector.
//!
//! It also reads, from the tile cell each bed sample lands in
//! (`core_model::tile_cell_role`), whether the point belongs to a fuelled or a
//! dummy pebble, and so measures the **realised fuel-ball volume fraction** in
//! the bed slab -- which the paper sets at 0.57 -- from the built geometry
//! rather than from the assignment that built it.
//!
//! # Sampling
//!
//! A fixed-seed 64-bit LCG per thread (`OUTRAM_HTR10_THREADS`, default 16),
//! each thread its own seed, so a run is reproducible for a given thread
//! count. Every quantity shares one sample set, so ratios between them are
//! correlated; the quoted errors are per-quantity.
//!
//! # Results (2026-09-25, two-ball cell, 14 rings)
//!
//! Recorded in `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`,
//! section "The two-ball cell".
use nee_soon::htr10_rmc::core_model::{
    assemble_explicit_triso, mat, tile_cell_role, TileCellRole, HTR10_CONUS_HEIGHT_CM,
    HTR10_CORE_RADIUS_CM, HTR10_DISCHARGE_TUBE_RADIUS_CM,
};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::pebble_beds::sphere_packing::cubic_pitch_for_count;
use std::f64::consts::PI;

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Tallies from one thread.
#[derive(Clone, Default)]
struct Tally {
    /// 0..7 materials (7 = reflector and beyond), 8 = void/none.
    counts: [u64; 9],
    /// Bed-slab samples, and of those: in a fuelled pebble, in a dummy pebble,
    /// in helium, in no tile cell.
    bed: u64,
    bed_fuel_ball: u64,
    bed_dummy_ball: u64,
    bed_helium: u64,
    bed_unclassified: u64,
    /// Radial bins: where does the tiling actually stop?
    bin_tot: [u64; 18],
    bin_lost: [u64; 18],
}

const NB: usize = 18;

fn main() {
    let rings = env_usize("OUTRAM_HTR10_RINGS", 14);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 25);
    let n = env_usize("OUTRAM_HTR10_SAMPLES", 400_000) as u64;
    let threads = env_usize("OUTRAM_HTR10_THREADS", 16).max(1) as u64;
    let core = assemble_explicit_triso(rings, layers, usize::MAX);
    let geom = &core.geometry;

    let r_max = HTR10_CORE_RADIUS_CM;
    // Sample the whole fuelled envelope, bed PLUS conus. Sampling only the bed
    // cylinder cannot see conus fuel at all, so it would report "no change"
    // however much was added.
    let z_hi = core.bed_half_height;
    let z_lo = core.conus_floor;

    let per = n / threads;
    let tallies: Vec<Tally> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut seed = 12345_u64 ^ (t.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                    let mut prn = || {
                        seed = seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
                    };
                    let mut tl = Tally::default();
                    let u = Direction::new(0.0, 0.0, 1.0);
                    for _ in 0..per {
                        // Uniform in the cylinder: r = R sqrt(xi).
                        let r = r_max * prn().sqrt();
                        let th = 2.0 * PI * prn();
                        let z = z_lo + (z_hi - z_lo) * prn();
                        let p = Position::new(r * th.cos(), r * th.sin(), z);
                        let b = (((r / r_max) * NB as f64) as usize).min(NB - 1);
                        tl.bin_tot[b] += 1;
                        let in_bed = z >= -core.bed_half_height;
                        if in_bed {
                            tl.bed += 1;
                        }
                        match geom.locate(p, u, SurfaceToken::NONE) {
                            Some(path) => {
                                match path.material {
                                    Some(m) => tl.counts[m.min(7)] += 1,
                                    None => tl.counts[8] += 1,
                                }
                                if in_bed {
                                    // The bed-tile level is the one entered
                                    // through lattice 0.
                                    let role = path
                                        .levels
                                        .iter()
                                        .find(|c| c.lattice == Some(0))
                                        .and_then(|c| tile_cell_role(geom.cells[c.cell].id));
                                    match role {
                                        Some(TileCellRole::FuelZone | TileCellRole::FuelShell) => {
                                            tl.bed_fuel_ball += 1;
                                        }
                                        Some(TileCellRole::DummyBall) => tl.bed_dummy_ball += 1,
                                        Some(TileCellRole::Helium) => tl.bed_helium += 1,
                                        None => tl.bed_unclassified += 1,
                                    }
                                }
                            }
                            None => {
                                tl.counts[8] += 1;
                                tl.bin_lost[b] += 1;
                            }
                        }
                    }
                    tl
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("sampler thread")).collect()
    });
    let mut t = Tally::default();
    for x in &tallies {
        for i in 0..9 {
            t.counts[i] += x.counts[i];
        }
        t.bed += x.bed;
        t.bed_fuel_ball += x.bed_fuel_ball;
        t.bed_dummy_ball += x.bed_dummy_ball;
        t.bed_helium += x.bed_helium;
        t.bed_unclassified += x.bed_unclassified;
        for i in 0..NB {
            t.bin_tot[i] += x.bin_tot[i];
            t.bin_lost[i] += x.bin_lost[i];
        }
    }
    let n = per * threads;
    let nf = n as f64;
    // Fraction and binomial standard error.
    let fr = |k: u64, of: u64| {
        let p = k as f64 / of as f64;
        (p, (p * (1.0 - p) / of as f64).sqrt())
    };

    // ---------------------------------------------------------- expectation
    // Paper-implied volume fractions over THIS envelope.
    let bed_h = 2.0 * core.bed_half_height;
    let band_h = -core.conus_floor - core.bed_half_height;
    let v_env = PI * r_max * r_max * (bed_h + band_h);
    let v_bed = PI * r_max * r_max * bed_h;
    let (r1, r2) = (HTR10_CORE_RADIUS_CM, HTR10_DISCHARGE_TUBE_RADIUS_CM);
    let v_cone = PI * HTR10_CONUS_HEIGHT_CM / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2);
    let pack = 0.61_f64;
    let f_fuel = 0.57_f64;
    let zone = (2.5_f64 / 3.0).powi(3);
    // TRISO: radii as `core_model`, count as BUILT (8340).
    let tr = [0.0250_f64, 0.0340, 0.0380, 0.0415, 0.0455];
    let (_, n_part) = cubic_pitch_for_count(tr[4], 2.5, 8335, [0.5, 0.5, 0.0]);
    let v_zone = 4.0 / 3.0 * PI * 2.5_f64.powi(3);
    let shell_frac = |i: usize| {
        let lo = if i == 0 { 0.0 } else { tr[i - 1] };
        n_part as f64 * 4.0 / 3.0 * PI * (tr[i].powi(3) - lo.powi(3)) / v_zone
    };
    let part_frac: f64 = (0..5).map(shell_frac).sum();
    let bed_zone = pack * f_fuel * zone; // fuel-zone fraction of the bed slab
    // Expected fraction of the envelope for each material slot.
    let mut expect = [0.0_f64; 8];
    for (i, e) in expect.iter_mut().enumerate().take(5) {
        *e = bed_zone * shell_frac(i) * v_bed / v_env;
    }
    let bed_graphite = pack * (1.0 - f_fuel) + pack * f_fuel * (1.0 - zone) + bed_zone * (1.0 - part_frac);
    expect[mat::GRAPHITE] = (bed_graphite * v_bed + pack * v_cone) / v_env;
    expect[mat::HELIUM] = ((1.0 - pack) * (v_bed + v_cone)) / v_env;
    expect[7] = (PI * r_max * r_max * band_h - v_cone) / v_env;

    println!(
        "envelope r <= {r_max:.2} cm, z in [{z_lo:.3}, {z_hi:.3}] cm ({:.3} cm tall), {n} samples, {threads} threads",
        z_hi - z_lo
    );
    println!("  (bed {bed_h:.3} cm + conus band {band_h:.3} cm below it; cone {:.4} of that band)", v_cone / (PI * r_max * r_max * band_h));
    println!(
        "  built: {} tiles, {} cells, {} universes, pitch {:.4} cm, tile height {:.4} cm, {n_part} TRISO per fuel pebble",
        core.tiles, core.cells, core.universes, core.lat_pitch, core.lat_height
    );
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
        "{:<12} {:>12} {:>10} {:>9} {:>10} {:>16}",
        "material", "count", "fraction", "+/-", "expected", "ratio"
    );
    for (i, nm) in names.iter().enumerate() {
        let (p, e) = fr(t.counts[i], n);
        if i < 8 && expect[i] > 0.0 {
            println!(
                "{nm:<12} {:>12} {p:>10.6} {e:>9.6} {:>10.6} {:>8.4} +/- {:.4}",
                t.counts[i],
                expect[i],
                p / expect[i],
                e / expect[i]
            );
        } else if t.counts[i] > 0 {
            println!("{nm:<12} {:>12} {p:>10.6} {e:>9.6}", t.counts[i]);
        }
    }
    let _ = nf;

    println!();
    println!("radial profile of UNTILED fraction (needs OUTRAM_HTR10_NO_OUTER=1):");
    for b in 0..NB {
        if t.bin_tot[b] == 0 || t.bin_lost[b] == 0 {
            continue;
        }
        let f = t.bin_lost[b] as f64 / t.bin_tot[b] as f64;
        let lo = r_max * b as f64 / NB as f64;
        let hi = r_max * (b + 1) as f64 / NB as f64;
        println!(
            "  r {lo:6.1}-{hi:6.1} cm : {f:6.3}  {}",
            "#".repeat((f * 40.0) as usize)
        );
    }

    // --------------------------------------------------------- the bed slab
    let balls = t.bed_fuel_ball + t.bed_dummy_ball;
    let (pk, pke) = fr(balls, t.bed);
    let (fb, fbe) = fr(t.bed_fuel_ball, balls);
    let (he, hee) = fr(t.bed_helium, t.bed);
    println!();
    println!("bed slab only (|z| <= {:.3} cm), {} samples:", core.bed_half_height, t.bed);
    println!("  pebble filling fraction   : {pk:.6} +/- {pke:.6}   (paper 0.61)");
    println!("  helium fraction           : {he:.6} +/- {hee:.6}   (paper 0.39)");
    println!("  fuel-BALL volume fraction : {fb:.6} +/- {fbe:.6}   (paper 0.57)");
    println!("  samples in no tile cell   : {}", t.bed_unclassified);

    // ------------------------------------------------------------ headline
    let (k, ke) = fr(t.counts[mat::KERNEL], n);
    println!();
    println!("paper-implied KERNEL fraction of the BED     : {:.6e}", bed_zone * shell_frac(0) * 8335.0 / n_part as f64);
    println!("  x {n_part}/8335 (built particles)            : {:.6e}", bed_zone * shell_frac(0));
    println!("  x bed/(bed+band) (dummy-only conus)        : {:.6e}", expect[mat::KERNEL]);
    println!("measured kernel fraction of the envelope     : {k:.6e} +/- {ke:.2e}");
    println!(
        "RATIO measured / expected                    : {:.4} +/- {:.4}",
        k / expect[mat::KERNEL],
        ke / expect[mat::KERNEL]
    );
}
