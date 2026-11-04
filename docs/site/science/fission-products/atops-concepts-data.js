// The TRISO-ATOPS concept-to-code map's cards (gh:#1207 part 2).
// HAND-WRITTEN, and checked so it cannot rot: `node check-concepts.js` fails
// if a named port file or function, a test function, a lesson page or a demo
// rung or query key is missing, if a port file's provenance header no longer
// names the upstream Python file (or, for an extension, no longer says NOT a
// port), or if a quoted code-to-code row is not in the record it cites.
//
// Fields, per card:
//   id, stage (fuel | graphite | coolant | accident | extension), title, idea,
//   eq       the equation as the code computes it (plain text, monospace)
//   upstream {file, fn} at TRISO-ATOPS de374c8 (null for an extension)
//   port     [{file, fn}] repo paths; the first is the entry point
//   tests    [{file, fn}]
//   c2c      [{group, cases, dev}] rows of RECORD (below), quoted as printed
//   c2cNote  extra recorded context (with file and date)
//   lessons  [file] relative to this folder
//   demo     a query for ../../demos/triso-atops/ (null: not in the demo), demoNote
window.ATOPS_CONCEPTS = {
  RECORD: {
    file: "crates/boon-lay/docs/triso-atops-code-to-code.md",
    passes: "first pass 2026-09-15 (5 699 cases); second and third passes 2026-09-21 (14 130 cases), upstream de374c8"
  },
  // Lessons being written in parallel (gh:#1207). check-concepts.js accepts
  // a lesson that exists OR is listed here; once they land, clear this list
  // and the check requires every one to exist (and its title to match).
  // Cleared 2026-11-04: all five landed, so every lesson must now exist and
  // its title must match.
  PENDING_LESSONS: [],
  // The link text for each lesson; check-concepts.js fails if a title here
  // differs from the page's own <title> (for a page that exists).
  LESSON_TITLES: {
    "atops-big-picture.html": "TRISO-ATOPS: the big picture",
    "atops-groups.html": "84 nuclides, five groups",
    "atops-diffusion.html": "One diffusion law per group",
    "atops-release-models.html": "One release model per group",
    "atops-normal-operation.html": "Normal operation, node by node",
    "atops-coolant.html": "Into the coolant",
    "atops-accident.html": "Accidents: Booth on the integral, and venting",
    "kernel-rb.html": "The R/B fingerprint",
    "kernel-booth-grain.html": "Booth on one grain",
    "kernel-many-spheres.html": "Many Booth spheres",
    "barriers-sic.html": "Why SiC matters most",
    "failure-release.html": "From failed particles to release",
    "decay-clock.html": "An atom's clock",
    "walk-on-spheres.html": "Walking on spheres",
    "chem-air.html": "Air on hot graphite",
    "chem-steam.html": "Steam on graphite and kernels",
    "atops-source-term.html": "Into the source term",
    "diffusion-three-ways.html": "Diffusion, three ways",
    "atops-conservative.html": "Is ATOPS conservative?",
    "atops-breathing.html": "Breathing"
  },
  STAGES: [
    ["fuel", "Fuel: kernel to particle surface"],
    ["graphite", "Graphite: the fuel element's matrix"],
    ["coolant", "Coolant: the primary circuit's pools"],
    ["accident", "Accident: heat-up and venting"],
    ["extension", "Extensions: NOT a port"]
  ],
  CARDS: [
    {
      id: "nuclide-groups", stage: "fuel", title: "84 nuclides, five transport groups",
      idea: "Every nuclide is put in one of five groups by its atomic number alone, and the group picks every model downstream: noble gas (Kr, Xe), halogen (I, Br, and Te counted with them), special metal (Cs, Rb, Sr), silver (Ag, and Pd with it), and other. Each nuclide's decay constant comes from its tabulated half-life.",
      eq: "group = from_atomic_number(Z)\nλ = ln 2 / t½",
      upstream: { file: "calculation_functions.py", fn: "class Nuclide; nuclides; noble_gases / halogens / special_metals" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs", fn: "from_atomic_number" },
             { file: "crates/boon-lay/src/triso_atops_fork/nuclide_model/nuclide_database.rs", fn: "supported_nuclides" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "every_nuclide_decay_constant_matches_upstream" }],
      c2c: [{ group: "nuclide decay constants (all 84)", cases: "84", dev: "0 (exact)" }],
      lessons: ["atops-groups.html", "decay-clock.html"],
      demo: "rung=release&pick=all", demoNote: "all HTR-10 nuclides, coloured by group"
    },
    {
      id: "diffusion", stage: "fuel", title: "Diffusion coefficients, one law per group",
      idea: "D(T) is an Arrhenius law per group and per material (kernel, matrix graphite), some with a low- and a high-temperature term summed, and each clamped at the bottom of its range. Silver has its own law for intact SiC. A group with no law gets a fixed nominal D.",
      eq: "D(T) = Σ D0·exp(−Q / (R·T_K))   (T clamped per group)\nD_SiC,Ag = 3.6e-9·exp(−215 kJ/mol / (R·T))\nD' = D / a²",
      upstream: { file: "calculation_functions.py", fn: "diffusion_coefficient, diffusion_coefficient_SiC_Ag" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs", fn: "diffusion_coefficient" },
             { file: "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs", fn: "diffusion_coefficient_sic_ag" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "diffusion_coefficient_kernel_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "diffusion_coefficient_graphite_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "diffusion_coefficient_sic_ag_matches_upstream" }],
      c2c: [{ group: "diffusion_coefficient.kernel", cases: "516", dev: "1.40e-16" },
            { group: "diffusion_coefficient.graphite", cases: "516", dev: "1.55e-16" },
            { group: "diffusion_coefficient_sic_ag", cases: "20", dev: "2.22e-16" }],
      c2cNote: "The SiC-silver group was widened to 102 cases (25 K steps over 250-2500 °C) in the third pass, 2026-09-21.",
      lessons: ["atops-diffusion.html", "walk-on-spheres.html"],
      demo: "rung=release&pick=metal&hold=1600", demoNote: "the metals' release as the hold temperature moves D"
    },
    {
      id: "rb-noble-gas", stage: "fuel", title: "<R/B>_fail for noble gases and halogens: an empirical fit",
      idea: "The volatiles' release-to-birth ratio from a failed particle is not computed from diffusion: it is a fit in the decay constant and the temperature, with krypton's coefficients for Kr and xenon's for Xe and the halogens.",
      eq: "<R/B> = exp( n·ln(1/λ) + B/T_K + C )   (n, B, C: Kr fit if Z = 36, else Xe fit)",
      upstream: { file: "calculation_functions.py", fn: "RB_fail_Noble_Gases" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs", fn: "rb_fail_noble_gases" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "rb_fail_noble_gases_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "noble_gases_upstream_cannot_evaluate_use_the_xenon_fit" }],
      c2c: [{ group: "rb_fail_noble_gases", cases: "270", dev: "1.75e-16" }],
      c2cNote: "Deliberate divergence 1 in the record: for He, Ne, Ar and Rn upstream cannot evaluate the fit; the port uses the xenon fit.",
      lessons: ["atops-release-models.html", "kernel-rb.html"],
      demo: "rung=release&pick=noble", demoNote: "the noble gases alone"
    },
    {
      id: "booth-short", stage: "fuel", title: "Booth, short-lived special metals",
      idea: "A short-lived Cs, Rb or Sr nuclide decays before it can diffuse far, so the steady Booth sphere with decay applies. The sphere is the Booth equivalent radius, built from the grain size and the kernel radius.",
      eq: "a = √(2·a_grain·r)\nx = √(λ·a² / D)\n<R/B> = (3/x)·( coth x − 1/x )",
      upstream: { file: "calculation_functions.py", fn: "booth_shortlived_fastdiffuse" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs", fn: "booth_shortlived_fast_diffuse" },
             { file: "crates/boon-lay/src/triso_atops_fork/release_models/mod.rs", fn: "rb_fail" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "booth_shortlived_matches_upstream" }],
      c2c: [{ group: "booth_shortlived_fast_diffuse", cases: "75", dev: "6.74e-13" }],
      lessons: ["atops-release-models.html", "kernel-booth-grain.html"],
      demo: "rung=release&pick=metal", demoNote: "Cs and Sr together"
    },
    {
      id: "booth-long", stage: "fuel", title: "Booth, long-lived special metals",
      idea: "A long-lived Cs or Sr nuclide is released by Booth's series without decay: the fraction that has diffused out of the equivalent sphere after the irradiation time. The series stops before i = 5000 (upstream num_terms=5000), which leaves a floor near 1.2e-4 (upstream defect 10).",
      eq: "D' = D / a²\nRF = 1 − (6/π²)·Σ_{i=1}^{4999} (1/i²)·exp(−(iπ)²·D'·t)",
      upstream: { file: "calculation_functions.py", fn: "booth_longlived" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs", fn: "booth_longlived" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "booth_longlived_matches_upstream" }],
      c2c: [{ group: "booth_longlived", cases: "60", dev: "7.16e-12" }],
      lessons: ["atops-release-models.html", "kernel-many-spheres.html"],
      demo: "rung=release&pick=Cs-137&leak=off", demoNote: "Cs-137 alone"
    },
    {
      id: "breakthrough-ag", stage: "fuel", title: "Silver: breakthrough through intact SiC",
      idea: "Silver is the one group released from intact particles: it diffuses through the SiC layer, a membrane with a time lag. Before the lag nothing comes through; after it, the release grows as steady permeation. The result is scaled by √(λ_Ag-110m/λ).",
      eq: "D' = D_SiC,Ag / a_SiC²\nRF = 3·D·t/(r·a) − a/(2r) − (6a/r)·Σ_{n=1}^{999} (−1)ⁿ/(nπ)²·exp(−(nπ)²·D'·t)\n<R/B> = RF·√(λ_Ag-110m / λ)",
      upstream: { file: "calculation_functions.py", fn: "breakthrough_model" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs", fn: "breakthrough_model" },
             { file: "crates/boon-lay/src/triso_atops_fork/release_models/mod.rs", fn: "rb_fail" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "breakthrough_model_matches_upstream" }],
      c2c: [{ group: "breakthrough_model", cases: "120", dev: "0 (exact)" }],
      c2cNote: "27 of the 120 cases have a widened tolerance (ill-conditioned inputs, analysed in the record).",
      lessons: ["atops-release-models.html", "barriers-sic.html"],
      demo: "rung=release&pick=silver", demoNote: "silver: zero at normal operation, rising in the hold"
    },
    {
      id: "rb-other", stage: "fuel", title: "<R/B>_fail for the other metals: a fixed number",
      idea: "Everything outside the four named groups (Zr, Nb, Mo, Ru, the lanthanides…) gets one fixed release-to-birth ratio, with no temperature or time dependence. The dispatcher rb_fail is where the group picks the model.",
      eq: "<R/B> = OTHER_METAL_RB_FAIL (1e-5)\nrb_fail: NobleGas|Halogen → fit; SpecialMetal → Booth short/long; Silver → breakthrough; Other → fixed",
      upstream: { file: "calculation_functions.py", fn: "R_B_fail" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/mod.rs", fn: "rb_fail" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "rb_fail_dispatcher_matches_upstream" }],
      c2c: [{ group: "rb_fail (dispatcher)", cases: "468", dev: "6.48e-11" }],
      lessons: ["atops-release-models.html", "atops-groups.html"],
      demo: "rung=release&pick=other", demoNote: "the other metals"
    },
    {
      id: "release-rate", stage: "fuel", title: "Release rate from the fuel",
      idea: "The release rate R is the ratio times the birth rate, weighted by the failed fractions the group can leave by: volatiles only from exposed heavy metal and in-service failures, silver from every particle (its model already includes the SiC), the other metals from all four fractions. The birth rate is the inventory activity for a short-lived nuclide and A/(1 − e^{−λt}) for a long-lived one.",
      eq: "R = f·<R/B>_fail·B\nf = f_hm + f_inc (volatiles) | 1 (Ag, Pd) | f_hm + f_SiC + f_inc + f_inc,SiC (others)\nB = A (short-lived) | A / (1 − e^{−λt}) (long-lived)",
      upstream: { file: "calculation_functions.py", fn: "release_rate" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs", fn: "release_rate" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "release_rate_matches_upstream" }],
      c2c: [{ group: "release_rate", cases: "180", dev: "0 (exact)" }],
      lessons: ["atops-normal-operation.html", "failure-release.html"],
      demo: "rung=release&pick=all&now=0", demoNote: "the source at the start of the hold"
    },
    {
      id: "graphite-attenuation", stage: "graphite", title: "Graphite attenuation factor",
      idea: "Metals that leave the particles still have to cross the fuel element's matrix graphite. A slab-diffusion series gives an attenuation factor Af: what reaches the helium is R/Af. Volatiles pass straight through (Af = 1); the other metals take a fixed Af = 1e8; a series that has not started is capped at 1e8.",
      eq: "S_ser = Σ_{i odd < 500} (4/(iπ))·sin(iπ/2)·exp(−(iπ)²·D_graph·t / (4·a_graph²))\nAf = 1 / (1 − S_ser)   (1e8 if S_ser = 1, Af > 1e8 or Af < 0)",
      upstream: { file: "calculation_functions.py", fn: "attenuation_factor" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs", fn: "attenuation_factor" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "attenuation_factor_matches_upstream" }],
      c2c: [{ group: "attenuation_factor", cases: "60", dev: "4.16e-10" }],
      lessons: ["atops-release-models.html", "atops-coolant.html"],
      demo: "rung=release&pick=metal", demoNote: "G, the graphite box, holds the metals"
    },
    {
      id: "graphite-holdup", stage: "graphite", title: "Source rate to the coolant, and graphite hold-up",
      idea: "base_activities splits R into the coolant source S = R/Af and the inventory the graphite holds, G. Volatiles put everything into S and nothing into G.",
      eq: "S = R / Af\nG = R·(1 − 1/Af)·(1 − e^{−λt}) / λ\n(noble gas, halogen: S = R, G = 0)",
      upstream: { file: "calculation_functions.py", fn: "base_activities" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs", fn: "base_activities" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "base_activities_source_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "base_activities_graphite_matches_upstream" }],
      c2c: [{ group: "base_activities.source", cases: "216", dev: "1.83e-12" },
            { group: "base_activities.graphite", cases: "216", dev: "1.65e-16" }],
      lessons: ["atops-coolant.html", "atops-normal-operation.html"],
      demo: "rung=release&pick=Cs-137&leak=off", demoNote: "S and G for Cs-137"
    },
    {
      id: "circulating", stage: "coolant", title: "The circulating pool",
      idea: "The helium carries a pool C, fed by S (and a parent's decay), and emptied by decay, plate-out and clean-up together. Normal operation runs it from empty over the run time.",
      eq: "β = λ + k_plate + k_clean\nC = S·(1 − e^{−βt}) / β + λ·C_parent / β\n(steady: C = (S + λ·C_parent) / β)",
      upstream: { file: "calculation_functions.py", fn: "circulating, circulating_steadystate" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs", fn: "circulating" },
             { file: "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs", fn: "circulating_steadystate" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "circulating_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "circulating_steadystate_matches_upstream" }],
      c2c: [{ group: "circulating", cases: "432", dev: "1.12e-14" },
            { group: "circulating_steadystate", cases: "144", dev: "0 (exact)" }],
      lessons: ["atops-coolant.html"],
      demo: "rung=release&pick=all&leak=off", demoNote: "the helium box"
    },
    {
      id: "plate-out", stage: "coolant", title: "The plate-out pool",
      idea: "What sticks to the circuit's cool walls: fed by k_plate·C, lost only to decay. Noble gases never plate out.",
      eq: "P = k_plate / (β − λ)·( S/λ·(1 − e^{−λt}) − C ) + P_parent\n(steady: P = k_plate·S / (λβ) + P_parent)",
      upstream: { file: "calculation_functions.py", fn: "plate_out, plate_out_steadystate" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs", fn: "plate_out" },
             { file: "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs", fn: "plate_out_steadystate" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "plate_out_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "plate_out_steadystate_matches_upstream" }],
      c2c: [{ group: "plate_out", cases: "432", dev: "1.15e-14" },
            { group: "plate_out_steadystate", cases: "144", dev: "0 (exact)" }],
      lessons: ["atops-coolant.html"],
      demo: "rung=release&pick=metal&leak=off", demoNote: "the plated box, for the metals"
    },
    {
      id: "clean-up", stage: "coolant", title: "The clean-up (purification) pool",
      idea: "What the helium purification system takes out: fed by k_clean·C, lost only to decay. It only exists when the system is modelled, and only for noble gases and halogens.",
      eq: "HPS = k_clean / (β − λ)·( S/λ·(1 − e^{−λt}) − C ) + HPS_parent\n(steady: HPS = k_clean·S / (λβ))",
      upstream: { file: "calculation_functions.py", fn: "clean_up, clean_up_steadystate" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs", fn: "clean_up" },
             { file: "crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs", fn: "clean_up_steadystate" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "clean_up_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "clean_up_without_guard_is_nan_where_upstream_raises" }],
      c2c: [{ group: "clean_up", cases: "360", dev: "1.27e-14" },
            { group: "clean_up_steadystate", cases: "144", dev: "0 (exact)" }],
      c2cNote: "Deliberate divergence 2 in the record: with k_plate = k_clean = 0 upstream raises; the port's guarded form returns the limit.",
      lessons: ["atops-coolant.html"],
      demo: "rung=release&pick=halogen&leak=off", demoNote: "the purification box, for the halogens"
    },
    {
      id: "routing", stage: "coolant", title: "Routing the removal rates by group",
      idea: "Which pools a group can reach is decided in one match: noble gases do not plate out, halogens plate out and are cleaned, metals plate out and are never cleaned. With the purification system off, k_clean is zero for all. This uniform routing is what the demo made visible (gh:#583).",
      eq: "NobleGas: (k_plate, k_clean) = (0, k_clean), P = 0\nHalogen:  (k_plate, k_clean)\nmetals:   (k_plate, 0)\nno HPS:   k_clean = 0",
      upstream: { file: "trisoatops.py", fn: "normal_operation; calculation_functions.py::higher_activities" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs", fn: "normal_operation_node" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "node_plate_out_activity_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "node_clean_up_activity_matches_upstream" }],
      c2c: [{ group: "node.plate_out_activity", cases: "144", dev: "3.10e-11" },
            { group: "node.clean_up_activity", cases: "72", dev: "1.93e-16" }],
      c2cNote: "Mutation 'noble-gas k_plate zeroing removed' is caught by node.circulating_activity and node.clean_up_activity (record, non-vacuity).",
      lessons: ["atops-coolant.html", "atops-big-picture.html"],
      demo: "rung=release&pick=all&leak=off", demoNote: "dashed arrows: the zeros of the routing"
    },
    {
      id: "normal-operation-node", stage: "coolant", title: "Normal operation, one node",
      idea: "One call chains everything for one nuclide at one node: D at the node's temperatures, <R/B>_fail, R, the split into S and G, the routing, and the three pools. The core is many nodes; the source term sums them.",
      eq: "D → <R/B>_fail → R → (S, G) → route (k_plate, k_clean) → C, P, HPS\nCi = atoms·λ / 3.7e10",
      upstream: { file: "trisoatops.py", fn: "normal_operation" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs", fn: "normal_operation_node" },
             { file: "crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs", fn: "to_curies" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "node_release_rate_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "node_circulating_activity_matches_upstream" }],
      c2c: [{ group: "node.release_rate", cases: "144", dev: "3.10e-11" },
            { group: "node.circulating_activity", cases: "144", dev: "3.10e-11" }],
      lessons: ["atops-normal-operation.html", "atops-big-picture.html"],
      demo: "rung=release&pick=all&now=0&leak=off", demoNote: "every nuclide at the start of the hold"
    },
    {
      id: "integral-d", stage: "accident", title: "∫D dt along the transient",
      idea: "When the temperature changes, the release models' D·t is replaced by the time integral of D, summed by trapezoids from D = 0 at the first sample. The kernel and the graphite each get one; for silver (Z = 47) the kernel integral uses the SiC law.",
      eq: "∫D dt ≈ Σ_k ½·(D(T_k) + D(T_{k−1}))·(t_k − t_{k−1}),  starting from 0",
      upstream: { file: "calculation_functions.py", fn: "integrate" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs", fn: "integrate_diffusion_over_time" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "integrate_kernel_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "integrate_graphite_matches_upstream" }],
      c2c: [{ group: "integrate.kernel", cases: "60", dev: "0 (exact)" },
            { group: "integrate.graphite", cases: "60", dev: "0 (exact)" }],
      lessons: ["atops-accident.html", "diffusion-three-ways.html"],
      demo: "rung=failure&hold=1600&hours=300", demoNote: "a hold that drives the integral"
    },
    {
      id: "booth-transient", stage: "accident", title: "Booth on the integral (accident)",
      idea: "The kernel's release in an accident is Booth's series with D'·t replaced by ∫D' dt, on the kernel radius (not normal operation's smaller sphere). The 1e-6 snap-to-zero guard can never fire: the 5000-term floor is 1.2e-4 (upstream defect 10).",
      eq: "RF = 1 − 6·Σ_{i=1}^{4999} (1/(iπ)²)·exp(−(iπ)²·∫D' dt)   (0 if ∫D' dt = 0; RF < 1e-6 → 0)",
      upstream: { file: "calculation_functions.py", fn: "booth_transient" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs", fn: "booth_transient" },
             { file: "crates/boon-lay/src/triso_atops_fork/release_models/mod.rs", fn: "release_fraction_transient" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "booth_transient_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "booth_transient_zero_floor_is_unreachable" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "release_fraction_kernel_matches_upstream" }],
      c2c: [{ group: "booth_transient", cases: "8", dev: "3.62e-11" },
            { group: "release_fraction.kernel", cases: "120", dev: "3.32e-4" }],
      c2cNote: "booth_transient widened to 53 cases in the third pass (2026-09-21), which exposed defect 10. release_fraction.kernel's 3.32e-4 is on 16 ill-conditioned cases with widened tolerance (record, 'Ill-conditioned inputs').",
      lessons: ["atops-accident.html", "failure-release.html", "atops-conservative.html"],
      demo: "rung=release&pick=Cs-137&hold=1800&now=500&leak=off", demoNote: "Cs-137 at the end of a hot hold"
    },
    {
      id: "breakthrough-transient", stage: "accident", title: "Silver breakthrough on the integral (accident)",
      idea: "Silver's membrane model with D·t replaced by ∫D dt through the SiC: the time lag is now crossed by heating, not by years at temperature.",
      eq: "RF = 3·(∫D dt)/(a·r) − a/(2r) − (6a/r)·Σ_{n=1}^{999} (−1)ⁿ/(nπ)²·exp(−(nπ)²·∫D' dt)",
      upstream: { file: "calculation_functions.py", fn: "breakthrough_model_transient" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs", fn: "breakthrough_model_transient" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "breakthrough_model_transient_matches_upstream" }],
      c2c: [{ group: "breakthrough_model_transient", cases: "64", dev: "0 (exact)" }],
      lessons: ["atops-accident.html", "barriers-sic.html", "atops-conservative.html"],
      demo: "rung=release&pick=silver&hold=1600&now=200", demoNote: "silver during a hold"
    },
    {
      id: "graphite-rf", stage: "accident", title: "Graphite release fraction (accident)",
      idea: "What the matrix graphite gives up during the transient: a slab series on its own ∫D dt. Noble gases and halogens are given nothing here (they were never held).",
      eq: "RF = Σ_{i odd < 5000} (8/(iπ)²)·( 1 − exp(−(iπ)²·(∫D dt) / (4·a_graph²)) )",
      upstream: { file: "calculation_functions.py", fn: "RF_Graph" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/release_models/transient.rs", fn: "rf_graph" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "rf_graph_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "release_fraction_graphite_matches_upstream" }],
      c2c: [{ group: "rf_graph", cases: "18", dev: "4.05e-15" },
            { group: "release_fraction.graphite", cases: "120", dev: "2.01e-12" }],
      c2cNote: "rf_graph widened to 135 cases (both arguments swept) in the third pass, 2026-09-21.",
      lessons: ["atops-accident.html"],
      demo: "rung=release&pick=metal&hold=1800&now=500", demoNote: "the graphite box during a hot hold"
    },
    {
      id: "release-activity", stage: "accident", title: "What is left to release (mass balance)",
      idea: "The atoms available in an accident: the node's inventory scaled by the group's accident failure fraction, minus what normal operation already moved into graphite, circulating, plate-out (and clean-up if fitted), times the transient release fraction. The silver branch reads z == 47 or 48 upstream (defect 3); the port uses Ag and Pd unless asked for the typo.",
      eq: "kernel:   avail = RF·( N·f_group − G − C − P − [HPS] )   if N·Σf_normal − P > 0\n          avail = RF·( N·Σf_accident − G − C − [HPS] )      otherwise\n          f_group = volatile sum (noble gas, halogen) | 1 (Ag, Pd) | all six (others)\ngraphite: avail = RF·G",
      upstream: { file: "calculation_functions.py", fn: "release_activity" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/accident/mod.rs", fn: "release_activity" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "release_activity_kernel_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "release_activity_graphite_matches_upstream" }],
      c2c: [{ group: "release_activity.kernel", cases: "3 024", dev: "0 (bit-exact)" },
            { group: "release_activity.graphite", cases: "3 024", dev: "0 (bit-exact)" }],
      lessons: ["atops-accident.html", "atops-source-term.html"],
      demo: "rung=source-term", demoNote: "the recorded HTR-10 accident release"
    },
    {
      id: "venting", stage: "accident", title: "Venting: the coolant pushed out while the core heats",
      idea: "The release leaves the vessel only with gas pushed out by heating, by the ideal gas law at fixed volume; when the core cools the gas contracts and the release stops. The first venting sample is forced to 1, and the trapezoid sum drops half of the first step.",
      eq: "dn/dt = −(P/R)·(dT/dt) / T²\nfrac(t) = |∫ dn/dt dt / n0|   over samples with mean dT/dt ≥ 0; frac[0] = 1",
      upstream: { file: "calculation_functions.py", fn: "coolant_release" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/accident/mod.rs", fn: "coolant_release" },
             { file: "crates/boon-lay/src/triso_atops_fork/accident/mod.rs", fn: "mean_temperature_rate" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "coolant_release_fraction_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "coolant_release_vent_times_match_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "coolant_mean_temperature_rate_matches_upstream" }],
      c2c: [{ group: "coolant_release.fraction", cases: "10", dev: "4.69e-16" },
            { group: "coolant_release.vent_time", cases: "10", dev: "0 (bit-exact)" },
            { group: "coolant_release.mean_dtdt", cases: "12", dev: "1.12e-15" }],
      c2cNote: "Widened to 331 coolant_release cases (1-6 nodes, every hot node, three pressures) in the third pass, 2026-09-21.",
      lessons: ["atops-accident.html", "atops-breathing.html", "atops-source-term.html"],
      demo: null, demoNote: "not in the demo: its release rung holds the temperature, so nothing vents (the lesson's widget shows it)"
    },
    {
      id: "lift-off", stage: "accident", title: "Lift-off: a share of the plate-out",
      idea: "The blowdown re-entrains a fraction x_liftoff of the plated-out activity. It is added once, not scaled by the venting fraction. sembawang's HTR-10 case uses 0.05, an NP-MHTGR stand-in (sembawang/src/htr10.rs, X_LIFTOFF_NP_MHTGR_STAND_IN).",
      eq: "circuit = C + x_liftoff·P",
      upstream: { file: "trisoatops.py", fn: "accident_case" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/accident/mod.rs", fn: "accident_release_curies" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "accident_case_total_matches_upstream" }],
      c2c: [{ group: "accident_case.total", cases: "54", dev: "3.63e-15" }],
      c2cNote: "Mutation 'accident_release_curies scales the circuit term by frac' is killed by accident_case_total_matches_upstream (record, third pass).",
      lessons: ["atops-source-term.html", "atops-accident.html"],
      demo: "rung=release&pick=metal&now=0&leak=off", demoNote: "the plated pool that lift-off draws on"
    },
    {
      id: "accident-total", stage: "accident", title: "The accident total, pool by pool",
      idea: "Released curies at each sample: the vented fraction of what diffuses out of kernels and graphite, plus the whole circulating pool, plus x_liftoff of the plate-out. The clean-up pool never leaves. There is no single accident_case function in the port: callers compose these pieces (gh:#447).",
      eq: "A_rel(t) = frac(t)·( K(t) + Gr(t) ) + C + x_liftoff·P     (HPS: never)\nK, Gr = release_activity·λ / 3.7e10",
      upstream: { file: "trisoatops.py", fn: "accident_case" },
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/accident/mod.rs", fn: "accident_release_curies" },
             { file: "crates/boon-lay/src/triso_atops_fork/accident/mod.rs", fn: "atoms_to_curies" }],
      tests: [{ file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "accident_case_total_matches_upstream" },
              { file: "crates/boon-lay/tests/triso_atops_code_to_code.rs", fn: "accident_case_nodal_kernel_matches_upstream" }],
      c2c: [{ group: "accident_case.total", cases: "54", dev: "3.63e-15" },
            { group: "accident_case.nodal_kernel", cases: "180", dev: "2.44e-9" }],
      lessons: ["atops-source-term.html", "atops-breathing.html", "chem-air.html", "chem-steam.html"],
      demo: "rung=source-term", demoNote: "the recorded HTR-10 release (sembawang)"
    },
    {
      id: "live-pools", stage: "extension", title: "live_pools: the pools stepped from any state (NOT a port)",
      idea: "boon-lay's own exact stepping of the circulating, plate-out and clean-up balances from any starting state, with a leak out of the circuit, so a transient can carry them forward. From empty with no leak it reproduces the ported closed forms.",
      eq: "dC/dt = S − (λ + k_plate + k_clean + k_leak)·C\ndP/dt = k_plate·C − λP,  dH/dt = k_clean·C − λH   (exact for S constant over a step)",
      upstream: null,
      port: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs", fn: "step" }],
      tests: [{ file: "crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs", fn: "one_step_from_empty_reproduces_the_ported_closed_forms" },
              { file: "crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs", fn: "atoms_are_conserved_and_small_steps_equal_one_big_step" }],
      c2c: [],
      c2cNote: "Not code-to-code (there is no upstream). Against the ported closed forms from empty: worst relative difference 2.6e-16 (the test's doc comment, 2026-09-29).",
      lessons: ["atops-coolant.html", "atops-breathing.html"],
      demo: "rung=release&pick=Cs-137&leak=on", demoNote: "the demo carries the pools through the hold with live_pools, leak on"
    },
    {
      id: "multilayer-diffusion", stage: "extension", title: "Multilayer diffusion on a mesh (NOT a port)",
      idea: "boon-lay's own finite-volume solution of the diffusion equation through every layer of a particle (gh:#1209): the reference TRISO-ATOPS's analytical formulas are measured against. Booth on the bare kernel is reproduced to the mesh error; intact particles are not zero for Cs at 1600 °C; silver's breakthrough is conservative.",
      eq: "V_i (c_i' − c_i)/Δt = −Σ_j G_ij (c_i' − c_j') − λ V_i c_i'\nG = A / (Δr₁/2D₁ + Δr₂/2D₂)   (backward Euler, c = 0 at the surface)",
      upstream: null,
      port: [{ file: "crates/boon-lay/src/triso_atops_extensions/multilayer_diffusion.rs", fn: "step" },
             { file: "crates/boon-lay/src/triso_atops_extensions/multilayer_diffusion.rs", fn: "mean_exit_time" },
             { file: "crates/boon-lay/src/triso_atops_extensions/multilayer_diffusion.rs", fn: "steady_release_to_birth" }],
      tests: [{ file: "crates/boon-lay/src/triso_atops_extensions/multilayer_diffusion.rs", fn: "one_sphere_against_crank" },
              { file: "crates/boon-lay/src/triso_atops_extensions/multilayer_diffusion.rs", fn: "mean_exit_time_against_the_layered_formula" },
              { file: "crates/boon-lay/src/triso_atops_extensions/multilayer_diffusion.rs", fn: "steady_release_to_birth_against_booth" }],
      c2c: [],
      c2cNote: "Not code-to-code (no upstream). Verified 2026-11-04 against Crank's series (7.9e-5), the exact layered mean (1.4e-4) and Booth's R/B (6.8e-7); conservation missed its 1e-12 prediction (6.6e-12, round-off).",
      lessons: ["diffusion-three-ways.html", "atops-conservative.html"],
      demo: null, demoNote: "not in the demo"
    },
    {
      id: "removal-rates", stage: "extension", title: "RemovalRates: plate-out and clean-up per group or element (NOT a port)",
      idea: "boon-lay's extension that lets k_plate and k_clean differ per transport group or per element, e.g. Liu & Cao 2002's HTR-10 per-cycle figures, instead of upstream's one rate for everything. Under RemovalRates::upstream it reduces to the port bit for bit.",
      eq: "(k_plate, k_clean) = rates.for_element(Z)   (element overrides group)\nk = −ln(1 − fraction) / cycle_time",
      upstream: null,
      port: [{ file: "crates/boon-lay/src/triso_atops_extensions/removal_rates.rs", fn: "upstream" },
             { file: "crates/boon-lay/src/triso_atops_extensions/normal_operation.rs", fn: "normal_operation_node_with_rates" }],
      tests: [{ file: "crates/boon-lay/src/triso_atops_extensions/normal_operation.rs", fn: "upstream_rates_reproduce_the_port_bit_for_bit" },
              { file: "crates/boon-lay/src/triso_atops_extensions/removal_rates.rs", fn: "upstream_routes_as_trisoatops" }],
      c2c: [],
      c2cNote: "Not code-to-code. Reduces to normal_operation_node bit for bit under RemovalRates::upstream (336 cases, crates/boon-lay/CLAUDE.md module map, 2026-10-05).",
      lessons: ["atops-coolant.html"],
      demo: null, demoNote: "not in the demo: its release rung uses upstream's routing with two sliders"
    }
  ]
};
