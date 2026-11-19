! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
!
! TRANSCRIBED (route 3) from PFLOTRAN 669fbce reaction_isotherm.F90, routine
! ReactionIsothermTotalSorbKD, `select case(isotherm%eqisothermtype(irxn))`
! lines 326-343. Only two substitutions, both of derived-type components that
! the full routine reads from `isotherm_rxn%`:
!   isotherm_rxn%eqisothermlangmuirb(irxn)   -> langmuir_b
!   isotherm_rxn%eqisothermfreundlichn(irxn) -> freundlich_n
! check_verbatim.sh proves every other statement is present, byte for byte, in
! the pinned upstream file. Output: `<kind> <Kd> <b|n> <molality> <res> <dres_dc>`
! as hex doubles.
program isotherm_kernel
  implicit none
  real(8) :: kd_kgw_m3b, molality, tempreal, res, dres_dc
  real(8) :: one_over_n, molality_one_over_n, langmuir_b, freundlich_n
  integer :: i, j, k
  real(8) :: kds(3), bs(3), ns(3), cs(6)
  kds = [0.5d0, 12.5d0, 3.0d-2]
  bs = [1.0d-3, 0.2d0, 4.0d0]
  ns = [1.0d0, 1.5d0, 2.5d0]
  cs = [1.0d-9, 1.0d-6, 1.0d-4, 1.0d-3, 0.1d0, 1.0d0]
  do i = 1, 3
    kd_kgw_m3b = kds(i)
    do j = 1, 6
      molality = cs(j)
        res = kd_kgw_m3b*molality
        dres_dc = kd_kgw_m3b
      write(*,'(a,5(1x,Z16.16))') 'LIN', kd_kgw_m3b, 0.d0, molality, res, dres_dc
      do k = 1, 3
        langmuir_b = bs(k)
        tempreal = kd_kgw_m3b*molality
        res = tempreal*langmuir_b / &
              (1.d0 + tempreal)
        dres_dc = res/molality - &
                  res / (1.d0 + tempreal) * tempreal / molality
        write(*,'(a,5(1x,Z16.16))') 'LNG', kd_kgw_m3b, langmuir_b, molality, res, dres_dc
        freundlich_n = ns(k)
        one_over_n = 1.d0/freundlich_n
        molality_one_over_n = molality**one_over_n
        res = kd_kgw_m3b*molality**one_over_n
        dres_dc = res/molality*one_over_n
        write(*,'(a,5(1x,Z16.16))') 'FRD', kd_kgw_m3b, freundlich_n, molality, res, dres_dc
      enddo
    enddo
  enddo
end program isotherm_kernel
