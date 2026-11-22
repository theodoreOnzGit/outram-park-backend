// Driver for OpenMC's REAL sample_fission_neutron (physics.cpp:1081) and
// Nuclide::nu (nuclide.cpp:532), verbatim via fission_extract.inc (gh:#1444 d).
// Fission distribution (synthetic, documented in README.md): prompt = isotropic
// uncorrelated Watt; delayed groups 1..3 = isotropic uncorrelated Maxwell with
// distinct theta(E); nu_total, nu_d(E) tabulated; group yield_k = nu_d(E) * p_k.
#include <cstdio>
#include <cstring>
#include <cmath>
#include <map>
#include "openmc/hdf5_interface.h"
#include "openmc/random_lcg.h"
#include "openmc/secondary_uncorrelated.h"
#include "openmc/distribution.h"
#include "openmc/endf.h"
#include "openmc/math_functions.h"
#include "openmc/search.h"
#include "math_extract.inc"
#include "distribution_extract.inc"
#include "endf_extract.inc"
#include "fission_stubs.h"
#include "fission_extract.inc"

using namespace openmc;
using mem::Node;
typedef unsigned long long ull;

static uint64_t bits(double d) { uint64_t u; std::memcpy(&u, &d, sizeof u); return u; }
static void set_attr(Node& n, const char* name, std::vector<double> v)
{
  auto& a = n.attrs[name];
  a.v = v;
  a.shape = {v.size()};
}
static void set_str(Node& n, const char* name, const char* s) { n.attrs[name].s = s; }
static void set_data(Node& n, std::vector<double> v, std::vector<size_t> shape) { n.data.v = v; n.data.shape = shape; }
static void print_list(const char* key, const std::vector<double>& v)
{
  std::printf(" %s=", key);
  for (size_t i = 0; i < v.size(); i++) std::printf("%s%016llx", i ? "," : "", (ull)bits(v[i]));
}
static void fill_tab1(Node& parent, const char* name, std::vector<double> x, std::vector<double> y)
{
  Node& n = parent.kids[name];
  set_attr(n, "breakpoints", {(double)x.size()});
  set_attr(n, "interpolation", {2.0});
  std::vector<double> v = x;
  v.insert(v.end(), y.begin(), y.end());
  set_data(n, v, {2, x.size()});
}
// A Tabulated1D from (x, y), printed as `tab <name>` so the port builds the same table.
static std::unique_ptr<Tabulated1D> make_tab(const char* name, std::vector<double> x, std::vector<double> y)
{
  Node n;
  set_attr(n, "breakpoints", {(double)x.size()});
  set_attr(n, "interpolation", {2.0});
  std::vector<double> v = x;
  v.insert(v.end(), y.begin(), y.end());
  set_data(n, v, {2, x.size()});
  std::printf("tab %s", name); print_list("x", x); print_list("y", y); std::printf("\n");
  return std::make_unique<Tabulated1D>(mem::handle(n));
}
static std::unique_ptr<AngleEnergy> iso_law(const char* type, const char* a, std::vector<double> ax,
  std::vector<double> ay, const char* b, std::vector<double> bx, std::vector<double> by)
{
  Node iso;
  Node& en = iso.kids["energy"];
  set_str(en, "type", type);
  set_attr(en, "u", {1.0e4});
  fill_tab1(en, a, ax, ay);
  if (b) fill_tab1(en, b, bx, by);
  return std::make_unique<UncorrelatedAngleEnergy>(mem::handle(iso));
}

int main()
{
  const std::vector<double> eg = {1.0e-5, 1.0e6, 2.0e7};
  const double pk[3] = {0.1, 0.25, 0.65};
  const double lam[3] = {0.0124, 0.0305, 0.111};
  // synthetic, INFLATED delayed yield (beta ~ 0.12) so the delayed branch is exercised
  const std::vector<double> nud = {0.28, 0.30, 0.34};
  Nuclide nuc;
  nuc.name_ = "SYNTH-FISSILE";
  nuc.n_precursor_ = 3;
  nuc.total_nu_ = make_tab("nu_total", eg, {2.43, 2.60, 3.30});
  Reaction rx;
  rx.products_.resize(4);
  rx.products_[0].emission_mode_ = ReactionProduct::EmissionMode::prompt;
  rx.products_[0].yield_ = make_tab("yield_prompt", eg, {2.15, 2.30, 2.96});
  rx.products_[0].distribution_.push_back(iso_law("watt", "a", {1.0e-5, 2.0e7}, {0.8e5, 1.2e5}, "b", {1.0e-5, 2.0e7}, {2.5e-6, 3.0e-6}));
  std::printf("tab watt_a x=%016llx,%016llx y=%016llx,%016llx\n", (ull)bits(1.0e-5), (ull)bits(2.0e7), (ull)bits(0.8e5), (ull)bits(1.2e5));
  std::printf("tab watt_b x=%016llx,%016llx y=%016llx,%016llx\n", (ull)bits(1.0e-5), (ull)bits(2.0e7), (ull)bits(2.5e-6), (ull)bits(3.0e-6));
  for (int g = 1; g <= 3; g++) {
    ReactionProduct& rp = rx.products_[g];
    rp.emission_mode_ = ReactionProduct::EmissionMode::delayed;
    rp.decay_rate_ = lam[g - 1];
    std::vector<double> y;
    for (double d : nud) y.push_back(d * pk[g - 1]);
    char nm[32]; std::snprintf(nm, sizeof nm, "yield_g%d", g);
    rp.yield_ = make_tab(nm, eg, y);
    const double th = 0.6e5 * g;
    rp.distribution_.push_back(iso_law("maxwell", "theta", {1.0e-5, 1.0e6, 2.0e7}, {th, 2 * th, 4 * th}, nullptr, {}, {}));
    std::printf("tab theta_g%d x=%016llx,%016llx,%016llx y=%016llx,%016llx,%016llx\n", g,
      (ull)bits(1.0e-5), (ull)bits(1.0e6), (ull)bits(2.0e7), (ull)bits(th), (ull)bits(2 * th), (ull)bits(4 * th));
  }
  std::printf("tab nu_delayed"); print_list("x", eg); print_list("y", nud); std::printf("\n");
  std::printf("param n_groups=3 emax=%016llx\n", (ull)bits(1.5e6));
  for (int g = 0; g < 3; g++) std::printf("param lam%d=%016llx p%d=%016llx\n", g, (ull)bits(lam[g]), g, (ull)bits(pk[g]));
  nuc.fission_rx_.push_back(&rx);
  data::nuclides.push_back(std::make_unique<Nuclide>());
  // Nuclide is move-only (unique_ptr): rebuild the registered entry from ours.
  data::nuclides.back()->name_ = nuc.name_;
  data::nuclides.back()->n_precursor_ = nuc.n_precursor_;
  data::nuclides.back()->total_nu_ = std::move(nuc.total_nu_);
  data::nuclides.back()->fission_rx_ = nuc.fission_rx_;
  // small max energy so the resampling loop is exercised
  data::energy_max[0] = 1.5e6;
  // E_in must exceed the restriction energy u = 1e4 eV: upstream rejects E_out > E_in - u in a loop
  const double Es[6] = {3.0e4, 5.0e4, 2.0e5, 1.0e6, 2.7e6, 9.0e6};
  const Direction dirs[3] = {Direction {0.0, 0.0, 1.0}, Direction {0.6, 0.0, 0.8}, Direction {-0.48, 0.64, -0.6}};
  for (double E : Es)
    for (int id = 0; id < 3; id++)
      for (int s = 1; s <= 120; s++) {
        Particle p;
        p.E_ = E;
        p.u_ = dirs[id];
        p.seed_ = (uint64_t)s * 7919u + 11 + (uint64_t)id * 104729u;
        SourceSite site;
        site.time = 0.0;
        sample_fission_neutron(0, rx, &site, p);
        std::printf("F E=%016llx id=%d seed0=%016llx grp=%d Eo=%016llx u=%016llx,%016llx,%016llx time=%016llx end=%016llx\n",
          (ull)bits(E), id, (ull)((uint64_t)s * 7919u + 11 + (uint64_t)id * 104729u), site.delayed_group,
          (ull)bits(site.E), (ull)bits(site.u.x), (ull)bits(site.u.y), (ull)bits(site.u.z),
          (ull)bits(site.time), (ull)p.seed_);
      }
  return 0;
}
