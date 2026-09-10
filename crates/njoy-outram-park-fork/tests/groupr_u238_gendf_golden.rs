//! GROUPR group-averaging engine vs a real NJOY2016 **GENDF golden tape** —
//! U-238 (MAT 9237), ENDF/B-VIII.0, 293.6 K, 29 groups, `iwt = 3` (1/E),
//! six background dilutions.
//!
//! This is the first golden-file validation of the ported GROUPR numeric
//! engine (`op-cjw.15` / `op-3ut` / `op-cpz` / `op-ini`: "none of it is
//! validated against NJOY output yet"). It exercises the **vector** path
//! (`mfd = 3`) with the **Bondarenko self-shielded flux** (`nsigz > 1`,
//! narrow-resonance branch of `genflx`, `groupr.f90:5623-5665`), i.e.
//! [`genflx_bondarenko`] + [`group_integral`] / [`self_shielded_group_xs`].
//!
//! # Oracle
//!
//! NJOY2016 upstream commit `ac5adf5` (version 2016.79), built in-session with
//! gfortran (2026-09-10), run on `reference-data/endf/n-092_U_238.endf` with the
//! deck committed verbatim next to the golden tape,
//! `reference-data/gendf/u238-ENDF8.0-293.6K-29g-iwt3-6sigz.njoy-input`:
//!
//! ```text
//! reconr  20 21 / 9237 0 / 0.001 /
//! broadr  20 21 22 / 9237 1 / 0.001 / 293.6 /
//! groupr  20 22 0 23 / 9237 1 0 3 0 1 6 1 /   (ign=1, igg=0, iwt=3, lord=0,
//!         293.6 / 1e10 1e4 1e3 1e2 10 1 /       ntemp=1, nsigz=6, iprint=1)
//!         29 / <30 group breaks, eV> /
//!         3 1 / 3 2 / 3 18 / 3 102 / 0 / 0 /
//! ```
//!
//! The golden tape is the resulting GENDF (`tape23`, 427 lines, ASCII). NJOY
//! stores every value with 7 significant figures, which caps the resolution of
//! any comparison at ~5e-7 relative. NJOY's own listing for the run: RECONR
//! 448,168 unionized points (363,092 "affected by resonance integral check"),
//! BROADR `points in= 448168  points out= 155207`, GROUPR narrow-resonance
//! flux on 156,854 points.
//!
//! # What upstream does, read before measuring (CLAUDE.md hard rule)
//!
//! - `getwtf` (`groupr.f90:5203-5205`): `iwt = 3` is `wtf = 1/e` with
//!   `enext = 1.01*e` — the 1/E weight adds its own break point every 1 % in
//!   energy. With `nsigz > 1`, `genflx` (`:5623-5665`) tabulates
//!   `fout = wtf * (sigpot + sigz)/(sigt + sigz)` on the **greedy union** of
//!   those 1 % steps and the PENDF total-cross-section grid (`gety1` on MT=1),
//!   and `getflx` (`:6478-6490`) then interpolates that table **lin-lin**
//!   (`terp1(..., 2)`). `sigpot = 0` unless `iwt < 0` (`:4978`).
//! - Because `lord = 0` with `nsigz > 1`, `getflx` promotes to `llord = 1`, so
//!   the GENDF's **MT=1** section carries `NL = 2` Legendre orders (P0 flux, and
//!   the P1 "current" weight `fout(l-1)*fac`) — its group LISTs hold
//!   `NW = NL*NZ*NG2 = 24` words ordered `il` fastest, then `iz`, then `it`
//!   (`groupr.f90:900-946`); MT=2/18/102 stay at `NL = 1` (12 words). Only the
//!   P0 words (`il = 1`) are compared here: index `NL*iz` for the flux and
//!   `NL*NZ + NL*iz` for the cross section (`iz = 0..5`).
//! - `panel` (`:5858-6091`) integrates `phi` by the trapezoid over each
//!   sub-panel between successive break points and assumes the reaction rate
//!   `sigma*phi` is **linear across the panel** (`rr = b + (a-b)*t1`), which
//!   the Lobatto rule then integrates as the same trapezoid — exactly the rule
//!   [`group_integral`] applies. Its only departures are the inward sampling
//!   nudges `elo*1.000002` / `ehi*0.999995` on the panel end points.
//!
//! **Prediction stated before measuring.** Feeding NJOY's *own* PENDF into the
//! Rust engine with the flux tabulated on the same greedy grid, both the group
//! flux `int phi dE` and `sigma_g(sigma_0)` should agree to better than 1e-5
//! relative in every group and dilution (the residual being the sampling
//! nudges and the 7-figure storage). Feeding the crate's *own* RECONR + BROADR
//! instead should agree to O(1e-3) — the 0.001 reconstruction tolerance on
//! both sides applied to different point sets.
//!
//! # Two tiers
//!
//! 1. **Engine isolation (strict)** — `njoy_pendf_isolates_the_group_averaging_engine`:
//!    the pointwise input is NJOY's own 293.6 K PENDF (41.6 MB, *not* committed;
//!    set `OUTRAM_PARK_NJOY_U238_PENDF` to it — the deck above regenerates it).
//!    Skips with a note when the variable is unset.
//! 2. **End-to-end (committed)** — `crate_reconr_broadr_vs_njoy_gendf`: the
//!    pointwise input is the crate's RECONR (`tolerance = 0.001`) + BROADR
//!    (293.6 K) from the committed ENDF tape. Skips when either the tape or the
//!    golden GENDF is absent. Takes ~3 min in release (the U-238 RECONR).
//! 3. **URR self-shielding through MT=152 (strict)** —
//!    `njoy_unresr_pendf_urr_self_shielding`: a second oracle run with
//!    `unresr 20 22 24 / 9237 1 6 1 / 293.6 / <sigz> / 0 /` between BROADR and
//!    GROUPR (deck `…-unresr.njoy-input`, golden `…-unresr.gendf`), so NJOY's
//!    `stounr` found the MF=2/MT=152 table and `getunr` shielded both the
//!    flux's total and the reaction cross sections inside the unresolved range
//!    (20 keV – 149 keV for U-238: groups 22, 23, bottom of 24). The pointwise
//!    input is that run's PENDF (`OUTRAM_PARK_NJOY_U238_UNRESR_PENDF`, *not*
//!    committed); [`read_urr_from_tape`] on the same tape feeds the engine its
//!    table. Skips when the variable is unset.
//! 4. **Flux calculator (`iwt = -3`, strict)** —
//!    `njoy_flux_calculator_slowing_down`: a third oracle run, the tier-1 deck
//!    with `iwt = -3` and card 8a `fehi = 1e4 eV, sigpot = 11.29 b,
//!    nflmax = 300000` (deck `…-iwt-3-fehi1e4-6sigz.njoy-input`, golden
//!    `…-iwt-3-fehi1e4-6sigz.gendf`), so `genflx` solves the integral
//!    slowing-down equation from `egn(1)` to `fehi` (`groupr.f90:5396-5620`) —
//!    the homogeneous single-moderator branch [`genflx_slowing_down`] ports.
//!    Pointwise input is the tier-1 PENDF (same variable). NJOY's listing
//!    gives the grid to reproduce: 926 tail points (1e-5 → 0.0997 eV on the
//!    1 % ladder), 99,934 solved points (0.1 eV → 1e4), 55,488 NR points to
//!    3e7 eV.
//!
//! # Results (2026-09-10, all tiers run on this machine)
//!
//! **Tier 1 — engine isolated on NJOY's PENDF** (155,207 sigt points → 156,864
//! flux points, NJOY's listing says 156,854 — its loop stops one point short
//! of `etop`): max relative deviation over all 29 groups × 6 dilutions
//!
//! | MT | `sigma_g`, σ0 = ∞ | `sigma_g`, σ0 = 1 b | worst (any σ0) |
//! |---|---|---|---|
//! | 1 (total) | 6.85e-7 | 4.69e-7 | 6.85e-7 |
//! | 2 (elastic) | 4.55e-7 | 3.84e-7 | 4.55e-7 |
//! | 18 (fission) | 2.07e-6 | 7.28e-7 | 2.07e-6 |
//! | 102 (capture) | 2.65e-6 | 1.08e-6 | **2.65e-6** (group 9, σ0 = ∞: 1.5263030 vs 1.526299) |
//!
//! Group flux: worst **4.93e-7** (MT=1, group 10, σ0 = 1 b: 0.010239215 vs
//! 0.01023921) — i.e. at the 7-figure storage floor. Prediction (< 1e-5) met
//! with a factor of ~4 to spare; the engine reproduces `genflx` + `panel`.
//!
//! **Tier 2 — crate RECONR + BROADR** (961,139 sigt points → 962,803 flux
//! points; RECONR + BROADR 156 s):
//!
//! | MT | `sigma_g`, σ0 = ∞ | `sigma_g`, σ0 = 1 b |
//! |---|---|---|
//! | 1 | 1.98e-4 | 6.58e-5 |
//! | 2 | 1.98e-4 | 4.08e-5 |
//! | 18 | **1.12e-2** | **1.14e-2** (group 13, 40–50 eV, σ0 = 1 b: 2.4897e-7 vs 2.4618e-7 b) |
//! | 102 | 3.04e-4 | 9.24e-4 |
//!
//! Group flux: worst 6.06e-5 (MT=1, group 20, σ0 = 1 b). MT=1/2/102 meet the
//! O(1e-3) prediction. MT=18 does not, and upstream explains why: sub-threshold
//! U-238 fission is ~2.5e-7 b, far under RECONR's resonance-integral floor
//! `errint = err/20000 = 5e-8` b (`reconr.f90:112-117, 430`; NJOY's listing:
//! "max resonance-integral error 5.000E-08", 363,092 points affected). Where a
//! point's integral contribution is below `errint`, NJOY converges it only to
//! `errmax = 10*err = 1 %` (`:109-111, 428`). The crate's RECONR has no such
//! relaxation (hence 961k vs NJOY's 448k grid points), so it is the *crate*
//! value that is the tighter reconstruction; the 1.1 % is NJOY's own
//! linearization slack on a negligible cross section, and tier 1 shows the
//! group-averaging engine reproduces NJOY's `sigma_f` to 2e-6 when fed NJOY's
//! grid. MT=18 is therefore asserted at 3e-2 in tier 2, the others at 3e-3.
//!
//! **Tier 3 — URR self-shielding via MT=152, engine isolated on the UNRESR
//! PENDF** (same 155,207-point grid). First measurement, with the flux built
//! from the *smooth* total: `sigma_g` within 1.48e-3 (MT=102, group 22,
//! σ0 = 1 b) / 3.43e-3 (MT=18) but the group flux **6.78 % low** in group 22
//! at σ0 = 1 b (0.062356 vs 0.066892) — exactly the prediction from reading
//! `genflx`, which calls `getunr(1, e, en, tot)` (`groupr.f90:5636-5650`) so
//! the denominator of `fac` is the MT=152 *shielded* total per dilution, while
//! the port used the smooth one. After porting that (`genflx_bondarenko_urr`,
//! now what `self_shielded_group_xs` uses): `sigma_g` within **2.65e-6**
//! (MT=102, group 9) / 2.07e-6 (MT=18), group flux within **4.93e-7** — the
//! maxima moved back to the resolved range, i.e. the URR groups agree to the
//! storage floor. The `read_urr_from_tape` decoder + `UnresolvedTable::shield`
//! (`stounr`/`getunr`) + the URR flux are thereby validated for `LSSF = 1`
//! U-238 at one temperature.
//!
//! **Tier 4 — slowing-down flux calculator, engine isolated on NJOY's PENDF.**
//! Three port defects surfaced, each predicted from `groupr.f90` before the
//! measurement that confirmed it: (a) the tail below `felo` was a single point
//! at 1e-5 eV where NJOY tabulates `factor*wtf` on `getwtf`'s 1 % ladder
//! (`:5563-5577`; 926 points) — a lin-lin 1/E across four decades; (b) the NR
//! extension above `fehi` marched only the total-xs grid where NJOY also takes
//! the 1 % step (`:5626-5631`); (c) the NR in-scatter source below `fehi`
//! uses the loop's last `wtf`, i.e. the weight *at `fehi`*, not `wtf(e)`
//! (`:5460`) — with `wtf(e)` the 5–10 keV group flux was 7.4e-4 off at
//! σ0 = 1 b, the rest already ≤ 8e-6. After (a)–(c): the port's table is
//! 926 / 99,934 / 55,489 points (NJOY reports 55,488 for the NR part — one
//! point at the 3e7 eV top, outside every group), `sigma_g` within
//! **2.87e-6** (MT=102, group 9, σ0 = 1e3 b) / 1.95e-6 (MT=18), group flux
//! within **3.89e-7** (MT=1, group 21, σ0 = 1 b). The homogeneous
//! slowing-down solve, the weight-shape tail and the NR matching are thereby
//! validated for U-238 at one temperature (op-eqa's "needs a golden NJOY
//! genflx flux" half; its heterogeneity / multi-moderator terms stay unported).
//!
//! Side findings filed as follow-ups rather than fixed here: the crate's
//! RECONR does not implement the `errint`/`errmax` resonance-integral
//! relaxation, and the crate's BROADR does not thin (NJOY: 448,168 → 155,207).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::panel::{group_integral, GroupFlux, PointwiseXs};
use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
use njoy_outram_park_fork::groupr::self_shielded::self_shielded_group_xs;
use njoy_outram_park_fork::groupr::slowing_down::{genflx_slowing_down, SlowingDownParams};
use njoy_outram_park_fork::groupr::unresolved::{
    genflx_bondarenko_urr, SelfShieldedFluxSet, UnresolvedTable, UrrReaction,
};
use njoy_outram_park_fork::groupr::urr_pendf::read_urr_from_tape;
use njoy_outram_park_fork::groupr::weights::AnalyticWeight;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
const GOLDEN_GENDF: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz.gendf";
const NJOY_PENDF_ENV: &str = "OUTRAM_PARK_NJOY_U238_PENDF";
const GOLDEN_GENDF_URR: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.gendf";
const NJOY_PENDF_URR_ENV: &str = "OUTRAM_PARK_NJOY_U238_UNRESR_PENDF";
const GOLDEN_GENDF_SD: &str = "u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-6sigz.gendf";
/// Card 8a of the flux-calculator deck: `fehi sigpot nflmax`.
const SD_FEHI: f64 = 1e4;
const SD_SIGPOT: f64 = 11.29;
const SD_NFLMAX: usize = 300_000;

/// The 30 group breaks of the deck (card 6b), eV, ascending.
const BOUNDS: [f64; 30] = [
    1e-5, 0.1, 0.625, 1.0, 4.0, 6.5, 8.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 100.0, 200.0,
    500.0, 1e3, 2e3, 5e3, 1e4, 2e4, 5e4, 1e5, 5e5, 1e6, 2e6, 5e6, 1e7, 2e7,
];
const N_GROUPS: usize = BOUNDS.len() - 1;
/// Card 5 `sigz`, barn, in NJOY's order (infinity first).
const SIGZ: [f64; 6] = [1e10, 1e4, 1e3, 1e2, 10.0, 1.0];
const N_SIGZ: usize = SIGZ.len();
/// The reactions in the deck and the URR column each corresponds to (the
/// column is unused here because no URR table is supplied — NJOY ran without
/// UNRESR, so its `stounr` reported "no unresolved sigma zero data" and both
/// sides shield through the Bondarenko flux alone).
const MTS: [(i32, UrrReaction); 4] = [
    (1, UrrReaction::Total),
    (2, UrrReaction::Elastic),
    (18, UrrReaction::Fission),
    (102, UrrReaction::Capture),
];

/// Tier-1 pass criterion (the prediction): every `sigma_g` and group flux
/// within this relative tolerance of the GENDF.
const TIER1_TOL: f64 = 1e-5;
/// Tier-2 pass criteria: MT=1/2/102 `sigma_g` and the group flux; and MT=18
/// separately (NJOY's `errmax` slack on sub-threshold fission, see module doc).
const TIER2_TOL: f64 = 3e-3;
const TIER2_TOL_MT18: f64 = 3e-2;

/// One reaction's P0 golden values: `[group][dilution]`.
struct GoldenReaction {
    flux: Vec<[f64; N_SIGZ]>,
    sigma: Vec<[f64; N_SIGZ]>,
}

/// Load the golden GENDF: MF=1/451 group breaks (checked against [`BOUNDS`])
/// and the P0 flux / cross-section words of every MF=3 section.
fn load_golden(path: &std::path::Path) -> BTreeMap<i32, GoldenReaction> {
    let tape = Tape::read_file(path).expect("golden GENDF parses as an ENDF tape");

    // MF=1/451: HEAD (za, 0, 0, nz, -1, ntw) + LIST (temp, 0, ngn, ngg, nw, 0)
    // whose data words are [ntw title slots][nz sigz][ngn+1 egn][ngg+1 egg].
    let head = tape.section(MAT, 1, 451).expect("GENDF MF=1/451 present");
    let rows = &head.rows;
    let nz = rows[0][3] as usize;
    let ntw = rows[0][5] as usize;
    let ngn = rows[1][2] as usize;
    assert_eq!(nz, N_SIGZ, "GENDF nsigz");
    assert_eq!(ngn, N_GROUPS, "GENDF ngn");
    let words: Vec<f64> = rows[2..].iter().flat_map(|r| r.iter().copied()).collect();
    for (iz, &s0) in SIGZ.iter().enumerate() {
        let got = words[ntw + iz];
        assert!(
            ((got - s0) / s0).abs() < 1e-6,
            "GENDF sigz[{iz}] = {got:e}, deck says {s0:e}"
        );
    }
    for (g, &eg) in BOUNDS.iter().enumerate() {
        let got = words[ntw + nz + g];
        assert!(
            ((got - eg) / eg).abs() < 1e-6,
            "GENDF egn[{g}] = {got:e}, deck says {eg:e}"
        );
    }

    let mut out = BTreeMap::new();
    for (mt, _) in MTS {
        let sec = tape
            .section(MAT, 3, mt)
            .unwrap_or_else(|| panic!("GENDF MF=3/MT={mt} present"));
        let g = GendfSection::from_rows(3, mt, &sec.rows).expect("GENDF section decodes");
        // lord=0 with nsigz>1: NJOY promotes MT=1 (the total, which carries the
        // P1 "current" weight fout(l-1)*fac) to NL=2; the other reactions stay
        // NL=1. Index by each section's own NL.
        let expected_nl = if mt == 1 { 2 } else { 1 };
        assert_eq!(g.nl, expected_nl, "MT={mt}: NL");
        assert_eq!(g.nz, N_SIGZ as i32, "MT={mt}: NZ");
        assert_eq!(g.num_groups, N_GROUPS as i32, "MT={mt}: NGN");
        let nl = g.nl as usize;
        let mut flux = vec![[0.0; N_SIGZ]; N_GROUPS];
        let mut sigma = vec![[0.0; N_SIGZ]; N_GROUPS];
        for rec in &g.records {
            assert_eq!(rec.ng2, 2, "vector record");
            assert_eq!(rec.data.len(), nl * N_SIGZ * 2, "NW = NL*NZ*NG2");
            let ig = (rec.ig - 1) as usize;
            for iz in 0..N_SIGZ {
                // ans(il, iz, it): il fastest. P0 is il = 0.
                flux[ig][iz] = rec.data[nl * iz];
                sigma[ig][iz] = rec.data[nl * N_SIGZ + nl * iz];
            }
        }
        out.insert(mt, GoldenReaction { flux, sigma });
    }
    out
}

/// NJOY's flux tabulation grid for `iwt = 3`, `nsigz > 1`: starting from the
/// first total-cross-section energy, each step is `min(1.01*e, next sigt point)`
/// (`genflx` loop, `groupr.f90:5623-5636`, with `getwtf`'s `enext = 1.01*e`).
/// Reproduced exactly so the lin-lin flux table matches NJOY's point for point.
fn njoy_flux_grid(sigt: &[(f64, f64)]) -> Vec<f64> {
    let etop = sigt.last().unwrap().0;
    let mut grid = Vec::with_capacity(sigt.len() * 2);
    let mut e = sigt[0].0;
    grid.push(e);
    let mut ip = 1;
    while e < etop {
        while ip < sigt.len() && sigt[ip].0 <= e {
            ip += 1;
        }
        let en = if ip < sigt.len() { sigt[ip].0 } else { etop };
        let enext = (1.01 * e).min(en);
        grid.push(enext);
        e = enext;
    }
    grid
}

/// Largest relative deviation seen, with where it happened.
#[derive(Debug, Clone, Copy, Default)]
struct MaxDev {
    rel: f64,
    mt: i32,
    group: usize,
    iz: usize,
    ours: f64,
    njoy: f64,
}

impl MaxDev {
    fn update(&mut self, mt: i32, group: usize, iz: usize, ours: f64, njoy: f64) {
        let rel = if njoy != 0.0 {
            ((ours - njoy) / njoy).abs()
        } else {
            ours.abs()
        };
        if rel > self.rel {
            *self = MaxDev {
                rel,
                mt,
                group,
                iz,
                ours,
                njoy,
            };
        }
    }
}

impl fmt::Display for MaxDev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.3e} (MT={}, group {}, sigma0={:e} b: ours {:.7e} vs NJOY {:.7e})",
            self.rel, self.mt, self.group, SIGZ[self.iz], self.ours, self.njoy
        )
    }
}

/// Per-tier deviation summary: `sigma` for MT=1/2/102, `sigma_mt18` for
/// fission, and the group `flux`.
struct Deviations {
    sigma: MaxDev,
    sigma_mt18: MaxDev,
    flux: MaxDev,
}

/// Run the ported engine on `pointwise` (MT → lin-lin pairs) and compare with
/// the golden values.
fn compare(
    label: &str,
    pointwise: &BTreeMap<i32, Vec<(f64, f64)>>,
    golden: &BTreeMap<i32, GoldenReaction>,
    urr: Option<&UnresolvedTable>,
    flux_override: Option<&SelfShieldedFluxSet>,
) -> Deviations {
    let sigt_pairs = &pointwise[&1];
    let grid = njoy_flux_grid(sigt_pairs);
    println!(
        "[{label}] sigt grid {} points, NJOY-style flux grid {} points",
        sigt_pairs.len(),
        grid.len()
    );
    let sig_t = PointwiseXs::LinLin(Arc::new(sigt_pairs.clone()));
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    // sigpot = 0: NJOY reads sigpot only for iwt < 0 (groupr.f90:4978).
    let bondarenko;
    let flux_set = match flux_override {
        Some(f) => f,
        None => {
            bondarenko = genflx_bondarenko_urr(&sig_t, urr, &weight, 0.0, &SIGZ, &grid).unwrap();
            &bondarenko
        }
    };

    let mut dev = Deviations {
        sigma: MaxDev::default(),
        sigma_mt18: MaxDev::default(),
        flux: MaxDev::default(),
    };
    for (mt, reaction) in MTS {
        let pairs = &pointwise[&mt];
        let sig_rx = PointwiseXs::LinLin(Arc::new(pairs.clone()));
        // With a precomputed (flux-calculator) flux set there is no URR table
        // in play, so sigma_g is the plain flux-weighted average with that flux;
        // otherwise it is the engine's assembled path.
        let sigma_g: Vec<Vec<f64>> = match flux_override {
            Some(f) => (0..N_SIGZ)
                .map(|iz| {
                    let phi = f.flux(iz).unwrap();
                    (0..N_GROUPS)
                        .map(|g| group_integral(&sig_rx, phi, BOUNDS[g], BOUNDS[g + 1]).average())
                        .collect()
                })
                .collect(),
            None => {
                let m = self_shielded_group_xs(
                    &sig_t, &sig_rx, urr, reaction, &weight, 0.0, &SIGZ, &BOUNDS, &grid,
                )
                .unwrap();
                (0..N_SIGZ)
                    .map(|iz| (0..N_GROUPS).map(|g| m.get(iz, g)).collect())
                    .collect()
            }
        };
        let gold = &golden[&mt];
        let dev_sigma = if mt == 18 {
            &mut dev.sigma_mt18
        } else {
            &mut dev.sigma
        };
        let mut worst_inf = 0.0f64;
        let mut worst_1b = 0.0f64;
        for iz in 0..N_SIGZ {
            let phi = flux_set.flux(iz).unwrap();
            for g in 0..N_GROUPS {
                let integ = group_integral(&sig_rx, phi, BOUNDS[g], BOUNDS[g + 1]);
                // Without a URR table the assembled path and the explicit path
                // must be the same number (with one, the engine shields sigma_rx).
                if urr.is_none() {
                    assert!(
                        (sigma_g[iz][g] - integ.average()).abs() <= 1e-12 * integ.average().abs(),
                        "self_shielded_group_xs and group_integral disagree"
                    );
                }
                dev_sigma.update(mt, g + 1, iz, sigma_g[iz][g], gold.sigma[g][iz]);
                dev.flux.update(mt, g + 1, iz, integ.flux, gold.flux[g][iz]);
                let r = ((sigma_g[iz][g] - gold.sigma[g][iz]) / gold.sigma[g][iz]).abs();
                if iz == 0 {
                    worst_inf = worst_inf.max(r);
                }
                if iz == N_SIGZ - 1 {
                    worst_1b = worst_1b.max(r);
                }
            }
        }
        println!(
            "[{label}] MT={mt:>3}: max |dsigma/sigma| = {worst_inf:.3e} (sigma0=inf), \
             {worst_1b:.3e} (sigma0=1 b)"
        );
    }
    println!("[{label}] max sigma dev, MT=1/2/102: {}", dev.sigma);
    println!("[{label}] max sigma dev, MT=18:      {}", dev.sigma_mt18);
    println!("[{label}] max group-flux dev:        {}", dev.flux);
    dev
}

/// **Tier 1 — engine isolation.** Pointwise input = NJOY's own PENDF.
///
/// Methodology: read MF=3 MT=1/2/18/102 from the NJOY 293.6 K PENDF with the
/// ported `getsig` feeder ([`read_pendf_cross_section`]), tabulate the
/// Bondarenko flux on NJOY's greedy grid, and group-average with the ported
/// `panel` rule. Pass criterion: every P0 group flux and every `sigma_g` (all
/// four reactions) within **1e-5** relative of the GENDF — the prediction.
///
/// Result (2026-09-10): worst `sigma_g` 2.65e-6 (MT=102, group 9, σ0 = ∞),
/// worst flux 4.93e-7 (MT=1, group 10, σ0 = 1 b); see the module doc table.
#[test]
fn njoy_pendf_isolates_the_group_averaging_engine() {
    let Some(golden_path) = reference_file_or_skip("gendf", GOLDEN_GENDF, "groupr-u238-golden")
    else {
        return;
    };
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!(
            "[groupr-u238-golden] SKIP tier 1: set {NJOY_PENDF_ENV} to the NJOY 293.6 K PENDF \
             (regenerate with the committed deck)"
        );
        return;
    };
    let golden = load_golden(&golden_path);
    let tape = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let mut pointwise = BTreeMap::new();
    for (mt, _) in MTS {
        let xs = read_pendf_cross_section(&tape, MAT, mt).expect("PENDF MF=3 section");
        let PointwiseXs::LinLin(pairs) = xs.xs else {
            panic!("PENDF feeder returned a non-tabulated cross section");
        };
        pointwise.insert(mt, (*pairs).clone());
    }
    let dev = compare("tier1 njoy-pendf", &pointwise, &golden, None, None);
    assert!(
        dev.sigma.rel < TIER1_TOL,
        "sigma_g deviates from NJOY: {}",
        dev.sigma
    );
    assert!(
        dev.sigma_mt18.rel < TIER1_TOL,
        "sigma_g (MT=18) deviates from NJOY: {}",
        dev.sigma_mt18
    );
    assert!(
        dev.flux.rel < TIER1_TOL,
        "group flux deviates from NJOY: {}",
        dev.flux
    );
}

/// **Tier 2 — end to end from the committed ENDF tape.** Pointwise input =
/// the crate's own RECONR (`tolerance = 0.001`) + BROADR (293.6 K).
///
/// Methodology: as tier 1, but the pointwise cross sections come from the Rust
/// RECONR/BROADR, so the comparison folds in reconstruction-grid differences.
/// Pass criteria: MT=1/2/102 `sigma_g` and every group flux within **3e-3**
/// relative (prediction: O(1e-3)); MT=18 within **3e-2** (NJOY's `errmax`
/// slack on sub-threshold fission — module doc).
///
/// Result (2026-09-10): worst `sigma_g` 9.24e-4 for MT=1/2/102 (MT=102,
/// σ0 = 1 b), 1.14e-2 for MT=18 (group 13, σ0 = 1 b), worst flux 6.06e-5.
#[test]
fn crate_reconr_broadr_vs_njoy_gendf() {
    let Some(golden_path) = reference_file_or_skip("gendf", GOLDEN_GENDF, "groupr-u238-golden")
    else {
        return;
    };
    let Some(endf_path) = reference_endf_or_skip("n-092_U_238.endf", "groupr-u238-golden") else {
        return;
    };
    let golden = load_golden(&golden_path);
    let tape = Tape::read_file(&endf_path).expect("U-238 ENDF tape parses");
    let config = ReconrConfig {
        mat: MAT,
        tolerance: 0.001,
        temperature: 0.0,
    };
    let t0 = std::time::Instant::now();
    let recon = reconr(&tape, &config).expect("RECONR U-238");
    let broadened = broaden_result(&recon, TEMP_K);
    println!(
        "[tier2] RECONR + BROADR(293.6 K): {} sections in {:.1?}",
        broadened.sections.len(),
        t0.elapsed()
    );
    let mut pointwise = BTreeMap::new();
    for (mt, _) in MTS {
        let sec = broadened
            .sections
            .iter()
            .find(|s| i32::from(s.mt) == mt)
            .unwrap_or_else(|| panic!("MT={mt} reconstructed"));
        pointwise.insert(mt, sec.pairs.clone());
    }
    let dev = compare("tier2 crate-pendf", &pointwise, &golden, None, None);
    assert!(
        dev.sigma.rel < TIER2_TOL,
        "sigma_g deviates from NJOY: {}",
        dev.sigma
    );
    assert!(
        dev.sigma_mt18.rel < TIER2_TOL_MT18,
        "sigma_g (MT=18) deviates from NJOY: {}",
        dev.sigma_mt18
    );
    assert!(
        dev.flux.rel < TIER2_TOL,
        "group flux deviates from NJOY: {}",
        dev.flux
    );
}

/// **Tier 3 — URR self-shielding through MT=152.** Pointwise input = NJOY's
/// own PENDF *after UNRESR* (the deck in
/// `reference-data/gendf/u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.njoy-input`
/// adds `unresr 20 22 24 / 9237 1 6 1 / 293.6 / <sigz> / 0 /` before GROUPR),
/// golden = the GENDF GROUPR wrote from that PENDF, so `stounr` found the
/// MF=2/MT=152 table and `getunr` shielded both the flux's total and the
/// reaction cross sections inside the unresolved range (20 keV – 149 keV for
/// U-238: groups 22, 23 and the bottom of 24).
///
/// Methodology: as tier 1, plus [`read_urr_from_tape`] on the same PENDF feeds
/// [`self_shielded_group_xs`] its `urr` table. Pass criterion: every `sigma_g`
/// and group flux within **1e-5** relative of the GENDF.
///
/// Result (2026-09-10): before the `genflx_bondarenko_urr` port the group flux
/// was 6.78 % low in group 22 at σ0 = 1 b; after it, worst `sigma_g` 2.65e-6
/// and worst flux 4.93e-7, both in resolved-range groups (module doc).
#[test]
fn njoy_unresr_pendf_urr_self_shielding() {
    let Some(golden_path) =
        reference_file_or_skip("gendf", GOLDEN_GENDF_URR, "groupr-u238-golden-urr")
    else {
        return;
    };
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_URR_ENV) else {
        println!(
            "[groupr-u238-golden-urr] SKIP tier 3: set {NJOY_PENDF_URR_ENV} to the NJOY 293.6 K \
             PENDF written by UNRESR (regenerate with the committed deck)"
        );
        return;
    };
    let golden = load_golden(&golden_path);
    let tape = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let urr = read_urr_from_tape(Some(&tape), MAT, 1, TEMP_K)
        .expect("PENDF MF=2/MT=152 decodes")
        .expect("UNRESR wrote MF=2/MT=152 to this PENDF");
    let mut pointwise = BTreeMap::new();
    for (mt, _) in MTS {
        let xs = read_pendf_cross_section(&tape, MAT, mt).expect("PENDF MF=3 section");
        let PointwiseXs::LinLin(pairs) = xs.xs else {
            panic!("PENDF feeder returned a non-tabulated cross section");
        };
        pointwise.insert(mt, (*pairs).clone());
    }
    let dev = compare(
        "tier3 njoy-unresr-pendf",
        &pointwise,
        &golden,
        Some(&urr),
        None,
    );
    assert!(
        dev.sigma.rel < TIER1_TOL,
        "sigma_g deviates from NJOY: {}",
        dev.sigma
    );
    assert!(
        dev.sigma_mt18.rel < TIER1_TOL,
        "sigma_g (MT=18) deviates from NJOY: {}",
        dev.sigma_mt18
    );
    assert!(
        dev.flux.rel < TIER1_TOL,
        "group flux deviates from NJOY: {}",
        dev.flux
    );
}

/// **Tier 4 — the flux calculator (`iwt = -3`).** Pointwise input = NJOY's
/// own PENDF (the tier-1 tape), golden = the GENDF from the same deck with
/// `iwt = -3` and card 8a `fehi = 1e4 eV, sigpot = 11.29 b, nflmax = 300000`
/// (deck `…-iwt-3-fehi1e4-6sigz.njoy-input`): `genflx` solves the integral
/// slowing-down equation from `felo = egn(1)` to `fehi` on the total-xs grid,
/// extends it with the weight shape below `felo` on `getwtf`'s 1 % ladder and
/// with the narrow-resonance flux above `fehi` (`groupr.f90:5396-5665`).
///
/// Methodology: [`genflx_slowing_down`] with the same parameters (the absorber
/// `awr` is the PENDF's own, 236.0058), then the plain flux-weighted group
/// average with that flux. Pass criterion: every group flux and `sigma_g`
/// within **1e-5** relative of the GENDF.
///
/// Result (2026-09-10): after the three `genflx_slowing_down` fixes the
/// module doc describes, worst `sigma_g` 2.87e-6 and worst flux 3.89e-7; the
/// flux table reproduces NJOY's 926 / 99,934 / 55,488(+1) point counts.
#[test]
fn njoy_flux_calculator_slowing_down() {
    let Some(golden_path) =
        reference_file_or_skip("gendf", GOLDEN_GENDF_SD, "groupr-u238-golden-sd")
    else {
        return;
    };
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!(
            "[groupr-u238-golden-sd] SKIP tier 4: set {NJOY_PENDF_ENV} to the NJOY 293.6 K PENDF \
             (regenerate with the committed deck)"
        );
        return;
    };
    let golden = load_golden(&golden_path);
    let tape = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let mut pointwise = BTreeMap::new();
    let mut awr = 0.0;
    for (mt, _) in MTS {
        let xs = read_pendf_cross_section(&tape, MAT, mt).expect("PENDF MF=3 section");
        awr = xs.awr;
        let PointwiseXs::LinLin(pairs) = xs.xs else {
            panic!("PENDF feeder returned a non-tabulated cross section");
        };
        pointwise.insert(mt, (*pairs).clone());
    }
    let sig_t = PointwiseXs::LinLin(Arc::new(pointwise[&1].clone()));
    let sig_el = PointwiseXs::LinLin(Arc::new(pointwise[&2].clone()));
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    let params = SlowingDownParams {
        felo: BOUNDS[0],
        fehi: SD_FEHI,
        nflmax: SD_NFLMAX,
        sigpot: SD_SIGPOT,
        absorber_awr: awr,
        alpha2: 0.0,
        alpha3: 0.0,
        beta: 0.0,
        sam: 0.0,
        gamma: 0.0,
    };
    let flux_set = genflx_slowing_down(&sig_t, &sig_el, &weight, &SIGZ, &params).unwrap();
    if let GroupFlux::Tabulated(t) = flux_set.flux(0).unwrap() {
        let n_tail = t.iter().take_while(|p| p.0 < 0.1).count();
        let n_solved = t.iter().filter(|p| p.0 >= 0.1 && p.0 <= SD_FEHI).count();
        println!(
            "[tier4 flux-calc] flux table {} points: {n_tail} tail (< 0.1 eV), {n_solved} solved \
             (0.1 eV..fehi), {} narrow-resonance above fehi",
            t.len(),
            t.len() - n_tail - n_solved
        );
    }
    let dev = compare(
        "tier4 flux-calc",
        &pointwise,
        &golden,
        None,
        Some(&flux_set),
    );
    assert!(
        dev.sigma.rel < TIER1_TOL,
        "sigma_g deviates from NJOY: {}",
        dev.sigma
    );
    assert!(
        dev.sigma_mt18.rel < TIER1_TOL,
        "sigma_g (MT=18) deviates from NJOY: {}",
        dev.sigma_mt18
    );
    assert!(
        dev.flux.rel < TIER1_TOL,
        "group flux deviates from NJOY: {}",
        dev.flux
    );
}
