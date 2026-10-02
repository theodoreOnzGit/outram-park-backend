! SPDX-License-Identifier: GPL-3.0
!
! Shim for FLEXPART's random_mod.f90, for dev/flexpart_reference_advance.f90 ONLY.
!
! Upstream random_mod.f90 holds Numerical Recipes generators (ran1, ran3,
! gasdev, gasdev1). Their licence is not GPL-compatible, so this workspace
! neither ports nor re-implements them (maintainer decision, gh:#410). The
! routines under test (advance.f90, initialize.f90, initialize_cbl_vel.f90)
! call them only to obtain draws. This shim returns values the DRIVER sets, so
! the Fortran and the Rust port consume identical numbers; the routines under
! test stay verbatim. It contains no generator; only random_mod's interface
! (integer idum argument, default-real result) was read.
module random_mod
  implicit none
  real :: shim_ran3 = 0.5
  real :: shim_gasdev = 0.0
contains
  function ran3(idum)
    integer :: idum
    real :: ran3
    ran3 = shim_ran3
  end function ran3

  function gasdev(idum)
    integer :: idum
    real :: gasdev
    gasdev = shim_gasdev
  end function gasdev
end module random_mod
