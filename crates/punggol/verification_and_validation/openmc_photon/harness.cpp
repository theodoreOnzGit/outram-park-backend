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
#include "openmc/particle.h"
#include "openmc/memory.h"
#include "openmc/settings.h"
#include "openmc/geometry.h"
#include "openmc/material.h"
#include "openmc/physics.h"
#include "openmc/bremsstrahlung.h"
#include "openmc/tensor.h"

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
  data::elements.push_back(make_unique<PhotonInteraction>(g));
  PhotonInteraction& p = *data::elements[0];

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
  dump_vec(d, "energy", p.energy_.data(), p.energy_.size());
  dump_vec(d, "coherent", p.coherent_.data(), p.coherent_.size());
  dump_vec(d, "incoherent", p.incoherent_.data(), p.incoherent_.size());
  dump_vec(d, "pair_production_total", p.pair_production_total_.data(), p.pair_production_total_.size());
  dump_vec(d, "cross_sections", p.cross_sections_.data(), p.cross_sections_.size());
  fprintf(d, "cross_sections_shape %zu %zu\n", p.cross_sections_.shape(0), p.cross_sections_.shape(1));
  dump_tab("incoherent_form_factor", p.incoherent_form_factor_);
  dump_tab("coherent_int_form_factor", p.coherent_int_form_factor_);
  // probe values of the two tabulated functions: the Rust interpolation must reproduce them
  fprintf(d, "probes 40\n");
  for (int i = 0; i < 20; ++i) {
    double x = 1.0e-3 * std::pow(1.5, i);
    fprintf(d, "%.17g %.17g %.17g\n", x, p.incoherent_form_factor_(x), p.coherent_int_form_factor_(x));
  }
  fprintf(d, "has_atomic_relaxation %d\n", p.has_atomic_relaxation_ ? 1 : 0);
  fprintf(d, "max_stack %d\n", p.has_atomic_relaxation_ ? p.calc_max_stack_size() : 0);
  fprintf(d, "shells %zu\n", p.shells_.size());
  for (auto& sh : p.shells_) {
    fprintf(d, "shell %d %d %.17g %zu\n", sh.index_subshell, sh.threshold, sh.binding_energy, sh.transitions.size());
    for (auto& t : sh.transitions) fprintf(d, "%d %d %.17g %.17g\n", t.primary_subshell, t.secondary_subshell, t.energy, t.probability);
  }
  // Synthetic thick-target bremsstrahlung tables (Material::init_bremsstrahlung
  // is NOT exercised here; the sampler is). Smooth, positive, monotone CDFs.
  const int NE = 24;
  data::ttb_e_grid = tensor::zeros<double>({(size_t)NE});
  for (int i = 0; i < NE; ++i) data::ttb_e_grid(i) = std::log(1.0e3 * std::pow(1.0e9 / 1.0e3, i / (NE - 1.0)));
  model::materials.push_back(make_unique<Material>());
  Material& mat = *model::materials[0];
  mat.element_ = {0};
  mat.nuclide_ = {0};
  mat.atom_density_ = tensor::zeros<double>({(size_t)1});
  mat.atom_density_(0) = 0.048;
  mat.ttb_ = make_unique<Bremsstrahlung>();
  for (int part = 0; part < 2; ++part) {
    BremsstrahlungData* t = part == 0 ? &mat.ttb_->electron : &mat.ttb_->positron;
    t->pdf = tensor::zeros<double>({(size_t)NE, (size_t)NE});
    t->cdf = tensor::zeros<double>({(size_t)NE, (size_t)NE});
    t->yield = tensor::zeros<double>({(size_t)NE});
    for (int i = 0; i < NE; ++i) {
      t->yield(i) = std::log(0.004 * (part == 0 ? 1.0 : 0.7) * std::pow(std::exp(data::ttb_e_grid(i)) / 1.0e5, 0.6));
      double acc = 0.0;
      for (int w = 0; w <= i; ++w) {
        double k = std::exp(data::ttb_e_grid(w));
        t->pdf(i, w) = (part == 0 ? 1.0 : 0.8) * (1.0 + 0.3 * w) / (k * (1.0 + 0.1 * i));
        if (w > 0) acc += 0.5 * (t->pdf(i, w) + t->pdf(i, w - 1)) * (std::exp(data::ttb_e_grid(w)) - std::exp(data::ttb_e_grid(w - 1)));
        t->cdf(i, w) = acc;
      }
    }
  }
  dump_vec(d, "ttb_e_grid", data::ttb_e_grid.data(), data::ttb_e_grid.size());
  for (int part = 0; part < 2; ++part) {
    BremsstrahlungData* t = part == 0 ? &mat.ttb_->electron : &mat.ttb_->positron;
    dump_vec(d, part == 0 ? "ttb_e_pdf" : "ttb_p_pdf", t->pdf.data(), t->pdf.size());
    dump_vec(d, part == 0 ? "ttb_e_cdf" : "ttb_p_cdf", t->cdf.data(), t->cdf.size());
    dump_vec(d, part == 0 ? "ttb_e_yield" : "ttb_p_yield", t->yield.data(), t->yield.size());
  }
  fclose(d);

  // samplers
  FILE* o = fopen(argv[4], "w");
  {
    Particle part;
    std::vector<double> es;
    for (int i = 0; i <= 160; ++i) es.push_back(500.0 * std::pow(1.0e9 / 500.0, i / 160.0));
    for (size_t i = 0; i < p.energy_.size(); i += 97) es.push_back(std::exp(p.energy_[i]));
    es.push_back(10.0); es.push_back(1.0e12);
    for (double e : es) {
      part.E() = e;
      p.calculate_xs(part);
      auto& x = part.photon_xs(0);
      fprintf(o, "xs %.17g %d %.17g %.17g %.17g %.17g %.17g %.17g\n", e, x.index_grid, x.interp_factor, x.coherent, x.incoherent, x.photoelectric, x.pair_production, x.total);
    }
  }
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
  // atomic relaxation (local energy deposition for charged secondaries, so only
  // the fluorescent photons are banked)
  settings::electron_treatment = ElectronTreatment::LED;
  model::n_coord_levels = 1;
  const double pes[] = {5.0e4, 1.0e5, 1.0e6};
  for (size_t ish = 0; ish < p.shells_.size(); ++ish) {
    for (double pe : pes) {
      for (uint64_t s0 = 1; s0 <= 40; ++s0) {
        Particle part;
        part.E() = pe;
        part.wgt() = 1.0;
        part.stream() = 0;
        part.seeds(0) = s0 * 15485863ULL;
        uint64_t seed_in = part.seeds(0);
        part.local_secondary_bank().clear();
        p.atomic_relaxation((int)ish, part);
        auto& bank = part.local_secondary_bank();
        fprintf(o, "relax %zu %.17g %llu %llu %zu", ish, pe, (unsigned long long)seed_in, (unsigned long long)part.seeds(0), bank.size());
        for (auto& sb : bank) fprintf(o, " %.17g %.17g %.17g %.17g", sb.E, sb.u.x, sb.u.y, sb.u.z);
        fprintf(o, "\n");
      }
    }
  }
  // sample_photon_reaction, TTB on (synthetic tables) and off
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
        p.calculate_xs(part);
        part.macro_xs().total = mat.atom_density_(0) * part.photon_xs(0).total;
        sample_photon_reaction(part);
        auto& bank = part.local_secondary_bank();
        fprintf(o, "react %d %.17g %llu %d %d %.17g %.17g %.17g %.17g %.17g %.17g %llu %zu", mode, e, (unsigned long long)seed_in, (int)part.event(), part.event_mt(), part.E(), part.wgt(), part.u().x, part.u().y, part.u().z, part.mu(), (unsigned long long)part.seeds(0), bank.size());
        for (auto& sb : bank) fprintf(o, " %d %.17g %.17g %.17g %.17g %.17g", sb.particle.transport_index(), sb.E, sb.wgt, sb.u.x, sb.u.y, sb.u.z);
        fprintf(o, "\n");
      }
    }
  }
  fclose(o);
  return 0;
}
