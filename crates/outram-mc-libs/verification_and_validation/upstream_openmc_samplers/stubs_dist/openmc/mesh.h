// STUB of openmc/mesh.h (the real one needs HDF5 and MOAB). distribution_spatial.h
// includes it only for MeshSpatial, which this differential does not compile
// (the port's MeshSource is gh:#264). Declarations only.
#pragma once
#include <cstdint>
#include <memory>
#include <vector>
#include "openmc/position.h"
namespace openmc {
enum class ElementType { UNSUPPORTED = -1, LINEAR_TET, LINEAR_HEX };
class Mesh {
public:
  virtual ~Mesh() = default;
  virtual int n_bins() const = 0;
  virtual double volume(int bin) const = 0;
  virtual Position sample_element(int32_t, uint64_t*) const = 0;
};
class UnstructuredMesh : public Mesh {
public:
  virtual ElementType element_type(int) const = 0;
};
namespace model {
extern std::vector<std::unique_ptr<Mesh>> meshes;
}
} // namespace openmc
