// Transcription driver for Moltres kernels and the GenericMoltresMaterial
// table path (gh:#1400, kernel level).
//
// Upstream: arfc/moltres 3dd2ce7a60f59424d0d8f46748ba4e243d76b671
// (LGPL-2.1) and MOOSE framework/src/utils/LinearInterpolation.C at the
// MOOSE commit d7c4bde named by Moltres' submodule pointer (LGPL-2.1).
//
// Lines marked `UPSTREAM_MOLTRES` are copied verbatim from the pinned Moltres
// tree, lines marked `UPSTREAM_MOOSE` from MOOSE's LinearInterpolation.C;
// check_verbatim.sh greps for every one of them (whitespace-normalised,
// trailing // comment stripped). Everything else is shim: plain vectors in
// place of MOOSE's material-property and coupled-variable machinery, with
// `_test = 1`, `_prec_scale = 1`, `use_exp_form = false`.
//
// NOT reproduced: MOOSE's real quadrature, shape functions, libMesh, PETSc.
// The kernels are evaluated pointwise (one "quadrature point"), which is all
// a cell-centred finite-volume code can be compared against.
//
// Build:  g++ -O2 -ffp-contract=off -o dump_moltres_kernels dump_moltres_kernels.cpp
// Run:    ./dump_moltres_kernels <property_file_dir root, e.g. data/newt_fuel_> > reference_kernels.txt
// Floats are printed as u64 bit patterns in hex.
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

typedef double Real;
typedef std::vector<Real> VariableValue;

static std::string hx(double d)
{
  uint64_t u;
  std::memcpy(&u, &d, 8);
  char b[24];
  std::snprintf(b, sizeof b, "%016llx", (unsigned long long)u);
  return b;
}

// ---- deterministic generator (PCG-free: 64-bit LCG, Knuth MMIX constants) ----
static uint64_t g_state = 0x9E3779B97F4A7C15ULL;
static double rnd(double lo, double hi)
{
  g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
  double u = double(g_state >> 11) / double(1ULL << 53);
  return lo + (hi - lo) * u;
}

// ---- MOOSE LinearInterpolation (extrap = false), the two sample functions ----
struct LinearInterpolation
{
  std::vector<Real> _x, _y;
  bool _extrap = false;

  Real sample(const Real & x) const
  {
    // special case for single function nodes
    if (_x.size() == 1) // UPSTREAM_MOOSE
      return _y[0]; // UPSTREAM_MOOSE
    // endpoint cases
    if (x < _x[0]) // UPSTREAM_MOOSE
      return _y[0]; // UPSTREAM_MOOSE
    if (x >= _x.back()) // UPSTREAM_MOOSE
      return _y.back(); // UPSTREAM_MOOSE
    auto upper = std::upper_bound(_x.begin(), _x.end(), x); // UPSTREAM_MOOSE
    const auto i = std::size_t(std::distance(_x.begin(), upper) - 1);
    return _y[i] + (_y[i + 1] - _y[i]) * (x - _x[i]) / (_x[i + 1] - _x[i]); // UPSTREAM_MOOSE
  }

  Real sampleDerivative(const Real & x) const
  {
    if (x < _x[0]) // UPSTREAM_MOOSE
      return 0.0; // UPSTREAM_MOOSE
    if (x >= _x.back()) // UPSTREAM_MOOSE
      return 0.0; // UPSTREAM_MOOSE
    auto upper = std::upper_bound(_x.begin(), _x.end(), x); // UPSTREAM_MOOSE
    const auto i = std::size_t(std::distance(_x.begin(), upper) - 1);
    return (_y[i + 1] - _y[i]) / (_x[i + 1] - _x[i]); // UPSTREAM_MOOSE
  }
};

// ---- ScalarTransportBase::computeConcentration with use_exp_form = false ----
struct ScalarTransportBase
{
  bool _use_exp_form = false;
  Real computeConcentration(const VariableValue & u, unsigned int qp)
  {
    if (_use_exp_form) // UPSTREAM_MOLTRES
      return std::exp(u[qp]); // UPSTREAM_MOLTRES
    else // UPSTREAM_MOLTRES
      return u[qp]; // UPSTREAM_MOLTRES
  }
};

// ---- the kernels, one quadrature point ----
struct Kernels : ScalarTransportBase
{
  unsigned int _qp = 0, _i = 0, _j = 0;
  std::vector<std::vector<Real>> _test{{1.0}};

  // InScatter
  std::vector<std::vector<Real>> _gtransfxs;
  std::vector<const VariableValue *> _group_fluxes;
  std::vector<unsigned int> _idx;
  unsigned int _group = 0, _num_groups = 0;

  Real inscatter()
  {
    Real r = 0; // UPSTREAM_MOLTRES
    for (unsigned int i = 0; i < _num_groups; ++i) // UPSTREAM_MOLTRES
    {
      if (i == _group || _gtransfxs[_qp][_idx[i]] == 0.) // UPSTREAM_MOLTRES
        continue; // UPSTREAM_MOLTRES
      else // UPSTREAM_MOLTRES
        r += -_test[_i][_qp] * _gtransfxs[_qp][_idx[i]] * // UPSTREAM_MOLTRES
             computeConcentration((*_group_fluxes[i]), _qp); // UPSTREAM_MOLTRES
    }
    return r;
  }

  // CoupledFissionKernel, account_delayed = true
  std::vector<std::vector<Real>> _nsf, _chi_p;
  std::vector<Real> _beta;
  Real _eigenvalue_scaling = 1.0;
  bool _account_delayed = true;

  Real fission()
  {
    Real r = 0; // UPSTREAM_MOLTRES
    for (unsigned int i = 0; i < _num_groups; ++i) // UPSTREAM_MOLTRES
      r += -_nsf[_qp][i] * computeConcentration((*_group_fluxes[i]), _qp); // UPSTREAM_MOLTRES
    if (_account_delayed) // UPSTREAM_MOLTRES
      r *= (1. - _beta[_qp]) * _chi_p[_qp][_group]; // UPSTREAM_MOLTRES
    if ((_eigenvalue_scaling != 1.0)) // UPSTREAM_MOLTRES
      r /= _eigenvalue_scaling; // UPSTREAM_MOLTRES
    return _test[_i][_qp] * r; // UPSTREAM_MOLTRES
  }

  // PrecursorSource (no neutron_source coupling)
  std::vector<std::vector<Real>> _beta_eff;
  unsigned int _precursor_group = 0;
  Real _prec_scale = 1.0;

  Real precursor_source()
  {
    Real r = 0; // UPSTREAM_MOLTRES
    for (unsigned int i = 0; i < _num_groups; ++i) // UPSTREAM_MOLTRES
      r += -_test[_i][_qp] * _beta_eff[_qp][_precursor_group] * _nsf[_qp][i] * // UPSTREAM_MOLTRES
           computeConcentration((*_group_fluxes[i]), _qp) * _prec_scale; // UPSTREAM_MOLTRES
    if ((_eigenvalue_scaling != 1.0)) // UPSTREAM_MOLTRES
      r /= _eigenvalue_scaling; // UPSTREAM_MOLTRES
    return r;
  }

  // DelayedNeutronSource (no delayed_neutron_source coupling)
  std::vector<std::vector<Real>> _decay_constant, _chi_d;
  unsigned int _num_precursor_groups = 0;
  std::vector<const VariableValue *> _pre_concs;

  Real delayed_neutron_source()
  {
    Real r = 0; // UPSTREAM_MOLTRES
    for (unsigned int i = 0; i < _num_precursor_groups; ++i) // UPSTREAM_MOLTRES
      r += -_decay_constant[_qp][i] * computeConcentration((*_pre_concs[i]), _qp); // UPSTREAM_MOLTRES
    return _chi_d[_qp][_group] * _test[_i][_qp] * r; // UPSTREAM_MOLTRES
  }

  // PrecursorDecay
  VariableValue _u;
  Real decay()
  {
    return _test[_i][_qp] * _decay_constant[_qp][_precursor_group] * computeConcentration(_u, _qp) * // UPSTREAM_MOLTRES
           _prec_scale; // UPSTREAM_MOLTRES
  }
};

struct Table
{
  std::vector<Real> temperature;
  std::vector<std::vector<Real>> col;
  std::vector<LinearInterpolation> lin;
};

static bool read_table(const std::string & file, unsigned o, Table & t)
{
  std::ifstream myfile(file.c_str());
  if (!myfile.good())
    return false;
  Real value;
  t.col.assign(o, {});
  while (myfile >> value)
  {
    t.temperature.push_back(value); // shim: upstream pushes to a local `temperature`
    for (unsigned k = 0; k < o; ++k)
    {
      myfile >> value;
      t.col[k].push_back(value); // shim: upstream pushes to _xsec_map[_xsec_names[j]][k]
    }
  }
  t.lin.resize(o);
  for (unsigned k = 0; k < o; ++k)
  {
    t.lin[k]._x = t.temperature;
    t.lin[k]._y = t.col[k];
  }
  return true;
}

int main(int argc, char ** argv)
{
  if (argc < 2)
    return 2;
  const std::string root = argv[1];
  const unsigned G = 2, P = 6;
  // sss2_input = false names, `account_delayed = false` data set newt_fuel_.
  const char * names[] = {"REMXS", "NSF", "DIFFCOEF", "BETA_EFF", "FISSXS", "FISSE",
                          "CHI", "GTRANSFXS", "DECAY_CONSTANT", "RECIPVEL"};
  const unsigned lens[] = {G, G, G, P, G, G, G, G * G, P, G};
  const unsigned N = 4096;

  // K1: table interpolation, value and slope, 200 temperatures per column,
  // spanning below-range, in-range, on-node and above-range points.
  for (unsigned n = 0; n < 10; ++n)
  {
    Table t;
    if (!read_table(root + names[n] + ".txt", lens[n], t))
    {
      std::fprintf(stderr, "cannot read %s%s.txt\n", root.c_str(), names[n]);
      return 1;
    }
    for (unsigned k = 0; k < lens[n]; ++k)
    {
      for (unsigned s = 0; s < 200; ++s)
      {
        double T = rnd(780.0, 1070.0);
        if (s % 17 == 0)
          T = t.temperature[s % t.temperature.size()]; // exactly on a node
        std::printf("TBL %s %u %s %s %s\n", names[n], k, hx(T).c_str(),
                    hx(t.lin[k].sample(T)).c_str(), hx(t.lin[k].sampleDerivative(T)).c_str());
      }
    }
  }

  // K2: InScatter, G = 3, both index layouts.
  for (int sss2 = 0; sss2 < 2; ++sss2)
    for (unsigned s = 0; s < N; ++s)
    {
      Kernels K;
      K._num_groups = 3;
      K._group = s % 3;
      std::vector<Real> gt(9);
      for (auto & v : gt)
        v = (rnd(0, 1) < 0.2) ? 0.0 : rnd(1e-4, 0.4);
      K._gtransfxs = {gt};
      std::vector<VariableValue> ph(3, VariableValue(1));
      for (unsigned i = 0; i < 3; ++i)
      {
        ph[i][0] = rnd(1e-3, 1e3);
        K._group_fluxes.push_back(&ph[i]);
        K._idx.push_back(sss2 ? i * 3 + K._group : i + K._group * 3);
      }
      double out = K.inscatter();
      std::printf("INSC %d %u", sss2, K._group);
      for (auto v : gt)
        std::printf(" %s", hx(v).c_str());
      for (unsigned i = 0; i < 3; ++i)
        std::printf(" %s", hx(ph[i][0]).c_str());
      std::printf(" %s\n", hx(out).c_str());
    }

  // K5: CoupledFissionKernel prompt term, G = 2 and G = 3.
  for (unsigned G2 = 2; G2 <= 3; ++G2)
    for (unsigned s = 0; s < N; ++s)
    {
      Kernels K;
      K._num_groups = G2;
      K._group = s % G2;
      std::vector<Real> nsf(G2), chi(G2);
      std::vector<VariableValue> ph(G2, VariableValue(1));
      for (unsigned i = 0; i < G2; ++i)
      {
        nsf[i] = rnd(1e-3, 0.1);
        chi[i] = rnd(0.0, 1.0);
        ph[i][0] = rnd(1e-3, 1e3);
        K._group_fluxes.push_back(&ph[i]);
      }
      K._nsf = {nsf};
      K._chi_p = {chi};
      K._beta = {rnd(0.003, 0.008)};
      K._eigenvalue_scaling = rnd(0.9, 1.1);
      double out = K.fission();
      std::printf("FISS %u %u", G2, K._group);
      for (auto v : nsf)
        std::printf(" %s", hx(v).c_str());
      for (unsigned i = 0; i < G2; ++i)
        std::printf(" %s", hx(ph[i][0]).c_str());
      std::printf(" %s %s %s %s\n", hx(chi[K._group]).c_str(), hx(K._beta[0]).c_str(),
                  hx(K._eigenvalue_scaling).c_str(), hx(out).c_str());
    }

  // K6: PrecursorSource, G = 2 and G = 3, one family.
  for (unsigned G2 = 2; G2 <= 3; ++G2)
    for (unsigned s = 0; s < N; ++s)
    {
      Kernels K;
      K._num_groups = G2;
      std::vector<Real> nsf(G2);
      std::vector<VariableValue> ph(G2, VariableValue(1));
      for (unsigned i = 0; i < G2; ++i)
      {
        nsf[i] = rnd(1e-3, 0.1);
        ph[i][0] = rnd(1e-3, 1e3);
        K._group_fluxes.push_back(&ph[i]);
      }
      K._nsf = {nsf};
      K._beta_eff = {{rnd(1e-4, 3e-3)}};
      K._eigenvalue_scaling = rnd(0.9, 1.1);
      double out = K.precursor_source();
      std::printf("PSRC %u %s", G2, hx(K._beta_eff[0][0]).c_str());
      for (auto v : nsf)
        std::printf(" %s", hx(v).c_str());
      for (unsigned i = 0; i < G2; ++i)
        std::printf(" %s", hx(ph[i][0]).c_str());
      std::printf(" %s %s\n", hx(K._eigenvalue_scaling).c_str(), hx(out).c_str());
    }

  // K3: DelayedNeutronSource with 6 families; K4: PrecursorDecay.
  for (unsigned s = 0; s < N; ++s)
  {
    Kernels K;
    K._num_precursor_groups = P;
    K._group = 0;
    std::vector<Real> lam(P), chid = {rnd(0.0, 1.0)};
    std::vector<VariableValue> c(P, VariableValue(1));
    for (unsigned i = 0; i < P; ++i)
    {
      lam[i] = rnd(1e-2, 3.0);
      c[i][0] = rnd(1e-3, 1e3);
      K._pre_concs.push_back(&c[i]);
    }
    K._decay_constant = {lam};
    K._chi_d = {chid};
    double out = K.delayed_neutron_source();
    std::printf("DNS %s", hx(chid[0]).c_str());
    for (auto v : lam)
      std::printf(" %s", hx(v).c_str());
    for (unsigned i = 0; i < P; ++i)
      std::printf(" %s", hx(c[i][0]).c_str());
    std::printf(" %s\n", hx(out).c_str());

    K._precursor_group = s % P;
    K._u = c[K._precursor_group];
    std::printf("DEC %s %s %s\n", hx(lam[K._precursor_group]).c_str(),
                hx(c[K._precursor_group][0]).c_str(), hx(K.decay()).c_str());
  }
  return 0;
}
