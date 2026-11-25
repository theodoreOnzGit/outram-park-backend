//! **The HTR-10 core rung's model**: which bed the core runs on, the DEM
//! random bed, the reader's own pour, and the recorded k of the lattice and
//! random beds (gh:#1742; moved 2026-11-25 from `dhoby-ghaut/examples/
//! monte_carlo_web/htr10/core/random_bed.rs`, `htr10/core/pour.rs` and
//! `beds.rs`, verbatim except the paths and two arguments).
//!
//! **Bed codes.** The rung's worker pool passes the choice of bed around as
//! one number: N = 10..=20 is Şeker & Çolak (2003)'s lattice at N layers (as
//! every recorded k), [`DEM_BED`] (0) the DEM random bed, [`USER_POUR`] (1)
//! the reader's own pour (gh:#1244).
//!
//! **The DEM random bed** is the #216 friction study's pour at µ = 0.1,
//! µ_r = 0 (`reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`),
//! cut to the lattice's [`DEM_CORE_BALLS`] core pebbles at N = 12: every
//! pebble at or below the floor kept in file order, then the lowest
//! `16 681` above it by height. The cut is
//! [`crate::htr10_rmc::explicit_bed::trim_to_core_balls`] on
//! [`crate::htr10_rmc::explicit_bed::read_dem_centres_csv`], which the rung's
//! own copy duplicated (it gives the same centres, bit for bit:
//! `the_dem_cut_is_the_rungs_cut`). Fuel identity by `paper_fuel_assignment`
//! (57:43 above the floor, conus and tube all dummy).
//!
//! **What the caller keeps.** The CSV's text is the caller's (the rung
//! compiles it in; an `include_str!` from `reference-data/` here would break
//! this crate's `cargo package`), and so is the reader's pour, which each
//! browser worker holds; both are passed in.
//!
//! **The pour model** (gh:#1244, maintainer 2026-11-05): the settled DEM
//! centres become the core's bed ([`poured_bed`]), cut at
//! [`HEIGHT_LIMIT_CM`], fuel by `fuel_assignment_at` at the reader's
//! fraction; [`max_pebbles`] is the most pebbles predicted to stay under
//! that line. The pour itself is [`crate::dem_pour::engine`] (feature
//! `dem_ui`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::RecordedK;
use crate::htr10_rmc::control_rod::N_CONTROL_RODS;
use crate::htr10_rmc::core_design::{Htr10CoreDesign, TrisoPacking};
use crate::htr10_rmc::core_model::{assemble_explicit_triso, assemble_explicit_triso_with, AssembledCore, HTR10_CORE_CAVITY_CM};
use crate::htr10_rmc::explicit_bed::{
    assemble_explicit_triso_from_centres_with, fuel_assignment_at, paper_fuel_assignment, read_dem_centres_csv, trim_to_core_balls,
};
use uom::si::f64::Length;
use uom::si::length::centimeter;

/// TRISO tile rings of the core the rung builds (`assemble_explicit_triso`'s
/// `n_rings`, the record's).
pub const RINGS: usize = 14;

// ─── Bed codes ───────────────────────────────────────────────────────────────

/// The bed code of the DEM random bed.
pub const DEM_BED: usize = 0;
/// The bed code of the reader's own pour (gh:#1244): the bed the caller
/// passes in.
pub const USER_POUR: usize = 1;

/// Whether a bed code is one of the explicit (DEM) beds.
pub fn is_dem(code: usize) -> bool {
    code == DEM_BED || code == USER_POUR
}

/// Core pebbles (centred above the floor) the random bed is cut to: the
/// lattice's count at N = 12 (`run_e8_N12.log`, "16681 balls").
pub const DEM_CORE_BALLS: usize = 16_681;

/// The random bed's centres: the pour in `dem_csv` (`id,x,y,z,vx,vy,vz`,
/// metres, DEM frame), cut as the module docs say.
///
/// # Errors
/// A malformed CSV, or fewer than [`DEM_CORE_BALLS`] pebbles above the floor.
pub fn dem_centres(dem_csv: &str) -> Result<Vec<[Length; 3]>, String> {
    let all = read_dem_centres_csv(dem_csv).map_err(|e| e.to_string())?;
    trim_to_core_balls(&all, DEM_CORE_BALLS).map_err(|e| e.to_string())
}

/// The centres and fuel flags of an explicit bed code: the DEM bed from
/// `dem_csv`, or the reader's pour `poured`.
///
/// # Errors
/// Not an explicit bed, the baked pour cannot be read, or no pour has been
/// handed over.
pub fn explicit_bed(code: usize, dem_csv: &str, poured: Option<&PouredBed>) -> Result<(Vec<[Length; 3]>, Vec<bool>), String> {
    match code {
        DEM_BED => {
            let c = dem_centres(dem_csv)?;
            let fuel = paper_fuel_assignment(&c);
            Ok((c, fuel))
        }
        USER_POUR => poured
            .map(|b| (b.centres.clone(), b.is_fuel.clone()))
            .ok_or_else(|| "no pour has been made yet: pour a bed first".to_string()),
        _ => Err(format!("bed code {code} is not a DEM bed")),
    }
}

/// Number of control rods.
pub const N_RODS: usize = N_CONTROL_RODS;

/// Every rod withdrawn: the record's state and the default.
pub const WITHDRAWN: [f64; N_CONTROL_RODS] = [0.0; N_CONTROL_RODS];

/// The core for a bed code, rods withdrawn (the record's state).
///
/// # Errors
/// As [`build_core_with`].
pub fn build_core(code: usize, dem_csv: &str, poured: Option<&PouredBed>) -> Result<AssembledCore, String> {
    build_core_with(code, &WITHDRAWN, dem_csv, poured)
}

/// The core for a bed code with each control rod at its own insertion
/// (`rods[i]` in `[0, 1]`, rod `i` as `Htr10CoreDesign::rod_insertions`
/// numbers them; gh:#1025). With [`WITHDRAWN`] it is [`build_core`]'s core
/// exactly, since `Htr10CoreDesign::default()` is the record's design.
///
/// # Errors
/// An insertion outside `[0, 1]`, or the DEM pour cannot be read.
pub fn build_core_with(code: usize, rods: &[f64; N_CONTROL_RODS], dem_csv: &str, poured: Option<&PouredBed>) -> Result<AssembledCore, String> {
    let mut design = Htr10CoreDesign::default();
    design.rod_insertions = *rods;
    design.check()?;
    if is_dem(code) {
        let (c, fuel) = explicit_bed(code, dem_csv, poured)?;
        // The DEM beds packs its TRISO at random (gh:#1243), as the worker
        // pool's `ParametricStudy` builds it, so the picture is the core run.
        design.triso_packing = TrisoPacking::DEM_DEFAULT;
        Ok(assemble_explicit_triso_from_centres_with(&c, &fuel, RINGS, 0, &design))
    } else if rods == &WITHDRAWN {
        Ok(assemble_explicit_triso(RINGS, code.clamp(10, 20), 0))
    } else {
        Ok(assemble_explicit_triso_with(RINGS, code.clamp(10, 20), 0, &design))
    }
}

/// Core pebbles (centred above the bed floor, `PebbleBed::core_balls`) of
/// the lattice the core view builds, `assemble_explicit_triso(14, N, 0)`, for
/// N = 10 ..= 20. Quoted so the page can show the count without building the
/// bed; pinned to the model by the rung's `the_pebble_counts_are_the_beds`.
pub const LATTICE_CORE_BALLS: [usize; 11] = [14_021, 15_351, 16_681, 18_011, 19_341, 20_671, 22_001, 23_331, 24_661, 25_991, 27_321];

/// The core pebbles of a bed code: the DEM bed's, the reader's pour's, or
/// the lattice's at N.
pub fn core_pebbles(code: usize, poured: Option<&PouredBed>) -> Option<usize> {
    if code == DEM_BED {
        Some(DEM_CORE_BALLS)
    } else if code == USER_POUR {
        poured.map(|b| b.core_balls())
    } else {
        code.checked_sub(10).and_then(|i| LATTICE_CORE_BALLS.get(i)).copied()
    }
}

// ─── Recorded k of the two beds ──────────────────────────────────────────────

/// The lattice at N = 12 on ENDF/B-VIII.0, 10 000 × [5 + 135], recorded
/// 2026-10-07 (`nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`).
pub const LATTICE_K: RecordedK = RecordedK {
    k: 0.995125,
    sigma: 0.001055,
    record: "2026-10-07, at dbb9e26eb1",
};

/// This random bed (the gh:#216 pour cut to the lattice's 16 681 balls),
/// same driver and statistics, recorded 2026-10-08
/// (`nee_soon/verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).
pub const RANDOM_K: RecordedK = RecordedK {
    k: 0.989293,
    sigma: 0.000962,
    record: "2026-10-08, at 713cd5bb60",
};

// ─── The reader's pour ───────────────────────────────────────────────────────

/// The highest a poured pebble's top may stand above the bed floor \[cm\]:
/// 95 % of the core cavity (221.818 cm), the maintainer's limit.
pub const HEIGHT_LIMIT_CM: f64 = 0.95 * HTR10_CORE_CAVITY_CM;

/// Pebble radius \[cm\] (`htr10_fill::PEBBLE_RADIUS_M`).
pub const R_CM: f64 = outram_park_fork_liggghts::htr10_fill::PEBBLE_RADIUS_M * 100.0;

/// Whole-core φ of a fresh `htr10_fill` pour from empty (`seed_loose`'s doc,
/// 16 890 pebbles, settled after 26 000 steps): the pour the page runs, so
/// the cap uses it rather than the 0.6047 of the LIGGGHTS-seeded reference
/// bed. A measured figure, not fitted to anything here.
pub const FRESH_POUR_PHI: f64 = 0.5954;

/// Pebbles centred at or below the floor (conus and tube) in the reference
/// pour (`htr10_conus_presettled_mu10_mur00.csv`, 2 457 of 27 554; pinned by
/// the rung's `the_cap_is_derived_from_the_reference_pour`).
pub const BELOW_FLOOR_PEBBLES: usize = 2_457;

/// Core pebbles of the record's loading the page starts at (maintainer
/// 2026-11-05: "default to the record's loading, ~16,900 core pebbles"):
/// HTR-10's first-criticality loading, 16 890 balls (`htr10_fill`'s
/// settings doc).
pub const RECORD_CORE_PEBBLES: usize = 16_890;

/// The pour the page starts at: [`RECORD_CORE_PEBBLES`] in the core plus the
/// conus and tube's [`BELOW_FLOOR_PEBBLES`] (the reference pour's count; a
/// fresh pour's own split may differ by a few).
pub const DEFAULT_PEBBLES: usize = RECORD_CORE_PEBBLES + BELOW_FLOOR_PEBBLES;

/// **Measured browser pours** (gh:#1244 record, headless Chromium, one
/// thread, a 4-vCPU container; maintainer chose on gh:#1250, 2026-11-05, to
/// keep the CPU pour and say how long it takes): pebbles, seconds to settle.
/// The 4 000 and 8 000 pours ran on a loaded machine, the 30 687 on an idle
/// one, so the table is an upper bound for a desktop of that class.
pub const MEASURED_BROWSER_POURS: [(usize, f64); 3] = [(4_000, 242.0), (8_000, 339.0), (30_687, 1_051.0)];

/// **How long a pour of `n` pebbles takes in a browser**, s: straight lines
/// between the [`MEASURED_BROWSER_POURS`] (no fit), the first line extended
/// below 4 000. A guide for the reader before they press Pour, not a
/// promise: a phone or a busy machine is slower.
pub fn expected_pour_s(n: usize) -> f64 {
    let m = MEASURED_BROWSER_POURS;
    let x = n as f64;
    let seg = if n <= m[1].0 { (m[0], m[1]) } else { (m[1], m[2]) };
    let ((x0, y0), (x1, y1)) = ((seg.0 .0 as f64, seg.0 .1), (seg.1 .0 as f64, seg.1 .1));
    let y = y0 + (y1 - y0) * (x - x0) / (x1 - x0);
    y.max(0.0)
}

/// The fewest pebbles the page pours: enough to fill the tube and conus and
/// put a layer in the core.
pub const MIN_PEBBLES: usize = 3_000;

/// **The most pebbles the page lets the reader pour**: the conus and tube's
/// [`BELOW_FLOOR_PEBBLES`] plus a core filled at [`FRESH_POUR_PHI`] to a
/// surface at [`HEIGHT_LIMIT_CM`] (`htr10_fill`'s whole-core instrument,
/// `φ = N V / (π R² h)`, solved for `N`).
pub fn max_pebbles() -> usize {
    let r_core = outram_park_fork_liggghts::htr10_fill::CORE_RADIUS_M * 100.0;
    let v = 4.0 / 3.0 * core::f64::consts::PI * R_CM * R_CM * R_CM;
    let core = FRESH_POUR_PHI * core::f64::consts::PI * r_core * r_core * HEIGHT_LIMIT_CM / v;
    BELOW_FLOOR_PEBBLES + core.floor() as usize
}

/// A poured bed the core is built on.
#[derive(Clone, Debug, PartialEq)]
pub struct PouredBed {
    /// Centres, DEM frame.
    pub centres: Vec<[Length; 3]>,
    pub is_fuel: Vec<bool>,
    /// Which pour this is (the page counts them), so a rebuilt slice or core
    /// is told apart from the last one.
    pub version: u32,
    /// Pebbles removed for standing above [`HEIGHT_LIMIT_CM`].
    pub cut: usize,
    /// The fuel fraction asked for (above the floor).
    pub fuel_fraction: f64,
}

impl PouredBed {
    /// Pebbles centred above the floor.
    pub fn core_balls(&self) -> usize {
        self.centres.iter().filter(|c| c[2].value > 0.0).count()
    }
    /// Fuel pebbles.
    pub fn fuel_balls(&self) -> usize {
        self.is_fuel.iter().filter(|&&f| f).count()
    }
    /// The top of the highest pebble above the floor \[cm\].
    pub fn top_cm(&self) -> f64 {
        self.centres
            .iter()
            .map(|c| c[2].get::<centimeter>() + R_CM)
            .fold(f64::NEG_INFINITY, f64::max)
    }
    /// The page's wire form: centres in cm (x, y, z) and the fuel flags as
    /// 0/1, and back ([`Self::from_wire`]).
    pub fn to_wire(&self) -> (Vec<f64>, Vec<f64>) {
        (
            self.centres
                .iter()
                .flat_map(|c| c.map(|v| v.get::<centimeter>()))
                .collect(),
            self.is_fuel.iter().map(|&f| f64::from(u8::from(f))).collect(),
        )
    }
    /// The bed from its wire form ([`Self::to_wire`]).
    ///
    /// # Errors
    /// The lengths do not match, or there are no pebbles.
    pub fn from_wire(cm: &[f64], fuel: &[f64], version: u32, cut: usize, fuel_fraction: f64) -> Result<Self, String> {
        if cm.len() != 3 * fuel.len() || fuel.is_empty() {
            return Err(format!("poured bed: {} numbers for {} flags", cm.len(), fuel.len()));
        }
        Ok(Self {
            centres: cm.chunks_exact(3).map(|c| [0, 1, 2].map(|k| Length::new::<centimeter>(c[k]))).collect(),
            is_fuel: fuel.iter().map(|&f| f != 0.0).collect(),
            version,
            cut,
            fuel_fraction,
        })
    }
}

/// **The pour's centres (m, `x, y, z` flattened, as the engine streams
/// them) as the core's bed**: pebbles whose top stands above
/// [`HEIGHT_LIMIT_CM`] removed, fuel at `fuel_fraction` of the pebbles above
/// the floor by `fuel_assignment_at`.
///
/// # Errors
/// No pebble is left above the floor.
pub fn poured_bed(centres_m: &[f32], fuel_fraction: f64, version: u32) -> Result<PouredBed, String> {
    let all: Vec<[Length; 3]> = centres_m
        .chunks_exact(3)
        .map(|c| [0, 1, 2].map(|k| Length::new::<centimeter>(f64::from(c[k]) * 100.0)))
        .collect();
    let n = all.len();
    let centres: Vec<[Length; 3]> = all
        .into_iter()
        .filter(|c| c[2].get::<centimeter>() + R_CM <= HEIGHT_LIMIT_CM)
        .collect();
    let cut = n - centres.len();
    if !centres.iter().any(|c| c[2].value > 0.0) {
        return Err("the pour has no pebble above the bed floor".into());
    }
    let is_fuel = fuel_assignment_at(&centres, fuel_fraction);
    Ok(PouredBed {
        centres,
        is_fuel,
        version,
        cut,
        fuel_fraction: fuel_fraction.clamp(0.0, 1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bed codes, the lattice counts and the pour's wire form.
    #[test]
    fn codes_counts_and_the_wire_form() {
        assert!(is_dem(DEM_BED) && is_dem(USER_POUR) && !is_dem(12));
        assert_eq!(core_pebbles(12, None), Some(16_681));
        assert_eq!(core_pebbles(DEM_BED, None), Some(DEM_CORE_BALLS));
        assert_eq!(core_pebbles(USER_POUR, None), None);
        assert!(explicit_bed(USER_POUR, "", None).is_err());
        assert!(explicit_bed(12, "", None).is_err());
        let bed = poured_bed(&[0.0, 0.0, -0.1, 0.1, 0.0, 0.5, 0.0, 0.1, 5.0], 0.5, 3).expect("a bed");
        assert_eq!((bed.centres.len(), bed.cut, bed.core_balls()), (2, 1, 1));
        let (cm, fuel) = bed.to_wire();
        let back = PouredBed::from_wire(&cm, &fuel, 3, 1, 0.5).expect("round trip");
        assert_eq!(back.is_fuel, bed.is_fuel);
        assert_eq!(core_pebbles(USER_POUR, Some(&back)), Some(1));
        assert!(poured_bed(&[0.0, 0.0, -0.1], 0.5, 1).is_err());
        assert!(max_pebbles() > DEFAULT_PEBBLES);
        assert!(expected_pour_s(8_000) == 339.0 && expected_pour_s(0) >= 0.0);
        assert!(build_core_with(12, &[2.0; N_CONTROL_RODS], "", None).is_err());
    }

    /// **The DEM cut is the rung's cut, bit for bit** (gh:#1742): the rung's
    /// own parser and cut (moved here as the oracle: every field parsed,
    /// `z <= 0` kept in file order, the rest sorted by height alone and
    /// truncated) against [`dem_centres`] on the committed pour. The two
    /// sort rules differ only on equal heights, and the bed has none in the
    /// kept range.
    #[cfg(feature = "std")]
    #[test]
    fn the_dem_cut_is_the_rungs_cut() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");
        let text = std::fs::read_to_string(&path).expect("the committed DEM bed");
        let mut below = Vec::new();
        let mut above = Vec::new();
        for line in text.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            let f: Vec<f64> = line.split(',').map(|v| v.trim().parse::<f64>().expect("a number")).collect();
            let c = [f[1], f[2], f[3]];
            if c[2] <= 0.0 {
                below.push(c);
            } else {
                above.push(c);
            }
        }
        above.sort_by(|p, q| p[2].total_cmp(&q[2]));
        above.truncate(DEM_CORE_BALLS);
        below.extend(above);
        let oracle: Vec<[Length; 3]> = below.iter().map(|c| c.map(Length::new::<uom::si::length::meter>)).collect();
        let ours = dem_centres(&text).expect("the cut");
        assert_eq!(ours.len(), oracle.len());
        for (a, b) in ours.iter().zip(&oracle) {
            for k in 0..3 {
                assert_eq!(a[k].value.to_bits(), b[k].value.to_bits());
            }
        }
    }
}
