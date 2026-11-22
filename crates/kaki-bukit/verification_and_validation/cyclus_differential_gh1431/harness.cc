// Standalone harness calling CYCLUS's own toolkit routines (gh:#1431).
// Prints every result at 17 significant digits ("%.17g") so binary64
// round-trips. Output is committed as cyclus_output/toolkit.txt.
//
// Build (conda env E with cyclus 1.6.0, see README.md "Route"):
//   g++ -std=c++17 -O2 -I$E/include -I$E/include/cyclus harness.cc \
//       -L$E/lib -lcyclus -Wl,-rpath,$E/lib -o harness
//
// Routines exercised are cyclus's, unmodified: cyclus::toolkit::FeedQty /
// TailsQty / SwuRequired / ValueFunc (toolkit/enrichment.cc),
// cyclus::CycArithmetic::KahanSum (cyc_arithmetic.cc), cyclus::compmath::*
// (comp_math.cc), cyclus::Composition::Decay (decayer.cc + uniform_taylor.cc).
#include <cstdio>
#include <cstdlib>
#include <map>
#include <set>
#include <vector>

#include "comp_math.h"
#include "composition.h"
#include "decayer.h"
#include "cyc_arithmetic.h"
#include "pyne.h"
#include "toolkit/enrichment.h"

using cyclus::CompMap;

static void P(const char* tag, double v) { std::printf("%s %.17g\n", tag, v); }

int main() {
  pyne::NUC_DATA_PATH = std::getenv("CYCLUS_NUC_DATA");  // nuc_data.h5 of the conda package
  // C4: enrichment toolkit. feed 0.711 % (as atom/mass fraction 0.00711),
  // tails 0.25 %, ten product assays, product quantity 1 kg and 37.5 kg.
  const double feed = 0.00711, tails = 0.0025;
  const double prods[10] = {0.02, 0.03, 0.04, 0.05, 0.0675,
                            0.0825, 0.1, 0.15, 0.1975, 0.2};
  for (int i = 0; i < 10; ++i) {
    cyclus::toolkit::Assays a(feed, prods[i], tails);
    char t[64];
    for (double q : {1.0, 37.5}) {
      std::snprintf(t, sizeof t, "swu %.4f %.1f", prods[i], q);
      P(t, cyclus::toolkit::SwuRequired(q, a));
      std::snprintf(t, sizeof t, "feed %.4f %.1f", prods[i], q);
      P(t, cyclus::toolkit::FeedQty(q, a));
      std::snprintf(t, sizeof t, "tails %.4f %.1f", prods[i], q);
      P(t, cyclus::toolkit::TailsQty(q, a));
    }
    std::snprintf(t, sizeof t, "value %.4f", prods[i]);
    P(t, cyclus::toolkit::ValueFunc(prods[i]));
  }

  // C3a: KahanSum of a badly conditioned list (sorted internally by upstream).
  std::vector<double> v;
  v.push_back(1.0);
  for (int i = 0; i < 1000; ++i) v.push_back(1e-16 * (1 + i % 7));
  v.push_back(-1.0);
  v.push_back(0.1);
  v.push_back(0.2);
  v.push_back(0.3);
  P("kahan", cyclus::CycArithmetic::KahanSum(v));

  // C3b: comp_math on two compositions (nuclide ids as ZZZAAAMMMM).
  CompMap a, b;
  a[922350000] = 0.0355;
  a[922380000] = 0.9645;
  a[942390000] = 1e-3;
  b[922350000] = 0.1;
  b[922380000] = 0.3;
  b[942400000] = 0.7;
  CompMap s = cyclus::compmath::Add(a, b);
  CompMap d = cyclus::compmath::Sub(s, b);
  CompMap n = s;
  cyclus::compmath::Normalize(&n, 1.0);
  P("sum_a", cyclus::compmath::Sum(a));
  for (auto& kv : s) std::printf("add %d %.17g\n", kv.first, kv.second);
  for (auto& kv : d) std::printf("sub %d %.17g\n", kv.first, kv.second);
  for (auto& kv : n) std::printf("norm %d %.17g\n", kv.first, kv.second);

  // C3c: decay. Dump upstream's data (pyne) for a closed chain, then decay
  // a 5-nuclide atom composition for one 30-day step with Composition::Decay.
  const int nucs[9] = {380900000, 390900000, 400900000, 551370000,
                       561370000, 270600000, 280600000, 360850000,
                       370850000};
  for (int nuc : nucs) {
    std::printf("dc %d %.17g\n", nuc, pyne::decay_const(nuc));
    std::set<int> ch = pyne::decay_children(nuc);
    for (int c : ch)
      std::printf("br %d %d %.17g\n", nuc, c, pyne::branch_ratio(nuc, c));
  }
  CompMap c0;
  c0[380900000] = 1.0;
  c0[390900000] = 0.2;
  c0[551370000] = 0.7;
  c0[561370000] = 0.05;
  c0[270600000] = 0.4;
  cyclus::Composition::Ptr comp = cyclus::Composition::CreateFromAtom(c0);
  const uint64_t secs = 2592000;  // 30 days per step
  for (int steps : {1, 400, 4000}) {
    cyclus::Composition::Ptr dc = comp->Decay(steps, secs);
    for (auto& kv : dc->atom())
      std::printf("decay %d %d %.17g\n", steps, kv.first, kv.second);
  }

  // C3d: the legacy Decayer (uniform_taylor.cc), which Composition::Decay no
  // longer calls. Its BuildDecayMatrix "gross heuristic" zeroes the decay
  // constant of every nuclide with lambda > ~2.6e-7 /s (see README.md), so
  // two compositions are run: "taylor" holds only slow nuclides (Cs-137,
  // Ba-137, Co-60, Ni-60, Kr-85) and is unaffected; "taylor_sr" is the
  // Sr-90 / Y-90 composition above, where the heuristic freezes Y-90.
  CompMap c1;
  c1[551370000] = 0.7;
  c1[561370000] = 0.05;
  c1[270600000] = 0.4;
  c1[280600000] = 0.1;
  c1[360850000] = 0.3;
  for (int steps : {1, 40, 4000}) {
    cyclus::Decayer dec(c1);
    dec.Decay(static_cast<double>(steps) * static_cast<double>(secs));
    CompMap out;
    dec.GetResult(out);
    for (auto& kv : out)
      std::printf("taylor %d %d %.17g\n", steps, kv.first, kv.second);
  }
  {
    cyclus::Decayer dec(c0);
    dec.Decay(static_cast<double>(secs));
    CompMap out;
    dec.GetResult(out);
    for (auto& kv : out)
      std::printf("taylor_sr %d %d %.17g\n", 1, kv.first, kv.second);
  }
  return 0;
}
