// STUB of the cmake-generated openmc/version.h (configured from version.h.in).
// Only the declarations constants.h needs; the values are not used by any
// compared routine.
#pragma once
#include "openmc/array.h"
namespace openmc {
constexpr int VERSION_MAJOR {0};
constexpr int VERSION_MINOR {0};
constexpr int VERSION_RELEASE {0};
constexpr bool VERSION_DEV {true};
constexpr const char* VERSION_COMMIT_COUNT = "0";
constexpr const char* VERSION_COMMIT_HASH = "8f79d9d0";
constexpr std::array<int, 3> VERSION {VERSION_MAJOR, VERSION_MINOR, VERSION_RELEASE};
} // namespace openmc
