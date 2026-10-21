// Neutron scattering lengths for the neutronics maths track, rung 5i (gh:#906).
// Source: NIST Center for Neutron Research, "Neutron scattering lengths and
// cross sections", https://www.ncnr.nist.gov/resources/activation/scattering_table.html,
// accessed 2026-10-16; the table says it "has been compiled from various sources
// ... and does not represent a critical evaluation". The standard compilation
// is Sears, Neutron News 3(3) 26-37 (1992), doi:10.1080/10448639208218770.
// Copied by hand, isotope rows, uncertainties dropped: spin I; coherent length
// bc [fm]; b+ and b- [fm], the lengths for total spin I + 1/2 and I - 1/2
// (for H-1: triplet and singlet); coherent and incoherent cross sections [b].
window.SPIN_DATA = {
  "H-1": { "I": 0.5, "bc": -3.7395, "bplus": 10.817, "bminus": -47.420, "sc": 1.7583, "si": 80.27 },
  "C-12": { "I": 0, "bc": 6.6535, "sc": 5.5590, "si": 0 },
  "O-16": { "I": 0, "bc": 5.805, "sc": 4.232, "si": 0 }
};
