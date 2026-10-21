! SPDX-License-Identifier: GPL-3.0
!
! Stand-in for FLEXPART's random_mod.f90, for the convection reference harness
! (dev/flexpart_reference_convection.f90) ONLY.
!
! WHY: redist.f90 does `use random_mod` and draws its one uniform number per
! particle with `rn = ran3(iseed)`. random_mod.f90 is Numerical Recipes code
! (ran1/ran3/gasdev): it is NOT ported and NOT reimplemented in this project
! (licence; maintainer decision gh:#410). The Rust port therefore takes the
! draws as an input, and the code-to-code comparison needs upstream redist to
! consume the SAME draws. This module provides a `ran3` with upstream's
! interface (integer idum in/out, default-real result; only the interface was
! read from random_mod.f90) that returns the next value of a queue the driver
! fills, and counts how many were taken. redist.f90 itself is compiled
! verbatim; only the source of its random numbers is replaced.
module random_mod
  implicit none
  integer, parameter :: shim_max = 100000
  real :: shim_draws(shim_max)
  integer :: shim_n = 0, shim_next = 0
contains
  function ran3(idum)
    integer :: idum
    real :: ran3
    shim_next = shim_next + 1
    if (shim_next > shim_n) then
      write(*,*) 'random_mod shim: draw queue exhausted'
      stop 1
    end if
    ran3 = shim_draws(shim_next)
  end function ran3
end module random_mod
