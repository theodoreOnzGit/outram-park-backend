//! Consumer surface: incoherent-inelastic thermal scattering for Monte Carlo.
//!
//! This is the **data surface `outram-mc-libs` consumes** for the thermal
//! S(α,β) treatment of a THERMAL pincell. Where [`mf7`](super::mf7) and
//! [`inelastic`](super::inelastic) are the *reader* and *physics kernel*, this
//! module is the *hand-off*: one struct, [`IncoherentInelasticScattering`],
//! built once per material + temperature, that answers the two questions a
//! transport code asks at a thermal collision —
//!
//! 1. **How likely is a bound-inelastic scatter?** → [`inelastic_xs`] returns
//!    `σ_inel(E)` *per principal atom* (per H for H-in-H₂O) as a `uom`
//!    [`CrossSection`]; multiply by the principal-atom number density.
//! 2. **Where does the neutron go?** → [`emission`] returns equally-probable
//!    outgoing-energy / cosine bins to sample the secondary energy and angle.
//!
//! ```no_run
//! use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
//! use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
//! use uom::si::{energy::electronvolt, thermodynamic_temperature::kelvin, area::barn};
//!
//! // H in light water, ENDF/B-VIII.0 tsl, MAT = 1, at room temperature.
//! let path = "tsl-HinH2O.endf";
//! let t = Temperature::new::<kelvin>(293.6);
//! let sab = IncoherentInelasticScattering::from_endf_file(path, 1, t).unwrap();
//!
//! // σ_inel per H atom at 0.0253 eV.
//! let e = NeutronEnergy::new::<electronvolt>(0.0253);
//! let xs = sab.inelastic_xs(e).get::<barn>();
//!
//! // Secondary distribution: 16 equiprobable E', 8 equiprobable cosines each.
//! let bins = sab.emission(e, 16, 8);
//! ```
//!
//! Provenance: the underlying kernel is a port of NJOY2016 (release 2016.79,
//! commit `ac5adf5f`) `thermr.f90`; see [`super`] and the crate `NOTICE`.

use std::fs::File;
use std::path::Path;

use uom::si::{
    area::barn, energy::electronvolt, thermodynamic_temperature::kelvin,
};

use crate::endf::tape::Tape;
use crate::thermr::inelastic::OutgoingBin;
use crate::thermr::mf7::{parse_mf7_at_temperature, IncoherentInelastic};
use crate::units::{CrossSection, NeutronEnergy, Temperature};
use crate::NjoyError;

/// One equally-probable outgoing-energy bin of a thermal inelastic scatter:
/// a `uom`-typed outgoing energy and its equally-probable scattering cosines.
///
/// A transport code samples a scatter by choosing one bin uniformly (each has
/// probability `1/nieb`), then one cosine uniformly from `cosines` (each
/// `1/nang`). This is the in-memory analogue of the ACE ITXE block (IFENG=0).
#[derive(Debug, Clone)]
pub struct ThermalEmissionBin {
    /// Representative outgoing neutron energy for this equiprobable bin.
    pub outgoing_energy: NeutronEnergy,
    /// Equally-probable scattering cosines μ ∈ \[−1, 1\], ascending.
    pub cosines: Vec<f64>,
}

/// Incoherent-inelastic thermal scattering for one bound scatterer at a fixed
/// temperature — the S(α,β) data surface a Monte-Carlo transport code queries.
///
/// Built from an ENDF MF=7/MT=4 thermal evaluation (`tsl-*`) with
/// [`from_endf_file`](Self::from_endf_file); the S(α,β) tables at the tabulated
/// temperature nearest the request are selected, and the short-collision-time
/// (SCT) tail is used beyond the tabulated `(α,β)` grid, so `σ(E)` runs from the
/// bound thermal value up to the free-atom limit at high `E`.
///
/// **Cross sections are per principal atom** (per H for H-in-H₂O). The material
/// composition (e.g. two H per H₂O molecule) is the caller's number-density
/// bookkeeping, not this struct's.
///
/// **Scope.** Only the incoherent-*inelastic* channel is exposed here — the one
/// H-in-H₂O needs (light water has no thermal elastic). Coherent /
/// incoherent-elastic bound scattering lives in [`super::coherent`] and
/// [`super::incoherent_elastic`].
pub struct IncoherentInelasticScattering {
    inelastic: IncoherentInelastic,
    requested_temperature: Temperature,
    natom: f64,
}

impl IncoherentInelasticScattering {
    /// Load the incoherent-inelastic S(α,β) for material `mat` from the ENDF
    /// `tsl-*` file at `path`, selecting the tabulated temperature nearest
    /// `temperature`.
    ///
    /// `mat` is the ENDF material number of the thermal evaluation (`1` for the
    /// ENDF/B-VIII.0 `tsl-HinH2O` file). The number of principal atoms is taken
    /// from `B(6)` of the evaluation (`2` for H-in-H₂O).
    ///
    /// Read the *actual* tabulated temperature used back with
    /// [`selected_temperature`](Self::selected_temperature) — the nearest grid
    /// point may differ from the request by a few kelvin.
    ///
    /// # Errors
    /// - [`NjoyError::Io`] if the file cannot be opened.
    /// - [`NjoyError::SectionNotFound`] if `mat` has no MF=7 data.
    /// - [`NjoyError::NotPorted`] if the evaluation lacks incoherent-inelastic
    ///   (MT=4) data, or is a `B(1)=0` pure-analytic principal scatterer.
    /// - [`NjoyError::EndfParse`] on malformed records.
    pub fn from_endf_file<P: AsRef<Path>>(
        path: P,
        mat: i32,
        temperature: Temperature,
    ) -> Result<Self, NjoyError> {
        let tape = Tape::read(File::open(path).map_err(NjoyError::Io)?)?;
        Self::from_tape(&tape, mat, temperature)
    }

    /// Build from an already-parsed [`Tape`] (avoids re-reading the file), same
    /// selection and errors as [`from_endf_file`](Self::from_endf_file).
    pub fn from_tape(tape: &Tape, mat: i32, temperature: Temperature) -> Result<Self, NjoyError> {
        let target_k = temperature.get::<kelvin>();
        let mf7 = parse_mf7_at_temperature(tape, mat, Some(target_k))?;
        let inelastic = mf7.incoherent_inelastic.ok_or(NjoyError::NotPorted(
            "thermal scattering requested but evaluation has no incoherent-inelastic (MT=4) data",
        ))?;
        // B(6) records the number of principal scattering atoms (2 for H₂O);
        // fall back to monatomic if the evaluation omits it.
        let natom = inelastic.b.get(5).copied().filter(|&n| n > 0.0).unwrap_or(1.0);
        Ok(Self { inelastic, requested_temperature: temperature, natom })
    }

    /// The temperature the caller requested.
    pub fn requested_temperature(&self) -> Temperature {
        self.requested_temperature
    }

    /// The tabulated temperature actually used for the S(α,β) tables — the grid
    /// point nearest the request. Compare against
    /// [`requested_temperature`](Self::requested_temperature) to confirm the
    /// evaluation tabulated a point close enough for the intended purpose.
    pub fn selected_temperature(&self) -> Temperature {
        Temperature::new::<kelvin>(self.inelastic.temperature_k)
    }

    /// Number of principal scattering atoms in the material (`B(6)`; `2` for
    /// H-in-H₂O). All cross sections here are *per one* such atom.
    pub fn principal_atom_count(&self) -> f64 {
        self.natom
    }

    /// Mass ratio `A` of the principal scatterer (`B(3)`; ≈ 0.999 for H).
    pub fn mass_ratio(&self) -> f64 {
        self.inelastic.mass_ratio()
    }

    /// Bound scattering cross section `σ_b` per principal atom — the `E → 0`
    /// static limit, `((A+1)/A)²·σ_free` (≈ 81.8 b for H in H₂O).
    pub fn bound_cross_section(&self) -> CrossSection {
        CrossSection::new::<barn>(self.inelastic.sigma_bound(self.natom))
    }

    /// Free-atom scattering cross section per principal atom, `B(1)/natom` — the
    /// high-`E` limit `σ_inel(E)` approaches (≈ 20.4 b for H in H₂O).
    pub fn free_cross_section(&self) -> CrossSection {
        CrossSection::new::<barn>(self.inelastic.b[0] / self.natom)
    }

    /// Principal-scatterer effective temperature `T_eff` used by the SCT tail at
    /// the selected temperature (≈ 1194 K for H in H₂O at 293.6 K).
    pub fn effective_temperature(&self) -> Temperature {
        let teff_ev = self.inelastic.teff_ev(self.inelastic.temperature_k);
        Temperature::new::<kelvin>(teff_ev / crate::common::phys::BK_EV_PER_K)
    }

    /// Incoherent-inelastic cross section `σ_inel(E)` **per principal atom** at
    /// incident neutron energy `e`, at the selected temperature.
    ///
    /// Integrates the S(α,β) double-differential kernel over the kinematically
    /// allowed outgoing energies and angles (with the SCT tail beyond the
    /// tabulated grid). Returns zero below/above the meaningful range.
    pub fn inelastic_xs(&self, e: NeutronEnergy) -> CrossSection {
        let sigma = self.inelastic.cross_section(
            e.get::<electronvolt>(),
            self.inelastic.temperature_k,
            self.natom,
        );
        CrossSection::new::<barn>(sigma)
    }

    /// Secondary energy/angle distribution for a scatter from incident energy
    /// `e`: `n_outgoing` equally-probable outgoing energies, each with
    /// `n_cosines` equally-probable scattering cosines (the ACE IFENG=0 form).
    ///
    /// Returns an empty vector when the cross section is zero (no scatter).
    /// NJOY-typical dimensions are `n_outgoing = 16`, `n_cosines = 8`.
    pub fn emission(
        &self,
        e: NeutronEnergy,
        n_outgoing: usize,
        n_cosines: usize,
    ) -> Vec<ThermalEmissionBin> {
        self.inelastic
            .equiprobable_emission(
                e.get::<electronvolt>(),
                self.inelastic.temperature_k,
                self.natom,
                n_outgoing,
                n_cosines,
            )
            .into_iter()
            .map(|OutgoingBin { e_out_ev, cosines }| ThermalEmissionBin {
                outgoing_energy: NeutronEnergy::new::<electronvolt>(e_out_ev),
                cosines,
            })
            .collect()
    }

    /// The raw kernel, for callers that need the double-differential directly or
    /// the tabulated `S(α,β)` grid (e.g. custom quadrature or ACE writing).
    pub fn kernel(&self) -> &IncoherentInelastic {
        &self.inelastic
    }
}
