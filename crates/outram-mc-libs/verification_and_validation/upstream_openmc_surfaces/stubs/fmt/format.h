// STUB (substitution S1, see README): the real fmt/format.h is not installed
// and OpenMC's position.h only uses it for a fmt::formatter<Position>
// specialisation, which no compared function touches.
#pragma once
#include <string>
namespace fmt {
template<class T, class = void>
struct formatter {
  template<class A, class B>
  auto format(const A&, B&) -> int { return 0; }
};
template<class... A>
std::string format(const char*, A&&...)
{
  return {};
}
} // namespace fmt
