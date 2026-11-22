// Driver for the OpenMC-compatible seeding differential (gh:#1386 sec. 2 item 1).
// Links OpenMC src/random_lcg.cpp UNMODIFIED, and the verbatim extract of
// overall_generation / compute_particle_id / compute_transport_seed. Stubs below
// replace the globals those three read. Prints hex bits.
#include "openmc/random_lcg.h"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

namespace openmc {
namespace settings { bool use_shared_secondary_bank = false; int64_t n_particles = 0; int gen_per_batch = 1; bool ifp_on() { return false; } }
namespace simulation { std::vector<int64_t> work_index = {0}; int64_t simulation_tracks_completed = 0; int current_batch = 1; int current_gen = 1; int total_gen = 0; std::vector<int> fission_bank; }
namespace mpi { int rank = 0; }
// init_particle_seeds is defined in random_lcg.cpp but not declared in the header.
void init_particle_seeds(int64_t id, uint64_t* seeds);
void copy_ifp_data_from_fission_banks(int64_t, int64_t, std::vector<std::vector<int>>&, std::vector<std::vector<double>>&) {}
}
#include "simulation_seed_extract.inc"
#include "comb_extract.inc"
#include "openmc/position.h"
#include "openmc/random_dist.h"
#include "openmc/constants.h"
#include "isotropic_extract.inc"

using namespace openmc;
static uint64_t bits_of(double d) { uint64_t u; std::memcpy(&u, &d, sizeof u); return u; }

int main()
{
  std::printf("# OpenMC init_particle_seeds / compute_*_seed: commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf\n");
  // 1. init_particle_seeds(id, seeds): masters x ids, all N_STREAMS seeds.
  const int64_t masters[5] = {1, 42, -3, (int64_t)1 << 62, 9223372036854775807LL - 8};
  const int64_t ids[12] = {0, 1, 2, 3, 10, 1000, 65535, 152917, 1000000007LL,
                           (int64_t)1 << 40, ((int64_t)1 << 50) + 12345, 9007199254740993LL};
  for (int64_t m : masters) {
    openmc_set_seed(m);
    for (int64_t id : ids) {
      uint64_t s[N_STREAMS];
      init_particle_seeds(id, s);
      std::printf("seeds master=%lld id=%lld", (long long)m, (long long)id);
      for (int i = 0; i < N_STREAMS; i++) std::printf(" s%d=%016llx", i, (unsigned long long)s[i]);
      uint64_t t = s[STREAM_TRACKING], u = s[STREAM_URR_PTABLE];
      double a = prn(&t); prn(&t); double b = prn(&t);
      double c = prn(&u);
      std::printf(" trk1=%016llx trk3=%016llx urr1=%016llx\n", (unsigned long long)bits_of(a),
                  (unsigned long long)bits_of(b), (unsigned long long)bits_of(c));
    }
  }
  // Non-default stride.
  openmc_set_seed(1);
  openmc_set_stride(1000003);
  for (int64_t id : {0LL, 1LL, 7LL, 123456LL}) {
    uint64_t s[N_STREAMS]; init_particle_seeds(id, s);
    std::printf("seeds_stride1000003 master=1 id=%lld", (long long)id);
    for (int i = 0; i < N_STREAMS; i++) std::printf(" s%d=%016llx", i, (unsigned long long)s[i]);
    std::printf("\n");
  }
  openmc_set_stride(DEFAULT_STRIDE);

  // 2. compute_particle_id / compute_transport_seed.
  const int64_t nps[2] = {1000, 5000};
  for (int shared = 0; shared < 2; shared++) {
    for (int64_t np : nps) {
      settings::n_particles = np;
      settings::use_shared_secondary_bank = shared != 0;
      for (int rank_case = 0; rank_case < 2; rank_case++) {
        simulation::work_index = rank_case == 0 ? std::vector<int64_t>{0, np}
                                                : std::vector<int64_t>{0, np / 4, np / 2, 3 * np / 4, np};
        mpi::rank = rank_case == 0 ? 0 : 1;
        simulation::simulation_tracks_completed = shared ? 17 : 0;
        for (int gpb : {1, 5}) {
          settings::gen_per_batch = gpb;
          for (int total_gen : {0, 30}) {
            simulation::total_gen = total_gen;
            for (int batch : {1, 2, 7}) for (int gen : {1, gpb}) {
              simulation::current_batch = batch; simulation::current_gen = gen;
              for (int64_t is : std::vector<int64_t>{1, 2, 500, np / 4}) {
                int64_t pid = compute_particle_id(is);
                int64_t ts = compute_transport_seed(pid);
                std::printf("ids shared=%d np=%lld rank=%d gpb=%d total_gen=%d batch=%d gen=%d index=%lld pid=%lld tseed=%lld\n",
                            shared, (long long)np, rank_case, gpb, total_gen, batch, gen, (long long)is,
                            (long long)pid, (long long)ts);
              }
            }
          }
        }
      }
    }
  }
  // 3. init_particle_seeds(id)[k] == init_seed(id, k) for k < N_STREAMS.
  openmc_set_seed(1);
  for (int64_t id = 0; id < 64; id++) {
    uint64_t s[N_STREAMS]; init_particle_seeds(id, s);
    for (int k = 0; k < N_STREAMS; k++)
      if (s[k] != init_seed(id, k)) { std::printf("MISMATCH init_seed id=%lld k=%d\n", (long long)id, k); return 1; }
  }
  std::printf("identity init_particle_seeds==init_seed ok ids=64 streams=%d\n", N_STREAMS);

  // 4. synchronize_bank's uniform comb (verbatim block), single rank, bank[i] = i.
  settings::gen_per_batch = 1;
  settings::n_particles = 1000;
  for (int64_t total : {937LL, 999LL, 1000LL, 1203LL, 2999LL, 5000LL})
    for (int total_gen : {0, 30})
      for (int batch : {1, 2, 7, 50}) {
        openmc_set_seed(1);
        simulation::total_gen = total_gen;
        simulation::current_batch = batch;
        simulation::current_gen = 1;
        simulation::fission_bank.resize(total);
        for (int64_t i = 0; i < total; i++) simulation::fission_bank[i] = (int)i;
        std::vector<int> idx = comb_block(total);
        std::printf("comb total=%lld n=%lld total_gen=%d batch=%d count=%zu idx=", (long long)total,
                    (long long)settings::n_particles, total_gen, batch, idx.size());
        for (size_t i = 0; i < idx.size(); i++) std::printf("%s%d", i ? "," : "", idx[i]);
        std::printf("\n");
      }

  // 5. isotropic_direction (verbatim): azimuth first, then the cosine.
  for (int s = 1; s <= 24; s++) {
    uint64_t seed = (uint64_t)s * 7919u + 11;
    Direction d = isotropic_direction(&seed);
    std::printf("isodir seed=%d x=%016llx y=%016llx z=%016llx end=%016llx\n", s,
                (unsigned long long)bits_of(d.x), (unsigned long long)bits_of(d.y),
                (unsigned long long)bits_of(d.z), (unsigned long long)seed);
  }
  return 0;
}
