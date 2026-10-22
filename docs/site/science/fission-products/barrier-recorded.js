// Cited and pending values for chapter 4 "TRISO layer migration and
// particle failure" (lessons 29-38), in ONE place. Hand-edited, not
// generated: each entry names its source and where in it; `src: null` means
// "source pending" and the page says so. Checked by check-fission.js.
//
// Open-corpus sources, read 2026-10-22 from crates/kovan-literature/
// reactor-literature (quotes are verbatim, ellipses marked):
//  - kugeler2017htgr: Kugeler, Nabielek & Buckthorpe, "The High Temperature
//    Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul",
//    EUR 28712 EN, JRC 2017, doi:10.2760/270321.
//    kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf. Printed page numbers.
//  - wichner1991nureg5647: Wichner, "Fission Product Plateout and Liftoff in
//    the MHTGR Primary System: A Review", NUREG/CR-5647 (ORNL/TM-11685),
//    1991. kovan-standard-open-corpus/us-doe/ornl-tm-11685-nureg-cr-5647-
//    fp-plateout-liftoff-mhtgr.pdf. Printed page numbers.
//  - fang2021analysis: Fang, Li, Cao, Liu & Fang, "Analysis of Fission
//    Products' Release in Pebble-Bed High-Temperature Gas-Cooled Reactor Fuel
//    Elements Using a Modified FRESCO II Numerical Model", Science and
//    Technology of Nuclear Installations 2021, doi:10.1155/2021/6627789
//    (CC BY). theodore-open-corpus/cc-by/fang2021-htgr-fission-product-
//    release-fresco.pdf. Article page 2.
// The PANAMA-I report (Verfondern & Nabielek, HTA-IB-03/90) is restricted:
// only its equations and constants appear, through boon-lay's code and the
// page numbers boon-lay's module docs give. Nothing of its prose or figures.
window.BARRIER_RECORDED = {
  sources: {
    kugeler: "Kugeler, Nabielek & Buckthorpe (2017), EUR 28712 EN, JRC, doi:10.2760/270321",
    wichner: "Wichner (1991), NUREG/CR-5647 (ORNL/TM-11685)",
    fang: "Fang et al. (2021), Sci. Technol. Nucl. Install., doi:10.1155/2021/6627789 (CC BY)",
    panama: "Verfondern & Nabielek (1990), PANAMA-I report, HTA-IB-03/90 (restricted: equations only)"
  },
  // Lesson 29, history.
  history: [
    { src: "fang", page: "2", quote: "The research and design of coated fuel particles have gone through two stages. In the initial design, the coated fuel particle structure is of Bi-isotropic (BISO) type, i.e., a loose and a dense pyrolytic carbon layer on the outer coating of the active core." },
    { src: "fang", page: "2", quote: "the BISO fuel particle gradually exposed its structural shortcomings; these were mainly reflected in the high level of heavy metal contamination during the coating deposition and the low retention ability for certain metal FPs (such as strontium). Based on the BISO fuel particle, the tri-isotropic (TRISO) fuel particle is designed." },
    { src: "kugeler", page: "28", quote: "the initially developed BISO-coated particles are completely sufficient concerning fission-product release, however they are outperformed by the behaviour of TRISO-coated particles in case of hypothetical accident conditions, for example with core heat-up to temperatures > 1 500 °C." },
    { src: "kugeler", page: "28", quote: "Today, fuel elements with BISO and TRISO particles can be seen to be fully tested and commercially deployable due to the long-term tests in AVR reactors." }
  ],
  // When SiC was first added, and which particles Dragon, Peach Bottom and
  // Fort St. Vrain used: not in the three sources read. Candidate: Demkowicz,
  // Liu & Hunn, J. Nucl. Mater. 515 (2019), not in the corpus.
  historyPending: ["the year and programme in which SiC was first added", "which particle types Dragon, Peach Bottom and Fort St. Vrain used"],
  // Lesson 29: Kugeler Table 5, "Design functions of the ceramic coating
  // layers of the TRISO-coated fuel particle", pp. 32-33 (verbatim cells).
  layerJobs: {
    page: "32-33",
    buffer: ["Provides void volume for gaseous fission products and reaction products (CO, CO2) released from fuel kernel.", "Accommodates fuel-kernel swelling.", "Protects PyC and SiC layers from fission product recoil."],
    ipyc: ["Diffusion barrier to fission products, retains gaseous fission products.", "Impermeable layer prevents Cl2 from reaching kernel during SiC deposition and prevents CO from interacting with SiC during irradiation.", "Provides mechanical substrate for deposition of SiC layer.", "Induces compressive stresses in SiC due to irradiation induced shrinkage."],
    sic: ["Primary barrier to fission products, retains all gaseous and solid fission products (except Ag) at normal operating temperatures < 1 250 °C and in accidents up to 1 600 °C.", "Load bearing layer for particle."],
    opyc: ["Creates compressive stress on SiC due to irradiation-induced shrinkage.", "Retains gaseous fission products.", "Provides bonding layer with carbonaceous fuel element matrix."]
  },
  // Lesson 29 and 31: Kugeler p. 42, 131I release in 1600 °C heating tests
  // (Schenk et al. 1991 as Kugeler cites it). Levels as Kugeler states them.
  iodine: {
    page: "42",
    rows: [
      { what: "exposed UO2 kernels (no coating)", level: "~100 %", lo: 1, hi: 1 },
      { what: "TRISO fuel elements cross-contaminated in the AVR by older BISO fuel of the 1960s and 1970s", level: "10^-5 level", lo: 1e-5, hi: 1e-5 },
      { what: "modern TRISO fuel elements", level: "10^-9 to 10^-8 level", lo: 1e-9, hi: 1e-8 }
    ],
    quote: "between these are nine orders of magnitude in release",
    quotePage: "43"
  },
  // Lesson 31: Wichner p. 13 (verbatim).
  wichner: [
    { page: "13", quote: "Diffusivities of metallic FPs through the PyC coatings, which completely stop the fission gases, are sufficiently high so that an intact SiC coating is required to prevent diffusive release of the metals from fuel particles." },
    { page: "13", quote: "intact Triso particles retain Cs and Sr to temperatures well above that expected in normal operation, but that silver is readily released above ~1423 K (1150°C)." }
  ],
  // Lesson 31: Kugeler p. 41 (verbatim).
  kernelSr: { page: "41", quote: "Only the fission product 90Sr is strongly retained in the oxide kernels at this temperature, but this is not the case for carbide kernels." },
  // Lesson 38, still pending.
  pending: {
    pdLanthanideAttack: null,  // named in the theory book step 4 from proprietary-folder sources; not open here
    amoebaEffect: null,        // kernel migration in UO2; not modelled anywhere in the workspace
    realGasPressure: null,     // ideal gas only (theory book step 6)
    pycCreepSwelling: null     // not in the failure chain (theory book step 6)
  }
};
