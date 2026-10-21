! SPDX-License-Identifier: GPL-3.0
!
! Minimal stand-in for FLEXPART's gributils/class_gribfile_mod.f90, for the
! reference harness ONLY.
!
! WHY: obukhov.f90 does `use class_gribfile`, but the only thing it takes from
! that module is the integer PARAMETER GRIBFILE_CENTRE_ECMWF. The real module
! additionally `USE grib_api`, pulling in ecCodes, which this harness has no
! need for and which is not installed here.
!
! This shim declares that one parameter with the identical value from upstream
! (gributils/class_gribfile_mod.f90:46, `GRIBFILE_CENTRE_ECMWF = 2`). Because it
! is a compile-time parameter, obukhov.f90 compiles to exactly the same code as
! it would against the real module -- the shim changes nothing about the routine
! under test, which is compiled verbatim from upstream.
module class_gribfile
  implicit none
  integer, parameter :: GRIBFILE_CENTRE_UNKNOWN = -1, &
                        GRIBFILE_CENTRE_NCEP    = 1,  &
                        GRIBFILE_CENTRE_ECMWF   = 2
end module class_gribfile
