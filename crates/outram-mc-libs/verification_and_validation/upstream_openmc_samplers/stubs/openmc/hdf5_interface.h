// STUB of openmc/hdf5_interface.h: an IN-MEMORY tree that implements exactly
// the calls the distribution constructors make (open_group / open_dataset,
// read_attribute, read_dataset, object_exists). HDF5 is not installed; with
// this the REAL upstream constructors and sample() routines run unmodified on
// data the driver puts into the tree, in the same layout `openmc.data` writes.
#pragma once
#include <cstddef>
#include <cstdint>
#include <map>
#include <stdexcept>
#include <string>

#include "hdf5.h"
#include "openmc/tensor.h"
#include "openmc/vector.h"

namespace openmc {
namespace mem {
struct Attr {
  std::vector<double> v;
  std::vector<size_t> shape;
  std::string s;
};
struct Node {
  std::map<std::string, Node> kids;
  std::map<std::string, Attr> attrs;
  Attr data;
};
inline Node& node(hid_t h) { return *reinterpret_cast<Node*>(h); }
inline hid_t handle(Node& n) { return reinterpret_cast<hid_t>(&n); }
} // namespace mem

inline hid_t open_object(hid_t g, const char* name)
{
  auto& k = mem::node(g).kids;
  auto it = k.find(name);
  if (it == k.end())
    throw std::runtime_error(std::string("missing object ") + name);
  return mem::handle(it->second);
}
inline hid_t open_dataset(hid_t g, const char* n) { return open_object(g, n); }
inline hid_t open_group(hid_t g, const char* n) { return open_object(g, n); }
inline void close_dataset(hid_t) {}
inline void close_group(hid_t) {}
inline void close_object(hid_t) {}
inline bool object_exists(hid_t g, const char* n) { return mem::node(g).kids.count(n) != 0; }
inline bool attribute_exists(hid_t g, const char* n) { return mem::node(g).attrs.count(n) != 0; }
inline std::string object_name(hid_t) { return "mem"; }

inline const mem::Attr& attr_of(hid_t h, const char* n)
{
  auto& a = mem::node(h).attrs;
  auto it = a.find(n);
  if (it == a.end())
    throw std::runtime_error(std::string("missing attribute ") + n);
  return it->second;
}

template<class V> void fill_vec(const mem::Attr& a, V& out)
{
  out.clear();
  for (double x : a.v)
    out.push_back(static_cast<typename V::value_type>(x));
}
template<class T> void fill_tensor(const mem::Attr& a, tensor::Tensor<T>& out)
{
  out = tensor::Tensor<T>(a.shape);
  for (size_t i = 0; i < a.v.size(); i++)
    out.data()[i] = static_cast<T>(a.v[i]);
}

inline void read_attribute(hid_t h, const char* n, int& x) { x = static_cast<int>(attr_of(h, n).v.at(0)); }
inline void read_attribute(hid_t h, const char* n, double& x) { x = attr_of(h, n).v.at(0); }
inline void read_attribute(hid_t h, const char* n, std::string& x) { x = attr_of(h, n).s; }
inline void read_attribute(hid_t h, const char* n, vector<int>& x) { fill_vec(attr_of(h, n), x); }
inline void read_attribute(hid_t h, const char* n, vector<double>& x) { fill_vec(attr_of(h, n), x); }
inline void read_attribute(hid_t h, const char* n, tensor::Tensor<int>& x) { fill_tensor(attr_of(h, n), x); }
inline void read_attribute(hid_t h, const char* n, tensor::Tensor<double>& x) { fill_tensor(attr_of(h, n), x); }

inline void read_dataset(hid_t h, vector<double>& x) { fill_vec(mem::node(h).data, x); }
inline void read_dataset(hid_t h, vector<int>& x) { fill_vec(mem::node(h).data, x); }
inline void read_dataset(hid_t h, tensor::Tensor<double>& x) { fill_tensor(mem::node(h).data, x); }
inline void read_dataset(hid_t h, tensor::Tensor<int>& x) { fill_tensor(mem::node(h).data, x); }
inline void read_dataset(hid_t g, const char* n, vector<double>& x) { read_dataset(open_dataset(g, n), x); }
inline void read_dataset(hid_t g, const char* n, tensor::Tensor<double>& x) { read_dataset(open_dataset(g, n), x); }
} // namespace openmc
