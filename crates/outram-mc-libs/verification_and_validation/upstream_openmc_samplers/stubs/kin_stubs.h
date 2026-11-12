// STUBS for the collision-kinematics extracts (physics_extract.inc, gh:#1444).
// The extracted routines (elastic_scatter, sample_target_velocity,
// sample_cxs_target_velocity, Nuclide::elastic_xs_0K, rotate_angle and the CM->lab
// block of inelastic_scatter) are OpenMC's text, verbatim. They are written
// against Particle / Nuclide / Reaction / settings, whose real headers pull the
// whole library, so this file declares exactly the members those routines touch:
// nothing else. Defaults of `settings::` are upstream's (src/settings.cpp:125-134).
#pragma once
#include <memory>
#include <vector>

#include "openmc/angle_energy.h"
#include "openmc/constants.h"
#include "openmc/math_functions.h"
#include "openmc/position.h"
#include "openmc/random_dist.h"
#include "openmc/random_lcg.h"
#include "openmc/search.h"
#include "openmc/secondary_uncorrelated.h"

namespace openmc {

namespace settings {
inline ResScatMethod res_scat_method {ResScatMethod::rvs};
inline double res_scat_energy_min {0.01};
inline double res_scat_energy_max {1000.0};
inline double free_gas_threshold {400.0};
} // namespace settings

// Members of Nuclide used by the extracted routines (include/openmc/nuclide.h).
struct Nuclide {
  double awr_ {1.0};
  bool resonant_ {false};
  std::vector<double> energy_0K_;
  std::vector<double> elastic_0K_;
  std::vector<double> xs_cdf_;
  double elastic_xs_0K(double E) const;
  void build_xs_cdf();
};

namespace data {
inline std::vector<std::unique_ptr<Nuclide>> nuclides;
}

struct NeutronMicroXS {
  bool use_ptable {false};
  double elastic {0.0};
};

// The members of Particle that elastic_scatter reads and writes.
struct Particle {
  double E_ {0.0};
  Direction u_ {};
  double mu_ {0.0};
  uint64_t seed_ {0};
  NeutronMicroXS xs_;
  double& E() { return E_; }
  const double& E() const { return E_; }
  Direction& u() { return u_; }
  const Direction& u() const { return u_; }
  double& mu() { return mu_; }
  const double& mu() const { return mu_; }
  uint64_t* current_seed() { return &seed_; }
  const NeutronMicroXS& neutron_xs(int) const { return xs_; }
};

struct ReactionProductStub {
  std::vector<std::unique_ptr<AngleEnergy>> distribution_;
};
struct Reaction {
  std::vector<ReactionProductStub> products_;
  bool scatter_in_cm_ {true};
};

// Forward declarations (physics.cpp declares these in physics.h).
Direction sample_target_velocity(const Nuclide& nuc, double E, Direction u,
  Direction v_neut, double xs_eff, double kT, uint64_t* seed);
Direction sample_cxs_target_velocity(
  double awr, double E, Direction u, double kT, uint64_t* seed);

} // namespace openmc
