// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
//   `offbeatLib/gapGasModel/gapGasModel.C`
//     (`correctMassFractions`, `correctMolFractions`, `correctMass`)
//   `offbeatLib/gapGasModel/gapFRAPCON.C`
//     (`kappa`, `a`, `rhoMixture`, and the default `speciesW` /
//      `conductivity_A` / `conductivity_B` tables in the constructor).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The fill-gas / fission-gas mixture in the fuel/cladding gap.
//!
//! # What this computes
//!
//! A fuel rod is filled with helium at fabrication. As it burns, fission gas —
//! overwhelmingly **xenon** with some **krypton** — is released from the fuel
//! matrix into the free volume and dilutes that helium. Xenon conducts heat
//! roughly twenty times worse than helium at the same temperature, so the
//! dilution degrades gap conductance, raises fuel temperature, and accelerates
//! further release. This module holds the composition and the two mixture
//! properties the gap conductance needs from it:
//!
//! - [`GapGasMixture::conductivity`] — the mixture thermal conductivity \[W/m/K\],
//! - [`GapGasMixture::accommodation_coefficient`] — the thermal accommodation
//!   coefficient that sets the temperature-jump distance at each surface.
//!
//! # Which gases
//!
//! Exactly the six noble gases upstream tabulates: helium, neon, argon,
//! krypton, xenon and radon (see [`GapGasSpecies`]). Helium is the fill gas;
//! xenon and krypton are the fission products; argon and neon appear as
//! alternative fill gases in experimental rods; radon is tabulated by upstream
//! but is not a meaningful rod constituent.
//!
//! **Upstream defect reproduced:** upstream's default conductivity coefficients
//! for **neon and radon are placeholders** — `A = 1.0`, `B = 1.0`, giving
//! `k = T` W/m/K, which at 500 K is 500 W/m/K, four orders of magnitude too
//! high. The numbers are reproduced bit-for-bit so a comparison against an
//! OFFBEAT run is not silently shifted, but
//! [`GapGasSpecies::has_placeholder_conductivity`] flags them and
//! [`GapGasMixture::conductivity_checked`] refuses a mixture that contains them.
//!
//! # Which mixing rule
//!
//! The **Lindsay–Bromley form of the Wassiljewa equation**, i.e. a
//! mole-fraction-weighted sum with binary interaction factors:
//!
//! ```text
//! k_i  = A_i · T^(B_i)
//! φ_ij = [1 + (k_i/k_j)^(1/2) · (M_i/M_j)^(1/4)]²  /  [2^(3/2) · (1 + M_i/M_j)^(1/2)]
//! ψ_ij = φ_ij · [1 + 2.41 · (M_i − M_j)(M_i − 0.142 M_j) / (M_i + M_j)²]
//! k_mix = Σ_i  k_i x_i / ( x_i + Σ_{j≠i} ψ_ij x_j )
//! ```
//!
//! with `x` mole fractions and `M` molar masses. Upstream (`gapFRAPCON::kappa`)
//! attributes this to the FRAPCON-4.0 manual. MATPRO (NUREG/CR-6150 §12.1.1,
//! subcode GTHCON) uses the same rule.
//!
//! **On the `(M_i + M_j)²` denominator (gh:#1070).** The RELAP-7 Theory Manual
//! (INL/EXT-14-31366, §4.2.3), which cites MATPRO §12.1.1 for this rule,
//! prints Eq 233 with `(M_i − M_j)²` in the denominator. That was checked on
//! the rendered PDF on 2026-11-01, not only on extracted text: the PDF really
//! does print a minus. This port keeps OFFBEAT's `(M_i + M_j)²`, for three
//! reasons: it is the published Brokaw/Lindsay–Bromley form; with a minus the
//! factor collapses to `(M_i − 0.142 M_j)/(M_i − M_j)`, which is singular for
//! two species of equal molar mass; and on the RELAP-7 He/Kr/Xe composition
//! the minus form gives 0.00647 W/m/K at 300 K, barely above pure xenon
//! (0.00593), against 0.01039 with the plus. The same page of the manual also
//! departs from the standard form in Eq 232 (`ψ_ij x_i` where the sum needs
//! `ψ_ij x_j`) and Eq 234 (`2^(2/3)` where the form has `2^(3/2)`, and the
//! `1 +` placed inside the square root), so Eq 233's minus reads as one of
//! several typesetting slips. **Not re-checked against MATPRO's own text**,
//! which was not available to this session. Its one structural property worth
//! knowing: **it reduces exactly to `k_i` at `x_i = 1`**, because the inner sum
//! is then empty and the term is `k_i · 1 / 1`. That exactness is asserted in
//! the tests.
//!
//! # Units
//!
//! Strict SI (kelvin, W/m/K, kilogram, mole, pascal, m³) with the single
//! documented exception of [`GapGasSpecies::molar_mass_g_per_mol`].

use petir::mathf::RealMath;
use crate::error::{OffbeatError, Result};

/// Universal gas constant `R` \[J/(mol·K)\], CODATA exact value.
///
/// Upstream reads `Foam::constant::physicoChemical::R`, which carries the same
/// SI-2019 exact definition.
pub const GAS_CONSTANT: f64 = 8.314_462_618_153_24;

/// A noble-gas component of the gap gas — upstream's `species_` entries.
///
/// The six species are exactly the keys of upstream `gapFRAPCON`'s default
/// `speciesW` / `conductivity_A` / `conductivity_B` dictionaries. The
/// discriminant is used as an array index throughout this module, so the order
/// is part of the type's contract and must not be reordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum GapGasSpecies {
    /// Helium — the as-fabricated fill gas of essentially every LWR rod.
    Helium = 0,
    /// Neon — an alternative fill gas in some experimental rods.
    ///
    /// **Its upstream conductivity coefficients are placeholders**; see
    /// [`Self::has_placeholder_conductivity`].
    Neon = 1,
    /// Argon — an alternative fill gas in some experimental rods.
    Argon = 2,
    /// Krypton — a released fission gas, roughly 10–15% of the released
    /// fission-gas moles alongside xenon.
    Krypton = 3,
    /// Xenon — the dominant released fission gas and the dominant cause of
    /// gap-conductance degradation through life.
    Xenon = 4,
    /// Radon — tabulated by upstream; not a meaningful rod constituent.
    ///
    /// **Its upstream conductivity coefficients are placeholders**; see
    /// [`Self::has_placeholder_conductivity`].
    Radon = 5,
}

/// Number of gas species tracked — the length of every composition array here.
pub const N_SPECIES: usize = 6;

impl GapGasSpecies {
    /// All six species in discriminant order, for iteration.
    pub const ALL: [GapGasSpecies; N_SPECIES] = [
        GapGasSpecies::Helium,
        GapGasSpecies::Neon,
        GapGasSpecies::Argon,
        GapGasSpecies::Krypton,
        GapGasSpecies::Xenon,
        GapGasSpecies::Radon,
    ];

    /// Array index of this species — its discriminant.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Chemical symbol, matching upstream's dictionary keys (`"He"`, `"Xe"`, …).
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Helium => "He",
            Self::Neon => "Ne",
            Self::Argon => "Ar",
            Self::Krypton => "Kr",
            Self::Xenon => "Xe",
            Self::Radon => "Rn",
        }
    }

    /// Molar mass \[**g/mol**\] — upstream's `speciesW` default dictionary.
    ///
    /// **This is the one non-SI quantity in the module**, kept in g/mol because
    /// that is the unit upstream tabulates and every fuel-performance input deck
    /// quotes. Use [`Self::molar_mass`] for the SI value. The numbers are
    /// upstream's verbatim, including its rounded krypton (`83.8`) and radon
    /// (`222`) entries.
    #[must_use]
    pub const fn molar_mass_g_per_mol(self) -> f64 {
        match self {
            Self::Helium => 4.0026,
            Self::Neon => 20.183,
            Self::Argon => 39.948,
            Self::Krypton => 83.8,
            Self::Xenon => 131.3,
            Self::Radon => 222.0,
        }
    }

    /// Molar mass \[kg/mol\] — the SI form of
    /// [`Self::molar_mass_g_per_mol`], divided by 1000.
    #[must_use]
    pub fn molar_mass(self) -> f64 {
        self.molar_mass_g_per_mol() / 1000.0
    }

    /// Coefficients `(A, B)` of the pure-gas conductivity fit
    /// `k = A · T^B` \[W/m/K, T in K\] — upstream's default `conductivity_A`
    /// and `conductivity_B` dictionaries.
    ///
    /// # Valid range
    ///
    /// Upstream states none. The fits are power laws with `B < 1`, so they are
    /// finite and monotonically increasing for all `T > 0`; treat them as
    /// trustworthy over roughly 300–2000 K, the range a rod gap actually spans.
    ///
    /// # Placeholder entries
    ///
    /// Neon and radon return `(1.0, 1.0)`. That is upstream's literal default
    /// and is **not a physical fit** — see
    /// [`Self::has_placeholder_conductivity`].
    #[must_use]
    pub const fn conductivity_coefficients(self) -> (f64, f64) {
        match self {
            Self::Helium => (2.531e-3, 0.7146),
            Self::Neon => (1.0, 1.0),
            Self::Argon => (4.092e-4, 0.6748),
            Self::Krypton => (1.966e-4, 0.7006),
            Self::Xenon => (9.825e-5, 0.7334),
            Self::Radon => (1.0, 1.0),
        }
    }

    /// Coefficients `(A, B)` of the **MATPRO** pure-gas conductivity fit
    /// `k = A · T^B` \[W/m/K, T in K\], or `None` where this port has no
    /// MATPRO fit for the species.
    ///
    /// | Species | A | B |
    /// |---|---|---|
    /// | He | 2.639e-3 | 0.7085 |
    /// | Kr | 8.247e-5 | 0.8363 |
    /// | Xe | 4.351e-5 | 0.8616 |
    ///
    /// Neon, argon and radon return `None`: the cited source gives only these
    /// three (MATPRO itself has more gases; they were not available to this
    /// session and are not invented here).
    ///
    /// # Valid range
    ///
    /// 300–3000 K, the range over which the RELAP-7 manual tabulates the
    /// mixture from these fits.
    ///
    /// # Source
    ///
    /// MATPRO, NUREG/CR-6150, §12.1.1 (GTHCON); constants as reproduced in the
    /// RELAP-7 Theory Manual (INL/EXT-14-31366) §4.2.3. Hand-checked values:
    /// unit test `matpro_pure_gas_fits_match_hand_values`.
    #[must_use]
    pub const fn matpro_conductivity_coefficients(self) -> Option<(f64, f64)> {
        match self {
            Self::Helium => Some((2.639e-3, 0.7085)),
            Self::Krypton => Some((8.247e-5, 0.8363)),
            Self::Xenon => Some((4.351e-5, 0.8616)),
            Self::Neon | Self::Argon | Self::Radon => None,
        }
    }

    /// Coefficients `(A, B)` from the chosen [`GasConductivityFit`], or `None`
    /// where that set has no fit for this species. `Frapcon` always returns
    /// `Some` (including the neon/radon placeholders — see
    /// [`Self::has_placeholder_conductivity`]).
    #[must_use]
    pub const fn conductivity_coefficients_for(
        self,
        fit: GasConductivityFit,
    ) -> Option<(f64, f64)> {
        match fit {
            GasConductivityFit::Frapcon => Some(self.conductivity_coefficients()),
            GasConductivityFit::Matpro => self.matpro_conductivity_coefficients(),
        }
    }

    /// Whether this species' upstream conductivity coefficients are the
    /// placeholder `(1.0, 1.0)` rather than a real fit.
    ///
    /// True for [`Neon`](Self::Neon) and [`Radon`](Self::Radon). `k = 1·T^1`
    /// gives 500 W/m/K at 500 K — about four orders of magnitude above any noble
    /// gas — so a mixture containing either of these carries a meaningless
    /// conductivity. The value is reproduced anyway (so a run can be compared
    /// against OFFBEAT), but [`GapGasMixture::conductivity_checked`] rejects
    /// such a mixture and this predicate lets a caller test for it first.
    #[must_use]
    pub const fn has_placeholder_conductivity(self) -> bool {
        matches!(self, Self::Neon | Self::Radon)
    }

    /// Pure-gas thermal conductivity `k = A·T^B` \[W/m/K\] at temperature
    /// `t` \[K\].
    ///
    /// Returns `0.0` for a non-positive or non-finite temperature rather than a
    /// NaN. See [`Self::conductivity_coefficients`] for validity and for the
    /// neon/radon placeholder caveat.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::GapGasSpecies;
    ///
    /// // Helium conducts far better than xenon at the same temperature.
    /// let he = GapGasSpecies::Helium.conductivity(1000.0);
    /// let xe = GapGasSpecies::Xenon.conductivity(1000.0);
    /// assert!(he > 20.0 * xe);
    /// ```
    #[must_use]
    pub fn conductivity(self, t: f64) -> f64 {
        if !(t > 0.0) || !t.is_finite() {
            return 0.0;
        }
        let (a, b) = self.conductivity_coefficients();
        a * t.r_powf(b)
    }
}

/// Which set of pure-gas conductivity fits `k = A·T^B` to use.
///
/// [`Frapcon`](Self::Frapcon) is OFFBEAT's default and what every existing
/// method in this module uses; nothing about it changes. [`Matpro`](Self::Matpro)
/// is the MATPRO set the RELAP-7 heat-structure properties use (gh:#1070).
/// Only the coefficients differ; the mixing rule is the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GasConductivityFit {
    /// Upstream `gapFRAPCON`'s default `conductivity_A` / `conductivity_B`
    /// tables — [`GapGasSpecies::conductivity_coefficients`]. The default.
    #[default]
    Frapcon,
    /// MATPRO (NUREG/CR-6150 §12.1.1) fits for helium, krypton and xenon, as
    /// reproduced in the RELAP-7 Theory Manual §4.2.3 —
    /// [`GapGasSpecies::matpro_conductivity_coefficients`]. **Only those three
    /// species have a fit**; a mixture holding any other species is refused.
    Matpro,
}

/// Upper temperature \[K\] at which upstream freezes the accommodation-coefficient
/// correlations — `min(T, 1300)` in `gapFRAPCON::a`.
pub const ACCOMMODATION_T_CAP: f64 = 1300.0;

/// The gap gas: what it is made of, and how much of it there is.
///
/// Mirrors the composition state upstream's `gapGasModel` carries — mass
/// fractions `Y_`, mole fractions `M_` and total mass `gasM_` — with the
/// normalisation invariants enforced at construction instead of by a
/// `correctMassFractions()` call the caller must remember to make.
///
/// # Invariants
///
/// Both fraction arrays sum to 1 (to within floating-point rounding) at all
/// times, and the mass is non-negative and finite. Every constructor and mutator
/// re-normalises, so these hold by construction.
///
/// # Units
///
/// Mass in kilogram, fractions dimensionless, everything derived in strict SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapGasMixture {
    /// Mass fractions, indexed by [`GapGasSpecies::index`]; sums to 1.
    mass_fractions: [f64; N_SPECIES],
    /// Mole fractions, indexed by [`GapGasSpecies::index`]; sums to 1.
    mole_fractions: [f64; N_SPECIES],
    /// Total gas mass \[kg\].
    mass: f64,
}

impl GapGasMixture {
    /// Build from **mass** fractions \[-\] and a total mass \[kg\].
    ///
    /// The fractions need not be normalised — they are divided by their sum,
    /// exactly as upstream's `correctMassFractions()` does. Mole fractions are
    /// then derived as `x_i ∝ Y_i / M_i` and normalised
    /// (`correctMolFractions()`).
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] if any fraction is negative or non-finite,
    ///   if their sum is not strictly positive, or if `mass` is negative or
    ///   non-finite.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{GapGasMixture, GapGasSpecies};
    ///
    /// // 90% helium / 10% xenon by mass, 1 gram of it.
    /// let mut y = [0.0; 6];
    /// y[GapGasSpecies::Helium.index()] = 0.9;
    /// y[GapGasSpecies::Xenon.index()] = 0.1;
    /// let mix = GapGasMixture::from_mass_fractions(y, 1.0e-3).unwrap();
    ///
    /// // By moles it is overwhelmingly helium: xenon is 33x heavier.
    /// assert!(mix.mole_fraction(GapGasSpecies::Helium) > 0.99);
    /// ```
    pub fn from_mass_fractions(mass_fractions: [f64; N_SPECIES], mass: f64) -> Result<Self> {
        if !mass.is_finite() || mass < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "gap gas mass",
                value: mass,
                unit: "kg",
                reason: "must be finite and non-negative",
            });
        }
        let mut total = 0.0;
        for (i, y) in mass_fractions.iter().enumerate() {
            if !y.is_finite() || *y < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity: GapGasSpecies::ALL[i].symbol(),
                    value: *y,
                    unit: "-",
                    reason: "gas mass fraction must be finite and non-negative",
                });
            }
            total += *y;
        }
        if !(total > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "sum of gap gas mass fractions",
                value: total,
                unit: "-",
                reason: "must be strictly positive; upstream aborts with \
                         \"Sum of mass fractions is zero or negative\"",
            });
        }

        let mut y = [0.0; N_SPECIES];
        for i in 0..N_SPECIES {
            y[i] = mass_fractions[i] / total;
        }

        let mut x = [0.0; N_SPECIES];
        let mut mol_total = 0.0;
        for i in 0..N_SPECIES {
            x[i] = y[i] / GapGasSpecies::ALL[i].molar_mass();
            mol_total += x[i];
        }
        // Unreachable given `total > 0` and all molar masses positive, but the
        // division is guarded rather than assumed.
        if !(mol_total > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "sum of gap gas molar fractions",
                value: mol_total,
                unit: "-",
                reason: "must be strictly positive",
            });
        }
        for xi in x.iter_mut() {
            *xi /= mol_total;
        }

        Ok(Self {
            mass_fractions: y,
            mole_fractions: x,
            mass,
        })
    }

    /// Build from **mole** fractions \[-\] and a total mass \[kg\].
    ///
    /// Converts to mass fractions `Y_i ∝ x_i · M_i` and defers to
    /// [`Self::from_mass_fractions`], so the stored mole fractions are the
    /// given ones normalised (to round-off). Not an upstream constructor:
    /// added because compositions such as the RELAP-7 gap gas are published by
    /// mole.
    ///
    /// # Errors
    ///
    /// As [`Self::from_mass_fractions`]: a negative or non-finite fraction, a
    /// zero sum, or a negative or non-finite mass.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{GapGasMixture, GapGasSpecies};
    ///
    /// let mut x = [0.0; 6];
    /// x[GapGasSpecies::Helium.index()] = 0.5;
    /// x[GapGasSpecies::Xenon.index()] = 0.5;
    /// let mix = GapGasMixture::from_mole_fractions(x, 1.0e-3).unwrap();
    /// assert!((mix.mole_fraction(GapGasSpecies::Xenon) - 0.5).abs() < 1e-15);
    /// ```
    pub fn from_mole_fractions(mole_fractions: [f64; N_SPECIES], mass: f64) -> Result<Self> {
        let mut y = [0.0; N_SPECIES];
        for i in 0..N_SPECIES {
            let x = mole_fractions[i];
            if !x.is_finite() || x < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity: GapGasSpecies::ALL[i].symbol(),
                    value: x,
                    unit: "-",
                    reason: "gas mole fraction must be finite and non-negative",
                });
            }
            y[i] = x * GapGasSpecies::ALL[i].molar_mass();
        }
        Self::from_mass_fractions(y, mass)
    }

    /// A pure single-species gas of the given `mass` \[kg\].
    ///
    /// The beginning-of-life state of an LWR rod is
    /// `GapGasMixture::pure(GapGasSpecies::Helium, m)`.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite mass.
    pub fn pure(species: GapGasSpecies, mass: f64) -> Result<Self> {
        let mut y = [0.0; N_SPECIES];
        y[species.index()] = 1.0;
        Self::from_mass_fractions(y, mass)
    }

    /// Mass fraction \[-\] of one species.
    #[must_use]
    pub fn mass_fraction(&self, species: GapGasSpecies) -> f64 {
        self.mass_fractions[species.index()]
    }

    /// Mole fraction \[-\] of one species — upstream's `M_`.
    #[must_use]
    pub fn mole_fraction(&self, species: GapGasSpecies) -> f64 {
        self.mole_fractions[species.index()]
    }

    /// All mass fractions \[-\], indexed by [`GapGasSpecies::index`].
    #[must_use]
    pub fn mass_fractions(&self) -> [f64; N_SPECIES] {
        self.mass_fractions
    }

    /// All mole fractions \[-\], indexed by [`GapGasSpecies::index`].
    #[must_use]
    pub fn mole_fractions(&self) -> [f64; N_SPECIES] {
        self.mole_fractions
    }

    /// Total gas mass \[kg\] — upstream's `gasM_`.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.mass
    }

    /// Total gas amount \[mol\].
    ///
    /// `n = Σ_i Y_i · m / M_i`, the quantity upstream computes inline as `molN`
    /// in `gapFRAPCON::correct()` before applying the ideal-gas law.
    #[must_use]
    pub fn moles(&self) -> f64 {
        let mut n = 0.0;
        for (i, y) in self.mass_fractions.iter().enumerate() {
            n += y * self.mass / GapGasSpecies::ALL[i].molar_mass();
        }
        n
    }

    /// Specific gas constant of the mixture \[J/(kg·K)\] — upstream's
    /// `R_mixture` inside `rhoMixture`.
    ///
    /// `R_mix = Σ_i Y_i · R / M_i`, the mass-weighted mean of the per-species
    /// specific gas constants.
    #[must_use]
    pub fn specific_gas_constant(&self) -> f64 {
        let mut r = 0.0;
        for (i, y) in self.mass_fractions.iter().enumerate() {
            r += GAS_CONSTANT / GapGasSpecies::ALL[i].molar_mass() * y;
        }
        r
    }

    /// Ideal-gas mixture density \[kg/m³\] at pressure `p` \[Pa\] and
    /// temperature `t` \[K\] — upstream's `gapFRAPCON::rhoMixture`.
    ///
    /// `ρ = p / (R_mix · T)`. Returns `0.0` for a non-positive temperature
    /// rather than dividing by zero.
    ///
    /// # Assumptions
    ///
    /// Ideal gas. At rod-plenum conditions (a few MPa, several hundred kelvin)
    /// helium is within about 1% of ideal; a xenon-rich mixture at high pressure
    /// is less so, and this model does not correct for it.
    #[must_use]
    pub fn density(&self, p: f64, t: f64) -> f64 {
        if !(t > 0.0) {
            return 0.0;
        }
        let r = self.specific_gas_constant();
        if !(r > 0.0) {
            return 0.0;
        }
        p / (r * t)
    }

    /// Specific heat capacity at constant pressure \[J/(kg·K)\] of the mixture
    /// as an ideal monatomic gas: `c_p = (5/2) · R_mix`, with `R_mix` from
    /// [`Self::specific_gas_constant`].
    ///
    /// Exact for an ideal gas of the six noble gases here, all monatomic
    /// (`c_p = 5R/2` per mole, no rotational or vibrational modes), and
    /// independent of temperature. Not an OFFBEAT upstream quantity: added for
    /// the RELAP-7 heat-structure set (gh:#1070), whose manual (§4.2.3) derives
    /// its gap-gas `c_p` from the perfect-gas relation the same way. On the
    /// RELAP-7 composition (He/Kr/Xe = 0.1066/0.1340/0.7594 by mole) this gives
    /// 186.649 J/(kg·K) against the manual's 186.65.
    ///
    /// # Assumptions
    ///
    /// Ideal gas; real-gas corrections at a few MPa are below 1 % for helium
    /// and somewhat larger for xenon-rich gas, and are not applied.
    #[must_use]
    pub fn specific_heat_capacity(&self) -> f64 {
        2.5 * self.specific_gas_constant()
    }

    /// Mixture thermal conductivity \[W/m/K\] at temperature `t` \[K\] —
    /// upstream's `gapFRAPCON::kappa(T)`.
    ///
    /// Evaluates the Lindsay–Bromley/Wassiljewa rule written out in the
    /// [module documentation](self). Reduces **exactly** to the pure-gas value
    /// when one mole fraction is 1.
    ///
    /// # Valid range
    ///
    /// `t > 0`; returns `0.0` otherwise. See
    /// [`GapGasSpecies::conductivity_coefficients`] for the per-species fits.
    ///
    /// # Caveat
    ///
    /// If the mixture contains neon or radon the result is meaningless because
    /// upstream's coefficients for those two are placeholders — use
    /// [`Self::conductivity_checked`] to be told rather than to guess.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{GapGasMixture, GapGasSpecies};
    ///
    /// let he = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
    /// let xe = GapGasMixture::pure(GapGasSpecies::Xenon, 1.0e-4).unwrap();
    /// assert!(he.conductivity(1000.0) > 20.0 * xe.conductivity(1000.0));
    /// ```
    #[must_use]
    pub fn conductivity(&self, t: f64) -> f64 {
        if !(t > 0.0) || !t.is_finite() {
            return 0.0;
        }

        let mut k = [0.0; N_SPECIES];
        for (ki, species) in k.iter_mut().zip(GapGasSpecies::ALL) {
            *ki = species.conductivity(t);
        }
        self.mix_conductivities(&k)
    }

    /// Mixture conductivity \[W/m/K\] at `t` \[K\] from a chosen set of
    /// pure-gas fits, through the same mixing rule as [`Self::conductivity`].
    ///
    /// `conductivity_with_fit(GasConductivityFit::Frapcon, t)` equals
    /// [`Self::conductivity_checked`]`(t)` bit-for-bit.
    /// `GasConductivityFit::Matpro` is the RELAP-7 heat-structure choice.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] for a non-positive or non-finite `t`.
    /// - [`OffbeatError::NotImplemented`] if a species with a non-zero mole
    ///   fraction has no fit in the chosen set (neon, argon or radon under
    ///   `Matpro`), or has the placeholder fit (neon or radon under
    ///   `Frapcon`).
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{GapGasMixture, GapGasSpecies};
    /// use outram_park_fork_offbeat::gap::gas::GasConductivityFit;
    ///
    /// let he = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
    /// let k = he.conductivity_with_fit(GasConductivityFit::Matpro, 1000.0).unwrap();
    /// assert!((k - 2.639e-3 * 1000f64.powf(0.7085)).abs() < 1e-12);
    /// ```
    pub fn conductivity_with_fit(&self, fit: GasConductivityFit, t: f64) -> Result<f64> {
        if fit == GasConductivityFit::Frapcon {
            return self.conductivity_checked(t);
        }
        if !(t > 0.0) || !t.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "gap gas temperature",
                value: t,
                unit: "K",
                reason: "must be finite and strictly positive",
            });
        }
        let mut k = [0.0; N_SPECIES];
        for (ki, species) in k.iter_mut().zip(GapGasSpecies::ALL) {
            match species.conductivity_coefficients_for(fit) {
                Some((a, b)) => *ki = a * t.r_powf(b),
                None if self.mole_fraction(species) > 0.0 => {
                    return Err(OffbeatError::NotImplemented(
                        "MATPRO gap gas conductivity for a mixture containing neon, \
                         argon or radon (only He, Kr and Xe MATPRO fits are carried)",
                    ));
                }
                // Absent species: never read by the mixing rule.
                None => *ki = 0.0,
            }
        }
        Ok(self.mix_conductivities(&k))
    }

    /// The Lindsay–Bromley/Wassiljewa mixing rule over given pure-gas
    /// conductivities `k` \[W/m/K\], indexed by [`GapGasSpecies::index`].
    /// Species with zero mole fraction are skipped, so their `k` is never
    /// read. Shared by [`Self::conductivity`] and
    /// [`Self::conductivity_with_fit`].
    fn mix_conductivities(&self, k: &[f64; N_SPECIES]) -> f64 {
        let mut k_mix = 0.0;
        // Index-based double loop, mirroring upstream's `forAll(species_, ...)`
        // pair: the inner loop needs `k[j]` for arbitrary `j`, and the `i == j`
        // skip is upstream's Kronecker `deltas_`. An iterator rewrite would
        // obscure the correspondence.
        #[allow(clippy::needless_range_loop)]
        for i in 0..N_SPECIES {
            let x_i = self.mole_fractions[i];
            // A species that is absent contributes nothing, and skipping it also
            // avoids a 0/0 if it were the only species considered.
            if x_i <= 0.0 {
                continue;
            }
            let m_i = GapGasSpecies::ALL[i].molar_mass();
            let mut sum_term = 0.0;
            for j in 0..N_SPECIES {
                if i == j {
                    continue;
                }
                let x_j = self.mole_fractions[j];
                if x_j <= 0.0 {
                    continue;
                }
                let m_j = GapGasSpecies::ALL[j].molar_mass();
                let ratio_m = m_i / m_j;
                let phi = (1.0 + (k[i] / k[j]).sqrt() * ratio_m.r_powf(0.25)).r_powi(2)
                    / (2.0f64.r_powf(1.5) * (1.0 + ratio_m).sqrt());
                let psi =
                    phi * (1.0 + 2.41 * ((m_i - m_j) * (m_i - 0.142 * m_j)) / (m_i + m_j).r_powi(2));
                sum_term += psi * x_j;
            }
            k_mix += k[i] * x_i / (x_i + sum_term);
        }
        k_mix
    }

    /// [`Self::conductivity`], but refusing a mixture whose conductivity would
    /// be meaningless.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] for `t <= 0`.
    /// - [`OffbeatError::NotImplemented`] if the mixture contains a non-zero
    ///   mole fraction of a species whose upstream conductivity coefficients are
    ///   the `(1.0, 1.0)` placeholder — neon or radon. Upstream would happily
    ///   return a number four orders of magnitude too large; this port declines.
    pub fn conductivity_checked(&self, t: f64) -> Result<f64> {
        if !(t > 0.0) || !t.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "gap gas temperature",
                value: t,
                unit: "K",
                reason: "must be finite and strictly positive",
            });
        }
        for s in GapGasSpecies::ALL {
            if s.has_placeholder_conductivity() && self.mole_fraction(s) > 0.0 {
                return Err(OffbeatError::NotImplemented(
                    "gap gas conductivity for a mixture containing neon or radon \
                     (upstream OFFBEAT's default A=1.0, B=1.0 coefficients for \
                     those two species are placeholders, not a physical fit)",
                ));
            }
        }
        Ok(self.conductivity(t))
    }

    /// Mixture thermal accommodation coefficient at temperature `t` \[K\] —
    /// upstream's `gapFRAPCON::a(T)`.
    ///
    /// # What it is for
    ///
    /// It appears only in the temperature-jump distance
    /// ([`crate::gap::conductance::temperature_jump_distance`]): gas molecules do
    /// not fully equilibrate with a solid surface in one collision, so a
    /// discontinuity in temperature sits at each wall, equivalent to adding an
    /// extra thickness of gas to the gap. A *smaller* accommodation coefficient
    /// means a *poorer* exchange and a *larger* effective gap.
    ///
    /// # The correlation
    ///
    /// ```text
    /// a_He(T) = 0.425 − 2.3e−4 · min(T, 1300)
    /// a_Xe(T) = 0.749 − 2.5e−4 · min(T, 1300)
    /// a_i     = a_He + (W_i − W_He)/(W_Xe − W_He) · (a_Xe − a_He)     [W in g/mol]
    /// a_mix   = Σ_i x_i · a_i / sqrt(W_i)
    /// ```
    ///
    /// The per-species value is a linear interpolation in molar mass between the
    /// helium and xenon endpoints, so helium and xenon return their own
    /// endpoints exactly and the other four are interpolated (argon, krypton) or
    /// extrapolated (radon, which is heavier than xenon).
    ///
    /// # Upstream defect, reproduced deliberately
    ///
    /// **The final mixture sum is not normalised and is not dimensionless.**
    /// Weighting by `1/sqrt(W_i)` with `W` in g/mol means `a_mix` does *not*
    /// reduce to `a_i` for a pure gas: pure helium at 300 K gives
    /// `a_He/sqrt(4.0026) ≈ 0.1780` rather than `a_He ≈ 0.3560`. A genuine
    /// accommodation coefficient is dimensionless and bounded by 1. This port
    /// reproduces upstream exactly, because the empirical constant `0.0137` in
    /// the jump-distance formula was fitted with this scaling baked in, and
    /// "fixing" the normalisation here would silently shift every gap
    /// temperature. Treat the return value as *upstream's `a`*, meaningful only
    /// as the denominator of
    /// [`crate::gap::conductance::temperature_jump_distance`] — not as a physical
    /// accommodation coefficient.
    ///
    /// # Valid range
    ///
    /// `t > 0`; frozen above 1300 K by the `min(T, 1300)` clamp, so the return
    /// value is constant for hotter gas. Returns `0.0` for a non-positive
    /// temperature.
    #[must_use]
    pub fn accommodation_coefficient(&self, t: f64) -> f64 {
        if !(t > 0.0) || !t.is_finite() {
            return 0.0;
        }
        let t_eff = t.min(ACCOMMODATION_T_CAP);
        let a_he = 0.425 - 2.3e-4 * t_eff;
        let a_xe = 0.749 - 2.5e-4 * t_eff;
        let w_he = GapGasSpecies::Helium.molar_mass_g_per_mol();
        let w_xe = GapGasSpecies::Xenon.molar_mass_g_per_mol();

        let mut a_mix = 0.0;
        for i in 0..N_SPECIES {
            let w_i = GapGasSpecies::ALL[i].molar_mass_g_per_mol();
            let a_i = a_he + (w_i - w_he) / (w_xe - w_he) * (a_xe - a_he);
            a_mix += self.mole_fractions[i] * a_i / w_i.sqrt();
        }
        a_mix
    }

    /// Add fission gas released from the fuel, in **moles per species** —
    /// upstream's `gapGasModel::correctMass()`.
    ///
    /// The released amounts are added to the existing inventory, the total mass
    /// is increased accordingly, and both fraction arrays are re-normalised.
    /// This is the mechanism by which released xenon and krypton progressively
    /// dilute the helium fill and degrade the gap.
    ///
    /// `released` is indexed by [`GapGasSpecies::index`] and carries **moles**
    /// \[mol\], matching the `fissionGasRelease::gasMols()` interface upstream
    /// consumes.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative or non-finite released
    /// amount, or if the resulting mixture has zero mass.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{GapGasMixture, GapGasSpecies};
    ///
    /// let mut mix = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-5).unwrap();
    /// let k_before = mix.conductivity(1000.0);
    ///
    /// // Release 1 mmol of xenon into the gap.
    /// let mut released = [0.0; 6];
    /// released[GapGasSpecies::Xenon.index()] = 1.0e-3;
    /// mix.add_released_gas(released).unwrap();
    ///
    /// // Gap gas conducts worse than it did.
    /// assert!(mix.conductivity(1000.0) < k_before);
    /// ```
    pub fn add_released_gas(&mut self, released: [f64; N_SPECIES]) -> Result<()> {
        let mut absolute = [0.0; N_SPECIES];
        let mut new_mass = self.mass;
        for i in 0..N_SPECIES {
            let moles = released[i];
            if !moles.is_finite() || moles < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity: GapGasSpecies::ALL[i].symbol(),
                    value: moles,
                    unit: "mol",
                    reason: "released fission-gas amount must be finite and non-negative",
                });
            }
            let added = moles * GapGasSpecies::ALL[i].molar_mass();
            absolute[i] = self.mass_fractions[i] * self.mass + added;
            new_mass += added;
        }
        if !(new_mass > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "gap gas mass after release",
                value: new_mass,
                unit: "kg",
                reason: "must be strictly positive",
            });
        }
        *self = Self::from_mass_fractions(absolute, new_mass)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference-checked against upstream's own tabulated coefficients.
    ///
    /// **Methodology.** Upstream `gapFRAPCON.C` lines 1035–1041 tabulate the
    /// default molar masses; lines 1069–1075 and 1102–1107 the conductivity
    /// coefficients. This test asserts this port carries the same numbers
    /// exactly (tolerance: bitwise equality), because a silent typo in a molar
    /// mass would shift every mixture property.
    ///
    /// **Result** (2026-07-29): all six molar masses and all six `(A, B)` pairs
    /// match upstream verbatim.
    #[test]
    fn species_constants_match_upstream_tables() {
        assert_eq!(GapGasSpecies::Xenon.molar_mass_g_per_mol(), 131.3);
        assert_eq!(GapGasSpecies::Neon.molar_mass_g_per_mol(), 20.183);
        assert_eq!(GapGasSpecies::Argon.molar_mass_g_per_mol(), 39.948);
        assert_eq!(GapGasSpecies::Krypton.molar_mass_g_per_mol(), 83.8);
        assert_eq!(GapGasSpecies::Radon.molar_mass_g_per_mol(), 222.0);
        assert_eq!(GapGasSpecies::Helium.molar_mass_g_per_mol(), 4.0026);

        assert_eq!(
            GapGasSpecies::Helium.conductivity_coefficients(),
            (2.531e-3, 0.7146)
        );
        assert_eq!(
            GapGasSpecies::Xenon.conductivity_coefficients(),
            (9.825e-5, 0.7334)
        );
        assert_eq!(
            GapGasSpecies::Krypton.conductivity_coefficients(),
            (1.966e-4, 0.7006)
        );
        assert_eq!(
            GapGasSpecies::Argon.conductivity_coefficients(),
            (4.092e-4, 0.6748)
        );
        // The two placeholders, reproduced deliberately.
        assert_eq!(GapGasSpecies::Neon.conductivity_coefficients(), (1.0, 1.0));
        assert_eq!(GapGasSpecies::Radon.conductivity_coefficients(), (1.0, 1.0));
        assert!(GapGasSpecies::Neon.has_placeholder_conductivity());
        assert!(GapGasSpecies::Radon.has_placeholder_conductivity());
        assert!(!GapGasSpecies::Helium.has_placeholder_conductivity());
    }

    /// Self-consistency check — the mixture rule's pure-gas limit.
    ///
    /// **Methodology.** The Lindsay–Bromley sum has an empty inner sum when one
    /// mole fraction is 1, so `k_mix = k_i · 1/(1 + 0) = k_i` *exactly*, not
    /// approximately. Evaluated for each of the six species at 800 K against
    /// `A·T^B` computed directly. Tolerance: 1e-15 relative — this is an
    /// algebraic identity, not a numerical agreement. **This is not a validation
    /// against measured conductivity data.**
    ///
    /// **Result** (2026-07-29): all six species reduce exactly; the largest
    /// observed relative deviation was 0.
    #[test]
    fn mixture_rule_reduces_exactly_to_the_pure_gas_value() {
        for s in GapGasSpecies::ALL {
            let mix = GapGasMixture::pure(s, 1.0e-4).unwrap();
            let mixed = mix.conductivity(800.0);
            let pure = s.conductivity(800.0);
            assert!(
                (mixed - pure).abs() <= 1e-15 * pure.abs(),
                "{}: mixture {mixed} != pure {pure}",
                s.symbol()
            );
        }
    }

    /// Self-consistency check — helium conducts far better than xenon, and the
    /// port's own computed values are recorded.
    ///
    /// **Methodology.** Evaluate the pure-gas fits at 300 K and 1000 K and
    /// record what this port produces. The physical expectation is only the
    /// ordering `k_He >> k_Xe` (helium is the lightest noble gas and the best
    /// conductor of the six; xenon the heaviest tabulated real fill/fission gas
    /// and the worst). **The absolute numbers below are what this code computes,
    /// not values taken from a reference — this is a regression anchor, not a
    /// validation.**
    ///
    /// **Result** (2026-07-29, measured):
    ///
    /// | T \[K\] | `k_He` \[W/m/K\] | `k_Xe` \[W/m/K\] | ratio |
    /// |---|---|---|---|
    /// | 300 | 0.1490881 | 0.00644248 | 23.141 |
    /// | 1000 | 0.3524456 | 0.01557875 | 22.624 |
    ///
    /// For orientation only (not asserted, and not a cited reference): the
    /// textbook thermal conductivity of helium near room temperature is of order
    /// 0.15 W/m/K and of xenon of order 0.006 W/m/K, so the fits are the right
    /// order of magnitude.
    #[test]
    fn helium_conducts_far_better_than_xenon() {
        let k_he_300 = GapGasSpecies::Helium.conductivity(300.0);
        let k_xe_300 = GapGasSpecies::Xenon.conductivity(300.0);
        let k_he_1000 = GapGasSpecies::Helium.conductivity(1000.0);
        let k_xe_1000 = GapGasSpecies::Xenon.conductivity(1000.0);

        assert!(
            (k_he_300 - 0.149_088_1).abs() < 1e-6,
            "k_He(300) = {k_he_300}"
        );
        assert!(
            (k_xe_300 - 0.006_442_48).abs() < 1e-7,
            "k_Xe(300) = {k_xe_300}"
        );
        assert!(
            (k_he_1000 - 0.352_445_6).abs() < 1e-6,
            "k_He(1000) = {k_he_1000}"
        );
        assert!(
            (k_xe_1000 - 0.015_578_75).abs() < 1e-7,
            "k_Xe(1000) = {k_xe_1000}"
        );

        assert!((k_he_300 / k_xe_300 - 23.141).abs() < 1e-2);
        assert!((k_he_1000 / k_xe_1000 - 22.624).abs() < 1e-2);
    }

    /// Self-consistency check — fission-gas release monotonically degrades the
    /// gap gas.
    ///
    /// **Methodology.** Start from pure helium and add xenon in ten equal
    /// increments, recording the mixture conductivity at 1000 K after each.
    /// Pass criterion: the sequence is strictly decreasing, it never falls below
    /// the pure-xenon value, and it never rises above the pure-helium value.
    /// This is the positive-feedback loop the module documentation describes;
    /// **it is an ordering property, not a validation against data.**
    ///
    /// **Result** (2026-07-29, measured): strictly decreasing over all ten steps.
    /// Adding 10 mmol of xenon to 1e-5 kg (2.50 mmol) of helium takes the xenon
    /// mole fraction to 0.8001, and the mixture conductivity at 1000 K falls
    /// from 0.3524456 to 0.0354082 W/m/K — a factor of 9.95.
    #[test]
    fn released_xenon_monotonically_degrades_conductivity() {
        let mut mix = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-5).unwrap();
        let pure_he = mix.conductivity(1000.0);
        let pure_xe = GapGasSpecies::Xenon.conductivity(1000.0);

        let mut previous = pure_he;
        for _ in 0..10 {
            let mut released = [0.0; N_SPECIES];
            released[GapGasSpecies::Xenon.index()] = 1.0e-3;
            mix.add_released_gas(released).unwrap();
            let k = mix.conductivity(1000.0);
            assert!(k < previous, "conductivity rose: {k} >= {previous}");
            assert!(k > pure_xe, "fell below the pure-xenon floor: {k}");
            assert!(k < pure_he, "rose above the pure-helium ceiling: {k}");
            previous = k;
        }
        assert!(
            (previous - 0.035_408_2).abs() < 1e-6,
            "final mixture conductivity = {previous}"
        );
        let x_xe = mix.mole_fraction(GapGasSpecies::Xenon);
        assert!((x_xe - 0.800_104).abs() < 1e-5, "x_Xe = {x_xe}");
    }

    /// Self-consistency check — mass and mole fractions stay normalised, and the
    /// mole basis is the mass basis re-weighted by molar mass.
    ///
    /// **Methodology.** A 90/10 helium/xenon mixture by mass. Assert both arrays
    /// sum to 1 within 1e-14, and that helium's mole fraction exceeds its mass
    /// fraction (xenon being 32.8x heavier per mole, a mass-minority xenon is an
    /// even smaller mole-minority).
    ///
    /// **Result** (2026-07-29, measured): mass fractions sum to 1.0 exactly and
    /// mole fractions sum to 1.0 exactly; helium mole fraction 0.9966243
    /// against a mass fraction of 0.9.
    #[test]
    fn fraction_arrays_stay_normalised() {
        let mut y = [0.0; N_SPECIES];
        y[GapGasSpecies::Helium.index()] = 0.9;
        y[GapGasSpecies::Xenon.index()] = 0.1;
        let mix = GapGasMixture::from_mass_fractions(y, 1.0e-3).unwrap();

        let sum_y: f64 = mix.mass_fractions().iter().sum();
        let sum_x: f64 = mix.mole_fractions().iter().sum();
        assert!((sum_y - 1.0).abs() < 1e-14, "mass fractions sum to {sum_y}");
        assert!((sum_x - 1.0).abs() < 1e-14, "mole fractions sum to {sum_x}");

        let x_he = mix.mole_fraction(GapGasSpecies::Helium);
        assert!(x_he > mix.mass_fraction(GapGasSpecies::Helium));
        assert!((x_he - 0.996_624_3).abs() < 1e-6, "x_He = {x_he}");
    }

    /// Self-consistency check — unnormalised input is normalised, matching
    /// upstream's `correctMassFractions()`.
    #[test]
    fn unnormalised_mass_fractions_are_rescaled() {
        let mut y = [0.0; N_SPECIES];
        y[GapGasSpecies::Helium.index()] = 9.0;
        y[GapGasSpecies::Krypton.index()] = 1.0;
        let mix = GapGasMixture::from_mass_fractions(y, 1.0).unwrap();
        assert!((mix.mass_fraction(GapGasSpecies::Helium) - 0.9).abs() < 1e-15);
        assert!((mix.mass_fraction(GapGasSpecies::Krypton) - 0.1).abs() < 1e-15);
    }

    /// Self-consistency check — the ideal-gas law round-trips.
    ///
    /// **Methodology.** For pure helium, `ρ = p/(R_mix T)` must satisfy
    /// `p = ρ R_mix T`, and `R_mix` must equal `R / M_He`. Tolerance 1e-12
    /// relative.
    ///
    /// **Result** (2026-07-29, measured): `R_mix = 2077.2654 J/(kg·K)` for
    /// helium; the round-trip closed to within 1e-16 relative.
    #[test]
    fn ideal_gas_density_round_trips() {
        let mix = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
        let r = mix.specific_gas_constant();
        assert!((r - GAS_CONSTANT / GapGasSpecies::Helium.molar_mass()).abs() < 1e-9);
        assert!((r - 2077.2654).abs() < 1e-3, "R_mix = {r}");

        let p = 2.25e6;
        let t = 600.0;
        let rho = mix.density(p, t);
        assert!((rho * r * t - p).abs() < 1e-12 * p);
    }

    /// Self-consistency check — moles are consistent with mass and composition.
    ///
    /// **Methodology.** 1 g of pure xenon must be `1e-3 / 0.1313 = 7.6161e-3`
    /// mol. Tolerance 1e-12 relative.
    #[test]
    fn moles_follow_from_mass_and_molar_mass() {
        let mix = GapGasMixture::pure(GapGasSpecies::Xenon, 1.0e-3).unwrap();
        let expected = 1.0e-3 / GapGasSpecies::Xenon.molar_mass();
        assert!((mix.moles() - expected).abs() < 1e-12 * expected);
    }

    /// Reproduced upstream defect — the accommodation coefficient does **not**
    /// reduce to the pure-gas value.
    ///
    /// **Methodology.** Upstream `gapFRAPCON::a()` sums
    /// `Σ x_i a_i / sqrt(W_i)` without normalising by `Σ x_i / sqrt(W_i)`. For
    /// pure helium at 300 K the endpoint correlation gives
    /// `a_He = 0.425 − 2.3e−4·300 = 0.3560`, so a correctly normalised mixture
    /// rule would return 0.3560; upstream returns `0.3560/sqrt(4.0026)`.
    /// This test pins the *upstream* behaviour deliberately, so that a future
    /// "fix" cannot land unnoticed.
    ///
    /// **Result** (2026-07-29, measured): pure helium at 300 K returns
    /// `0.1779422`, i.e. `a_He / 2.00065`, confirming the missing
    /// normalisation. Pure xenon at 300 K returns `0.0588203` against
    /// `a_Xe = 0.6740`.
    #[test]
    fn accommodation_coefficient_reproduces_upstream_missing_normalisation() {
        let he = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
        let a_he_endpoint: f64 = 0.425 - 2.3e-4 * 300.0;
        let a_mix = he.accommodation_coefficient(300.0);
        assert!((a_he_endpoint - 0.3560).abs() < 1e-12);
        assert!(
            (a_mix - 0.177_942_2).abs() < 1e-6,
            "a_mix(pure He) = {a_mix}"
        );
        assert!(
            (a_mix - a_he_endpoint / GapGasSpecies::Helium.molar_mass_g_per_mol().sqrt()).abs()
                < 1e-12
        );

        let xe = GapGasMixture::pure(GapGasSpecies::Xenon, 1.0e-4).unwrap();
        let a_xe_endpoint: f64 = 0.749 - 2.5e-4 * 300.0;
        let a_xe_mix = xe.accommodation_coefficient(300.0);
        assert!((a_xe_endpoint - 0.6740).abs() < 1e-12);
        assert!(
            (a_xe_mix - 0.058_820_3).abs() < 1e-6,
            "a_mix(pure Xe) = {a_xe_mix}"
        );
    }

    /// Self-consistency check — the accommodation correlation is frozen above
    /// 1300 K, as upstream's `min(T, 1300)` requires.
    #[test]
    fn accommodation_coefficient_is_frozen_above_the_cap() {
        let mix = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
        let at_cap = mix.accommodation_coefficient(ACCOMMODATION_T_CAP);
        assert!((mix.accommodation_coefficient(2000.0) - at_cap).abs() < 1e-15);
        assert!((mix.accommodation_coefficient(5000.0) - at_cap).abs() < 1e-15);
        // Below the cap it still varies.
        assert!(mix.accommodation_coefficient(600.0) > at_cap);
    }

    /// Self-consistency check — the placeholder guard fires.
    #[test]
    fn conductivity_checked_rejects_placeholder_species() {
        let ne = GapGasMixture::pure(GapGasSpecies::Neon, 1.0e-4).unwrap();
        assert!(matches!(
            ne.conductivity_checked(600.0),
            Err(OffbeatError::NotImplemented(_))
        ));
        // ... but the unchecked path still reproduces upstream's number.
        assert!((ne.conductivity(600.0) - 600.0).abs() < 1e-9);

        let he = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
        assert!(he.conductivity_checked(600.0).is_ok());
        assert!(he.conductivity_checked(-1.0).is_err());
    }

    /// Self-consistency check — invalid compositions are rejected rather than
    /// producing NaNs, matching upstream's fatal errors.
    #[test]
    fn invalid_compositions_are_rejected() {
        assert!(GapGasMixture::from_mass_fractions([0.0; N_SPECIES], 1.0).is_err());
        let mut negative = [0.0; N_SPECIES];
        negative[GapGasSpecies::Helium.index()] = -1.0;
        assert!(GapGasMixture::from_mass_fractions(negative, 1.0).is_err());
        assert!(GapGasMixture::pure(GapGasSpecies::Helium, -1.0).is_err());

        let mut mix = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-5).unwrap();
        let mut bad = [0.0; N_SPECIES];
        bad[GapGasSpecies::Xenon.index()] = -1.0;
        assert!(mix.add_released_gas(bad).is_err());
    }

    /// RELAP-7 gap-gas composition (§4.2.3): He/Kr/Xe = 0.1066/0.1340/0.7594
    /// by mole.
    fn relap7_mixture() -> GapGasMixture {
        let mut x = [0.0; N_SPECIES];
        x[GapGasSpecies::Helium.index()] = 0.1066;
        x[GapGasSpecies::Krypton.index()] = 0.1340;
        x[GapGasSpecies::Xenon.index()] = 0.7594;
        GapGasMixture::from_mole_fractions(x, 1.0e-4).unwrap()
    }

    /// **Verification against hand values of the published fits** (gh:#1070).
    ///
    /// *Methodology.* The MATPRO pure-gas fits (NUREG/CR-6150 §12.1.1, as
    /// reproduced in RELAP-7 Theory Manual §4.2.3) `k = A T^B` are evaluated
    /// by hand in double precision (Python `**`, 2026-11-01):
    /// He `2.639e-3 * 300^0.7085 = 0.150134257`, `* 1000^0.7085 = 0.352321698`;
    /// Kr `8.247e-5 * 300^0.8363 = 9.72551258e-3`;
    /// Xe `4.351e-5 * 300^0.8616 = 5.92757089e-3`, `* 1000^0.8616 = 1.67258841e-2`.
    /// Pass criterion: `< 1e-12` relative, through the pure-gas mixture path
    /// (which must reduce exactly to `k_i`).
    ///
    /// *Result* (2026-11-01): all agree. For comparison the FRAPCON fits give
    /// He 0.14909 and Xe 0.0064425 W/m/K at 300 K — MATPRO is 0.7 % higher
    /// for helium and 8.0 % lower for xenon at 300 K, so the choice is not
    /// cosmetic.
    #[test]
    fn matpro_pure_gas_fits_match_hand_values() {
        let cases = [
            (GapGasSpecies::Helium, 300.0, 0.150_134_257_055_189_57),
            (GapGasSpecies::Helium, 1000.0, 0.352_321_697_885_509_9),
            (GapGasSpecies::Krypton, 300.0, 9.725_512_578_517_583e-3),
            (GapGasSpecies::Xenon, 300.0, 5.927_570_887_140_366e-3),
            (GapGasSpecies::Xenon, 1000.0, 1.672_588_410_865_923_8e-2),
        ];
        for (species, t, hand) in cases {
            let mix = GapGasMixture::pure(species, 1.0e-4).unwrap();
            let k = mix.conductivity_with_fit(GasConductivityFit::Matpro, t).unwrap();
            assert!(((k - hand) / hand).abs() < 1e-12, "{} at {t} K: {k}", species.symbol());
        }
        assert_eq!(GapGasSpecies::Argon.matpro_conductivity_coefficients(), None);
        assert_eq!(
            GapGasSpecies::Xenon.conductivity_coefficients_for(GasConductivityFit::Frapcon),
            Some(GapGasSpecies::Xenon.conductivity_coefficients())
        );
        assert_eq!(GasConductivityFit::default(), GasConductivityFit::Frapcon);
    }

    /// **Verification against hand values, plus a check on the RELAP-7
    /// manual's stated gap-gas numbers** (gh:#1070).
    ///
    /// *Methodology.* The RELAP-7 composition (He/Kr/Xe = 0.1066/0.1340/0.7594
    /// by mole) with the MATPRO fits, through this module's mixing rule
    /// (`(M_i + M_j)²` form). Hand values from an independent Python
    /// implementation of the same rule (2026-11-01), W/m/K:
    /// `k_mix(300) = 0.0103922505`, `k_mix(1000) = 0.0283825480`,
    /// `k_mix(2000) = 0.0505903864`. The minus form printed in the manual's
    /// Eq 233 gives 0.0064665, 0.0174171 and 0.0307851 instead (not used;
    /// see the module doc). Density and `c_p` against the manual's §4.2.3
    /// statements: `M_mix = 111.365 g/mol`, so at 4.1 MPa and 300 K
    /// `rho = p M/(R T) = 183.053 kg/m³` (manual: 183.06) and
    /// `c_p = 2.5 R / M = 186.649 J/(kg K)` (manual: 186.65).
    /// Pass criteria: `< 1e-10` relative on `k_mix`; `< 0.01` absolute on
    /// `rho` and `c_p` against the manual's rounded figures.
    ///
    /// *Result* (2026-11-01): all pass. The 0.007 kg/m³ density gap is the
    /// manual's rounding and gas constant, not a physics difference.
    /// Interpretation: the port reproduces the published fits and the
    /// manual's derived gap-gas density and heat capacity; the mixture
    /// conductivity is not stated as a number in the manual, so it is checked
    /// against the hand calculation only.
    #[test]
    fn relap7_gap_gas_mixture_matches_hand_values() {
        let mix = relap7_mixture();
        for (t, hand) in [
            (300.0, 0.010_392_250_534_062_232),
            (1000.0, 0.028_382_547_970_259_835),
            (2000.0, 0.050_590_386_374_433_4),
        ] {
            let k = mix.conductivity_with_fit(GasConductivityFit::Matpro, t).unwrap();
            assert!(((k - hand) / hand).abs() < 1e-10, "{t} K: {k}");
        }
        assert!((mix.density(4.1e6, 300.0) - 183.06).abs() < 0.01);
        assert!((mix.density(4.1e6, 300.0) - 183.053_280_904_974_86).abs() < 1e-6);
        assert!((mix.specific_heat_capacity() - 186.65).abs() < 0.01);
        assert!((mix.specific_heat_capacity() - 186.648_753_290_595_15).abs() < 1e-6);
        // Frapcon path through the new method is the old checked method.
        assert_eq!(
            mix.conductivity_with_fit(GasConductivityFit::Frapcon, 800.0).unwrap(),
            mix.conductivity_checked(800.0).unwrap()
        );
    }

    /// Self-consistency check — the MATPRO set refuses species it has no fit
    /// for, and pure helium's `c_p` is the textbook `5R/(2 M_He)`
    /// (5193.16 J/(kg K); the commonly tabulated helium value is 5193).
    #[test]
    fn matpro_fit_refuses_unfitted_species_and_cp_is_monatomic() {
        let ar = GapGasMixture::pure(GapGasSpecies::Argon, 1.0e-4).unwrap();
        assert!(matches!(
            ar.conductivity_with_fit(GasConductivityFit::Matpro, 600.0),
            Err(OffbeatError::NotImplemented(_))
        ));
        assert!(relap7_mixture()
            .conductivity_with_fit(GasConductivityFit::Matpro, -5.0)
            .is_err());
        let he = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-4).unwrap();
        let cp = he.specific_heat_capacity();
        assert!((cp - 2.5 * GAS_CONSTANT / 4.0026e-3).abs() < 1e-9);
        assert!((cp - 5193.16).abs() < 0.01, "{cp}");
        let mut bad = [0.0; N_SPECIES];
        bad[GapGasSpecies::Helium.index()] = f64::NAN;
        assert!(GapGasMixture::from_mole_fractions(bad, 1.0).is_err());
    }
}
