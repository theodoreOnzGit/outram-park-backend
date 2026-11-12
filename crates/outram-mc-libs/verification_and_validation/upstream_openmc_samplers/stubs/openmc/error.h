// STUB of openmc/error.h (the real one pulls settings.h, capi.h and fmt).
// fatal_error / warning keep their upstream signatures; the sampler differential
// never reaches them on valid data (the driver aborts if it does).
#pragma once
#include <cstdio>
#include <cstdlib>
#include <string>
#define UNREACHABLE() __builtin_unreachable()
namespace openmc {
[[noreturn]] inline void fatal_error(const std::string& m) { std::fprintf(stderr, "fatal_error: %s\n", m.c_str()); std::abort(); }
[[noreturn]] inline void fatal_error(const char* m) { fatal_error(std::string(m)); }
inline void warning(const std::string&) {}
inline void write_message(const std::string&, int = 0) {}
} // namespace openmc
