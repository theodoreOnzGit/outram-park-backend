//! The W-3 critical-heat-flux correlation and the departure-from-nucleate-
//! boiling ratio.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `w3chf.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # Method reference
//!
//! The W-3 (Tong) correlation for departure from nucleate boiling in a PWR rod
//! bundle. It is published in the open literature in **British units** —
//! pressure in psia, mass flux in lb/(hr·ft²), enthalpy in Btu/lb, equivalent
//! diameter in inches, heat flux in Btu/(hr·ft²) — and the reference has folded
//! the unit conversions into the coefficients so it can work in its own cm-g-s
//! and MPa units throughout.
//!
//! **That folding was checked term by term during translation and it is
//! correct.** The check is worth recording, because a reader comparing this
//! code against a textbook statement of W-3 will otherwise see eight
//! unexplained constants:
//!
//! | Reference constant | Published W-3 | Conversion |
//! |---|---|---|
//! | `0.06238` | `0.0004302` per psia | `x 145.038` psia/MPa |
//! | `0.01427` | `0.0000984` per psia | `x 145.038` |
//! | `0.5987` | `0.004129` per psia | `x 145.038` |
//! | `2.326` | `G/1e6` then `x 1e6` Btu/hr/ft2 | see below |
//! | `3271` | `1.037e6` Btu/(hr·ft²) | `x 3.15459e-4` W/cm² per Btu/hr/ft², then `x 10` |
//! | `124.1` per m | `3.151` per inch | `/ 0.0254` m/inch |
//! | `0.0003413` | `0.000794` per Btu/lb | `/ 2.326` kJ/kg per Btu/lb |
//!
//! The `2.326` deserves its own line because it looks like the Btu/lb to kJ/kg
//! factor and **is not** — that is a coincidence. Mass flux enters the
//! published correlation as `G/1e6` with `G` in lb/(hr·ft²), and the whole
//! bracket is later multiplied by `1e6` Btu/(hr·ft²). Carrying
//! `Gm` in g/(cm²·s) through that chain gives
//! `1 g/(cm²·s) = 7373.4 lb/(hr·ft²)`, so the mass-flux term becomes
//! `Gm x 7373.4 x 3.15459e-4 = Gm x 2.3258` W/cm². The agreement with 2.326 to
//! four figures is what confirms the whole conversion chain, and it also fixes
//! the units of every input: **pressure MPa, enthalpy kJ/kg, hydraulic
//! diameter cm, density g/cm³, velocity cm/s, and the result W/cm².**
//!
//! # Scope — this is a correlation, not a model
//!
//! W-3 is an empirical fit valid over roughly 5.5-16 MPa, mass fluxes of
//! 1.4-6.8 Mg/(m²·s), qualities from -0.15 to 0.15 and equivalent diameters of
//! 0.5-1.8 cm. **Nothing here checks any of that**, matching the reference,
//! which evaluates the fit wherever it is asked. A DNBR computed outside the
//! correlation's range is an extrapolation and should not be reported as a
//! safety margin.

use crate::fixinfnan::fixinfnan;
use crate::iapws_if97::basic::{h1_pt, hl_p};
use crate::types::{FuelGeometry, Params, Th, W3Form};

/// `chf` — the critical heat flux and the margin to it.
#[derive(Clone, Debug, Default)]
pub struct Chf {
    /// `chf.chf` — critical heat flux per node, **W/cm²**.
    pub chf: Vec<f64>,
    /// `chf.dnbr` — departure-from-nucleate-boiling ratio, `chf / heatflux`,
    /// dimensionless.
    ///
    /// Above 1 the node is in nucleate boiling with margin; at or below 1 the
    /// correlation predicts departure. A node with zero heat flux would give
    /// infinity, which [`crate::fixinfnan`] turns into `0` — see the note on
    /// [`w3chf`].
    pub dnbr: Vec<f64>,
}

/// `chf = w3chf(params, geometry, th)` — critical heat flux at each node by the W-3
/// correlation, and the DNBR against the actual wall heat flux.
///
/// # Arguments
///
/// - `fuel` — needs `hydia`, the subchannel hydraulic diameter in **cm**.
///   (`subarea` is read by the reference and never used; see below.)
/// - `th` — needs the coolant pressure, void fraction, mixture velocity, phase
///   densities, enthalpy and quality per node, plus the inlet temperature and
///   pressure and the wall heat flux.
///
/// # Returns
///
/// [`Chf`], both fields as long as `th.heatflux`.
///
/// # Reference defect — the upwind enthalpy is halved
///
/// The reference builds the enthalpy that enters the subcooling term as
///
/// ```text
/// enthshift(1) = enthin;
/// enthshift(i) = (0.5*enth(i) + 0.5*enth(i-1))/2;
/// ```
///
/// The second line is `(h_i + h_{i-1})/4` — an average, **halved again**. The
/// stray `/2` is almost certainly a typo for the two-point average
/// `(h_i + h_{i-1})/2`: nothing in the correlation motivates a factor of a
/// half, and the first element is set to the full inlet enthalpy rather than
/// half of it, so the two branches are inconsistent with each other.
///
/// Halving the enthalpy *raises* `hLsat - enthshift`, which raises `Kfour` and
/// so **overpredicts** the critical heat flux — a non-conservative error in the
/// direction that matters for a safety margin. Translated as written and
/// recorded as defect T1; see `docs/bedok-reference-defects.md`.
///
/// # A second deviation from published W-3, this one possibly deliberate
///
/// Published W-3 uses the **inlet** enthalpy in the subcooling term, constant
/// along the channel. The reference instead uses a per-node upwind-shifted
/// local enthalpy, which reduces to the inlet value only at the first node.
/// That may be an intentional local-conditions variant, or it may be the same
/// unfinished edit as the `/2`. The snapshot says nothing either way, so it is
/// preserved and flagged rather than repaired.
///
/// # `fixinfnan` masks a division by zero
///
/// `dnbr = chf / heatflux` is infinite wherever the heat flux is zero — every
/// unfuelled node, and every node before power is applied. The reference passes
/// the result through [`crate::fixinfnan::fixinfnan`], which substitutes `0`.
/// So **a zero DNBR means "no heat flux here", not "no margin"**, and the two
/// are indistinguishable in the output. This is the same masking defect C5
/// records against `fixinfnan`'s use after the flux solves.
///
/// # Dead reads in the reference
///
/// `gearth`, the gravitational acceleration, is assigned and never used;
/// `subarea` is read from the geometry and never used; and `ldens`/`gdens`
/// enter only through the mixture density. The three `writematrix` calls that
/// end the function are diagnostic dumps and are not reproduced — see the
/// module docs of [`crate::diffusion_solverxyz`] for why file writes are
/// returned rather than performed.
///
/// # Panics
///
/// If the per-node vectors in `th` are not all the same length.
pub fn w3chf(params: &Params, fuel: &FuelGeometry, th: &Th) -> Chf {
    let n = th.heatflux.len();
    let c = &th.coolant;
    for (name, len) in [
        ("press", c.press.len()),
        ("alphag", c.alphag.len()),
        ("vm", c.vm.len()),
        ("ldens", c.ldens.len()),
        ("gdens", c.gdens.len()),
        ("enth", c.enth.len()),
        ("quality", c.quality.len()),
    ] {
        assert_eq!(len, n, "th.coolant.{name} is {len} long, heatflux is {n}");
    }

    // `enthin = IAPWS_IF97('h1_pT', inletpress, inlett)` — the inlet state is
    // taken as compressed liquid.
    let enthin = h1_pt(c.inletpress, c.inlettemp);

    // The subcooling enthalpy for `K4`. See `crate::types::W3Form`:
    // `Published` uses the constant inlet enthalpy the correlation is written
    // with; `Reference` reproduces the snapshot's halved per-node upwind
    // average, defects T5 and T6.
    let mut enthshift = vec![enthin; n];
    if params.w3_form == W3Form::Reference {
        for i in 1..n {
            *enthshift.get_mut(i).expect("in range") =
                (0.5 * c.enth[i] + 0.5 * c.enth[i - 1]) / 2.0;
        }
    }

    let mut chf = vec![0.0; n];
    let mut dnbr = vec![0.0; n];

    for i in 0..n {
        let press = c.press[i];
        let quale = c.quality[i];
        // `hLsat = IAPWS_IF97('hL_p', th.coolant.press)` — saturated liquid
        // enthalpy at the local pressure, kJ/kg.
        let hlsat = hl_p(press);

        // Mixture mass flux, g/(cm^2 s).
        let gm = (c.alphag[i] * c.gdens[i] + (1.0 - c.alphag[i]) * c.ldens[i]) * c.vm[i];

        let kone = (2.022 - 0.06238 * press)
            + (0.1722 - 0.01427 * press) * ((18.177 - 0.5987 * press) * quale).exp();
        let ktwo =
            (0.1484 - 1.596 * quale + 0.1729 * quale * quale.abs()) * 2.326 * gm * 10.0 + 3271.0;
        let kthree =
            (1.157 - 0.869 * quale) * (0.2664 + 0.8357 * (-124.1 * fuel.hydia / 100.0).exp());
        let kfour = 0.8258 + 0.000_341_3 * (hlsat - enthshift[i]);

        chf[i] = kone * ktwo * kthree * kfour / 10.0;
        dnbr[i] = chf[i] / th.heatflux[i];
    }

    // `chf.dnbr = fixinfnan(chf.dnbr)` — one argument, so the substitute is 0.
    let dnbr = fixinfnan(&dnbr, false);

    Chf { chf, dnbr }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Coolant;

    /// A uniform single-phase PWR channel at nominal conditions.
    fn pwr_channel(n: usize, heatflux: f64) -> (FuelGeometry, Th) {
        let fuel = FuelGeometry {
            hydia: 1.18,
            ..Default::default()
        };
        let th = Th {
            coolant: Coolant {
                inlettemp: 560.0,
                inletpress: 15.5,
                press: vec![15.5; n],
                enth: vec![1300.0; n],
                quality: vec![-0.05; n],
                alphag: vec![0.0; n],
                vm: vec![500.0; n],
                ldens: vec![0.70; n],
                gdens: vec![0.10; n],
                ..Default::default()
            },
            heatflux: vec![heatflux; n],
            ..Default::default()
        };
        (fuel, th)
    }

    /// The correlation produces a physically plausible PWR critical heat flux.
    ///
    /// # Methodology
    ///
    /// A uniform channel at 15.5 MPa, quality -0.05 (subcooled), mixture
    /// velocity 5 m/s in water of 0.70 g/cm³ — a mass flux of 3.5 Mg/(m²·s),
    /// inside W-3's stated validity band — with a 1.18 cm hydraulic diameter.
    ///
    /// Pass criterion: the CHF lands between 100 and 1000 W/cm². That band is
    /// deliberately wide. PWR critical heat fluxes are order 200-400 W/cm²
    /// (2-4 MW/m²), a figure quoted widely enough to be a sound magnitude
    /// check, but this is **not** a comparison against a published W-3
    /// evaluation — no tabulated case was available to check against — so a
    /// tight tolerance would claim more than the test establishes.
    ///
    /// What this **does** verify is the unit-conversion chain documented at the
    /// module level: an error in any of the seven folded constants, or in the
    /// g/cm³ versus kg/m³ reading of density, would move the result by orders
    /// of magnitude and fall outside the band.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Node 0 gives **274.85 W/cm²** and nodes 1-4 give **335.88 W/cm²** —
    /// 2.75 and 3.36 MW/m², both inside the quoted 2-4 MW/m² band for a PWR.
    /// (The two differ only by defect T1; see
    /// `the_upwind_enthalpy_is_halved_and_overpredicts_the_flux`.) The DNBRs
    /// against a 100 W/cm² wall flux are 2.75 and 3.36.
    ///
    /// **Interpretation.** Landing within a factor of ~1.3 of the expected
    /// magnitude, from a chain of seven folded unit conversions each of which
    /// would shift the answer by 10x-1000x if mistranscribed, is strong
    /// evidence the conversions are right. It is a verification of the
    /// implementation, not a validation of W-3 — no published evaluation of the
    /// correlation was available to compare against, so no accuracy claim
    /// about the correlation itself is made here.
    #[test]
    fn the_critical_heat_flux_is_physically_plausible_for_a_pwr() {
        let (fuel, th) = pwr_channel(5, 100.0);
        let out = w3chf(&Params::default(), &fuel, &th);

        for (i, q) in out.chf.iter().enumerate() {
            eprintln!("node {i}: chf = {q} W/cm2, dnbr = {}", out.dnbr[i]);
            assert!(
                (100.0..1000.0).contains(q),
                "chf = {q} W/cm2 is outside the plausible PWR band"
            );
        }
    }

    /// The DNBR is the CHF divided by the wall heat flux, and it falls as the
    /// wall flux rises.
    #[test]
    fn the_dnbr_falls_as_the_wall_flux_rises() {
        let (fuel, low) = pwr_channel(1, 50.0);
        let (_, high) = pwr_channel(1, 200.0);

        let a = w3chf(&Params::default(), &fuel, &low);
        let b = w3chf(&Params::default(), &fuel, &high);

        assert!((a.dnbr[0] - a.chf[0] / 50.0).abs() < 1e-12);
        assert!(a.dnbr[0] > b.dnbr[0], "{} vs {}", a.dnbr[0], b.dnbr[0]);
        // Same conditions, so the same CHF — only the margin changed.
        assert!((a.chf[0] - b.chf[0]).abs() < 1e-12);
    }

    /// A zero heat flux gives a zero DNBR, not infinity — the `fixinfnan`
    /// masking, pinned.
    ///
    /// This is the behaviour defect C5 is about: the output cannot distinguish
    /// "no heat flux here" from "no margin left".
    #[test]
    fn a_zero_heat_flux_reports_zero_margin_not_infinite_margin() {
        let (fuel, th) = pwr_channel(1, 0.0);
        let out = w3chf(&Params::default(), &fuel, &th);

        assert!(out.chf[0] > 0.0, "the correlation still yields a CHF");
        assert_eq!(
            out.dnbr[0], 0.0,
            "an unheated node reads as zero margin, not infinite"
        );
    }

    /// Defect T1, pinned: the upwind enthalpy is a quarter-sum, not a half-sum.
    ///
    /// # Methodology
    ///
    /// With a uniform enthalpy `h` along the channel, the intended two-point
    /// average `(h + h)/2` would be `h`; the reference's `(0.5h + 0.5h)/2` is
    /// `h/2`. Rather than reach into the private intermediate, this test
    /// detects it through the output: `Kfour = 0.8258 + 0.0003413*(hLsat -
    /// enthshift)`, so halving `enthshift` raises `Kfour` and hence the CHF.
    ///
    /// Node 0 takes the full inlet enthalpy and node 1 onwards the halved
    /// local one. Constructing a channel whose enthalpy equals the inlet
    /// enthalpy everywhere therefore makes node 0 and node 1 differ **only** by
    /// this defect — with the correct formula they would be identical.
    ///
    /// Pass criterion: node 1's CHF strictly exceeds node 0's, and the ratio
    /// matches the `Kfour` algebra to 1e-9.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// With `enthin = 1267.74 kJ/kg` at 15.5 MPa and 560 K, node 0 gives
    /// **274.85 W/cm²** and node 1 **337.48 W/cm²**. The ratio is
    /// **1.2278737302**, matching the `Kfour` algebra to better than 1e-12.
    ///
    /// **Interpretation, and why this one matters.** The stray `/2` inflates
    /// the predicted critical heat flux by **22.8%** at PWR conditions. That is
    /// not a rounding-level defect: it is a large, systematic, *non-conservative*
    /// error in a quantity whose entire purpose is to bound a safety margin, and
    /// it grows with subcooling. A DNBR computed from this correlation as
    /// shipped should not be read as a margin. Translated as written per the
    /// no-silent-repairs policy, and recorded as defect T1.
    #[test]
    fn the_upwind_enthalpy_is_halved_and_overpredicts_the_flux() {
        let fuel = FuelGeometry {
            hydia: 1.18,
            ..Default::default()
        };

        // Enthalpy uniform and equal to the inlet enthalpy, so the only
        // difference between node 0 and node 1 is the stray `/2`.
        let enthin = h1_pt(15.5, 560.0);
        let n = 3;
        let th = Th {
            coolant: Coolant {
                inlettemp: 560.0,
                inletpress: 15.5,
                press: vec![15.5; n],
                enth: vec![enthin; n],
                quality: vec![-0.05; n],
                alphag: vec![0.0; n],
                vm: vec![500.0; n],
                ldens: vec![0.70; n],
                gdens: vec![0.10; n],
                ..Default::default()
            },
            heatflux: vec![100.0; n],
            ..Default::default()
        };

        let out = // This test PINS defect T5, so it must select the defective form;
        // the crate default corrects it.
        w3chf(&Params::reference_faithful(), &fuel, &th);
        eprintln!(
            "enthin = {enthin} kJ/kg; chf node0 = {}, node1 = {}",
            out.chf[0], out.chf[1]
        );

        assert!(
            out.chf[1] > out.chf[0],
            "the halved enthalpy must overpredict: {} vs {}",
            out.chf[1],
            out.chf[0]
        );

        // The ratio is exactly the ratio of the two Kfour values.
        let hlsat = hl_p(15.5);
        let kfour_correct = 0.8258 + 0.000_341_3 * (hlsat - enthin);
        let kfour_defect = 0.8258 + 0.000_341_3 * (hlsat - enthin / 2.0);
        let expected = kfour_defect / kfour_correct;
        let actual = out.chf[1] / out.chf[0];
        eprintln!("kfour ratio expected {expected}, got {actual}");
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    /// Nodes 1 and 2 agree, confirming the defect affects only the first node's
    /// special case and not the shift itself.
    #[test]
    fn the_interior_nodes_are_self_consistent() {
        let (fuel, th) = pwr_channel(4, 100.0);
        let out = w3chf(&Params::default(), &fuel, &th);
        assert!((out.chf[1] - out.chf[2]).abs() < 1e-12);
        assert!((out.chf[2] - out.chf[3]).abs() < 1e-12);
    }

    /// **T5/T6 — what correcting the `K4` enthalpy does to a real case's
    /// reported critical heat flux and DNBR.**
    ///
    /// # Methodology
    ///
    /// The pinning tests above measure the defect on a synthetic node. This
    /// measures it where the number actually leaves the crate: NEACRP A2's
    /// coupled steady state, whose `CoupledOutput` carries `chf` and `dnbr`
    /// for the hottest channel.
    ///
    /// The same converged solve is post-processed both ways —
    /// [`W3Form::Reference`] and [`W3Form::Published`] — so the thermal
    /// hydraulic state is identical and the only difference is the `K4`
    /// enthalpy. That isolates the correction from every other effect,
    /// including the G1/G2/G3 correction, which is held at its default in both
    /// arms.
    ///
    /// The pass criterion is directional and follows from the algebra rather
    /// than from a reference: `K4 = 0.8258 + 0.0003413*(h_Lsat - h)` decreases
    /// with `h`, so restoring the full inlet enthalpy in place of a halved one
    /// must **lower** the predicted critical heat flux and lower the DNBR.
    /// A safety margin that gets smaller when a non-conservative error is
    /// removed is the expected direction; the reverse would mean the sign of
    /// the defect had been misread.
    ///
    /// # Results — measured 2026-08-21
    ///
    /// NEACRP A2, coupled steady, `k_eff = 1.0153550800` in **both** arms — so
    /// the W-3 form does not feed back into the solve and the two are being
    /// post-processed from an identical state.
    ///
    /// | | reference (T5/T6) | published W-3 |
    /// |---|---|---|
    /// | peak CHF, W/cm2 | 337.8054 | **275.3966** |
    /// | limiting DNBR | 2.5462 | **2.1034** |
    /// | CHF overprediction | **+22.66%** | — |
    ///
    /// **Interpretation.** The overprediction on a real converged case,
    /// **+22.66%**, matches the +22.8% measured on the synthetic node above —
    /// two independent routes to the same figure, which is what a systematic
    /// factor should give.
    ///
    /// The consequence is stated in the units that matter for a safety margin:
    /// the reported limiting **DNBR falls from 2.55 to 2.10**, a 17% cut in
    /// apparent margin. The reference was not merely imprecise, it was
    /// **non-conservative by about a fifth** in the one quantity whose purpose
    /// is to say how far the fuel is from departing nucleate boiling. Nothing
    /// downstream in this snapshot consumes it — defect C3 discards it, and
    /// this crate returns it instead — so the error has never had a chance to
    /// be noticed.
    ///
    /// **This run also exposes defect C2/T4 live.** The reported hottest
    /// channel is `analysed: (2, 2)` while the true peak is at `(2, 5)`:
    /// `w3chfhottest` searches with `highy = ix` where `iy` is meant, so it can
    /// only ever return a **diagonal** column. Every number in the table above
    /// is therefore computed for the wrong channel — correct arithmetic on the
    /// wrong data. The two defects compound: one picks the wrong channel, the
    /// other overpredicts the flux in it.
    #[test]
    #[ignore = "T5/T6 on a real case; one coupled A2 solve, minutes"]
    fn t5_what_correcting_the_k4_enthalpy_does_to_a_real_case() {
        let run = |form: W3Form| {
            let base = Params {
                th_model: crate::types::ThModel::Hem,
                nodalupd: 20,
                w3_form: form,
                ..Default::default()
            };
            let (params, geometry, th, whichsigma, sigmavalues, feedback) =
                crate::neacrpa2::neacrpa2(&base);
            crate::thdiffusion_solverxyz::thdiffusion_solverxyz(
                &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, Some(1.0),
            )
            .expect("A2 on the hem path should run")
        };

        let reference = run(W3Form::Reference);
        let published = run(W3Form::Published);

        // The two arms must be the same solve, or the comparison is invalid.
        assert_eq!(
            reference.k_eff, published.k_eff,
            "the W-3 form must not feed back into the solve"
        );

        let peak = |c: &Chf| c.chf.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        // The limiting DNBR is the SMALLEST one over the channel, ignoring the
        // zeros `fixinfnan` substitutes where the heat flux is zero.
        let limiting = |c: &Chf| {
            c.dnbr
                .iter()
                .cloned()
                .filter(|d| *d > 0.0)
                .fold(f64::INFINITY, f64::min)
        };

        let (cr, cp) = (&reference.chf, &published.chf);
        eprintln!("NEACRP A2, hottest channel {:?}:", reference.chf_channel);
        eprintln!("  k_eff (both arms)   = {:.10}", reference.k_eff);
        eprintln!();
        eprintln!("  {:<22} {:>16} {:>16}", "", "reference (T5/T6)", "published W-3");
        eprintln!(
            "  {:<22} {:>16.4} {:>16.4}",
            "peak CHF, W/cm2", peak(cr), peak(cp)
        );
        eprintln!(
            "  {:<22} {:>16.4} {:>16.4}",
            "limiting DNBR", limiting(cr), limiting(cp)
        );
        eprintln!(
            "  {:<22} {:>15.2}% {:>15}",
            "CHF overprediction",
            (peak(cr) / peak(cp) - 1.0) * 100.0,
            "-"
        );

        assert!(
            peak(cp) < peak(cr),
            "removing the halving must LOWER the predicted CHF: {:.4} vs {:.4}",
            peak(cp),
            peak(cr)
        );
        assert!(
            limiting(cp) < limiting(cr),
            "and so must lower the margin: {:.4} vs {:.4}",
            limiting(cp),
            limiting(cr)
        );
    }

    /// **The CHF corrections together — C2/T4 and T5/T6 on NEACRP A2.**
    ///
    /// # Methodology
    ///
    /// `t5_what_correcting_the_k4_enthalpy_does_to_a_real_case` isolates the
    /// `K4` enthalpy with the channel search held at the reference. This runs
    /// the two arms the crate actually ships between:
    /// [`Params::reference_faithful`] (both defects present, as the MATLAB has
    /// them) and [`Params::default`] (both corrected).
    ///
    /// They are not independent, and that is the reason for a separate test:
    /// C2/T4 changes **which channel** is analysed, T5/T6 changes **the flux
    /// computed in it**, and the combined effect is not the product of the two
    /// measured alone — a different channel has a different enthalpy, so the
    /// `K4` correction lands on different numbers.
    ///
    /// The same converged neutronics feeds both arms; only post-processing
    /// differs. No directional assertion is made on the DNBR here, because
    /// moving to the true hot channel *lowers* the margin while nothing
    /// guarantees the two effects share a sign — the measurement is the point.
    ///
    /// # Results — measured 2026-08-21
    ///
    /// NEACRP A2, coupled steady, identical `k_eff` in both arms.
    ///
    /// | | channel analysed | true peak | peak CHF, W/cm2 | limiting DNBR |
    /// |---|---|---|---|---|
    /// | as written | **(2, 2)** | (2, 5) | 337.8054 | 2.5462 |
    /// | corrected | (2, 5) | (2, 5) | **275.3966** | **2.0816** |
    ///
    /// **The reported margin falls 18.2%**, from a DNBR of 2.55 to 2.08.
    ///
    /// **Interpretation.** Splitting the two contributions against the
    /// `K4`-only measurement:
    ///
    /// | step | limiting DNBR |
    /// |---|---|
    /// | as written | 2.5462 |
    /// | correcting T5/T6 only (still the wrong channel) | 2.1034 |
    /// | also correcting C2/T4 | **2.0816** |
    ///
    /// So the enthalpy defect carries about **17.4%** of the overstatement and
    /// the channel defect a further **1.0%**. The channel error is the smaller
    /// of the two *here*, and that is a property of this case rather than of
    /// the defect: A2's power distribution happens to make `(2, 2)` and
    /// `(2, 5)` thermally similar. The synthetic test in
    /// [`crate::w3chfhottest`] shows the same defect overstating a margin by a
    /// factor of **5** when the two channels are not similar, so the small
    /// figure here should not be read as a bound.
    ///
    /// **Both defects push the same way — they overstate margin.** Correcting
    /// them makes the reported DNBR smaller, which is the direction that
    /// matters for a number whose only purpose is to say how close the fuel is
    /// to departing nucleate boiling.
    ///
    /// **What this does not establish.** The corrected CHF has been checked
    /// against the published W-3 correlation's *form*, not against measured
    /// CHF data, and W-3 has its own stated range of validity. This is a
    /// correction from "not the correlation" to "the correlation", which is
    /// verification, not validation.
    #[test]
    #[ignore = "the CHF corrections on a real case; one coupled A2 solve, minutes"]
    fn the_chf_corrections_together_on_neacrp_a2() {
        let run = |p: Params| {
            let base = Params {
                th_model: crate::types::ThModel::Hem,
                nodalupd: 20,
                ..p
            };
            let (params, geometry, th, whichsigma, sigmavalues, feedback) =
                crate::neacrpa2::neacrpa2(&base);
            crate::thdiffusion_solverxyz::thdiffusion_solverxyz(
                &geometry, &params, &th, &sigmavalues, &feedback, &whichsigma, Some(1.0),
            )
            .expect("A2 on the hem path should run")
        };

        // Only the CHF switches differ; the operator correction stays on in
        // both arms so the neutronics is identical.
        let as_written = run(Params {
            w3_form: W3Form::Reference,
            hot_channel_search: crate::types::HotChannelSearch::Reference,
            ..Default::default()
        });
        let corrected = run(Params::default());

        assert_eq!(
            as_written.k_eff, corrected.k_eff,
            "the CHF post-processing must not feed back into the solve"
        );

        let peak = |c: &Chf| c.chf.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let limiting = |c: &Chf| {
            c.dnbr
                .iter()
                .cloned()
                .filter(|d| *d > 0.0)
                .fold(f64::INFINITY, f64::min)
        };

        for (label, out) in [("as written", &as_written), ("corrected", &corrected)] {
            eprintln!(
                "{label:<12} channel analysed {:?}, true peak {:?}{}",
                out.chf_channel.analysed,
                out.chf_channel.true_peak,
                if out.chf_channel.misidentified() { "  <- WRONG CHANNEL" } else { "" }
            );
            eprintln!(
                "             peak CHF {:.4} W/cm2, limiting DNBR {:.4}",
                peak(&out.chf),
                limiting(&out.chf)
            );
        }
        eprintln!();
        eprintln!(
            "reported margin changes by {:+.1}% ({:.4} -> {:.4})",
            (limiting(&corrected.chf) / limiting(&as_written.chf) - 1.0) * 100.0,
            limiting(&as_written.chf),
            limiting(&corrected.chf)
        );

        // The corrected arm must at least be self-consistent about its channel.
        assert!(
            !corrected.chf_channel.misidentified(),
            "the corrected search must analyse the channel it found"
        );
    }
}
