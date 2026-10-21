! SPDX-License-Identifier: GPL-3.0
!
! Shim for FLEXPART's random_mod.f90, for dev/flexpart_reference_release.f90 ONLY.
!
! Upstream random_mod.f90 holds Numerical Recipes generators (ran1, ran3,
! gasdev, gasdev1). Their licence is not GPL-compatible, so this workspace
! neither ports nor re-implements them (maintainer decision, gh:#410). The
! routines under test (releaseparticles.f90, init_domainfill.f90,
! boundcond_domainfill.f90) call only ran1, to obtain uniform draws. This shim
! returns the values the DRIVER has stored in shim_seq, in order, and counts
! them (shim_pos), so the driver can echo exactly which draws each call
! consumed and the Rust port can be fed the same sequence. It contains no
! generator; only random_mod's interface (integer idum argument, default-real
! result) was read.
module random_mod
  implicit none
  integer, parameter :: shim_n = 400000
  real :: shim_seq(shim_n)
  integer :: shim_pos = 0
contains
  function ran1(idum)
    integer :: idum
    real :: ran1
    shim_pos = shim_pos + 1
    if (shim_pos > shim_n) stop 'random_mod shim: draw sequence exhausted'
    ran1 = shim_seq(shim_pos)
  end function ran1
end module random_mod
