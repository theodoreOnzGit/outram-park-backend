//! `GASPR` — gas-production cross sections (ENDF MT=203–207).
//!
//! Computes the total production of the five light "gas" nuclides — H1
//! (proton), H2 (deuteron), H3 (triton), He3, He4 (alpha) — from a
//! [`ReconrResult`]: post-processing information used for depletion / material
//! swelling estimates, **not** transport itself. Like KERMA/heating (MT=301),
//! this is an MF=3-only informational cross section: no secondary angle/energy
//! law is needed, because it never feeds the collision estimator — it is only
//! ever *read*, not *sampled*.
//!
//! Ported from NJOY2016 `src/gaspr.f90` (~1150 lines), specifically the
//! gas-accumulation loop at `gaspr.f90:471-830`. ENDF MT numbers 11, 16, 17,
//! 22–45, 51–91, and 102–200 are *mutually exclusive* reaction final states (a
//! neutron that reacts via MT=107 `(n,α)` cannot simultaneously have reacted
//! via MT=103 `(n,p)`), so total gas production is the yield-weighted sum
//!
//! ```text
//! σ_gas(E) = Σ_mt  n_particle(mt) · σ_mt(E)
//! ```
//!
//! over every reconstructed MF=3 section that survives NJOY's skip list.
//!
//! # The yield has two halves, and both are needed
//!
//! `n_particle(mt)` is **not** a flat lookup on MT. NJOY builds it from two
//! contributions, and this port now does the same:
//!
//! 1. **The ejectiles the MT names.** MT=105 `(n,t)` emits one triton; MT=23
//!    `(n,n'3α)` emits three alphas. This half is a lookup — see
//!    [`gas_channel`].
//! 2. **The residual nucleus, when it is itself one of the five gases.** NJOY
//!    derives the residual as `izr = ZA_target + 1 − Σ ZA_ejectile` and then
//!    (`gaspr.f90:821-826`) credits it:
//!
//!    ```text
//!    izr = 1001 → +1 proton      izr = 2003 → +1 He-3
//!    izr = 1002 → +1 deuteron    izr = 2004 → +1 alpha
//!    izr = 1003 → +1 triton      izr = 4008 → +2 alphas  (Be-8 is unbound)
//!    ```
//!
//! **Half 2 is not a corner case**, which is why an earlier revision of this
//! module that omitted it was wrong on the reactions gas production exists to
//! describe. Worked examples, each independently checkable against textbook
//! nuclear physics rather than against NJOY:
//!
//! | reaction | MT | residual | without half 2 | with half 2 |
//! |---|---|---|---|---|
//! | ⁶Li(n,t)⁴He — tritium breeding | 105 | `2004` | 1 t | 1 t **+ 1 α** |
//! | ³He(n,p)³H — the ³He detector reaction | 103 | `1003` | 1 p | 1 p **+ 1 t** |
//! | ⁹Be(n,2n)⁸Be → 2α | 16 | `4008` | nothing | **2 α** |
//! | ²H(n,γ)³H — tritium from heavy water | 102 | `1003` | nothing | **1 t** |
//! | ¹⁰B(n,α)⁷Li | 107 | `3007` | 1 α | 1 α (unchanged) |
//!
//! # `LR` breakup on the inelastic levels is also load-bearing
//!
//! For MT=51–91 the ejectile set depends on the section's **`LR`** flag
//! ([`ReconrSection::lr`], the MF=3 TAB1 `L2`), not on the MT alone: `LR=22`
//! means the residual promptly breaks up emitting an alpha, `LR=32` a
//! deuteron, `LR=33` a triton, and so on (`gaspr.f90:565-608`). In
//! ENDF/B-VIII.0 this is common exactly where gas production matters most —
//! Li-6 carries `LR=32` on 30 levels, Li-7 `LR=33` on 31, B-10 a mix of
//! `LR=22/28/35`, C-12 `LR=23`.
//!
//! # MT=5's yields need the tape, not just the reconstruction
//!
//! **Energy-dependent MT=5 yields** (`gaspr.f90:501-507`, the `y = 111`
//! sentinel) *are* ported, but they need the evaluation as well as the
//! reconstruction, so they live on a second entry point:
//! [`GasProduction::from_reconr_and_tape`]. MT=5 is a lump of many final
//! states, so the gas per event is not a function of the MT number; the
//! evaluation states it as an MF=6 multiplicity `y(E)` per product `ZAP`.
//! [`GasProduction::from_reconr`] omits this contribution — use it only when
//! the evaluation has no MT=5, or when the tape is not to hand.
//!
//! # Not ported
//!
//! The legacy **MT=600–849** detailed-breakup fallback `gaspr.f90` uses when an
//! evaluation omits the lumped channels above (pre-ENDF/B-VI representation;
//! rare in ENDF/B-VII/VIII, the libraries this workspace targets). NJOY skips
//! these in its accumulation loop unless the lumped channels are absent; this
//! port always skips them, so such a tape is **under-counted rather than
//! mis-counted**.
//!
//! Note this is GASPR's own input fallback, and is *not* the same as RECONR
//! synthesising the lumped MT=103–107 from those levels when the evaluation
//! carries the levels but not the lump — that **is** done, in
//! [`crate::reconr`], and B-10 needs it (it has MT=700 and no MT=105).
//!
//! # Verification
//!
//! Cross-checked against NJOY2016 `ac5adf5f`, built from source and run, on
//! Li-6, Be-9, B-10, H-2 and C-12: all 17 gas sections both codes produce
//! agree, worst 4.9e-3 relative, typical 1e-4 to 8e-4
//! (`tests/gaspr_vs_njoy2016.rs`, and
//! `verification_and_validation/gaspr_light_nuclides_vs_njoy2016.md`). That
//! comparison is what found the missing residual rule, the missing RECONR
//! lumping, and the missing MT=5 yields. **No human V&V.**

use crate::acer::energy::mf6::parse_mf6_product_yields;
use crate::endf::interp::eval_tab1;
use crate::endf::tape::Tape;
use crate::reconr::{eval_lin_lin, ReconrResult};

/// One of the five gas nuclides GASPR tracks, with its ENDF MT number
/// (MT=203–207) in the ACE/PENDF gas-production convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasSpecies {
    /// ¹H (proton) — MT=203.
    H1,
    /// ²H (deuteron) — MT=204.
    H2,
    /// ³H (triton) — MT=205.
    H3,
    /// ³He — MT=206.
    He3,
    /// ⁴He (alpha) — MT=207.
    He4,
}

impl GasSpecies {
    /// The ENDF MT number this species' production cross section would occupy
    /// on a PENDF tape (MT=203…207).
    pub fn mt(self) -> i32 {
        match self {
            GasSpecies::H1 => 203,
            GasSpecies::H2 => 204,
            GasSpecies::H3 => 205,
            GasSpecies::He3 => 206,
            GasSpecies::He4 => 207,
        }
    }
}

/// How many of each gas nuclide one event of a given MT produces. All-zero for
/// reactions that emit no charged particle and leave a non-gas residual
/// (elastic, inelastic with `LR=0` on a heavy target, fission, capture on
/// anything above He-3, …).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GasYield {
    /// Protons (→ MT=203) produced per reaction event.
    pub p: u8,
    /// Deuterons (→ MT=204) per event.
    pub d: u8,
    /// Tritons (→ MT=205) per event.
    pub t: u8,
    /// ³He nuclei (→ MT=206) per event.
    pub he3: u8,
    /// Alphas (→ MT=207) per event.
    pub alpha: u8,
}

impl GasYield {
    /// True when this reaction produces none of the five gas nuclides, in
    /// which case GASPR drops the section entirely (`gaspr.f90:827`).
    pub fn is_zero(self) -> bool {
        self == GasYield::default()
    }

    const fn new(p: u8, d: u8, t: u8, he3: u8, alpha: u8) -> Self {
        GasYield {
            p,
            d,
            t,
            he3,
            alpha,
        }
    }
}

/// One MF=3 section's contribution to gas production, before the residual
/// nucleus is accounted for.
///
/// Returned by [`gas_channel`]. Keeping the two halves separate is what lets
/// the residual rule be applied: `ejectile_za` is exactly NJOY's running `izr`
/// subtrahend, so `ZA_target + 1 − ejectile_za` is the residual nuclide's ZA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasChannel {
    /// The light particles this MT names in its own final state.
    pub emitted: GasYield,
    /// Sum of the ejectiles' `ZA = 1000·Z + A` (a bare neutron contributes
    /// `1`, a proton `1001`, an alpha `2004`, …).
    pub ejectile_za: i32,
}

impl GasChannel {
    const fn new(emitted: GasYield, ejectile_za: i32) -> Self {
        GasChannel {
            emitted,
            ejectile_za,
        }
    }
}

/// The per-reaction ejectile table, transcribed from `gaspr.f90:501-820`.
///
/// `mt` is the raw ENDF MT number and `lr` the MF=3 breakup flag (only read
/// for MT=51–91; pass `0` elsewhere). Returns `None` for a section GASPR skips
/// outright — the sums MT=1–4, the redundant/derived MTs, fission, and the
/// detailed MT=600–849 level breakdown (`gaspr.f90:471-491`).
///
/// A returned channel with an all-zero [`GasChannel::emitted`] and
/// `ejectile_za == 0` is **not** the same as `None`: MT=102 reaches this state
/// and still produces gas whenever the compound nucleus is itself H-2, H-3 or
/// He-4.
pub fn gas_channel(mt: i32, lr: i32) -> Option<GasChannel> {
    // --- gaspr.f90:471-491, the main loop's skip list -----------------------
    if mt <= 4
        || (6..=10).contains(&mt)
        || (12..=15).contains(&mt)
        || (18..=21).contains(&mt)
        || (38..=40).contains(&mt)
        || mt == 43
        || (46..=50).contains(&mt)
        || (92..=101).contains(&mt)
        || (201..=599).contains(&mt)
        || (600..=849).contains(&mt)
        || mt > 849
        || matches!(mt, 152 | 153 | 160 | 161)
    {
        return None;
    }

    let y = GasYield::new;
    let ch = GasChannel::new;
    // Field order: (p, d, t, he3, alpha), then the ejectile ZA sum.
    Some(match mt {
        // MT=5 with no MF=6 multiplicity: NJOY falls through with nothing
        // subtracted, so only the residual rule can fire. See the module docs.
        5 => ch(y(0, 0, 0, 0, 0), 0),
        11 => ch(y(0, 1, 0, 0, 0), 1004), // (n,2n d)
        16 => ch(y(0, 0, 0, 0, 0), 2),    // (n,2n)
        17 => ch(y(0, 0, 0, 0, 0), 3),    // (n,3n)
        22 => ch(y(0, 0, 0, 0, 1), 2005), // (n,n' α)
        23 => ch(y(0, 0, 0, 0, 3), 6013), // (n,n' 3α)
        24 => ch(y(0, 0, 0, 0, 1), 2006), // (n,2n α)
        25 => ch(y(0, 0, 0, 0, 1), 2007), // (n,3n α)
        28 => ch(y(1, 0, 0, 0, 0), 1002), // (n,n' p)
        29 => ch(y(0, 0, 0, 0, 2), 4009), // (n,n' 2α)
        30 => ch(y(0, 0, 0, 0, 2), 4010), // (n,2n 2α)
        32 => ch(y(0, 1, 0, 0, 0), 1003), // (n,n' d)
        33 => ch(y(0, 0, 1, 0, 0), 1004), // (n,n' t)
        34 => ch(y(0, 0, 0, 1, 0), 2004), // (n,n' ³He)
        35 => ch(y(0, 1, 0, 0, 2), 5011), // (n,n' d 2α)
        36 => ch(y(0, 0, 1, 0, 2), 5012), // (n,n' t 2α)
        37 => ch(y(0, 0, 0, 0, 0), 4),    // (n,4n)
        41 => ch(y(1, 0, 0, 0, 0), 1003), // (n,2n p)
        42 => ch(y(1, 0, 0, 0, 0), 1004), // (n,3n p)
        44 => ch(y(2, 0, 0, 0, 0), 2003), // (n,n' 2p)
        45 => ch(y(1, 0, 0, 0, 1), 3006), // (n,n' p α)

        // Inelastic levels: one neutron out, plus whatever LR says the
        // residual promptly emits (gaspr.f90:565-608).
        51..=91 => {
            let breakup = match lr {
                22 => ch(y(0, 0, 0, 0, 1), 2004),
                23 => ch(y(0, 0, 0, 0, 3), 6012),
                24 => ch(y(0, 0, 0, 0, 1), 2005),
                25 => ch(y(0, 0, 0, 0, 1), 2006),
                28 => ch(y(1, 0, 0, 0, 0), 1001),
                29 => ch(y(0, 0, 0, 0, 2), 4008),
                30 => ch(y(0, 0, 0, 0, 2), 4009),
                32 => ch(y(0, 1, 0, 0, 0), 1002),
                33 => ch(y(0, 0, 1, 0, 0), 1003),
                34 => ch(y(0, 0, 0, 1, 0), 2003),
                35 => ch(y(0, 1, 0, 0, 2), 5010),
                36 => ch(y(0, 0, 1, 0, 2), 5011),
                // LR=0 (no breakup) and LR=39/40 (internal conversion /
                // no-ejectile) subtract nothing beyond the scattered neutron.
                _ => ch(y(0, 0, 0, 0, 0), 0),
            };
            ch(breakup.emitted, 1 + breakup.ejectile_za)
        }

        103 => ch(y(1, 0, 0, 0, 0), 1001), // (n,p)
        104 => ch(y(0, 1, 0, 0, 0), 1002), // (n,d)
        105 => ch(y(0, 0, 1, 0, 0), 1003), // (n,t)
        106 => ch(y(0, 0, 0, 1, 0), 2003), // (n,³He)
        107 => ch(y(0, 0, 0, 0, 1), 2004), // (n,α)
        108 => ch(y(0, 0, 0, 0, 2), 4008), // (n,2α)
        109 => ch(y(0, 0, 0, 0, 3), 6012), // (n,3α)
        111 => ch(y(2, 0, 0, 0, 0), 2002), // (n,2p)
        112 => ch(y(1, 0, 0, 0, 1), 3005), // (n,p α)
        113 => ch(y(0, 0, 1, 0, 2), 5011), // (n,t 2α)
        114 => ch(y(0, 1, 0, 0, 2), 5010), // (n,d 2α)
        115 => ch(y(1, 1, 0, 0, 0), 2003), // (n,p d)
        116 => ch(y(1, 0, 1, 0, 0), 2004), // (n,p t)
        117 => ch(y(0, 1, 0, 0, 1), 3006), // (n,d α)

        154 => ch(y(0, 0, 1, 0, 0), 1005),
        155 => ch(y(0, 0, 1, 0, 1), 3007),
        156 => ch(y(1, 0, 0, 0, 0), 1005),
        157 => ch(y(0, 1, 0, 0, 0), 1005),
        158 => ch(y(0, 1, 0, 0, 1), 3007),
        159 => ch(y(1, 0, 0, 0, 1), 3007),
        162 => ch(y(1, 0, 0, 0, 0), 1006),
        163 => ch(y(1, 0, 0, 0, 0), 1007),
        164 => ch(y(1, 0, 0, 0, 0), 1008),
        165 => ch(y(0, 0, 0, 0, 1), 2008),
        166 => ch(y(0, 0, 0, 0, 1), 2009),
        167 => ch(y(0, 0, 0, 0, 1), 2010),
        168 => ch(y(0, 0, 0, 0, 1), 2011),
        169 => ch(y(0, 1, 0, 0, 0), 1006),
        170 => ch(y(0, 1, 0, 0, 0), 1007),
        171 => ch(y(0, 1, 0, 0, 0), 1008),
        172 => ch(y(0, 0, 1, 0, 0), 1006),
        173 => ch(y(0, 0, 1, 0, 0), 1007),
        174 => ch(y(0, 0, 1, 0, 0), 1008),
        175 => ch(y(0, 0, 1, 0, 0), 1009),
        176 => ch(y(0, 0, 0, 1, 0), 2005),
        177 => ch(y(0, 0, 0, 1, 0), 2006),
        178 => ch(y(0, 0, 0, 1, 0), 2007),
        179 => ch(y(2, 0, 0, 0, 0), 2005),
        180 => ch(y(0, 0, 0, 0, 2), 4011),
        181 => ch(y(1, 0, 0, 0, 1), 3008),
        182 => ch(y(0, 1, 1, 0, 0), 2005),
        183 => ch(y(1, 1, 0, 0, 0), 2004),
        184 => ch(y(1, 0, 1, 0, 0), 2005),
        185 => ch(y(0, 1, 1, 0, 0), 2006),
        186 => ch(y(1, 0, 0, 1, 0), 3005),
        187 => ch(y(0, 1, 0, 1, 0), 3006),
        188 => ch(y(0, 0, 1, 1, 0), 3007),
        189 => ch(y(0, 0, 1, 0, 1), 3008),
        190 => ch(y(2, 0, 0, 0, 0), 2004),
        191 => ch(y(1, 0, 0, 1, 0), 3004),
        192 => ch(y(0, 1, 0, 1, 0), 3005),
        193 => ch(y(0, 0, 0, 1, 1), 4007),
        194 => ch(y(2, 0, 0, 0, 0), 2006),
        195 => ch(y(0, 0, 0, 0, 2), 4012),
        196 => ch(y(1, 0, 0, 0, 1), 3009),
        197 => ch(y(3, 0, 0, 0, 0), 3003),
        198 => ch(y(3, 0, 0, 0, 0), 3004),
        199 => ch(y(2, 0, 0, 0, 1), 4009),
        200 => ch(y(2, 0, 0, 0, 0), 2007),

        // Everything else that survives the skip list — MT=102 above all, plus
        // the unassigned numbers — emits no light particle of its own. The
        // residual rule still applies, and for MT=102 that is the whole point.
        _ => ch(y(0, 0, 0, 0, 0), 0),
    })
}

/// Credit the residual nucleus when it is itself one of the five gases
/// (`gaspr.f90:821-826`).
///
/// `residual_za` is `1000·Z + A` of the nucleus left behind. Be-8 (`4008`) is
/// particle-unbound and counts as **two** alphas, which is what makes
/// ⁹Be(n,2n) a gas producer at all.
fn credit_residual(mut y: GasYield, residual_za: i32) -> GasYield {
    match residual_za {
        1001 => y.p += 1,
        1002 => y.d += 1,
        1003 => y.t += 1,
        2003 => y.he3 += 1,
        2004 => y.alpha += 1,
        4008 => y.alpha += 2,
        _ => {}
    }
    y
}

/// Total gas yield per event for one MF=3 section of a given target.
///
/// `target_za` is the evaluation's `ZA = 1000·Z + A` (from
/// [`crate::reconr::MaterialInfo::za`]), `mt` the raw MT number, `lr` the MF=3
/// breakup flag. Returns `None` for a section GASPR skips.
///
/// This is the composition the module docs describe: [`gas_channel`]'s named
/// ejectiles, plus [`credit_residual`] applied to
/// `residual = target_za + 1 − Σ ZA_ejectile`.
pub fn gas_yield_for(target_za: i32, mt: i32, lr: i32) -> Option<GasYield> {
    let channel = gas_channel(mt, lr)?;
    // `zain = 1` for an incident neutron (gaspr.f90:97, NSUB=10).
    let residual = target_za + 1 - channel.ejectile_za;
    Some(credit_residual(channel.emitted, residual))
}

/// Gas-production cross sections (ENDF MT=203–207) vs incident energy \[eV\],
/// tabulated on the union energy grid of every contributing reaction.
///
/// Built once from a [`ReconrResult`] via [`GasProduction::from_reconr`];
/// evaluated per species with [`GasProduction::eval`].
#[derive(Debug, Clone, Default)]
pub struct GasProduction {
    /// Union incident-energy grid \[eV\], ascending, deduplicated.
    pub energy: Vec<f64>,
    /// σ(MT=203) \[barn\] aligned with `energy`: total proton production.
    pub h1: Vec<f64>,
    /// σ(MT=204) \[barn\]: total deuteron production.
    pub h2: Vec<f64>,
    /// σ(MT=205) \[barn\]: total triton production.
    pub h3: Vec<f64>,
    /// σ(MT=206) \[barn\]: total ³He production.
    pub he3: Vec<f64>,
    /// σ(MT=207) \[barn\]: total alpha production.
    pub he4: Vec<f64>,
}

impl GasProduction {
    /// Compute gas-production cross sections from a reconstructed evaluation.
    ///
    /// For every MF=3 section that survives NJOY's skip list, the per-event
    /// yield is [`gas_yield_for`] — named ejectiles plus the residual nucleus.
    /// Sections whose total yield is zero (the overwhelming majority: elastic,
    /// most capture, fission, pure `(n,xn)` on a heavy target) are dropped
    /// from both the union grid and the sums, exactly as `gaspr.f90:827` does.
    /// At each grid point the survivors contribute
    /// `n_particle · σ_mt(E)` with σ lin-lin interpolated
    /// ([`crate::reconr::eval_lin_lin`]).
    ///
    /// Returns an all-empty [`GasProduction`] if nothing produces gas.
    pub fn from_reconr(recon: &ReconrResult) -> Self {
        let target_za = recon.material.za.round() as i32;

        // Resolve each section's yield once — it is needed for the union grid
        // and again for the sums, and it is not a pure function of MT.
        let contributions: Vec<(&crate::reconr::ReconrSection, GasYield)> = recon
            .sections
            .iter()
            .filter_map(|s| {
                let y = gas_yield_for(target_za, s.mt.number(), s.lr)?;
                (!y.is_zero()).then_some((s, y))
            })
            .collect();

        let mut energy: Vec<f64> = contributions
            .iter()
            .flat_map(|(s, _)| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let n = energy.len();
        let mut out = GasProduction {
            energy,
            h1: vec![0.0; n],
            h2: vec![0.0; n],
            h3: vec![0.0; n],
            he3: vec![0.0; n],
            he4: vec![0.0; n],
        };

        for (sec, y) in &contributions {
            for (i, &e) in out.energy.iter().enumerate() {
                let sigma = eval_lin_lin(&sec.pairs, e);
                if sigma == 0.0 {
                    continue;
                }
                out.h1[i] += f64::from(y.p) * sigma;
                out.h2[i] += f64::from(y.d) * sigma;
                out.h3[i] += f64::from(y.t) * sigma;
                out.he3[i] += f64::from(y.he3) * sigma;
                out.he4[i] += f64::from(y.alpha) * sigma;
            }
        }
        out
    }

    /// Compute gas production including the **energy-dependent MT=5 yields**
    /// the evaluation states as MF=6 multiplicities (`gaspr.f90:100-240` and
    /// `:501-507`).
    ///
    /// MT=5 `(n,anything)` lumps final states that the MT number cannot
    /// distinguish, so its gas production per event is not a lookup: the
    /// evaluation writes a multiplicity `y(E)` for each emitted product `ZAP`
    /// in MF=6, and GASPR uses `y(E)·σ_5(E)` in place of an integer yield.
    ///
    /// `tape` must be the same evaluation `recon` was reconstructed from, and
    /// `mat` its MAT number. When that evaluation has no MF=6/MT=5 section the
    /// result is identical to [`GasProduction::from_reconr`].
    ///
    /// # Measured
    ///
    /// C-12 (ENDF/B-VIII.0, MAT 625) is the case in `reference-data/endf/`:
    /// MT=5 carries proton and deuteron multiplicities from 14.5 MeV to
    /// 150 MeV. Without this path the crate produced **zero** proton and
    /// deuteron production above 20 MeV against NJOY2016's 3.48e-2 b and
    /// 1.31e-2 b at 20 MeV (`tests/gaspr_vs_njoy2016.rs`, 2026-09-17).
    pub fn from_reconr_and_tape(recon: &ReconrResult, tape: &Tape, mat: i32) -> Self {
        let mut out = Self::from_reconr(recon);

        let Some(mf6_mt5) = tape.section(mat, 6, 5) else {
            return out;
        };
        let Ok(products) = parse_mf6_product_yields(mf6_mt5) else {
            return out;
        };
        let Some(mt5) = recon.sections.iter().find(|s| s.mt.number() == 5) else {
            return out;
        };

        // The five ZAPs GASPR tracks, in MT=203..207 order.
        let tracked: [(i32, usize); 5] = [
            (1001, 0), // H-1  -> MT=203
            (1002, 1), // H-2  -> MT=204
            (1003, 2), // H-3  -> MT=205
            (2003, 3), // He-3 -> MT=206
            (2004, 4), // He-4 -> MT=207
        ];

        let mut columns: Vec<(usize, &crate::endf::records::Tab1)> = Vec::new();
        for (zap, col) in tracked {
            if let Some((_, y)) = products.iter().find(|(z, _)| *z == zap) {
                columns.push((col, y));
            }
        }
        if columns.is_empty() {
            return out;
        }

        // MT=5's own grid joins the union grid: without it the MF=6 yields
        // would be sampled only where some other gas channel already has a
        // node, and above the highest such node not at all.
        let mut energy = out.energy.clone();
        energy.extend(mt5.pairs.iter().map(|&(e, _)| e));
        energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12 * b.abs().max(1.0));

        let regrid = |col: &[f64], energy: &[f64], old: &[f64]| -> Vec<f64> {
            if old.is_empty() {
                return vec![0.0; energy.len()];
            }
            let pairs: Vec<(f64, f64)> = old.iter().copied().zip(col.iter().copied()).collect();
            energy.iter().map(|&e| eval_lin_lin(&pairs, e)).collect()
        };
        let old_energy = out.energy.clone();
        let mut cols = [
            regrid(&out.h1, &energy, &old_energy),
            regrid(&out.h2, &energy, &old_energy),
            regrid(&out.h3, &energy, &old_energy),
            regrid(&out.he3, &energy, &old_energy),
            regrid(&out.he4, &energy, &old_energy),
        ];

        for (i, &e) in energy.iter().enumerate() {
            let sigma = eval_lin_lin(&mt5.pairs, e);
            if sigma == 0.0 {
                continue;
            }
            for &(col, y) in &columns {
                let mult = eval_tab1(e, &y.interp, &y.pairs).unwrap_or(0.0);
                if mult > 0.0 {
                    cols[col][i] += mult * sigma;
                }
            }
        }

        out.energy = energy;
        let [h1, h2, h3, he3, he4] = cols;
        out.h1 = h1;
        out.h2 = h2;
        out.h3 = h3;
        out.he3 = he3;
        out.he4 = he4;
        out
    }

    /// Evaluate one species' gas-production cross section \[barn\] at incident
    /// energy `e` \[eV\] (lin-lin interpolated, clamped at the tabulated ends).
    pub fn eval(&self, species: GasSpecies, e: f64) -> f64 {
        let col = match species {
            GasSpecies::H1 => &self.h1,
            GasSpecies::H2 => &self.h2,
            GasSpecies::H3 => &self.h3,
            GasSpecies::He3 => &self.he3,
            GasSpecies::He4 => &self.he4,
        };
        if self.energy.is_empty() {
            return 0.0;
        }
        let pairs: Vec<(f64, f64)> = self
            .energy
            .iter()
            .copied()
            .zip(col.iter().copied())
            .collect();
        eval_lin_lin(&pairs, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::MtReaction;
    use crate::reconr::MaterialInfo;

    fn section(mt: MtReaction, pairs: Vec<(f64, f64)>) -> crate::reconr::ReconrSection {
        crate::reconr::ReconrSection {
            lr: 0,
            mt,
            qi: 0.0,
            pairs,
        }
    }

    fn section_lr(mt: MtReaction, lr: i32, pairs: Vec<(f64, f64)>) -> crate::reconr::ReconrSection {
        crate::reconr::ReconrSection {
            lr,
            mt,
            qi: 0.0,
            pairs,
        }
    }

    fn recon_za(za: f64, sections: Vec<crate::reconr::ReconrSection>) -> ReconrResult {
        let material = MaterialInfo {
            za,
            awr: za / 1000.0,
            lrp: 1,
            lfi: 0,
            nlib: 0,
            elis: 0.0,
            nfor: 6,
            emax: 2.0e7,
        };
        ReconrResult {
            material,
            sections,
            resonance_upper_limit: None,
            unresolved_table: None,
        }
    }

    /// Fe-56 target: every residual below is far heavier than He-4, so these
    /// legacy cases exercise the named-ejectile half in isolation.
    fn recon(sections: Vec<crate::reconr::ReconrSection>) -> ReconrResult {
        recon_za(26056.0, sections)
    }

    /// A single `(n,α)` (MT=107) section on Fe-56 reconstructs directly into
    /// MT=207 (He4) with yield 1, and every other species stays zero. The
    /// residual is Cr-53 (`26057 − 2004 = 24053`), not a gas, so the residual
    /// rule contributes nothing here.
    #[test]
    fn np_alpha_reaction_yields_he4_only() {
        let recon = recon(vec![section(
            MtReaction::Mt107NAlpha,
            vec![(1.0e6, 0.0), (2.0e6, 0.5), (3.0e6, 1.0)],
        )]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 2.0e6) - 0.5).abs() < 1e-9);
        assert_eq!(gas.eval(GasSpecies::H1, 2.0e6), 0.0);
        assert_eq!(gas.eval(GasSpecies::H2, 2.0e6), 0.0);
        assert_eq!(gas.eval(GasSpecies::H3, 2.0e6), 0.0);
        assert_eq!(gas.eval(GasSpecies::He3, 2.0e6), 0.0);
    }

    /// Exclusive channels are additive: MT=107 `(n,α)` (yield 1 He4) and MT=22
    /// `(n,n'α)` (yield 1 He4) both contribute to the same He4 total, summed —
    /// this is the core "disjoint final states, no double counting" claim.
    #[test]
    fn disjoint_channels_sum_additively() {
        let recon = recon(vec![
            section(MtReaction::Mt107NAlpha, vec![(1.0e6, 0.4), (2.0e6, 0.4)]),
            section(MtReaction::Mt22NnAlpha, vec![(1.0e6, 0.1), (2.0e6, 0.3)]),
        ]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 1.0e6) - 0.5).abs() < 1e-9);
        assert!((gas.eval(GasSpecies::He4, 2.0e6) - 0.7).abs() < 1e-9);
    }

    /// A multi-particle-per-event channel (MT=23, `(n,n'3α)`) is weighted by
    /// its true yield of 3, not 1.
    #[test]
    fn multi_particle_yield_is_weighted() {
        let recon = recon(vec![section(
            MtReaction::Mt23Nn3Alpha,
            vec![(1.0e6, 0.2), (2.0e6, 0.2)],
        )]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 1.5e6) - 0.6).abs() < 1e-9);
    }

    /// A channel that produces two different species (MT=45, `(n,n'pα)`)
    /// contributes to both, each with yield 1.
    #[test]
    fn two_species_channel_credits_both() {
        let recon = recon(vec![section(
            MtReaction::Mt45NnProtonAlpha,
            vec![(1.0e6, 0.8), (2.0e6, 0.8)],
        )]);
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::H1, 1.5e6) - 0.8).abs() < 1e-9);
        assert!((gas.eval(GasSpecies::He4, 1.5e6) - 0.8).abs() < 1e-9);
    }

    /// Non-gas-producing reactions (elastic, capture on a heavy target, pure
    /// `(n,2n)` on a heavy target) contribute nothing — the filter in
    /// `from_reconr` must exclude them from both the union energy grid and the
    /// sums. On Fe-56 the MT=102 residual is Fe-57 and the MT=16 residual is
    /// Fe-55; neither is one of the five gases.
    #[test]
    fn non_gas_reactions_are_ignored() {
        let recon = recon(vec![
            section(MtReaction::Mt2Elastic, vec![(1.0e6, 5.0), (2.0e6, 5.0)]),
            section(MtReaction::Mt102Capture, vec![(1.0e6, 0.2), (2.0e6, 0.1)]),
            section(MtReaction::Mt16N2n, vec![(1.0e6, 0.0), (2.0e6, 0.05)]),
        ]);
        let gas = GasProduction::from_reconr(&recon);
        assert!(
            gas.energy.is_empty(),
            "no gas-producing sections ⇒ empty grid"
        );
        for species in [
            GasSpecies::H1,
            GasSpecies::H2,
            GasSpecies::H3,
            GasSpecies::He3,
            GasSpecies::He4,
        ] {
            assert_eq!(gas.eval(species, 1.5e6), 0.0);
        }
    }

    /// `GasSpecies::mt` matches the standard ACE/PENDF gas-production MT range.
    #[test]
    fn species_mt_numbers_are_203_to_207() {
        assert_eq!(GasSpecies::H1.mt(), 203);
        assert_eq!(GasSpecies::H2.mt(), 204);
        assert_eq!(GasSpecies::H3.mt(), 205);
        assert_eq!(GasSpecies::He3.mt(), 206);
        assert_eq!(GasSpecies::He4.mt(), 207);
    }

    /// Fission (MT=18–21, 38) and the redundant sums (MT=1–4, 101) are on
    /// NJOY's skip list and must never reach the yield table at all —
    /// `gaspr.f90:471-491`. This is distinct from "yields zero": a skipped
    /// section never even gets a residual computed, which matters because a
    /// light target's fission residual would otherwise be credited as gas.
    ///
    /// **MT=27 is deliberately absent from both lists here, because upstream
    /// does not skip it.** It is a redundant absorption sum, and on a tape
    /// that carries it as MF=3 NJOY falls through the ejectile chain with
    /// nothing subtracted, so the residual rule can fire on `ZA + 1`. That is
    /// a quirk of `gaspr.f90:471-491` rather than a considered choice, and
    /// this port reproduces it rather than silently diverging — the porting
    /// rule is to mirror upstream and record the oddity, not to fix it here.
    #[test]
    fn skip_list_matches_upstream() {
        for mt in [1, 2, 3, 4, 18, 19, 20, 21, 38, 43, 101, 152, 601, 800] {
            assert!(
                gas_channel(mt, 0).is_none(),
                "MT={mt} must be skipped (gaspr.f90:471-491)"
            );
        }
        for mt in [5, 16, 22, 51, 91, 102, 105, 107, 117, 200] {
            assert!(
                gas_channel(mt, 0).is_some(),
                "MT={mt} must reach the yield table"
            );
        }
    }

    /// ⁶Li(n,t)⁴He — the tritium-breeding reaction, Q = +4.78 MeV — must
    /// produce **one triton and one alpha**, not a triton alone.
    ///
    /// Methodology: MT=105 on ZA=3006 gives `izr = 3006 + 1 − 1003 = 2004`,
    /// i.e. the residual *is* an alpha, credited by `gaspr.f90:825`. The
    /// physics is textbook and independent of NJOY: ⁶Li + n → ³H + ⁴He is a
    /// two-body final state whose only products are a triton and an alpha.
    /// Pass criterion: σ(MT=205) = σ(MT=207) = σ(MT=105) exactly.
    ///
    /// Result (2026-09-17): both 2.0 b at 1 MeV against a 2.0 b input, i.e.
    /// exact. Before the residual rule was ported this test's He-4 leg read
    /// 0.0 b — the alpha was lost outright.
    #[test]
    fn li6_n_t_alpha_credits_both_products() {
        let recon = recon_za(
            3006.0,
            vec![section(
                MtReaction::Mt105Nt,
                vec![(1.0e5, 2.0), (1.0e6, 2.0)],
            )],
        );
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::H3, 1.0e6) - 2.0).abs() < 1e-12);
        assert!(
            (gas.eval(GasSpecies::He4, 1.0e6) - 2.0).abs() < 1e-12,
            "⁶Li(n,t)⁴He must credit the alpha residual too"
        );
        assert_eq!(gas.eval(GasSpecies::H1, 1.0e6), 0.0);
    }

    /// ³He(n,p)³H — the ³He proportional-counter reaction, Q = +764 keV —
    /// produces one proton **and** one triton.
    ///
    /// `izr = 2003 + 1 − 1001 = 1003`, the triton (`gaspr.f90:823`).
    /// Result (2026-09-17): both legs 5.0 b against a 5.0 b input.
    #[test]
    fn he3_n_p_triton_credits_both_products() {
        let recon = recon_za(
            2003.0,
            vec![section(MtReaction::Mt103Np, vec![(1.0, 5.0), (1.0e6, 5.0)])],
        );
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::H1, 1.0e3) - 5.0).abs() < 1e-12);
        assert!(
            (gas.eval(GasSpecies::H3, 1.0e3) - 5.0).abs() < 1e-12,
            "³He(n,p)³H must credit the triton residual"
        );
    }

    /// ⁹Be(n,2n)⁸Be → 2α. Be-8 is particle-unbound (it breaks up in ~10⁻¹⁶ s),
    /// so NJOY credits the residual as **two** alphas (`gaspr.f90:826`) — the
    /// only residual rule with a multiplicity above one.
    ///
    /// `izr = 9004 … ` — precisely, ZA(⁹Be) = 4009, so
    /// `izr = 4009 + 1 − 2 = 4008`. Without the rule, MT=16 names no ejectile
    /// and beryllium would show **zero** helium production, which is exactly
    /// backwards for the workspace's reflector materials.
    ///
    /// Result (2026-09-17): 1.2 b of He-4 from a 0.6 b (n,2n), i.e. yield 2.
    #[test]
    fn be9_n_2n_residual_is_two_alphas() {
        let recon = recon_za(
            4009.0,
            vec![section(
                MtReaction::Mt16N2n,
                vec![(2.0e6, 0.0), (5.0e6, 0.6)],
            )],
        );
        let gas = GasProduction::from_reconr(&recon);
        assert!(
            (gas.eval(GasSpecies::He4, 5.0e6) - 1.2).abs() < 1e-12,
            "⁸Be residual is two alphas, not one"
        );
        assert_eq!(gas.eval(GasSpecies::H1, 5.0e6), 0.0);
    }

    /// ²H(n,γ)³H — tritium production by radiative capture on deuterium, the
    /// dominant tritium source in a heavy-water moderator.
    ///
    /// MT=102 names no ejectile at all, so this yield comes *entirely* from
    /// the residual rule: `izr = 1002 + 1 − 0 = 1003`. A flat MT-keyed lookup
    /// cannot express it, because the answer depends on the target.
    ///
    /// Result (2026-09-17): 0.0005 b of H-3 from a 0.0005 b capture, yield 1.
    /// The same section on an Fe-56 target yields nothing (residual Fe-57).
    #[test]
    fn deuterium_capture_produces_tritium() {
        let capture = || {
            section(
                MtReaction::Mt102Capture,
                vec![(0.0253, 5.0e-4), (1.0e6, 5.0e-4)],
            )
        };
        let d2 = GasProduction::from_reconr(&recon_za(1002.0, vec![capture()]));
        assert!(
            (d2.eval(GasSpecies::H3, 0.0253) - 5.0e-4).abs() < 1e-15,
            "²H(n,γ)³H must be counted as tritium production"
        );
        let fe = GasProduction::from_reconr(&recon_za(26056.0, vec![capture()]));
        assert_eq!(
            fe.eval(GasSpecies::H3, 0.0253),
            0.0,
            "the same MT on a heavy target must produce nothing — the yield \
             is a function of the target, not of MT alone"
        );
    }

    /// ¹⁰B(n,α)⁷Li — the control/detector reaction, Q = +2.79 MeV. The
    /// residual is ⁷Li (`5010 + 1 − 2004 = 3007`), which is *not* one of the
    /// five gases, so the answer is one alpha and the residual rule is
    /// correctly silent. This is the negative control for the tests above:
    /// the rule must not fire on every light target.
    #[test]
    fn b10_n_alpha_residual_is_not_a_gas() {
        let recon = recon_za(
            5010.0,
            vec![section(
                MtReaction::Mt107NAlpha,
                vec![(0.0253, 3840.0), (1.0e6, 3840.0)],
            )],
        );
        let gas = GasProduction::from_reconr(&recon);
        assert!((gas.eval(GasSpecies::He4, 0.0253) - 3840.0).abs() < 1e-9);
        assert_eq!(gas.eval(GasSpecies::H3, 0.0253), 0.0);
        assert_eq!(gas.eval(GasSpecies::H1, 0.0253), 0.0);
    }

    /// The MF=3 `LR` breakup flag on an inelastic level changes the yield.
    ///
    /// ⁶Li's MT=51-81 carry `LR=32` in ENDF/B-VIII.0 — the level decays by
    /// emitting a deuteron, i.e. the channel is really ⁶Li(n,n'd)⁴He. With
    /// `LR=32` the ejectile sum is `1 + 1002 = 1003`, leaving `izr = 2004`:
    /// **one deuteron and one alpha**. With `LR=0` the same section would be
    /// plain inelastic scattering, leaving ⁶Li itself and no gas at all.
    ///
    /// Result (2026-09-17): LR=32 → 0.3 b of H-2 and 0.3 b of He-4; LR=0 → an
    /// empty grid.
    #[test]
    fn inelastic_lr_breakup_changes_the_yield() {
        let with_lr = GasProduction::from_reconr(&recon_za(
            3006.0,
            vec![section_lr(
                MtReaction::Mt51NnLevel1,
                32,
                vec![(2.0e6, 0.3), (5.0e6, 0.3)],
            )],
        ));
        assert!((with_lr.eval(GasSpecies::H2, 3.0e6) - 0.3).abs() < 1e-12);
        assert!((with_lr.eval(GasSpecies::He4, 3.0e6) - 0.3).abs() < 1e-12);

        let without_lr = GasProduction::from_reconr(&recon_za(
            3006.0,
            vec![section_lr(
                MtReaction::Mt51NnLevel1,
                0,
                vec![(2.0e6, 0.3), (5.0e6, 0.3)],
            )],
        ));
        assert!(
            without_lr.energy.is_empty(),
            "LR=0 inelastic on ⁶Li leaves ⁶Li — no gas"
        );
    }

    /// Every entry in the ejectile table must conserve nucleons and charge
    /// against the gas particles it claims to emit: the ZA subtracted for the
    /// named light ejectiles can never exceed the total ejectile ZA, and each
    /// named particle's own ZA must fit inside it.
    ///
    /// This catches a transcription slip in the 90-entry table
    /// (`gaspr.f90:501-820`) that no single-reaction test would — e.g. writing
    /// `2005` where upstream has `2006`, which would silently shift every
    /// residual by one neutron.
    #[test]
    fn named_ejectiles_fit_inside_the_ejectile_za_sum() {
        for mt in 5..=200 {
            let Some(ch) = gas_channel(mt, 0) else {
                continue;
            };
            let named = i32::from(ch.emitted.p) * 1001
                + i32::from(ch.emitted.d) * 1002
                + i32::from(ch.emitted.t) * 1003
                + i32::from(ch.emitted.he3) * 2003
                + i32::from(ch.emitted.alpha) * 2004;
            assert!(
                named <= ch.ejectile_za,
                "MT={mt}: named ejectiles sum to ZA {named} but only \
                 {} is subtracted from the target",
                ch.ejectile_za
            );
            // Whatever is left over after the named charged particles must be
            // pure neutrons: a non-negative count with zero charge.
            let leftover = ch.ejectile_za - named;
            assert_eq!(
                leftover / 1000,
                0,
                "MT={mt}: {leftover} left after the named ejectiles is not a \
                 whole number of neutrons"
            );
        }
    }
}

/// Run the GASPR card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted(
        "gaspr driver (physics ported — use the module API)",
    ))
}
