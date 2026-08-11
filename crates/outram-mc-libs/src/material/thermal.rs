//! S(α,β) thermal scattering tables — the bound-atom scattering treatment.
//!
//! C++ analogue: `src/thermal.cpp`, `include/openmc/thermal.h` (the ACE
//! `ThermalScattering` / `ThermalData` blocks).
//!
//! **Why this exists.** Below a few eV the free-gas treatment of scattering from
//! bound atoms is wrong: chemical binding raises the bound cross section
//! (≈ 81.8 b per H in H₂O at E→0 vs the 20.4 b free-atom limit) and lets the
//! neutron *up-scatter* off the thermal motion of the molecule, which is what
//! establishes the Maxwellian thermal spectrum in a moderator. A fast-metal
//! sphere (Godiva) never reaches these energies, but a *thermal* LWR pin-cell
//! lives or dies on it.
//!
//! **Data-free, like the rest of `outram-mc-libs`.** This struct holds no ENDF
//! parsing of its own: it is built from the njoy consumer surface
//! [`njoy_outram_park_fork::thermr::scattering`], which reads the ENDF/B `tsl-*`
//! thermal evaluation and answers the transport questions — σ(E) per channel and
//! the secondary energy/angle distributions. To keep the transport hot-loop cheap
//! (the inelastic kernel integrates the S(α,β) double-differential per call),
//! this type **pre-tabulates** the smooth quantities on a fixed energy grid at
//! construction, exactly as NJOY/ACE bakes the ITIE/ITXE blocks, and the run-time
//! path only interpolates and samples.
//!
//! **Scope — all three ENDF MF=7 thermal channels.**
//!
//! | Channel | ENDF | Scatterers | Held as |
//! |---|---|---|---|
//! | Incoherent inelastic | MT=4 | every `tsl` evaluation | [`ThermalScattering`] (always) |
//! | Coherent elastic (Bragg) | MT=2, LTHR=1/3 | graphite, Be, BeO, SiC, Al | [`ThermalElastic::Coherent`] |
//! | Incoherent elastic | MT=2, LTHR=2/3 | H in ZrH, polyethylene | [`ThermalElastic::Incoherent`] |
//!
//! A given evaluation has **at most one** elastic law, so the two are an enum
//! (workspace rule: enum dispatch, never trait objects). Light water has no
//! thermal elastic at all and takes [`ThermalElastic::None`], which reproduces
//! the pre-2026-08-11 inelastic-only behaviour exactly.
//!
//! **Why the elastic channel matters (HTR-10).** For crystalline graphite —
//! the HTR-10 moderator — coherent elastic *dominates*: measured through this
//! path against ENDF/B-VIII.0 `tsl-crystalline-graphite` (MAT 30) at 0.0253 eV
//! and 296 K, σ_coh_el = 4.5514 b against σ_inel = 0.4864 b, so the elastic
//! channel is 90.3 % of bound thermal scattering. Wiring only the inelastic
//! channel would give a graphite pebble ~10 % of its true thermal cross section
//! — worse than the free-gas treatment it replaces. See bead `op-nhoa`.

use njoy_outram_park_fork::NjoyError;

use crate::rng::lcg::prn;

/// Default upper energy \[eV\] of the S(α,β) treatment — the "thermal cutoff".
///
/// Above it the neutron sees the ordinary free-gas / WMP elastic channel; below
/// it the bound S(α,β) treatment replaces elastic scattering off the principal
/// atom. 4 eV is the OpenMC/NJOY convention for light-water thermal tables
/// (`ENERGY_MAX_THERMAL`-class cutoff): by ~4 eV the S(α,β) cross section has
/// relaxed to the free-atom limit and the up-scatter probability is negligible,
/// so the join to free-gas is smooth.
pub const DEFAULT_THERMAL_CUTOFF_EV: f64 = 4.0;

/// Number of incident-energy points on the pre-tabulated σ_inel(E) grid.
const N_XS_GRID: usize = 200;
/// Number of incident-energy points on the pre-tabulated emission grid.
const N_EMIT_GRID: usize = 48;
/// Equally-probable outgoing energies per emission table (NJOY-typical).
const N_OUTGOING: usize = 16;
/// Equally-probable cosines per outgoing-energy bin (NJOY-typical).
const N_COSINES: usize = 8;
/// Bottom of the pre-tabulation grid \[eV\] — the thermal tail of a Maxwellian.
const E_MIN_GRID_EV: f64 = 1.0e-5;
/// Number of incident-energy points on the pre-tabulated *incoherent-elastic*
/// σ(E) / cosine grid. σ_inc_el(E) is smooth (no Bragg edges), so a log grid
/// resolves it; the coherent channel is stored exactly instead.
///
/// Sized by measurement, not by taste: over 1e-5 → 4 eV (5.6 decades) the
/// linear-interpolation error against the njoy surface for H-in-ZrH at 296 K is
/// 1.05e-3 at 200 points and 2.63e-4 at 400 — the expected 4× improvement for a
/// halved step. 400 points × 16 cosines is ~54 kB per scatterer, which is
/// negligible next to the emission tables, so take the accuracy.
const N_INC_ELASTIC_GRID: usize = 400;
/// Equally-probable cosines per incident energy for incoherent elastic
/// (NJOY/ACE convention for the ITCA block).
const N_INC_ELASTIC_COSINES: usize = 16;

/// The bound-atom thermal **elastic** channel of one scatterer, if it has one.
///
/// An ENDF `tsl` evaluation carries at most one MF=7/MT=2 elastic law, so this
/// is an enum rather than a set (and enum dispatch is the workspace rule — no
/// trait objects). Both elastic laws share the defining property that the
/// neutron **keeps its energy** (`E_out = E_in`, ENDF-102 Eq. 7-1) and only its
/// direction changes; they differ in how μ is distributed.
///
/// C++ analogue: the `ThermalData::elastic_` slot in OpenMC's `src/thermal.cpp`,
/// whose distribution is a `CoherentElasticAE` or an `IncoherentElasticAE`.
pub enum ThermalElastic {
    /// No thermal elastic scattering — σ_el(E) ≡ 0 at every energy.
    ///
    /// Correct for light water (`tsl-HinH2O`): a liquid has no lattice to
    /// diffract from and H's bound incoherent-elastic law is not tabulated.
    None,
    /// **Coherent elastic** (Bragg diffraction) — crystalline solids: graphite,
    /// beryllium, BeO, SiC, aluminium. The HTR-10 moderator case.
    Coherent(CoherentElasticTable),
    /// **Incoherent elastic** — hydrogenous solids where the bound proton
    /// scatters incoherently off a rigid lattice: H in ZrH, polyethylene.
    Incoherent(IncoherentElasticTable),
}

/// Coherent-elastic (Bragg) scattering for one crystalline scatterer at one
/// temperature, stored **exactly** as the Bragg-edge step table.
///
/// σ_coh_el(E) is a `1/E` sawtooth that steps *discontinuously* upward at each
/// Bragg edge, so — unlike the smooth channels — resampling it onto a log grid
/// would smear every edge. Instead this holds the edge energies and the
/// cumulative structure factor from
/// [`CoherentElasticScattering::bragg_edge_table`](njoy_outram_park_fork::thermr::scattering::CoherentElasticScattering::bragg_edge_table),
/// which reproduces both σ(E) and the sampling law with no interpolation error
/// at all. ENDF/B-VIII.0 crystalline graphite tabulates 221 edges, so the table
/// is ~3.5 kB — *cheaper* than the log resample it replaces, as well as exact
/// (measured worst relative deviation from the njoy surface: 7.67e-16 over
/// 21 199 points spanning 1e-4 → 4 eV; see `tests/thermal_graphite_elastic.rs`).
///
/// Units: edge energies \[eV\], cross sections \[barn\] **per principal atom**
/// (per carbon for graphite — the caller multiplies by the principal-atom
/// number density in atoms/barn·cm).
pub struct CoherentElasticTable {
    /// Bragg-edge energies E_i \[eV\], ascending.
    edges_ev: Vec<f64>,
    /// Cumulative structure factor per principal atom, `S(E_i,T)/natom`
    /// \[eV·barn\]. σ(E) = `s_cum[i] / E` for E ∈ \[E_i, E_{i+1}), 0 below E_0.
    s_cum: Vec<f64>,
}

impl CoherentElasticTable {
    /// Coherent-elastic cross section \[barn per principal atom\] at incident
    /// energy `e` \[eV\]. Exactly zero below the Bragg cutoff (the first edge
    /// with nonzero structure factor); a `1/E` decay between edges above it.
    pub fn cross_section(&self, e: f64) -> f64 {
        if e <= 0.0 {
            return 0.0;
        }
        let n = self.edges_ev.partition_point(|&edge| edge <= e);
        if n == 0 {
            return 0.0;
        }
        self.s_cum[n - 1] / e
    }

    /// The Bragg cutoff \[eV\] — the lowest energy at which coherent-elastic
    /// scattering is possible (1.8223 meV for ENDF/B-VIII.0 graphite, the (002)
    /// plane). Returns `f64::INFINITY` for an empty table.
    pub fn bragg_cutoff_ev(&self) -> f64 {
        match self.s_cum.iter().position(|&s| s > 0.0) {
            Some(i) => self.edges_ev[i],
            None => f64::INFINITY,
        }
    }

    /// Sample a coherent-elastic scatter at incident energy `e` \[eV\]:
    /// `Some((e_out, mu))` with `e_out == e` (elastic) and `mu` the discrete
    /// cosine `1 − 2·E_k/E` of the Bragg edge whose cumulative structure-factor
    /// share brackets the random draw. `None` below the Bragg cutoff.
    ///
    /// Mirrors OpenMC `CoherentElasticAE::sample`
    /// (`src/secondary_thermal.cpp:34-54`): the edge is chosen by inverting the
    /// cumulative `factors` array, which is exactly `s_cum` here.
    pub fn sample(&self, e: f64, seed: &mut u64) -> Option<(f64, f64)> {
        if e <= 0.0 {
            return None;
        }
        let n = self.edges_ev.partition_point(|&edge| edge <= e);
        if n == 0 {
            return None;
        }
        let total = self.s_cum[n - 1];
        if total <= 0.0 {
            return None;
        }
        let target = prn(seed) * total;
        let k = self.s_cum[..n].partition_point(|&s| s < target).min(n - 1);
        let mu = (1.0 - 2.0 * self.edges_ev[k] / e).clamp(-1.0, 1.0);
        Some((e, mu))
    }
}

/// Incoherent-elastic scattering for one hydrogenous solid at one temperature,
/// pre-tabulated on a log energy grid.
///
/// σ_inc_el(E) = (σ_b/2N)·(1 − e^{−4EW'})/(2EW') is smooth in E — no edges — so
/// unlike the coherent channel a 200-point log grid resolves it. Each grid point
/// also carries the equally-probable cosines of the forward-peaked angular law
/// p(μ) ∝ e^{−2EW'(1−μ)}, the ACE ITCA block's in-memory analogue.
///
/// Units: energies \[eV\], cross sections \[barn per principal atom\], cosines
/// dimensionless on \[−1, 1\].
pub struct IncoherentElasticTable {
    /// Ascending incident-energy grid \[eV\].
    e_grid: Vec<f64>,
    /// σ_inc_el per principal atom \[barn\] at each grid point.
    sigma: Vec<f64>,
    /// Equally-probable cosines, row-major (`grid_point * n_mu + j`).
    cosines: Vec<f64>,
    /// Number of equally-probable cosines per grid point.
    n_mu: usize,
}

impl IncoherentElasticTable {
    /// Incoherent-elastic cross section \[barn per principal atom\] at incident
    /// energy `e` \[eV\], linearly interpolated on the pre-tabulated grid and
    /// clamped to its endpoints.
    pub fn cross_section(&self, e: f64) -> f64 {
        if self.e_grid.is_empty() {
            return 0.0;
        }
        interp_linear(&self.e_grid, &self.sigma, e)
    }

    /// Sample an incoherent-elastic scatter at incident energy `e` \[eV\]:
    /// `Some((e_out, mu))` with `e_out == e` (elastic) and `mu` drawn uniformly
    /// from the nearest grid point's equally-probable cosine bins.
    ///
    /// Mirrors OpenMC `IncoherentElasticAEDiscrete::sample`
    /// (`src/secondary_thermal.cpp`): pick the incident-energy bin, then one
    /// equiprobable cosine. `None` if the table carries no cosines.
    pub fn sample(&self, e: f64, seed: &mut u64) -> Option<(f64, f64)> {
        if self.n_mu == 0 || self.e_grid.is_empty() {
            return None;
        }
        let i = nearest_index(&self.e_grid, e);
        let j = ((prn(seed) * self.n_mu as f64) as usize).min(self.n_mu - 1);
        let mu = self.cosines[i * self.n_mu + j].clamp(-1.0, 1.0);
        Some((e, mu))
    }
}

impl ThermalElastic {
    /// Thermal-elastic cross section \[barn per principal atom\] at incident
    /// energy `e` \[eV\]; `0.0` for [`ThermalElastic::None`].
    pub fn cross_section(&self, e: f64) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Coherent(t) => t.cross_section(e),
            Self::Incoherent(t) => t.cross_section(e),
        }
    }

    /// Sample an elastic scatter — `Some((e_out = e, mu))` — or `None` if this
    /// scatterer has no elastic channel or none is open at `e`.
    pub fn sample(&self, e: f64, seed: &mut u64) -> Option<(f64, f64)> {
        match self {
            Self::None => None,
            Self::Coherent(t) => t.sample(e, seed),
            Self::Incoherent(t) => t.sample(e, seed),
        }
    }

    /// A short human-readable channel name for provenance/logging:
    /// `"none"`, `"coherent elastic"`, or `"incoherent elastic"`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Coherent(_) => "coherent elastic",
            Self::Incoherent(_) => "incoherent elastic",
        }
    }
}

/// One pre-tabulated equiprobable emission table for a single incident energy —
/// the in-memory analogue of an ACE ITXE sub-block (IFENG=0).
///
/// A scatter is sampled by choosing one of the `n_out` outgoing energies
/// uniformly (each probability `1/n_out`), then one of that bin's `n_mu` cosines
/// uniformly. Both outgoing energy and cosine are **laboratory-frame** (S(α,β)
/// secondary distributions are given in the lab frame, unlike the CM elastic
/// law), so the caller applies the cosine directly with `rotate_direction` — no
/// CM→lab transform.
#[derive(Debug, Clone)]
struct EmissionTable {
    /// Representative outgoing energies \[eV\], one per equiprobable bin.
    e_out: Vec<f64>,
    /// Equally-probable cosines μ ∈ \[−1, 1\] for each outgoing-energy bin,
    /// laid out row-major (`bin * n_mu + j`).
    cosines: Vec<f64>,
    /// Number of cosines per outgoing-energy bin.
    n_mu: usize,
}

/// Bound-atom thermal scattering for one scatterer at one temperature — the
/// transport-side data surface, carrying **both** the incoherent-inelastic
/// S(α,β) channel and the scatterer's elastic channel (if it has one).
///
/// Built once per material+temperature with [`from_endf_file`](Self::from_endf_file),
/// then queried in the transport loop by:
/// - [`inelastic_xs`](Self::inelastic_xs) — σ_inel(E) **per principal atom**
///   \[barn\] at incident energy `E` \[eV\] (multiply by the principal-atom number
///   density to get the macroscopic contribution);
/// - [`elastic_xs`](Self::elastic_xs) — σ_el(E) \[barn per principal atom\],
///   zero when the scatterer has no thermal elastic law;
/// - [`total_xs`](Self::total_xs) — their sum, which is what replaces the
///   free-gas elastic channel below the cutoff;
/// - [`sample`](Self::sample) — a laboratory-frame outgoing energy \[eV\] and
///   cosine for a thermal scatter, choosing elastic vs inelastic in proportion
///   to their cross sections.
///
/// Cross sections are **per principal atom** (per H for H-in-H₂O, per C for
/// graphite). The material composition (two H per H₂O) is the caller's
/// number-density bookkeeping.
pub struct ThermalScattering {
    /// Human-readable scatterer name, e.g. `"H in H2O"`, `"C in graphite"`.
    pub name: String,
    /// Upper energy of the S(α,β) treatment \[eV\] (the thermal cutoff).
    cutoff_ev: f64,
    /// The temperature \[K\] the inelastic tables actually represent — a
    /// tabulated grid point when the request matched one within the NJOY
    /// `T/1000 + 5` K tolerance, otherwise the (interpolated) request itself.
    /// Recorded for the V&V provenance.
    selected_temperature_k: f64,
    /// Ascending incident-energy grid \[eV\] for the σ_inel lookup.
    xs_e: Vec<f64>,
    /// σ_inel per principal atom \[barn\] at each `xs_e` point.
    xs_sigma: Vec<f64>,
    /// Ascending incident-energy grid \[eV\] for the emission tables.
    emit_e: Vec<f64>,
    /// One emission table per `emit_e` point (parallel arrays).
    emit_tables: Vec<EmissionTable>,
    /// The elastic channel — [`ThermalElastic::None`] for a scatterer with no
    /// thermal elastic law (light water).
    elastic: ThermalElastic,
}

impl ThermalScattering {
    /// Build the pre-tabulated bound-atom thermal treatment — inelastic **and**
    /// elastic — from an ENDF `tsl-*` thermal evaluation file.
    ///
    /// # Parameters
    /// - `path` — the ENDF `tsl-*` file (e.g. ENDF/B-VIII.0 `tsl-HinH2O.endf`
    ///   or `tsl-crystalline-graphite.endf`).
    /// - `mat` — the ENDF material number of the thermal evaluation (`1` for
    ///   `tsl-HinH2O`; `30`/`31`/`32` for crystalline / 10 %-porous /
    ///   30 %-porous reactor graphite).
    /// - `temperature_k` — the requested temperature \[K\]. Must lie inside the
    ///   evaluation's tabulated range (296–2000 K for the VIII.0 graphites,
    ///   283.6–1000 K for `tsl-HinH2O`); a request matching a tabulated point
    ///   within NJOY's `T/1000 + 5` K tolerance uses it directly, one strictly
    ///   between two points is **interpolated** with the evaluation's `LI` law,
    ///   and one outside the range is a hard
    ///   [`NjoyError::TemperatureOutOfRange`] — never a silent snap. Read back
    ///   what was used with
    ///   [`selected_temperature_k`](Self::selected_temperature_k).
    /// - `name` — a label carried for diagnostics (e.g. `"C in graphite"`).
    ///
    /// The elastic channel is **detected from the evaluation**, not requested:
    /// an MT=2 `LTHR=1/3` section gives [`ThermalElastic::Coherent`], `LTHR=2/3`
    /// gives [`ThermalElastic::Incoherent`], and an evaluation with no MT=2 at
    /// all gives [`ThermalElastic::None`] — so light water behaves exactly as it
    /// did before the elastic channel existed.
    ///
    /// The σ(E) and secondary-distribution grids are baked here (this is the
    /// expensive step: it integrates the S(α,β) kernel at `N_XS_GRID` +
    /// `N_EMIT_GRID` incident energies), so the transport loop stays cheap. The
    /// coherent-elastic sawtooth is stored *exactly* as its Bragg-edge table
    /// rather than resampled — see [`CoherentElasticTable`]. The ENDF tape is
    /// read once and shared across all three channel constructors.
    ///
    /// # Errors
    /// Propagates [`NjoyError`] if the file cannot be read, `mat` has no
    /// MF=7/MT=4 incoherent-inelastic data, `temperature_k` is outside the
    /// tabulated range of *either* channel, or a record is malformed.
    pub fn from_endf_file(
        path: &str,
        mat: i32,
        temperature_k: f64,
        name: &str,
    ) -> Result<Self, NjoyError> {
        use njoy_outram_park_fork::endf::tape::Tape;
        use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
        use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
        use uom::si::{area::barn, energy::electronvolt, thermodynamic_temperature::kelvin};

        let t = Temperature::new::<kelvin>(temperature_k);
        // One parse of the tape, shared by all three channel constructors.
        let tape = Tape::read(std::fs::File::open(path).map_err(NjoyError::Io)?)?;
        let sab = IncoherentInelasticScattering::from_tape(&tape, mat, t)?;
        let selected_temperature_k = sab.selected_temperature().get::<kelvin>();
        let cutoff_ev = DEFAULT_THERMAL_CUTOFF_EV;

        // σ_inel(E) grid — log-spaced from the thermal tail to the cutoff.
        let xs_e = log_grid(E_MIN_GRID_EV, cutoff_ev, N_XS_GRID);
        let xs_sigma: Vec<f64> = xs_e
            .iter()
            .map(|&e| sab.inelastic_xs(NeutronEnergy::new::<electronvolt>(e)).get::<barn>())
            .collect();

        // Emission grid — a coarser log-spaced incident-energy grid, each point
        // carrying an equiprobable (E', μ) table.
        let emit_e = log_grid(E_MIN_GRID_EV, cutoff_ev, N_EMIT_GRID);
        let emit_tables: Vec<EmissionTable> = emit_e
            .iter()
            .map(|&e| {
                let bins =
                    sab.emission(NeutronEnergy::new::<electronvolt>(e), N_OUTGOING, N_COSINES);
                build_emission_table(&bins)
            })
            .collect();

        let elastic = build_elastic_channel(&tape, mat, t)?;

        Ok(Self {
            name: name.to_string(),
            cutoff_ev,
            selected_temperature_k,
            xs_e,
            xs_sigma,
            emit_e,
            emit_tables,
            elastic,
        })
    }

    /// Upper energy \[eV\] of the S(α,β) treatment (the thermal cutoff). Above it
    /// the caller uses ordinary free-gas / WMP elastic scattering.
    pub fn cutoff_ev(&self) -> f64 {
        self.cutoff_ev
    }

    /// The temperature \[K\] the S(α,β) tables actually represent — a tabulated
    /// grid point when the request matched one within NJOY's `T/1000 + 5` K
    /// tolerance, otherwise the requested temperature itself (the tables were
    /// interpolated to it). Recorded so a V&V reader knows the exact data point.
    pub fn selected_temperature_k(&self) -> f64 {
        self.selected_temperature_k
    }

    /// The scatterer's thermal **elastic** channel, detected from the
    /// evaluation: coherent (Bragg) for a crystalline solid, incoherent for a
    /// hydrogenous solid, or [`ThermalElastic::None`] for a liquid such as
    /// light water.
    pub fn elastic(&self) -> &ThermalElastic {
        &self.elastic
    }

    /// Thermal-elastic cross section σ_el(E) **per principal atom** \[barn\] at
    /// incident energy `e` \[eV\]. Zero at or above the thermal cutoff, and
    /// zero at every energy for a scatterer with no elastic law.
    ///
    /// For graphite this is the **dominant** thermal channel below ~0.1 eV.
    pub fn elastic_xs(&self, e: f64) -> f64 {
        if e >= self.cutoff_ev {
            return 0.0;
        }
        self.elastic.cross_section(e)
    }

    /// Total bound-atom thermal cross section \[barn per principal atom\] at
    /// incident energy `e` \[eV\] — σ_inel(E) + σ_el(E).
    ///
    /// This is the quantity that **replaces** the free-gas elastic channel
    /// below the cutoff (see [`crate::material::nuclide::Nuclide::xs_at_energy`]);
    /// zero at or above the cutoff, where free-gas takes over again.
    pub fn total_xs(&self, e: f64) -> f64 {
        self.inelastic_xs(e) + self.elastic_xs(e)
    }

    /// Incoherent-inelastic cross section σ_inel(E) **per principal atom**
    /// \[barn\] at incident energy `e` \[eV\], by linear interpolation on the
    /// pre-tabulated grid. Zero at or above the cutoff (the caller reverts to
    /// free-gas there); clamped to the grid endpoints below it.
    pub fn inelastic_xs(&self, e: f64) -> f64 {
        if e >= self.cutoff_ev {
            return 0.0;
        }
        interp_linear(&self.xs_e, &self.xs_sigma, e)
    }

    /// Sample a thermal scatter at incident energy `e` \[eV\], returning
    /// `Some((e_out, mu_lab))` — a laboratory-frame outgoing energy \[eV\] and
    /// scattering cosine — or `None` at or above the cutoff.
    ///
    /// **Channel choice.** Elastic scattering is chosen with probability
    /// σ_el(E)/σ_thermal(E) and inelastic otherwise, mirroring OpenMC's
    /// `ThermalData::sample_dist` (`src/thermal.cpp:296-302`, which tests
    /// `prn(seed) < micro_xs.thermal_elastic / micro_xs.thermal`). An elastic
    /// scatter returns `e_out == e` and a Bragg (or incoherent-elastic) cosine;
    /// an inelastic scatter draws a new energy, which is where thermal
    /// **up-scatter** — and hence the Maxwellian spectrum — comes from.
    ///
    /// **Inelastic sampling** mirrors the ACE incoherent-inelastic law
    /// (`src/thermal.cpp` `ThermalData::sample`, IFENG=0): bracket the incident
    /// energy on the emission grid, pick the lower/upper table by statistical
    /// interpolation, then draw one equiprobable outgoing energy and one
    /// equiprobable cosine.
    ///
    /// Both channels return **laboratory-frame** quantities — S(α,β) secondary
    /// distributions are lab-frame, unlike the CM elastic law — so the caller
    /// applies `mu_lab` directly with `rotate_direction`, with no CM→lab
    /// transform.
    pub fn sample(&self, e: f64, seed: &mut u64) -> Option<(f64, f64)> {
        if e >= self.cutoff_ev {
            return None;
        }
        // Elastic / inelastic split, in proportion to the two cross sections.
        let sigma_el = self.elastic.cross_section(e);
        if sigma_el > 0.0 {
            let sigma_tot = self.inelastic_xs(e) + sigma_el;
            if sigma_tot > 0.0 && prn(seed) < sigma_el / sigma_tot {
                if let Some(out) = self.elastic.sample(e, seed) {
                    return Some(out);
                }
                // An elastic table that declines to sample (below its cutoff)
                // falls through to the inelastic channel rather than losing the
                // collision.
            }
        }
        if self.emit_tables.is_empty() {
            return None;
        }
        let table = self.select_table(e, seed);
        if table.e_out.is_empty() {
            return None;
        }
        // One equiprobable outgoing-energy bin, then one equiprobable cosine.
        let n_out = table.e_out.len();
        let i_out = ((prn(seed) * n_out as f64) as usize).min(n_out - 1);
        let e_out = table.e_out[i_out];
        let base = i_out * table.n_mu;
        let mu = if table.n_mu == 0 {
            2.0 * prn(seed) - 1.0
        } else {
            let j = ((prn(seed) * table.n_mu as f64) as usize).min(table.n_mu - 1);
            table.cosines[base + j].clamp(-1.0, 1.0)
        };
        Some((e_out, mu))
    }

    /// Pick the emission table to sample from for incident energy `e` \[eV\] by
    /// statistical interpolation between the two bracketing incident-energy grid
    /// points (ACE convention: choose the upper table with probability equal to
    /// the interpolation factor `r`).
    fn select_table(&self, e: f64, seed: &mut u64) -> &EmissionTable {
        let grid = &self.emit_e;
        let n = grid.len();
        if e <= grid[0] {
            return &self.emit_tables[0];
        }
        if e >= grid[n - 1] {
            return &self.emit_tables[n - 1];
        }
        let mut i = 0;
        while i + 1 < n && grid[i + 1] <= e {
            i += 1;
        }
        let (e0, e1) = (grid[i], grid[i + 1]);
        let r = if e1 > e0 { (e - e0) / (e1 - e0) } else { 0.0 };
        if r > prn(seed) {
            &self.emit_tables[i + 1]
        } else {
            &self.emit_tables[i]
        }
    }
}

/// Detect and build the scatterer's thermal elastic channel from an
/// already-parsed ENDF tape.
///
/// The two elastic laws are mutually exclusive in an ENDF `tsl` evaluation, so
/// this tries coherent first, then incoherent, and returns
/// [`ThermalElastic::None`] when neither is present.
///
/// A [`NjoyError::NotPorted`] from a channel constructor means *"this evaluation
/// has no such section"* (that is the error the njoy surface documents for an
/// absent channel) and is therefore not an error here. Every other error —
/// notably [`NjoyError::TemperatureOutOfRange`] — **is** propagated: silently
/// dropping graphite's dominant channel because the requested temperature is out
/// of range would be far worse than refusing to build.
fn build_elastic_channel(
    tape: &njoy_outram_park_fork::endf::tape::Tape,
    mat: i32,
    t: njoy_outram_park_fork::units::Temperature,
) -> Result<ThermalElastic, NjoyError> {
    use njoy_outram_park_fork::thermr::scattering::{
        CoherentElasticScattering, IncoherentElasticScattering,
    };
    use njoy_outram_park_fork::units::NeutronEnergy;
    use uom::si::{area::barn, energy::electronvolt};

    match CoherentElasticScattering::from_tape(tape, mat, t) {
        Ok(ce) => {
            // Store the sawtooth exactly: (E_i, σ_i) ⇒ cumulative structure
            // factor S_i/natom = σ_i·E_i [eV·barn], from which σ(E) = S_i/E.
            let table = ce.bragg_edge_table();
            let mut edges_ev = Vec::with_capacity(table.len());
            let mut s_cum = Vec::with_capacity(table.len());
            for (e, sigma) in table {
                let e_ev = e.get::<electronvolt>();
                edges_ev.push(e_ev);
                s_cum.push(sigma.get::<barn>() * e_ev);
            }
            return Ok(ThermalElastic::Coherent(CoherentElasticTable {
                edges_ev,
                s_cum,
            }));
        }
        Err(NjoyError::NotPorted(_)) => {}
        Err(e) => return Err(e),
    }

    match IncoherentElasticScattering::from_tape(tape, mat, t) {
        Ok(ie) => {
            let e_grid = log_grid(E_MIN_GRID_EV, DEFAULT_THERMAL_CUTOFF_EV, N_INC_ELASTIC_GRID);
            let mut sigma = Vec::with_capacity(e_grid.len());
            let mut cosines = Vec::with_capacity(e_grid.len() * N_INC_ELASTIC_COSINES);
            for &e_ev in &e_grid {
                let e = NeutronEnergy::new::<electronvolt>(e_ev);
                sigma.push(ie.cross_section(e).get::<barn>());
                let mus = ie.equiprobable_cosines(e, N_INC_ELASTIC_COSINES);
                for j in 0..N_INC_ELASTIC_COSINES {
                    cosines.push(mus.get(j).copied().unwrap_or(0.0));
                }
            }
            Ok(ThermalElastic::Incoherent(IncoherentElasticTable {
                e_grid,
                sigma,
                cosines,
                n_mu: N_INC_ELASTIC_COSINES,
            }))
        }
        Err(NjoyError::NotPorted(_)) => Ok(ThermalElastic::None),
        Err(e) => Err(e),
    }
}

/// Index of the grid point nearest to `x` on an ascending grid (ties go low).
/// Used to pick an incoherent-elastic cosine bin; the grid is non-empty.
fn nearest_index(xs: &[f64], x: f64) -> usize {
    let n = xs.len();
    let i = xs.partition_point(|&v| v < x);
    if i == 0 {
        return 0;
    }
    if i >= n {
        return n - 1;
    }
    if x - xs[i - 1] <= xs[i] - x {
        i - 1
    } else {
        i
    }
}

/// Flatten the njoy equiprobable emission bins into a cache-friendly
/// [`EmissionTable`]. An empty input (σ = 0 at this energy) yields an empty table.
fn build_emission_table(
    bins: &[njoy_outram_park_fork::thermr::scattering::ThermalEmissionBin],
) -> EmissionTable {
    use uom::si::energy::electronvolt;
    if bins.is_empty() {
        return EmissionTable { e_out: Vec::new(), cosines: Vec::new(), n_mu: 0 };
    }
    let n_mu = bins.iter().map(|b| b.cosines.len()).min().unwrap_or(0);
    let mut e_out = Vec::with_capacity(bins.len());
    let mut cosines = Vec::with_capacity(bins.len() * n_mu);
    for b in bins {
        e_out.push(b.outgoing_energy.get::<electronvolt>());
        for j in 0..n_mu {
            cosines.push(b.cosines[j]);
        }
    }
    EmissionTable { e_out, cosines, n_mu }
}

/// A logarithmically spaced grid of `n` points on `[lo, hi]` \[eV\] (both
/// endpoints included). Used for both the σ and emission incident-energy grids.
fn log_grid(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if n <= 1 {
        return vec![lo];
    }
    let (llo, lhi) = (lo.ln(), hi.ln());
    (0..n)
        .map(|k| (llo + (lhi - llo) * k as f64 / (n as f64 - 1.0)).exp())
        .collect()
}

/// Linear interpolation of `y(x)` on an ascending grid, clamped to the endpoints
/// outside the grid. `xs` and `ys` are parallel and non-empty.
fn interp_linear(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let mut i = 0;
    while i + 1 < n && xs[i + 1] < x {
        i += 1;
    }
    let (x0, x1) = (xs[i], xs[i + 1]);
    let (y0, y1) = (ys[i], ys[i + 1]);
    if x1 > x0 {
        y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    } else {
        y0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_grid_spans_endpoints_ascending() {
        let g = log_grid(1.0e-5, 4.0, 50);
        assert_eq!(g.len(), 50);
        assert!((g[0] - 1.0e-5).abs() < 1.0e-12);
        assert!((g[49] - 4.0).abs() < 1.0e-9);
        for w in g.windows(2) {
            assert!(w[1] > w[0], "grid must be strictly ascending");
        }
    }

    #[test]
    fn nearest_index_picks_the_closer_grid_point() {
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        assert_eq!(nearest_index(&xs, -5.0), 0);
        assert_eq!(nearest_index(&xs, 0.4), 0);
        assert_eq!(nearest_index(&xs, 0.5), 0, "a tie must go low");
        assert_eq!(nearest_index(&xs, 0.6), 1);
        assert_eq!(nearest_index(&xs, 2.9), 3);
        assert_eq!(nearest_index(&xs, 99.0), 3);
    }

    /// The Bragg step table reproduces the `1/E` sawtooth exactly: σ is zero
    /// below the first edge, constant × `1/E` between edges, and steps up at
    /// each one. Built from a synthetic three-edge table so the arithmetic is
    /// checkable by hand — no ENDF file needed.
    #[test]
    fn coherent_table_is_a_one_over_e_sawtooth() {
        // Edges at 1, 2, 4 eV with cumulative S/natom = 0, 10, 30 eV·barn.
        // The leading zero-weight edge mimics graphite's 0.4556 meV point.
        let t = CoherentElasticTable {
            edges_ev: vec![1.0, 2.0, 4.0],
            s_cum: vec![0.0, 10.0, 30.0],
        };
        assert_eq!(t.cross_section(0.5), 0.0, "below the first edge");
        assert_eq!(t.cross_section(1.5), 0.0, "first edge carries zero weight");
        assert!((t.cross_section(2.0) - 5.0).abs() < 1.0e-12); // 10 / 2
        assert!((t.cross_section(3.0) - 10.0 / 3.0).abs() < 1.0e-12);
        assert!((t.cross_section(4.0) - 7.5).abs() < 1.0e-12); // 30 / 4
        assert!((t.cross_section(8.0) - 3.75).abs() < 1.0e-12); // 1/E decay
                                                                // Bragg cutoff skips the zero-weight leading point.
        assert!((t.bragg_cutoff_ev() - 2.0).abs() < 1.0e-12);
    }

    /// Sampling the same synthetic table: `E_out == E_in` always, μ is one of
    /// the discrete Bragg cosines `1 − 2E_k/E`, and the zero-weight edge is
    /// never selected. At E = 4 eV the two open edges (2 and 4 eV) carry
    /// weights 10 and 20, so μ = 0.0 should appear ~1/3 of the time and
    /// μ = −1.0 ~2/3.
    #[test]
    fn coherent_table_samples_only_open_edges() {
        let t = CoherentElasticTable {
            edges_ev: vec![1.0, 2.0, 4.0],
            s_cum: vec![0.0, 10.0, 30.0],
        };
        assert!(
            t.sample(0.5, &mut 1).is_none(),
            "no scatter below the cutoff"
        );
        let mut seed = 42u64;
        let (mut n0, mut n1) = (0usize, 0usize);
        for _ in 0..20_000 {
            let (e_out, mu) = t.sample(4.0, &mut seed).unwrap();
            assert_eq!(e_out, 4.0, "coherent elastic conserves energy");
            if (mu - 0.0).abs() < 1.0e-12 {
                n0 += 1;
            } else if (mu + 1.0).abs() < 1.0e-12 {
                n1 += 1;
            } else {
                panic!("unexpected cosine {mu}: the 1 eV edge has zero weight");
            }
        }
        let f0 = n0 as f64 / 20_000.0;
        // 1/3 ± 5σ with σ = sqrt(p(1-p)/N) ≈ 0.0033.
        assert!(
            (f0 - 1.0 / 3.0).abs() < 5.0 * 0.0033,
            "edge weighting off: f0 = {f0}"
        );
        assert_eq!(n0 + n1, 20_000);
    }

    #[test]
    fn no_elastic_channel_contributes_nothing() {
        let e = ThermalElastic::None;
        assert_eq!(e.cross_section(0.0253), 0.0);
        assert!(e.sample(0.0253, &mut 1).is_none());
        assert_eq!(e.kind_name(), "none");
    }

    #[test]
    fn interp_linear_clamps_and_interpolates() {
        let xs = vec![0.0, 1.0, 2.0];
        let ys = vec![10.0, 20.0, 40.0];
        assert_eq!(interp_linear(&xs, &ys, -1.0), 10.0); // clamp low
        assert_eq!(interp_linear(&xs, &ys, 3.0), 40.0); // clamp high
        assert!((interp_linear(&xs, &ys, 0.5) - 15.0).abs() < 1.0e-12);
        assert!((interp_linear(&xs, &ys, 1.5) - 30.0).abs() < 1.0e-12);
    }
}
