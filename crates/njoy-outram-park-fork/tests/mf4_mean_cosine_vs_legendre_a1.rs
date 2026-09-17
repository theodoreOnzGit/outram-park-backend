//! **`EnergyAngular::mean_cosine` against the evaluation's own `a1` — an exact
//! oracle, with no quadrature and no linearisation on either side.**
//!
//! # Why this oracle is the right one
//!
//! For an MF=4 `LTT=1` section the angular density is
//! `f(mu) = sum_l ((2l+1)/2) a_l P_l(mu)` with `a_0 = 1`. Orthogonality makes the
//! mean cosine **exactly `a_1`** — one number read straight off the tape, needing
//! no integration and no grid.
//!
//! Every other check of `<mu>` in this workspace goes through a *linearised*
//! cosine table and a quadrature rule, so it can only be as good as both. This
//! one has neither, which is why it caught what they could not.
//!
//! # What it caught (2026-09-16)
//!
//! `EnergyAngular::mean_cosine` integrated `mu*f(mu)` by the **trapezoid rule**,
//! under a comment asserting that was exact because `f` is linear. `f` linear
//! makes `mu*f(mu)` **quadratic**, and the trapezoid rule is exact only for a
//! linear integrand. Measured on U-235 MT=2 (ENDF/B-VIII.0):
//!
//! | E (eV) | grid points | `a1` (exact) | closed form | trapezoid |
//! |---|---|---|---|---|
//! | 1.0e3 | 9 | +0.001195 | **+0.001195** | +0.001232 |
//! | 1.0e5 | 9 | +0.126123 | **+0.126091** | +0.130067 |
//! | 2.0e6 | 92 | +0.621682 | **+0.622143** | +0.622697 |
//!
//! At 1.0e5 eV — a 9-point grid — the trapezoid rule is **123 times** further
//! from the truth than the closed form. The error scales as the cube of the grid
//! spacing and does not cancel across segments for a monotone `f`, which a
//! forward-peaked angular distribution is over most of its range.
//!
//! **It had propagated into a V&V reference.** `outram-mc-libs`'s
//! `tests/elastic_mubar_vs_openmc.rs` compares against OpenMC `<mu>` values whose
//! provenance note says they are *"the trapezoidal integral of mu*p(mu) over the
//! stored cosine grid"*. Its committed `0.13007` at 1.0e5 eV reproduces **our
//! trapezoid** to 3e-6 while the true value is `0.12612`. That test was passing
//! on two matching errors, which is the failure mode that check exists to
//! prevent — and it only surfaced because fixing the library made the test fail.
//!
//! # Methodology
//!
//! Parse MF=4 for U-235 and U-238 MT=2 (elastic) and their discrete inelastic
//! levels, walk the Legendre rows, and for every incident energy that is also a
//! grid point of the parsed `ElasticAngular`, compare `mean_cosine()` against
//! that row's `a_1`. Only `LTT=1`/`LTT=3` rows are usable, since `a_1` exists
//! only where the evaluation stored coefficients.
//!
//! Two pass criteria, both set from the measurement and both chosen so that a
//! regression to the trapezoid rule would trip them:
//!
//! - **below 6 MeV: `2e-3`.** This is the band a fission spectrum actually puts
//!   flux in. The closed form's worst there is `1.2e-3`; the trapezoid's worst is
//!   `3.9e-3` (U-235 elastic at 1.0e5 eV), so the gate separates them.
//! - **whole tabulated range: `1e-2`.** Deliberately loose, because the residual
//!   above 6 MeV is the *lineariser's* error and not the integral's — see below.
//!
//! # Results (2026-09-16, ENDF/B-VIII.0 U-235 + U-238, MT=2 and MT=51..60)
//!
//! **1857 Legendre rows compared.**
//!
//! | population | rows | worst \|mean_cosine − a1\| |
//! |---|---|---|
//! | all | 1857 | 6.21e-3 |
//! | below 6 MeV | 1211 | **1.20e-3** |
//! | below 2 MeV | 852 | **1.20e-3** |
//! | rows above 1e-3 | 7 | — |
//!
//! # What the residual is, and what it is NOT
//!
//! It is **not** the positivity clamp. `legendre_cosine_law` clamps a truncated
//! series that dips below zero and renormalises, which would legitimately move
//! the mean away from `a1` — so this test evaluates the raw series at every grid
//! point itself and separates the two populations. **Zero rows are clamped.**
//! That hypothesis was checked and killed rather than assumed.
//!
//! It is the **lineariser**. `legendre_cosine_law` bisects until lin-lin
//! interpolation reproduces `f` within `ANGLE_TOL = 5e-3` *relative*, and the
//! exact mean of that interpolant is not the exact mean of the series. The
//! worst rows bear this out: U-238 MT=59 at 13 MeV has **129 grid points** and
//! still differs by 6.2e-3, so it is not a coarse grid — it is a tolerance
//! stated on `f` producing a residual in a *moment* of `f`.
//!
//! **This is a real, actionable finding and is not fixed here.** `mu_bar` enters
//! transport as `Sigma_tr = Sigma_t (1 - mu_bar)`, so 6.2e-3 at `mu_bar ≈ 0.386`
//! is a 1.0 % error in `Sigma_tr` for that level at that energy. Tightening
//! `ANGLE_TOL` would reduce it at the cost of larger tables. It is left alone
//! because the whole effect sits above 6 MeV — below it the worst is 1.2e-3 —
//! and because changing a lineariser tolerance shifts every angular table in the
//! workspace, which deserves its own paired measurement rather than a
//! drive-by.

use njoy_outram_park_fork::acer::angular::parse_mf4_angular;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

/// The raw truncated Legendre density `f(mu) = sum_l ((2l+1)/2) a_l P_l(mu)`,
/// evaluated independently of the crate so this test's discriminator does not
/// come from the code under test.
fn legendre_density(coeffs: &[f64], mu: f64) -> f64 {
    // a_0 = 1 by normalisation; `coeffs` holds a_1 onward.
    let mut p_prev = 1.0f64; // P_0
    let mut p_cur = mu; // P_1
    let mut acc = 0.5; // l = 0 term
    for (i, &a) in coeffs.iter().enumerate() {
        let l = (i + 1) as f64;
        acc += (2.0 * l + 1.0) / 2.0 * a * p_cur;
        let p_next = ((2.0 * l + 1.0) * mu * p_cur - l * p_prev) / (l + 1.0);
        p_prev = p_cur;
        p_cur = p_next;
    }
    acc
}

/// Worst tolerable `|mean_cosine - a1|` **below 6 MeV**, where a fission
/// spectrum's flux is. Measured `1.20e-3`; the trapezoid rule this replaced gave
/// `3.9e-3` on U-235 elastic at 1.0e5 eV, so this gate tells them apart.
const TOL_6MEV: f64 = 2.0e-3;

/// Worst tolerable `|mean_cosine - a1|` over the **whole** tabulated range,
/// which runs to 30 MeV. Loose on purpose: above 6 MeV the residual is the
/// lineariser's interpolation error, not the integral's. See the module docs.
const TOL_ALL: f64 = 1.0e-2;

fn check_nuclide(
    file: &str,
    label: &str,
    mts: &[i32],
) -> Option<(
    f64,
    usize,
    Vec<(f64, i32, f64, f64, usize)>,
    (f64, usize),
    (f64, usize),
)> {
    let p = reference_endf_or_skip(file, label)?;
    let tape = Tape::read_file(&p).expect("tape parses");
    let mat = tape.materials()[0];

    let mut worst = 0.0f64;
    let mut compared = 0usize;
    let mut rows: Vec<(f64, i32, f64, f64, usize)> = Vec::new();
    let (mut clean_worst, mut clamped_worst) = (0.0f64, 0.0f64);
    let (mut n_clean, mut n_clamped) = (0usize, 0usize);

    for &mt in mts {
        let Some(sec) = tape.section(mat, 4, mt) else {
            continue;
        };
        let ang = parse_mf4_angular(sec).expect("MF=4 parses");
        if ang.energies.is_empty() {
            continue;
        }

        // Walk the raw Legendre rows for a_1.
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont().expect("HEAD");
        let ltt = head.l2;
        let _trans = cur.read_cont().expect("LI/LCT");
        if ltt != 1 && ltt != 3 {
            continue; // no coefficients stored; a_1 does not exist
        }
        let tab2 = cur.read_tab2().expect("TAB2");
        for _ in 0..tab2.head.n2 {
            let list = cur.read_list().expect("LIST");
            let e_ev = list.head.c2;
            let Some(&a1) = list.data.first() else {
                continue;
            };
            let e_mev = e_ev * 1.0e-6;
            // Find the parsed table at this exact energy.
            let Some(d) = ang
                .energies
                .iter()
                .find(|d| (d.e_mev - e_mev).abs() <= 1.0e-9 * e_mev.abs().max(1.0e-12))
            else {
                continue;
            };
            if d.cosines.is_empty() {
                continue;
            }
            let got = d.mean_cosine();
            let diff = (got - a1).abs();
            if diff > worst {
                worst = diff;
            }
            // Does the truncated series go negative anywhere on this row's own
            // grid? `legendre_cosine_law` clamps it to zero and renormalises --
            // deliberately, since a density that dips below zero cannot be
            // sampled -- and that legitimately shifts the mean away from `a1`.
            // Such a row is testing the positivity fix, not the integral.
            let clamped = d
                .cosines
                .iter()
                .any(|&m| legendre_density(&list.data, m) < 0.0);
            rows.push((diff, mt, e_ev, a1, d.cosines.len()));
            if clamped {
                clamped_worst = clamped_worst.max(diff);
                n_clamped += 1;
            } else {
                clean_worst = clean_worst.max(diff);
                n_clean += 1;
            }
            compared += 1;
        }
    }
    Some((
        worst,
        compared,
        rows,
        (clean_worst, n_clean),
        (clamped_worst, n_clamped),
    ))
}

#[test]
fn mean_cosine_reproduces_the_evaluations_first_legendre_coefficient() {
    // Elastic plus a spread of discrete inelastic levels.
    let mts: Vec<i32> = std::iter::once(2).chain(51..=60).collect();

    let mut overall = 0.0f64;
    let mut overall_clean = 0.0f64;
    let mut overall_6mev = 0.0f64;
    let mut n_clean_total = 0usize;
    let mut total = 0usize;
    let mut ran = false;

    for (file, label) in [
        ("n-092_U_235-ENDF8.0.endf", "U-235 (MF=4 a1 oracle)"),
        ("n-092_U_238-ENDF8.0.endf", "U-238 (MF=4 a1 oracle)"),
    ] {
        let Some((worst, n, mut rows, clean, clamped)) = check_nuclide(file, label, &mts) else {
            continue;
        };
        ran = true;
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let over = |t: f64| rows.iter().filter(|r| r.0 > t).count();
        println!(
            "   {label}: {n} rows, worst {worst:.3e}; >1e-3: {}, >1e-4: {}",
            over(1e-3),
            over(1e-4)
        );
        println!(
            "      positivity-clamped rows: {} (worst {:.3e});  unclamped: {} (worst {:.3e})",
            clamped.1, clamped.0, clean.1, clean.0
        );
        overall_clean = overall_clean.max(clean.0);
        n_clean_total += clean.1;
        // Where a fission spectrum actually puts flux. Reporting the worst over
        // the whole tabulated range up to 30 MeV answers a question nobody asked
        // -- almost no neutron in a reactor is there.
        for band in [2.0e6f64, 6.0e6] {
            let w = rows
                .iter()
                .filter(|r| r.2 <= band)
                .map(|r| r.0)
                .fold(0.0, f64::max);
            let n = rows.iter().filter(|r| r.2 <= band).count();
            println!(
                "      below {:.0} MeV: {n} rows, worst {w:.3e}",
                band / 1.0e6
            );
            if band == 6.0e6 {
                overall_6mev = overall_6mev.max(w);
            }
        }
        for r in rows.iter().take(4) {
            println!(
                "      worst: MT={} E={:.3e} eV  a1={:+.6}  npts={}  diff={:.3e}",
                r.1, r.2, r.3, r.4, r.0
            );
        }
        overall = overall.max(worst);
        total += n;
    }
    if !ran {
        eprintln!("SKIP: no evaluations available");
        return;
    }

    println!(
        "   overall: {total} rows, worst {overall:.3e}; unclamped {n_clean_total} rows, worst \
         {overall_clean:.3e}"
    );
    assert!(
        total > 100,
        "only {total} Legendre rows compared; the oracle is barely exercised."
    );
    assert!(
        n_clean_total > 100,
        "only {n_clean_total} unclamped rows; the exact comparison is barely exercised."
    );
    println!("   worst below 6 MeV across both nuclides: {overall_6mev:.3e} (gate {TOL_6MEV:.0e})");
    assert!(
        overall_6mev <= TOL_6MEV,
        "worst |mean_cosine - a1| below 6 MeV is {overall_6mev:.3e}, above {TOL_6MEV:.0e}. That \
         is the band a fission spectrum puts flux in, and `a1` IS the exact mean of the \
         evaluation's own density, so nothing should move it there. The most likely cause is a \
         quadrature that is not exact for the quadratic integrand mu*f(mu) -- the defect fixed \
         on 2026-09-16, which gave 3.9e-3 here."
    );
    assert!(
        overall_clean <= TOL_ALL,
        "worst |mean_cosine - a1| over the whole tabulated range is {overall_clean:.3e}, above \
         {TOL_ALL:.0e}. Above 6 MeV this is the lineariser's interpolation error rather than \
         the integral's (measured 6.2e-3 at introduction, with ZERO positivity-clamped rows), \
         so a jump here means ANGLE_TOL or the bisection changed."
    );
}
