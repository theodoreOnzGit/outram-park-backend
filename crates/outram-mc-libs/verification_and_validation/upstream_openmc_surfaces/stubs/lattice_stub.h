// STUB (substitution S3, see README): the real include/openmc/lattice.h needs
// pugixml and HDF5. Declares only the data members and the methods whose bodies
// extract.sh copies verbatim from src/lattice.cpp. Member names and types are
// copied by hand from include/openmc/lattice.h.
#pragma once
#include <algorithm>
#include <cmath>
#include <utility>
#include "openmc/array.h"
#include "openmc/position.h"
#include "constants_extract.h"

namespace openmc {

bool isclose(double a, double b, double rel_tol, double abs_tol);
bool coincident(double d1, double d2);

class RectLattice {
public:
  std::pair<double, array<int, 3>> distance(
    Position r, Direction u, const array<int, 3>& i_xyz) const;
  void get_indices(Position r, Direction u, array<int, 3>& result) const;
  Position get_local_position(Position r, const array<int, 3>& i_xyz) const;
  bool is_3d_;
  array<int, 3> n_cells_;
  Position lower_left_;
  Position pitch_;
};

class HexLattice {
public:
  enum class Orientation { y, x };
  std::pair<double, array<int, 3>> distance(
    Position r, Direction u, const array<int, 3>& i_xyz) const;
  void get_indices(Position r, Direction u, array<int, 3>& result) const;
  Position get_local_position(Position r, const array<int, 3>& i_xyz) const;
  bool is_3d_;
  int n_rings_;
  int n_axial_;
  Orientation orientation_;
  Position center_;
  array<double, 2> pitch_;
};

} // namespace openmc
