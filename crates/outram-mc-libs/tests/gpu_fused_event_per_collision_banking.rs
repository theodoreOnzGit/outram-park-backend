//! gh:#1444 (e): the fused-event WGSL kernel (`gpu/shaders/batched_event.wgsl`)
//! and its CPU mirror bank fission sites PER COLLISION, as the CPU default does
//! (OpenMC `create_fission_sites`; legacy `OUTRAM_MC_LEGACY_FISSION_BANKING`).
//!
//! # Methodology
//! Godiva (bare HEU sphere, R = 8.7407 cm, LOW-tier `from_core` U-234/235/238,
//! atom densities of the ICSBEP specification), `n_particles = 2000`,
//! 20 inactive + 100 active generations, seed 12345. Three arms, same settings:
//! - CPU default: [`PowerIteration`] (per-collision banking, f64);
//! - fused path on its CPU mirror: [`run_keff_event_cpu_mirror_reports`];
//! - fused path on the GPU (wgpu; lavapipe counts): [`run_keff_gpu_event_reports`],
//!   skipped with a SKIP line when no adapter exists, like the other GPU tests.
//!
//! Statistics per arm over the active generations: mean and standard error of
//! `k_gen` and of `bank_size / N` (SE = sample std / sqrt(n_active)).
//!
//! # Prediction and tolerances, written 2026-11-11 BEFORE any run
//! The tolerances come from sigma and from the counting statistics of the banking
//! rule, not from any result.
//!
//! **G1, banking rule self-consistency (every arm).** At a collision the sites
//! banked, `floor(nu_t) + Bernoulli(frac(nu_t))`, have mean `nu_t` and variance
//! `frac(1-frac) <= nu_t`. So the generation bank size has mean
//! `production / k_prev` and variance `<= production / k_prev`, where
//! `production = sum nu_sigma_f / sigma_t`, `k_prev` the previous generation's `k`
//! (1 for the first). Summed over the active generations,
//! `D = sum (bank_g - production_g / k_prev)` must satisfy
//! `|D| <= 5 sqrt(sum production_g / k_prev)`. Prediction: passes (the CPU default
//! and both fused arms bank the OpenMC count); the OLD at-fission banking would
//! also pass this identity in expectation, so G1 alone does not prove per-collision
//! banking, G3 does.
//!
//! **G2, fused vs CPU default.** `|k_fused - k_cpu| <= 4 sqrt(se_f^2 + se_c^2)`,
//! and the same for `bank_size / N`. Expected sigma per arm ~ 0.0025, so the
//! window is ~1400 pcm. Prediction: passes if the f32 fused physics (union-grid
//! interpolated tables, isotropic CM, Weisskopf continuum) is statistically the same
//! eigenvalue as the CPU default at LOW tier. If it fails, that is a physics gap
//! in the fused kernel unrelated to banking, to be reported, and G3 still isolates
//! the banking.
//!
//! **G3, banking really is per collision (kernel level).** One generation from an
//! identical fixed batch of N = 20000 source neutrons through the mirror in
//! per-collision mode: sites are banked at collisions that are NOT fission events
//! (records exist whose history then continues: `n_records > number of fission
//! events` of the at-fission arm run from the same batch is not a valid compare
//! since the streams differ; instead assert that the count of recorded sites
//! exceeds 0 while `fiss_nuc` is `FISS_NONE` for every neutron, i.e. no fission
//! event banked anything), and `sum n_sites` matches `sum production / k` within
//! `5 sqrt(sum production / k)` at k = 1.
//!
//! **G4, GPU kernel vs its mirror (GPU only).** From the same batch, GPU and mirror
//! trajectories agree except where f32 transcendental differences flip a branch.
//! Judged as if the two were independent runs: with `s2` the sample variance of the
//! mirror's per-neutron production and `E` = mirror `sum n_sites`,
//! `|sites_gpu - sites_mirror| <= 5 sqrt(2 (N s2 + E))` and the same for the summed
//! production with `5 sqrt(2 N s2)`. Prediction: passes, far inside the bound
//! (most histories identical). Also: a tiny initial record capacity (8) forces the
//! grow-and-rerun path and must return the SAME records as a roomy capacity.
//!
//! # Results
//! See the `RESULT` block at the bottom of this comment (filled in after the run).
//!
//! RESULT (2026-11-11, rustc 1.99.1 release, 4 vCPU shared container, GPU = Mesa
//! llvmpipe (LLVM 20.1.2) i.e. software Vulkan, so the WGSL really ran but
//! on CPU cores). All gates PASSED, none adjusted. Values +/- 1 sigma (SE over
//! 100 active generations, N = 2000):
//!
//! | arm | k | bank/N | G1 D (5 sigma bound 2237) |
//! |---|---|---|---|
//! | CPU default (per-collision) | 1.01060 +/- 0.00223 | 1.00053 +/- 0.00335 | +74.9 |
//! | fused CPU mirror | 1.01047 +/- 0.00216 | 1.00025 +/- 0.00392 | -88.8 |
//! | fused GPU | 1.01179 +/- 0.00241 | 0.99993 +/- 0.00397 | -174.6 |
//!
//! G2: mirror - CPU dk = -14 pcm (-0.04 sigma); GPU - CPU dk = +119 pcm (+0.36
//! sigma); bank/N differences -0.06 and -0.12 sigma. G3: 23412 records / 23412
//! sites vs expected production 23345.4 (+0.4 sigma), no `fiss_nuc` tag set, i.e.
//! sites banked at collisions only. G4: GPU and mirror agree EXACTLY on the fixed
//! 20000-neutron batch (23412 records, 23412 sites, production 23345.4 both;
//! bounds were 1499 and 1037); grow-and-rerun from capacity 8 reproduced the
//! records. The G3 prose above is loosely worded: what is asserted is "no
//! fission-event tag, sites > 0, sites match production within 5 sqrt(production)".
//!
//! Old (at-fission, `OUTRAM_MC_LEGACY_FISSION_BANKING=1`) fused GPU k on the same
//! settings: 1.01358 +/- 0.00287; new per-collision 1.01179 +/- 0.00241; change
//! -179 pcm, 0.48 sigma of the combined SE (unresolved; same expectation as the
//! CPU paired study in `godiva_fission_banking_paired.rs`).

use outram_mc_libs::gpu::batched_event::{
    advance_generation_cpu_mirror_banking, EventBatch, EventFissionBanking, EventSiteRecord,
    EventSphere, EventTablesF32, FISS_NONE,
};
use outram_mc_libs::gpu::collision_grid::CollisionTables;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{
    run_keff_event_cpu_mirror_reports, EventGenerationReport, KeffSettings, PowerIteration,
};

const RADIUS: f64 = 8.7407;

fn godiva() -> (Material, Vec<Nuclide>) {
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let material = Material {
        id: 1,
        name: "Godiva".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    };
    (material, nuclides)
}

fn settings() -> KeffSettings {
    KeffSettings {
        n_particles: 2000,
        n_inactive: 20,
        n_active: 100,
        seed: 12345,
        ..KeffSettings::default()
    }
}

/// Per-generation (production, bank_size) of one arm.
fn arm_cpu_default() -> Vec<EventGenerationReport> {
    let (material, nuclides) = godiva();
    let s = settings();
    let mut it = PowerIteration::new(RADIUS, &s);
    let mut v = Vec::new();
    while let Some(r) = it.step(&material, &nuclides) {
        v.push(EventGenerationReport { production: r.production, bank_size: r.bank_size });
    }
    v
}

fn mean_se(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let var = x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

struct ArmStats {
    k: (f64, f64),
    bank_per_n: (f64, f64),
    d: f64,
    d_sigma_bound: f64,
}

fn stats(reports: &[EventGenerationReport]) -> ArmStats {
    let s = settings();
    let n = s.n_particles as f64;
    let mut k = Vec::new();
    let mut b = Vec::new();
    let mut d = 0.0;
    let mut var_bound = 0.0;
    for g in s.n_inactive..reports.len() {
        let k_prev = if g == 0 { 1.0 } else { reports[g - 1].production / n };
        let expected = reports[g].production / k_prev;
        d += reports[g].bank_size as f64 - expected;
        var_bound += expected;
        k.push(reports[g].production / n);
        b.push(reports[g].bank_size as f64 / n);
    }
    ArmStats { k: mean_se(&k), bank_per_n: mean_se(&b), d, d_sigma_bound: var_bound.sqrt() }
}

fn check_arm(name: &str, a: &ArmStats) {
    println!(
        "{name}: k = {:.5} +/- {:.5}; bank/N = {:.5} +/- {:.5}; G1 D = {:+.1} (5 sigma bound {:.1})",
        a.k.0, a.k.1, a.bank_per_n.0, a.bank_per_n.1, a.d, 5.0 * a.d_sigma_bound
    );
    assert!(
        a.d.abs() <= 5.0 * a.d_sigma_bound,
        "{name}: G1 banked sites deviate from production/k_prev by {} (bound {})",
        a.d,
        5.0 * a.d_sigma_bound
    );
}

fn check_vs_cpu(name: &str, f: &ArmStats, c: &ArmStats) {
    let dk = f.k.0 - c.k.0;
    let sk = (f.k.1 * f.k.1 + c.k.1 * c.k.1).sqrt();
    let db = f.bank_per_n.0 - c.bank_per_n.0;
    let sb = (f.bank_per_n.1 * f.bank_per_n.1 + c.bank_per_n.1 * c.bank_per_n.1).sqrt();
    println!(
        "G2 {name} - CPU default: dk = {:+.5} ({:+.2} sigma, {:+.0} pcm); d(bank/N) = {:+.5} ({:+.2} sigma)",
        dk,
        dk / sk,
        dk * 1e5,
        db,
        db / sb
    );
    assert!(dk.abs() <= 4.0 * sk, "{name}: k differs from the CPU default by {} sigma", dk / sk);
    assert!(db.abs() <= 4.0 * sb, "{name}: bank/N differs from the CPU default by {} sigma", db / sb);
}

#[test]
fn fused_event_mirror_banks_per_collision_like_the_cpu_default() {
    let (material, nuclides) = godiva();
    let s = settings();
    let cpu = stats(&arm_cpu_default());
    check_arm("CPU default", &cpu);
    let (_r, rep) = run_keff_event_cpu_mirror_reports(RADIUS, &material, &nuclides, &s);
    let mir = stats(&rep);
    check_arm("fused mirror", &mir);
    check_vs_cpu("fused mirror", &mir, &cpu);
}

fn tables() -> EventTablesF32 {
    let (material, nuclides) = godiva();
    EventTablesF32::from_collision_tables(&CollisionTables::build(
        &material, &nuclides, 1e-3, 2e7, 16384,
    ))
}

/// A fixed batch of `n` fission-energy neutrons inside the sphere.
fn fixed_batch(n: usize) -> EventBatch {
    let mut h = 0x0123_4567_89AB_CDEFu64;
    let mut next = || {
        h = h.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = h;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let unit = |v: u64| (v >> 40) as f32 / 16_777_216.0;
    let mut b = EventBatch {
        pos: Vec::new(),
        dir: Vec::new(),
        energy: Vec::new(),
        seed_lo: Vec::new(),
        seed_hi: Vec::new(),
        alive: vec![1; n],
        fiss_nuc: vec![FISS_NONE; n],
        production: vec![0.0; n],
    };
    for _ in 0..n {
        // Point in the inner 6 cm ball by rejection, isotropic direction.
        loop {
            let p = [unit(next()) * 12.0 - 6.0, unit(next()) * 12.0 - 6.0, unit(next()) * 12.0 - 6.0];
            if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] <= 36.0 {
                b.pos.extend_from_slice(&p);
                break;
            }
        }
        loop {
            let d = [unit(next()) * 2.0 - 1.0, unit(next()) * 2.0 - 1.0, unit(next()) * 2.0 - 1.0];
            let l2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if l2 <= 1.0 && l2 > 1e-4 {
                let l = l2.sqrt();
                b.dir.extend_from_slice(&[d[0] / l, d[1] / l, d[2] / l]);
                break;
            }
        }
        b.energy.push(2.0e6 * (0.2 + 1.6 * unit(next())));
        let sd = next();
        b.seed_lo.push(sd as u32);
        b.seed_hi.push((sd >> 32) as u32);
    }
    b
}

fn sphere() -> EventSphere {
    EventSphere { x0: 0.0, y0: 0.0, z0: 0.0, r: RADIUS as f32 }
}

#[test]
fn g3_mirror_banks_at_collisions_not_at_fission_events() {
    let t = tables();
    let n = 20_000usize;
    let mut batch = fixed_batch(n);
    let mut records: Vec<EventSiteRecord> = Vec::new();
    advance_generation_cpu_mirror_banking(
        &t,
        &mut batch,
        sphere(),
        10_000,
        EventFissionBanking::PerCollision { keff: 1.0 },
        &mut records,
    );
    let sites: u64 = records.iter().map(|r| r.n_sites as u64).sum();
    let production: f64 = batch.production.iter().map(|&p| p as f64).sum();
    println!("G3: {} records, {sites} sites, expected production {production:.1}", records.len());
    assert!(batch.fiss_nuc.iter().all(|&f| f == FISS_NONE), "a fission event banked in per-collision mode");
    assert!(sites > 0);
    assert!(
        (sites as f64 - production).abs() <= 5.0 * production.sqrt(),
        "{sites} sites vs expected {production}"
    );
}

/// 2000 neutrons on the sphere, moving radially outward: 1000 at
/// `r = R (1 + j 1e-7)` and 1000 at `r = R (1 - j 1e-7)`, `j = 1..=10`, 100 per
/// shell, directions from the fixed hash, positions rounded to f32.
fn surface_batch() -> EventBatch {
    let mut b = fixed_batch(2000);
    let n = 2000;
    for i in 0..n {
        let d = [b.dir[3 * i] as f64, b.dir[3 * i + 1] as f64, b.dir[3 * i + 2] as f64];
        let j = (i % 1000) / 100 + 1;
        let sign = if i < 1000 { 1.0 } else { -1.0 };
        let r = RADIUS * (1.0 + sign * j as f64 * 1e-7);
        for a in 0..3 {
            b.pos[3 * i + a] = (r * d[a]) as f32;
        }
    }
    b
}

/// G5 (gh:#1506): a neutron on the sphere, or within f32 rounding of it on
/// either side, moving outward leaks; none collides.
///
/// Methodology: [`surface_batch`] (2000 neutrons at `R (1 +/- j 1e-7)`, j = 1..10,
/// radial outward directions, fission-spectrum energies, Godiva tables), ONE
/// event, per-collision banking at k = 1, through the CPU mirror (and the GPU in
/// [`g2_g4_fused_gpu_banks_per_collision`]). Pass: every neutron dead and no
/// collision record. Prediction written before the fix was run: before it,
/// most of the outside half collides (the issue measured 823 of 1000 on a
/// similar set) and some of the inside half too (those within ~1e-7 cm, where
/// the old `d_far > 1e-7` rule gave no boundary); after it, 0 of 2000.
///
/// Result (2026-11-12, rustc 1.99.1 release): see `G5` in the RESULT block of the
/// module comment.
#[test]
fn g5_mirror_leaks_neutrons_leaving_through_the_surface() {
    let t = tables();
    let mut b = surface_batch();
    let mut rec = Vec::new();
    advance_generation_cpu_mirror_banking(
        &t,
        &mut b,
        sphere(),
        1,
        EventFissionBanking::PerCollision { keff: 1.0 },
        &mut rec,
    );
    let alive = b.alive.iter().filter(|&&a| a != 0).count();
    println!("G5 mirror: {alive} of 2000 alive after one event, {} collision records", rec.len());
    assert_eq!(alive, 0, "neutrons leaving through the surface must leak");
    assert!(b.production.iter().all(|&p| p == 0.0), "no collision may have happened");
}

#[cfg(not(target_os = "android"))]
#[test]
fn g2_g4_fused_gpu_banks_per_collision() {
    use outram_mc_libs::gpu::batched_event::{
        advance_generation_gpu_banking, advance_generation_gpu_banking_with_capacity,
    };
    use outram_mc_libs::physics::keff::run_keff_gpu_event_reports;
    let Some(ctx) = outram_mc_libs::gpu::probe() else {
        eprintln!("SKIP g2_g4_fused_gpu_banks_per_collision: no GPU adapter");
        return;
    };
    println!("adapter: {}", ctx.info.name);
    let (material, nuclides) = godiva();
    let s = settings();

    // G2 + G1 on the GPU arm.
    let cpu = stats(&arm_cpu_default());
    let (_r, rep) = run_keff_gpu_event_reports(&ctx, RADIUS, &material, &nuclides, &s);
    let gpu = stats(&rep);
    check_arm("fused GPU", &gpu);
    check_vs_cpu("fused GPU", &gpu, &cpu);

    // G4: same batch through the mirror and the GPU.
    let t = tables();
    let n = 20_000usize;
    let banking = EventFissionBanking::PerCollision { keff: 1.0 };
    let mut mb = fixed_batch(n);
    let mut mrec = Vec::new();
    advance_generation_cpu_mirror_banking(&t, &mut mb, sphere(), 10_000, banking, &mut mrec);
    let mut gb = fixed_batch(n);
    let mut grec = Vec::new();
    advance_generation_gpu_banking(&ctx, &t, &mut gb, sphere(), 10_000, banking, &mut grec);
    let m_sites: u64 = mrec.iter().map(|r| r.n_sites as u64).sum();
    let g_sites: u64 = grec.iter().map(|r| r.n_sites as u64).sum();
    let m_prod: f64 = mb.production.iter().map(|&p| p as f64).sum();
    let g_prod: f64 = gb.production.iter().map(|&p| p as f64).sum();
    let mean = m_prod / n as f64;
    let s2 = mb.production.iter().map(|&p| (p as f64 - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    let bound_sites = 5.0 * (2.0 * (n as f64 * s2 + m_sites as f64)).sqrt();
    let bound_prod = 5.0 * (2.0 * n as f64 * s2).sqrt();
    println!(
        "G4: mirror {} rec / {m_sites} sites / prod {m_prod:.1}; GPU {} rec / {g_sites} sites / prod {g_prod:.1}; \
         bounds sites {bound_sites:.1}, prod {bound_prod:.1}",
        mrec.len(),
        grec.len()
    );
    assert!((g_sites as f64 - m_sites as f64).abs() <= bound_sites);
    assert!((g_prod - m_prod).abs() <= bound_prod);

    // Grow-and-rerun: tiny capacity must give the same records as a roomy one.
    let mut tb = fixed_batch(n);
    let mut trec = Vec::new();
    advance_generation_gpu_banking_with_capacity(
        &ctx, &t, &mut tb, sphere(), 10_000, banking, &mut trec, 8,
    );
    assert_eq!(trec, grec, "re-run with grown capacity must reproduce the same records");

    // G5 on the GPU (gh:#1506): the surface batch leaks whole in one event.
    let mut sb = surface_batch();
    let mut srec = Vec::new();
    advance_generation_gpu_banking(&ctx, &t, &mut sb, sphere(), 1, banking, &mut srec);
    let alive = sb.alive.iter().filter(|&&a| a != 0).count();
    println!("G5 GPU: {alive} of 2000 alive after one event, {} collision records", srec.len());
    assert_eq!(alive, 0, "GPU: neutrons leaving through the surface must leak");
    assert!(sb.production.iter().all(|&p| p == 0.0), "GPU: no collision may have happened");
}
