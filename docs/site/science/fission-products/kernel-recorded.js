// Cited and pending values for the chapter "TRISO kernel" (lessons 10-28),
// in ONE place. Hand-edited, not generated: each entry names its source and
// where in it; `src: null` means "source pending" and the page says so.
// Checked by check-fission.js.
window.KERNEL_RECORDED = {
  // The chemical families the chapter follows (maintainer's design,
  // 2026-10-20). Membership is the design's teaching grouping. The chemical
  // STATE of each family in UO2 is pending: Kleykamp (1985) is the candidate
  // source and has not been read here.
  families: [
    { id: "gas", name: "Noble gases", members: ["Xe", "Kr"], rep: 54,
      state: "insoluble gas atoms; bubbles", stateSrc: null },
    { id: "volatile", name: "Volatiles", members: ["Cs", "I", "Te", "Rb"], rep: 55,
      state: "volatile elements and compounds", stateSrc: null },
    { id: "alkearth", name: "Alkaline earths", members: ["Sr", "Ba"], rep: 38,
      state: "oxides", stateSrc: null },
    { id: "noblemetal", name: "Noble metals", members: ["Mo", "Ru", "Rh", "Pd", "Tc"], rep: 42,
      state: "metallic precipitates", stateSrc: null },
    { id: "lanth", name: "Lanthanides and Zr", members: ["La", "Ce", "Nd", "Zr"], rep: 57,
      state: "dissolved in the UO2 lattice", stateSrc: null },
    { id: "silver", name: "Silver", members: ["Ag"], rep: 47,
      state: "", stateSrc: null }
  ],
  // NUREG-1465 (Soffer et al., 1995), Table 3.8 "Revised Radionuclide
  // Groups", p. 10 of the open-corpus PDF's printed numbering (read
  // 2026-10-20 from theodore-open-corpus/soffer2009nureg1465.pdf). These are
  // LWR source-term groups by release behaviour, not TRISO groups.
  nureg1465: {
    page: 10,
    groups: [
      ["Noble gases", ["Xe", "Kr"]],
      ["Halogens", ["I", "Br"]],
      ["Alkali Metals", ["Cs", "Rb"]],
      ["Tellurium group", ["Te", "Sb", "Se"]],
      ["Barium, strontium", ["Ba", "Sr"]],
      ["Noble Metals", ["Ru", "Rh", "Pd", "Mo", "Tc", "Co"]],
      ["Lanthanides", ["La", "Zr", "Nd", "Eu", "Nb", "Pm", "Pr", "Sm", "Y", "Cm", "Am"]],
      ["Cerium group", ["Ce", "Pu", "Np"]]
    ]
  },
  // SCIANTIX documentation at commit 2344575 (MIT), docs/source/models/,
  // read 2026-10-20. What each file says, in our words; the page cites the
  // file.
  sciantix: {
    gasDiffusion: "gas_diffusion.rst: intra-granular diffusion of fission gases by a spectral (Booth-type) formulation, with trapping and re-solution; the gas reaching the grain boundary is found by mass balance",
    intergranular: "intergranular_bubble_behavior.rst: the evolution of grain-boundary bubbles (concentration, size, fractional coverage), needed for fission gas release and swelling",
    venting: "grain_boundary_venting.rst: venting of gas from the grain boundary to the free volume, modelled by a coverage-driven sigmoid for bubble interconnection or by open-porosity correlations",
    sweeping: "grain_boundary_sweeping.rst: grain growth sweeps the gas in the swept shell to the boundary; in SCIANTIX this is applied to helium only, deliberately not to xenon and krypton",
    grainGrowth: "grain_growth.rst: the grain radius grows with temperature; options Ainscough et al. (1973), with burnup retardation, and Van Uffelen et al. (2013), up to a limiting size",
    hbs: "high_burnup_structure_formation.rst: formation of the high-burnup structure, a restructuring law in effective burnup (Barani et al. 2020 option)",
    stoichiometry: "stoichiometry_deviation.rst: the deviation x in UO2+x under oxidising conditions, and the oxygen potential from it (Blackburn relation)",
    release: "gas_release.rst: released gas = produced - decayed - retained in grain - retained at grain boundaries (mass balance)"
  },
  // Still pending, each shown as "source pending" on its page.
  pending: {
    familyStates: null,          // Kleykamp 1985 (candidate, unread)
    pdAttacksSiC: null,
    csIVapourAlongPores: null,
    lanthanidesStayDissolved: null,
    srBaOxidesRetainedInCarbon: null,
    poreMigrationMechanism: null,  // evaporation-condensation up the gradient
    amoebaEffect: null,            // kernel migration in TRISO; not in the workspace
    knockout: null,
    burstOnHeating: null,
    coFromOxygen: null,            // spare oxygen -> CO in UO2 kernels; why UCO
    conductivityWithBurnup: null,  // tampines implements only fresh UO2
    kernelSwelling: null,
    uo2GrainSizeDistribution: null // TRISO UO2 grain sizes: no cited value in the workspace
  }
};
