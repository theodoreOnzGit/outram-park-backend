// STUB of openmc/xml_interface.h (the real one pulls tensor.h; its .cpp pulls fmt).
// Same signatures and semantics as xml_interface.h / xml_interface.cpp for the
// calls the distribution sources make; plumbing only, not under test.
#pragma once
#include <algorithm>
#include <cctype>
#include <sstream>
#include <string>
#include "pugixml.hpp"
#include "openmc/error.h"
#include "openmc/position.h"
#include "openmc/vector.h"
namespace openmc {
inline bool check_for_node(pugi::xml_node node, const char* name)
{
  return node.attribute(name) || node.child(name);
}
inline std::string get_node_value(pugi::xml_node node, const char* name, bool lowercase = false, bool strip = false)
{
  std::string value;
  if (node.attribute(name)) value = node.attribute(name).value();
  else if (node.child(name)) value = node.child_value(name);
  else fatal_error(std::string("Node \"") + name + "\" is not a member of the XML node");
  if (lowercase) std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) { return std::tolower(c); });
  if (strip) {
    value.erase(0, value.find_first_not_of(" \t\r\n"));
    value.erase(value.find_last_not_of(" \t\r\n") + 1);
  }
  return value;
}
inline bool get_node_value_bool(pugi::xml_node node, const char* name)
{
  if (node.attribute(name)) return node.attribute(name).as_bool();
  fatal_error("bool node missing");
}
template<typename T>
vector<T> get_node_array(pugi::xml_node node, const char* name, bool lowercase = false)
{
  std::string s {get_node_value(node, name, lowercase)};
  std::stringstream iss {s};
  T value;
  vector<T> values;
  while (iss >> value) values.push_back(value);
  return values;
}
inline std::vector<Position> get_node_position_array(pugi::xml_node node, const char* name, bool lowercase = false)
{
  vector<double> coords = get_node_array<double>(node, name, lowercase);
  if (coords.size() % 3 != 0) fatal_error("bad position array");
  vector<Position> positions;
  for (size_t i = 0; i < coords.size(); i += 3) positions.push_back({coords[i], coords[i + 1], coords[i + 2]});
  return positions;
}
} // namespace openmc
