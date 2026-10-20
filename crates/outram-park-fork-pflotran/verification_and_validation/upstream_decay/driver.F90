! Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.
!
! Minimal driver for upstream PFLOTRAN's UFD_DECAY decay solver (gh:#1000).
!
! The solver body (ufd_body_extract.F90) and the LU it calls
! (lu_extract.F90) are extracted VERBATIM from PFLOTRAN 2af572b8 by
! extract.sh; this file only supplies (a) the declarations the extracted
! lines need, as dummy arguments named exactly as the derived-type components
! they replace (`this%isotope_decay_rate` -> `isotope_decay_rate`, ...), and
! (b) the parent list built as PMUFDDecaySetup does it (pm_ufd_decay.F90
! lines 1203-1222), and (c) the test cases. Decay rates come from half-lives
! exactly as the HALF_LIFE card does (pm_ufd_decay.F90 line 480:
! `-1.d0*log(0.5d0)/tempreal`).
!
! Output, one line per value: `<case> <variant> <step> <isotope> <hex bits>`,
! variant `E` (explicit 3-generation) or `I` (IMPLICIT_SOLUTION); for `I` an
! extra `<case> N <step> 0 <iterations>` line records the Newton count.
! `L <case> <isotope> <hex bits>` records each decay rate.
!
! Usage: ./driver            -> the committed cases (upstream_output.txt)
!        ./driver zero       -> implicit solve from a zero daughter inventory
!        ./driver zeroparent -> implicit solve from a zero parentless isotope

module lu_mod
  implicit none
  private
  public :: LUDecomposition, LUBackSubstitution
  interface LUDecomposition
    module procedure LUDecomposition1
    module procedure LUDecomposition2
  end interface
contains
  include 'lu_extract.F90'
end module lu_mod

module ufd_mod
  use lu_mod
  implicit none
contains

  subroutine ufd_step(implicit_solution, num_isotopes, isotope_decay_rate, &
                      isotope_parents, isotope_daughters, &
                      isotope_daughter_stoich, mass_iso_tot0, dt, &
                      mass_iso_tot1, it)
    logical, intent(in) :: implicit_solution
    integer, intent(in) :: num_isotopes
    real(8), intent(in) :: isotope_decay_rate(:)
    integer, intent(in) :: isotope_parents(0:,:)
    integer, intent(in) :: isotope_daughters(0:,:)
    real(8), intent(in) :: isotope_daughter_stoich(:,:)
    real(8), intent(in) :: mass_iso_tot0(num_isotopes)
    real(8), intent(in) :: dt
    real(8), intent(out) :: mass_iso_tot1(num_isotopes)
    integer, intent(out) :: it

    ! Declarations copied from PMUFDDecaySolveISPDIAtCell (lines 1785-1817).
    integer :: i, p, g, ip, ig, iiso
    real(8) :: coeff(num_isotopes)
    real(8) :: mass_old(num_isotopes)
    real(8) :: norm
    real(8) :: residual(num_isotopes)
    real(8) :: solution(num_isotopes)
    real(8) :: prev_solution(num_isotopes)
    real(8) :: rhs(num_isotopes)
    integer :: indices(num_isotopes)
    real(8) :: Jacobian(num_isotopes,num_isotopes)
    real(8) :: rate, rate_constant, stoich, one_over_dt
    real(8), parameter :: tolerance = 1.d-6
    integer :: idaughter

    one_over_dt = 1.d0 / dt
    it = 0
    ! line 1865
    mass_old(:) = mass_iso_tot0(:)

    include 'ufd_body_extract.F90'

  end subroutine ufd_step

end module ufd_mod

program driver
  use ufd_mod
  implicit none
  real(8), parameter :: MINUTE = 60.d0, HOUR = 3600.d0, DAY = 86400.d0
  real(8), parameter :: YEAR = 365.25d0 * DAY
  integer, parameter :: MAXN = 5, MAXD = 2
  character(len=32) :: arg

  arg = ''
  if (command_argument_count() > 0) call get_command_argument(1, arg)

  select case (trim(arg))
  case ('zero')
    call zero_daughter()
  case ('zeroparent')
    call zero_parentless()
  case default
    call radon('radon', [1000.d0, 1.d0, 1.d0, 1.d0, 1.d0], .true.)
    call radon('radon0', [1000.d0, 0.d0, 0.d0, 0.d0, 0.d0], .false.)
    call thorium_c()
    call converging()
  end select

contains

  real(8) function hl_rate(hl)
    real(8), intent(in) :: hl
    hl_rate = -1.d0*log(0.5d0)/hl
  end function hl_rate

  ! Build isotope_parents exactly as PMUFDDecaySetup (lines 1203-1222).
  subroutine build_parents(n, daughters, parents)
    integer, intent(in) :: n
    integer, intent(in) :: daughters(0:MAXD, n)
    integer, intent(out) :: parents(0:MAXN, n)
    integer :: iisotope, icount
    parents = 0
    do iisotope = 1, n
      do icount = 1, daughters(0,iisotope)
        parents(0,daughters(icount,iisotope)) = &
          parents(0,daughters(icount,iisotope)) + 1
        parents(parents(0,daughters(icount,iisotope)), &
                daughters(icount,iisotope)) = iisotope
      enddo
    enddo
  end subroutine build_parents

  subroutine run(name, n, rates, daughters, stoich, m0, dt, nsteps, do_implicit)
    character(len=*), intent(in) :: name
    integer, intent(in) :: n, nsteps
    real(8), intent(in) :: rates(n), stoich(MAXD, n), m0(n), dt
    integer, intent(in) :: daughters(0:MAXD, n)
    logical, intent(in) :: do_implicit
    integer :: parents(0:MAXN, n), step, i, it, v
    real(8) :: m(n), m1(n)
    character :: tag

    call build_parents(n, daughters, parents)
    do i = 1, n
      write(*,'(A,1X,A,1X,I0,1X,Z16.16)') 'L', name, i, transfer(rates(i), 0_8)
    enddo
    do v = 0, 1
      if (v == 1 .and. .not. do_implicit) cycle
      tag = merge('I', 'E', v == 1)
      m = m0
      do step = 1, nsteps
        call ufd_step(v == 1, n, rates, parents, daughters, stoich, m, dt, m1, it)
        m = m1
        do i = 1, n
          write(*,'(A,1X,A,1X,I0,1X,I0,1X,Z16.16)') name, tag, step, i, &
            transfer(m(i), 0_8)
        enddo
        if (v == 1) write(*,'(A,1X,A,1X,I0,1X,I0,1X,I0)') name, 'N', step, 0, it
      enddo
    enddo
  end subroutine run

  ! Rn-222 -> Po-218 -> Pb-214 -> Bi-214 -> Pb-210, unit branching.
  subroutine radon(name, m0, do_implicit)
    character(len=*), intent(in) :: name
    real(8), intent(in) :: m0(5)
    logical, intent(in) :: do_implicit
    real(8) :: rates(5), stoich(MAXD, 5)
    integer :: daughters(0:MAXD, 5), i
    rates = [hl_rate(3.8235d0*DAY), hl_rate(3.098d0*MINUTE), &
             hl_rate(26.8d0*MINUTE), hl_rate(19.9d0*MINUTE), &
             hl_rate(22.2d0*YEAR)]
    daughters = 0
    stoich = 0.d0
    do i = 1, 4
      daughters(0,i) = 1
      daughters(1,i) = i + 1
      stoich(1,i) = 1.d0
    enddo
    call run(name, 5, rates, daughters, stoich, m0, HOUR, 10, do_implicit)
  end subroutine radon

  ! Pb-212 -> Bi-212 -> {Po-212 0.6406, Tl-208 0.3594} -> Pb-208 (stable).
  subroutine thorium_c()
    real(8) :: rates(5), stoich(MAXD, 5)
    integer :: daughters(0:MAXD, 5)
    rates = [hl_rate(10.64d0*HOUR), hl_rate(60.55d0*MINUTE), &
             hl_rate(0.299d-6), hl_rate(3.053d0*MINUTE), 0.d0]
    daughters = 0
    stoich = 0.d0
    daughters(0:1,1) = [1, 2];    stoich(1,1) = 1.d0
    daughters(0:2,2) = [2, 3, 4]; stoich(1:2,2) = [0.6406d0, 0.3594d0]
    daughters(0:1,3) = [1, 5];    stoich(1,3) = 1.d0
    daughters(0:1,4) = [1, 5];    stoich(1,4) = 1.d0
    call run('thc', 5, rates, daughters, stoich, &
             [1000.d0, 1.d0, 1.d0, 1.d0, 1.d0], 600.d0, 10, .true.)
  end subroutine thorium_c

  ! P1 (4 s), P2 (9 s) -> D (stable).
  subroutine converging()
    real(8) :: rates(3), stoich(MAXD, 3)
    integer :: daughters(0:MAXD, 3)
    rates = [hl_rate(4.d0), hl_rate(9.d0), 0.d0]
    daughters = 0
    stoich = 0.d0
    daughters(0:1,1) = [1, 3]; stoich(1,1) = 1.d0
    daughters(0:1,2) = [1, 3]; stoich(1,2) = 1.d0
    call run('conv', 3, rates, daughters, stoich, [10.d0, 20.d0, 1.d0], &
             1.d0, 5, .true.)
  end subroutine converging

  ! A (50 s) -> B (8 s), B starting at zero: implicit only.
  subroutine zero_daughter()
    real(8) :: rates(2), stoich(MAXD, 2), m1(2)
    integer :: daughters(0:MAXD, 2), parents(0:MAXN, 2), it
    rates = [hl_rate(50.d0), hl_rate(8.d0)]
    daughters = 0
    stoich = 0.d0
    daughters(0:1,1) = [1, 2]; stoich(1,1) = 1.d0
    call build_parents(2, daughters, parents)
    call ufd_step(.true., 2, rates, parents, daughters, stoich, &
                  [1000.d0, 0.d0], 1.d0, m1, it)
    write(*,'(A,1X,I0,2(1X,ES24.16E3))') 'zero daughter: it, m1 =', it, m1
  end subroutine zero_daughter

  ! A (50 s) alone, starting at zero: implicit only.
  subroutine zero_parentless()
    real(8) :: rates(1), stoich(MAXD, 1), m1(1)
    integer :: daughters(0:MAXD, 1), parents(0:MAXN, 1), it
    rates = [hl_rate(50.d0)]
    daughters = 0
    stoich = 0.d0
    call build_parents(1, daughters, parents)
    call ufd_step(.true., 1, rates, parents, daughters, stoich, &
                  [0.d0], 1.d0, m1, it)
    write(*,'(A,1X,I0,1X,ES24.16E3)') 'zero parentless: it, m1 =', it, m1
  end subroutine zero_parentless

end program driver
