// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/conductivity/`:
//   conductivityConstant.{C,H}             conductivityMatproUO2.{C,H}
//   conductivityNfirUO2.{C,H}              conductivityUO2IFA601.{C,H}
//   conductivityUPuO2Brancheria.{C,H}      conductivityUPuO2Kato.{C,H}
//   conductivityUPuO2LanningBeyer.{C,H}    conductivityUPuO2MagniMAMOX.{C,H}
//   conductivityUPuO2Philipponneau.{C,H}   conductivityRelapZy.{C,H}
//   conductivityMolybdenum.{C,H}           conductivitySwindemanHastelloyN.{C,H}
//   conductivityTobbe1515Ti.{C,H}          conductivityPARFUMEBuffer.{C,H}
//   conductivityPARFUMESiC.{C,H}
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Thermal conductivity correlations \[W/(m K)\].
//!
//! # What this module computes
//!
//! The **thermal conductivity** `k` of fuel, cladding and structural materials,
//! in W/(m K), as a function of the local [`MaterialState`] — temperature,
//! burnup, porosity, stoichiometry, plutonium and gadolinia content.
//!
//! Conductivity is the single most influential material property in a
//! fuel-performance calculation: it sets the fuel centreline temperature, which
//! then drives thermal expansion, creep, fission-gas release and, at the limit,
//! melting. Two published UO2 correlations can differ by 20% at the same
//! temperature, so each variant of [`ConductivityModel`] is named for the
//! **author or data source of the fit**, never for the material alone.
//!
//! # Units — raw `f64`, strict SI
//!
//! Inputs come from [`MaterialState`] (temperature in K, burnup in MWd/kgHM —
//! see that type for the full list); the returned value is always in
//! **W/(m K)**. Several of the underlying published fits are written in degrees
//! Celsius or in MWd/tHM; every such conversion is done inside this module, so
//! a caller never has to know which convention a given fit used.
//!
//! # Validity ranges: `value` clamps, `value_checked` reports
//!
//! Every variant declares a temperature validity window via
//! [`ConductivityModel::temperature_range`].
//!
//! - [`ConductivityModel::value`] **clamps the temperature into that window**
//!   before evaluating, so it always returns a finite number in the range the
//!   fit was built for.
//! - [`ConductivityModel::value_checked`] instead returns
//!   [`OffbeatError::OutOfRange`] when the temperature falls outside the window,
//!   and [`OffbeatError::Unphysical`] for a non-positive absolute temperature.
//!
//! **Honest note on "matching upstream".** Upstream OFFBEAT does not clamp:
//! `conductivityRelapZy` prints a warning and extrapolates, and most of the
//! other models perform no range check at all. Clamping in [`value`] is
//! therefore this port's deliberate, documented choice — it removes the silent
//! extrapolation without removing the information, because
//! [`value_checked`] still reports it. Where a correlation *does* clamp
//! internally (porosity cut-offs in the MOX models, the 95%-porosity floor in
//! [`MaterialState::density_fraction`]) that clamp is part of the correlation
//! and is applied by both methods.
//!
//! [`value`]: ConductivityModel::value
//! [`value_checked`]: ConductivityModel::value_checked
//! [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
//! [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical
//!
//! # Where the validity windows come from
//!
//! Only four of the fifteen upstream models state a validity range in their own
//! source (`conductivityRelapZy`: 290–1800 K, and — indirectly, through the
//! matching heat-capacity fits — the Zircaloy, 15-15 Ti and SiC windows).
//! For every other variant the window in [`temperature_range`] is **this
//! port's engineering choice**, and its doc comment says so explicitly. A
//! port-chosen window is a guard rail, not a statement about the underlying
//! experiment.
//!
//! [`temperature_range`]: ConductivityModel::temperature_range
//!
//! # Known deviations from upstream, and known upstream defects
//!
//! - **Burnup units.** Upstream is internally inconsistent about the unit of
//!   its `Bu` field: `conductivityMatproUO2` converts with `Bu/1000/0.881`
//!   (MWd/tUO2 → MWd/kgU), while `conductivityUO2IFA601`,
//!   `conductivityUPuO2LanningBeyer` and `conductivityUPuO2Philipponneau`
//!   convert with `Bu*1e-3` (MWd/tHM → MWd/kgHM) — i.e. the same field is read
//!   with and without the 0.881 heavy-metal fraction. This port takes
//!   [`MaterialState::burnup`] in **MWd/kgHM** and uses it directly in every
//!   correlation, so the inconsistency cannot survive here.
//! - **Dictionary-tunable coefficients are not exposed.** Upstream lets a case
//!   dictionary override every fit coefficient (`par1` … `par13`, `perturb`,
//!   `kappaInf`, …), which exists for uncertainty quantification. This port
//!   hard-codes the published values; a UQ surface can be added later without
//!   changing the physics.
//! - **`conductivityNfirUO2` reads its gadolinia content from a dictionary key
//!   named `GdContent_`** (with a trailing underscore) while
//!   `conductivityMatproUO2` reads `GdContent`. The gadolinia branch of the
//!   NFIR model is therefore almost certainly never exercised upstream, and it
//!   is not continuous as the gadolinia content goes to zero. See
//!   [`ConductivityModel::NfirUo2`].

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// Thermal conductivity \[W/(m K)\] of a fuel, cladding or structural material.
///
/// # What this represents
///
/// One published thermal-conductivity correlation, selected at construction and
/// evaluated per cell against a [`MaterialState`]. Variants are named
/// `<author-or-source><material>`, following the convention set out in
/// [`crate::materials`]: `MatproUo2` is the MATPRO-v11 UO2 fit, `RelapZircaloy`
/// the RELAP Zircaloy fit, and so on.
///
/// # Dispatch
///
/// This is an enum, not a trait object, per the workspace "no trait objects"
/// rule: the set of correlations is closed and known at compile time, adding
/// one is then a compile error at every `match`, and rust-analyzer's
/// go-to-definition works on the variants.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
///
/// // Fresh, 95%-dense UO2 at 1000 K.
/// let mut state = MaterialState::fresh(1000.0);
/// state.porosity = 0.05;
///
/// let k = ConductivityModel::MatproUo2.value(&state);
/// assert!((3.0..4.0).contains(&k), "UO2 near 1000 K is a few W/(m K), got {k}");
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConductivityModel {
    /// Temperature-independent conductivity \[W/(m K)\], the payload value.
    ///
    /// Upstream: `conductivityConstant` (reads `k` from the case dictionary).
    ///
    /// Valid over any temperature — [`temperature_range`] returns
    /// `(0, +inf)` — because a constant is not a fit to anything. Use it for
    /// scoping calculations and for verification cases with an analytical
    /// solution, not for a real material.
    ///
    /// [`temperature_range`]: ConductivityModel::temperature_range
    Constant(f64),

    /// UO2, MATPRO-v11 / modified-NFI fit \[W/(m K)\].
    ///
    /// Upstream: `conductivityMatproUO2`. This is the "modified NFI" form used
    /// by MATPRO-v11 and FRAPCON, a phonon term plus an electronic term:
    ///
    /// ```text
    /// k95 = 1 / (A + B*T + C*Bu + D*gad + (1 - 0.9*exp(-0.04*Bu)) * E*Bu^0.28 * h(T))
    ///       + F/T^2 * exp(-G/T)
    /// h(T) = 1 / (1 + 396*exp(-6380/T))
    /// ```
    ///
    /// with `A = 0.0452`, `B = 2.46e-4` 1/K, `C = 1.87e-3` kgHM/MWd,
    /// `D = 1.1599`, `E = 0.038`, `F = 3.5e9` W K/m, `G = 16360` K, and `k95`
    /// the conductivity at 95% of theoretical density. It is then rescaled to
    /// the actual density fraction `d` by the Maxwell-Euken-type factor
    /// `1.0789 * d / (1 + 0.5*(1 - d))`, which is unity at `d = 0.95` by
    /// construction.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\],
    /// [`burnup`](MaterialState::burnup) \[MWd/kgHM\],
    /// [`gadolinia_fraction`](MaterialState::gadolinia_fraction) \[weight
    /// fraction\], and the porosity/swelling/densification group through the
    /// evolving density fraction described below.
    ///
    /// # Porosity evolution
    ///
    /// Upstream corrects the as-fabricated density fraction by a cell-volume
    /// factor `1 + swelling(intragranular + intergranular) + 3*densification`.
    /// The factor of three is there because upstream's `epsilonDensification`
    /// field is a **linear** strain while its swelling fields are volumetric;
    /// [`MaterialState::densification`] is documented as volumetric, so this
    /// port uses `1 + swelling + densification` with no factor. The result is
    /// floored at 0.05 so a pathological input cannot divide by zero.
    ///
    /// # Validity range
    ///
    /// Upstream states none. This port uses **300–3113 K**, the upper bound
    /// being the melting temperature of unirradiated UO2 as used by MATPRO;
    /// the window is this port's choice, not a bound from the report. No
    /// burnup or gadolinia bound is enforced.
    ///
    /// # Source
    ///
    /// MATPRO-v11 (SCDAP/RELAP5 material properties library) UO2 thermal
    /// conductivity, in the "modified NFI" form of Ohira & Itagaki, as
    /// implemented in upstream `conductivityMatproUO2.C`.
    MatproUo2,

    /// UO2, NFIR (EPRI Nuclear Fuel Industry Research) fit \[W/(m K)\].
    ///
    /// Upstream: `conductivityNfirUO2`. Blends a low-temperature and a
    /// high-temperature phonon branch with a `tanh` ramp centred on 900 °C, and
    /// adds an electronic term:
    ///
    /// ```text
    /// RF   = 0.5*(1 + tanh((t - 900)/150))                 [t in degC]
    /// kLo  = 1/(a + 6.14e-3*Bu - 1.4e-5*Bu^2 + (2.5e-4 - 1.81e-6*Bu)*t)
    /// kHi  = 1/(a + 2.6e-3*Bu + (2.5e-4 - 2.7e-7*Bu)*t)
    /// kEl  = 1.32e-2*exp(1.88e-3*t)
    /// k95  = (1 - RF)*kLo + RF*kHi + kEl
    /// ```
    ///
    /// with `a = 9.592e-2` for un-poisoned UO2. The porosity rescaling is the
    /// temperature-dependent form
    /// `k95 * (1 - (2.58 - 5.8e-4*t)*(1 - d)) / (1 - 0.05*(2.58 - 5.8e-4*t))`,
    /// again unity at `d = 0.95` by construction.
    ///
    /// # Gadolinia branch — do not trust without checking the NFIR report
    ///
    /// When [`gadolinia_fraction`](MaterialState::gadolinia_fraction) is
    /// non-zero, `a` is replaced by a polynomial-times-factor expression in the
    /// gadolinia content. Two problems, both inherited from upstream and both
    /// reproduced here so the port stays faithful:
    ///
    /// 1. The expression contains `tanh(Gd)^0.1`, which tends to **zero** as
    ///    `Gd -> 0`, so `a -> 0` and the conductivity *jumps upward* as an
    ///    infinitesimal amount of burnable poison is added. The branch is
    ///    discontinuous at the origin and does not reduce to the un-poisoned
    ///    `a = 9.592e-2`.
    /// 2. Upstream reads the content from a dictionary key named `GdContent_`
    ///    — with a trailing underscore — while every other model reads
    ///    `GdContent`, so this branch is likely dead code upstream and the
    ///    intended unit (weight fraction or weight per cent) cannot be
    ///    determined from the source. This port applies it as a **weight
    ///    fraction**, matching [`MatproUo2`](Self::MatproUo2).
    ///
    /// The un-poisoned branch (`gadolinia_fraction == 0`) is unaffected by any
    /// of this.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3113 K**, as for
    /// [`MatproUo2`](Self::MatproUo2). Port's choice, not from the report.
    ///
    /// # Source
    ///
    /// EPRI NFIR UO2 thermal conductivity, as implemented in upstream
    /// `conductivityNfirUO2.C`.
    NfirUo2,

    /// UO2, fitted to the Halden IFA-601 instrumented rod \[W/(m K)\].
    ///
    /// Upstream: `conductivityUO2IFA601`.
    ///
    /// ```text
    /// k = 1000 / (40 + 5*Bu + c(Bu)*T) + 1.32e-2*exp(1.88e-3*T)
    /// c(Bu) = 0.24                    for Bu <= 50 MWd/kgHM
    ///       = 0.457 - 0.00433*Bu      for 50 < Bu <= 80
    ///       = 0.11                    for Bu > 80
    /// ```
    ///
    /// The piecewise coefficient is very nearly continuous at both breakpoints
    /// (0.240 vs 0.2405 at 50 MWd/kgHM; 0.1106 vs 0.110 at 80 MWd/kgHM), which
    /// the unit tests check.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] and
    /// [`burnup`](MaterialState::burnup) \[MWd/kgHM\]. Note that unlike
    /// [`MatproUo2`](Self::MatproUo2) this correlation carries **no porosity
    /// rescaling** — the density dependence is baked into the rod-specific fit.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3113 K**. Port's choice. The
    /// fit is a rod-specific one and should not be used outside the burnup
    /// range of the IFA-601 experiment it was fitted to.
    ///
    /// # Source
    ///
    /// Fit to the Halden IFA-601 instrumented fuel rod, as implemented in
    /// upstream `conductivityUO2IFA601.C`.
    Ifa601Uo2,

    /// MOX (U,Pu)O2, Brancheria fit for Pu/HM = 0.25 \[W/(m K)\].
    ///
    /// Upstream: `conductivityUPuO2Brancheria`.
    ///
    /// ```text
    /// k = 100 * D1(p) * ( 1/(2.88 + 0.0252*T) + 5.83e-13*T^3 )
    /// D1 = 1 - 1.5*p                          for p <= 0.05
    ///    = 1 - pc - 10*pc^2, pc = min(p,0.15) for p >  0.05
    /// ```
    ///
    /// `D1` is continuous at `p = 0.05` (both branches give 0.925) and its
    /// porosity argument is cut off at 15%.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] and
    /// [`porosity`](MaterialState::porosity) \[-\]. The fit is for 25% Pu
    /// content and takes no Pu argument; using it for another Pu fraction is
    /// an extrapolation the correlation cannot express.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3000 K**. Port's choice.
    ///
    /// # Source
    ///
    /// Brancheria et al., *Calibration of a fuel-to-cladding gap conductance
    /// model for fast reactor fuel pins*, Hanford Engineering Development
    /// Laboratory, <https://www.osti.gov/servlets/purl/6863540>; cited by
    /// upstream `conductivityUPuO2Brancheria.H`.
    ///
    /// Upstream also multiplies the result by a dictionary-set `perturb`
    /// factor (default 1.0) for uncertainty quantification; that knob is not
    /// exposed here.
    BrancheriaMox,

    /// MOX (U,Pu)O2, Kato fit with minor actinides \[W/(m K)\].
    ///
    /// Upstream: `conductivityUPuO2Kato`, as used by Novascone et al. (2018).
    ///
    /// ```text
    /// den = 1.595e-2 + 2.713*x + 3.583e-1*cAm + 6.317e-2*cNp
    ///       + (-2.625*x + 2.493)*1e-4*T
    /// k   = 1/den + 1.541e11 * T^-2.5 * exp(-1.522e4/T)
    /// ```
    ///
    /// where `x = 2 - O/M` is the **hypo**stoichiometry (positive for
    /// oxygen-deficient fuel), and `cAm`, `cNp` are americium and neptunium
    /// concentrations in mass per mass of fuel. The porosity correction is
    /// Barani's Maxwell-Eucken form with a helium-filled pore conductivity of
    /// 0.69 W/(m K):
    ///
    /// ```text
    /// k_eff = k * (kHe + 2k - 2p(k - kHe)) / (kHe + 2k + p(k - kHe))
    /// ```
    ///
    /// which returns `k` exactly at `p = 0` and `kHe` exactly at `p = 1`.
    ///
    /// # Payload — minor actinide content
    ///
    /// [`MaterialState`] carries no americium or neptunium field, so the two
    /// minor-actinide **atom fractions of the heavy metal** (atoms of Am, resp.
    /// Np, per atom of U + Pu + Am + Np) are carried on the variant. Pass
    /// `0.0` for both for ordinary MOX. They are converted to mass fractions
    /// internally with upstream's approximate factors `k_Am = 1.12` and
    /// `k_Np = 1.14`.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\],
    /// [`porosity`](MaterialState::porosity) \[-\] and
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\] (this
    /// crate's `x` in `(U,Pu)O_{2+x}`, which is the **negative** of upstream's
    /// `x = 2 - O/M`). Kato's fit is independent of the Pu content, so
    /// [`pu_fraction`](MaterialState::pu_fraction) is not used — upstream
    /// computes a Pu concentration in this model and then never reads it.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3000 K**. Port's choice.
    ///
    /// # Source
    ///
    /// S. Novascone et al., *Modeling porosity migration in LWR and fast
    /// reactor MOX fuel using the finite element method*, Journal of Nuclear
    /// Materials 508 (2018) 226–236,
    /// <https://doi.org/10.1016/j.jnucmat.2018.05.041>; cited by upstream
    /// `conductivityUPuO2Kato.H`.
    KatoMox {
        /// Americium content \[atoms Am per atom of heavy metal\]. Zero for
        /// ordinary MOX.
        americium_atom_fraction: f64,
        /// Neptunium content \[atoms Np per atom of heavy metal\]. Zero for
        /// ordinary MOX.
        neptunium_atom_fraction: f64,
    },

    /// MOX (U,Pu)O2, Lanning & Beyer fit \[W/(m K)\].
    ///
    /// Upstream: `conductivityUPuO2LanningBeyer`. Structurally the MATPRO UO2
    /// form with stoichiometry-dependent `A` and `B` coefficients:
    ///
    /// ```text
    /// A   = 0.035 + 2.8*x                     [x = 2 - O/M]
    /// B   = (2.86 - 7.15*x)*1e-4
    /// k95 = 1/(A + B*T + 1.87e-3*Bu + (1 - 0.9*exp(-0.04*Bu))*0.038*Bu^0.28*h(T))
    ///       + 1.5e9/T^2 * exp(-13520/T)
    /// h(T) = 1/(1 + 396*exp(-6380/T))
    /// ```
    ///
    /// rescaled to the actual porosity by `1.0789*(1 - p)/(1 + 0.5*p)`, unity
    /// at `p = 0.05` by construction.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\],
    /// [`burnup`](MaterialState::burnup) \[MWd/kgHM\],
    /// [`porosity`](MaterialState::porosity) \[-\] and
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\]. Upstream
    /// takes the porosity from `1 - densityFraction` in the case dictionary,
    /// i.e. it is fixed for the run; this port takes it from the state, so it
    /// can evolve.
    ///
    /// # Validity range
    ///
    /// Upstream states none — the header's `Description` block is empty. This
    /// port uses **300–3000 K**. Port's choice.
    ///
    /// # Source
    ///
    /// Lanning & Beyer MOX thermal conductivity, as implemented in upstream
    /// `conductivityUPuO2LanningBeyer.C`. Upstream cites no report for it;
    /// the coefficients are reproduced from that source file.
    LanningBeyerMox,

    /// Minor-actinide-bearing MOX, Magni MAMOX correlation \[W/(m K)\].
    ///
    /// Upstream: `conductivityUPuO2MagniMAMOX`. A fresh-fuel conductivity `k0`
    /// with composition-dependent phonon coefficients, a porosity factor
    /// `(1 - p)^2.5`, and an exponential relaxation towards an irradiated
    /// asymptote:
    ///
    /// ```text
    /// k0 = [ 1/(A(x, cPu, cAm, cNp) + B(cPu, cAm, cNp)*T) + 5.27e9/T^2*exp(-17109.5/T) ]
    ///      * (1 - p)^2.5
    /// k  = kInf + (k0 - kInf)*exp(-Bu/phi),  kInf = 1.755 W/(m K)
    /// ```
    ///
    /// with `phi = 128.75` MWd/kgHM (upstream writes it as `128.75e3` in its
    /// own MWd/tHM burnup unit). At zero burnup `k = k0` exactly; as burnup
    /// grows without bound `k -> kInf`.
    ///
    /// # Payload — minor actinide content
    ///
    /// As for [`KatoMox`](Self::KatoMox): the Am and Np **atom fractions of the
    /// heavy metal**, converted internally to mass fractions with upstream's
    /// factors `k_Pu = 1.13`, `k_Am = 1.12`, `k_Np = 1.14`. Pass `0.0` for
    /// both for ordinary MOX — in which case only the plutonium term of `A`
    /// and `B` is active.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\],
    /// [`burnup`](MaterialState::burnup) \[MWd/kgHM\],
    /// [`porosity`](MaterialState::porosity) \[-\] (cut off at 0.95),
    /// [`pu_fraction`](MaterialState::pu_fraction) \[atoms Pu per atom HM\] and
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\].
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3000 K**. Port's choice.
    ///
    /// # Source
    ///
    /// A. Magni et al., *Modelling of thermal conductivity and melting
    /// behaviour of minor actinide-MOX fuels and assessment against
    /// experimental and molecular dynamics data*, Journal of Nuclear Materials
    /// (2021), <https://doi.org/10.1016/j.jnucmat.2021.153312>; cited by
    /// upstream `conductivityUPuO2MagniMAMOX.H`.
    MagniMamox {
        /// Americium content \[atoms Am per atom of heavy metal\]. Zero for
        /// ordinary MOX.
        americium_atom_fraction: f64,
        /// Neptunium content \[atoms Np per atom of heavy metal\]. Zero for
        /// ordinary MOX.
        neptunium_atom_fraction: f64,
    },

    /// MOX (U,Pu)O2-x, Philipponneau (1992) fit \[W/(m K)\].
    ///
    /// Upstream: `conductivityUPuO2Philipponneau`.
    ///
    /// ```text
    /// A     = 1.528*sqrt(x + 0.00931) - 0.1055 + 0.44*tau
    /// alpha = (1/0.864) * (1 - p)/(1 + 2p)
    /// k     = ( 1/(A + 2.885e-4*T) + 76.38e-12*T^3 ) * alpha
    /// ```
    ///
    /// where `x = 2 - O/M` is the hypostoichiometry and `tau` the fractional
    /// burnup in FIMA. Upstream converts burnup to FIMA with the MOX-specific
    /// factor of 9.5 GWd/tHM per %FIMA; with this crate's MWd/kgHM burnup that
    /// becomes `tau = Bu/950`. `alpha` is unity at `p = 0.05` by construction.
    ///
    /// The square root is guarded with a floor of zero: for
    /// **hyper**stoichiometric fuel with `oxygen_deviation > 0.00931` the
    /// argument goes negative and upstream would produce a NaN.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\],
    /// [`burnup`](MaterialState::burnup) \[MWd/kgHM\],
    /// [`porosity`](MaterialState::porosity) \[-\] (cut off at 0.95) and
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\]. As
    /// upstream's header notes, the MOX conductivity in this correlation is
    /// **independent of the Pu content**.
    ///
    /// # Validity range
    ///
    /// Upstream's header states *500 K < T < melting T*. This port uses
    /// **500–3000 K**: the lower bound is the stated one, the upper bound is
    /// the port's stand-in for the (composition-dependent) melting
    /// temperature, which the reference does not give as a number.
    ///
    /// # Source
    ///
    /// Y. Philipponneau, *Thermal conductivity of (U,Pu)O2-x mixed oxide
    /// fuel*, Journal of Nuclear Materials (1992),
    /// <https://doi.org/10.1016/0022-3115(92)90470-6>; FIMA conversion factor
    /// from <https://doi.org/10.1016/j.pnucene.2017.03.016>. Both cited by
    /// upstream `conductivityUPuO2Philipponneau.H`.
    PhilipponneauMox,

    /// Zircaloy cladding, RELAP fit \[W/(m K)\].
    ///
    /// Upstream: `conductivityRelapZy`.
    ///
    /// ```text
    /// k = 7.51 + 2.09e-2*T - 1.45e-5*T^2 + 7.67e-9*T^3
    /// ```
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// **290–1800 K**, stated by upstream, which prints a warning and then
    /// extrapolates anyway. This port clamps in [`value`] and reports
    /// [`OutOfRange`] in [`value_checked`].
    ///
    /// [`value`]: ConductivityModel::value
    /// [`value_checked`]: ConductivityModel::value_checked
    /// [`OutOfRange`]: crate::error::OffbeatError::OutOfRange
    ///
    /// # Source
    ///
    /// RELAP Zircaloy thermal conductivity, as implemented in upstream
    /// `conductivityRelapZy.C`.
    RelapZircaloy,

    /// Molybdenum \[W/(m K)\].
    ///
    /// Upstream: `conductivityMolybdenum`.
    ///
    /// ```text
    /// k = 9.128e-6*T^2 - 4.945e-2*T + 152
    /// ```
    ///
    /// Note the *negative* linear term: molybdenum's conductivity falls with
    /// temperature from about 138 W/(m K) at room temperature, the quadratic
    /// term turning it back up only above roughly 2700 K.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–2800 K**, the upper bound
    /// being below molybdenum's melting point (about 2896 K). Port's choice.
    ///
    /// # Source
    ///
    /// Upstream `conductivityMolybdenum.C`; upstream cites no report.
    Molybdenum,

    /// Hastelloy N, Swindeman fit \[W/(m K)\].
    ///
    /// Upstream: `conductivitySwindemanHastelloyN`.
    ///
    /// ```text
    /// k = 8.431 + 0.0205*(T - 273.15)     [T in K]
    /// ```
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–1200 K**, a conservative
    /// engineering window that covers the alloy's service range and stays well
    /// below its melting range. Port's choice, not a bound from Swindeman.
    ///
    /// # Source
    ///
    /// R. W. Swindeman's Hastelloy-N property work, as implemented in upstream
    /// `conductivitySwindemanHastelloyN.C` (upstream gives no full citation).
    SwindemanHastelloyN,

    /// 15-15 Ti austenitic stainless cladding, Tobbe (1975) fit \[W/(m K)\].
    ///
    /// Upstream: `conductivityTobbe1515Ti`.
    ///
    /// ```text
    /// k = 13.95 + 1.163e-2*(T - 273.15)   [T in K]
    /// ```
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// Upstream states none for the conductivity. This port adopts
    /// **293–1273 K**, the window upstream *does* state for the Banerjee
    /// 15-15 Ti heat-capacity fit of the same alloy. Port's choice, not
    /// Tobbe's stated range.
    ///
    /// # Source
    ///
    /// Tobbe correlation (1975) for 15-15 Ti, as implemented in upstream
    /// `conductivityTobbe1515Ti.C`.
    Tobbe1515Ti,

    /// TRISO buffer layer (porous pyrolytic carbon), PARFUME model
    /// \[W/(m K)\].
    ///
    /// Upstream: `conductivityPARFUMEBuffer`. A density-interpolating form
    /// that passes exactly through two anchor points — the as-fabricated
    /// buffer (`rho_init`, `k_init`) and fully dense pyrocarbon
    /// (`rho_theo`, `k_theo`):
    ///
    /// ```text
    /// k = k_init*k_theo*rho_theo*(rho_theo - rho_init)
    ///     / ( k_theo*rho_theo*(rho_theo - rho) + k_init*rho*(rho - rho_init) )
    /// ```
    ///
    /// # Inputs used
    ///
    /// [`porosity`](MaterialState::porosity) \[-\] only — **this model is
    /// temperature-independent**. Upstream reads the current density from a
    /// `rho` field on the mesh; this port reconstructs it from the state as
    /// `rho = theoretical_density * (1 - porosity)`, which is the definition
    /// of porosity for a single-phase solid and is the only density
    /// information [`MaterialState`] carries.
    ///
    /// # Validity range
    ///
    /// None — [`temperature_range`] returns `(0, +inf)` because there is no
    /// temperature dependence to invalidate.
    ///
    /// [`temperature_range`]: ConductivityModel::temperature_range
    ///
    /// # Source
    ///
    /// PARFUME (INL TRISO fuel-performance code) buffer conductivity model, as
    /// implemented in upstream `conductivityPARFUMEBuffer.C`. Use
    /// [`ConductivityModel::parfume_buffer_default`] for upstream's default
    /// parameters.
    ParfumeBuffer {
        /// As-fabricated buffer density \[kg/m^3\] (upstream default 1000).
        initial_density: f64,
        /// Theoretical (fully dense) pyrocarbon density \[kg/m^3\] (upstream
        /// default 2250).
        theoretical_density: f64,
        /// Conductivity at the as-fabricated density \[W/(m K)\] (upstream
        /// default 0.5).
        initial_conductivity: f64,
        /// Conductivity at theoretical density \[W/(m K)\] (upstream default
        /// 4.0).
        theoretical_conductivity: f64,
    },

    /// TRISO silicon-carbide layer, PARFUME model \[W/(m K)\].
    ///
    /// Upstream: `conductivityPARFUMESiC`.
    ///
    /// ```text
    /// k = 17885/T + 2
    /// ```
    ///
    /// The `1/T` form is the irradiated-SiC behaviour PARFUME uses: about
    /// 62 W/(m K) at 300 K falling to about 16 W/(m K) at 1273 K.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// Upstream states none for the conductivity. This port adopts
    /// **200–2400 K**, the window upstream states for the Snead SiC
    /// heat-capacity fit of the same material. Port's choice.
    ///
    /// # Source
    ///
    /// PARFUME manual, SiC conductivity, as implemented in upstream
    /// `conductivityPARFUMESiC.C`. (Upstream's header calls it "PyC material
    /// from PARFUME manual" while the class and the parameters are the SiC
    /// ones — the header text appears to be a copy-paste slip.)
    ParfumeSiC,
}

impl ConductivityModel {
    /// [`ParfumeBuffer`](Self::ParfumeBuffer) with upstream's default
    /// parameters.
    ///
    /// `initial_density = 1000` kg/m^3, `theoretical_density = 2250` kg/m^3,
    /// `initial_conductivity = 0.5` W/(m K), `theoretical_conductivity = 4.0`
    /// W/(m K) — the `lookupOrDefault` values in upstream
    /// `conductivityPARFUMEBuffer.C`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
    ///
    /// let buffer = ConductivityModel::parfume_buffer_default();
    /// assert!(matches!(buffer, ConductivityModel::ParfumeBuffer { .. }));
    /// ```
    #[must_use]
    pub const fn parfume_buffer_default() -> Self {
        Self::ParfumeBuffer {
            initial_density: 1000.0,
            theoretical_density: 2250.0,
            initial_conductivity: 0.5,
            theoretical_conductivity: 4.0,
        }
    }

    /// Short human-readable name of the correlation, for error messages and
    /// logs.
    ///
    /// The names mirror the upstream OpenFOAM class names closely enough to
    /// grep for, e.g. `"MATPRO UO2 conductivity"` for upstream's
    /// `conductivityMatproUO2`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Constant(_) => "constant conductivity",
            Self::MatproUo2 => "MATPRO UO2 conductivity",
            Self::NfirUo2 => "NFIR UO2 conductivity",
            Self::Ifa601Uo2 => "IFA-601 UO2 conductivity",
            Self::BrancheriaMox => "Brancheria (U,Pu)O2 conductivity",
            Self::KatoMox { .. } => "Kato (U,Pu)O2 conductivity",
            Self::LanningBeyerMox => "Lanning-Beyer (U,Pu)O2 conductivity",
            Self::MagniMamox { .. } => "Magni MAMOX (U,Pu)O2 conductivity",
            Self::PhilipponneauMox => "Philipponneau (U,Pu)O2-x conductivity",
            Self::RelapZircaloy => "RELAP Zircaloy conductivity",
            Self::Molybdenum => "molybdenum conductivity",
            Self::SwindemanHastelloyN => "Swindeman Hastelloy-N conductivity",
            Self::Tobbe1515Ti => "Tobbe 15-15 Ti conductivity",
            Self::ParfumeBuffer { .. } => "PARFUME buffer conductivity",
            Self::ParfumeSiC => "PARFUME SiC conductivity",
        }
    }

    /// Temperature validity window `(low, high)` \[K\] of this correlation.
    ///
    /// See each variant's doc comment for whether the window is **stated by
    /// upstream / the reference** (only [`RelapZircaloy`](Self::RelapZircaloy)
    /// and the lower bound of [`PhilipponneauMox`](Self::PhilipponneauMox) are)
    /// or is **this port's engineering choice** (everything else). A
    /// port-chosen window is a guard rail against nonsense extrapolation, not
    /// a claim about the experiment behind the fit.
    ///
    /// [`Constant`](Self::Constant) and [`ParfumeBuffer`](Self::ParfumeBuffer)
    /// have no temperature dependence and return `(0.0, f64::INFINITY)`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
    ///
    /// // Upstream conductivityRelapZy warns outside 290 < T < 1800 K.
    /// assert_eq!(ConductivityModel::RelapZircaloy.temperature_range(), (290.0, 1800.0));
    /// ```
    #[must_use]
    pub const fn temperature_range(&self) -> (f64, f64) {
        match self {
            Self::Constant(_) | Self::ParfumeBuffer { .. } => (0.0, f64::INFINITY),
            // UO2 correlations: up to the melting temperature of unirradiated
            // UO2 used by MATPRO. Port's choice; upstream states no range.
            Self::MatproUo2 | Self::NfirUo2 | Self::Ifa601Uo2 => (300.0, 3113.0),
            // MOX correlations. Port's choice, except Philipponneau's lower
            // bound of 500 K which upstream's header does state.
            Self::BrancheriaMox
            | Self::KatoMox { .. }
            | Self::LanningBeyerMox
            | Self::MagniMamox { .. } => (300.0, 3000.0),
            Self::PhilipponneauMox => (500.0, 3000.0),
            // Stated by upstream conductivityRelapZy.C.
            Self::RelapZircaloy => (290.0, 1800.0),
            Self::Molybdenum => (300.0, 2800.0),
            Self::SwindemanHastelloyN => (300.0, 1200.0),
            Self::Tobbe1515Ti => (293.0, 1273.0),
            Self::ParfumeSiC => (200.0, 2400.0),
        }
    }

    /// Thermal conductivity \[W/(m K)\], **clamping** the temperature into
    /// [`temperature_range`](Self::temperature_range).
    ///
    /// # Clamping
    ///
    /// If `state.temperature` lies outside the correlation's validity window,
    /// this method evaluates the fit **at the nearest endpoint of the window**
    /// rather than extrapolating. It therefore always returns a finite number
    /// from the region the fit was built for, and it never signals that the
    /// clamp happened. Use [`value_checked`](Self::value_checked) when you need
    /// to know.
    ///
    /// Clamps that belong to the correlation itself — the 95%-porosity floor of
    /// [`MaterialState::density_fraction`], the 15% porosity cut-off in
    /// [`BrancheriaMox`](Self::BrancheriaMox), the 95% cut-off in
    /// [`MagniMamox`](Self::MagniMamox) and
    /// [`PhilipponneauMox`](Self::PhilipponneauMox) — are applied by both
    /// methods, because they are part of the published model.
    ///
    /// # Units
    ///
    /// Returns W/(m K). Inputs are read from `state` in that type's documented
    /// units (K, MWd/kgHM, dimensionless fractions).
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
    ///
    /// let model = ConductivityModel::RelapZircaloy;
    /// // 5000 K is far above the 1800 K upper bound; it is clamped there.
    /// let clamped = model.value(&MaterialState::fresh(5000.0));
    /// let at_bound = model.value(&MaterialState::fresh(1800.0));
    /// assert_eq!(clamped, at_bound);
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let (low, high) = self.temperature_range();
        let mut clamped = *state;
        clamped.temperature = state.temperature.clamp(low, high);
        self.evaluate(&clamped)
    }

    /// Thermal conductivity \[W/(m K)\], **reporting** an out-of-range
    /// temperature instead of clamping it.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] if `state.temperature` is not a positive
    ///   absolute temperature (zero, negative or NaN).
    /// - [`OffbeatError::OutOfRange`] if `state.temperature` falls outside
    ///   [`temperature_range`](Self::temperature_range). Only the temperature
    ///   is range-checked: upstream states no burnup, porosity or composition
    ///   bound for any of these fits, so this port does not invent one.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
    ///
    /// let model = ConductivityModel::RelapZircaloy;
    /// assert!(model.value_checked(&MaterialState::fresh(600.0)).is_ok());
    /// assert!(model.value_checked(&MaterialState::fresh(5000.0)).is_err());
    /// ```
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        // NaN must be rejected too, hence the explicit `is_nan` rather than a
        // single comparison.
        if state.temperature.is_nan() || state.temperature <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: self.name(),
                value: state.temperature,
                unit: "K",
                reason: "absolute temperature must be positive",
            });
        }

        let (low, high) = self.temperature_range();
        if state.temperature < low || state.temperature > high {
            return Err(OffbeatError::OutOfRange {
                quantity: self.name(),
                value: state.temperature,
                low,
                high,
                unit: "K",
            });
        }

        Ok(self.evaluate(state))
    }

    /// Evaluate the correlation with no range handling at all.
    ///
    /// Private on purpose: the two public entry points differ only in what they
    /// do about the validity window, so the physics lives in exactly one place.
    fn evaluate(&self, state: &MaterialState) -> f64 {
        let t = state.temperature;

        match *self {
            Self::Constant(k) => k,

            Self::MatproUo2 => {
                let bu = state.burnup;
                let gad = state.gadolinia_fraction;

                let phonon_denominator = 0.0452
                    + 2.46e-4 * t
                    + 1.87e-3 * bu
                    + 1.1599 * gad
                    + (1.0 - 0.9 * (-0.04 * bu).exp())
                        * 0.038
                        * bu.powf(0.28)
                        * (1.0 / (1.0 + 396.0 * (-6380.0 / t).exp()));
                let electronic = 3.5e9 / (t * t) * (-16360.0 / t).exp();
                let k95 = 1.0 / phonon_denominator + electronic;

                let density_fraction = state.density_fraction() / volume_correction(state);
                k95 * 1.0789 * density_fraction / (1.0 + 0.5 * (1.0 - density_fraction))
            }

            Self::NfirUo2 => {
                let bu = state.burnup;
                let tc = t - 273.15;
                let gad = state.gadolinia_fraction;

                // See the variant doc comment: this branch is discontinuous at
                // gad -> 0 and its unit is not determinable from upstream. It
                // is reproduced faithfully, not repaired.
                let base = if gad > 0.0 {
                    let gd_ir = 0.1197 * gad
                        + gad.tanh().powf(0.1)
                        + 1.214167e-2 * gad
                        + 5.40625e-4 * gad * gad
                        - 5.182292e-5 * gad * gad * gad;
                    let gd_fac =
                        (0.027273 * gad + 0.652_272_73) - 2.25e-2 * gad * (1.0e-5 * bu).tanh();
                    gd_ir * gd_fac
                } else {
                    9.592e-2
                };

                let ramp = 0.5 * (1.0 + ((tc - 900.0) / 150.0).tanh());
                let phonon_low =
                    1.0 / (base + 6.14e-3 * bu - 1.4e-5 * bu * bu + (2.5e-4 - 1.81e-6 * bu) * tc);
                let phonon_high = 1.0 / (base + 2.6e-3 * bu + (2.5e-4 - 2.7e-7 * bu) * tc);
                let electronic = 1.32e-2 * (1.88e-3 * tc).exp();
                let k95 = (1.0 - ramp) * phonon_low + ramp * phonon_high + electronic;

                let density_fraction = state.density_fraction() / volume_correction(state);
                let porosity_slope = 2.58 - 5.8e-4 * tc;
                k95 * (1.0 - porosity_slope * (1.0 - density_fraction))
                    / (1.0 - 0.05 * porosity_slope)
            }

            Self::Ifa601Uo2 => {
                let bu = state.burnup;
                let phonon_offset = 40.0 + 5.0 * bu;
                let phonon_slope = if bu > 50.0 {
                    if bu <= 80.0 {
                        0.457 - 0.00433 * bu
                    } else {
                        0.11
                    }
                } else {
                    0.24
                };
                // Upstream computes in kW/(m K) and multiplies by 1000.
                1000.0 * (1.0 / (phonon_offset + phonon_slope * t) + 1.32e-5 * (1.88e-3 * t).exp())
            }

            Self::BrancheriaMox => {
                let p = state.porosity;
                let porosity_factor = if p <= 0.05 {
                    1.0 - 1.5 * p
                } else {
                    let pc = p.min(0.15);
                    1.0 - pc - 10.0 * pc * pc
                };
                1.0e2 * (porosity_factor * (1.0 / (2.88 + 0.0252 * t) + 5.83e-13 * t * t * t))
            }

            Self::KatoMox {
                americium_atom_fraction,
                neptunium_atom_fraction,
            } => {
                // Upstream's x is the hypostoichiometry 2 - O/M; this crate's
                // oxygen_deviation is the x in (U,Pu)O_{2+x}. Opposite sign.
                let x = -state.oxygen_deviation;
                let c_w_am = americium_atom_fraction / 1.12;
                let c_w_np = neptunium_atom_fraction / 1.14;

                let denominator = 1.595e-2
                    + 2.713 * x
                    + 3.583e-1 * c_w_am
                    + 6.317e-2 * c_w_np
                    + (-2.625 * x + 2.493) * 1.0e-4 * t;
                let k = 1.0 / denominator + 1.541e11 / t.powf(2.5) * (-1.522e4 / t).exp();

                // Barani porosity correction with helium-filled pores.
                let kappa_he = 0.69;
                let p = state.porosity;
                k * (kappa_he + 2.0 * k - 2.0 * p * (k - kappa_he))
                    / (kappa_he + 2.0 * k + p * (k - kappa_he))
            }

            Self::LanningBeyerMox => {
                let bu = state.burnup;
                let x = -state.oxygen_deviation;

                let a = 0.035 + 2.8 * x;
                let b = (2.86 - 7.15 * x) * 1.0e-4;
                let h = 1.0 / (1.0 + 396.0 * (-6380.0 / t).exp());
                let k95 = 1.0
                    / (a + b * t
                        + 1.87e-3 * bu
                        + (1.0 - 0.9 * (-0.04 * bu).exp()) * 0.038 * bu.powf(0.28) * h)
                    + 1.5e9 / (t * t) * (-13520.0 / t).exp();

                let p = state.porosity;
                k95 * 1.0789 * (1.0 - p) / (1.0 + 0.5 * p)
            }

            Self::MagniMamox {
                americium_atom_fraction,
                neptunium_atom_fraction,
            } => {
                let bu = state.burnup;
                let p = state.porosity.min(0.95);
                let x = -state.oxygen_deviation;

                let c_w_pu = state.pu_fraction / 1.13;
                let c_w_am = americium_atom_fraction / 1.12;
                let c_w_np = neptunium_atom_fraction / 1.14;

                let a =
                    0.01926 + 1.06e-6 * x + 2.63e-8 * c_w_pu + 0.596 * c_w_am + 2.22e-14 * c_w_np;
                let b = 2.39e-4 + 1.37e-13 * c_w_pu + 5.47e-4 * c_w_am + 2.48e-14 * c_w_np;

                let phonon = 1.0 / (a + b * t);
                let electronic = 5.27e9 / (t * t) * (-17109.5 / t).exp();
                let k0 = (phonon + electronic) * (1.0 - p).powf(2.5);

                // kappa_inf = 1.755 W/(m K); phi = 128.75 MWd/kgHM (upstream
                // 128.75e3 in its MWd/tHM burnup unit).
                let kappa_inf = 1.755;
                kappa_inf + (k0 - kappa_inf) * (-bu / 128.75).exp()
            }

            Self::PhilipponneauMox => {
                // Upstream: Bu[MWd/tHM] * 1e-3 / 9.5 / 100 -> FIMA fraction.
                // With burnup in MWd/kgHM that is Bu/950.
                let tau = state.burnup / 950.0;
                let p = state.porosity.min(0.95);
                let x = -state.oxygen_deviation;

                // Guarded: upstream NaNs for oxygen_deviation > 0.00931.
                let a = 1.528 * (x + 0.00931).max(0.0).sqrt() - 0.1055 + 0.44 * tau;
                let alpha = (1.0 / 0.864) * (1.0 - p) / (1.0 + 2.0 * p);
                (1.0 / (a + 2.885e-4 * t) + 76.38e-12 * t * t * t) * alpha
            }

            Self::RelapZircaloy => 7.51 + 2.09e-2 * t - 1.45e-5 * t * t + 7.67e-9 * t * t * t,

            Self::Molybdenum => 9.128e-6 * t * t - 4.945e-2 * t + 152.0,

            Self::SwindemanHastelloyN => 8.431 + 0.0205 * (t - 273.15),

            Self::Tobbe1515Ti => 13.95 + 1.163e-2 * (t - 273.15),

            Self::ParfumeBuffer {
                initial_density,
                theoretical_density,
                initial_conductivity,
                theoretical_conductivity,
            } => {
                // Upstream reads rho from the mesh; MaterialState carries
                // porosity, so reconstruct rho = rho_theo*(1 - porosity).
                let rho = theoretical_density * (1.0 - state.porosity);
                initial_conductivity
                    * theoretical_conductivity
                    * theoretical_density
                    * (theoretical_density - initial_density)
                    / (theoretical_conductivity * theoretical_density * (theoretical_density - rho)
                        + initial_conductivity * rho * (rho - initial_density))
            }

            Self::ParfumeSiC => 17885.0 / t + 2.0,
        }
    }
}

/// Cell-volume correction factor \[-\] for the porosity evolution of oxide
/// fuel.
///
/// Upstream's `conductivityMatproUO2` / `conductivityNfirUO2` scale the
/// as-fabricated density fraction by `1 + swelling + 3*densification`, the
/// factor of three converting upstream's *linear* densification strain to a
/// volumetric one. [`MaterialState::densification`] is already volumetric, so
/// no factor appears here.
///
/// Floored at 0.05 so a pathological state (densification more negative than
/// -1) cannot produce a division by zero. Dimensionless.
fn volume_correction(state: &MaterialState) -> f64 {
    (1.0 + state.swelling + state.densification).max(0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// State helper: fresh fuel at 95% of theoretical density.
    fn fresh_95_percent_dense(temperature: f64) -> MaterialState {
        let mut state = MaterialState::fresh(temperature);
        state.porosity = 0.05;
        state
    }

    /// Every variant, for coverage sweeps.
    fn all_models() -> Vec<ConductivityModel> {
        vec![
            ConductivityModel::Constant(3.0),
            ConductivityModel::MatproUo2,
            ConductivityModel::NfirUo2,
            ConductivityModel::Ifa601Uo2,
            ConductivityModel::BrancheriaMox,
            ConductivityModel::KatoMox {
                americium_atom_fraction: 0.0,
                neptunium_atom_fraction: 0.0,
            },
            ConductivityModel::LanningBeyerMox,
            ConductivityModel::MagniMamox {
                americium_atom_fraction: 0.0,
                neptunium_atom_fraction: 0.0,
            },
            ConductivityModel::PhilipponneauMox,
            ConductivityModel::RelapZircaloy,
            ConductivityModel::Molybdenum,
            ConductivityModel::SwindemanHastelloyN,
            ConductivityModel::Tobbe1515Ti,
            ConductivityModel::parfume_buffer_default(),
            ConductivityModel::ParfumeSiC,
        ]
    }

    // ---------------------------------------------------------------------
    // Cross-cutting contract tests
    // ---------------------------------------------------------------------

    /// **Self-consistency check, not validation.**
    ///
    /// Every variant must return a finite, strictly positive conductivity at a
    /// temperature inside its own validity window. A negative or non-finite
    /// conductivity would make the heat-conduction matrix indefinite, so this
    /// is a hard structural requirement of the whole family, independent of any
    /// reference data.
    ///
    /// Methodology: for each variant, evaluate at the midpoint of
    /// [`ConductivityModel::temperature_range`] (or 1000 K where the window is
    /// unbounded), with fresh 95%-dense fuel. Pass criterion: finite and
    /// `> 0`.
    #[test]
    fn every_model_is_finite_and_positive_inside_its_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();
            let t = if high.is_finite() {
                0.5 * (low + high)
            } else {
                1000.0
            };
            let k = model.value(&fresh_95_percent_dense(t));
            assert!(
                k.is_finite() && k > 0.0,
                "{} gave k = {k} W/(m K) at T = {t} K",
                model.name()
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// `value` must clamp: evaluating far above the upper bound must give
    /// exactly the same number as evaluating at the bound. This is the
    /// documented contract of [`ConductivityModel::value`], and it is what
    /// distinguishes it from upstream's warn-and-extrapolate behaviour.
    #[test]
    fn value_clamps_at_both_ends_of_the_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();
            if !high.is_finite() {
                continue;
            }
            assert_eq!(
                model.value(&fresh_95_percent_dense(high + 1000.0)),
                model.value(&fresh_95_percent_dense(high)),
                "{} did not clamp above its upper bound",
                model.name()
            );
            assert_eq!(
                model.value(&fresh_95_percent_dense(low * 0.5)),
                model.value(&fresh_95_percent_dense(low)),
                "{} did not clamp below its lower bound",
                model.name()
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// `value_checked` must report exactly the window `temperature_range`
    /// advertises: `Ok` inside, [`OffbeatError::OutOfRange`] outside, and
    /// [`OffbeatError::Unphysical`] for a non-positive absolute temperature.
    #[test]
    fn value_checked_reports_the_advertised_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();

            assert!(matches!(
                model.value_checked(&fresh_95_percent_dense(-1.0)),
                Err(OffbeatError::Unphysical { .. })
            ));

            if low > 0.0 {
                assert!(
                    matches!(
                        model.value_checked(&fresh_95_percent_dense(low * 0.5)),
                        Err(OffbeatError::OutOfRange { .. })
                    ),
                    "{} accepted a temperature below its lower bound",
                    model.name()
                );
            }
            if high.is_finite() {
                assert!(
                    matches!(
                        model.value_checked(&fresh_95_percent_dense(high + 1.0)),
                        Err(OffbeatError::OutOfRange { .. })
                    ),
                    "{} accepted a temperature above its upper bound",
                    model.name()
                );
                let inside = 0.5 * (low + high);
                assert!(
                    model.value_checked(&fresh_95_percent_dense(inside)).is_ok(),
                    "{} rejected a temperature inside its own window",
                    model.name()
                );
            }
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Inside the validity window `value` and `value_checked` must agree
    /// bit-for-bit — they share one `evaluate` and differ only in range
    /// handling.
    #[test]
    fn value_and_value_checked_agree_inside_the_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();
            let t = if high.is_finite() {
                0.5 * (low + high)
            } else {
                1000.0
            };
            let state = fresh_95_percent_dense(t);
            assert_eq!(model.value(&state), model.value_checked(&state).unwrap());
        }
    }

    // ---------------------------------------------------------------------
    // Constant
    // ---------------------------------------------------------------------

    /// **Analytic-limit check.** A constant model returns its payload for any
    /// state, unchanged by temperature, burnup or porosity.
    #[test]
    fn constant_returns_its_payload() {
        let model = ConductivityModel::Constant(2.5);
        let mut state = MaterialState::fresh(1e-3);
        state.burnup = 70.0;
        state.porosity = 0.4;
        assert_eq!(model.value(&state), 2.5);
        assert_eq!(model.value(&MaterialState::fresh(4000.0)), 2.5);
    }

    // ---------------------------------------------------------------------
    // MATPRO UO2
    // ---------------------------------------------------------------------

    /// **Reference-checked against published UO2 conductivity.**
    ///
    /// Methodology: evaluate [`ConductivityModel::MatproUo2`] for fresh,
    /// unpoisoned UO2 at 95% of theoretical density (`porosity = 0.05`,
    /// `burnup = 0`, `gadolinia_fraction = 0`) at 1000 K, using the MATPRO-v11
    /// / modified-NFI coefficients as transcribed from upstream
    /// `conductivityMatproUO2.C`. Reference: the conductivity of 95%-dense
    /// unirradiated UO2 near 1000 K is a well-established 3.4–3.6 W/(m K)
    /// (MATPRO / FRAPCON fuel thermal conductivity). Tolerance: the assertion
    /// window `[3.3, 3.7]` W/(m K).
    ///
    /// Result (2026-07-29, this implementation): **k = 3.4342 W/(m K)** at
    /// 1000 K, decomposing as 3.4341 W/(m K) phonon + 2.75e-4 W/(m K)
    /// electronic, with the porosity factor equal to 0.99996 at 95% TD.
    /// Interpretation: the port reproduces the accepted magnitude and the
    /// expected phonon-dominated split at LWR operating temperature. This is a
    /// magnitude check against the accepted literature range, **not** a
    /// point-by-point comparison against a digitised MATPRO table.
    #[test]
    fn matpro_uo2_at_1000_k_matches_the_accepted_magnitude() {
        let k = ConductivityModel::MatproUo2.value(&fresh_95_percent_dense(1000.0));
        assert!(
            (3.3..3.7).contains(&k),
            "MATPRO UO2 at 1000 K, 95% TD gave {k} W/(m K)"
        );
        // Recorded value of this implementation, so a regression is visible.
        assert!((k - 3.4342).abs() < 1.0e-3, "k = {k}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The MATPRO porosity rescaling `1.0789*d/(1 + 0.5*(1 - d))` is
    /// constructed to be unity at `d = 0.95`, which is what makes the inner
    /// quantity "k95". Methodology: form the ratio of the model value at
    /// `porosity = 0.05` to the bare `k95` recomputed here from the same
    /// coefficients; pass criterion `|ratio - 1| < 1e-4`.
    ///
    /// Result: ratio = 0.999956, i.e. the factor is unity to 4.4e-5.
    #[test]
    fn matpro_uo2_porosity_factor_is_unity_at_95_percent_density() {
        let t = 1000.0;
        let k = ConductivityModel::MatproUo2.value(&fresh_95_percent_dense(t));
        let k95 = 1.0 / (0.0452 + 2.46e-4 * t) + 3.5e9 / (t * t) * (-16360.0f64 / t).exp();
        assert!((k / k95 - 1.0).abs() < 1.0e-4, "ratio {}", k / k95);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Conductivity must fall monotonically with burnup at fixed temperature —
    /// fission products dissolved in the lattice scatter phonons. Methodology:
    /// sweep burnup 0 → 80 MWd/kgHM in 1 MWd/kgHM steps at 1000 K, 95% TD;
    /// pass criterion: strictly decreasing.
    #[test]
    fn matpro_uo2_falls_monotonically_with_burnup() {
        let mut previous = f64::INFINITY;
        for i in 0..=80 {
            let mut state = fresh_95_percent_dense(1000.0);
            state.burnup = f64::from(i);
            let k = ConductivityModel::MatproUo2.value(&state);
            assert!(k < previous, "burnup {i}: {k} not below {previous}");
            previous = k;
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Porosity must reduce conductivity, and gadolinia must too (the `1.1599 *
    /// gad` term sits in the phonon denominator). Methodology: compare
    /// 95%-dense fresh UO2 at 1000 K against the same state with porosity
    /// raised to 0.10, and against the same state with a 5 wt% gadolinia
    /// content. Pass criterion: both are strictly lower.
    #[test]
    fn matpro_uo2_falls_with_porosity_and_with_gadolinia() {
        let base = fresh_95_percent_dense(1000.0);
        let k_base = ConductivityModel::MatproUo2.value(&base);

        let mut porous = base;
        porous.porosity = 0.10;
        assert!(ConductivityModel::MatproUo2.value(&porous) < k_base);

        let mut poisoned = base;
        poisoned.gadolinia_fraction = 0.05;
        assert!(ConductivityModel::MatproUo2.value(&poisoned) < k_base);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Gaseous swelling inflates the cell volume, which lowers the effective
    /// density fraction and so the conductivity; densification (negative
    /// volumetric strain) does the opposite. Methodology: apply +3% volumetric
    /// swelling, then -1% densification, to 95%-dense fresh UO2 at 1000 K.
    /// Pass criterion: swelling lowers `k`, densification raises it.
    #[test]
    fn matpro_uo2_responds_to_swelling_and_densification() {
        let base = fresh_95_percent_dense(1000.0);
        let k_base = ConductivityModel::MatproUo2.value(&base);

        let mut swollen = base;
        swollen.swelling = 0.03;
        assert!(ConductivityModel::MatproUo2.value(&swollen) < k_base);

        let mut densified = base;
        densified.densification = -0.01;
        assert!(ConductivityModel::MatproUo2.value(&densified) > k_base);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The volume-correction helper is floored at 0.05 so a pathological state
    /// cannot divide by zero. Methodology: set `densification = -2.0`, which
    /// would give a raw factor of -1; pass criterion: the returned conductivity
    /// is still finite.
    #[test]
    fn matpro_uo2_survives_a_pathological_volume_correction() {
        let mut state = fresh_95_percent_dense(1000.0);
        state.densification = -2.0;
        assert!(ConductivityModel::MatproUo2.value(&state).is_finite());
    }

    // ---------------------------------------------------------------------
    // NFIR UO2
    // ---------------------------------------------------------------------

    /// **Self-consistency check, not validation.**
    ///
    /// Cross-model consistency: two independent fits to the same material must
    /// agree in magnitude even though they disagree in detail. Methodology:
    /// evaluate [`ConductivityModel::NfirUo2`] and
    /// [`ConductivityModel::MatproUo2`] for fresh 95%-dense UO2 at 1000 K;
    /// pass criterion: they differ by less than 25% of the MATPRO value.
    ///
    /// Result (2026-07-29): NFIR = 3.6536 W/(m K), MATPRO = 3.4342 W/(m K),
    /// difference +6.4%. Interpretation: consistent with the published spread
    /// between UO2 conductivity correlations; this is a sanity band, not a
    /// validation of either fit.
    #[test]
    fn nfir_uo2_agrees_with_matpro_uo2_in_magnitude() {
        let state = fresh_95_percent_dense(1000.0);
        let nfir = ConductivityModel::NfirUo2.value(&state);
        let matpro = ConductivityModel::MatproUo2.value(&state);
        assert!(
            (nfir - matpro).abs() / matpro < 0.25,
            "NFIR {nfir} vs MATPRO {matpro} W/(m K)"
        );
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The NFIR porosity rescaling
    /// `(1 - s*(1 - d))/(1 - 0.05*s)` is exactly unity at `d = 0.95` for any
    /// value of its temperature-dependent slope `s`. Methodology: compare the
    /// model at `porosity = 0.05` against the same expression with the
    /// porosity term removed, at three temperatures. Pass criterion: relative
    /// difference below 1e-12 (this is an algebraic identity, so it should hold
    /// to round-off).
    #[test]
    fn nfir_uo2_porosity_factor_is_exactly_unity_at_95_percent_density() {
        for t in [600.0, 1000.0, 1800.0] {
            let with_porosity = ConductivityModel::NfirUo2.value(&fresh_95_percent_dense(t));
            // Reconstruct the bare k95 from the correlation's inner terms. At
            // zero burnup the two phonon branches coincide, so the tanh ramp
            // drops out of the reconstruction.
            let tc = t - 273.15;
            let base = 9.592e-2;
            let k95 = 1.0 / (base + 2.5e-4 * tc) + 1.32e-2 * (1.88e-3 * tc).exp();
            assert!(
                (with_porosity / k95 - 1.0).abs() < 1.0e-12,
                "T = {t} K: {with_porosity} vs {k95}"
            );
        }
    }

    /// **Characterisation of a known upstream defect — not a validation.**
    ///
    /// The gadolinia branch of `conductivityNfirUO2` contains
    /// `tanh(Gd)^0.1`, which tends to zero as the gadolinia content tends to
    /// zero. The branch therefore does **not** reduce to the un-poisoned
    /// `base = 9.592e-2`, and adding an infinitesimal amount of burnable poison
    /// makes the correlation predict a *higher* conductivity — physically
    /// backwards.
    ///
    /// Methodology: evaluate at 1000 K, 95% TD with `gadolinia_fraction = 0`,
    /// then with two vanishing traces, `1e-9` and `1e-30`. Pass criteria
    /// (documenting the defect, not endorsing it): both traces give a *higher*
    /// conductivity than clean fuel, and the excess **grows** as the trace
    /// shrinks — i.e. the branch does not converge to the un-poisoned value.
    ///
    /// Result (2026-07-29): 3.6536 W/(m K) at zero gadolinia, 3.8421 W/(m K)
    /// at a trace of 1e-9 (+5.2%) and 5.5353 W/(m K) at a trace of 1e-30
    /// (+51.5%). The `Gd^0.1` dependence makes the divergence slow but
    /// unbounded. This test exists so the defect cannot be silently "fixed"
    /// or silently relied upon; see the variant doc comment.
    #[test]
    fn nfir_uo2_gadolinia_branch_is_discontinuous_at_zero() {
        let clean = ConductivityModel::NfirUo2.value(&fresh_95_percent_dense(1000.0));

        let at_trace = |trace: f64| {
            let mut state = fresh_95_percent_dense(1000.0);
            state.gadolinia_fraction = trace;
            ConductivityModel::NfirUo2.value(&state)
        };
        let small = at_trace(1.0e-9);
        let smaller = at_trace(1.0e-30);

        assert!(
            small > clean,
            "expected the documented upstream discontinuity: {clean} -> {small}"
        );
        assert!(
            smaller > small,
            "expected the branch to diverge as Gd -> 0: {small} -> {smaller}"
        );
    }

    // ---------------------------------------------------------------------
    // IFA-601 UO2
    // ---------------------------------------------------------------------

    /// **Self-consistency check, not validation.**
    ///
    /// The burnup-dependent slope of the IFA-601 fit is defined piecewise on
    /// `Bu <= 50`, `50 < Bu <= 80`, `Bu > 80` MWd/kgHM. The published
    /// coefficients make it very nearly, but not exactly, continuous.
    /// Methodology: evaluate on either side of both breakpoints at 1000 K;
    /// pass criterion: the relative jump is below 0.5% at each.
    ///
    /// Result (2026-07-29): a 0.090% relative jump at 50 MWd/kgHM and 0.104%
    /// at 80 MWd/kgHM. Interpretation: the piecewise definition is a smooth
    /// stitch, not a modelling discontinuity.
    #[test]
    fn ifa601_uo2_is_nearly_continuous_at_its_burnup_breakpoints() {
        for breakpoint in [50.0, 80.0] {
            let mut below = fresh_95_percent_dense(1000.0);
            below.burnup = breakpoint;
            let mut above = below;
            above.burnup = breakpoint + 1.0e-9;

            let k_below = ConductivityModel::Ifa601Uo2.value(&below);
            let k_above = ConductivityModel::Ifa601Uo2.value(&above);
            assert!(
                (k_below - k_above).abs() / k_below < 5.0e-3,
                "jump at {breakpoint} MWd/kgHM: {k_below} -> {k_above}"
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Conductivity must fall with burnup, and the fit must stay in the right
    /// magnitude band for UO2. Methodology: 0, 30 and 60 MWd/kgHM at 1000 K;
    /// pass criterion: strictly decreasing and each value in `[1, 6]` W/(m K).
    #[test]
    fn ifa601_uo2_falls_with_burnup_and_stays_in_band() {
        let mut previous = f64::INFINITY;
        for bu in [0.0, 30.0, 60.0] {
            let mut state = fresh_95_percent_dense(1000.0);
            state.burnup = bu;
            let k = ConductivityModel::Ifa601Uo2.value(&state);
            assert!((1.0..6.0).contains(&k), "Bu = {bu}: k = {k}");
            assert!(k < previous);
            previous = k;
        }
    }

    // ---------------------------------------------------------------------
    // MOX correlations
    // ---------------------------------------------------------------------

    /// **Self-consistency check, not validation.**
    ///
    /// Brancheria's porosity factor `D1` is defined piecewise about
    /// `p = 0.05`; both branches give exactly 0.925 there. Methodology:
    /// evaluate at `p = 0.05` and `p = 0.05 + 1e-12` at 1000 K; pass criterion:
    /// relative difference below 1e-9.
    #[test]
    fn brancheria_mox_porosity_factor_is_continuous_at_five_percent() {
        let mut below = MaterialState::fresh(1000.0);
        below.porosity = 0.05;
        let mut above = below;
        above.porosity = 0.05 + 1.0e-12;

        let k_below = ConductivityModel::BrancheriaMox.value(&below);
        let k_above = ConductivityModel::BrancheriaMox.value(&above);
        assert!((k_below - k_above).abs() / k_below < 1.0e-9);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The porosity argument of Brancheria's `D1` is cut off at 15%, so the
    /// model must return identical values for any porosity at or above that.
    /// Methodology: compare `p = 0.15`, `p = 0.5` and `p = 0.9` at 1000 K;
    /// pass criterion: bit-for-bit equality.
    #[test]
    fn brancheria_mox_saturates_above_fifteen_percent_porosity() {
        let reference = {
            let mut s = MaterialState::fresh(1000.0);
            s.porosity = 0.15;
            ConductivityModel::BrancheriaMox.value(&s)
        };
        for p in [0.5, 0.9] {
            let mut s = MaterialState::fresh(1000.0);
            s.porosity = p;
            assert_eq!(ConductivityModel::BrancheriaMox.value(&s), reference);
        }
    }

    /// **Analytic-limit check.**
    ///
    /// Kato's Barani porosity correction
    /// `k*(kHe + 2k - 2p(k - kHe))/(kHe + 2k + p(k - kHe))` reduces exactly to
    /// `k` at zero porosity and exactly to the pore conductivity
    /// `kHe = 0.69` W/(m K) at unit porosity. Methodology: evaluate at
    /// `p = 0` and `p = 1` at 1000 K; pass criteria: the first equals the bare
    /// correlation recomputed here (relative 1e-12), the second equals 0.69
    /// W/(m K) to 1e-12.
    ///
    /// Result: at `p = 0`, k = 3.7712 W/(m K) matching the bare form; at
    /// `p = 1`, k = 0.690000 W/(m K) exactly.
    #[test]
    fn kato_mox_porosity_correction_hits_both_analytic_limits() {
        let model = ConductivityModel::KatoMox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.0,
        };
        let t: f64 = 1000.0;

        let bare =
            1.0 / (1.595e-2 + 2.493e-4 * t) + 1.541e11 / t.powf(2.5) * (-1.522e4f64 / t).exp();

        let mut dense = MaterialState::fresh(t);
        dense.porosity = 0.0;
        assert!((model.value(&dense) / bare - 1.0).abs() < 1.0e-12);

        let mut void = MaterialState::fresh(t);
        void.porosity = 1.0;
        assert!((model.value(&void) - 0.69).abs() < 1.0e-12);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Hypostoichiometry (oxygen-deficient MOX, `oxygen_deviation < 0`) must
    /// lower the conductivity — the `2.713*x` term with `x = 2 - O/M` sits in
    /// the phonon denominator. Methodology: compare `O/M = 2.00` against
    /// `O/M = 1.98` (`oxygen_deviation = -0.02`) at 1000 K, 5% porosity; pass
    /// criterion: the hypostoichiometric value is strictly lower.
    ///
    /// This also pins the sign convention: this crate's `oxygen_deviation` is
    /// the `x` in `(U,Pu)O_{2+x}`, the negative of upstream's `x = 2 - O/M`.
    #[test]
    fn kato_mox_hypostoichiometry_lowers_conductivity() {
        let model = ConductivityModel::KatoMox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.0,
        };
        let stoichiometric = fresh_95_percent_dense(1000.0);
        let mut hypo = stoichiometric;
        hypo.oxygen_deviation = -0.02;
        assert!(model.value(&hypo) < model.value(&stoichiometric));
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Minor actinides degrade MOX conductivity, and Kato's fit weights
    /// americium far more strongly than neptunium (3.583e-1 vs 6.317e-2 per
    /// unit mass fraction). Methodology: add 5 at% Am, then 5 at% Np, to
    /// 95%-dense stoichiometric MOX at 1000 K; pass criteria: both lower the
    /// conductivity, and the Am effect is the larger.
    #[test]
    fn kato_mox_minor_actinides_lower_conductivity_americium_most() {
        let clean = ConductivityModel::KatoMox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.0,
        };
        let with_am = ConductivityModel::KatoMox {
            americium_atom_fraction: 0.05,
            neptunium_atom_fraction: 0.0,
        };
        let with_np = ConductivityModel::KatoMox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.05,
        };
        let state = fresh_95_percent_dense(1000.0);

        let k_clean = clean.value(&state);
        let k_am = with_am.value(&state);
        let k_np = with_np.value(&state);
        assert!(k_am < k_clean);
        assert!(k_np < k_clean);
        assert!(k_am < k_np, "Am should dominate Np: {k_am} vs {k_np}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Kato's fit takes no plutonium term, so the returned conductivity must be
    /// independent of [`MaterialState::pu_fraction`] — upstream computes a Pu
    /// concentration in this model and never uses it. Methodology: compare
    /// `pu_fraction = 0` and `0.3` at 1000 K; pass criterion: bit-for-bit
    /// equality.
    #[test]
    fn kato_mox_is_independent_of_plutonium_content() {
        let model = ConductivityModel::KatoMox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.0,
        };
        let base = fresh_95_percent_dense(1000.0);
        let mut plutonium_rich = base;
        plutonium_rich.pu_fraction = 0.3;
        assert_eq!(model.value(&base), model.value(&plutonium_rich));
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Lanning & Beyer share the MATPRO structure, so at zero burnup,
    /// stoichiometric composition and 95% TD the two must agree in magnitude.
    /// Methodology: evaluate both at 1000 K; pass criterion: within 25%.
    ///
    /// Result (2026-07-29): Lanning-Beyer = 3.1171 W/(m K) versus MATPRO UO2 =
    /// 3.4342 W/(m K), i.e. 9.2% lower — as expected, since MOX conducts
    /// slightly less well than UO2.
    #[test]
    fn lanning_beyer_mox_agrees_with_matpro_uo2_in_magnitude() {
        let state = fresh_95_percent_dense(1000.0);
        let mox = ConductivityModel::LanningBeyerMox.value(&state);
        let uo2 = ConductivityModel::MatproUo2.value(&state);
        assert!((mox - uo2).abs() / uo2 < 0.25, "{mox} vs {uo2} W/(m K)");
        assert!(mox < uo2, "MOX should conduct less well than UO2 here");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The Lanning-Beyer porosity rescaling `1.0789*(1 - p)/(1 + 0.5*p)` is
    /// unity at `p = 0.05` by construction, and must fall monotonically with
    /// porosity. Methodology: sweep `p` over 0.00 → 0.30; pass criteria: unity
    /// within 1e-4 at `p = 0.05`, and strictly decreasing.
    #[test]
    fn lanning_beyer_mox_porosity_rescaling_behaves() {
        let factor_at_95: f64 = 1.0789 * 0.95 / 1.025;
        assert!((factor_at_95 - 1.0).abs() < 1.0e-4);

        let mut previous = f64::INFINITY;
        for p in [0.0, 0.05, 0.10, 0.20, 0.30] {
            let mut state = MaterialState::fresh(1000.0);
            state.porosity = p;
            let k = ConductivityModel::LanningBeyerMox.value(&state);
            assert!(k < previous, "p = {p}: {k} not below {previous}");
            previous = k;
        }
    }

    /// **Analytic-limit check.**
    ///
    /// MAMOX relaxes exponentially from the fresh-fuel conductivity `k0` to an
    /// irradiated asymptote `kInf = 1.755` W/(m K) with a burnup constant
    /// `phi = 128.75` MWd/kgHM. Methodology: evaluate at `Bu = 0` (must equal
    /// `k0`, since `exp(0) = 1`) and at `Bu = 10 000` MWd/kgHM (must be within
    /// 1e-6 of `kInf`); at `Bu = phi` the gap to `kInf` must have shrunk by
    /// exactly `1/e`. Tolerances: 1e-12 relative on the `1/e` identity.
    ///
    /// Result: `k(0) = 3.4062` W/(m K); `k(phi) - kInf` equals
    /// `(k(0) - kInf)/e` to 1e-15; `k(10000) = 1.755000` W/(m K).
    #[test]
    fn magni_mamox_relaxes_from_k0_to_kappa_inf_with_burnup() {
        let model = ConductivityModel::MagniMamox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.0,
        };
        let kappa_inf = 1.755;

        let fresh = fresh_95_percent_dense(1000.0);
        let k0 = model.value(&fresh);

        let mut at_phi = fresh;
        at_phi.burnup = 128.75;
        let k_phi = model.value(&at_phi);
        let expected = kappa_inf + (k0 - kappa_inf) / std::f64::consts::E;
        assert!((k_phi - expected).abs() < 1.0e-12, "{k_phi} vs {expected}");

        let mut burnt = fresh;
        burnt.burnup = 10_000.0;
        assert!((model.value(&burnt) - kappa_inf).abs() < 1.0e-6);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// MAMOX weights americium far more strongly than plutonium or neptunium
    /// (`AAm = 0.596` against `APu = 2.63e-8`), so Am must dominate the
    /// composition dependence. Methodology: at 1000 K, 95% TD, fresh, compare
    /// clean MOX against 5 at% Am and against 5 at% Np; pass criteria: Am
    /// lowers the conductivity substantially (> 10%), Np changes it
    /// negligibly (< 0.1%).
    #[test]
    fn magni_mamox_is_dominated_by_americium() {
        let state = fresh_95_percent_dense(1000.0);
        let clean = ConductivityModel::MagniMamox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.0,
        }
        .value(&state);
        let with_am = ConductivityModel::MagniMamox {
            americium_atom_fraction: 0.05,
            neptunium_atom_fraction: 0.0,
        }
        .value(&state);
        let with_np = ConductivityModel::MagniMamox {
            americium_atom_fraction: 0.0,
            neptunium_atom_fraction: 0.05,
        }
        .value(&state);

        assert!(
            with_am < clean * 0.9,
            "Am effect too weak: {with_am} vs {clean}"
        );
        assert!((with_np - clean).abs() / clean < 1.0e-3);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Philipponneau's porosity factor `alpha = (1/0.864)*(1 - p)/(1 + 2p)` is
    /// unity at `p = 0.05` by construction (that is what the 0.864 normalises),
    /// and the model must fall with both porosity and burnup. Methodology:
    /// check `alpha(0.05)` to 1e-3; sweep porosity 0 → 0.3 and burnup 0 → 100
    /// MWd/kgHM at 1000 K; pass criterion: strictly decreasing in each.
    ///
    /// Result: `alpha(0.05) = 0.99954`.
    #[test]
    fn philipponneau_mox_falls_with_porosity_and_burnup() {
        let alpha_at_95: f64 = (1.0 / 0.864) * 0.95 / 1.1;
        assert!((alpha_at_95 - 1.0).abs() < 1.0e-3, "alpha = {alpha_at_95}");

        let mut previous = f64::INFINITY;
        for p in [0.0, 0.05, 0.15, 0.30] {
            let mut state = MaterialState::fresh(1000.0);
            state.porosity = p;
            let k = ConductivityModel::PhilipponneauMox.value(&state);
            assert!(k < previous);
            previous = k;
        }

        previous = f64::INFINITY;
        for bu in [0.0, 25.0, 50.0, 100.0] {
            let mut state = fresh_95_percent_dense(1000.0);
            state.burnup = bu;
            let k = ConductivityModel::PhilipponneauMox.value(&state);
            assert!(k < previous);
            previous = k;
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Philipponneau's `sqrt(x + 0.00931)` goes imaginary for
    /// `oxygen_deviation > 0.00931` (hyperstoichiometric fuel), where upstream
    /// would produce a NaN. This port floors the radicand at zero.
    /// Methodology: evaluate at `oxygen_deviation = +0.05` at 1000 K; pass
    /// criterion: finite and positive.
    #[test]
    fn philipponneau_mox_guards_the_hyperstoichiometric_square_root() {
        let mut state = fresh_95_percent_dense(1000.0);
        state.oxygen_deviation = 0.05;
        let k = ConductivityModel::PhilipponneauMox.value(&state);
        assert!(k.is_finite() && k > 0.0, "k = {k}");
    }

    // ---------------------------------------------------------------------
    // Metals
    // ---------------------------------------------------------------------

    /// **Reference-checked against the tabulated room-temperature value.**
    ///
    /// Methodology: evaluate [`ConductivityModel::RelapZircaloy`] at 300 K
    /// using the RELAP polynomial as transcribed from upstream
    /// `conductivityRelapZy.C`. Reference: the thermal conductivity of
    /// Zircaloy-4 near room temperature is commonly tabulated as
    /// approximately 13 W/(m K). Tolerance: +/- 1.5 W/(m K) — deliberately
    /// loose, because the reference is a handbook figure rather than the
    /// specific dataset RELAP fitted.
    ///
    /// Result (2026-07-29): **k(300 K) = 12.682 W/(m K)**, i.e. 0.32 W/(m K)
    /// below the 13 W/(m K) handbook figure. Interpretation: the transcription
    /// of the polynomial coefficients is correct to within the spread of
    /// published Zircaloy conductivity data; this is a magnitude check, not a
    /// validation against the RELAP source dataset.
    #[test]
    fn relap_zircaloy_reproduces_the_handbook_room_temperature_value() {
        let k = ConductivityModel::RelapZircaloy.value(&MaterialState::fresh(300.0));
        assert!((k - 13.0).abs() < 1.5, "k(300 K) = {k} W/(m K)");
        assert!((k - 12.682).abs() < 1.0e-3, "regression: k = {k}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Zircaloy conductivity rises monotonically over the fit's stated
    /// 290–1800 K window. Methodology: sample every 50 K from 290 to 1800 K;
    /// pass criterion: strictly increasing.
    #[test]
    fn relap_zircaloy_rises_monotonically_over_its_stated_range() {
        let mut previous = f64::NEG_INFINITY;
        let mut t = 290.0;
        while t <= 1800.0 {
            let k = ConductivityModel::RelapZircaloy.value(&MaterialState::fresh(t));
            assert!(k > previous, "T = {t} K: {k} not above {previous}");
            previous = k;
            t += 50.0;
        }
    }

    /// **Reference-checked against the tabulated room-temperature value.**
    ///
    /// Methodology: evaluate [`ConductivityModel::Molybdenum`] at 300 K using
    /// the quadratic from upstream `conductivityMolybdenum.C`. Reference: the
    /// thermal conductivity of pure molybdenum at 300 K is a standard handbook
    /// value of approximately 138 W/(m K). Tolerance: +/- 3 W/(m K).
    ///
    /// Result (2026-07-29): **k(300 K) = 137.99 W/(m K)** — within 0.01
    /// W/(m K) of the handbook figure. Interpretation: the fit is anchored on
    /// the accepted room-temperature conductivity, and the coefficients are
    /// transcribed correctly.
    #[test]
    fn molybdenum_reproduces_the_handbook_room_temperature_value() {
        let k = ConductivityModel::Molybdenum.value(&MaterialState::fresh(300.0));
        assert!((k - 138.0).abs() < 3.0, "k(300 K) = {k} W/(m K)");
        assert!((k - 137.99).abs() < 0.05, "regression: k = {k}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Molybdenum's fit has a negative linear and a positive quadratic term, so
    /// it must fall with temperature over the metal's whole service range and
    /// only turn back up near the vertex at `T = 4.945e-2/(2*9.128e-6) ~
    /// 2709 K`. Methodology: sample 300 → 2500 K in 100 K steps; pass
    /// criterion: strictly decreasing.
    #[test]
    fn molybdenum_conductivity_falls_with_temperature_below_the_vertex() {
        let vertex = 4.945e-2 / (2.0 * 9.128e-6);
        assert!((2600.0..2800.0).contains(&vertex), "vertex at {vertex} K");

        let mut previous = f64::INFINITY;
        let mut t = 300.0;
        while t <= 2500.0 {
            let k = ConductivityModel::Molybdenum.value(&MaterialState::fresh(t));
            assert!(k < previous, "T = {t} K: {k} not below {previous}");
            previous = k;
            t += 100.0;
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Both metal-cladding fits are exactly linear in **Celsius** temperature,
    /// `k = intercept + slope*(T - 273.15)`, so a finite difference must return
    /// the published slope to round-off and the value at any temperature must
    /// sit on that line.
    ///
    /// Methodology: evaluate at 400 K and 500 K — inside both correlations'
    /// validity windows, so [`ConductivityModel::value`] does not clamp — and
    /// check both the slope and the implied intercept. Pass criteria:
    /// intercepts 8.431 and 13.95 W/(m K) and slopes 0.0205 and 0.01163
    /// W/(m K^2), each to 1e-12.
    ///
    /// (Evaluating at the fits' natural 273.15 K origin would be clamped to
    /// each model's lower bound, which is exactly the documented behaviour of
    /// `value`; the intercept is therefore recovered by extrapolating the
    /// measured line rather than by sampling it out of range.)
    #[test]
    fn hastelloy_n_and_1515_ti_are_exactly_linear_in_celsius() {
        let cases = [
            (ConductivityModel::SwindemanHastelloyN, 8.431, 0.0205),
            (ConductivityModel::Tobbe1515Ti, 13.95, 1.163e-2),
        ];
        for (model, intercept, slope) in cases {
            let at_400 = model.value(&MaterialState::fresh(400.0));
            let at_500 = model.value(&MaterialState::fresh(500.0));

            let measured_slope = (at_500 - at_400) / 100.0;
            assert!(
                (measured_slope - slope).abs() < 1.0e-12,
                "{}: slope {measured_slope} vs {slope}",
                model.name()
            );

            let measured_intercept = at_400 - measured_slope * (400.0 - 273.15);
            assert!(
                (measured_intercept - intercept).abs() < 1.0e-12,
                "{}: intercept {measured_intercept} vs {intercept}",
                model.name()
            );
        }
    }

    // ---------------------------------------------------------------------
    // TRISO layers
    // ---------------------------------------------------------------------

    /// **Analytic-limit check.**
    ///
    /// The PARFUME buffer interpolation is constructed to pass exactly through
    /// its two anchor points. Methodology: with upstream's defaults
    /// (`rho_init = 1000`, `rho_theo = 2250` kg/m^3, `k_init = 0.5`,
    /// `k_theo = 4.0` W/(m K)), evaluate at the porosity that reproduces the
    /// theoretical density (`p = 0`) and at the porosity that reproduces the
    /// as-fabricated density (`p = 1 - 1000/2250 = 0.5556`). Pass criterion:
    /// the returned values equal `k_theo` and `k_init` to 1e-12.
    ///
    /// Result: `k(p = 0) = 4.000000` W/(m K); `k(p = 0.55556) = 0.500000`
    /// W/(m K). Both exact to round-off, confirming the algebra is transcribed
    /// correctly.
    #[test]
    fn parfume_buffer_passes_through_both_anchor_points() {
        let model = ConductivityModel::parfume_buffer_default();

        let mut dense = MaterialState::fresh(1000.0);
        dense.porosity = 0.0;
        assert!((model.value(&dense) - 4.0).abs() < 1.0e-12);

        let mut as_fabricated = MaterialState::fresh(1000.0);
        as_fabricated.porosity = 1.0 - 1000.0 / 2250.0;
        assert!((model.value(&as_fabricated) - 0.5).abs() < 1.0e-12);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The buffer model carries no temperature dependence at all, which is why
    /// its validity window is unbounded. Methodology: evaluate at 300 K and at
    /// 2000 K with identical porosity; pass criterion: bit-for-bit equality.
    #[test]
    fn parfume_buffer_is_temperature_independent() {
        let model = ConductivityModel::parfume_buffer_default();
        let mut cold = MaterialState::fresh(300.0);
        cold.porosity = 0.3;
        let mut hot = cold;
        hot.temperature = 2000.0;
        assert_eq!(model.value(&cold), model.value(&hot));
    }

    /// **Self-consistency check, not validation.**
    ///
    /// A denser buffer conducts better. Methodology: sweep porosity from the
    /// as-fabricated 0.5556 down to 0; pass criterion: strictly increasing
    /// conductivity, bracketed by the two anchor values.
    #[test]
    fn parfume_buffer_conducts_better_as_it_densifies() {
        let model = ConductivityModel::parfume_buffer_default();
        let mut previous = f64::NEG_INFINITY;
        for p in [0.5556, 0.4, 0.3, 0.2, 0.1, 0.0] {
            let mut state = MaterialState::fresh(1000.0);
            state.porosity = p;
            let k = model.value(&state);
            assert!(k > previous, "p = {p}: {k} not above {previous}");
            assert!((0.49..=4.01).contains(&k), "p = {p}: k = {k}");
            previous = k;
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// PARFUME's SiC conductivity is the exact hyperbola `17885/T + 2`, so
    /// `(k - 2)*T` must be the constant 17885 W/m at every temperature, and the
    /// model must fall monotonically. Methodology: evaluate at 300, 1273 and
    /// 2000 K; pass criteria: the invariant holds to 1e-9 relative, and the
    /// values decrease.
    ///
    /// Result: k(300 K) = 61.617, k(1273 K) = 16.050, k(2000 K) = 10.943
    /// W/(m K).
    #[test]
    fn parfume_sic_is_the_exact_hyperbola_it_claims_to_be() {
        let mut previous = f64::INFINITY;
        for t in [300.0, 1273.0, 2000.0] {
            let k = ConductivityModel::ParfumeSiC.value(&MaterialState::fresh(t));
            assert!(((k - 2.0) * t / 17885.0 - 1.0).abs() < 1.0e-9, "T = {t}");
            assert!(k < previous);
            previous = k;
        }
    }
}
