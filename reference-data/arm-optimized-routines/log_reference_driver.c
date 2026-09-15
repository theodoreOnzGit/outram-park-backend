/* log_reference_driver.c -- oracle generator for PETIR's `fast_log` port.
 *
 * Compiles ARM optimized-routines' own `math/log.c` + `math/log_data.c` and
 * dumps `log` over a fixed probe grid as raw IEEE-754 bit patterns.
 *
 * IMPORTANT -- build WITHOUT FMA (no `-mfma`, no `-march=native`).  See this
 * directory's README: `HAVE_FAST_FMA` selects a different argument reduction
 * AND a different near-1 branch, and GCC contracts the polynomial on top of
 * that.  The portable branch is what `src/fast_log.rs` transcribes.
 *
 * The upstream symbol is `log`, which collides with <math.h>; objcopy renames
 * it after compilation (see the README's regeneration recipe).
 */
#include <stdio.h>
#include <stdint.h>
#include <string.h>

double arm_log (double);

static uint64_t bits (double x) { uint64_t u; memcpy (&u, &x, 8); return u; }

static void emit (double x)
{
  printf ("%016llx %016llx\n", (unsigned long long) bits (x),
          (unsigned long long) bits (arm_log (x)));
}

int
main (void)
{
  long k;
  int e, d;

  puts ("# PETIR fast_log oracle -- ARM optimized-routines math/log.c, built without FMA.");
  puts ("# Columns: <x bits, hex64> <log(x) bits, hex64>.  One probe per line.");

  /* Main grid: every binade from the subnormal floor to the top of the
     range, sampled at several mantissas.  Built by repeated multiplication
     by 2 so no library call is involved -- `pow` is itself under test in
     this directory and must not be the thing that generates its own probes. */
  {
    static const double mant[] = { 1.0, 1.1, 1.3, 1.5, 1.7, 1.9 };
    for (e = -1070; e <= 1020; e++)
      {
        double s = 1.0;
        int i;
        unsigned m;
        if (e >= 0) for (i = 0; i < e; i++) s *= 2.0;
        else        for (i = 0; i < -e; i++) s *= 0.5;
        if (s == 0.0) continue;
        for (m = 0; m < sizeof mant / sizeof *mant; m++)
          {
            double x = s * mant[m];
            if (x > 0.0) emit (x);
          }
      }
  }

  /* The near-1 branch: |x - 1| inside and just outside the
     [1 - 0x1p-4, 1 + 0x1.09p-4] window, where a different polynomial runs. */
  for (k = -2000; k <= 2000; k++)
    emit (1.0 + (double) k * 4.0e-5);

  /* Around the subnormal boundary (2^-1022), where x is renormalised. */
  for (d = 1; d <= 200; d++)
    emit ((double) d * 5.0e-324);

  emit (1.0);
  emit (0.0);
  emit (-0.0);
  emit (-1.0);
  return 0;
}
