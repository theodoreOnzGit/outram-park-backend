// STUB of pugixml for the source-distribution differential (gh:#1388): a tiny
// in-memory XML node with exactly the calls distribution.cpp and
// distribution_spatial.cpp make (attribute, child, children, child_value, name).
// The driver builds trees by hand, so upstream's REAL XML constructors run.
#pragma once
#include <map>
#include <memory>
#include <string>
#include <vector>
namespace pugi {
typedef char char_t;
struct attr_t {
  bool ok = false;
  std::string v;
  explicit operator bool() const { return ok; }
  const char* value() const { return v.c_str(); }
  bool as_bool() const { return v == "true" || v == "1"; }
};
struct text_t {
  std::string v;
  bool as_bool() const { return v == "true" || v == "1"; }
};
class xml_node {
public:
  struct Data {
    std::string name;
    std::map<std::string, std::string> attrs;
    std::vector<std::pair<std::string, xml_node>> kids;
  };
  std::shared_ptr<Data> d;
  xml_node() = default;
  explicit xml_node(const std::string& name) : d(std::make_shared<Data>()) { d->name = name; }
  explicit operator bool() const { return (bool)d; }
  xml_node& set(const std::string& k, const std::string& v) { d->attrs[k] = v; return *this; }
  xml_node& add(const std::string& k, xml_node n) { d->kids.push_back({k, n}); return *this; }
  attr_t attribute(const char* n) const {
    attr_t a;
    if (d) { auto it = d->attrs.find(n); if (it != d->attrs.end()) { a.ok = true; a.v = it->second; } }
    return a;
  }
  xml_node child(const char* n) const {
    if (d) for (auto& k : d->kids) if (k.first == n) return k.second;
    return xml_node();
  }
  std::vector<xml_node> children(const char* n) const {
    std::vector<xml_node> out;
    if (d) for (auto& k : d->kids) if (k.first == n) out.push_back(k.second);
    return out;
  }
  const char* child_value(const char* n) const {
    xml_node c = child(n);
    return (c && c.d->attrs.count("text")) ? c.d->attrs["text"].c_str() : "";
  }
  text_t text() const { return {d && d->attrs.count("text") ? d->attrs["text"] : ""}; }
  const char* name() const { return d ? d->name.c_str() : ""; }
};
} // namespace pugi
