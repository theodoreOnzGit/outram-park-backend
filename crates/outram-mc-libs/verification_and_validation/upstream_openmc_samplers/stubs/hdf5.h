// STUB: HDF5 is not installed. OpenMC's headers only need these two typedefs;
// no HDF5 call is made by the code the sampler differential compiles (data are
// built in memory, see driver.cpp).
#pragma once
#include <cstdint>
typedef int64_t hid_t;
typedef uint64_t hsize_t;
