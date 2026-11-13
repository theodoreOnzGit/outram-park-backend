// Transcription driver for MOOSE's J2 radial return with linear hardening
// (gh:#1434, case 3).  Upstream: idaholab/moose fe80d24fa4c58f677d6672e5063ec08715953c32
// (LGPL-2.1): modules/solid_mechanics/src/materials/
//   RadialReturnStressUpdate.C            (updateState, computeStressInitialize,
//                                          computeReferenceResidual, maximumPermissibleValue)
//   IsotropicPlasticityStressUpdate.C     (residual, derivative, hardening)
//   SingleVariableReturnMappingSolution.C (internalSolve, converged, updateBounds)
// and framework/include/utils/RankFourTensorImplementation.h
// (fillSymmetricIsotropicEandNu, fillSymmetricIsotropic).
//
// Every line marked UPSTREAM is verbatim (check_verbatim.sh, whitespace
// normalised).  The rest is a shim: RankTwoTensor is a 9-array; deviatoric()
// and tr() are written out as upstream does (addIa(-1/3 tr)); doubleContraction
// and `RankFourTensor * RankTwoTensor` are plain sequential sums.  Upstream
// computes the latter with Eigen (RankFourTensorImplementation.h:134-143) and
// libMesh's TensorValue::contract, whose summation orders are NOT reproduced
// here, so bit-identity with a compiled MOOSE is not claimed.
//
// Build:  g++ -O2 -ffp-contract=off -o dump_moose_j2 dump_moose_j2.cpp
// Run:    ./dump_moose_j2 > reference_moose_j2.txt        (floats: u64 hex)
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <string>
#include <vector>

typedef double Real;

struct T2 {  // RankTwoTensor shim, row-major 3x3
  Real c[9];
  T2() { for (int i = 0; i < 9; i++) c[i] = 0.0; }
  Real& operator()(int i, int j) { return c[3 * i + j]; }
  Real operator()(int i, int j) const { return c[3 * i + j]; }
  Real tr() const { return c[0] + c[4] + c[8]; }
  void addIa(const Real& a) { for (int i = 0; i < 3; i++) (*this)(i, i) += a; }
  T2 deviatoric() const {
    T2 deviatoric(*this);  // shim (type changed from GenericReal/RankTwoTensor); not marked verbatim
    deviatoric.addIa(-1.0 / 3.0 * this->tr());  // UPSTREAM
    return deviatoric;  // UPSTREAM
  }
  Real doubleContraction(const T2& b) const {
    Real s = 0.0;
    for (int i = 0; i < 9; i++) s += c[i] * b.c[i];
    return s;
  }
  T2& operator+=(const T2& a) { for (int i = 0; i < 9; i++) c[i] += a.c[i]; return *this; }
  T2& operator-=(const T2& a) { for (int i = 0; i < 9; i++) c[i] -= a.c[i]; return *this; }
  T2 operator+(const T2& a) const { T2 r(*this); r += a; return r; }
  T2 operator*(Real s) const { T2 r; for (int i = 0; i < 9; i++) r.c[i] = c[i] * s; return r; }
};

struct T4 {  // RankFourTensor shim, flat 81 (ij*9 + kl)
  Real v[81];
  T4() { for (int i = 0; i < 81; i++) v[i] = 0.0; }
  void fillSymmetricIsotropic(const Real& lambda, const Real& G) {
    for (int i = 0; i < 81; i++) v[i] = 0.0;
    const Real l2g = lambda + 2.0 * G;  // shim: upstream array entry `lambda + 2.0 * G`
    for (int i = 0; i < 3; i++)
      for (int k = 0; k < 3; k++) {
        // C_iikk
        v[(3 * i + i) * 9 + (3 * k + k)] = (i == k) ? l2g : lambda;
      }
    for (int i = 0; i < 3; i++)
      for (int j = 0; j < 3; j++)
        if (i != j) {
          v[(3 * i + j) * 9 + (3 * i + j)] = G;
          v[(3 * i + j) * 9 + (3 * j + i)] = G;
        }
  }
  void fillSymmetricIsotropicEandNu(const Real& E, const Real& nu) {
    const Real& lambda = E * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));  // shim: upstream declares const T-ref here (RankFourTensorImplementation.h:755-756)
    const Real& G = E / (2.0 * (1.0 + nu));  // shim: upstream declares const T-ref here (RankFourTensorImplementation.h:755-756)
    fillSymmetricIsotropic(lambda, G);
  }
  T2 operator*(const T2& b) const {  // sequential kl sum (upstream: Eigen)
    T2 result;
    for (int ij = 0; ij < 9; ij++) {
      Real tmp = 0;
      for (int kl = 0; kl < 9; kl++) tmp += v[ij * 9 + kl] * b.c[kl];
      result.c[ij] = tmp;
    }
    return result;
  }
};

static std::string hexs(Real x) {
  uint64_t b;
  std::memcpy(&b, &x, 8);
  char buf[32];
  std::snprintf(buf, sizeof buf, "%016llx", (unsigned long long)b);
  return buf;
}

namespace MathUtils {
inline Real sign(Real x) { return x < 0.0 ? -1.0 : 1.0; }
inline Real clamp(Real x, Real lo, Real hi) { return x < lo ? lo : (x > hi ? hi : x); }
}  // namespace MathUtils

// ---- the stress-update object: the members the transcribed lines name -----
struct Update {
  const int _qp = 0;
  Real _yield_stress, _hardening_constant, _yield_condition, _hardening_slope, _three_shear_modulus;
  std::vector<Real> _hardening_variable, _hardening_variable_old;
  Real _effective_inelastic_strain_increment;
  // SingleVariableReturnMappingSolution state
  bool _check_range = false, _line_search = true, _bracket_solution = true;
  unsigned _max_its = 1000;
  Real _relative_tolerance = 1e-8, _absolute_tolerance = 1e-11, _acceptable_multiplier = 10;
  unsigned _num_resids = 30, _iteration = 0;
  std::vector<Real> _residual_history;
  Real _residual = 0.0, _derivative = 0.0, _initial_residual = 0.0;
  int iterations_used = 0;

  Update() : _hardening_variable(1, 0.0), _hardening_variable_old(1, 0.0),
             _residual_history(30, std::numeric_limits<Real>::max()) {}

  // IsotropicPlasticityStressUpdate
  Real computeHardeningDerivative(const Real&) { return _hardening_constant; }
  Real computeHardeningValue(const Real& scalar) {
    return _hardening_variable_old[_qp] + _hardening_slope * scalar;  // UPSTREAM
  }
  Real computeResidual(const Real& effective_trial_stress, const Real& scalar) {
    if (_yield_condition > 0.0)  // UPSTREAM
    {  // UPSTREAM
      _hardening_slope = computeHardeningDerivative(scalar);  // UPSTREAM
      _hardening_variable[_qp] = computeHardeningValue(scalar);  // UPSTREAM
      return (effective_trial_stress - _hardening_variable[_qp] - _yield_stress) /  // UPSTREAM
             _three_shear_modulus -  // UPSTREAM
             scalar;  // UPSTREAM
    }  // UPSTREAM
    return 0.0;  // UPSTREAM
  }
  Real computeDerivative(const Real&, const Real&) {
    if (_yield_condition > 0.0)  // UPSTREAM
      return -1.0 - _hardening_slope / _three_shear_modulus;  // UPSTREAM
    return 1.0;  // UPSTREAM
  }
  void iterationFinalize(const Real& scalar) {
    if (_yield_condition > 0.0)  // UPSTREAM
      _hardening_variable[_qp] = computeHardeningValue(scalar);  // UPSTREAM
  }
  // RadialReturnStressUpdate
  Real computeReferenceResidual(const Real& effective_trial_stress, const Real& scalar_effective_inelastic_strain) {
    return effective_trial_stress / _three_shear_modulus - scalar_effective_inelastic_strain;
    // (upstream: raw_value(effective_trial_stress / _three_shear_modulus) - raw_value(scalar...))
  }
  Real minimumPermissibleValue(const Real&) const { return 0.0; }  // shim (type changed from GenericReal/RankTwoTensor); not marked verbatim
  Real maximumPermissibleValue(const Real& effective_trial_stress) const {
    return effective_trial_stress / _three_shear_modulus;  // UPSTREAM
  }
  Real initialGuess(const Real&) { return 0.0; }  // shim (type changed from GenericReal/RankTwoTensor); not marked verbatim

  void computeResidualAndDerivativeHelper(const Real& effective_trial_stress, const Real& scalar) {
    _residual = computeResidual(effective_trial_stress, scalar);  // UPSTREAM
    _derivative = computeDerivative(effective_trial_stress, scalar);  // UPSTREAM
  }
  bool converged(const Real& residual, const Real reference) {
    return (std::abs(residual) <= _absolute_tolerance ||  // UPSTREAM
            std::abs(residual / reference) <= _relative_tolerance);  // UPSTREAM
  }
  bool convergedAcceptable(const unsigned int it, const Real reference) {
    if (it < _num_resids)  // UPSTREAM
      return false;  // UPSTREAM
    const Real convergence_history_factor = 10.0;  // UPSTREAM
    if (std::abs(_residual * convergence_history_factor) < std::abs(_residual_history[(it + 1) % _num_resids]))
      return false;
    return converged(_residual / _acceptable_multiplier, reference);  // UPSTREAM
  }
  void updateBounds(const Real& scalar, const Real& residual, const Real init_resid_sign, Real& scalar_upper_bound,
                    Real& scalar_lower_bound) {
    if (residual * init_resid_sign < 0.0 && scalar < scalar_upper_bound)  // UPSTREAM
    {  // UPSTREAM
      scalar_upper_bound = scalar;  // UPSTREAM
      if (scalar_upper_bound < scalar_lower_bound)  // UPSTREAM
      {  // UPSTREAM
        scalar_upper_bound = scalar_lower_bound;  // UPSTREAM
        scalar_lower_bound = 0.0;  // UPSTREAM
      }  // UPSTREAM
    }  // UPSTREAM
    else if (residual * init_resid_sign > 0.0 && scalar > scalar_lower_bound &&  // UPSTREAM
             scalar < scalar_upper_bound)  // UPSTREAM
      scalar_lower_bound = scalar;  // UPSTREAM
  }

  // SingleVariableReturnMappingSolution::internalSolve (output and the
  // range-check branch, off by default, removed)
  void internalSolve(const Real effective_trial_stress, Real& scalar) {
    scalar = initialGuess(effective_trial_stress);  // UPSTREAM
    Real scalar_old = scalar;  // shim (type changed from GenericReal/RankTwoTensor); not marked verbatim
    Real scalar_increment = 0.0;
    const Real min_permissible_scalar = minimumPermissibleValue(effective_trial_stress);
    const Real max_permissible_scalar = maximumPermissibleValue(effective_trial_stress);
    Real scalar_upper_bound = max_permissible_scalar;
    Real scalar_lower_bound = min_permissible_scalar;
    _iteration = 0;  // UPSTREAM
    computeResidualAndDerivativeHelper(effective_trial_stress, scalar);  // UPSTREAM
    _initial_residual = _residual;  // UPSTREAM
    Real residual_old = _residual;
    Real init_resid_sign = MathUtils::sign(_residual);
    Real reference_residual = computeReferenceResidual(effective_trial_stress, scalar);  // UPSTREAM
    if (converged(_residual, reference_residual))  // UPSTREAM
    {  // UPSTREAM
      iterationFinalize(scalar);  // UPSTREAM
      iterations_used = 0;
      return;  // (upstream: SolveState::SUCCESS)
    }  // UPSTREAM
    _residual_history.assign(_num_resids, std::numeric_limits<Real>::max());  // UPSTREAM
    _residual_history[0] = _residual;
    while (_iteration < _max_its && !converged(_residual, reference_residual) &&  // UPSTREAM
           !convergedAcceptable(_iteration, reference_residual))  // UPSTREAM
    {  // UPSTREAM
      scalar_increment = -_residual / _derivative;  // UPSTREAM
      scalar = scalar_old + scalar_increment;  // UPSTREAM
      computeResidualAndDerivativeHelper(effective_trial_stress, scalar);  // UPSTREAM
      reference_residual = computeReferenceResidual(effective_trial_stress, scalar);  // UPSTREAM
      iterationFinalize(scalar);  // UPSTREAM
      if (_bracket_solution)  // UPSTREAM
        updateBounds(scalar, _residual, init_resid_sign, scalar_upper_bound, scalar_lower_bound);
      if (converged(_residual, reference_residual))  // UPSTREAM
      {  // UPSTREAM
        break;  // UPSTREAM
      }  // UPSTREAM
      else  // UPSTREAM
      {  // UPSTREAM
        bool modified_increment = false;  // UPSTREAM
        if (_line_search)  // UPSTREAM
        {  // UPSTREAM
          if (residual_old - _residual != 0.0)  // UPSTREAM
          {  // UPSTREAM
            Real alpha = residual_old / (residual_old - _residual);  // shim (type changed from GenericReal/RankTwoTensor); not marked verbatim
            alpha = MathUtils::clamp(alpha, 1.0e-2, 1.0);  // UPSTREAM
            if (alpha != 1.0)  // UPSTREAM
            {  // UPSTREAM
              modified_increment = true;  // UPSTREAM
              scalar_increment *= alpha;  // UPSTREAM
            }  // UPSTREAM
          }  // UPSTREAM
        }  // UPSTREAM
        if (_bracket_solution)  // UPSTREAM
        {  // UPSTREAM
          if (scalar_old + scalar_increment >= scalar_upper_bound ||  // UPSTREAM
              scalar_old + scalar_increment <= scalar_lower_bound)  // UPSTREAM
          {  // UPSTREAM
            if (scalar_upper_bound != max_permissible_scalar &&  // UPSTREAM
                scalar_lower_bound != min_permissible_scalar)  // UPSTREAM
            {  // UPSTREAM
              const Real frac = 0.5;  // UPSTREAM
              scalar_increment =  // UPSTREAM
                  (1.0 - frac) * scalar_lower_bound + frac * scalar_upper_bound - scalar_old;  // UPSTREAM
              modified_increment = true;  // UPSTREAM
            }  // UPSTREAM
          }  // UPSTREAM
        }  // UPSTREAM
        if (modified_increment)  // UPSTREAM
        {  // UPSTREAM
          scalar = scalar_old + scalar_increment;  // UPSTREAM
          computeResidualAndDerivativeHelper(effective_trial_stress, scalar);  // UPSTREAM
          reference_residual = computeReferenceResidual(effective_trial_stress, scalar);  // UPSTREAM
          iterationFinalize(scalar);  // UPSTREAM
          if (_bracket_solution)  // UPSTREAM
            updateBounds(scalar, _residual, init_resid_sign, scalar_upper_bound, scalar_lower_bound);
        }  // UPSTREAM
      }  // UPSTREAM
      ++_iteration;  // UPSTREAM
      residual_old = _residual;  // UPSTREAM
      scalar_old = scalar;  // UPSTREAM
      _residual_history[_iteration % _num_resids] = _residual;
    }  // UPSTREAM
    iterations_used = (int)_iteration;
  }

  // RadialReturnStressUpdate::updateState (single substep, no tangent)
  void updateState(T2& strain_increment, T2& inelastic_strain_increment, T2& stress_new, const T4& elasticity_tensor,
                   const T2& elastic_strain_old, T2& plastic_strain, Real& effective_inelastic_strain) {
    T2 deviatoric_trial_stress = stress_new.deviatoric();
    Real dev_trial_stress_squared = deviatoric_trial_stress.doubleContraction(deviatoric_trial_stress);
    Real effective_trial_stress = dev_trial_stress_squared ? std::sqrt(3.0 / 2.0 * dev_trial_stress_squared) : 0.0;
    // computeStressInitialize (Isotropic...): base sets 3G, then yield condition
    _three_shear_modulus = 3.0 * elasticity_tensor.v[(3 * 0 + 1) * 9 + (3 * 0 + 1)];
    _yield_condition = effective_trial_stress - _hardening_variable_old[_qp] - _yield_stress;  // UPSTREAM
    _hardening_variable[_qp] = _hardening_variable_old[_qp];  // UPSTREAM
    _effective_inelastic_strain_increment = 0.0;  // UPSTREAM
    if (!(std::abs(effective_trial_stress - 0.0) <= 1.0e-12))  // MooseUtils::absoluteFuzzyEqual(x, 0.0)
    {
      internalSolve(effective_trial_stress, _effective_inelastic_strain_increment);
      if (_effective_inelastic_strain_increment != 0.0)  // UPSTREAM
        inelastic_strain_increment =
            deviatoric_trial_stress * (1.5 * _effective_inelastic_strain_increment / effective_trial_stress);  // shim (type changed from GenericReal/RankTwoTensor); not marked verbatim
      else
        inelastic_strain_increment = T2();
    }
    else
      inelastic_strain_increment = T2();
    strain_increment -= inelastic_strain_increment;  // UPSTREAM
    effective_inelastic_strain = effective_inelastic_strain + _effective_inelastic_strain_increment;
    stress_new = elasticity_tensor * (strain_increment + elastic_strain_old);  // UPSTREAM
    plastic_strain += inelastic_strain_increment;  // (computeStressFinalize)
  }
};

int main() {
  const Real E = 200.0e9, nu = 0.3, sy0 = 250.0e6, H = 1.0e9;
  T4 C;
  C.fillSymmetricIsotropicEandNu(E, nu);
  Update u;
  u._yield_stress = sy0;
  u._hardening_constant = H;
  std::printf("JP %s %s %s %s\n", hexs(E).c_str(), hexs(nu).c_str(), hexs(sy0).c_str(), hexs(H).c_str());

  // Strain path: total strain = a(t) * Shape; a rises 0 -> 0.006 in 24 steps,
  // then reverses to -0.004 in 36 steps.
  const Real shape[9] = {1.0, 0.2, 0.0, 0.2, -0.35, 0.1, 0.0, 0.1, -0.3};
  T2 elastic_strain_old, plastic_strain, total_old;
  Real eps_eff = 0.0, a_old = 0.0;
  for (int step = 1; step <= 60; step++) {
    Real a = (step <= 24) ? 0.006 * step / 24.0 : 0.006 - 0.010 * (step - 24) / 36.0;
    T2 strain_increment;
    for (int i = 0; i < 9; i++) strain_increment.c[i] = (a - a_old) * shape[i];
    T2 elastic_strain = elastic_strain_old + strain_increment;
    T2 stress_new = C * elastic_strain;  // trial, as ComputeMultipleInelasticStress
    T2 inelastic;
    u._hardening_variable_old[0] = u._hardening_variable[0];
    u.updateState(strain_increment, inelastic, stress_new, C, elastic_strain_old, plastic_strain, eps_eff);
    elastic_strain_old = elastic_strain_old + strain_increment;  // strain_increment now elastic part
    a_old = a;
    std::printf("J %d %s", step, hexs(a).c_str());
    const int idx[6] = {0, 4, 8, 5, 2, 1};  // 11 22 33 23 13 12
    for (int k = 0; k < 6; k++) std::printf(" %s", hexs(stress_new.c[idx[k]]).c_str());
    for (int k = 0; k < 6; k++) std::printf(" %s", hexs(plastic_strain.c[idx[k]]).c_str());
    std::printf(" %s %d %d\n", hexs(eps_eff).c_str(), u._yield_condition > 0.0 ? 1 : 0, u.iterations_used);
  }
  return 0;
}
