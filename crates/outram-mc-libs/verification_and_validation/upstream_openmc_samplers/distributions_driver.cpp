// Driver: OpenMC source distributions with a fixed seed (gh:#1388).
// Compiles OpenMC's REAL distribution.cpp (lines 26-772, verbatim extract) and
// distribution_spatial.cpp (verbatim extract), random_dist.cpp, random_lcg.cpp
// and position.cpp. Only XML (stubs_dist/pugixml.hpp, an in-memory tree) and
// mesh.h are replaced, so upstream's own XML constructors, distribution_from_xml,
// Vose alias tables, Mixture and the frame construction of CylindricalIndependent
// all run. Prints value, importance weight and the final RNG state as hex bits.
#include <algorithm>
#include <cmath>
#include <iterator>
#include <numeric>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include "openmc/distribution.h"
#include "openmc/distribution_spatial.h"
#include "openmc/math_functions.h"
#include "openmc/random_dist.h"
#include "openmc/random_lcg.h"
#include "openmc/xml_interface.h"

// definitions the verbatim extracts need
namespace openmc {
namespace model {
std::vector<std::unique_ptr<Mesh>> meshes;
}
// DecaySpectrum needs the depletion chain (not part of this differential):
// referenced by distribution_from_xml's "decay_spectrum" branch only.
DecaySpectrum::DecaySpectrum(pugi::xml_node) { fatal_error("DecaySpectrum is not built here"); }
DecaySpectrum::Sample DecaySpectrum::sample_with_parent(uint64_t*) const { fatal_error("no"); }
std::pair<double, double> DecaySpectrum::sample(uint64_t*) const { fatal_error("no"); }
double DecaySpectrum::integral() const { return 0; }
double DecaySpectrum::sample_unbiased(uint64_t*) const { fatal_error("no"); }
void DecaySpectrum::init(vector<int>, const vector<double>&) {}
// exprel / log1prel (math_functions.cpp:931,940), verbatim
double exprel(double x)
{
  if (std::abs(x) < 1e-16)
    return 1.0;
  else {
    return std::expm1(x) / x;
  }
}
double log1prel(double x)
{
  if (std::abs(x) < 1e-16)
    return 1.0;
  else {
    return std::log1p(x) / x;
  }
}
} // namespace openmc
#include "normal_cdf.inc"
#include "distribution_full.inc"
#include "spatial_full.inc"

using namespace openmc;
typedef unsigned long long ull;
using pugi::xml_node;

static uint64_t bits(double d) { uint64_t u; std::memcpy(&u, &d, sizeof u); return u; }

static xml_node dist(const char* type, const char* params)
{
  xml_node n("dist");
  n.set("type", type);
  if (params) n.set("parameters", params);
  return n;
}

static void run_dist(const char* name, const Distribution& d, int nseeds = 40)
{
  for (int s = 1; s <= nseeds; s++) {
    uint64_t seed = (uint64_t)s * 104729u + 3;
    auto [v, w] = d.sample(&seed);
    std::printf("S %s seed=%d v=%016llx w=%016llx end=%016llx\n", name, s, (ull)bits(v), (ull)bits(w), (ull)seed);
  }
}
static void eval_dist(const char* name, const Distribution& d, const std::vector<double>& xs)
{
  for (size_t i = 0; i < xs.size(); i++)
    std::printf("E %s i=%zu x=%016llx f=%016llx\n", name, i, (ull)bits(xs[i]), (ull)bits(d.evaluate(xs[i])));
}
static void run_space(const char* name, const SpatialDistribution& d, int nseeds = 40)
{
  for (int s = 1; s <= nseeds; s++) {
    uint64_t seed = (uint64_t)s * 104729u + 3;
    auto [p, w] = d.sample(&seed);
    std::printf("P %s seed=%d x=%016llx y=%016llx z=%016llx w=%016llx end=%016llx\n", name, s, (ull)bits(p.x),
      (ull)bits(p.y), (ull)bits(p.z), (ull)bits(w), (ull)seed);
  }
}

int main()
{
  std::printf("# OpenMC source distributions: commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf\n");
  const std::vector<double> xs = {0.5, 1.0, 1.7, 2.0, 2.4, 3.3, 4.0, 4.9, 5.0, 6.0, 1.0e6, 2.5e6};

  auto uniform = dist("uniform", "1.0 4.0");
  auto powerlaw = dist("powerlaw", "2.0 5.0 2.0");
  auto powerlaw_frac = dist("powerlaw", "1.0 3.0 1.5");
  auto maxwell = dist("maxwell", "1.3e6");
  auto watt = dist("watt", "0.988e6 2.249e-6");
  auto normal = dist("normal", "2.0 0.5");
  auto normal_trunc = dist("normal", "2.0 0.5 1.8 2.4");
  auto normal_tail = dist("normal", "0.0 1.0 2.0 4.0");
  auto discrete5 = dist("discrete", "1.0 2.0 3.0 4.0 5.0  0.1 0.4 0.2 0.05 0.25");
  auto discrete1 = dist("discrete", "7.5 3.0");
  auto discrete2 = dist("discrete", "1.0e6 2.0e6 0.7 0.3");
  auto tab_hist = dist("tabular", "1.0 2.0 3.5 5.0  0.5 1.5 1.0 0.0");
  auto tab_lin = dist("tabular", "1.0 2.0 3.5 5.0  0.5 1.5 1.0 0.2");
  auto tab_flat = dist("tabular", "1.0 2.0 3.0  1.0 1.0 1.0");
  auto tab_loglin = dist("tabular", "1.0 2.0 3.5 5.0  0.5 1.5 1.0 0.2");
  tab_hist.set("interpolation", "histogram");
  tab_lin.set("interpolation", "linear-linear");
  tab_flat.set("interpolation", "linear-linear");
  tab_loglin.set("interpolation", "log-linear");
  auto tab_loglog = dist("tabular", "1.0 2.0 3.5 5.0  0.5 1.5 1.0 0.2");
  tab_loglog.set("interpolation", "log-log");

  struct Case { const char* name; xml_node n; bool eval; };
  std::vector<Case> cases = {{"uniform", uniform, true}, {"powerlaw", powerlaw, true},
    {"powerlaw_frac", powerlaw_frac, true}, {"maxwell", maxwell, true}, {"watt", watt, true},
    {"normal", normal, true}, {"normal_trunc", normal_trunc, true}, {"normal_tail", normal_tail, true},
    {"discrete5", discrete5, false}, {"discrete1", discrete1, false}, {"discrete2", discrete2, false},
    {"tab_hist", tab_hist, true}, {"tab_lin", tab_lin, true}, {"tab_flat", tab_flat, true},
    {"tab_loglin", tab_loglin, true}, {"tab_loglog", tab_loglog, true}};
  for (auto& c : cases) {
    auto d = distribution_from_xml(c.n);
    std::printf("I %s integral=%016llx\n", c.name, (ull)bits(d->integral()));
    run_dist(c.name, *d);
    if (c.eval) {
      std::vector<double> pts = xs;
      if (std::string(c.name) == "normal_tail") pts = {1.0, 2.0, 2.5, 3.0, 4.0, 5.0};
      eval_dist(c.name, *d, pts);
    }
  }

  // Mixture: uniform + discrete + tabular, weights 0.2/0.5/0.3; and a nested mixture.
  {
    xml_node m("dist");
    m.set("type", "mixture");
    auto pair = [&](double p, xml_node dn) {
      xml_node pr("pair");
      pr.set("probability", std::to_string(p));
      pr.add("dist", dn);
      m.add("pair", pr);
    };
    pair(0.2, uniform);
    pair(0.5, discrete5);
    pair(0.3, tab_lin);
    auto d = distribution_from_xml(m);
    std::printf("I mixture integral=%016llx\n", (ull)bits(d->integral()));
    run_dist("mixture", *d);

    xml_node outer("dist");
    outer.set("type", "mixture");
    xml_node p1("pair"), p2("pair");
    p1.set("probability", "0.4"); p1.add("dist", m);
    p2.set("probability", "0.6"); p2.add("dist", maxwell);
    outer.add("pair", p1);
    outer.add("pair", p2);
    auto d2 = distribution_from_xml(outer);
    std::printf("I mixture_nested integral=%016llx\n", (ull)bits(d2->integral()));
    run_dist("mixture_nested", *d2);

    // biased mixture (bias probabilities per component)
    xml_node mb = m;
    // the same tree with a bias attribute (a fresh tree, since nodes are shared)
    xml_node mbias("dist");
    mbias.set("type", "mixture");
    mbias.set("bias", "0.5 0.25 0.25");
    for (auto& k : m.d->kids) mbias.add("pair", k.second);
    auto d3 = distribution_from_xml(mbias);
    run_dist("mixture_biased", *d3);
  }

  // biased Discrete (bias attribute), biased Uniform (bias child = powerlaw on [1,4])
  {
    auto db = dist("discrete", "1.0 2.0 3.0 4.0 5.0  0.1 0.4 0.2 0.05 0.25");
    db.set("bias", "0.2 0.2 0.2 0.2 0.2");
    auto d = distribution_from_xml(db);
    run_dist("discrete_biased", *d);
    auto ub = dist("uniform", "1.0 4.0");
    ub.add("bias", dist("powerlaw", "1.0 4.0 1.0"));
    auto d2 = distribution_from_xml(ub);
    run_dist("uniform_biased", *d2);
    auto mb = dist("maxwell", "1.3e6");
    mb.add("bias", dist("watt", "0.988e6 2.249e-6"));
    auto d3 = distribution_from_xml(mb);
    run_dist("maxwell_biased", *d3);
  }

  // ---- spatial (upstream's XML constructors) ----
  {
    xml_node n("space");
    n.add("x", dist("uniform", "-1.0 3.0"));
    n.add("y", dist("powerlaw", "1.0 2.0 2.0"));
    n.add("z", dist("discrete", "0.0 1.0 2.0  0.2 0.3 0.5"));
    run_space("cartesian", CartesianIndependent(n));
    xml_node n0("space");
    n0.add("y", dist("normal", "0.0 1.0"));
    run_space("cartesian_default", CartesianIndependent(n0));
    xml_node nb("space");
    auto ux = dist("uniform", "1.0 4.0");
    ux.add("bias", dist("powerlaw", "1.0 4.0 1.0"));
    nb.add("x", ux);
    run_space("cartesian_biased", CartesianIndependent(nb));
  }
  {
    xml_node n("space");
    n.add("r", dist("powerlaw", "0.0 2.0 1.0"));
    n.add("phi", dist("uniform", "0.0 6.283185307179586"));
    n.add("z", dist("uniform", "-1.0 1.0"));
    run_space("cyl_default", CylindricalIndependent(n));
    n.set("origin", "1.0 -2.0 0.5");
    n.set("z_dir", "0.0 1.0 1.0");
    n.set("r_dir", "1.0 0.0 0.0");
    run_space("cyl_tilted", CylindricalIndependent(n));
    n.set("z_dir", "1.0 1.0 1.0");
    n.set("r_dir", "1.0 -1.0 0.0");
    run_space("cyl_diag", CylindricalIndependent(n));
    xml_node nb("space");
    auto ur = dist("uniform", "0.0 2.0");
    ur.add("bias", dist("powerlaw", "0.0 2.0 1.0"));
    nb.add("r", ur);
    nb.add("phi", dist("uniform", "0.0 3.0"));
    nb.add("z", dist("discrete", "0.5 1.5 0.25 0.75"));
    run_space("cyl_biased", CylindricalIndependent(nb));
  }
  {
    xml_node n("space");
    n.add("r", dist("powerlaw", "1.0 3.0 2.0"));
    n.add("cos_theta", dist("uniform", "-1.0 1.0"));
    n.add("phi", dist("uniform", "0.0 6.283185307179586"));
    n.set("origin", "0.5 0.25 -1.0");
    run_space("sph", SphericalIndependent(n));
    xml_node n0("space");
    run_space("sph_default", SphericalIndependent(n0));
  }
  {
    xml_node n("space");
    n.set("coords", "0 0 0  1 0 0  0 2 0  0 0 3  1 1 1");
    n.set("strengths", "1.0 2.0 0.5 4.0 1.5");
    run_space("cloud", PointCloud(n));
    xml_node nn("space");
    nn.set("coords", "0 0 0  1 0 0  0 2 0");
    run_space("cloud_uniform", PointCloud(nn));
    std::vector<Position> pts = {{0, 0, 0}, {1, 0, 0}, {0, 2, 0}, {0, 0, 3}};
    std::vector<double> st = {3.0, 1.0, 1.0, 5.0};
    run_space("cloud_span", PointCloud(pts, span<const double>(st.data(), st.size())));
    // upstream's XML bias route re-reads the UNBIASED strengths as the bias
    // (distribution_spatial.cpp:420): every weight is exactly 1
    xml_node nb("space");
    nb.set("coords", "0 0 0  1 0 0  0 2 0");
    nb.set("strengths", "1.0 2.0 3.0");
    xml_node bn("bias");
    bn.set("strengths", "3.0 1.0 1.0");
    nb.add("bias", bn);
    run_space("cloud_bias_xml", PointCloud(nb));
  }
  {
    xml_node n("space");
    n.set("parameters", "-1.0 -2.0 -3.0 4.0 5.0 6.0");
    run_space("box", SpatialBox(n));
    xml_node p("space");
    p.set("parameters", "1.5 2.5 3.5");
    run_space("point", SpatialPoint(p), 2);
  }
  return 0;
}
