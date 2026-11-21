// Differential harness for punggol::openmc_photon: links libopenmc built from
// the pinned source, loads a photon HDF5 library and (1) dumps the data
// members the three samplers read, (2) runs the samplers with fixed seeds.
#include <cstdio>
#include <cstdint>
#include <vector>
#include <hdf5.h>
#include "openmc/photon.h"
#include "openmc/constants.h"
#include "openmc/random_lcg.h"

using namespace openmc;

static void dump_vec(FILE* f, const char* name, const double* p, size_t n) {
  fprintf(f, "%s %zu\n", name, n);
  for (size_t i = 0; i < n; ++i) fprintf(f, "%.17g\n", p[i]);
}

int main(int argc, char** argv) {
  const char* file = argv[1];
  const char* element = argv[2];
  hid_t fid = H5Fopen(file, H5F_ACC_RDONLY, H5P_DEFAULT);
  hid_t g = H5Gopen(fid, element, H5P_DEFAULT);
  PhotonInteraction p(g);

  FILE* d = fopen(argv[3], "w");
  fprintf(d, "Z %d\n", p.Z_);
  fprintf(d, "ngrid %zu\n", p.energy_.size());
  dump_vec(d, "profile_pz", data::compton_profile_pz.data(), data::compton_profile_pz.size());
  dump_vec(d, "profile_pdf", p.profile_pdf_.data(), p.profile_pdf_.size());
  fprintf(d, "profile_pdf_shape %zu %zu\n", p.profile_pdf_.shape(0), p.profile_pdf_.shape(1));
  dump_vec(d, "profile_cdf", p.profile_cdf_.data(), p.profile_cdf_.size());
  dump_vec(d, "profile_tail_slope", p.profile_tail_slope_.data(), p.profile_tail_slope_.size());
  dump_vec(d, "profile_negative_mass", p.profile_negative_mass_.data(), p.profile_negative_mass_.size());
  dump_vec(d, "binding_energy", p.binding_energy_.data(), p.binding_energy_.size());
  dump_vec(d, "electron_pdf", p.electron_pdf_.data(), p.electron_pdf_.size());
  fprintf(d, "subshell_map %zu\n", p.subshell_map_.size());
  for (size_t i = 0; i < p.subshell_map_.size(); ++i) fprintf(d, "%d\n", p.subshell_map_(i));
  auto dump_tab = [&](const char* name, const Tabulated1D& t) {
    fprintf(d, "%s %zu\n", name, t.x().size());
    for (size_t i = 0; i < t.x().size(); ++i) fprintf(d, "%.17g %.17g\n", t.x()[i], t.y()[i]);
  };
  dump_tab("incoherent_form_factor", p.incoherent_form_factor_);
  dump_tab("coherent_int_form_factor", p.coherent_int_form_factor_);
  // probe values of the two tabulated functions: the Rust interpolation must reproduce them
  fprintf(d, "probes 40\n");
  for (int i = 0; i < 20; ++i) {
    double x = 1.0e-3 * std::pow(1.5, i);
    fprintf(d, "%.17g %.17g %.17g\n", x, p.incoherent_form_factor_(x), p.coherent_int_form_factor_(x));
  }
  fclose(d);

  // samplers
  FILE* o = fopen(argv[4], "w");
  const double energies[] = {1.0e3, 1.0e4, 1.0e5, 5.0e5, 1.0e6, 3.0e6, 1.0e7, 1.0e8};
  for (double e : energies) {
    double alpha = e / MASS_ELECTRON_EV;
    for (int dop = 0; dop < 2; ++dop) {
      for (uint64_t s0 = 1; s0 <= 200; ++s0) {
        uint64_t seed = s0 * 7919ULL;
        uint64_t seed_in = seed;
        double a_out, mu; int ish;
        p.compton_scatter(alpha, dop == 1, &a_out, &mu, &ish, &seed);
        fprintf(o, "compton %.17g %d %llu %.17g %.17g %d %llu\n", alpha, dop, (unsigned long long)seed_in, a_out, mu, ish, (unsigned long long)seed);
      }
    }
    for (uint64_t s0 = 1; s0 <= 200; ++s0) {
      uint64_t seed = s0 * 104729ULL, seed_in = seed;
      double mu = p.rayleigh_scatter(alpha, &seed);
      fprintf(o, "rayleigh %.17g %llu %.17g %llu\n", alpha, (unsigned long long)seed_in, mu, (unsigned long long)seed);
    }
    if (e > 2.0 * MASS_ELECTRON_EV) {
      for (uint64_t s0 = 1; s0 <= 200; ++s0) {
        uint64_t seed = s0 * 1299709ULL, seed_in = seed;
        double ee, ep, mue, mup;
        p.pair_production(alpha, &ee, &ep, &mue, &mup, &seed);
        fprintf(o, "pair %.17g %llu %.17g %.17g %.17g %.17g %llu\n", alpha, (unsigned long long)seed_in, ee, ep, mue, mup, (unsigned long long)seed);
      }
    }
  }
  fclose(o);
  return 0;
}
