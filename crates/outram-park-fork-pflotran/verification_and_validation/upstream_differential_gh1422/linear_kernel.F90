! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
!
! TRANSCRIBED (route 3) from PFLOTRAN 669fbce
! src/pflotran/characteristic_curves_common.F90 (LGPL-3.0):
!   SFLinearSaturation          lines 2802-2841 (body 2826-2840)
!   SFLinearCapillaryPressure   lines 2737-2798 (body 2766-2796)
!   RPFBurdineLinearLiqRelPerm  lines 5191-5228 (body 5208-5226)
! Substitutions (derived-type components -> local variables, nothing else):
!   this%Sr -> Sr, this%alpha -> alpha, this%pcmax -> pcmax
! The option/trapped-gas arguments and the #if defined(MATCH_TOUGH2) block
! (not compiled in the regression build) are dropped.
! check_verbatim_linear.sh proves every transcribed statement is present, byte
! for byte, in the pinned upstream file.
! Output (hex doubles): `SET Sr alpha pcmax`, then `SAT pc Sl dsat_dpres`,
! `CHN pc kr dkr_sat` (kr of the Sl that SAT returned for pc: the Richards chain),
! `CAP Sl pc dpc_dsatl`, `KR Sl kr dkr_sat`.
program linear_kernel
  implicit none
  integer, parameter :: nset = 3
  double precision :: sets(3,nset)
  double precision :: Sr, alpha, pcmax
  double precision :: pc, sl, dsat_dpres, dpc_dsatl, kr, dkr_sat
  integer :: iset, i
  sets(:,1) = [0.3333333d0, 1.d30, 9807.0d0]
  sets(:,2) = [0.1d0, 1.d-4, 1.d8]
  sets(:,3) = [0.d0, 5.d-5, 1.d6]
  do iset = 1, nset
    Sr = sets(1,iset); alpha = sets(2,iset); pcmax = sets(3,iset)
    write(*,'(a,3(1x,z16.16))') 'SET', transfer(Sr,1_8), transfer(alpha,1_8), transfer(pcmax,1_8)
    do i = -4, 44
      pc = pcmax*dble(i)/40.d0
      call sat(pc, sl, dsat_dpres)
      write(*,'(a,3(1x,z16.16))') 'SAT', transfer(pc,1_8), transfer(sl,1_8), transfer(dsat_dpres,1_8)
      call relperm(sl, kr, dkr_sat)
      write(*,'(a,3(1x,z16.16))') 'CHN', transfer(pc,1_8), transfer(kr,1_8), transfer(dkr_sat,1_8)
    enddo
    do i = -4, 44
      sl = dble(i)/40.d0
      call cap(sl, pc, dpc_dsatl)
      write(*,'(a,3(1x,z16.16))') 'CAP', transfer(sl,1_8), transfer(pc,1_8), transfer(dpc_dsatl,1_8)
      call relperm(sl, kr, dkr_sat)
      write(*,'(a,3(1x,z16.16))') 'KR', transfer(sl,1_8), transfer(kr,1_8), transfer(dkr_sat,1_8)
    enddo
  enddo
contains
  subroutine sat(capillary_pressure, liquid_saturation, dsat_dpres)
    double precision, intent(in) :: capillary_pressure
    double precision, intent(out) :: liquid_saturation, dsat_dpres
    double precision :: Se, dSe_dpc
    double precision, parameter :: dpc_dpres = -1.d0
  dsat_dpres = 0.d0
  if (capillary_pressure <= 0.d0) then
    liquid_saturation = 1.d0
    return
  else
    Se = (pcmax-capillary_pressure) / (pcmax-1.d0/alpha)
    dSe_dpc = -1.d0/(pcmax-1.d0/alpha)
    liquid_saturation = Sr + (1.d0-Sr)*Se
    dsat_dpres = (1.d0-Sr)*dSe_dpc*dpc_dpres
  endif
  end subroutine sat

  subroutine cap(liquid_saturation, capillary_pressure, dpc_dsatl)
    double precision, intent(in) :: liquid_saturation
    double precision, intent(out) :: capillary_pressure, dpc_dsatl
    double precision :: Se, dSe_dsatl, one_over_alpha_minus_pcmax
  dpc_dsatl = 0.d0
  if (liquid_saturation <= Sr) then
    capillary_pressure = pcmax
    return
  else if (liquid_saturation >= 1.d0) then
    capillary_pressure = 0.d0
    return
  endif
  dSe_dsatl = 1.d0/(1.d0-Sr)
  Se = (liquid_saturation-Sr)*dSe_dsatl
  one_over_alpha_minus_pcmax = 1.d0/alpha-pcmax
  capillary_pressure = one_over_alpha_minus_pcmax*Se + pcmax
  dpc_dsatl = one_over_alpha_minus_pcmax*dSe_dsatl
  if (capillary_pressure > pcmax) then
    capillary_pressure = pcmax
    dpc_dsatl = 0.d0
  endif
  end subroutine cap

  subroutine relperm(liquid_saturation, relative_permeability, dkr_sat)
    double precision, intent(in) :: liquid_saturation
    double precision, intent(out) :: relative_permeability, dkr_sat
    double precision :: Se
  relative_permeability = 0.d0
  dkr_sat = 0.d0
  Se = (liquid_saturation - Sr) / (1.d0 - Sr)
  if (Se >= 1.d0) then
    relative_permeability = 1.d0
    return
  else if (Se <= 0.d0) then
    relative_permeability = 0.d0
    return
  endif
  relative_permeability = Se
  dkr_sat = 1.d0 / (1.d0 - Sr)
  end subroutine relperm
end program linear_kernel
