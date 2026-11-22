// Driver for OpenMC scatter()'s reaction walk. The walk itself is
// walk_extract.inc = physics.cpp lines 732-775, verbatim (see run.sh).
// Mock types give it exactly the names it reads; the three collision routines
// record which branch ran. Each case is a nuclide with an elastic xs, an optional
// S(a,b) elastic part, and a list of inelastic-class reactions in MT order.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>
#include "openmc/random_lcg.h"
using namespace openmc;

struct Micro { double total, absorption, elastic, thermal; int index_sab; };
struct Rx {
  int mt_; double x;
  double xs(const Micro&) const { return x; }
};
struct Nuc {
  std::vector<Rx*> reactions_;            // [0] = elastic, then the inelastic class
  std::vector<int> index_inelastic_scatter_;
  bool multipole_ = false; std::vector<double> kTs_ = {2.5e-2};
};
struct Ev { int mt_ = 0; int& event_mt() { return mt_; } };
struct Particle { Ev ev; int& event_mt() { return ev.mt_; } double sqrtkT() const { return 0.1; } };
static int ELASTIC = 2;

// recorders: branch 1 = free-atom elastic, 2 = S(a,b), 3 = inelastic (index in rx.mt_)
static int branch = 0, chosen_mt = 0;
static void elastic_scatter(int, const Rx&, double, Particle&) { branch = 1; chosen_mt = 2; }
static void sab_scatter(int, int, Particle&) { branch = 2; chosen_mt = 2; }
static void inelastic_scatter(const Nuc&, const Rx& rx, Particle&) { branch = 3; chosen_mt = rx.mt_; }

static void walk(const Nuc* nucp, const Micro& micro, double cutoff, Particle& p)
{
  const Nuc* nuc = nucp;
  int i_nuclide = 0; int i_temp = 0;
  double kT = 0.0;
  bool sampled = false;
  (void)kT;
#define FWD
  // ---- verbatim from physics.cpp:732-775 ----
  // (the multipole_/kTs_ line is replaced by a constant via the macros below)
#include "walk_extract.inc"
}

int main()
{
  // Cases: {name, total, absorption, elastic, thermal, inelastic list (MT, xs)}.
  struct Case { const char* name; double tot, abs_, el, th; std::vector<std::pair<int,double>> in; };
  // A heavy fissile-like nuclide: 5 reactions of the inelastic class.
  std::vector<Case> cases = {
    {"heavy",   9.39, 1.5, 4.0, 0.0, {{5,0.05},{16,0.3},{17,0.1},{22,0.04},{51,1.0},{52,0.8},{53,0.6},{91,1.0}}},
    {"light",    4.0, 0.2, 3.7, 0.0, {{51,0.05},{52,0.03},{91,0.02}}},
    {"sab",      6.0, 0.5, 5.0, 3.0, {{51,0.2},{91,0.3}}},
    {"elastic_only", 3.0, 0.0, 3.0, 0.0, {}},
  };
  for (auto& c : cases) {
    Nuc nuc; std::vector<Rx> store; store.reserve(c.in.size() + 1);
    store.push_back({2, c.el});
    for (auto& r : c.in) store.push_back({r.first, r.second});
    for (auto& r : store) nuc.reactions_.push_back(&r);
    for (size_t k = 1; k < store.size(); ++k) nuc.index_inelastic_scatter_.push_back((int)k);
    Micro micro {c.tot, c.abs_, c.el, c.th, 0};
    for (uint64_t s = 1; s <= 400; ++s) {
      uint64_t seed = init_seed(s, 0);
      double cutoff = prn(&seed) * (micro.total - micro.absorption);
      Particle p; branch = 0; chosen_mt = -1;
      walk(&nuc, micro, cutoff, p);
      uint64_t cb; std::memcpy(&cb, &cutoff, 8);
      std::printf("%s %llu %016llx %d %d\n", c.name, (unsigned long long)s, (unsigned long long)cb, branch, chosen_mt);
    }
  }
  return 0;
}
