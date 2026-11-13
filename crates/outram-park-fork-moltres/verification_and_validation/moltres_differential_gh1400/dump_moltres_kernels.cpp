// Transcription driver for Moltres kernels and the GenericMoltresMaterial
// table path (gh:#1400, kernel level).
//
// Upstream: arfc/moltres 3dd2ce7a60f59424d0d8f46748ba4e243d76b671
// (LGPL-2.1) and MOOSE framework/src/utils/LinearInterpolation.C at the
// MOOSE commit d7c4bde named by Moltres' submodule pointer (LGPL-2.1).
// gh:#1538 (round 2): the full MOOSE sha is d7c4bde4025537ac4574cc80e78c2407e23a42cc
// (resolved from the Moltres 3dd2ce7 tree); added SplineInterpolation(+Base),
// MonotoneCubicInterpolation, BicubicSplineInterpolation (natural ends) and the
// least-squares branch of GenericMoltresMaterial (K9-K12). Moltres uses
// BicubicSplineInterpolation, not BicubicInterpolation.
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
#include <limits>
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

// ---- MOOSE SplineInterpolationBase / SplineInterpolation (gh:#1538) ----
static const Real _deriv_bound = std::numeric_limits<Real>::max();

struct SplineInterpolation
{
  std::vector<Real> _x, _y, _y2;

  static void spline(const std::vector<Real> & x,
                     const std::vector<Real> & y,
                     std::vector<Real> & y2,
                     Real yp1 = _deriv_bound,
                     Real ypn = _deriv_bound)
  {
    auto n = x.size();
    std::vector<Real> u(n, 0.); // UPSTREAM_MOOSE
    y2.assign(n, 0.); // UPSTREAM_MOOSE
    if (yp1 >= 1e30) // UPSTREAM_MOOSE
      y2[0] = u[0] = 0.; // UPSTREAM_MOOSE
    else
    {
      y2[0] = -0.5; // UPSTREAM_MOOSE
      u[0] = (3.0 / (x[1] - x[0])) * ((y[1] - y[0]) / (x[1] - x[0]) - yp1); // UPSTREAM_MOOSE
    }
    for (decltype(n) i = 1; i < n - 1; i++) // UPSTREAM_MOOSE
    {
      Real sig = (x[i] - x[i - 1]) / (x[i + 1] - x[i - 1]); // UPSTREAM_MOOSE
      Real p = sig * y2[i - 1] + 2.0; // UPSTREAM_MOOSE
      y2[i] = (sig - 1.0) / p; // UPSTREAM_MOOSE
      u[i] = (y[i + 1] - y[i]) / (x[i + 1] - x[i]) - (y[i] - y[i - 1]) / (x[i] - x[i - 1]); // UPSTREAM_MOOSE
      u[i] = (6.0 * u[i] / (x[i + 1] - x[i - 1]) - sig * u[i - 1]) / p; // UPSTREAM_MOOSE
    }
    Real qn, un; // UPSTREAM_MOOSE
    if (ypn >= 1e30) // UPSTREAM_MOOSE
      qn = un = 0.; // UPSTREAM_MOOSE
    else
    {
      qn = 0.5; // UPSTREAM_MOOSE
      un = (3.0 / (x[n - 1] - x[n - 2])) * (ypn - (y[n - 1] - y[n - 2]) / (x[n - 1] - x[n - 2])); // UPSTREAM_MOOSE
    }
    y2[n - 1] = (un - qn * u[n - 2]) / (qn * y2[n - 2] + 1.); // UPSTREAM_MOOSE
    for (auto k = n - 1; k >= 1; k--) // UPSTREAM_MOOSE
      y2[k - 1] = y2[k - 1] * y2[k] + u[k - 1]; // UPSTREAM_MOOSE
  }

  static void findInterval(const std::vector<Real> & x,
                           Real x_int,
                           unsigned int & klo,
                           unsigned int & khi)
  {
    klo = 0; // UPSTREAM_MOOSE
    khi = x.size() - 1; // UPSTREAM_MOOSE
    while (khi - klo > 1) // UPSTREAM_MOOSE
    {
      unsigned int k = (khi + klo) >> 1; // UPSTREAM_MOOSE
      if (x[k] > x_int) // UPSTREAM_MOOSE
        khi = k; // UPSTREAM_MOOSE
      else
        klo = k; // UPSTREAM_MOOSE
    }
  }

  static void computeCoeffs(const std::vector<Real> & x,
                            unsigned int klo,
                            unsigned int khi,
                            const Real & x_int,
                            Real & h,
                            Real & a,
                            Real & b)
  {
    h = x[khi] - x[klo]; // UPSTREAM_MOOSE
    a = (x[khi] - x_int) / h; // UPSTREAM_MOOSE
    b = (x_int - x[klo]) / h; // UPSTREAM_MOOSE
  }

  static Real sampleIn(const std::vector<Real> & x,
                       const std::vector<Real> & y,
                       const std::vector<Real> & y2,
                       const Real & x_int,
                       unsigned int klo,
                       unsigned int khi)
  {
    Real h; // UPSTREAM_MOOSE
    Real a, b;
    computeCoeffs(x, klo, khi, x_int, h, a, b);
    return a * y[klo] + b * y[khi] + // UPSTREAM_MOOSE
           ((a * a * a - a) * y2[klo] + (b * b * b - b) * y2[khi]) * (h * h) / 6.0; // UPSTREAM_MOOSE
  }

  static Real sampleBase(const std::vector<Real> & x,
                         const std::vector<Real> & y,
                         const std::vector<Real> & y2,
                         Real x_int)
  {
    unsigned int klo, khi; // UPSTREAM_MOOSE
    findInterval(x, x_int, klo, khi);
    return sampleIn(x, y, y2, x_int, klo, khi);
  }

  static Real sampleDerivativeBase(const std::vector<Real> & x,
                                   const std::vector<Real> & y,
                                   const std::vector<Real> & y2,
                                   Real x_int)
  {
    unsigned int klo, khi;
    findInterval(x, x_int, klo, khi);
    Real h, a, b; // UPSTREAM_MOOSE
    computeCoeffs(x, klo, khi, x_int, h, a, b);
    return (y[khi] - y[klo]) / h - // UPSTREAM_MOOSE
           (((3.0 * a * a - 1.0) * y2[klo] + (3.0 * b * b - 1.0) * -y2[khi]) * h / 6.0); // UPSTREAM_MOOSE
  }

  void setData(const std::vector<Real> & x, const std::vector<Real> & y)
  {
    _x = x;
    _y = y;
    spline(_x, _y, _y2);
  }
  Real sample(Real x) const { return sampleBase(_x, _y, _y2, x); }
  Real sampleDerivative(Real x) const { return sampleDerivativeBase(_x, _y, _y2, x); }
};

// ---- MOOSE MonotoneCubicInterpolation (gh:#1538) ----
struct MonotoneCubicInterpolation
{
  std::vector<Real> _x, _y, _h, _yp, _delta, _alpha, _beta;
  unsigned int _n_knots, _n_intervals, _internal_knots;

  Real sign(const Real & x) const
  {
    if (x < 0) // UPSTREAM_MOOSE
      return -1; // UPSTREAM_MOOSE
    else if (x > 0) // UPSTREAM_MOOSE
      return 1; // UPSTREAM_MOOSE
    else
      return 0; // UPSTREAM_MOOSE
  }
  Real phi(const Real & t) const
  {
    return 3. * t * t - 2. * t * t * t; // UPSTREAM_MOOSE
  }
  Real phiPrime(const Real & t) const
  {
    return 6. * t - 6. * t * t; // UPSTREAM_MOOSE
  }
  Real psi(const Real & t) const
  {
    return t * t * t - t * t; // UPSTREAM_MOOSE
  }
  Real psiPrime(const Real & t) const
  {
    return 3. * t * t - 2. * t; // UPSTREAM_MOOSE
  }

  Real h1(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo; // UPSTREAM_MOOSE
    Real t = (xhi - x) / h; // UPSTREAM_MOOSE
    return phi(t); // UPSTREAM_MOOSE
  }
  Real h1Prime(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (xhi - x) / h;
    Real tPrime = -1. / h; // UPSTREAM_MOOSE
    return phiPrime(t) * tPrime; // UPSTREAM_MOOSE
  }
  Real h2(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (x - xlo) / h; // UPSTREAM_MOOSE
    return phi(t);
  }
  Real h2Prime(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (x - xlo) / h;
    Real tPrime = 1. / h; // UPSTREAM_MOOSE
    return phiPrime(t) * tPrime;
  }
  Real h3(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (xhi - x) / h;
    return -h * psi(t); // UPSTREAM_MOOSE
  }
  Real h3Prime(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (xhi - x) / h;
    Real tPrime = -1. / h;
    return -h * psiPrime(t) * tPrime; // UPSTREAM_MOOSE
  }
  Real h4(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (x - xlo) / h;
    return h * psi(t); // UPSTREAM_MOOSE
  }
  Real h4Prime(const Real & xhi, const Real & xlo, const Real & x) const
  {
    Real h = xhi - xlo;
    Real t = (x - xlo) / h;
    Real tPrime = 1. / h;
    return h * psiPrime(t) * tPrime; // UPSTREAM_MOOSE
  }
  Real p(const Real & xhi, const Real & xlo, const Real & fhi, const Real & flo,
         const Real & dhi, const Real & dlo, const Real & x) const
  {
    return flo * h1(xhi, xlo, x) + fhi * h2(xhi, xlo, x) + dlo * h3(xhi, xlo, x) + // UPSTREAM_MOOSE
           dhi * h4(xhi, xlo, x); // UPSTREAM_MOOSE
  }
  Real pPrime(const Real & xhi, const Real & xlo, const Real & fhi, const Real & flo,
              const Real & dhi, const Real & dlo, const Real & x) const
  {
    return flo * h1Prime(xhi, xlo, x) + fhi * h2Prime(xhi, xlo, x) + dlo * h3Prime(xhi, xlo, x) + // UPSTREAM_MOOSE
           dhi * h4Prime(xhi, xlo, x); // UPSTREAM_MOOSE
  }

  void initialize_derivs()
  {
    for (unsigned int i = 1; i < _n_knots - 1; ++i) // UPSTREAM_MOOSE
      _yp[i] = (std::pow(_h[i - 1], 2) * _y[i + 1] - std::pow(_h[i], 2) * _y[i - 1] - // UPSTREAM_MOOSE
                _y[i] * (_h[i - 1] - _h[i]) * (_h[i - 1] + _h[i])) / // UPSTREAM_MOOSE
               (_h[i - 1] * _h[i] * (_h[i - 1] * _h[i])); // UPSTREAM_MOOSE

    _yp[0] = (-std::pow(_h[0], 2) * _y[2] - _h[1] * _y[0] * (2 * _h[0] + _h[1]) + // UPSTREAM_MOOSE
              _y[1] * std::pow(_h[0] + _h[1], 2)) / // UPSTREAM_MOOSE
             (_h[0] * _h[1] * (_h[0] + _h[1])); // UPSTREAM_MOOSE

    Real hlast = _h[_n_intervals - 1]; // UPSTREAM_MOOSE
    Real hsecond = _h[_n_intervals - 2]; // UPSTREAM_MOOSE
    Real ylast = _y[_n_knots - 1]; // UPSTREAM_MOOSE
    Real ysecond = _y[_n_knots - 2]; // UPSTREAM_MOOSE
    Real ythird = _y[_n_knots - 3]; // UPSTREAM_MOOSE
    _yp[_n_knots - 1] = (hsecond * ylast * (hsecond + 2 * hlast) + std::pow(hlast, 2) * ythird - // UPSTREAM_MOOSE
                         ysecond * std::pow(hsecond + hlast, 2)) / // UPSTREAM_MOOSE
                        (hsecond * hlast * (hsecond + hlast)); // UPSTREAM_MOOSE
  }

  void modify_derivs(const Real & alpha, const Real & beta, const Real & delta, Real & yp_lo, Real & yp_hi)
  {
    Real tau = 3. / std::sqrt(std::pow(alpha, 2) + std::pow(beta, 2)); // UPSTREAM_MOOSE
    Real alpha_star = alpha * tau; // UPSTREAM_MOOSE
    Real beta_star = beta * tau; // UPSTREAM_MOOSE
    yp_lo = alpha_star * delta; // UPSTREAM_MOOSE
    yp_hi = beta_star * delta; // UPSTREAM_MOOSE
  }

  void setData(const std::vector<Real> & x, const std::vector<Real> & y)
  {
    _x = x;
    _y = y;
    _n_knots = _x.size(), _n_intervals = _x.size() - 1, _internal_knots = _x.size() - 2; // UPSTREAM_MOOSE
    _h.resize(_n_intervals);
    _yp.resize(_n_knots);
    _delta.resize(_n_intervals);
    _alpha.resize(_n_intervals);
    _beta.resize(_n_intervals);

    for (unsigned int i = 0; i < _n_intervals; ++i)
      _h[i] = _x[i + 1] - _x[i]; // UPSTREAM_MOOSE

    initialize_derivs();
    for (unsigned int i = 0; i < _n_intervals; ++i)
      _delta[i] = (_y[i + 1] - _y[i]) / _h[i]; // UPSTREAM_MOOSE
    if (sign(_delta[0]) != sign(_yp[0])) // UPSTREAM_MOOSE
      _yp[0] = 0; // UPSTREAM_MOOSE
    if (sign(_delta[_n_intervals - 1]) != sign(_yp[_n_knots - 1])) // UPSTREAM_MOOSE
      _yp[_n_knots - 1] = 0; // UPSTREAM_MOOSE
    for (unsigned int i = 0; i < _internal_knots; ++i) // UPSTREAM_MOOSE
      if (sign(_delta[i + 1]) == 0 || sign(_delta[i]) == 0 || sign(_delta[i + 1]) != sign(_delta[i])) // UPSTREAM_MOOSE
        _yp[1 + i] = 0; // UPSTREAM_MOOSE

    for (unsigned int i = 0; i < _n_intervals; ++i) // UPSTREAM_MOOSE
    {
      if (_yp[i] == 0) // UPSTREAM_MOOSE
        _alpha[i] = 0; // UPSTREAM_MOOSE
      else if (_delta[i] == 0) // UPSTREAM_MOOSE
        _alpha[i] = 4; // UPSTREAM_MOOSE
      else
        _alpha[i] = _yp[i] / _delta[i]; // UPSTREAM_MOOSE

      if (_yp[i + 1] == 0) // UPSTREAM_MOOSE
        _beta[i] = 0; // UPSTREAM_MOOSE
      else if (_delta[i] == 0) // UPSTREAM_MOOSE
        _beta[i] = 4; // UPSTREAM_MOOSE
      else
        _beta[i] = _yp[i + 1] / _delta[i]; // UPSTREAM_MOOSE

      if (std::pow(_alpha[i], 2) + std::pow(_beta[i], 2) > 9.) // UPSTREAM_MOOSE
        modify_derivs(_alpha[i], _beta[i], _delta[i], _yp[i], _yp[i + 1]); // UPSTREAM_MOOSE
    }
  }

  void findInterval(const Real & x, unsigned int & klo, unsigned int & khi) const
  {
    klo = 0; // UPSTREAM_MOOSE
    khi = _n_knots - 1; // UPSTREAM_MOOSE
    while (khi - klo > 1) // UPSTREAM_MOOSE
    {
      unsigned int k = (khi + klo) >> 1; // UPSTREAM_MOOSE
      if (_x[k] > x) // UPSTREAM_MOOSE
        khi = k; // UPSTREAM_MOOSE
      else
        klo = k; // UPSTREAM_MOOSE
    }
  }
  Real sample(const Real & x) const
  {
    unsigned int klo, khi; // UPSTREAM_MOOSE
    findInterval(x, klo, khi); // UPSTREAM_MOOSE
    return p(_x[khi], _x[klo], _y[khi], _y[klo], _yp[khi], _yp[klo], x); // UPSTREAM_MOOSE
  }
  Real sampleDerivative(const Real & x) const
  {
    unsigned int klo, khi;
    findInterval(x, klo, khi);
    return pPrime(_x[khi], _x[klo], _y[khi], _y[klo], _yp[khi], _yp[klo], x); // UPSTREAM_MOOSE
  }
};

// ---- MOOSE BicubicSplineInterpolation, natural ends (gh:#1538) ----
struct BicubicSplineInterpolation
{
  std::vector<Real> _x1, _x2;
  std::vector<std::vector<Real>> _y, _y_trans, _y2_rows, _y2_columns;
  std::vector<Real> _row_spline_second_derivs, _row_spline_eval;
  std::vector<Real> _column_spline_second_derivs, _column_spline_eval;

  void setData(const std::vector<Real> & x1, const std::vector<Real> & x2,
               const std::vector<std::vector<Real>> & y)
  {
    _x1 = x1;
    _x2 = x2;
    _y = y;
    auto n = _x2.size(); // UPSTREAM_MOOSE
    _row_spline_second_derivs.resize(n); // UPSTREAM_MOOSE
    _column_spline_eval.resize(n); // UPSTREAM_MOOSE
    auto m = _x1.size(); // UPSTREAM_MOOSE
    _column_spline_second_derivs.resize(m); // UPSTREAM_MOOSE
    _row_spline_eval.resize(m); // UPSTREAM_MOOSE
    // errorCheck() fills the defaulted end slopes with _deriv_bound, so the
    // `else` branches of upstream's construct*SplineSecondDerivativeTable run
    // with natural ends.
    _y2_rows.resize(m); // UPSTREAM_MOOSE
    for (decltype(m) j = 0; j < m; ++j) // UPSTREAM_MOOSE
      SplineInterpolation::spline(_x2, _y[j], _y2_rows[j], _deriv_bound, _deriv_bound);
    _y2_columns.resize(n); // UPSTREAM_MOOSE
    _y_trans.resize(n); // UPSTREAM_MOOSE
    for (decltype(n) j = 0; j < n; ++j) // UPSTREAM_MOOSE
      _y_trans[j].resize(m); // UPSTREAM_MOOSE
    for (decltype(n) i = 0; i < _y.size(); ++i) // UPSTREAM_MOOSE
      for (decltype(n) j = 0; j < _y[0].size(); ++j) // UPSTREAM_MOOSE
        _y_trans[j][i] = _y[i][j]; // UPSTREAM_MOOSE
    for (decltype(n) j = 0; j < n; ++j) // UPSTREAM_MOOSE
      SplineInterpolation::spline(_x1, _y_trans[j], _y2_columns[j], _deriv_bound, _deriv_bound);
  }

  void constructRowSpline(Real x1, std::vector<Real> & column_spline_eval,
                          std::vector<Real> & row_spline_second_derivs)
  {
    auto n = _x2.size(); // UPSTREAM_MOOSE
    unsigned int klo, khi; // UPSTREAM_MOOSE
    SplineInterpolation::findInterval(_x1, x1, klo, khi);
    for (decltype(n) j = 0; j < n; ++j) // UPSTREAM_MOOSE
      _column_spline_eval[j] = // UPSTREAM_MOOSE
          SplineInterpolation::sampleIn(_x1, _y_trans[j], _y2_columns[j], x1, klo, khi);
    SplineInterpolation::spline(_x2, column_spline_eval, row_spline_second_derivs, _deriv_bound, _deriv_bound);
  }

  void constructColumnSpline(Real x2, std::vector<Real> & row_spline_eval,
                             std::vector<Real> & column_spline_second_derivs)
  {
    auto m = _x1.size(); // UPSTREAM_MOOSE
    unsigned int klo, khi; // UPSTREAM_MOOSE
    SplineInterpolation::findInterval(_x2, x2, klo, khi);
    for (decltype(m) j = 0; j < m; ++j) // UPSTREAM_MOOSE
      _row_spline_eval[j] = // shim: upstream calls SplineInterpolationBase::sample(..., klo, khi)
          SplineInterpolation::sampleIn(_x2, _y[j], _y2_rows[j], x2, klo, khi);
    SplineInterpolation::spline(_x1, row_spline_eval, column_spline_second_derivs, _deriv_bound, _deriv_bound);
  }

  Real sample(Real x1, Real x2)
  {
    constructColumnSpline(x2, _row_spline_eval, _column_spline_second_derivs);
    return SplineInterpolation::sampleBase(_x1, _row_spline_eval, _column_spline_second_derivs, x1);
  }

  Real sampleDerivative(Real x1, Real x2, unsigned int deriv_var)
  {
    if (deriv_var == 1) // UPSTREAM_MOOSE
    {
      constructColumnSpline(x2, _row_spline_eval, _column_spline_second_derivs);
      return SplineInterpolation::sampleDerivativeBase(
          _x1, _row_spline_eval, _column_spline_second_derivs, x1);
    }
    else
    {
      constructRowSpline(x1, _column_spline_eval, _row_spline_second_derivs);
      return SplineInterpolation::sampleDerivativeBase(
          _x2, _column_spline_eval, _row_spline_second_derivs, x2);
    }
  }
};

struct Table
{
  std::vector<Real> temperature;
  std::vector<std::vector<Real>> col;
  std::vector<LinearInterpolation> lin;
  std::vector<SplineInterpolation> spl;
  std::vector<MonotoneCubicInterpolation> mon;
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
  t.spl.resize(o);
  t.mon.resize(o);
  for (unsigned k = 0; k < o; ++k)
  {
    t.spl[k].setData(t.temperature, t.col[k]);
    t.mon[k].setData(t.temperature, t.col[k]);
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
        // gh:#1538: same temperatures, spline and monotone cubic
        std::printf("TBLS %s %u %s %s %s\n", names[n], k, hx(T).c_str(),
                    hx(t.spl[k].sample(T)).c_str(), hx(t.spl[k].sampleDerivative(T)).c_str());
        std::printf("TBLM %s %u %s %s %s\n", names[n], k, hx(T).c_str(),
                    hx(t.mon[k].sample(T)).c_str(), hx(t.mon[k].sampleDerivative(T)).c_str());
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
  // K11 (gh:#1538): BicubicSplineInterpolation, natural ends, a fixed smooth
  // non-polynomial surface on non-uniform 5 x 4 grids; value and both slopes
  // at 4096 points, including outside the grid.
  {
    const std::vector<Real> x1 = {780.0, 840.0, 905.0, 990.0, 1070.0};
    const std::vector<Real> x2 = {550.0, 640.0, 760.0, 900.0};
    std::vector<std::vector<Real>> y(x1.size(), std::vector<Real>(x2.size()));
    for (unsigned i = 0; i < x1.size(); ++i)
      for (unsigned j = 0; j < x2.size(); ++j)
        y[i][j] = std::sin(x1[i] / 300.0) * std::exp(-x2[j] / 700.0) + 1e-3 * x1[i] * x2[j] / 1e3;
    BicubicSplineInterpolation b;
    b.setData(x1, x2, y);
    std::printf("BIGRID");
    for (auto v : x1)
      std::printf(" %s", hx(v).c_str());
    std::printf(" |");
    for (auto v : x2)
      std::printf(" %s", hx(v).c_str());
    for (unsigned i = 0; i < x1.size(); ++i)
      for (unsigned j = 0; j < x2.size(); ++j)
        std::printf(" %s", hx(y[i][j]).c_str());
    std::printf("\n");
    for (unsigned s = 0; s < N; ++s)
    {
      double a = rnd(740.0, 1110.0), c = rnd(500.0, 960.0);
      std::printf("BICUB %s %s %s %s %s\n", hx(a).c_str(), hx(c).c_str(), hx(b.sample(a, c)).c_str(),
                  hx(b.sampleDerivative(a, c, 1)).c_str(), hx(b.sampleDerivative(a, c, 2)).c_str());
    }
  }

  // K12 (gh:#1538): least-squares branch, `c0 * T + c1` per
  // leastSquaresComputeQpProperties; the slope is c0.
  for (unsigned s = 0; s < N; ++s)
  {
    std::vector<std::vector<Real>> _remxs_consts(2, std::vector<Real>(1));
    std::vector<std::vector<Real>> _fisse_consts(2, std::vector<Real>(1));
    std::vector<Real> _temperature(1);
    unsigned i = 0, _qp = 0;
    _remxs_consts[0][0] = rnd(-1e-5, 1e-5);
    _remxs_consts[1][0] = rnd(1e-3, 0.1);
    _fisse_consts[0][0] = rnd(-1e-3, 1e-3);
    _fisse_consts[1][0] = rnd(190.0, 200.0);
    _temperature[0] = rnd(780.0, 1100.0);
    double remxs =
        _remxs_consts[0][i] * _temperature[_qp] + _remxs_consts[1][i]; // UPSTREAM_MOLTRES
    double fisse =
        (_fisse_consts[0][i] * _temperature[_qp] + _fisse_consts[1][i]) * 1e6 * // UPSTREAM_MOLTRES
        1.6e-19; // UPSTREAM_MOLTRES
    double dfisse =
        _fisse_consts[0][i] * 1e6 * 1.6e-19; // UPSTREAM_MOLTRES
    std::printf("LSQ %s %s %s %s %s %s\n", hx(_remxs_consts[0][0]).c_str(),
                hx(_remxs_consts[1][0]).c_str(), hx(_fisse_consts[0][0]).c_str(),
                hx(_fisse_consts[1][0]).c_str(), hx(_temperature[0]).c_str(),
                (hx(remxs) + " " + hx(fisse) + " " + hx(dfisse)).c_str());
  }
  return 0;
}
