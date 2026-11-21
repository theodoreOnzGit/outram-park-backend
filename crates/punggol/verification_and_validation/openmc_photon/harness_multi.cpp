// Differential harness for punggol::openmc_photon, material level (gh:#1690):
// two elements (U and H) in one material. Dumps the thick-target
// bremsstrahlung tables of Material::init_bremsstrahlung for (a) atom
// densities and (b) weight fractions (negative atom_density_ entries, the
// branch upstream's collision_stopping_power / init_bremsstrahlung take), and
// runs sample_photon_reaction with sample_element on the atom-density material.
// usage: harness_multi A.h5 /ZA B.h5 /ZB out.tables out.samples [photon_cutoff]
#include <cstdio>
#include <cstdint>
#include <vector>
#include <hdf5.h>
#include "openmc/photon.h"
#include "openmc/constants.h"
#include "openmc/random_lcg.h"
#include "openmc/particle.h"
#include "openmc/memory.h"
#include "openmc/settings.h"
#include "openmc/geometry.h"
#include "openmc/material.h"
#include "openmc/physics.h"
#include "openmc/bremsstrahlung.h"
#include "openmc/tensor.h"
#include "openmc/nuclide.h"
#include "openmc/hdf5_interface.h"
#include <cstring>
#include <unistd.h>

using namespace openmc;

static void dump_vec(FILE* f, const char* name, const double* p, size_t n) {
  fprintf(f, "%s %zu\n", name, n);
  for (size_t i = 0; i < n; ++i) fprintf(f, "%.17g\n", p[i]);
}

int main(int argc, char** argv) {
  const double cutoff = (argc > 7) ? atof(argv[7]) : 1000.0;
  settings::energy_cutoff[ParticleType::photon().transport_index()] = cutoff;
  for (int k = 0; k < 2; ++k) {
    hid_t fid = H5Fopen(argv[1 + 2 * k], H5F_ACC_RDONLY, H5P_DEFAULT);
    hid_t g = H5Gopen(fid, argv[2 + 2 * k], H5P_DEFAULT);
    data::elements.push_back(make_unique<PhotonInteraction>(g));
  }
  const double awrs[2] = {238.0289, 1.007940};
  static char nbuf[2][sizeof(Nuclide)] __attribute__((aligned(64)));
  for (int k = 0; k < 2; ++k) {
    memset(nbuf[k], 0, sizeof(Nuclide));
    Nuclide* fake = reinterpret_cast<Nuclide*>(nbuf[k]);
    fake->awr_ = awrs[k];
    data::nuclides.push_back(unique_ptr<Nuclide>(fake));
  }
  // material 0: atom densities (atom/b-cm); material 1: weight fractions
  const double dens[2][2] = {{0.0225, 0.0675}, {-0.9, -0.1}};
  const double density_entry[2] = {0.09, 10.0};
  FILE* d = fopen(argv[5], "w");
  for (int m = 0; m < 2; ++m) {
    model::materials.push_back(make_unique<Material>());
    Material& mat = *model::materials[m];
    mat.element_ = {0, 1};
    mat.nuclide_ = {0, 1};
    mat.atom_density_ = tensor::zeros<double>({(size_t)2});
    mat.atom_density_(0) = dens[m][0];
    mat.atom_density_(1) = dens[m][1];
    mat.density_ = density_entry[m];
    mat.init_bremsstrahlung();
  }
  fprintf(d, "grid_lin %zu\n", data::ttb_e_grid.size());
  for (double v : data::ttb_e_grid) fprintf(d, "%.17g\n", v);
  data::ttb_e_grid = tensor::log(data::ttb_e_grid);
  dump_vec(d, "ttb_e_grid", data::ttb_e_grid.data(), data::ttb_e_grid.size());
  for (int m = 0; m < 2; ++m) {
    Material& mat = *model::materials[m];
    fprintf(d, "mat %d %.17g %.17g %.17g %.17g %.17g\n", m, density_entry[m], awrs[0], dens[m][0], awrs[1], dens[m][1]);
    for (int part = 0; part < 2; ++part) {
      BremsstrahlungData* t = part == 0 ? &mat.ttb_->electron : &mat.ttb_->positron;
      const char* pre = part == 0 ? "e" : "p";
      char nm[32];
      snprintf(nm, sizeof nm, "m%d_%s_pdf", m, pre); dump_vec(d, nm, t->pdf.data(), t->pdf.size());
      snprintf(nm, sizeof nm, "m%d_%s_cdf", m, pre); dump_vec(d, nm, t->cdf.data(), t->cdf.size());
      snprintf(nm, sizeof nm, "m%d_%s_yield", m, pre); dump_vec(d, nm, t->yield.data(), t->yield.size());
    }
  }
  fclose(d);

  model::n_coord_levels = 1;
  FILE* o = fopen(argv[6], "w");
  Material& mat = *model::materials[0];
  const double rE[] = {2.0e3, 1.0e4, 5.0e4, 1.0e5, 5.0e5, 1.0e6, 2.0e6, 1.0e7, 1.0e8};
  for (int mode = 0; mode < 2; ++mode) {
    settings::electron_treatment = mode == 0 ? ElectronTreatment::TTB : ElectronTreatment::LED;
    for (double e : rE) {
      for (uint64_t s0 = 1; s0 <= 400; ++s0) {
        Particle part;
        part.E() = e;
        part.wgt() = 1.0;
        part.u() = Direction(0.48, 0.6, 0.64);
        part.material() = 0;
        part.stream() = 0;
        part.seeds(0) = s0 * 49979687ULL;
        uint64_t seed_in = part.seeds(0);
        part.local_secondary_bank().clear();
        double macro = 0.0;
        for (int k = 0; k < 2; ++k) {
          data::elements[k]->calculate_xs(part);
          macro += mat.atom_density_(k) * part.photon_xs(k).total;
        }
        part.macro_xs().total = macro;
        sample_photon_reaction(part);
        auto& bank = part.local_secondary_bank();
        fprintf(o, "mreact %d %.17g %llu %d %d %d %.17g %.17g %.17g %.17g %.17g %.17g %llu %zu", mode, e, (unsigned long long)seed_in, (int)part.event(), part.event_mt(), (int)part.event_nuclide(), part.E(), part.wgt(), part.u().x, part.u().y, part.u().z, part.mu(), (unsigned long long)part.seeds(0), bank.size());
        for (auto& sb : bank) fprintf(o, " %d %.17g %.17g %.17g %.17g %.17g", sb.particle.transport_index(), sb.E, sb.wgt, sb.u.x, sb.u.y, sb.u.z);
        fprintf(o, "\n");
      }
    }
  }
  fclose(o);
  fflush(stdout);
  _exit(0);
}
