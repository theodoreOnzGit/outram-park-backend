// STUBS for the fission-neutron extract (fission_extract.inc, gh:#1444 d):
// declares exactly the members of Particle, SourceSite, Nuclide, Reaction,
// ReactionProduct and settings:: that OpenMC's sample_fission_neutron and
// Nuclide::nu read (src/physics.cpp:1081, src/nuclide.cpp:532). The routines are
// OpenMC's text, verbatim. Not included together with kin_stubs.h.
#pragma once
#include <algorithm>
#include <cmath>
#include <memory>
#include <string>
#include <vector>

#include "openmc/angle_energy.h"
#include "openmc/constants.h"
#include "openmc/endf.h"
#include "openmc/math_functions.h"
#include "openmc/position.h"
#include "openmc/random_lcg.h"

namespace openmc {

namespace settings {
inline bool create_delayed_neutrons {true}; // src/settings.cpp:54
}

struct ParticleType {
  static ParticleType neutron() { return {}; }
  int transport_index() const { return 0; }
  bool is_neutron() const { return true; }
};

struct ReactionProduct {
  enum class EmissionMode { prompt, delayed, total };
  ParticleType particle_;
  EmissionMode emission_mode_ {EmissionMode::prompt};
  std::unique_ptr<Function1D> yield_;
  double decay_rate_ {0.0};
  std::vector<std::unique_ptr<AngleEnergy>> distribution_;
  // ReactionProduct::sample with one distribution (applicability_.size() <= 1):
  // sample_dist returns distribution_.back() without drawing (reaction_product.cpp:111-135).
  void sample(double E_in, double& E_out, double& mu, uint64_t* seed) const
  {
    distribution_.back()->sample(E_in, E_out, mu, seed);
  }
};

struct Reaction {
  std::vector<ReactionProduct> products_;
};

struct Nuclide {
  using EmissionMode = ReactionProduct::EmissionMode;
  std::string name_;
  bool fissionable_ {true};
  int n_precursor_ {0};
  std::unique_ptr<Function1D> total_nu_;
  std::vector<Reaction*> fission_rx_;
  double nu(double E, EmissionMode mode, int group = 0) const;
};

namespace data {
inline std::vector<std::unique_ptr<Nuclide>> nuclides;
inline std::array<double, 4> energy_max {20.0e6, 20.0e6, 100.0e6, 100.0e6};
} // namespace data

struct SourceSite {
  Direction u;
  double E {0.0};
  double time {0.0};
  int delayed_group {0};
};

struct Particle {
  double E_ {0.0};
  Direction u_ {};
  uint64_t seed_ {0};
  double E() const { return E_; }
  Direction u() const { return u_; }
  uint64_t* current_seed() { return &seed_; }
};

void sample_fission_neutron(int i_nuclide, const Reaction& rx, SourceSite* site, Particle& p);

} // namespace openmc
