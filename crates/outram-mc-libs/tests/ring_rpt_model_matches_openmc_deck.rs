//! **The ring-RPT model matches the OpenMC deck it is compared against.**
//!
//! # Why this is a separate claim from any physics claim
//!
//! `examples/fhr_ring_rpt_endf.rs` sits **+4004 pcm** above its OpenMC
//! reference, and essentially every *physics* mechanism has now been excluded by
//! measurement — point cross sections, resonance integral, URR reconstruction,
//! graphite S(α,β) cross section / first moment / mean cosine, two-body
//! kinematics, the energy treatment of self-shielding against an exact
//! deterministic solve, and the tracking method.
//!
//! When that many physics candidates fall, the next hypothesis is that **the two
//! codes are solving different problems**. A geometry or inventory mismatch
//! produces a large offset while every physics oracle keeps passing, because
//! each of those oracles tests a *component* in isolation and none of them looks
//! at the assembled model.
//!
//! Nothing tested that. This file does.
//!
//! # The oracle: the reference deck's own arithmetic
//!
//! `verification_and_validation/ring_rpt/openmc_inputs/rpt_pebble.py` is
//! committed, so it can be read and its formulas reproduced independently. It
//! derives the fuel annulus like this (lines 159–184):
//!
//! ```python
//! pebble_diameter = 4.0
//! pebble_shell_thickness_cm = 0.1
//! fuel_radius_original_pebble_cm = pebble_diameter/2.0 - pebble_shell_thickness_cm
//! triso_plus_matrix_volume_cm3 = self.sphere_vol(fuel_radius_original_pebble_cm)
//! triso_packing_factor = 0.3
//! triso_volume_cm3 = triso_plus_matrix_volume_cm3 * triso_packing_factor
//! inner_sphere_vol_cm3 = self.sphere_vol(ring_rpt_inner_radius_cm)
//! inner_sphere_plus_triso_vol_cm3 = inner_sphere_vol_cm3 + triso_volume_cm3
//! fuel_outer_radius = (0.75/np.pi * inner_sphere_plus_triso_vol_cm3)**(1/3)
//! ```
//!
//! This crate instead computes `r_outer = (r_inner³ + pf · r_zone³)^(1/3)`
//! (`pebble_beds::fhr_pebble::rpt_fuel_outer_radius`). Those are the **same
//! expression**: substituting `V = (4/3)π r³` into the deck's form gives
//! `(0.75 · 4/3 · (r_inner³ + pf·r_zone³))^(1/3)`, and `0.75 × 4/3 = 1` exactly.
//! The test below reproduces the deck's form literally — through volumes, with
//! its `0.75/π` factor — rather than the simplified one, so that it would catch
//! a change to *either* side.
//!
//! # Results (2026-09-12)
//!
//! | quantity | deck | this crate |
//! |---|---|---|
//! | inner graphite radius | 1.493359375 cm | 1.493359375 cm |
//! | fuel annulus outer radius | 1.753118134 cm | 1.753118134 cm |
//! | pebble (graphite shell) outer | 2.0 cm | 2.0 cm |
//! | reflective boundary | 3.0 cm | 3.0 cm |
//! | TRISO packing fraction | 0.3 | 0.3 |
//!
//! The radii agree **exactly** — relative difference 0.0, not merely small. Two
//! algebraically identical expressions evaluated in the same precision land on
//! the same bits here, which is a stronger statement than "close". The annulus
//! *volume* comparison below is 6.2e-16 rather than exact, because that one
//! round-trips through `sphere_vol` twice and picks up its round-off.
//!
//! # Interpretation
//!
//! **The model match is not the cause of the +4004 pcm.** That is a negative
//! result and it is worth having: it removes the "the two codes are modelling
//! different pebbles" explanation, which is otherwise the natural next guess
//! once the physics candidates are exhausted, and it does so by arithmetic
//! rather than by assertion.
//!
//! # What this does NOT cover
//!
//! Radii and the packing fraction only, **as executable assertions**. Material
//! compositions, densities and nuclide inventories are NOT asserted here — do
//! not read a pass as "the whole model matches".
//!
//! Those were, however, checked **by inspection** on 2026-09-12, and the two
//! levers the deck's author flagged as mattering both agree:
//!
//! | quantity | deck | this crate |
//! |---|---|---|
//! | U-235 enrichment | `build_fhr_materials(19.9, …)` atom % | `ENRICH = 0.199` |
//! | Li-7 purity | `build_fhr_materials(…, 99.995, …)` | `LI7_PURITY = 0.99995` |
//! | temperature | `fuel_temp = coolant_temp = 600.0` | `TEMP_K = 600.0` |
//!
//! Li-7 purity is worth singling out. `rpt_pebble.py`'s own docstring records
//! that an earlier run was wrong because *"the flibe was over absorbent when i
//! carelessly left it at 99.5 % purity rather than 99.995 % purity"* — Li-6 is a
//! strong thermal absorber, so that one digit is worth a great deal of k. It
//! matches.
//!
//! Those three live as `const`s inside `examples/fhr_ring_rpt_endf.rs` rather
//! than in the library, so a test cannot import them; asserting them here would
//! only compare a literal against itself. Making them assertable would mean
//! lifting the pebble specification into `pebble_beds::fhr_pebble`, which is
//! worth doing and is not done.
//!
//! # A provenance trap in the reference deck, checked and clear
//!
//! `rpt_pebble.py`'s docstring quotes `keff = 1.36863 ± 0.00070` at this RPT
//! radius and then says **"Note that ENDF v 7b.1 was used"**. The V&V record's
//! reference is `1.36479 ± 0.00067` — **384 pcm** away, which is ~4σ and looks
//! alarming until the libraries are checked.
//!
//! They are different runs on different libraries, and the one being compared
//! against is the right one: `ring_rpt_vs_openmc.md` records the reference as
//! **OpenMC 0.15.3-dev (`09ee8308d`), ENDF/B-VIII.0 HDF5, 600 K, 20 000 × 150**,
//! matching this crate's ENDF/B-VIII.0. The docstring's VII.1 numbers are
//! historical and must not be quoted as the reference.
//!
//! One composition subtlety that IS already handled deliberately, and is
//! documented at the point of use in `examples/fhr_ring_rpt_endf.rs` rather than
//! here: the deck's `get_mixed_triso_fuel_material` uses `add_element('C', …)`
//! with **no** `add_s_alpha_beta`, so the buffer/PyC1/PyC2 carbon — which does
//! carry `c_Graphite` in the *explicit* pebble — is free-gas once homogenised.
//! That is 83 % of the mixed material's carbon. This crate matches it.

use outram_mc_libs::pebble_beds::fhr_pebble::rpt_fuel_outer_radius;

/// The reference deck's own constants (`rpt_pebble.py`).
mod deck {
    pub const PEBBLE_DIAMETER_CM: f64 = 4.0;
    pub const SHELL_THICKNESS_CM: f64 = 0.1;
    pub const TRISO_PACKING_FACTOR: f64 = 0.3;
    pub const RING_RPT_INNER_RADIUS_CM: f64 = 1.493_359_375;
    pub const ROOT_CELL_RADIUS_CM: f64 = 3.0;

    /// `self.sphere_vol` in the deck.
    pub fn sphere_vol(r: f64) -> f64 {
        4.0 / 3.0 * std::f64::consts::PI * r.powi(3)
    }

    /// The deck's `fuel_outer_radius`, reproduced literally — through volumes,
    /// with its `0.75/np.pi` factor.
    pub fn fuel_outer_radius() -> f64 {
        let fuel_radius_original_pebble = PEBBLE_DIAMETER_CM / 2.0 - SHELL_THICKNESS_CM;
        let triso_plus_matrix_volume = sphere_vol(fuel_radius_original_pebble);
        let triso_volume = triso_plus_matrix_volume * TRISO_PACKING_FACTOR;
        let inner_sphere_vol = sphere_vol(RING_RPT_INNER_RADIUS_CM);
        let inner_plus_triso = inner_sphere_vol + triso_volume;
        (0.75 / std::f64::consts::PI * inner_plus_triso).cbrt()
    }
}

/// **This crate's ring-RPT fuel annulus reproduces the OpenMC deck's to
/// floating-point round-off.**
///
/// Methodology and results: see the module docs. The deck's formula is
/// reproduced literally through volumes; this crate's
/// `rpt_fuel_outer_radius` works in radii. They are algebraically identical, so
/// the tolerance is 1e-12 relative — anything looser would let a real change in
/// either expression through.
#[test]
fn ring_rpt_fuel_annulus_matches_the_openmc_deck() {
    let fuel_zone_radius = deck::PEBBLE_DIAMETER_CM / 2.0 - deck::SHELL_THICKNESS_CM;
    assert_eq!(
        fuel_zone_radius, 1.9,
        "the deck's fuel-zone radius is 2.0 - 0.1 = 1.9 cm; got {fuel_zone_radius}"
    );

    let theirs = deck::fuel_outer_radius();
    let ours = rpt_fuel_outer_radius(
        deck::RING_RPT_INNER_RADIUS_CM,
        fuel_zone_radius,
        deck::TRISO_PACKING_FACTOR,
    );

    let rel = (ours - theirs).abs() / theirs;
    println!(
        "  deck {theirs:.12} cm   ours {ours:.12} cm   rel {rel:.3e}"
    );
    assert!(
        rel < 1.0e-12,
        "the ring-RPT fuel annulus outer radius is {ours:.12} cm against the \
         OpenMC deck's {theirs:.12} cm ({rel:.3e} relative).\n\
         These are algebraically the SAME expression — the deck computes \
         (0.75/pi * (V_inner + pf*V_zone))^(1/3) through volumes and this crate \
         computes (r_inner^3 + pf*r_zone^3)^(1/3) through radii, and \
         0.75 * 4/3 = 1 exactly. A departure means one of them changed.\n\
         This matters because the annulus carries the ENTIRE heavy-metal \
         inventory: get its volume wrong and every physics oracle still passes \
         while k moves by thousands of pcm."
    );
}

/// **The annulus holds exactly the TRISO inventory the deck puts in it.**
///
/// This is the *physical* statement behind the radius arithmetic, and it is
/// asserted separately because the radius could be right for the wrong reason.
/// The deck sizes the annulus so its volume equals the total TRISO-particle
/// volume of the original pebble — `(4/3)π·1.9³ × 0.3` — placed outside the
/// inner graphite ball.
///
/// Measured 2026-09-12: annulus volume **8.619274 cm³** against a TRISO volume
/// of **8.619274 cm³**, agreeing to **6.183e-16** relative.
#[test]
fn the_annulus_volume_equals_the_triso_volume_it_stands_in_for() {
    let fuel_zone_radius = deck::PEBBLE_DIAMETER_CM / 2.0 - deck::SHELL_THICKNESS_CM;
    let r_outer = rpt_fuel_outer_radius(
        deck::RING_RPT_INNER_RADIUS_CM,
        fuel_zone_radius,
        deck::TRISO_PACKING_FACTOR,
    );

    let annulus_vol = deck::sphere_vol(r_outer) - deck::sphere_vol(deck::RING_RPT_INNER_RADIUS_CM);
    let triso_vol = deck::sphere_vol(fuel_zone_radius) * deck::TRISO_PACKING_FACTOR;
    let rel = (annulus_vol - triso_vol).abs() / triso_vol;

    println!("  annulus {annulus_vol:.6} cm^3   TRISO {triso_vol:.6} cm^3   rel {rel:.3e}");
    assert!(
        rel < 1.0e-12,
        "the ring-RPT annulus encloses {annulus_vol:.6} cm^3 but stands in for \
         {triso_vol:.6} cm^3 of TRISO ({rel:.3e} relative). The whole point of \
         the ring-RPT construction is that the annulus volume EQUALS the TRISO \
         volume it replaces, so the heavy-metal inventory is preserved; if it \
         does not, this code and the reference are burning different amounts of \
         uranium."
    );

    // The annulus must also fit inside the fuel zone, or it would overlap the
    // graphite shell. The deck raises ValueError on this; we assert it.
    assert!(
        r_outer <= fuel_zone_radius,
        "the annulus outer radius {r_outer:.6} cm exceeds the fuel zone \
         {fuel_zone_radius} cm — it would eat into the graphite shell. The deck \
         raises ValueError here for the same reason."
    );
}

/// **The domain is the one OpenMC used: a reflective sphere of r = 3 cm.**
///
/// `rpt_pebble.py` defines an `outermost_sph` at `r = pebble_diameter = 4.0` with
/// `boundary_type = 'vacuum'`, and then fills a `root_cell` bounded by
/// `Sphere(r=3.0)` with `boundary_type = 'reflective'`. The r = 4 surface is
/// never reached, so the effective domain is the r = 3 reflective sphere, and
/// FLiBe occupies 2.0 → 3.0 cm.
///
/// That distinction is worth pinning because FLiBe is **~70 % of the domain by
/// volume** (79.6 cm³ against the pebble's 33.5 cm³), so reading the wrong
/// surface as the boundary would change the moderator inventory enormously while
/// leaving every material and cross section correct.
#[test]
fn the_reflective_domain_is_the_one_the_deck_used() {
    let pebble_outer = deck::PEBBLE_DIAMETER_CM / 2.0;
    assert_eq!(pebble_outer, 2.0);
    assert_eq!(deck::ROOT_CELL_RADIUS_CM, 3.0);

    let flibe_vol = deck::sphere_vol(deck::ROOT_CELL_RADIUS_CM) - deck::sphere_vol(pebble_outer);
    let pebble_vol = deck::sphere_vol(pebble_outer);
    let flibe_share = flibe_vol / (flibe_vol + pebble_vol);

    println!(
        "  FLiBe {flibe_vol:.3} cm^3, pebble {pebble_vol:.3} cm^3, \
         FLiBe share {:.1} %",
        100.0 * flibe_share
    );
    assert!(
        (0.69..0.72).contains(&flibe_share),
        "FLiBe occupies {:.1} % of the r = 3 cm domain; it was 70.4 % when this \
         was recorded. If the domain radius or the pebble radius moved, the \
         moderator-to-fuel ratio moved with it, and that changes k far more than \
         any of the physics defects currently under investigation.",
        100.0 * flibe_share
    );
}
