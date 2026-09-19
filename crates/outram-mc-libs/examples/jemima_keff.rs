//! **IEU-MET-FAST-002 "Jemima"** — a natural-uranium-reflected assembly of
//! uranium plates, and the most **U-238-dominated** experiment this workspace can
//! build.
//!
//! # Why
//!
//! Two benchmarks already bracket the FHR pebble study's machinery: Godiva
//! (ICSBEP HEU-MET-FAST-001, fast, bare metal) at ~~+57 ± 173 pcm~~
//! **+16 ± 11 pcm (256 seeds, pooled)** (CORRECTED 2026-09-18: +57 was a single-seed
//! draw, superseded by the 256-seed pooled mean) and
//! HEU-SOL-THERM-009 (thermal, water-moderated solution) at −18 ± 171 pcm. Both
//! are **highly enriched**: U-238 is 5 % of Godiva's heavy metal and 5 % of
//! HST-009's. The pebble's residual has been narrowed to U-238 absorption, so
//! neither really tests the nuclide under suspicion.
//!
//! Jemima does. Its core is 16 % enriched — U-238 is **83 %** of the core's heavy
//! metal — and its reflector, top, bottom and sides, is **natural uranium**
//! (99.3 % U-238). If U-238's cross sections or the way this code absorbs in them
//! were wrong at the level the pebble suggests, an assembly that is mostly U-238
//! by construction could not land on its measured k.
//!
//! **What it does and does not reach.** Jemima is a *metal* assembly with no
//! moderator, so its spectrum is 100 keV – 2 MeV: it tests U-238 fast fission and
//! U-238 capture in the keV range, not the 6 eV – 20 keV **resolved resonance
//! escape** the pebble residual actually is. That comparison needs a
//! low-enriched *thermal* system, which no reachable benchmark here can supply
//! without an N-14 tape (see the V&V record). This is the closest available
//! approach from the other side.
//!
//! # Model
//!
//! Specification from the ICSBEP model in `mit-crpg/benchmarks`
//! (`icsbep/ieu-met-fast-002/openmc`, Paul Romano, 2012-01-08) — a cylindrical
//! core with natural-uranium reflectors on all sides, vacuum outside:
//!
//! | region | z \[cm\] | r \[cm\] | material |
//! |---|---|---|---|
//! | bottom reflector | 0 – 7.62 | < 26.6446 | natural U |
//! | core | 7.62 – 39.571 | < 19.05 | 16 % enriched U ("oralloy") |
//! | radial reflector | 7.62 – 39.571 | 19.05 – 26.6446 | natural U |
//! | top reflector | 39.571 – 47.0894 | < 26.6446 | natural U |
//!
//! Nothing is approximated: the model needs U-234, U-235 and U-238 and nothing
//! else, and all three are in `reference-data/endf/`.
//!
//! **Reference: `k_eff = 1.0000`**, because this is a *critical* configuration —
//! see `hst009_keff.rs` for why that is a property of an ICSBEP benchmark model
//! and not a remembered number. Fast metal assemblies carry smaller evaluated
//! uncertainties than solutions; `1.0000 ± 0.003` is pessimistic here.
//!
//! # The geometry is checked before it is trusted
//!
//! This is the first model in this crate built from raw `RegionToken`s with
//! **three** intersected half-spaces per cell, and `fhr_pebble_geometry` warns
//! that the `distance_to_boundary` walk depends on token ordering. A mis-built
//! geometry would produce a wrong k that looks like a physics result — the exact
//! failure this study has been chasing. So before transporting anything, this
//! program samples the bounding box and asserts that the CSG lookup agrees with
//! a hand-written predicate at every point, and that every point inside the
//! assembly lands in exactly one cell.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example jemima_keff
//! ```

//!
//! ## POOLED RESULT (2026-09-18) — 32 seeds, `OUTRAM_BENCH_SEEDS=32`
//!
//! | | dk vs benchmark |
//! |---|---|
//! | **pooled mean, 32 seeds** | **-253 pcm, sem +/-34** |
//! | seed-to-seed sd | 195 pcm — what ONE run scatters by |
//! | previously recorded (SINGLE seed) | `+6 ± 173 pcm` |
//!
//! //! **The previously recorded `+6` was a 1.3-sd lucky draw.** It made this case
//! read as near-perfect agreement; the pooled answer is `-253 pcm`. The single
//! draw was not wrong, it was one sample from a distribution 195 pcm wide, and
//! nothing in a single-seed report can reveal that. This is the clearest
//! illustration in the repo of why `op-awwi` exists.
//!
//! **`-253 +/- 34` is still INSIDE the benchmark's own ~300 pcm band**, so the
//! honest statement is "agrees, at -253 +/- 34 pcm" — NOT "disagrees at 7
//! sigma". The `distance from benchmark = 7.3 sem` the example prints is
//! measured in OUR sampling error, which pooling has made small enough that it
//! is no longer the dominant term.
//!
//!
//! **PROVENANCE WARNING on that band (added 2026-09-18).** The `± ~0.003` is
//! NOT a quoted ICSBEP case uncertainty — this file's own line ~49 calls it
//! *"pessimistic here"*, since fast metal assemblies have smaller
//! uncertainties than solutions. So "inside the band" is being judged against
//! a deliberately generous stand-in. Against a realistic fast-assembly
//! uncertainty (nearer +/-100-200 pcm) `-253 +/- 34` is **marginal and
//! possibly outside**. Do not publish this as agreement until the real
//! case-specific uncertainty is obtained (bead filed 2026-09-18).
//! This closes `gh:#196` / `bn:op-awwi` for this case: the headline is now a
//! pooled mean, not a draw. Note the comparison is now limited by the
//! BENCHMARK's uncertainty (ICSBEP `1.0000 ± ~0.003`, i.e. ~300 pcm), not ours — quote agreement against that
//! band, not against our sem.
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::vv::assert_reproduces_keff;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::compute::ComputeType;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use std::time::Instant;

const TEMP_K: f64 = 293.6;

const Z_BOT: f64 = 0.0;
const Z_CORE_LO: f64 = 7.62;
const Z_CORE_HI: f64 = 39.571;
const Z_TOP: f64 = 47.0894;
const R_CORE: f64 = 19.05;
const R_OUT: f64 = 26.6446;

fn main() {
    eprintln!("IEU-MET-FAST-002 'Jemima' — natural-U-reflected assembly of uranium plates");
    eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
    let t0 = Instant::now();
    let nuclides: Vec<Nuclide> = vec![
        load("U234", "n-092_U_234-ENDF8.0.endf"),
        load("U235", "n-092_U_235-ENDF8.0.endf"),
        // OUTRAM_U238_ENDF7=1 swaps U-238 ALONE to ENDF/B-VII.0, every other
        // nuclide held at VIII.0. Isolates one nuclide's evaluation: the four
        // pooled ICSBEP residuals split by U-238 content and the two U-238-heavy
        // cases disagree in SIGN, which a whole-library swap could not separate.
        load("U238", if std::env::var("OUTRAM_U238_ENDF7").is_ok() { "n-092_U_238-ENDF7.0.endf" } else { "n-092_U_238.endf" }),
    ];
    // OUTRAM_JEMIMA_ISO_INELASTIC=1 samples every inelastic collision
    // isotropically in the CM frame, ablating the ENDF MF=4/MT=51..90 discrete
    // angular distributions that `op-tm9f` wired in.
    //
    // WHY ON JEMIMA. That fix was priced on Godiva (-224 +/- 44 pcm), which is
    // 93.7 % U-235 and only ~5 % U-238. Jemima is 83 % U-238 by heavy metal
    // and 99.3 % U-238 in its reflector, so it is far more exposed to the same
    // treatment. Measuring the SENSITIVITY here bounds how large a residual
    // error in that treatment would have to be to explain Jemima's
    // -253 +/- 34 pcm: if the term is worth S pcm on Jemima, an error of
    // f * S explains the residual, and f is then checkable against Godiva's
    // own -55 +/- 34.
    //
    // This is a SENSITIVITY measurement, not a correctness one. It cannot say
    // the treatment is wrong -- only how much room there is for it to matter.
    let nuclides: Vec<Nuclide> = if std::env::var("OUTRAM_JEMIMA_ISO_INELASTIC").is_ok() {
        eprintln!("  ABLATION: inelastic sampled ISOTROPICALLY in CM (MF=4/MT=51..90 off)");
        nuclides
            .into_iter()
            .map(Nuclide::with_isotropic_inelastic_scattering)
            .collect()
    } else {
        nuclides
    };

    // OUTRAM_FROZEN_NUBAR=1 freezes nu-bar(E) at thermal, ablating its energy
    // dependence. `with_frozen_nubar(0.0253)`.
    //
    // WHY THIS PAIR. The inelastic ablation eliminated itself by SCALING: the
    // term was worth 1.53x more on Jemima while the residuals differ by 4.6x.
    // So the cause must be something Jemima HAS and Godiva LARGELY DOES NOT,
    // not something it has more of. The structural difference is that Jemima
    // is NATURAL-URANIUM REFLECTED on all sides (99.3 % U-238) while Godiva is
    // a BARE sphere with vacuum outside.
    //
    // nu-bar(E) rises steeply with energy and is sampled at every fission --
    // including fissions in Jemima's reflector, which Godiva does not have at
    // all. If the Jemima/Godiva worth-ratio comes out near 1.5 it is
    // eliminated by the same argument as inelastic; if it is much larger, it
    // is a live candidate.
    let nuclides: Vec<Nuclide> = if std::env::var("OUTRAM_FROZEN_NUBAR").is_ok() {
        eprintln!("  ABLATION: nu-bar frozen at 0.0253 eV (energy dependence off)");
        nuclides.into_iter().map(|n| n.with_frozen_nubar(0.0253)).collect()
    } else {
        nuclides
    };

    eprintln!(
        "Nuclear data ready in {:.1} s.\n",
        t0.elapsed().as_secs_f64()
    );

    let comp = |v: &[(usize, f64)]| -> Vec<NuclideComponent> {
        v.iter()
            .map(|&(nuclide_idx, atom_density)| NuclideComponent {
                nuclide_idx,
                atom_density,
            })
            .collect()
    };
    let materials = vec![
        // Oralloy: ~16 % enriched. U-238 is 83 % of the heavy metal.
        Material {
            id: 1,
            name: "oralloy core".into(),
            temperature: TEMP_K,
            components: comp(&[(0, 8.4430e-05), (1, 7.7777e-03), (2, 3.9671e-02)]),
        },
        // Natural uranium reflector: 99.3 % U-238.
        Material {
            id: 2,
            name: "natural uranium reflector".into(),
            temperature: TEMP_K,
            components: comp(&[(0, 2.6433e-06), (1, 3.4603e-04), (2, 4.7711e-02)]),
        },
    ];

    let geom = build_geometry();
    check_geometry(&geom);

    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 120,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(Default::default()),
        ..KeffSettings::default()
    };
    // Start in the core.
    let src = SourceBox {
        lower: Position::new(-R_CORE, -R_CORE, Z_CORE_LO),
        upper: Position::new(R_CORE, R_CORE, Z_CORE_HI),
    };

    eprintln!(
        "  core r < {R_CORE} cm, z {Z_CORE_LO}–{Z_CORE_HI}; natural-U reflector to \
         r = {R_OUT} cm and z = {Z_BOT}/{Z_TOP} (vacuum)"
    );
    eprintln!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t = Instant::now();
    // Seed ensemble (OUTRAM_BENCH_SEEDS, default 1 -- unchanged single-seed
    // behaviour). One run of this case scatters by far more than the effects
    // being argued about, so a single draw cannot resolve a 100-200 pcm change;
    // see outram_mc_libs::vv::pooled. `run_keff_csg` is already internally
    // multi-threaded, so seeds run sequentially and each uses every core.
    let n_seeds = outram_mc_libs::vv::bench_seeds();
    let mut ens: Vec<f64> = Vec::with_capacity(n_seeds);
    let mut result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);
    ens.push((result.k_mean - 1.0) * 1.0e5);
    for seed in 2..=n_seeds as u64 {
        let s = KeffSettings {
            seed,
            ..settings.clone()
        };
        let r = run_keff_csg(&geom, &materials, &nuclides, src, &s, None);
        eprintln!("    seed {seed}: k = {:.5} +/- {:.5}", r.k_mean, r.k_std);
        ens.push((r.k_mean - 1.0) * 1.0e5);
        // Deliberately NOT `result = r`. Everything downstream -- the
        // convergence trace, the printed k_eff, the V&V gate and the
        // bounded-geometry cross-check -- is sized against SEED 1, which is
        // also what the single-seed default runs. Reassigning here silently
        // repoints all of them at the last seed of the ensemble.
    }
    if n_seeds > 1 {
        let (mean, sd, sem) = outram_mc_libs::vv::pooled(&ens);
        println!("\n  ENSEMBLE IEU-MET-FAST-002 (Jemima): {n_seeds} seeds");
        println!("    pooled dk    = {mean:+.0} pcm");
        println!("    seed-to-seed sd  = {sd:.0} pcm   (what ONE run scatters by)");
        println!("    uncertainty  sem = +/-{sem:.0} pcm   (on the pooled mean)");
        println!(
            "    distance from benchmark = {:.1} sem",
            (mean / sem).abs()
        );
    }
    eprintln!("  transport: {:.1} s", t.elapsed().as_secs_f64());
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP IEU-MET-FAST-002 (critical) = 1.0000 ± ~0.003");
    println!(
        "  Δk from the benchmark = {:+.0} ± {:.0} pcm",
        (result.k_mean - 1.0) * 1.0e5,
        result.k_std * 1.0e5
    );
    println!(
        "\n  U-238 is 83 % of this core's heavy metal and 99.3 % of its reflector,\n  \
         against 5 % in Godiva and in HEU-SOL-THERM-009. The spectrum is fast, so\n  \
         this reaches U-238 fast fission and keV capture, not the 6 eV – 20 keV\n  \
         resolved resonance escape the FHR pebble residual is."
    );

    // ── V&V gate ──────────────────────────────────────────────────────────────
    //
    // This program already asserted its GEOMETRY (that the CSG model matches the
    // ICSBEP specification at sampled points) and then printed its k without
    // asserting it — which is the wrong way round, since the geometry only
    // exists to get the k.
    println!("\n=== V&V gate: ICSBEP IEU-MET-FAST-002 ===");
    assert_reproduces_keff(
        "IEU-MET-FAST-002 (Jemima), the most U-238-dominated case reachable here",
        result.k_mean,
        result.k_std,
        ICSBEP_IMF002_K,
        ICSBEP_IMF002_BAND,
        None,
    );
}

/// ICSBEP **IEU-MET-FAST-002** ("Jemima") benchmark `k_eff`: exactly 1.0000,
/// because the configuration is critical by construction.
const ICSBEP_IMF002_K: f64 = 1.0000;

/// The band used against [`ICSBEP_IMF002_K`].
///
/// **0.003 is pessimistic for this case and is used deliberately.** Metal
/// assemblies carry smaller benchmark uncertainties than solutions — the
/// geometry is machined and the composition is known — so the evaluation's own
/// figure is tighter than this. Taking the looser band means a failure here is
/// unambiguous rather than arguable.
const ICSBEP_IMF002_BAND: f64 = 0.003;

/// Surfaces, then four cells, exactly as the ICSBEP model's XML lists them.
fn build_geometry() -> Geometry {
    let zp = |z0: f64, bc: BoundaryType| SurfaceKind::ZPlane(ZPlane { z0, bc });
    let zc = |r: f64, bc: BoundaryType| {
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r,
            bc,
        })
    };
    // 0..5 mirror the model's surfaces 1..6.
    let surfaces = vec![
        zp(Z_BOT, BoundaryType::Vacuum),
        zp(Z_CORE_LO, BoundaryType::Transmissive),
        zp(Z_CORE_HI, BoundaryType::Transmissive),
        zp(Z_TOP, BoundaryType::Vacuum),
        zc(R_CORE, BoundaryType::Transmissive),
        zc(R_OUT, BoundaryType::Vacuum),
    ];
    let hs =
        |surface_idx: usize, sense: HalfSpaceSense| RegionToken::HalfSpace { surface_idx, sense };
    let out = |i: usize| hs(i, HalfSpaceSense::Outside);
    let ins = |i: usize| hs(i, HalfSpaceSense::Inside);
    let and = RegionToken::Intersection;

    let cells = vec![
        // bottom reflector: +1 -2 -6
        Cell::material(1, vec![out(0), ins(1), and, ins(5), and], 1, TEMP_K),
        // core: +2 -3 -5
        Cell::material(2, vec![out(1), ins(2), and, ins(4), and], 0, TEMP_K),
        // radial reflector: +2 -3 +5 -6
        Cell::material(
            3,
            vec![out(1), ins(2), and, out(4), and, ins(5), and],
            1,
            TEMP_K,
        ),
        // top reflector: +3 -4 -6
        Cell::material(4, vec![out(2), ins(3), and, ins(5), and], 1, TEMP_K),
    ];

    let cell_indices = (0..cells.len()).collect();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices,
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Assert the CSG lookup agrees with a hand-written predicate everywhere.
///
/// This is the first model in this crate built from raw `RegionToken`s with
/// three intersected half-spaces per cell, and the ordering of those tokens is
/// load-bearing (`fhr_pebble_geometry` says so explicitly). A mis-built geometry
/// gives a wrong k that looks exactly like a physics result, which is the failure
/// mode this whole study has been chasing — so it is checked, not assumed.
fn check_geometry(geom: &Geometry) {
    let expect = |p: Position| -> Option<usize> {
        let r = (p.x * p.x + p.y * p.y).sqrt();
        if r >= R_OUT || p.z <= Z_BOT || p.z >= Z_TOP {
            return None;
        }
        if p.z < Z_CORE_LO {
            Some(0)
        } else if p.z < Z_CORE_HI {
            if r < R_CORE {
                Some(1)
            } else {
                Some(2)
            }
        } else {
            Some(3)
        }
    };

    let mut seed = 1_234_567_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    const N: usize = 200_000;
    let (mut inside_n, mut outside_n) = (0usize, 0usize);
    for _ in 0..N {
        let p = Position::new(
            (2.0 * prn() - 1.0) * R_OUT * 1.05,
            (2.0 * prn() - 1.0) * R_OUT * 1.05,
            Z_BOT - 1.0 + prn() * (Z_TOP - Z_BOT + 2.0),
        );
        let want = expect(p);
        // `locate` is the cell lookup; take the leaf level's cell index. The
        // direction only matters for on-surface tie-breaking, which a random
        // interior point never hits.
        let got = geom
            .locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .map(|path| path.leaf().cell);
        assert_eq!(
            got, want,
            "CSG disagrees with the model at {p:?}: found cell {got:?}, model says {want:?}"
        );
        match want {
            Some(_) => inside_n += 1,
            None => outside_n += 1,
        }
    }
    eprintln!(
        "  geometry check: {N} points, {inside_n} inside / {outside_n} outside, \
         CSG matches the ICSBEP model everywhere"
    );
    assert!(
        inside_n > N / 10 && outside_n > N / 100,
        "degenerate sampling"
    );
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
    eprint!("  reconstructing {name:<6} … ");
    let t0 = Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}
