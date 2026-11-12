// Driver for PETIR's `rng::lcg` code-to-code comparison (gh:#1338, case 6).
//
// Links OpenMC's `src/random_lcg.cpp` UNMODIFIED (built by run.sh) and prints
// every quantity PETIR's port must reproduce, as hex bit patterns. Output is
// written to stdout; run.sh saves it as reference.txt.
//
// Compiled with: g++ -O2 -std=c++17 -ffp-contract=off -I<openmc>/include
//                driver.cpp <openmc>/src/random_lcg.cpp

#include "openmc/random_lcg.h"

#include <cstdint>
#include <cstdio>
#include <cstring>

using namespace openmc;

static uint64_t bits_of(double d)
{
  uint64_t u;
  std::memcpy(&u, &d, sizeof u);
  return u;
}

// FNV-1a 64 over the 8 little-endian bytes of each value. Mirrored exactly in
// tests/lcg_vs_openmc_code_to_code.rs.
static uint64_t fnv_update(uint64_t h, uint64_t v)
{
  for (int b = 0; b < 8; b++) {
    h ^= (v >> (8 * b)) & 0xffu;
    h *= 0x100000001b3ULL;
  }
  return h;
}

static const uint64_t FNV_OFFSET = 0xcbf29ce484222325ULL;

int main()
{
  const uint64_t seeds[3] = {1ULL, 42ULL, (1ULL << 63) - 1ULL};
  const uint64_t ns[8] = {0ULL,         1ULL,         2ULL,        10ULL,
                          1000ULL,      1ULL << 20,   1ULL << 40,  1ULL << 62};

  std::printf("# OpenMC random_lcg.cpp: commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf\n");
  std::printf("# stride=%llu\n", (unsigned long long)DEFAULT_STRIDE);

  // 1. prn: first 1000 draws from each seed (state after the step, and the
  //    double's bits).
  for (uint64_t seed0 : seeds) {
    uint64_t s = seed0;
    for (int i = 0; i < 1000; i++) {
      double x = prn(&s);
      std::printf("prn seed=%llu i=%d state=%016llx bits=%016llx dec=%.17g\n",
                  (unsigned long long)seed0, i, (unsigned long long)s,
                  (unsigned long long)bits_of(x), x);
    }
  }

  // 2. future_seed(n, seed) for each n and seed.
  for (uint64_t n : ns) {
    for (uint64_t seed0 : seeds) {
      std::printf("future_seed n=%llu seed=%llu out=%016llx\n",
                  (unsigned long long)n, (unsigned long long)seed0,
                  (unsigned long long)future_seed(n, seed0));
    }
  }

  // 3. future_prn(n, seed): the double after n skips.
  for (uint64_t n : ns) {
    for (uint64_t seed0 : seeds) {
      double x = future_prn((int64_t)n, seed0);
      std::printf("future_prn n=%llu seed=%llu bits=%016llx\n",
                  (unsigned long long)n, (unsigned long long)seed0,
                  (unsigned long long)bits_of(x));
    }
  }

  // 4. advance_prn_seed(n, &seed): mutates the state.
  for (uint64_t n : ns) {
    for (uint64_t seed0 : seeds) {
      uint64_t s = seed0;
      advance_prn_seed((int64_t)n, &s);
      std::printf("advance n=%llu seed=%llu out=%016llx\n",
                  (unsigned long long)n, (unsigned long long)seed0,
                  (unsigned long long)s);
    }
  }

  // 5. init_seed(id, offset) under two master seeds (OpenMC keeps the master
  //    in a global, set via openmc_set_seed; PETIR takes it as an argument).
  const int64_t masters[2] = {1, 42};
  const int64_t ids[7] = {0, 1, 2, 3, 10, 1000, 65535};
  for (int64_t m : masters) {
    openmc_set_seed(m);
    for (int64_t id : ids) {
      for (int off = 0; off < N_STREAMS; off++) {
        std::printf("init_seed master=%lld id=%lld offset=%d out=%016llx\n",
                    (long long)m, (long long)id, off,
                    (unsigned long long)init_seed(id, off));
      }
    }
  }
  openmc_set_seed(DEFAULT_SEED);

  // 6. Particle streams: for ids 0..15, the start state init_seed(id, 0) under
  //    master 1 and an FNV-1a digest of the next 10^4 draws from it.
  for (int64_t id = 0; id < 16; id++) {
    uint64_t s = init_seed(id, 0);
    uint64_t h = FNV_OFFSET;
    for (int i = 0; i < 10000; i++) {
      h = fnv_update(h, bits_of(prn(&s)));
    }
    std::printf("stream id=%lld start=%016llx digest_1e4=%016llx\n",
                (long long)id, (unsigned long long)init_seed(id, 0),
                (unsigned long long)h);
  }

  // 7. Full sequential streams: digest of 10^6 draws from each seed, and the
  //    final state.
  for (uint64_t seed0 : seeds) {
    uint64_t s = seed0;
    uint64_t h = FNV_OFFSET;
    for (int i = 0; i < 1000000; i++) {
      h = fnv_update(h, bits_of(prn(&s)));
    }
    std::printf("digest_full seed=%llu n=1000000 digest=%016llx end=%016llx\n",
                (unsigned long long)seed0, (unsigned long long)h,
                (unsigned long long)s);
  }
  return 0;
}
