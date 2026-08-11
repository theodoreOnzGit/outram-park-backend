//! Consumer surface: bound thermal scattering (all three channels) for Monte
//! Carlo.
//!
//! This is the **data surface `outram-mc-libs` consumes** for the thermal
//! S(α,β) treatment of a bound scatterer. Where [`mf7`](super::mf7) is the
//! *reader* and [`inelastic`](super::inelastic) / [`coherent`](super::coherent)
//! / [`incoherent_elastic`](super::incoherent_elastic) the *physics kernels*,
//! this module is the *hand-off*: one struct per thermal channel, each built
//! once per material + temperature, `uom`-typed, answering the questions a
//! transport code asks at a thermal collision:
//!
//! - [`IncoherentInelasticScattering`] — MF=7/MT=4. `σ_inel(E)` per principal
//!   atom ([`inelastic_xs`]) and the equally-probable secondary energy/angle
//!   bins ([`emission`]). The channel H-in-H₂O needs (water has no thermal
//!   elastic), and the *minority* channel for graphite.
//! - [`CoherentElasticScattering`] — MF=7/MT=2, `LTHR=1/3` (graphite, Be, Al).
//!   Bragg-edge `σ_coh_el(E)` per principal atom and the discrete reflection
//!   cosines. **~90 % of graphite's thermal scattering at 0.0253 eV**, so a
//!   graphite moderator without this channel is wrong by an order of
//!   magnitude. Scattering is elastic: `E_out = E_in`, only μ is sampled.
//! - [`IncoherentElasticScattering`] — MF=7/MT=2, `LTHR=2/3` (H in ZrH,
//!   polyethylene). `σ(E, T)` from the Debye-Waller integral and its smooth
//!   equally-probable cosines. Also elastic (`E_out = E_in`).
//!
//! All three resolve the requested temperature per the policy in the
//! [`mf7` module docs](super::mf7): a tabulated match within the NJOY
//! `T/1000 + 5` K tolerance is used directly, a temperature between tabulated
//! points is interpolated with the evaluation's `LI` law, and a temperature
//! outside the tabulated range is a hard
//! [`TemperatureOutOfRange`](crate::NjoyError::TemperatureOutOfRange) error —
//! never a silent snap.
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
/// [`from_endf_file`](Self::from_endf_file); the S(α,β) tables are resolved at
/// the requested temperature (tabulated match, `LI`-law interpolation, or a
/// [`TemperatureOutOfRange`](crate::NjoyError::TemperatureOutOfRange) refusal
/// — see the [`mf7` module docs](super::mf7)), and the short-collision-time
/// (SCT) tail is used beyond the tabulated `(α,β)` grid, so `σ(E)` runs from the
/// bound thermal value up to the free-atom limit at high `E`.
///
/// **Cross sections are per principal atom** (per H for H-in-H₂O). The material
/// composition (e.g. two H per H₂O molecule) is the caller's number-density
/// bookkeeping, not this struct's.
///
/// **Scope.** This struct is the incoherent-*inelastic* channel — the one
/// H-in-H₂O needs (light water has no thermal elastic). The elastic channels
/// have their own consumer structs in this module:
/// [`CoherentElasticScattering`] (graphite's dominant channel) and
/// [`IncoherentElasticScattering`] (ZrH, polyethylene).
pub struct IncoherentInelasticScattering {
    inelastic: IncoherentInelastic,
    requested_temperature: Temperature,
    natom: f64,
}

impl IncoherentInelasticScattering {
    /// Load the incoherent-inelastic S(α,β) for material `mat` from the ENDF
    /// `tsl-*` file at `path`, resolved at `temperature` (tabulated match
    /// within the NJOY tolerance, or `LI`-law interpolation between tabulated
    /// points — see the [`mf7` module docs](super::mf7)).
    ///
    /// `mat` is the ENDF material number of the thermal evaluation (`1` for the
    /// ENDF/B-VIII.0 `tsl-HinH2O` file). The number of principal atoms is taken
    /// from `B(6)` of the evaluation (`2` for H-in-H₂O).
    ///
    /// Read the temperature the tables actually represent back with
    /// [`selected_temperature`](Self::selected_temperature) — a tolerance
    /// match may differ from the request by up to `T/1000 + 5` K.
    ///
    /// # Errors
    /// - [`NjoyError::Io`] if the file cannot be opened.
    /// - [`NjoyError::SectionNotFound`] if `mat` has no MF=7 data.
    /// - [`NjoyError::NotPorted`] if the evaluation lacks incoherent-inelastic
    ///   (MT=4) data, or is a `B(1)=0` pure-analytic principal scatterer.
    /// - [`NjoyError::TemperatureOutOfRange`] if `temperature` is outside the
    ///   evaluation's tabulated range beyond the tolerance.
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

    /// The temperature the S(α,β) tables actually represent: the tabulated
    /// grid point when the request matched one within the NJOY `T/1000 + 5` K
    /// tolerance, or the request itself when the tables were interpolated
    /// between two tabulated temperatures. Compare against
    /// [`requested_temperature`](Self::requested_temperature) to see whether a
    /// tolerance snap occurred.
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

/// Coherent-elastic (Bragg) thermal scattering for one crystalline bound
/// scatterer at a fixed temperature — the data surface a Monte-Carlo transport
/// code queries for graphite's **dominant** thermal channel (~90 % of graphite
/// thermal scattering at 0.0253 eV; also Be, BeO, Al, Fe).
///
/// Built from an ENDF MF=7/MT=2 (`LTHR=1` or `3`) evaluation with
/// [`from_endf_file`](Self::from_endf_file). The cumulative structure factor
/// `S(E, T)` is resolved **once, at construction**, at the requested
/// temperature (tabulated match within the NJOY `T/1000 + 5` K tolerance,
/// `LI`-law interpolation between tabulated points, or a
/// [`TemperatureOutOfRange`](crate::NjoyError::TemperatureOutOfRange) refusal —
/// see the [`mf7` module docs](super::mf7)), so the per-collision queries are
/// plain table lookups.
///
/// **Cross sections are per principal atom** (`B(6)` of the evaluation's MT=4
/// section; `1` for graphite), matching [`IncoherentInelasticScattering`] so
/// the two channels can be summed with one number density.
///
/// **Physics.** `σ_coh_el(E) = S(E, T)/(E·natom)`: zero below the Bragg
/// cutoff (the first edge with nonzero `S` — 1.8223 meV for graphite, the
/// (002) plane), then a `1/E` sawtooth stepping up at each edge, suppressed
/// with rising `T` by the Debye-Waller factor. A coherent-elastic scatter keeps the neutron energy (`E_out = E_in`)
/// and picks the discrete cosine `μ_i = 1 − 2·E_i/E` of one active Bragg edge,
/// with probability proportional to that edge's structure-factor increment.
pub struct CoherentElasticScattering {
    /// Bragg-edge energies \[eV\], ascending (temperature-independent).
    bragg_energies_ev: Vec<f64>,
    /// Cumulative structure factor \[eV·b\] resolved at the construction
    /// temperature, on `bragg_energies_ev`.
    s_at_t: Vec<f64>,
    /// Number of principal scattering atoms (`B(6)`; cross sections are per one).
    natom: f64,
    requested_temperature: Temperature,
    resolved_temperature_k: f64,
    tabulated_temperatures_k: Vec<f64>,
}

impl CoherentElasticScattering {
    /// Load the coherent-elastic channel for material `mat` from the ENDF
    /// `tsl-*` file at `path`, resolved at `temperature`.
    ///
    /// `mat` is the ENDF material number of the thermal evaluation (`30` for
    /// ENDF/B-VIII.0 `tsl-crystalline-graphite`, `31`/`32` for the 10 %/30 %
    /// porosity reactor graphites). Valid temperatures span the evaluation's
    /// tabulated range (296–2000 K for the VIII.0 graphites) — see the
    /// temperature policy in the [`mf7` module docs](super::mf7).
    ///
    /// # Errors
    /// - [`NjoyError::Io`] if the file cannot be opened.
    /// - [`NjoyError::SectionNotFound`] if `mat` has no MF=7 data.
    /// - [`NjoyError::NotPorted`] if the evaluation has no coherent-elastic
    ///   data (e.g. H-in-H₂O, which has no thermal elastic at all, or ZrH,
    ///   which is incoherent-elastic — use
    ///   [`IncoherentElasticScattering`] there).
    /// - [`NjoyError::TemperatureOutOfRange`] if `temperature` is outside the
    ///   tabulated range beyond the NJOY `T/1000 + 5` K tolerance.
    /// - [`NjoyError::EndfParse`] on malformed records.
    pub fn from_endf_file<P: AsRef<Path>>(
        path: P,
        mat: i32,
        temperature: Temperature,
    ) -> Result<Self, NjoyError> {
        let tape = Tape::read(File::open(path).map_err(NjoyError::Io)?)?;
        Self::from_tape(&tape, mat, temperature)
    }

    /// Build from an already-parsed [`Tape`] (avoids re-reading the file); same
    /// selection and errors as [`from_endf_file`](Self::from_endf_file).
    pub fn from_tape(tape: &Tape, mat: i32, temperature: Temperature) -> Result<Self, NjoyError> {
        let mf7 = parse_mf7_at_temperature(tape, mat, None)?;
        let ce = mf7.coherent_elastic.ok_or(NjoyError::NotPorted(
            "coherent-elastic scattering requested but evaluation has no MT=2 LTHR=1/3 data",
        ))?;
        // Per-atom normalisation from MT=4 B(6), as for the inelastic surface
        // (1 for graphite; absent/zero degrades to monatomic).
        let natom = mf7
            .incoherent_inelastic
            .as_ref()
            .and_then(|ii| ii.b.get(5).copied())
            .filter(|&n| n > 0.0)
            .unwrap_or(1.0);
        let target_k = temperature.get::<kelvin>();
        let resolved_temperature_k = ce.resolved_temperature_k(target_k)?;
        let s_at_t: Vec<f64> =
            ce.s_of_e_at(target_k)?.into_iter().map(|(_, s)| s).collect();
        Ok(Self {
            bragg_energies_ev: ce.bragg_energies_ev,
            s_at_t,
            natom,
            requested_temperature: temperature,
            resolved_temperature_k,
            tabulated_temperatures_k: ce.temperatures_k,
        })
    }

    /// The temperature the caller requested.
    pub fn requested_temperature(&self) -> Temperature {
        self.requested_temperature
    }

    /// The temperature the resolved `S(E)` table actually represents: a
    /// tabulated grid point when the request matched one within the NJOY
    /// tolerance, otherwise the request itself (interpolated).
    pub fn resolved_temperature(&self) -> Temperature {
        Temperature::new::<kelvin>(self.resolved_temperature_k)
    }

    /// Every temperature the evaluation tabulates (296…2000 K, 10 points, for
    /// the ENDF/B-VIII.0 graphites). Queries are valid across this whole range.
    pub fn tabulated_temperatures(&self) -> Vec<Temperature> {
        self.tabulated_temperatures_k
            .iter()
            .map(|&t| Temperature::new::<kelvin>(t))
            .collect()
    }

    /// Number of principal scattering atoms (`B(6)`; `1` for graphite). All
    /// cross sections here are *per one* such atom.
    pub fn principal_atom_count(&self) -> f64 {
        self.natom
    }

    /// The Bragg cutoff — the first Bragg edge with a **nonzero** structure
    /// factor, below which the coherent-elastic cross section is exactly zero.
    /// For ENDF/B-VIII.0 graphite this is 1.8223 meV (the (002) plane); the
    /// evaluation's leading 0.4556 meV grid point carries `S = 0` and opens
    /// nothing.
    pub fn bragg_cutoff(&self) -> NeutronEnergy {
        let i = self.s_at_t.iter().position(|&s| s > 0.0).unwrap_or(0);
        NeutronEnergy::new::<electronvolt>(
            self.bragg_energies_ev.get(i).copied().unwrap_or(0.0),
        )
    }

    /// Coherent-elastic cross section `σ_coh_el(E)` **per principal atom** at
    /// incident neutron energy `e`, at the construction temperature.
    ///
    /// Zero below the Bragg cutoff; `S(E, T)/(E·natom)` above it (a `1/E`
    /// sawtooth stepping up at each Bragg edge).
    pub fn cross_section(&self, e: NeutronEnergy) -> CrossSection {
        let e_ev = e.get::<electronvolt>();
        if e_ev <= 0.0 {
            return CrossSection::new::<barn>(0.0);
        }
        let n_below = self.bragg_energies_ev.partition_point(|&edge| edge <= e_ev);
        if n_below == 0 {
            return CrossSection::new::<barn>(0.0);
        }
        CrossSection::new::<barn>(self.s_at_t[n_below - 1] / (e_ev * self.natom))
    }

    /// The **open** Bragg reflections at incident energy `e`: `(μ_i, p_i)`
    /// where `μ_i = 1 − 2·E_i/E` is the scattering cosine of edge `E_i` and
    /// `p_i` its **normalised probability** (the probabilities sum to 1).
    /// Zero-weight grid points (edges whose structure-factor increment is 0,
    /// e.g. graphite's leading 0.4556 meV point) are omitted. Empty below the
    /// Bragg cutoff. A coherent-elastic scatter keeps the neutron energy and
    /// takes one of these cosines.
    pub fn bragg_reflections(&self, e: NeutronEnergy) -> Vec<(f64, f64)> {
        let e_ev = e.get::<electronvolt>();
        let n = self.bragg_energies_ev.partition_point(|&edge| edge <= e_ev);
        if n == 0 || e_ev <= 0.0 {
            return Vec::new();
        }
        let total = self.s_at_t[n - 1];
        if total <= 0.0 {
            return Vec::new();
        }
        let mut prev_s = 0.0;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let w = self.s_at_t[i] - prev_s;
            if w > 0.0 {
                let mu = 1.0 - 2.0 * self.bragg_energies_ev[i] / e_ev;
                out.push((mu, w / total));
            }
            prev_s = self.s_at_t[i];
        }
        out
    }

    /// Sample a coherent-elastic scatter at incident energy `e` from a uniform
    /// random number `xi` ∈ \[0, 1): returns `(E_out, μ)` with `E_out = E_in`
    /// (elastic) and `μ` the cosine of the Bragg edge whose cumulative
    /// structure-factor share brackets `xi`. Returns `None` below the Bragg
    /// cutoff (no coherent-elastic scatter is possible there).
    ///
    /// The RNG stays with the caller — this crate is a data library and holds
    /// no random state.
    pub fn sample(&self, e: NeutronEnergy, xi: f64) -> Option<(NeutronEnergy, f64)> {
        let e_ev = e.get::<electronvolt>();
        let n = self.bragg_energies_ev.partition_point(|&edge| edge <= e_ev);
        if n == 0 || e_ev <= 0.0 {
            return None;
        }
        let total = self.s_at_t[n - 1];
        if total <= 0.0 {
            return None;
        }
        let target = xi.clamp(0.0, 1.0) * total;
        // First edge whose cumulative S reaches the sampled level.
        let i = self.s_at_t[..n].partition_point(|&s| s < target).min(n - 1);
        let mu = 1.0 - 2.0 * self.bragg_energies_ev[i] / e_ev;
        Some((e, mu.clamp(-1.0, 1.0)))
    }
}

/// Incoherent-elastic thermal scattering for one hydrogenous solid at a fixed
/// temperature — the data surface a Monte-Carlo transport code queries for
/// H-in-ZrH, polyethylene, and similar bound scatterers (`LTHR=2` or `3`).
/// Not needed for graphite (spin-0 C-12 has no incoherent elastic) or water
/// (no thermal elastic at all).
///
/// The cross section is `σ(E, T) = (σ_b/2N)·(1 − e^{−4EW'(T)})/(2EW'(T))` with
/// `W'(T)` the Debye-Waller integral interpolated from the evaluation's
/// `(T, W')` table; the scatter is elastic (`E_out = E_in`) with the smooth
/// forward-peaked angular law `p(μ) ∝ e^{−2EW'(1−μ)}` (no Bragg edges).
///
/// The construction temperature must lie inside the `(T, W')` table's range
/// (within the NJOY `T/1000 + 5` K tolerance at the ends) — outside it the
/// constructor refuses rather than extrapolating. **Cross sections are per
/// principal atom** (`B(6)`), matching the other two surfaces.
pub struct IncoherentElasticScattering {
    incoherent: crate::thermr::mf7::IncoherentElastic,
    natom: f64,
    requested_temperature: Temperature,
}

impl IncoherentElasticScattering {
    /// Load the incoherent-elastic channel for material `mat` from the ENDF
    /// `tsl-*` file at `path`, for evaluation at `temperature`.
    ///
    /// # Errors
    /// - [`NjoyError::Io`] if the file cannot be opened.
    /// - [`NjoyError::SectionNotFound`] if `mat` has no MF=7 data.
    /// - [`NjoyError::NotPorted`] if the evaluation has no incoherent-elastic
    ///   data (graphite and water do not — see
    ///   [`CoherentElasticScattering`] / [`IncoherentInelasticScattering`]).
    /// - [`NjoyError::TemperatureOutOfRange`] if `temperature` is outside the
    ///   `W'(T)` table's range beyond the NJOY `T/1000 + 5` K tolerance.
    /// - [`NjoyError::EndfParse`] on malformed records.
    pub fn from_endf_file<P: AsRef<Path>>(
        path: P,
        mat: i32,
        temperature: Temperature,
    ) -> Result<Self, NjoyError> {
        let tape = Tape::read(File::open(path).map_err(NjoyError::Io)?)?;
        Self::from_tape(&tape, mat, temperature)
    }

    /// Build from an already-parsed [`Tape`]; same selection and errors as
    /// [`from_endf_file`](Self::from_endf_file).
    pub fn from_tape(tape: &Tape, mat: i32, temperature: Temperature) -> Result<Self, NjoyError> {
        let mf7 = parse_mf7_at_temperature(tape, mat, None)?;
        let ie = mf7.incoherent_elastic.ok_or(NjoyError::NotPorted(
            "incoherent-elastic scattering requested but evaluation has no MT=2 LTHR=2/3 data",
        ))?;
        let natom = mf7
            .incoherent_inelastic
            .as_ref()
            .and_then(|ii| ii.b.get(5).copied())
            .filter(|&n| n > 0.0)
            .unwrap_or(1.0);
        // Refuse temperatures beyond the W'(T) table (with the NJOY tolerance
        // at the ends) — debye_waller() would silently clamp there.
        let target_k = temperature.get::<kelvin>();
        let (min_k, max_k) = match (ie.wp_of_t.first(), ie.wp_of_t.last()) {
            (Some(&(lo, _)), Some(&(hi, _))) => (lo, hi),
            _ => {
                return Err(NjoyError::EndfParse(
                    "incoherent-elastic W'(T) table is empty".into(),
                ))
            }
        };
        let tol = crate::thermr::mf7::temperature_match_tolerance_k(target_k);
        if target_k < min_k - tol || target_k > max_k + tol {
            return Err(NjoyError::TemperatureOutOfRange {
                requested_k: target_k,
                min_k,
                max_k,
            });
        }
        Ok(Self { incoherent: ie, natom, requested_temperature: temperature })
    }

    /// The temperature the caller requested (used directly — `W'(T)` is a
    /// proper table over `T`, interpolated at evaluation time).
    pub fn temperature(&self) -> Temperature {
        self.requested_temperature
    }

    /// Number of principal scattering atoms (`B(6)`). All cross sections here
    /// are *per one* such atom.
    pub fn principal_atom_count(&self) -> f64 {
        self.natom
    }

    /// Incoherent-elastic cross section **per principal atom** at incident
    /// neutron energy `e`, at the construction temperature.
    pub fn cross_section(&self, e: NeutronEnergy) -> CrossSection {
        let sigma = self.incoherent.cross_section(
            e.get::<electronvolt>(),
            self.requested_temperature.get::<kelvin>(),
            self.natom,
        );
        CrossSection::new::<barn>(sigma)
    }

    /// `nbin` equally-probable scattering cosines for an (elastic) incoherent
    /// scatter at incident energy `e` — sample one uniformly and keep
    /// `E_out = E_in`. NJOY-typical `nbin` is 8 (the ACE ITCA block width).
    pub fn equiprobable_cosines(&self, e: NeutronEnergy, nbin: usize) -> Vec<f64> {
        self.incoherent.equiprobable_cosines(
            e.get::<electronvolt>(),
            self.requested_temperature.get::<kelvin>(),
            nbin,
        )
    }
}
