! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
!
! gh:#1422 differential driver.  It LINKS the real PFLOTRAN objects built from
! commit 669fbce204af39428020c6b9d74beae363376689 (no routine is copied) and
! calls their own public entry points, printing every input and output as the
! hex bit pattern of the IEEE-754 double (Z16.16).  The Rust test
! tests/upstream_differential_gh1422.rs reads this file's output.
!
! Line formats (all numbers are 16 hex digits = the double's bits):
!   VGP <set> <alpha> <m> <Sr>            parameters of VG set
!   BCP <set> <alpha> <lambda> <Sr>       parameters of BC set
!   VGS <set> <pc> <Sl> <dSl_dP>          SFVGSaturation
!   BCS <set> <pc> <Sl> <dSl_dP>          SFBCSaturation
!   VGK <set> <Sl> <kr> <dkr_dSl>         RPFMualemVGLiqRelPerm
!   BCK <set> <Sl> <kr> <dkr_dSl>         RPFBurdineBCLiqRelPerm
!   RHO67 <T_C> <P> <kg/m3> <kmol/m3>     EOSWaterDensity, IFC67 (the default)
!   RHO97 <T_C> <P> <kg/m3> <kmol/m3>     EOSWaterDensity, IF97
!   H67   <T_C> <P> <J/kmol>              EOSWaterEnthalpy, IFC67 (default)
!   H97   <T_C> <P> <J/kmol>              EOSWaterEnthalpy, IF97
!   PS67  <T_C> <Pa>                      EOSWaterSaturationPressure, IFC67
!   PS97  <T_C> <Pa>                      EOSWaterSaturationPressure, IF97
!   VIS1  <T_C> <P> <PS> <Pa.s>           EOSWaterViscosity (default, Visc1)
!   DHCONST <A> <B> <Bdot>                reaction%debyeA, debyeB, debyeBdot
program gh1422_driver
  use petscsys
  use Option_module
  use Characteristic_Curves_Base_module
  use Characteristic_Curves_Common_module
  use EOS_Water_module
  use Reaction_Aux_module
  implicit none

  type(option_type), pointer :: option
  class(sat_func_vg_type), pointer :: vg
  class(rpf_mualem_vg_liq_type), pointer :: mvg
  class(sat_func_bc_type), pointer :: bc
  class(rpf_burdine_bc_liq_type), pointer :: bbc
  class(reaction_rt_type), pointer :: rxn
  PetscErrorCode :: ierr
  integer :: iset, k
  real(8) :: alpha(5), mm(5), lam(5), sr(5), pc, sl, dsl, kr, dkr
  real(8) :: dw, dwmol, hw, ps, vw, tc
  real(8) :: tgrid(10), pgrid(5)
  integer :: it, ip

  call PetscInitialize(PETSC_NULL_CHARACTER, ierr)
  option => OptionCreate()
  call EOSWaterInit()

  ! ---- characteristic curves --------------------------------------------
  alpha = [1.0d-4, 5.0d-4, 2.5d-5, 1.0d-3, 2.0d-4]
  mm    = [0.5d0, 0.75d0, 0.4d0, 0.2d0, 0.3d0]
  lam   = [0.5d0, 1.0d0, 2.0d0, 0.4d0, 3.0d0]
  sr    = [0.1d0, 0.05d0, 0.2d0, 0.0d0, 0.15d0]

  do iset = 1, 5
    vg => SFVGCreate()
    vg%alpha = alpha(iset); vg%m = mm(iset); vg%Sr = sr(iset)
    mvg => RPFMualemVGLiqCreate()
    mvg%m = mm(iset); mvg%Sr = sr(iset)
    bc => SFBCCreate()
    bc%alpha = alpha(iset); bc%lambda = lam(iset); bc%Sr = sr(iset)
    bbc => RPFBurdineBCLiqCreate()
    bbc%lambda = lam(iset); bbc%Sr = sr(iset)
    write(*,'(a,i2,3(1x,Z16.16))') 'VGP', iset, alpha(iset), mm(iset), sr(iset)
    write(*,'(a,i2,3(1x,Z16.16))') 'BCP', iset, alpha(iset), lam(iset), sr(iset)
    ! capillary pressure: -1 Pa (saturated), then 10^(k/8) Pa for k=0..80
    do k = -1, 80
      if (k < 0) then
        pc = -1.0d0
      else
        pc = 10.0d0**(dble(k)/8.0d0)
      endif
      call vg%Saturation(pc, sl, dsl, option)
      write(*,'(a,i2,3(1x,Z16.16))') 'VGS', iset, pc, sl, dsl
      call bc%Saturation(pc, sl, dsl, option)
      write(*,'(a,i2,3(1x,Z16.16))') 'BCS', iset, pc, sl, dsl
    enddo
    ! liquid saturation sweep: Sr + (1-Sr)*(k/100), k = 0..100
    do k = 0, 100
      sl = sr(iset) + (1.0d0 - sr(iset))*(dble(k)/100.0d0)
      call mvg%RelativePermeability(sl, kr, dkr, option)
      write(*,'(a,i2,3(1x,Z16.16))') 'VGK', iset, sl, kr, dkr
      call bbc%RelativePermeability(sl, kr, dkr, option)
      write(*,'(a,i2,3(1x,Z16.16))') 'BCK', iset, sl, kr, dkr
    enddo
  enddo

  ! ---- water EOS ----------------------------------------------------------
  tgrid = [5.d0, 25.d0, 50.d0, 75.d0, 100.d0, 150.d0, 200.d0, 250.d0, 300.d0, 340.d0]
  pgrid = [1.0d5, 1.0d6, 1.0d7, 5.0d7, 1.0d8]
  call EOSWaterSetSaturationPressure('IFC67')
  do it = 1, 10
    do ip = 1, 5
      tc = tgrid(it)
      ! Region 1 only: skip states at or below saturation.
      call EOSWaterSaturationPressure(tc, ps, ierr)
      if (pgrid(ip) <= ps*1.0001d0) cycle
      call EOSWaterSetDensity('IFC67')
      call EOSWaterDensity(tc, pgrid(ip), dw, dwmol, ierr)
      write(*,'(a,4(1x,Z16.16))') 'RHO67', tc, pgrid(ip), dw, dwmol
      call EOSWaterSetEnthalpy('IFC67')
      call EOSWaterEnthalpy(tc, pgrid(ip), hw, ierr)
      write(*,'(a,3(1x,Z16.16))') 'H67  ', tc, pgrid(ip), hw
      call EOSWaterSetDensity('IF97')
      call EOSWaterDensity(tc, pgrid(ip), dw, dwmol, ierr)
      write(*,'(a,4(1x,Z16.16))') 'RHO97', tc, pgrid(ip), dw, dwmol
      call EOSWaterSetEnthalpy('IF97')
      call EOSWaterEnthalpy(tc, pgrid(ip), hw, ierr)
      write(*,'(a,3(1x,Z16.16))') 'H97  ', tc, pgrid(ip), hw
      call EOSWaterViscosity(tc, pgrid(ip), ps, vw, ierr)
      write(*,'(a,4(1x,Z16.16))') 'VIS1 ', tc, pgrid(ip), ps, vw
    enddo
  enddo
  do it = 0, 34
    tc = 1.0d0 + 10.0d0*dble(it)
    call EOSWaterSetSaturationPressure('IFC67')
    call EOSWaterSaturationPressure(tc, ps, ierr)
    write(*,'(a,2(1x,Z16.16))') 'PS67 ', tc, ps
    call EOSWaterSetSaturationPressure('IF97')
    call EOSWaterSaturationPressure(tc, ps, ierr)
    write(*,'(a,2(1x,Z16.16))') 'PS97 ', tc, ps
  enddo

  ! ---- geochemistry constants ------------------------------------------------
  rxn => ReactionAuxCreateAux()
  write(*,'(a,3(1x,Z16.16))') 'DHCONST', rxn%debyeA, rxn%debyeB, rxn%debyeBdot
end program gh1422_driver
