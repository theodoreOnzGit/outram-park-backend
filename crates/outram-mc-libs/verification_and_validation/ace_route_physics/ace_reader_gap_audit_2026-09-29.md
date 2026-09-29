# ACE reader gap audit (GitHub #365 umbrella)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

The audit covers every ACE reading capability of the outram-mc ACE route. That
is `njoy_outram_park_fork::acer::{read, ce_decode, ce_laws, delayed,
photon_read, thermal_read}`, `purr::UrrProbabilityTables::from_ace`, and their
consumers `Nuclide::from_ace` and `ThermalScattering::from_ace`.

Each capability was compared against two references:
- **the reader:** OpenMC's Python ACE reader (`openmc/data/ace.py`,
  `neutron.py`, `reaction.py`, `angle_energy.py`, `urr.py`, `thermal.py`) and
  its C++ samplers (`src/*.cpp`);
- **the writer:** NJOY2016's ACER (`acefc.f90`, `aceth.f90`).

Every "after" row names the test that verifies it against OpenMC's reader of
the same file. A row that is still open is recorded on #365 with what closing
it would take.

**Status keys:**
- **fixed:** ported and verified in this pass.
- **was fixed:** fixed earlier in this pass (#365 F-19, #366).
- **open:** a known gap, left for the reason given.
- **out of scope:** transport has no consumer; the reason is given.
- **matches upstream:** OpenMC refuses it too.

## Files and formats

| feature | ACE location | upstream reader | before | after |
|---|---|---|---|---|
| Type 1, legacy header | header lines 1–12 | `ace.py:326-420` | read | read |
| Type 1, **ACE 2.0.1 header** | `2.0.x name source` / `awr kT date n` / n comment lines | `ace.py:349-359` | parse error | **fixed**: `acer::read::parse_type1_library`; test `ace_file_formats_vs_openmc` |
| **Several tables in one file**, one picked by name | concatenated tables | `ace.py:194-241`, `get_table` | XSS-length error | **fixed**: `read_library`, `read_table`; `read` refuses a multi-table file by name |
| NJOY's **exponent-less floats** (`1.2345-05`) | XSS | `ace.py:399-405` (`ENDF_FLOAT_RE`) | parse error | **fixed**: `parse_ace_float`. OpenMC 0.16.1.dev25 with NumPy 2.5.3 itself fails here, because `np.fromstring` raises before its fallback runs |
| Type 2, NJOY's Fortran **sequential** layout | record markers | (OpenMC cannot read it) | read | read |
| Type 2, MCNP/OpenMC **direct-access** layout (4096-byte records) | fixed records | `ace.py:243-325` | mis-sniffed as text | **fixed**: `parse_type2_direct_library` |
| **gzipped Type 2** | gzip member | — | always parsed as Type 1 | **fixed** |

## Continuous-energy neutron tables

| feature | ACE location | upstream | before | after |
|---|---|---|---|---|
| ESZ energy / total / elastic | JXS(1) | `neutron.py:557-584` | read, wired | unchanged |
| ESZ absorption (MT=101) | JXS(1) | same | decoded, unused | unused by design: absorption is summed from the channels, as OpenMC's micro XS is |
| ESZ heating | JXS(1) | `neutron.py:580-584` | decoded, unused | **out of scope**: no heating/KERMA tally consumer in this crate |
| MTR / LQR / LSIG / SIG | JXS(3,4,6,7) | `reaction.py:1015-1049` | read | read |
| **Transport channel set** (MT=4 vs levels, MT=18 vs partials) | MTR | `neutron.py:634-640` | lump kept, levels dropped | **was fixed (#366)**: `transport_channel_mts`; test `ace_transport_channels_vs_openmc` |
| **Partial-fission-only tables** (U-234): σ_f and χ | MTR 19/20/21/38 | `reaction.py:1073-1079` | σ_f = 0, first-chance χ only | **was fixed (#366)** |
| TYR sign (frame) | JXS(5) | `reaction.py:1055` | read; **CM analytic laws silently dropped** | **fixed**: kept with `lct = 2` and transformed as OpenMC's `scatter_in_cm`; test `uncorrelated_cm_emission` |
| **\|TY\| > 100 energy-dependent yield** | DLW + \|TY\|−101 | `reaction.py:1059-1062` | not read (MT=5 multiplicity always 1) | **fixed**: `ce_laws::decode_reaction_yield` wired to MT=5; test `ace_mt5_yield_vs_openmc` (e.g. U-235 y(10 MeV) = 1.98752, was 1) |
| AND: isotropic / tabulated | JXS(8,9) | `angle_distribution.py:143-200` | read | read |
| **AND 32 equiprobable bins** | AND | `angle_distribution.py:176-185` (a histogram `Tabular`) | read, but sampled **lin-lin** with the end bins' densities borrowed | **fixed**: a histogram, as OpenMC; test `correlated_angle_vs_openmc` (constructed U-235 MT=2, 72 rows converted, 3 energies, \|z\| ≤ 1.01; reading it lin-lin fails at z = −7.6) |
| AND tabulated `intt` (histogram vs lin-lin) | AND | `angle_distribution.py:143-200`; `Tabular::sample` | discarded (read as lin-lin), then refused by name when not 2 | **fixed**: `intt = 1` carried and sampled as a histogram; test `correlated_angle_vs_openmc` (constructed U-235 MT=51, \|z\| ≤ 1.51). No held or NJOY-written table uses it (census: 11 tables, 14 000+ rows, all 2) |
| DLW law 2, 3, 33 | JXS(10,11) | `angle_energy.py:83-88` | read | read |
| DLW law 4 / 44 / 61 | same | `energy_distribution.py`, `kalbach_mann.py`, `correlated.py` | read | read |
| **Law 4 as a continuum on MT=91/16/17: its AND cosine** | AND | `reaction.py:1131-1135` | AND ignored (stored as correlated-isotropic) | **fixed**: placed as `UncorrelatedEmission` with AND; test `ace_law4_continuum_cosine` on NJOY2016 Li-7 (ENDF/B-VIII.0) and U-238 (JENDL-3.3) |
| Law 4 / 44 / 61 **discrete lines** (INTT ≥ 10); ENDF MF=6 LAW=1 `ND > 0` | DLW | `energy_distribution.py:1239-1262`, `correlated.py:352-381`; `distribution_energy.cpp` | refused by name (ENDF: no law) | **fixed** (both readers; sampler as OpenMC: lines unscaled, envelope from `e_out[n_discrete]`). No ENDF/B-VIII.0 neutron subsection has lines (2377 scanned, 557 tapes), so a format-exact table was **constructed** from NJOY2016 Li-7 MT=16; test `ace_discrete_lines_vs_openmc`: ⟨E′⟩ at 5 energies |z| ≤ 2.40 (re-drawn with two other seed bases, |z| ≤ 2.35, signs mixed); ignoring the lines fails at z = −12.9. The ENDF-side path has no table to verify against |
| Law-61 cosine-table `intt` | DLW | `correlated.py` | discarded, then refused when not 2 | **fixed**: histogram rows carried and sampled; test `correlated_angle_vs_openmc` (constructed U-235 MT=91, \|z\| ≤ 1.96) |
| **Law-61 cosine row choice and inverse** (also the linearised MF=6 Legendre rows on the ENDF route) | DLW | `secondary_correlated.cpp` `sample_dist`; `distribution.cpp` `Tabular::sample` | always the lower row `k` of the `E′` bin; cosine inverse linear in the cdf | **fixed**: nearer cdf edge (`k` or `k+1`, lin-lin tables), quadratic inverse; test `correlated_angle_vs_openmc` on NJOY2016 U-235 MT=91/16, ⟨μ⟩ and ⟨μE′⟩ \|z\| ≤ 1.95 over 6 rows, the old scheme off by up to 18 sem; record `correlated_angle_2026-09-29.md` (k shift there) |
| **Law-44 `r`, `a` interpolation** | DLW | `secondary_kalbach.cpp` `sample_params` | row `k`'s `r`, `a` | **fixed**: interpolated to the raw `E′` on lin-lin tables; every held law-44 table is a histogram in `E′` (O-16 control unchanged, \|z\| ≤ 1.78), so the test **constructs** lin-lin O-16 MT=91: \|z\| ≤ 1.11, the old scheme off by up to 9.8 sem |
| DLW law 7 / 9 / 11 | same | `energy_distribution.py` | read | read |
| DLW law 66 | same | `nbody.py:122` | read | read |
| DLW law 5, 67, 1, 22, 24 | same | OpenMC raises too | refused | **matches upstream** |
| **LNW chains** of correlated / phase-space laws | DLW | `reaction.py:1082-1092` | refused (F-19) | **was fixed (#365)**: applicability-selected branches; test `ace_lnw_mixture_f19` |
| LNW chain mixing analytic + correlated links | DLW | OpenMC handles it | refused | **open, narrow**: ACER never writes one (MF=5 and MF=6 laws are not chained) |
| NU prompt + total | JXS(2) | `reaction.py:233-313` | total used | unchanged (correct) |
| **NU single block + DNU** = prompt | JXS(2), JXS(24) | `reaction.py:257-258` | read as total, so delayed lost | **fixed**: total = prompt + delayed; test `ace_nu_single_block_with_dnu` (the old reading was −0.66 % on a constructed U-235 case) |
| **NU, DNU, BDD and \|TY\| > 100 yield interpolation regions** (and on the ENDF route MF=1/452, 455, MF=5/455 `p_k`, MF=6 yields) | NU, DNU, BDD, DLW | `function.py` `Tabulated1D`; `endf.cpp` `Tabulated1D::operator()` | NU: dropped, then refused; DNU/BDD/ENDF: **dropped silently**; yields: refused | **fixed**: carried and evaluated as OpenMC's `Tabulated1D` (INT 1–5, clamped); lin-lin tables keep their old arithmetic bit for bit. No ENDF/B-VIII.0 nu table is non-lin-lin (86/86/84 MT=452/456/455) and the one non-lin-lin neutron yield (F-19, histogram) is constant, so a U-235 table was **constructed**; test `ace_interp_regions_vs_openmc`: 56 values equal to OpenMC's rule to 1.9e-16, where the lin-lin reading is off by up to 20 %. Record `interp_regions_2026-09-29.md`. **Still refused, narrow:** a single (prompt) NU block plus DNU where either carries a non-lin-lin region, because their sum is not one tabulated function; no producer is known |
| NU polynomial (LNU = 1) | NU | `reaction.py:263-268` (`Polynomial`, exact, unclamped) | tabulated 1e-5 eV–20 MeV, lin-lin, clamped above | **fixed** (both routes): `NuBar::poly` evaluated exactly; test `ace_nu_polynomial_vs_openmc` on a constructed U-235 table, 10 energies 1e-7 eV–150 MeV equal to OpenMC to 0.0 relative |
| DNU, BDD | JXS(24,25) | `reaction.py:319-365` | read, wired | read, wired |
| **DNEDL / DNED delayed spectra** | JXS(26,27) | `reaction.py:355-357`; `physics.cpp` `sample_fission_neutron` | not read: delayed neutrons born with the prompt χ (both routes) | **fixed** (both routes; ENDF via MF=5/455 LF=5/LF=1): test `delayed_spectra_vs_openmc`; record `delayed_spectra_2026-09-29.md` (the predicted k sign was wrong; recorded) |
| **UNR probability tables**: interpolation, total, bounds | JXS(23) | `nuclide.cpp` `calculate_urr_xs`, `urr.h` | nearest table energy; band total; inclusive bounds | **fixed** (both routes): test `urr_sampling_vs_openmc`, bit-equal to OpenMC on 186 rows; k effect consistent with zero (`urr_sampling_2026-09-29.md`) |
| UNR heating column | JXS(23) | — | decoded, unused | **out of scope**: no heating tally |
| Neutron-emitting reactions other than 5/16/17/18/51–91 (MT=22, 24, 28, 32–37, 41–45, …) | MTR + TYR + DLW (ENDF MF=6 or MF=4+5) | `physics.cpp` `sample_scatter` → `inelastic_scatter` | not transported: cross section in the elastic remainder, extra neutrons lost (**both routes**) | **fixed** (both routes, every kernel): own law, Q, frame and multiplicity; on by default (pinned), `without_other_neutron_channels()` ablates. Test `other_neutron_channels_vs_openmc`: channel sets, σ, yields and frames equal to OpenMC for O-16/F-19/Al-27/Si-28/Mn-55/U-234/Li-7 (62 rows; OpenMC's ENDF reader leaves MF=4+5 yields at 1, so those are checked against the ACE route); selection z = +0.21; ACE vs ENDF KS ≤ 4.4e-3 (crit 1.13e-2); partition closes to 3e-9 at 10–20 MeV (19–28 % open when ablated) |
| Absorption levels 600–849 without their lump | MTR | `neutron.py:617-630` | outside the disappearance sum (lost to elastic) | **fixed**: the lump is synthesised from its levels. ENDF/B-VIII.0 has 202 such cases (Mn-55, Fe-54/56, …); NJOY adds the lump, so the test **constructs** one from NJOY2016 Mn-55 (MT=103/107 relabelled): absorption equal to the lumped table to 1.1e-7 at 12 energies, where the lumps carry up to 98.8 % of absorption; reverting fails. The ENDF route already agrees (RECONR builds the lump; Mn-55 absorption ENDF = ACE at 2–19 MeV) |
| `channel_mts` double count of 650–799 under 104–106 and 875–891 under 16 in `reconstructed_total` | MTR | `SUM_RULES` | double counted (diagnostic only) | **fixed**: test `ace_reconstructed_total_light_nuclides` (8 light tables ≤ 5.0e-9) |
| GPD, YP, FIS | JXS(12,20,21) | not read by OpenMC | not read | **out of scope** |
| Photon production (MTRP … DLWP) | JXS(13-19) | `_get_photon_products_ace` | decoded, verified | **out of scope**: neutron-only transport |
| Charged-particle blocks | NXS(7), JXS(30+) | not read by OpenMC `IncidentNeutron` | not read | **out of scope** |

## Thermal S(α,β) tables

| feature | ACE location | upstream | before | after |
|---|---|---|---|---|
| Inelastic, IFENG = 0 (equiprobable) | NXS(7), JXS(1-3) | `thermal.py:794-808` | read and sampled | **fixed** (GitHub #407): OpenMC's `IncoherentInelasticAEDiscrete` (interpolated between the bracketing tables, discrete `E′`) is the default on both routes. Test `thermal_equiprobable_vs_openmc`: NJOY2016 H in H2O and graphite, 11 energies, five moments with \|z\| ≤ 1.86 and KS D/crit ≤ 0.92; the legacy scheme fails the KS at every energy (D/crit ≥ 4.1). ~~Named asymmetry with OpenMC … (#188, deliberate)~~ **CORRECTED 2026-09-29: never a maintainer decision; replaced by OpenMC's scheme per the maintainer**. The legacy scheme is `--ablate legacy-thermal-sampling`, pinned off by `correct_physics_is_default` |
| Inelastic, IFENG = 1 (skewed) | NXS(7) | `thermal.py:806`; `secondary_thermal.cpp` `IncoherentInelasticAEDiscrete` | read, **skew weights ignored** (silently mis-sampled) | **fixed**: OpenMC's discrete scheme with the skewed bin weights; test `thermal_skewed_vs_openmc` on an NJOY2016 `iwt = 0` H-in-H2O table (all |z| ≤ 2.84; 1 eV re-drawn at two other seed bases: |z| ≤ 1.29); a mutation to equiprobable bins fails at z = 234 |
| Inelastic, IFENG = 2 (continuous) | NXS(7) | `thermal.py:809-887`; `secondary_thermal.cpp` `IncoherentInelasticAE` | refused | **fixed**: decoded and sampled as OpenMC; test `thermal_ifeng2_vs_openmc` on an NJOY2016 `iwt = 2` H-in-H2O table (all |z| ≤ 1.38); record `thermal_ifeng2_2026-09-29.md` |
| Coherent elastic (IDPNC = 4) | JXS(4,5) | `thermal.py:897-907` | read and sampled | unchanged |
| Incoherent elastic (IDPNC = 3) | JXS(4-6) | `thermal.py:909-927`; `secondary_thermal.cpp` `IncoherentElasticAEDiscrete` | read; nearest-energy cosine, no smearing (doc claimed to mirror OpenMC) | **fixed** (both routes): interpolated and smeared as OpenMC; test `thermal_incoherent_elastic_vs_openmc` on NJOY2016 H in ZrH (all |z| ≤ 1.18); the old rule fails at z = 28 |
| **Mixed elastic (IDPNC = 5)** | JXS(4,5) + JXS(7-9), NXS(8) | `thermal.py:897-942`; `secondary_thermal.cpp` `MixedElasticAE` | **silently mis-decoded** (coherent data read as incoherent) | **fixed**: read in full and sampled as OpenMC. No ENDF/B-VIII.0 TSL is LTHR = 3 (34 scanned), so a format-exact table was **constructed** from NJOY2016 graphite (coherent) + H-in-ZrH (incoherent block); test `thermal_mixed_elastic_vs_openmc`: σ equal to OpenMC (0.0 rel), ⟨μ⟩/⟨μ²⟩ |z| ≤ 1.63; a swapped channel choice fails |

## Other table classes

| class | before / after |
|---|---|
| dosimetry `.y`, photoatomic `.p`, photonuclear `.u` | container read only; **out of scope** for a neutron transport consumer |
