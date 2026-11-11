// Driver: OpenMC secondary-distribution samplers with a fixed seed (gh:#1386,
// prio-1 row 4). Links upstream's REAL distribution_energy.cpp,
// distribution_angle.cpp, secondary_{uncorrelated,kalbach,correlated,nbody}.cpp,
// random_dist.cpp, random_lcg.cpp, math_functions.cpp, plus verbatim extracts
// (extract.sh) of Distribution/Tabular (distribution.cpp) and Tabulated1D
// (endf.cpp). Only the HDF5 layer is replaced, by the in-memory tree in
// stubs/openmc/hdf5_interface.h, so the REAL constructors run on tables this
// driver builds in `openmc.data`'s HDF5 layout. Every table is printed (as hex
// bits) into reference.txt so the Rust test builds the identical data.
//
// A "fixed xi sequence" is a fixed seed: both codes draw the same xi from
// random_lcg's prn, so a sampled value is a deterministic function of
// (table, E_in, seed) and any difference in draw ORDER or COUNT shows up.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <vector>

#include "openmc/angle_energy.h"
#include "openmc/distribution_angle.h"
#include "openmc/distribution_energy.h"
#include "openmc/hdf5_interface.h"
#include "openmc/random_dist.h"
#include "openmc/random_lcg.h"
#include "openmc/secondary_correlated.h"
#include "openmc/secondary_kalbach.h"
#include "openmc/secondary_nbody.h"
#include "openmc/secondary_uncorrelated.h"

#include <algorithm>
#include <iterator>
#include "openmc/distribution.h"
#include "openmc/endf.h"
#include "openmc/math_functions.h"
#include "openmc/search.h"
#include "math_extract.inc"
#include "distribution_extract.inc"
#include "endf_extract.inc"

using namespace openmc;
using mem::Node;
typedef unsigned long long ull;

static uint64_t bits(double d) { uint64_t u; std::memcpy(&u, &d, sizeof u); return u; }

static void set_attr(Node& n, const char* name, std::vector<double> v, std::vector<size_t> shape = {})
{
  auto& a = n.attrs[name];
  a.v = v;
  a.shape = shape.empty() ? std::vector<size_t>{v.size()} : shape;
}
static void set_str(Node& n, const char* name, const char* s) { n.attrs[name].s = s; }
static void set_data(Node& n, std::vector<double> v, std::vector<size_t> shape)
{
  n.data.v = v;
  n.data.shape = shape;
}

static void print_list(const char* key, const std::vector<double>& v)
{
  std::printf(" %s=", key);
  for (size_t i = 0; i < v.size(); i++) std::printf("%s%016llx", i ? "," : "", (ull)bits(v[i]));
}

// ---- tables ---------------------------------------------------------------
struct Table {
  std::vector<double> egrid;
  std::vector<std::vector<double>> eout, p, c, r, a;
  std::vector<int> interp, nd;
  // correlated: per outgoing point a mu table
  std::vector<std::vector<std::vector<double>>> mux, mup, muc;
};

// Build a small table: n_in incident energies, 6 outgoing points each.
// kind: 1 histogram (interp code 1), 2 lin-lin; ndisc discrete lines in front.
static Table make_table(const char* name, int interp, int ndisc, bool km, bool corr)
{
  Table t;
  t.egrid = {1.0e5, 1.0e6, 4.0e6, 2.0e7};
  int jglob = 0;
  for (size_t i = 0; i < t.egrid.size(); i++) {
    double E = t.egrid[i];
    int n = 6 + ndisc;
    std::vector<double> eo(n), p(n), c(n), r(n), a(n);
    std::vector<std::vector<double>> mx, mp, mc;
    for (int k = 0; k < n; k++) {
      if (k < ndisc) {
        eo[k] = E * (0.1 + 0.15 * k);
        p[k] = 0.10 + 0.05 * k; // line probability
      } else {
        int m = k - ndisc;
        eo[k] = E * (0.25 + 0.17 * m) * (1.0 + 0.03 * i);
        static const double shape[6] = {0.2, 0.9, 1.0, 0.7, 0.3, 0.05};
        p[k] = shape[m] / E;
      }
      r[k] = 0.1 + 0.12 * ((k + i) % 5);
      a[k] = 0.4 + 0.5 * ((2 * k + i) % 6);
    }
    // cumulative distribution (stored in the file, used by the samplers)
    for (int k = 0; k < n; k++) {
      if (k < ndisc) c[k] = (k ? c[k - 1] : 0.0) + p[k];
      else if (k == ndisc) c[k] = (k ? c[k - 1] : 0.0);
      else if (interp == 1) c[k] = c[k - 1] + p[k - 1] * (eo[k] - eo[k - 1]);
      else c[k] = c[k - 1] + 0.5 * (p[k - 1] + p[k]) * (eo[k] - eo[k - 1]);
    }
    double tot = c[n - 1];
    for (int k = 0; k < n; k++) {
      c[k] /= tot;
      p[k] /= tot;
    }
    if (corr) {
      for (int k = 0; k < n; k++) {
        double s = ((jglob + k) % 5 - 2) / 2.5; // slope in (-1, 1)
        std::vector<double> x = {-1.0, -0.5, 0.0, 0.5, 1.0}, q(5), cc(5);
        for (int j = 0; j < 5; j++) q[j] = 0.5 + 0.35 * s * x[j];
        cc[0] = 0.0;
        for (int j = 1; j < 5; j++) cc[j] = cc[j - 1] + 0.5 * (q[j - 1] + q[j]) * (x[j] - x[j - 1]);
        for (int j = 0; j < 5; j++) { q[j] /= cc[4]; }
        double c4 = cc[4];
        for (int j = 0; j < 5; j++) cc[j] /= c4;
        mx.push_back(x); mp.push_back(q); mc.push_back(cc);
      }
      jglob += n;
    }
    t.eout.push_back(eo); t.p.push_back(p); t.c.push_back(c); t.r.push_back(r); t.a.push_back(a);
    t.interp.push_back(interp); t.nd.push_back(ndisc);
    t.mux.push_back(mx); t.mup.push_back(mp); t.muc.push_back(mc);
  }
  std::printf("table %s ne=%zu km=%d corr=%d", name, t.egrid.size(), km, corr);
  print_list("egrid", t.egrid);
  std::printf("\n");
  for (size_t i = 0; i < t.egrid.size(); i++) {
    std::printf("row %s i=%zu interp=%d nd=%d", name, i, t.interp[i], t.nd[i]);
    print_list("eout", t.eout[i]); print_list("p", t.p[i]); print_list("c", t.c[i]);
    if (km) { print_list("r", t.r[i]); print_list("a", t.a[i]); }
    std::printf("\n");
    if (corr) {
      for (size_t k = 0; k < t.mux[i].size(); k++) {
        std::printf("mu %s i=%zu k=%zu", name, i, k);
        print_list("x", t.mux[i][k]); print_list("p", t.mup[i][k]); print_list("c", t.muc[i][k]);
        std::printf("\n");
      }
    }
  }
  return t;
}

// 'energy' dataset (+ interpolation attribute: one lin-lin region) and the
// outgoing-energy dataset of continuous / Kalbach / correlated laws.
static void fill_energy_ds(Node& g, const Table& t)
{
  Node& e = g.kids["energy"];
  set_attr(e, "interpolation", {(double)t.egrid.size(), 2.0}, {2, 1});
  set_data(e, t.egrid, {t.egrid.size()});
}
static void fill_out_ds(Node& g, const char* dname, const Table& t, int rows, bool km, bool corr)
{
  Node& d = g.kids[dname];
  std::vector<double> offs, interp, nd;
  size_t total = 0;
  for (size_t i = 0; i < t.egrid.size(); i++) {
    offs.push_back((double)total);
    interp.push_back(t.interp[i]);
    nd.push_back(t.nd[i]);
    total += t.eout[i].size();
  }
  set_attr(d, "offsets", offs);
  set_attr(d, "interpolation", interp);
  set_attr(d, "n_discrete_lines", nd);
  std::vector<double> v(rows * total, 0.0);
  size_t col = 0, mu_off = 0;
  std::vector<double> mux, mup, muc;
  for (size_t i = 0; i < t.egrid.size(); i++) {
    for (size_t k = 0; k < t.eout[i].size(); k++, col++) {
      v[0 * total + col] = t.eout[i][k];
      v[1 * total + col] = t.p[i][k];
      v[2 * total + col] = t.c[i][k];
      if (km) { v[3 * total + col] = t.r[i][k]; v[4 * total + col] = t.a[i][k]; }
      if (corr) {
        v[3 * total + col] = 2.0;           // interp_mu: lin-lin
        v[4 * total + col] = (double)mu_off; // offset into mu
        for (int j = 0; j < 5; j++) {
          mux.push_back(t.mux[i][k][j]); mup.push_back(t.mup[i][k][j]); muc.push_back(t.muc[i][k][j]);
        }
        mu_off += 5;
      }
    }
  }
  set_data(d, v, {(size_t)rows, total});
  if (corr) {
    std::vector<double> m;
    m.insert(m.end(), mux.begin(), mux.end());
    m.insert(m.end(), mup.begin(), mup.end());
    m.insert(m.end(), muc.begin(), muc.end());
    set_data(g.kids["mu"], m, {3, mux.size()});
  }
}

static void fill_tab1(Node& parent, const char* name, std::vector<double> x, std::vector<double> y)
{
  Node& n = parent.kids[name];
  set_attr(n, "breakpoints", {(double)x.size()});
  set_attr(n, "interpolation", {2.0});
  std::vector<double> v = x;
  v.insert(v.end(), y.begin(), y.end());
  set_data(n, v, {2, x.size()});
  std::printf("tab1 %s", name);
  print_list("x", x); print_list("y", y);
  std::printf("\n");
}

static const double EIN[6] = {5.0e4, 2.0e5, 1.0e6, 2.7e6, 9.0e6, 3.0e7};
static const int NSEED = 40;

static void emit_energy(const char* name, const EnergyDistribution& d, const double* es, int ne)
{
  for (int ie = 0; ie < ne; ie++)
    for (int s = 1; s <= NSEED; s++) {
      uint64_t seed = (uint64_t)s * 7919u + 11;
      double e = d.sample(es[ie], &seed);
      std::printf("S %s E=%016llx seed=%d eout=%016llx end=%016llx\n", name, (ull)bits(es[ie]), s,
                  (ull)bits(e), (ull)seed);
    }
}
static void emit_ae(const char* name, const AngleEnergy& d, const double* es, int ne)
{
  for (int ie = 0; ie < ne; ie++)
    for (int s = 1; s <= NSEED; s++) {
      uint64_t seed = (uint64_t)s * 7919u + 11;
      double e = 0, mu = 0;
      d.sample(es[ie], e, mu, &seed);
      std::printf("S %s E=%016llx seed=%d eout=%016llx mu=%016llx end=%016llx\n", name,
                  (ull)bits(es[ie]), s, (ull)bits(e), (ull)bits(mu), (ull)seed);
    }
}

int main()
{
  std::printf("# OpenMC secondary samplers: commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf\n");
  // random_dist: maxwell, watt, uniform, normal
  for (int s = 1; s <= 64; s++) {
    uint64_t seed = (uint64_t)s * 104729u + 3;
    double m = maxwell_spectrum(1.2e6, &seed);
    uint64_t e1 = seed;
    double w = watt_spectrum(0.988e6, 2.249e-6, &seed);
    uint64_t e2 = seed;
    double n = normal_variate(2.0, 0.5, &seed);
    std::printf("D seed=%d maxwell=%016llx m_end=%016llx watt=%016llx w_end=%016llx normal=%016llx n_end=%016llx\n",
                s, (ull)bits(m), (ull)e1, (ull)bits(w), (ull)e2, (ull)bits(n), (ull)seed);
  }

  // ContinuousTabular: histogram, lin-lin, lin-lin with discrete lines
  struct CT { const char* name; int interp; int nd; } cts[3] = {{"ct_hist", 1, 0}, {"ct_linlin", 2, 0}, {"ct_disc", 2, 2}};
  for (auto& k : cts) {
    Table t = make_table(k.name, k.interp, k.nd, false, false);
    Node root;
    fill_energy_ds(root, t);
    fill_out_ds(root, "distribution", t, 3, false, false);
    ContinuousTabular d {mem::handle(root)};
    emit_energy(k.name, d, EIN, 6);
  }
  // Maxwell / Evaporation / Watt
  {
    Node root;
    set_attr(root, "u", {1.0e4});
    fill_tab1(root, "theta", {1.0e-5, 1.0e6, 2.0e7}, {1.0e5, 2.0e5, 4.0e5});
    MaxwellEnergy m {mem::handle(root)};
    std::printf("param maxwell u=%016llx\n", (ull)bits(1.0e4));
    const double es[4] = {2.0e5, 1.0e6, 2.7e6, 9.0e6};
    emit_energy("maxwell", m, es, 4);
    Evaporation ev {mem::handle(root)};
    emit_energy("evaporation", ev, es, 4);
    Node w;
    set_attr(w, "u", {1.0e4});
    fill_tab1(w, "a", {1.0e-5, 2.0e7}, {0.8e5, 1.2e5});
    fill_tab1(w, "b", {1.0e-5, 2.0e7}, {2.5e-6, 3.0e-6});
    WattEnergy we {mem::handle(w)};
    const double ew[3] = {1.0e6, 2.7e6, 9.0e6};
    emit_energy("watt", we, ew, 3);
  }
  // LevelInelastic, DiscretePhoton
  {
    Node l;
    set_attr(l, "threshold", {2.5e6});
    set_attr(l, "mass_ratio", {0.9747});
    std::printf("param level threshold=%016llx mass_ratio=%016llx\n", (ull)bits(2.5e6), (ull)bits(0.9747));
    LevelInelastic li {mem::handle(l)};
    const double es[3] = {3.0e6, 5.5e6, 1.4e7};
    emit_energy("level", li, es, 3);
  }
  // NBody
  for (int nb = 3; nb <= 5; nb++) {
    Node g;
    set_attr(g, "n_particles", {(double)nb});
    set_attr(g, "total_mass", {2.9 + 0.1 * nb});
    set_attr(g, "atomic_weight_ratio", {235.0});
    set_attr(g, "q_value", {-6.5e6});
    std::printf("param nbody%d n=%d total_mass=%016llx awr=%016llx q=%016llx\n", nb, nb,
                (ull)bits(2.9 + 0.1 * nb), (ull)bits(235.0), (ull)bits(-6.5e6));
    NBodyPhaseSpace nbp {mem::handle(g)};
    char nm[16];
    std::snprintf(nm, sizeof nm, "nbody%d", nb);
    const double es[3] = {8.0e6, 1.4e7, 3.0e7};
    emit_ae(nm, nbp, es, 3);
  }
  // Kalbach-Mann
  for (int ip = 1; ip <= 2; ip++) {
    char nm[16];
    std::snprintf(nm, sizeof nm, "km_%s", ip == 1 ? "hist" : "linlin");
    Table t = make_table(nm, ip, 0, true, false);
    Node root;
    fill_energy_ds(root, t);
    fill_out_ds(root, "distribution", t, 5, true, false);
    KalbachMann km {mem::handle(root)};
    emit_ae(nm, km, EIN, 6);
  }
  // Correlated
  for (int ip = 1; ip <= 2; ip++) {
    char nm[16];
    std::snprintf(nm, sizeof nm, "corr_%s", ip == 1 ? "hist" : "linlin");
    Table t = make_table(nm, ip, 0, false, true);
    Node root;
    fill_energy_ds(root, t);
    fill_out_ds(root, "energy_out", t, 5, false, true);
    CorrelatedAngleEnergy ca {mem::handle(root)};
    emit_ae(nm, ca, EIN, 6);
  }
  // Uncorrelated: tabulated angle + Maxwell energy; and isotropic (no angle group)
  {
    Node root;
    Node& ang = root.kids["angle"];
    std::vector<double> egrid = {1.0e5, 1.0e6, 2.0e7};
    set_data(ang.kids["energy"], egrid, {3});
    std::vector<double> mx, mp, mc, offs;
    for (size_t i = 0; i < egrid.size(); i++) {
      offs.push_back((double)mx.size());
      double s = (i == 0) ? -0.8 : (i == 1 ? 0.0 : 0.9);
      std::vector<double> x = {-1.0, -0.4, 0.2, 0.7, 1.0}, q(5), cc(5);
      for (int j = 0; j < 5; j++) q[j] = 0.5 + 0.35 * s * x[j];
      cc[0] = 0.0;
      for (int j = 1; j < 5; j++) cc[j] = cc[j - 1] + 0.5 * (q[j - 1] + q[j]) * (x[j] - x[j - 1]);
      double c4 = cc[4];
      for (int j = 0; j < 5; j++) { q[j] /= c4; cc[j] /= c4; }
      mx.insert(mx.end(), x.begin(), x.end()); mp.insert(mp.end(), q.begin(), q.end()); mc.insert(mc.end(), cc.begin(), cc.end());
      std::printf("angle uncorr i=%zu", i);
      print_list("x", x); print_list("p", q); print_list("c", cc);
      std::printf("\n");
    }
    std::printf("angle uncorr_grid"); print_list("egrid", egrid); std::printf("\n");
    Node& mu = ang.kids["mu"];
    set_attr(mu, "offsets", offs);
    set_attr(mu, "interpolation", {2.0, 2.0, 2.0});
    std::vector<double> m;
    m.insert(m.end(), mx.begin(), mx.end()); m.insert(m.end(), mp.begin(), mp.end()); m.insert(m.end(), mc.begin(), mc.end());
    set_data(mu, m, {3, mx.size()});
    Node& en = root.kids["energy"];
    set_str(en, "type", "maxwell");
    set_attr(en, "u", {1.0e4});
    fill_tab1(en, "theta", {1.0e-5, 1.0e6, 2.0e7}, {1.0e5, 2.0e5, 4.0e5});
    UncorrelatedAngleEnergy u {mem::handle(root)};
    const double es[4] = {2.0e5, 1.0e6, 2.7e6, 9.0e6};
    emit_ae("uncorr_maxwell", u, es, 4);

    Node iso;
    Node& en2 = iso.kids["energy"];
    set_str(en2, "type", "maxwell");
    set_attr(en2, "u", {1.0e4});
    fill_tab1(en2, "theta", {1.0e-5, 1.0e6, 2.0e7}, {1.0e5, 2.0e5, 4.0e5});
    UncorrelatedAngleEnergy ui {mem::handle(iso)};
    emit_ae("uncorr_iso_maxwell", ui, es, 4);

    // Same isotropic-angle wrapper around Evaporation and Watt (the port's only
    // public route to those energy laws is the uncorrelated emission).
    Node ev;
    Node& en3 = ev.kids["energy"];
    set_str(en3, "type", "evaporation");
    set_attr(en3, "u", {1.0e4});
    fill_tab1(en3, "theta", {1.0e-5, 1.0e6, 2.0e7}, {1.0e5, 2.0e5, 4.0e5});
    UncorrelatedAngleEnergy uev {mem::handle(ev)};
    emit_ae("uncorr_iso_evaporation", uev, es, 4);
    Node wt;
    Node& en4 = wt.kids["energy"];
    set_str(en4, "type", "watt");
    set_attr(en4, "u", {1.0e4});
    fill_tab1(en4, "a", {1.0e-5, 2.0e7}, {0.8e5, 1.2e5});
    fill_tab1(en4, "b", {1.0e-5, 2.0e7}, {2.5e-6, 3.0e-6});
    UncorrelatedAngleEnergy uwt {mem::handle(wt)};
    const double ew2[3] = {1.0e6, 2.7e6, 9.0e6};
    emit_ae("uncorr_iso_watt", uwt, ew2, 3);
  }
  return 0;
}
