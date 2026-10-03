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
//!   (~~`8340` built, not the paper's 8335~~ 8335 built since 2026-10-01,
//!   gh:#430) in a 3.0 cm ball of graphite. ~~At the paper's 0.61~~ Since
//!   2026-10-01 the expectation comes from Şeker's bed's own ball list
//!   (gh:#472).
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
//! # Results (2026-10-01, Şeker's cell, 14 rings x N = 12, 4 M samples)
//!
//! `OUTRAM_HTR10_SAMPLES=4000000`, 16 threads, at the commit that set the TRISO
//! offset to [0.13, 0.37, 0.71] (8335 per pebble). Measured against the ball
//! list:
//!
//! | material | measured | expected | ratio |
//! |---|---|---|---|
//! | kernel | 0.001287 ± 0.000018 | 0.001270 | 1.013 ± 0.014 |
//! | SiC | 0.001347 ± 0.000018 | 0.001349 | 0.999 ± 0.014 |
//! | graphite | 0.511037 ± 0.000250 | 0.510947 | 1.0002 ± 0.0005 |
//! | helium | 0.354924 ± 0.000239 | 0.355187 | 0.9993 ± 0.0007 |
//! | reflector (zone 0 band) | 0.126383 ± 0.000166 | 0.126210 | 1.0014 ± 0.0013 |
//!
//! Bed slab: filling 0.601892 ± 0.000279 (ball list 0.601642), fuel-ball
//! volume fraction 0.568806 ± 0.000364 (the conus caps in the slab are
//! dummies). 0 samples in no tile cell. Every ratio is within 1.1 sigma: the
//! built geometry holds what the bed and the TRISO count say.
//!
//! *(2026-09-25, two-ball cell, withdrawn:* recorded in
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`,
//! section "The two-ball cell".)
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
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 12); // Şeker layers N (gh:#472); 12 = 123.576 cm
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
    // ~~`pack = 0.61`, `f_fuel = 0.57` applied to the bed slab and the cone~~
    // **CHANGED 2026-10-01 (gh:#472):** Şeker's bed is not built to a filling
    // fraction, so the expectation is taken from its BALL LIST. Every kept
    // ball, clipped to the envelope's z range (kept balls never cross the
    // container laterally); fuel balls are whole inside it.
    let bed = core.bed.as_ref().expect("the explicit core carries its bed");
    let (r_ball, r_zone) = (3.0_f64, 2.5_f64);
    let cap = |h: f64| PI * h * h * (3.0 * r_ball - h) / 3.0;
    let (mut v_balls, mut v_balls_bed, mut n_fuel) = (0.0_f64, 0.0_f64, 0usize);
    for id in bed.all_balls() {
        if !bed.is_present(id) {
            continue;
        }
        let z = bed.centre(id)[2];
        let above = |plane: f64| cap((z + r_ball - plane).clamp(0.0, 2.0 * r_ball));
        v_balls += above(z_lo) - above(z_hi);
        v_balls_bed += above(-core.bed_half_height) - above(z_hi);
        n_fuel += usize::from(bed.is_fuel(id));
    }
    let v_ball = 4.0 / 3.0 * PI * r_ball.powi(3);
    let v_fuel_zone = n_fuel as f64 * 4.0 / 3.0 * PI * r_zone.powi(3);
    // TRISO: radii and lattice as `core_model` (offset [0.13, 0.37, 0.71] since
    // 2026-10-01, gh:#430: 8335 built). ~~count as BUILT (8340)~~
    let tr = [0.0250_f64, 0.0340, 0.0380, 0.0415, 0.0455];
    let (_, n_part) = cubic_pitch_for_count(tr[4], 2.5, 8335, [0.13, 0.37, 0.71]);
    let v_zone = 4.0 / 3.0 * PI * 2.5_f64.powi(3);
    let shell_frac = |i: usize| {
        let lo = if i == 0 { 0.0 } else { tr[i - 1] };
        n_part as f64 * 4.0 / 3.0 * PI * (tr[i].powi(3) - lo.powi(3)) / v_zone
    };
    let part_frac: f64 = (0..5).map(shell_frac).sum();
    // Expected fraction of the envelope for each material slot.
    let mut expect = [0.0_f64; 8];
    for (i, e) in expect.iter_mut().enumerate().take(5) {
        *e = v_fuel_zone * shell_frac(i) / v_env;
    }
    let graphite = v_balls - n_fuel as f64 * v_ball // dummy balls
        + n_fuel as f64 * v_ball - v_fuel_zone // fuel-ball shells
        + v_fuel_zone * (1.0 - part_frac); // matrix inside the fuel zones
    expect[mat::GRAPHITE] = graphite / v_env;
    expect[mat::HELIUM] = (v_bed + v_cone - v_balls) / v_env;
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
    println!(
        "  pebble filling fraction   : {pk:.6} +/- {pke:.6}   (ball list {:.6}; ~~paper 0.61~~)",
        v_balls_bed / v_bed
    );
    println!("  helium fraction           : {he:.6} +/- {hee:.6}");
    println!("  fuel-BALL volume fraction : {fb:.6} +/- {fbe:.6}   (paper 0.57)");
    println!("  samples in no tile cell   : {}", t.bed_unclassified);

    // ------------------------------------------------------------ headline
    let (k, ke) = fr(t.counts[mat::KERNEL], n);
    println!();
    // ~~paper-implied 0.61 x 0.57 bed slab~~ (2026-10-01, gh:#472): the
    // expectation is the ball list's fuel-zone volume over the envelope.
    println!(
        "expected KERNEL fraction (ball list: {n_fuel} fuel balls, {n_part} TRISO each): {:.6e}",
        expect[mat::KERNEL]
    );
    println!("measured kernel fraction of the envelope     : {k:.6e} +/- {ke:.2e}");
    println!(
        "RATIO measured / expected                    : {:.4} +/- {:.4}",
        k / expect[mat::KERNEL],
        ke / expect[mat::KERNEL]
    );
}
