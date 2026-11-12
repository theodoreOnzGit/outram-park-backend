// STUB (substitution S2, see README): the real include/openmc/surface.h needs
// pugixml and HDF5. This declares only the classes, data members and the three
// virtual methods whose bodies extract.sh copies verbatim from src/surface.cpp.
// Member names and types are copied by hand from include/openmc/surface.h.
#pragma once
#include <algorithm>
#include <cmath>
#include <complex>
#include "openmc/external/quartic_solver.h"
#include "openmc/position.h"
#include "constants_extract.h"

namespace openmc {

class Surface {
public:
  virtual ~Surface() {}
  bool sense(Position r, Direction u) const;
  virtual double evaluate(Position r) const = 0;
  virtual double distance(Position r, Direction u, bool coincident) const = 0;
  virtual Direction normal(Position r) const = 0;
};

#define OP_DECL                                                                \
  double evaluate(Position r) const override;                                  \
  double distance(Position r, Direction u, bool coincident) const override;    \
  Direction normal(Position r) const override;

class SurfaceXPlane : public Surface { public: OP_DECL double x0_; };
class SurfaceYPlane : public Surface { public: OP_DECL double y0_; };
class SurfaceZPlane : public Surface { public: OP_DECL double z0_; };
class SurfacePlane : public Surface { public: OP_DECL double A_, B_, C_, D_; };
class SurfaceXCylinder : public Surface { public: OP_DECL double y0_, z0_, radius_; };
class SurfaceYCylinder : public Surface { public: OP_DECL double x0_, z0_, radius_; };
class SurfaceZCylinder : public Surface { public: OP_DECL double x0_, y0_, radius_; };
class SurfaceSphere : public Surface { public: OP_DECL double x0_, y0_, z0_, radius_; };
class SurfaceXCone : public Surface { public: OP_DECL double x0_, y0_, z0_, radius_sq_; };
class SurfaceYCone : public Surface { public: OP_DECL double x0_, y0_, z0_, radius_sq_; };
class SurfaceZCone : public Surface { public: OP_DECL double x0_, y0_, z0_, radius_sq_; };
class SurfaceQuadric : public Surface { public: OP_DECL double A_, B_, C_, D_, E_, F_, G_, H_, J_, K_; };
class SurfaceXTorus : public Surface { public: OP_DECL double x0_, y0_, z0_, A_, B_, C_; };
class SurfaceYTorus : public Surface { public: OP_DECL double x0_, y0_, z0_, A_, B_, C_; };
class SurfaceZTorus : public Surface { public: OP_DECL double x0_, y0_, z0_, A_, B_, C_; };

} // namespace openmc
