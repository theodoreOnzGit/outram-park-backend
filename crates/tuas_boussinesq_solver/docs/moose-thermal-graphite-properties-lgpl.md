# MOOSE `ThermalGraphiteProperties` — verbatim source excerpt (LGPL-2.1)

This file reproduces, **verbatim and unmodified**, the MOOSE framework's
graphite thermal-property object, kept here as the reference implementation
the workspace's Butland & Maddison graphite heat capacity is cross-checked
against (see
`nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial` in
`src/lib/boussinesq_thermophysical_properties/solid_database/nuclear_graphite.rs`).

## Attribution and licence

| | |
|---|---|
| Upstream project | MOOSE (Multiphysics Object-Oriented Simulation Environment), <https://mooseframework.inl.gov> |
| Repository | <https://github.com/idaholab/moose> |
| Files | `modules/solid_properties/src/solidproperties/ThermalGraphiteProperties.C`, `modules/solid_properties/include/solidproperties/ThermalGraphiteProperties.h`, `modules/solid_properties/doc/content/source/solidproperties/ThermalGraphiteProperties.md` (the documentation page) |
| Branch / commit | `next`; last commit touching the `.C` file: `9952567b9af429e626c4282119b286ca2c0faf17` (2025-02-14); touching the doc page: `eba6a3ca1bbba8e0ff0e7eee1e6c503578d96289` (2023-06-29) |
| Copyright | Battelle Energy Alliance, LLC (operator of Idaho National Laboratory) — "All rights reserved, see COPYRIGHT for full restrictions", <https://github.com/idaholab/moose/blob/master/COPYRIGHT> |
| Licence | **GNU Lesser General Public License v2.1** (LGPL-2.1), <https://www.gnu.org/licenses/lgpl-2.1.html> |
| Retrieved | 2026-09-28, from `raw.githubusercontent.com` |

The excerpt is redistributed under the terms of the LGPL-2.1, which permits
copying and redistribution provided the copyright notice and licence are kept
(they are: the upstream header is reproduced unchanged at the top of each
file below). LGPL-2.1 code may be combined with this GPL-3.0 workspace
(LGPL-2.1 section 3 allows conversion to the GNU GPL). **No MOOSE code is
compiled into this workspace**: the Rust implementation in `nuclear_graphite.rs`
is written from the published formula (Butland & Maddison 1973/74), and this
file is documentation for comparison only.

## What it contains

- **Specific heat** (`cp_from_T`): `4184 × (0.54212 − 2.42667e-6·T − 90.2725/T
  − 43449.3/T² + 1.59309e7/T³ − 1.43688e9/T⁴)` J/(kg·K), T in K. This is
  **Butland & Maddison's polynomial 3** (A. T. D. Butland and R. J. Maddison,
  "The specific heat of graphite: an evaluation of measurements", *J. Nucl.
  Mater.* 49 (1973/74) 45–56, p. 55), converted with the thermochemical
  calorie (4.184 J). Butland & Maddison state it may be used with confidence
  over **250–3000 K**; MOOSE's code enforces no range.
- Its derivative and antiderivative (`cp_from_T` overload, `cp_integral`).
- **Thermal conductivity** (`k_from_T`): `3.28248e-5·T² − 1.24890e-1·T +
  1.692145e2` W/(m·K), for grade **H-451** only. The documentation page
  (below) cites it to `nea2018` and gives a validity range of **500–1800 K**.
  **Not used in this workspace**: H-451 is a petroleum-coke reflector grade,
  not the HTR-10 pebble matrix (A3) or reflector (IG-110), and its range ends
  below 2000 K, so it does not help the above-2000 K gap.
- **Density**: a constant parameter, default 1850 kg/m³.
- The only grade implemented is `H_451`.

## `ThermalGraphiteProperties.h`

```cpp
//* This file is part of the MOOSE framework
//* https://mooseframework.inl.gov
//*
//* All rights reserved, see COPYRIGHT for full restrictions
//* https://github.com/idaholab/moose/blob/master/COPYRIGHT
//*
//* Licensed under LGPL 2.1, please see LICENSE for details
//* https://www.gnu.org/licenses/lgpl-2.1.html

#pragma once

#include "ThermalSolidProperties.h"

/**
 * Graphite thermal properties as a function of temperature.
 */
class ThermalGraphiteProperties : public ThermalSolidProperties
{
public:
  static InputParameters validParams();

  ThermalGraphiteProperties(const InputParameters & parameters);

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Woverloaded-virtual"

  virtual Real k_from_T(const Real & T) const override;

  virtual void k_from_T(const Real & T, Real & k, Real & dk_dT) const override;

  virtual Real cp_from_T(const Real & T) const override;

  virtual void cp_from_T(const Real & T, Real & cp, Real & dcp_dT) const override;

  virtual Real cp_integral(const Real & T) const override;

  virtual Real rho_from_T(const Real & T) const override;

  virtual void rho_from_T(const Real & T, Real & rho, Real & drho_dT) const override;

protected:
  /// enumeration for selecting the graphite grade
  enum GraphiteGrade
  {
    H_451
  } _grade;

  /// constant density
  const Real & _rho_const;

  // Constants used in specific heat relation
  const Real _c1;
  const Real _c2;
  const Real _c3;
  const Real _c4;
  const Real _c5;
  const Real _c6;
  const Real _c7;
};

#pragma GCC diagnostic pop
```

## `ThermalGraphiteProperties.C`

```cpp
//* This file is part of the MOOSE framework
//* https://mooseframework.inl.gov
//*
//* All rights reserved, see COPYRIGHT for full restrictions
//* https://github.com/idaholab/moose/blob/master/COPYRIGHT
//*
//* Licensed under LGPL 2.1, please see LICENSE for details
//* https://www.gnu.org/licenses/lgpl-2.1.html

#include "ThermalGraphiteProperties.h"
#include "libmesh/utility.h"

registerMooseObject("SolidPropertiesApp", ThermalGraphiteProperties);

InputParameters
ThermalGraphiteProperties::validParams()
{
  InputParameters params = ThermalSolidProperties::validParams();

  MooseEnum graphite_grade("H_451");
  params.addRequiredParam<MooseEnum>("grade", graphite_grade, "Graphite grade");
  params.addRangeCheckedParam<Real>("density", 1850.0, "density > 0.0", "(Constant) density");
  params.addClassDescription("Graphite thermal properties.");
  return params;
}

ThermalGraphiteProperties::ThermalGraphiteProperties(const InputParameters & parameters)
  : ThermalSolidProperties(parameters),
    _grade(getParam<MooseEnum>("grade").getEnum<GraphiteGrade>()),
    _rho_const(getParam<Real>("density")),
    _c1(4184.0),
    _c2(0.54212),
    _c3(2.42667e-6),
    _c4(90.2725),
    _c5(43449.3),
    _c6(1.59309e7),
    _c7(1.43688e9)
{
}

Real
ThermalGraphiteProperties::cp_from_T(const Real & T) const
{
  switch (_grade)
  {
    case GraphiteGrade::H_451:
      return _c1 * (_c2 - _c3 * T - _c4 / T - _c5 / Utility::pow<2>(T) + _c6 / Utility::pow<3>(T) -
                    _c7 / Utility::pow<4>(T));
    default:
      mooseError("Unhandled GraphiteGrade enum!");
  }
}

void
ThermalGraphiteProperties::cp_from_T(const Real & T, Real & cp, Real & dcp_dT) const
{
  cp = cp_from_T(T);

  switch (_grade)
  {
    case GraphiteGrade::H_451:
    {
      dcp_dT = _c1 * (-_c3 + _c4 / Utility::pow<2>(T) + 2.0 * _c5 / Utility::pow<3>(T) -
                      3.0 * _c6 / Utility::pow<4>(T) + 4.0 * _c7 / Utility::pow<5>(T));
      break;
    }
    default:
      mooseError("Unhandled GraphiteGrade enum!");
  }
}

Real
ThermalGraphiteProperties::cp_integral(const Real & T) const
{
  switch (_grade)
  {
    case GraphiteGrade::H_451:
    {
      return _c1 * (_c2 * T - 0.5 * _c3 * Utility::pow<2>(T) - _c4 * std::log(T) + _c5 / T -
                    0.5 * _c6 / Utility::pow<2>(T) + _c7 / (3.0 * Utility::pow<3>(T)));
    }
    default:
      mooseError("Unhandled GraphiteGrade enum!");
  }
}

Real
ThermalGraphiteProperties::k_from_T(const Real & T) const
{
  switch (_grade)
  {
    case GraphiteGrade::H_451:
      return 3.28248e-5 * Utility::pow<2>(T) - 1.24890e-1 * T + 1.692145e2;
    default:
      mooseError("Unhandled GraphiteGrade enum!");
  }
}

void
ThermalGraphiteProperties::k_from_T(const Real & T, Real & k, Real & dk_dT) const
{
  k = k_from_T(T);

  switch (_grade)
  {
    case GraphiteGrade::H_451:
    {
      dk_dT = 6.56496e-5 * T - 1.24890e-1;
      break;
    }
    default:
      mooseError("Unhandled GraphiteGrade enum!");
  }
}

Real
ThermalGraphiteProperties::rho_from_T(const Real & /* T */) const
{
  return _rho_const;
}

void
ThermalGraphiteProperties::rho_from_T(const Real & T, Real & rho, Real & drho_dT) const
{
  rho = rho_from_T(T);
  drho_dT = 0.0;
}
```

## Notes on the documentation page (read before relying on it)

- **Typo in the rendered formula:** the page writes the `43449.3` term as
  `T^{-3}`, so it has two `T^{-3}` terms. The code above (`_c5 /
  Utility::pow<2>(T)`) and Butland & Maddison's paper (polynomial 3, p. 55)
  both have **`43449.3 T^{-2}`**. The workspace follows the paper and the code.
- **Validity range:** the page says 200 K ≤ T ≤ 3500 K for C_p. That is the
  span of the data Butland & Maddison fitted. The authors themselves state the
  polynomial "may only be used with confidence over the range 250 K–3000 K"
  (sect. 4), and the workspace enforces 250–3000 K.
- The page attributes the use of one C_p correlation for many coke-based
  grades to Butland (`butland`) and Baker (`baker`).

## `ThermalGraphiteProperties.md` (documentation page, verbatim)

```markdown
# ThermalGraphiteProperties

!syntax description /SolidProperties/ThermalGraphiteProperties

## Description

This userobject provides thermal properties for graphite
as a function of temperature. Because there are many different
grades of graphite, this userobject computes properties individually
for each grade. Because many grades are encapsulated in this
userobject, the applicability ranges of the correlations are unique to
each grade.

Many of the graphite grades encapsulated in this userobject are coke-based.
Because many coke-based graphite grades show approximately the same specific heat,
it is a reasonable approximation to use the same $C_p$ correlation from
[!cite](butland) for many different grades [!cite](baker).

!include solid_properties_units.md

### H-451

H-451 graphite is a near-isotropic, artificial graphite based on
petroleum coke. H-451 graphite is commonly used for reflectors in nuclear
applications.

Isobaric specific heat is calculated from [!cite](butland) as

\begin{equation}
C_p=4184\left\lbrack 0.54212-2.42667e-6T-90.2725 T^{-1}-43449.3 T^{-3}+1.59309\times 10^7 T^{-3}-1.43688\times 10^9T^{-4}\right\rbrack
\end{equation}

with a validity range of 200 K $\le T \le$ 3500.

The thermal conductivity is calculated from [!cite](nea2018) as

\begin{equation}
k=3.28248\times 10^{-5}T^2-1.24890\times 10^{-1}T+1.69214\times 10^2
\end{equation}

with a validity range of 500 K $\le T \le$ 1800 K.

Density is taken as a constant value; a default value is provided based on
[!cite](nea2018) as

\begin{equation}
\rho=1850.0
\end{equation}

!syntax parameters /SolidProperties/ThermalGraphiteProperties

!syntax inputs /SolidProperties/ThermalGraphiteProperties

!syntax children /SolidProperties/ThermalGraphiteProperties

!bibtex bibliography
```

