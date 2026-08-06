# Virtual Test Bed (VTB) — vendored contents inventory

> **Added by OUTRAM PARK — not part of the upstream distribution.** Attribution,
> licence, provenance, and the subset rule are in [`NOTICE`](./NOTICE) beside
> this file. Read that first.
>
> **Upstream:** <https://github.com/idaholab/virtual_test_bed> — NRIC / Idaho
> National Laboratory, DOE NEAMS program. **CC-BY-4.0.**
> Commit `f3314028132912e5b02b9040d9e7cc60290fce4b`, accessed 2026-08-06.
> **Unmodified**; vendored as a subset (see `NOTICE`).

This is a map of what the vendored tree actually contains, so an agent or the
maintainer can find a reactor case without walking 1536 files. Every path below
was verified present in this copy, and every vendored file was checked
byte-for-byte against upstream at the pinned commit (1536/1536 identical).

**Nothing here is verified or validated by OUTRAM PARK.** The `gold/` CSVs are
upstream's own regression baselines. Treat them as third-party reference values,
not as OUTRAM PARK V&V results.

---

## What the VTB is

A collection of open reactor "challenge problems" — multiphysics input decks for
NEAMS tools — spanning nine reactor families. Most decks target MOOSE-based
applications. Upstream's own code table is at
[`doc/content/vtb_pages/codes.md`](./doc/content/vtb_pages/codes.md); in short:

| Code | Physics | Open source? |
|---|---|---|
| MOOSE | framework + physics modules | Yes |
| Cardinal | multiphysics (OpenMC + NekRS) | Yes |
| OpenMC | Monte Carlo transport | Yes |
| Nek5000 / NekRS | CFD | Yes |
| MASTODON | seismic | Yes |
| TMAP8 | tritium migration | Yes |
| Griffin | deterministic transport | No (NCRC) |
| Pronghorn | coarse-mesh thermal hydraulics | No (NCRC) |
| Bison | fuel performance | No (NCRC) |
| SAM | systems analysis | No (NCRC) |
| Sockeye | heat pipes | No (NCRC) |
| RELAP-7, Grizzly, DireWolf, BlueCRAB, Sabertooth | various | No (NCRC) |
| Serpent | Monte Carlo transport | No |

The input decks are all vendored here regardless of whether the code that runs
them is open — the decks themselves are CC-BY-4.0 and are readable as
specifications of geometry, materials, and coupling.

---

## Size and shape of this copy

| | Vendored here | Full upstream |
|---|---|---|
| Size | 37 MB | 352 MB |
| Files | 1536 upstream (+ `NOTICE`, `INVENTORY.md`) | 2491 tracked paths |

| Content type | Count here |
|---|---|
| MOOSE-style input decks (`.i`) | 377 |
| Documentation pages (`.md`) | 280 |
| Gold / reference results (`.csv`) | 486 |

Per family:

| Family | Path | Size | Files |
|---|---|---|---|
| High-temperature gas-cooled reactors | `htgr/` | 4.4 MB | 254 |
| Molten salt reactors | `msr/` | 9.7 MB | 269 |
| Microreactors | `microreactors/` | 7.6 MB | 432 |
| Sodium fast reactors | `sfr/` | 3.5 MB | 138 |
| Light-water reactors | `lwr/` | 2.7 MB | 36 |
| Lead-cooled fast reactors | `lfr/` | 2.6 MB | 29 |
| Pebble-bed fluoride-salt reactors | `pbfhr/` | 1.2 MB | 70 |
| Research reactors | `research_reactors/` | 284 KB | 8 |
| Fusion | `fusion/` | 40 KB | 2 |
| Documentation | `doc/content/` | 5.0 MB | 291 |

---

## High-temperature gas-cooled reactors — `htgr/`

Fourteen case directories. Docs under `doc/content/htgr/`.

| Case | Path | What it is |
|---|---|---|
| HTGR prismatic assembly | `htgr/assembly/` | Multiphysics coupling of OpenMC, MOOSE and THM for a prismatic HTGR assembly |
| Generic PBR (SAM) | `htgr/generic-pbr/` | SAM generic pebble-bed reactor system model |
| Generic PBR tutorial | `htgr/generic-pbr-tutorial/` | Eight-step guided build of a generic PBR in Pronghorn (`step1`…`step8`) |
| GPBR200 | `htgr/gpbr200/` | 200 MW generic pebble-bed reactor with stochastic analyses. Sub-cases: `core_neutronics/` (Griffin equilibrium core), `core_thermal_hydraulics/` (Pronghorn), `coupling/`, `pebble_thermomechanics/` (Bison), `pebble_surrogate_modeling/`, `sensitivity_analysis/` |
| HTR-10 | `htgr/htr10/` | Griffin neutronics model of the Chinese HTR-10 test reactor |
| HTR-PM | `htgr/htr-pm/` | `core-multiphysics/` (neutronics + thermal-fluid + DLOFC depressurised loss-of-forced-cooling transient) and `sam-htrpm/` (SAM plant model) |
| HTTF | `htgr/httf/` | Oregon State High Temperature Test Facility. `PG26/` transient, `sam_ring_model/` 2-D ring model, `lower_plenum_mixing/` Nek5000/NekRS CFD, `inputs/`, `positions/` |
| HTTR | `htgr/httr/` | Japanese High Temperature Engineering Test Reactor — `steady_state_and_null_transient/` multiphysics, `mesh/` |
| LEU pulse (TREAT) | `htgr/leu_pulse/` | Dispersed-UO2 low-enriched-uranium fuel pulse model |
| MHTGR | `htgr/mhtgr/` | 350 MW modular HTGR: `mhtgr_griffin/` numerical benchmark, `mhtgr_sam/` system model, `3D_mesh/` |
| Open Xe-100 | `htgr/open-xe100/` | X-energy Xe-100 open model — Griffin steady state plus null, PKE and IQS transients |
| 67-pebble core | `htgr/pb67_cardinal/` | Cardinal conjugate-heat-transfer LES of a 67-pebble core (NekRS + OpenMC) |
| PBMR-400 | `htgr/pbmr400/` | OECD/NEA PBMR-400 pebble-bed benchmark |
| TRISO fuel | `htgr/triso_fuel/` | Bison TRISO particle fuel performance model |

## Molten salt reactors — `msr/`

Six case directories. Docs under `doc/content/msr/`.

| Case | Path | What it is |
|---|---|---|
| CNRS benchmark | `msr/cnrs/` | CNRS molten-salt multiphysics benchmark, phases 0 (steady single-physics), 1 (steady coupled) and 2 (time-dependent coupled) |
| Generic MSR | `msr/generic_msr/` | `depletion/` (Griffin MSR depletion) and `seismic_analysis/` (MASTODON base-isolated nuclear power plant building) |
| Graphite behaviour | `msr/graphite_model/` | Graphite in molten-salt environments: baseline, groove and pit profiles, salt infiltration, hotspot, wear, 3-D stress, failure analysis |
| LOTUS (LMCR) | `msr/lotus/` | LOTUS molten chloride reactor experiment — Griffin-Pronghorn multiphysics |
| MSFR | `msr/msfr/` | Molten Salt Fast Reactor, the largest MSR case. `steady/` and `transient/` Griffin-Pronghorn, `plant/` (Griffin-Pronghorn-SAM coupled plant), `core_cfd/` (Nek5000 2-D RANS), `thermochemistry/` (Thermochimica), `mgxs/`, `mesh/` |
| MSRE | `msr/msre/` | Molten Salt Reactor Experiment (ORNL). `steady_state/` and `multiphysics_core_model/` RZ models, `reactivity_insertion/`, `lp_cfd/` lower-plenum NekRS, `pipe_cardinal/` thermal-striping in piping |

## Microreactors — `microreactors/`

Nine case directories — the deepest family in the VTB. Docs under
`doc/content/microreactors/`.

| Case | Path | What it is |
|---|---|---|
| Control-drum rotation | `microreactors/drum_rotation/` | Microreactor control-drum rotation transient |
| GCMR | `microreactors/gcmr/` | Gas-cooled microreactor. `assembly/` multiphysics, `core/` whole-core neutronics + multiphysics incl. depressurisation (DP), single-coolant-channel-blockage (SCB) and fission-product-tracking variants, `balance_of_plant/`, `airjacket/` (Nek5000 CFD). Includes a Serpent model under `core/Serpent_Model/` |
| gHPMR | `microreactors/gHPMR/` | 2-D generic heat-pipe microreactor mesh model |
| HPMR assembly | `microreactors/hpmr_assembly/` | Heat-pipe microreactor assembly |
| HPMR-H2 | `microreactors/hpmr_h2/` | Heat-pipe microreactor with hydrogen redistribution |
| KRUSTY | `microreactors/KRUSTY/` | Kilopower Reactor Using Stirling TechnologY. `Multiphysics_SS/` steady state, `Multiphysics_15C_RIT/` and `Multiphysics_30C_RIT/` reactivity-insertion tests (15¢ / 30¢), `Neutronics/` (Serpent + MC²-3 models), `MESH/`, `gold/` |
| MRAD (HP-MR) | `microreactors/mrad/` | Heat-pipe micro reactor, the most elaborate microreactor case. `steady/`, `transient_null/`, `load_following/`, `heat_pipe_failure/`, `3D_core_drum_rotation_ss/` and `_tr/` (inadvertent drum rotation), `triso_failure/` TRISO failure analysis, sodium variants `steady_Na/` `startup_Na/` `load_following_Na/` `transient_null_Na/`, plus `legacy/` and a `Serpent_Model/` |
| S8ER | `microreactors/s8er/` | SNAP-8 Experimental Reactor multiphysics model |
| STARTR | `microreactors/STARTR/` | Sodium-cooled Thermal-spectrum Advanced Research Test Reactor. **Monte Carlo only** — an `OpenMC/` model (geometry/materials/settings XML + Python driver) and an `MCNP/` input. No MOOSE `.i` decks |

## Sodium fast reactors — `sfr/`

Eight case directories. Docs under `doc/content/sfr/`.

| Case | Path | What it is |
|---|---|---|
| ABTR | `sfr/abtr/` | Advanced Burner Test Reactor core model |
| ABTR XS workflow | `sfr/abtr_xsgen_workflow/` | Cross-section generation plus full-core eigenvalue calculation for the ABTR |
| EBR-II DP11 | `sfr/ebr2_x447_dp11/` | Bison fuel performance for pin DP11 of the IFR X447/A experiment, with BISON-FIPD data integration |
| Hex duct bowing | `sfr/hex_duct_bowing/` | IAEA benchmarks VP1 (linear thermal gradient) and VP3A (symmetric sector bowing) |
| SEFOR | `sfr/sefor/` | Southwest Experimental Fast Oxide Reactor. `Core_IE/` and `Core_II/` Griffin models, `Cross_Section/` (MC²-3), `Mesh/` (MOOSE reactor module), `Shift_Reference/` Monte Carlo reference. Includes isothermal-test cases |
| Single assembly | `sfr/single_assembly/` | SFR single-assembly model |
| Subchannel | `sfr/subchannel/` | Five subchannel demonstrations: `EBR-II/` (SHRT-17 validation), `ornl_19_pin/`, `toshiba_37_pin/`, `THORS/` (partial blockages in simulated LMFBR assemblies), `multiple_SCM_assemblies/` |
| VTR | `sfr/vtr/` | Versatile Test Reactor core model |

## Light-water reactors — `lwr/`

| Case | Path | What it is |
|---|---|---|
| Metallic HCF | `lwr/hcf/` | Bison 3-D cycle fuel performance for metallic high-conductivity fuel |
| RPV fracture | `lwr/rpv_fracture/` | Reactor pressure vessel: `thermomechanical/` 3-D model and `probabilistic_fracture/` 3-D probabilistic fracture mechanics (Grizzly) |

## Lead-cooled fast reactors — `lfr/`

| Case | Path | What it is |
|---|---|---|
| 7-pin Cardinal demo | `lfr/7pin_cardinal_demo/` | Cardinal model of a 7-pin LFR assembly (OpenMC + NekRS) |
| Heterogeneous single assembly | `lfr/heterogeneous_single_assembly_3D/` | 3-D high-fidelity Griffin neutronics for an LFR assembly (127-pin, 9-group) |

## Pebble-bed fluoride-salt-cooled reactors — `pbfhr/`

| Case | Path | What it is |
|---|---|---|
| gFHR | `pbfhr/gFHR/` | Generic fluoride-cooled high-temperature reactor — `steady/` Griffin + Pronghorn equilibrium core, pebble/TRISO heat conduction, `data/` |
| Mark-1 PB-FHR | `pbfhr/mark1/` | UC Berkeley Mk1 PB-FHR. `steady/` Griffin-Pronghorn, `reflector/` bypass-flow reflector modelling (Nek5000), `plant/` balance of plant, `sam_model/` |

## Research reactors — `research_reactors/`

| Case | Path | What it is |
|---|---|---|
| AGN-201 | `research_reactors/agn/` | Aerojet General Nucleonics 201 research reactor — mesh and description |
| ATR butterfly valve | `research_reactors/atr/` | Advanced Test Reactor butterfly-valve coarse-mesh thermal hydraulics |

## Fusion — `fusion/`

| Case | Path | What it is |
|---|---|---|
| Divertor monoblock | `fusion/mcf/` | Magnetic-confinement fusion divertor monoblock during pulsed operation |

---

## Documentation and tutorials — `doc/content/`

The full MooseDocs source is vendored (figures excluded — see `NOTICE`). Useful
entry points:

| Path | What it is |
|---|---|
| `doc/content/index.md` | VTB documentation landing page |
| `doc/content/vtb_pages/codes.md` | Table of every code used, with licensing/availability |
| `doc/content/vtb_pages/models_by_codes_used.md` | Every model indexed by simulation tool — including which are **fully open-source runnable** |
| `doc/content/vtb_pages/models_by_simulation_type.md` | Models indexed by simulation type |
| `doc/content/vtb_pages/models_by_input_features.md` | Models indexed by MOOSE input features used |
| `doc/content/vtb_pages/running_models.md` | How to run the decks |
| `doc/content/vtb_tutorials/multiapps/` | Eight-chapter MOOSE MultiApps tutorial for reactor applications |
| `doc/content/vtb_tutorials/vtb_basics.md` | VTB basics tutorial |
| `doc/content/vtb_tutorials/neams-workbench.md` | NEAMS Workbench tutorial |
| `doc/content/bib/` | BibTeX bibliography for the whole VTB |
| `doc/content/vtb_pages/citing.md` | Upstream's own citation guidance |

Also present: `scripts/` (2 upstream helper scripts), `testroot`,
`testroot_bluecrab`.

---

## Excluded from this copy

Per the subset rule in `NOTICE`. To retrieve any of these, clone upstream at the
pinned commit and run `git lfs pull`.

**Figures and media** — all 711 files under `doc/content/media/` (191 MB of PNG,
JPEG, GIF, MP4). The markdown that references them is present; the images are
not, so `!media` directives will not resolve locally.

**Git LFS assets** — 73 files upstream stores in LFS. A plain clone yields
132-byte pointer stubs, not content, so these were never available to vendor.
They are mostly ExodusII meshes (`.e`), Nek meshes (`.re2`), restart fields
(`.fld`) and large multigroup cross-section XML — for example
`htgr/htr10/data/xs/htr-10-XS.xml`, `sfr/vtr/mesh/vtr_core.e`,
`pbfhr/gFHR/data/gFHR_4g_pebble.xml`, `microreactors/mrad/mesh/mrad_mesh.e`.

**Files at or above 1 MiB** — 32 files. Full list:

| Path | Kind |
|---|---|
| `htgr/assembly/geometry.xml` | OpenMC geometry |
| `htgr/leu_pulse/cross_sections/leu_20r_is_6g_d.xml` | multigroup XS |
| `htgr/mhtgr/mhtgr_griffin/data/materials_p0_trc.xml` | multigroup XS |
| `htgr/mhtgr/mhtgr_griffin/data/MHTGR_Tri_r2.e` | ExodusII mesh |
| `htgr/pbmr400/shared/oecd_pbmr400_yields_xs.txt` | fission yields / XS |
| `lfr/heterogeneous_single_assembly_3D/cross_section/LFR_127Pin_9g.xml` | multigroup XS |
| `lfr/heterogeneous_single_assembly_3D/cross_section/Step2/NonFuel/LFR_127Pin_NonFuel_9g.mcc3.sh.ISOTXS` | ISOTXS |
| `lfr/heterogeneous_single_assembly_3D/cross_section/Step2/NonFuel/LFR_127Pin_NonFuel_9g.mcc3.sh.xml` | multigroup XS |
| `lwr/rpv_fracture/probabilistic_fracture/plate_open_access.dat` | flaw data |
| `lwr/rpv_fracture/probabilistic_fracture/surface_open_access.dat` | flaw data |
| `lwr/rpv_fracture/probabilistic_fracture/weld_open_access.dat` | flaw data |
| `lwr/rpv_fracture/probabilistic_fracture/gold/rpv_pfm_3d_final_cpi_running_statistics_0035.csv` | gold result |
| `microreactors/gcmr/core/ISOXML/GCMR_XS_2grid_detailed.xml` | multigroup XS |
| `microreactors/KRUSTY/Neutronics/MC23/ISOTXS.adj.xml` | ISOTXS (adjoint) |
| `microreactors/mrad/isoxml/fullcore_xml_G11_endfb8_ss_tr.xml` | multigroup XS |
| `msr/lotus/mgxs/serpent_MCRE_xs_new.xml` | multigroup XS |
| `msr/msfr/core_cfd/2d_rans_Re1M/msfr.solution` | Nek5000 solution |
| `msr/msfr/plant/steady/gold/run_ns_restart.e` | ExodusII restart |
| `msr/msfr/plant/steady/restart/run_neutronics_out.e` | ExodusII restart |
| `msr/msfr/plant/steady/restart/run_neutronics_out_ns0_restart.e` | ExodusII restart |
| `msr/msfr/plant/steady/restart/run_neutronics_out_ns0_sam_balance_of_plant0_out_displaced.e` | ExodusII restart |
| `msr/msfr/plant/steady/restart/run_neutronics_restart.e` | ExodusII restart |
| `msr/msfr/plant/steady/restart/run_ns_coupled_restart.e` | ExodusII restart |
| `msr/msfr/plant/steady/restart/run_ns_restart.e` | ExodusII restart |
| `msr/msfr/steady/gold/run_ns_restart.e` | ExodusII restart |
| `msr/msfr/steady/restart/run_neutronics_out_ns0_restart.e` | ExodusII restart |
| `msr/msfr/steady/restart/run_neutronics_restart.e` | ExodusII restart |
| `msr/msfr/steady/restart/run_ns_restart.e` | ExodusII restart |
| `pbfhr/mark1/steady/cross_sections/2-D_8Gt_multiregions_transient.xml` | multigroup XS |
| `sfr/hex_duct_bowing/iaea_vp3a_symmetric_mesh.e` | ExodusII mesh |
| `sfr/sefor/Cross_Section/Core-I-E_450K_ENDF71.33gV2.xml` | multigroup XS |
| `sfr/subchannel/toshiba_37_pin/gold/toshiba_37_pin_out.e` | gold result |

**Submodules** — `apps/` (Bison, Griffin, Pronghorn, SAM, Sockeye, DireWolf,
BlueCRAB, Cardinal, RELAP-7, MOOSE, MASTODON, Grizzly, msd-tc). Most resolve to
INL-internal hosts (`github.inl.gov`, `code.ornl.gov`) and are not publicly
cloneable. The upstream mount points were empty in a plain clone.

**Git / CI plumbing** — `.github/`, `.civet/`, `.gitattributes`, `.gitmodules`,
and upstream's `.gitignore`. These two removals are load-bearing, not cosmetic:

- Upstream's `.gitignore` is a MOOSE build-artifact list that ignores `*.csv`,
  `*.xml`, `*.txt`, `*.e`, `*.dat` and `*.png` wholesale. Upstream keeps such
  files in tree only because they were force-added before the rules applied.
  Carried in here it would have silently dropped **57 genuine reference files**
  — material property CSVs (`lwr/hcf/U50Zr_*.csv`), OpenMC geometry/materials
  XML (`htgr/assembly/*.xml`), orifice coordinate tables (`sfr/vtr/*.txt`) —
  from the commit.
- `.gitattributes` declares `filter=lfs` on paths whose content is not LFS-backed
  in this copy, which would make Git try to apply LFS filters to plain files.

No content file was altered by either removal.
