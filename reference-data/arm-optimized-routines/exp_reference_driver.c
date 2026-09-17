/* exp_reference_driver.c -- oracle generator for PETIR's `fast_exp` port.
 *
 * Compiles ARM optimized-routines' own `math/exp.c` + `math/exp_data.c` and
 * dumps `exp` over a fixed probe grid as raw IEEE-754 bit patterns, so the
 * Rust port can be checked bit-for-bit rather than to a tolerance.
 *
 * IMPORTANT -- build WITHOUT FMA (no `-mfma`, no `-march=native`).  Upstream's
 * portable `#else` branch is what this port transcribes, and GCC will
 * contract `r2 * (C2 + r * C3)` into an FMA when the hardware allows it,
 * which changes the last bit on ~0.05 % of inputs.  See this directory's
 * README.
 *
 * The upstream symbol is `exp`, which collides with <math.h>; objcopy renames
 * it to `arm_exp` after compilation (see the README's regeneration recipe).
 */
#include <stdio.h>
#include <stdint.h>
#include <string.h>

double arm_exp (double);

static uint64_t bits (double x) { uint64_t u; memcpy (&u, &x, 8); return u; }

static void emit (double x)
{
  printf ("%016llx %016llx\n", (unsigned long long) bits (x),
          (unsigned long long) bits (arm_exp (x)));
}

int
main (void)
{
  long k;
  int e, d;

  puts ("# PETIR fast_exp oracle -- ARM optimized-routines math/exp.c, built without FMA.");
  puts ("# Columns: <x bits, hex64> <exp(x) bits, hex64>.  One probe per line.");

  /* Main grid: every 5th point of the Rust probe set, [-700, 700]. */
  for (k = -20000; k <= 20000; k += 5)
    emit ((double) k * 0.035);

  /* Powers of two on both signs -- crosses the tiny-x early return. */
  for (e = -60; e <= 9; e++)
    {
      double s = 1.0;
      int i;
      if (e >= 0) for (i = 0; i < e; i++) s *= 2.0;
      else        for (i = 0; i < -e; i++) s *= 0.5;
      emit (s);
      emit (-s);
    }

  /* The overflow (709.78..) and underflow (-745.13..) boundaries, where
     `specialcase` runs and the subnormal rounding step matters. */
  for (d = -50; d <= 50; d++)
    {
      emit (709.0 + (double) d * 0.02);
      emit (-745.0 + (double) d * 0.02);
    }

  emit (0.0);
  emit (-0.0);
  return 0;
}
