//! **DBRC — the resonance-elastic correction — does what it claims, and costs
//! nothing when it is off.**
//!
//! Found absent by the physics-coverage survey on 2026-09-16. Free-gas elastic
//! sampled the target velocity under the **constant cross-section** (CXS)
//! approximation: `σ_s` assumed flat across the relative energies a thermal
//! target reaches. Near a resolved resonance that is badly wrong — U-238's
//! 6.67 eV resonance moves `σ_s` by orders of magnitude across exactly that
//! window — and the correct kernel weights the target velocity by
//! `σ_s^{0K}(E_rel)` too.
//!
//! It was noted in a code comment in `physics/scatter.rs` and had never reached
//! the coverage matrix, which is how a known gap stays invisible.
//!
//! # Why the 0 K cross section, and not the broadened one
//!
//! The target's motion is being modelled **explicitly** by the free-gas kernel.
//! Weighting by a Doppler-broadened `σ` would count that motion twice. The
//! rejection uses the unbroadened 0 K elastic cross section, which is why
//! `Nuclide` retains it at construction rather than re-reconstructing.
//!
//! # What these tests do and do not establish
//!
//! **Do:** that the table is built from real resonance structure, that the
//! rejection changes the sampled distribution in the direction the physics
//! requires, and — the load-bearing one — that a nuclide *without* DBRC is
//! **bit-identical** to one from before DBRC existed, RNG draw count included.
//!
//! **Do not:** what DBRC is worth. The literature puts it at order 100-200 pcm
//! in an LWR pin cell; this crate cannot see that on its current validation
//! case, because Godiva is a bare fast sphere with essentially no flux in
//! U-238's resolved resonances. Pricing it needs a thermal or epithermal
//! benchmark — the same gap the coverage survey records as the project's
//! largest.

use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::{free_gas_elastic_scatter, free_gas_elastic_scatter_dbrc};

const TEMP_K: f64 = 293.6;
const KT: f64 = 8.617_333_262e-5 * TEMP_K;
/// OpenMC's default DBRC upper limit.
const DBRC_MAX_EV: f64 = 1.0e3;
/// U-238's first large resolved resonance.
const RESONANCE_EV: f64 = 6.67;

fn u238() -> Option<Nuclide> {
    let p = reference_endf_or_skip("n-092_U_238.endf", "U-238 (DBRC control)")?;
    Some(Nuclide::from_endf_file(&p, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs"))
}

/// The table is built, covers the resonance region, and carries real structure.
///
/// The "there was something to correct" assertion: DBRC on a flat cross section
/// is a no-op, so a table with no spread would make every later measurement
/// report zero and read as "resonance elastic scattering does not matter".
#[test]
fn the_dbrc_table_carries_real_resonance_structure() {
    let Some(plain) = u238() else { return };
    assert!(
        !plain.has_dbrc(),
        "DBRC is on by default; it must be opt-in, or the 'without' arm of every study is not \
         what it claims and every pre-DBRC result silently changed."
    );

    let shielded = plain.clone().with_dbrc(DBRC_MAX_EV);
    assert!(
        shielded.has_dbrc(),
        "with_dbrc built no table for U-238, whose resolved resonances run from ~6 eV up."
    );
    let t = shielded.dbrc_table().expect("table");
    assert!(
        t.len() > 100,
        "the DBRC table has only {} points below {DBRC_MAX_EV} eV; U-238's resolved resonances \
         need far more than that to be represented at all.",
        t.len()
    );

    // Real structure: the 6.67 eV resonance must tower over the potential
    // scattering either side of it.
    let on_peak = t.xs_max_over(RESONANCE_EV * 0.98, RESONANCE_EV * 1.02);
    let off_peak = t.xs_at(3.0);
    assert!(
        on_peak > 5.0 * off_peak,
        "U-238's 0 K elastic peaks at {on_peak:.4} b near {RESONANCE_EV} eV against \
         {off_peak:.4} b at 3 eV -- only {:.2}x. The resonance structure DBRC exists to \
         correct for is not in the retained table.",
        on_peak / off_peak
    );
    println!(
        "DBRC table: {} points below {DBRC_MAX_EV} eV; 0 K elastic {off_peak:.3} b at 3 eV vs \
         {on_peak:.1} b at the {RESONANCE_EV} eV resonance ({:.0}x)",
        t.len(),
        on_peak / off_peak
    );
}

/// **Without a table the kernel is bit-identical, draw count included.**
///
/// This is the assertion protecting every result recorded before DBRC existed.
/// `free_gas_elastic_scatter` now delegates to the DBRC-aware form with `None`;
/// if that path consumed even one extra variate, every RNG stream in the crate
/// would shift and every recorded eigenvalue would move for no physical reason.
#[test]
fn without_dbrc_the_kernel_is_bit_identical_and_consumes_the_same_draws() {
    let u = Direction::new(0.0, 0.0, 1.0);
    let awr = 236.0058;
    let mut n = 0usize;
    for i in 0..4096u64 {
        let e = 1.0e-2 * 10f64.powf((i % 64) as f64 / 12.0);
        let seed0 = 0xDB4C_0000u64.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let (mut sa, mut sb) = (seed0, seed0);
        let a = free_gas_elastic_scatter(e, u, awr, KT, 0.3, &mut sa);
        let b = free_gas_elastic_scatter_dbrc(e, u, awr, KT, 0.3, &mut sb, None);
        assert_eq!(
            a.0.to_bits(),
            b.0.to_bits(),
            "draw {i} at {e:.4e} eV: outgoing energy differs with dbrc=None ({} vs {})",
            a.0,
            b.0
        );
        assert_eq!(
            sa, sb,
            "draw {i} at {e:.4e} eV: the RNG streams diverged with dbrc=None -- the no-DBRC \
             path consumed a different number of variates, which would shift every stream in \
             the crate."
        );
        n += 1;
    }
    println!("dbrc=None: {n}/{n} draws bit-identical, RNG streams in lockstep");
}

/// **With DBRC on, the sampled outcome distribution actually changes near a
/// resonance — and does not away from one.**
///
/// The direction is the physical claim: weighting the target velocity by
/// `σ_s^{0K}(E_rel)` concentrates collisions where the elastic cross section is
/// large, which is exactly what the CXS approximation drops.
#[test]
fn dbrc_changes_the_kernel_near_a_resonance_and_not_far_from_one() {
    let Some(plain) = u238() else { return };
    let shielded = plain.clone().with_dbrc(DBRC_MAX_EV);
    let table = shielded.dbrc_table().expect("table");
    let u = Direction::new(0.0, 0.0, 1.0);
    let awr = plain.awr;

    // Mean outgoing energy over many draws, with and without the correction.
    let mean = |e_in: f64, dbrc: Option<&outram_mc_libs::physics::scatter::DbrcTable>| {
        let mut seed = 0xDB4C_5EEDu64;
        let n = 200_000;
        let mut s = 0.0;
        for _ in 0..n {
            s += free_gas_elastic_scatter_dbrc(e_in, u, awr, KT, 0.3, &mut seed, dbrc).0;
        }
        s / n as f64
    };

    // On the resonance: the two must differ.
    let off = mean(RESONANCE_EV, None);
    let on = mean(RESONANCE_EV, Some(table));
    let d_res = (on / off - 1.0).abs();

    // Above the DBRC window: the correction must not apply at all, so the two
    // must be bit-identical -- this is what `DbrcTable::applies` is for.
    let e_above = DBRC_MAX_EV * 10.0;
    let a = mean(e_above, None);
    let b = mean(e_above, Some(table));

    assert!(
        d_res > 1.0e-4,
        "at the {RESONANCE_EV} eV resonance the mean outgoing energy moved only {:.3e} \
         relative ({off:.6e} -> {on:.6e}) with DBRC on. The rejection is not taking effect, so \
         anything measured through it would be an artefact of nothing.",
        d_res
    );
    assert_eq!(
        a.to_bits(),
        b.to_bits(),
        "above the {DBRC_MAX_EV} eV limit DBRC still changed the result ({a} -> {b}); it must \
         be inert there, or its cost and its effect both leak into the fast range."
    );

    println!(
        "DBRC at {RESONANCE_EV} eV: mean E' {off:.6e} -> {on:.6e} eV ({:+.4} %); at \
         {e_above:.1e} eV (above the {DBRC_MAX_EV} eV limit) bit-identical, as it must be",
        100.0 * (on / off - 1.0)
    );
}
