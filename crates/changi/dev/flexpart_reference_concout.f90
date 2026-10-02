! SPDX-License-Identifier: GPL-3.0
!
! Stage "concout" code-to-code reference driver for `changi::flexpart`:
!
!  (1) concoutput, concoutput_nest, concoutput_surf -- the conversion of the
!      gridded particle mass (gridunc / wetgridunc / drygridunc and the nest
!      copies) into concentrations, mixing ratios and deposition, the air
!      density / dry-air factor, the uncertainty statistics over nclassunc
!      and the sparse-format packing of every file they write;
!  (2) the per-synchronisation-step bookkeeping of timemanager.f90.
!
! (1) UPSTREAM, VERBATIM. concoutput.f90, concoutput_nest.f90,
! concoutput_surf.f90, mean_mod.f90, caldate.f90 and the modules are compiled
! unmodified from upstream_source/FLEXPART/src by
! dev/build_reference_concout.sh. The routines write binary files; the driver
! runs each one in a scratch working directory (path(2) = './') and READS THE
! FILES BACK record by record, so what is compared is exactly what FLEXPART
! writes. Module arrays the routines leave behind (densityoutgrid,
! densitydrygrid, factor_drygrid, factor3d, and grid / gridsigma / wetgrid /
! wetgridsigma / drygrid / drygridsigma, which hold the LAST
! (species, release, age class) slice) and the three uncertainty outputs are
! printed as well. Why files and not only module arrays: the sparse packing,
! the record order, the per-slice unit conversion and the receptor records
! exist only in the files. The units 91/92 (receptor_conc / receptor_pptv),
! which upstream opens in openreceptors.f90, are opened by the driver.
!
! (2) EXTRACTED LINES. The bookkeeping exists only inline in timemanager.f90,
! so -- exactly as stage 0 did for radioactive decay (emit_decay in
! dev/flexpart_reference.f90) -- the upstream lines are copied BYTE FOR BYTE
! into driver subroutines, each block marked with the timemanager.f90 line
! numbers it was copied from:
!   tm_decaydep  lines 264-299  decay of the deposition grids at loutnext
!   tm_pre       lines 534-593  release slot, age class, initialize trigger,
!                               DRYBKDEP / WETBKDEP scavenging initialisation
!   tm_post      lines 625-703  termination, decay and dry-deposition mass
!                               removal, minmass, drydepokernel calls, age
!   tm_split     lines 468-499  particle splitting
!   tm_clock     lines 118-121, 151, 264, 345-359, 371-372, 431, 452-459,
!                468, 504, 509-513: the output/sampling clock. Here (only) the
!                CALL statements (conccalc, concoutput*, wetdepo, ...) are
!                replaced by recording which call would happen; every
!                condition and every assignment is upstream's text.
! The routines those blocks call (initialize, get_vdep_prob, get_wetscav,
! drydepokernel, drydepokernel_nest, initial_cond_calc) are replaced by stubs
! at the end of this file that RECORD their arguments (and return
! driver-chosen values): those routines are verified in their own stages
! (4, 3, 5); what is under test here is timemanager's use of them.
!
! par_mod.f90 VARIANTS (FLEXPART's user-edited configuration file; every
! routine under test is verbatim). The build compiles this driver three times
! per precision:
!   V0  par_mod.f90 as shipped (maxspec = maxageclass = nclassunc = 1);
!   V1  maxspec=2, maxageclass=2, nclassunc=4;
!   V2  as V1 plus lparticlecountoutput=.true.
! and, in the real(8) build ONLY, dep_prec=dp in all three. Reason, measured:
! upstream v10.4 does NOT COMPILE with -fdefault-real-8 and dep_prec=sp:
! `call mean(auxgrid, grid(ix,jy,kz), gridsigma(ix,jy,kz), nclassunc)` has
! auxgrid real(dep_prec)=real(4) and grid real(8), and mean_mod has no
! (sp, dp, dp) specific ("There is no specific subroutine for the generic
! 'mean'", concoutput.f90:322, concoutput_nest.f90:272,
! concoutput_surf.f90:294). dep_prec is documented in par_mod.f90 as
! "sp or dp"; dp is the setting a double-precision build needs.
!
! Synthetic inputs are exact dyadic numbers built from integer indices
! (function gval), identical in real(4), real(8) and the Rust test, which
! rebuilds them from the same formula; `setup.fp` rows carry a double-precision
! fingerprint of every input array so a formula mismatch is caught.
!
! Row format: function,a1;a2;...,o1;o2;...  real(4) builds print 9
! significant digits (parse as f32), real(8) builds 17 (ES25.17E3); rows
! named setup.fp are always printed with 17 digits.

module concout_stub_mod
  use par_mod, only: maxspec, dep_prec
  implicit none
  integer :: n_dry = 0, n_dryn = 0, n_init = 0, n_vdep = 0, n_wetscav = 0, n_icc = 0
  integer :: dry_nunc = 0, dry_nage = 0, dry_kp = 0
  integer :: dryn_nunc = 0, dryn_nage = 0, dryn_kp = 0
  real(dep_prec) :: dry_dep(maxspec), dryn_dep(maxspec)
  real :: dry_x = 0., dry_y = 0., dryn_x = 0., dryn_y = 0.
  integer :: icc_time(4) = 0
  real :: stub_prob_rec(maxspec)
  real :: stub_grfraction(3) = 0., stub_wetscav = 0.
end module concout_stub_mod

program flexpart_reference_concout

  use par_mod
  use com_mod
  use unc_mod
  use outg_mod
  use point_mod
  use concout_stub_mod
  implicit none

  integer, parameter :: mxo = 5, myo = 4, mzo = 3, mxn = 4, myn = 3, npt = 2
  integer, parameter :: mxm = 8, mym = 7, mzm = 6
  integer :: variant, icase
  ! drydeposit and xscav are timemanager LOCALS that persist from particle to
  ! particle; kept here at program scope for the same reason (see tm_post).
  real(dep_prec) :: drydeposit(maxspec)

  if (maxspec == 1) then
    variant = 0
  else if (.not. lparticlecountoutput) then
    variant = 1
  else
    variant = 2
  end if

  write(*,'(A)') '# FLEXPART stage "concout" reference, generated by dev/flexpart_reference_concout.f90'
  write(*,'(A)') '# upstream: github.com/flexpart/flexpart v10.4 @ 3d7eebf (GPL-3.0-or-later)'
  write(*,'(A)') '# DO NOT EDIT BY HAND -- regenerate with dev/build_reference_concout.sh'
  write(*,'(A)') 'function,args,outputs'

  call emit('setup.variant', (/ dble(variant), dble(maxspec), dble(maxageclass), &
       dble(nclassunc), dble(merge(1,0,lparticlecountoutput)), dble(dep_prec), &
       dble(kind(1.0)) /), (/ 0d0 /))

  call setup_static()

  ! V2 (particle-count output) changes only the concentration records:
  ! one forward and one backward case suffice.
  do icase = 1, 6
    if (variant == 2 .and. icase /= 1 .and. icase /= 5) cycle
    call run_concout_case(icase)
  end do

  call com_mod_allocate_part(maxpart)
  allocate(xscav_frac1(maxpart, maxspec))
  allocate(npart(npt), zpoint1(npt), zpoint2(npt))
  ! The clock is integer arithmetic independent of the variant: V0 only.
  ! The other blocks loop over species / classes: V0 and V1.
  if (variant == 0) call emit_clock()
  if (variant <= 1) then
    call emit_decaydep()
    call emit_pre()
    call emit_post()
    call emit_split()
  end if

contains

  ! ------------------------------------------------------------------------
  ! Row writer. Numbers print as ES16.8E3 in the real(4) build and ES25.17E3
  ! in the real(8) build (fmtnum of dev/flexpart_reference_physics.f90);
  ! dp=.true. forces 17 digits. Written piecewise, so rows have no length cap.
  subroutine emit(name, args, outs, dp)
    character(len=*), intent(in) :: name
    double precision, intent(in) :: args(:), outs(:)
    logical, intent(in), optional :: dp
    logical :: wide
    integer :: i
    wide = (kind(1.0) /= 4)
    if (present(dp)) wide = wide .or. dp
    write(*,'(A)',advance='no') trim(name)//','
    do i = 1, size(args)
      call putnum(args(i), wide)
      if (i < size(args)) write(*,'(A)',advance='no') ';'
    end do
    write(*,'(A)',advance='no') ','
    do i = 1, size(outs)
      call putnum(outs(i), wide)
      if (i < size(outs)) write(*,'(A)',advance='no') ';'
    end do
    write(*,'(A)') ''
  end subroutine emit

  subroutine putnum(x, wide)
    double precision, intent(in) :: x
    logical, intent(in) :: wide
    character(len=32) :: buf
    if (wide) then
      write(buf,'(ES25.17E3)') x
    else
      write(buf,'(ES16.8E3)') x
    end if
    write(*,'(A)',advance='no') trim(adjustl(buf))
  end subroutine putnum

  ! ------------------------------------------------------------------------
  ! Synthetic gridded mass. kind: 1 gridunc, 2 wetgridunc, 3 drygridunc,
  ! 4 griduncn, 5 wetgriduncn, 6 drygriduncn (kz = 0 for 2-D grids).
  ! A cell is zero in all classes when m <= 1 (runs of zeros for the sparse
  ! packing); for m = 6 all classes are equal (sigma exactly 0); otherwise
  ! the classes differ. Values are k/64 * 2**e * scale: exact in real(4).
  real function gval(icase, ix, jy, kz, ks, kp, l, nage, ikind)
    integer, intent(in) :: icase, ix, jy, kz, ks, kp, l, nage, ikind
    integer :: m, k, e
    m = mod(2*ix + 3*jy + 5*kz + ks + 2*kp + 3*nage + icase + 4*ikind, 7)
    if (m <= 1) then
      gval = 0.
      return
    end if
    if (m == 6) then
      k = 7 + mod(ix + jy + kz, 5)
    else
      k = 1 + mod(17*l + 5*ix + 3*jy + kz + m + ikind, 50)
    end if
    e = mod(ix + jy + kz + ikind, 5) - 2
    gval = real(k) / 64. * 2.**e * cscale(icase)
  end function gval

  ! Mass scale per case. Case 3 uses ~1e-17 kg so that the one-pass variance
  ! falls below mean_mod's absolute eps = 1e-30 (sigma forced to 0).
  real function cscale(icase)
    integer, intent(in) :: icase
    if (icase == 3) then
      cscale = 2.**(-56)
    else
      cscale = 2.**(-30)
    end if
  end function cscale

  real function rval(ix, jy, k, islot)
    integer, intent(in) :: ix, jy, k, islot
    rval = 1.25 - 0.09375*real(k-1) + 0.0078125*real(mod(ix+2*jy,5)) + 0.03125*real(islot-1)
  end function rval

  ! rho_dry = rho - 1/64 * mod(ix+jy,3): equal to rho (factor exactly 1) on
  ! a third of the columns, at every level.
  real function rdval(ix, jy, k, islot)
    integer, intent(in) :: ix, jy, k, islot
    rdval = rval(ix, jy, k, islot) - 0.015625*real(mod(ix+jy,3))
  end function rdval

  ! ------------------------------------------------------------------------
  subroutine setup_static()
    integer :: ix, jy, k, islot
    double precision :: s
    ! Mother met grid (only rho, rho_dry and height are read).
    nxmin1 = mxm - 1
    nymin1 = mym - 1
    nz = mzm
    xlon0 = 0.
    ylat0 = 10.
    dx = 1.
    dy = 0.5
    height(1:mzm) = (/ 0., 50., 150., 400., 1000., 2500. /)
    s = 0d0
    do islot = 1, 2
      do k = 1, mzm
        do jy = 0, mym-1
          do ix = 0, mxm-1
            rho(ix,jy,k,islot) = rval(ix,jy,k,islot)
            rho_dry(ix,jy,k,islot) = rdval(ix,jy,k,islot)
            s = s + dble(rho(ix,jy,k,islot)) + 2d0*dble(rho_dry(ix,jy,k,islot))
          end do
        end do
      end do
    end do
    call emit('setup.met', (/ dble(nxmin1), dble(nymin1), dble(nz), dble(xlon0), &
         dble(ylat0), dble(dx), dble(dy), dble(height(1:mzm)) /), (/ 0d0 /))
    call emit('setup.fp', (/ 0d0, 0d0 /), (/ s /), .true.)

    ! Output grids. x = (outlon0 + ix*dxout - xlon0)/dx = -1.5, 0, 1.5, 3, 4.5
    ! (nint ties and the clamp at 0); y = 0.5, 3, 5.5, 8 (clamp at nymin1).
    numxgrid = mxo
    numygrid = myo
    numzgrid = mzo
    outlon0 = -1.5
    outlat0 = 10.25
    dxout = 1.5
    dyout = 1.25
    numxgridn = mxn
    numygridn = myn
    outlon0n = 0.5
    outlat0n = 10.5
    dxoutn = 0.75
    dyoutn = 0.5
    call emit('setup.outgrid', (/ dble(numxgrid), dble(numygrid), dble(numzgrid), &
         dble(outlon0), dble(outlat0), dble(dxout), dble(dyout), dble(numxgridn), &
         dble(numygridn), dble(outlon0n), dble(outlat0n), dble(dxoutn), dble(dyoutn) /), &
         (/ 0d0 /))

    allocate(outheight(mzo), outheighthalf(mzo))
    allocate(area(0:mxo-1,0:myo-1), volume(0:mxo-1,0:myo-1,mzo))
    allocate(arean(0:mxn-1,0:myn-1), volumen(0:mxn-1,0:myn-1,mzo))
    ! As outgrid_init.f90: concoutput's work arrays have the larger of the
    ! mother and nest extents.
    allocate(gridsigma(0:mxo-1,0:myo-1,mzo), grid(0:mxo-1,0:myo-1,mzo))
    allocate(densityoutgrid(0:mxo-1,0:myo-1,mzo), densitydrygrid(0:mxo-1,0:myo-1,mzo))
    allocate(factor_drygrid(0:mxo-1,0:myo-1,mzo), factor3d(0:mxo-1,0:myo-1,mzo))
    allocate(sparse_dump_r(mxo*myo*mzo), sparse_dump_u(mxo*myo*mzo), sparse_dump_i(mxo*myo*mzo))
    allocate(wetgridsigma(0:mxo-1,0:myo-1), drygridsigma(0:mxo-1,0:myo-1))
    allocate(wetgrid(0:mxo-1,0:myo-1), drygrid(0:mxo-1,0:myo-1))
    allocate(gridunc(0:mxo-1,0:myo-1,mzo,maxspec,npt,nclassunc,maxageclass))
    allocate(wetgridunc(0:mxo-1,0:myo-1,maxspec,npt,nclassunc,maxageclass))
    allocate(drygridunc(0:mxo-1,0:myo-1,maxspec,npt,nclassunc,maxageclass))
    allocate(griduncn(0:mxn-1,0:myn-1,mzo,maxspec,npt,nclassunc,maxageclass))
    allocate(wetgriduncn(0:mxn-1,0:myn-1,maxspec,npt,nclassunc,maxageclass))
    allocate(drygriduncn(0:mxn-1,0:myn-1,maxspec,npt,nclassunc,maxageclass))
    allocate(xmass(npt, maxspec))

    do jy = 0, myo-1
      do ix = 0, mxo-1
        area(ix,jy) = 65536. * (1. + 0.25*real(mod(ix+jy,4)))
        do k = 1, mzo
          volume(ix,jy,k) = 2.**20 * (1. + 0.125*real(mod(ix+2*jy+k,5)))
        end do
      end do
    end do
    do jy = 0, myn-1
      do ix = 0, mxn-1
        arean(ix,jy) = 16384. * (1. + 0.5*real(mod(2*ix+jy,3)))
        do k = 1, mzo
          volumen(ix,jy,k) = 2.**18 * (1. + 0.25*real(mod(ix+jy+2*k,3)))
        end do
      end do
    end do
    s = 0d0
    do jy = 0, myo-1
      do ix = 0, mxo-1
        s = s + dble(area(ix,jy))
        do k = 1, mzo
          s = s + dble(volume(ix,jy,k))
        end do
      end do
    end do
    do jy = 0, myn-1
      do ix = 0, mxn-1
        s = s + dble(arean(ix,jy))
        do k = 1, mzo
          s = s + dble(volumen(ix,jy,k))
        end do
      end do
    end do
    call emit('setup.fp', (/ 0d0, 1d0 /), (/ s /), .true.)

    ! Receptors (met grid units): nint ties, both clamps.
    numreceptor = 4
    xreceptor(1:4) = (/ 0.5, 3.25, 7.75, -0.625 /)
    yreceptor(1:4) = (/ 2.5, -0.5, 6.875, 3.5 /)
    weightmolar(1:maxspec) = 352.
    if (maxspec >= 2) weightmolar(2) = 131.
    call emit('setup.receptors', (/ dble(xreceptor(1:4)), dble(yreceptor(1:4)), &
         dble(weightmolar(1:maxspec)) /), (/ 0d0 /))

    bdate = 2458849.5d0       ! 2020-01-01 00:00 UTC
    path(2) = './'
    length(2) = 2
    verbosity = 0
    nested_output = 1
    surf_only = 0
    DRYBKDEP = .false.
    WETBKDEP = .false.
  end subroutine setup_static

  ! ------------------------------------------------------------------------
  subroutine fill_case(icase)
    integer, intent(in) :: icase
    integer :: ix, jy, kz, ks, kp, l, nage, i
    double precision :: s(7)
    s = 0d0
    do nage = 1, maxageclass
      do l = 1, nclassunc
        do kp = 1, npt
          do ks = 1, maxspec
            do kz = 1, mzo
              do jy = 0, myo-1
                do ix = 0, mxo-1
                  gridunc(ix,jy,kz,ks,kp,l,nage) = gval(icase,ix,jy,kz,ks,kp,l,nage,1)
                  s(1) = s(1) + dble(gridunc(ix,jy,kz,ks,kp,l,nage))
                end do
              end do
              do jy = 0, myn-1
                do ix = 0, mxn-1
                  griduncn(ix,jy,kz,ks,kp,l,nage) = gval(icase,ix,jy,kz,ks,kp,l,nage,4)
                  s(4) = s(4) + dble(griduncn(ix,jy,kz,ks,kp,l,nage))
                end do
              end do
            end do
            do jy = 0, myo-1
              do ix = 0, mxo-1
                wetgridunc(ix,jy,ks,kp,l,nage) = gval(icase,ix,jy,0,ks,kp,l,nage,2)
                drygridunc(ix,jy,ks,kp,l,nage) = gval(icase,ix,jy,0,ks,kp,l,nage,3)
                s(2) = s(2) + dble(wetgridunc(ix,jy,ks,kp,l,nage))
                s(3) = s(3) + dble(drygridunc(ix,jy,ks,kp,l,nage))
              end do
            end do
            do jy = 0, myn-1
              do ix = 0, mxn-1
                wetgriduncn(ix,jy,ks,kp,l,nage) = gval(icase,ix,jy,0,ks,kp,l,nage,5)
                drygriduncn(ix,jy,ks,kp,l,nage) = gval(icase,ix,jy,0,ks,kp,l,nage,6)
                s(5) = s(5) + dble(wetgriduncn(ix,jy,ks,kp,l,nage))
                s(6) = s(6) + dble(drygriduncn(ix,jy,ks,kp,l,nage))
              end do
            end do
          end do
        end do
      end do
    end do
    do ks = 1, maxspec
      do i = 1, numreceptor
        creceptor(i,ks) = 0.125 * real(mod(3*i + ks + icase, 5)) * 2.**(-30)
        s(7) = s(7) + dble(creceptor(i,ks))
      end do
      do kp = 1, npt
        xmass(kp,ks) = 0.5*real(kp) + 0.25*real(ks)
      end do
    end do
    do i = 1, 7
      call emit('setup.fp', (/ dble(icase), dble(10+i) /), (/ s(i) /), .true.)
    end do
  end subroutine fill_case

  ! ------------------------------------------------------------------------
  ! One switch configuration, run through all three routines.
  subroutine run_concout_case(icase)
    integer, intent(in) :: icase
    integer :: iroutine, itime, ios
    real :: outnum
    real(sp) :: gridtotalunc
    real(dep_prec) :: wetgridtotalunc, drygridtotalunc

    nspec = maxspec
    maxpointspec_act = npt
    nageclass = maxageclass
    select case (icase)
    case (1)
      ldirect = 1; iout = 1; WETDEP = .true.;  DRYDEP = .true.;  outnum = 1.0
      outheight = (/ 100., 500., 1200. /); memind(1:2) = (/ 1, 2 /); loutaver = 10800
    case (2)
      ldirect = 1; iout = 2; WETDEP = .true.;  DRYDEP = .false.; outnum = 2.0
      outheight = (/ 60., 250., 6000. /);  memind(1:2) = (/ 2, 1 /); loutaver = 10800
    case (3)
      ldirect = 1; iout = 3; WETDEP = .false.; DRYDEP = .true.;  outnum = 2.5
      outheight = (/ 100., 500., 1200. /); memind(1:2) = (/ 2, 1 /); loutaver = 3600
    case (4)
      ldirect = 1; iout = 5; WETDEP = .true.;  DRYDEP = .true.;  outnum = 0.5
      outheight = (/ 60., 250., 6000. /);  memind(1:2) = (/ 1, 2 /); loutaver = 10800
    case (5)
      ldirect = -1; iout = 1; WETDEP = .false.; DRYDEP = .false.; outnum = 3.0
      outheight = (/ 100., 500., 1200. /); memind(1:2) = (/ 1, 2 /); loutaver = -10800
    case (6)
      ldirect = -1; iout = 3; WETDEP = .true.; DRYDEP = .true.; outnum = 1.5
      outheight = (/ 60., 250., 6000. /);  memind(1:2) = (/ 2, 1 /); loutaver = -7200
    end select
    itime = ldirect * 3600 * icase
    call emit('case', (/ dble(variant), dble(icase), dble(ldirect), dble(iout), &
         dble(merge(1,0,WETDEP)), dble(merge(1,0,DRYDEP)), dble(outnum), &
         dble(outheight), dble(memind(1)), dble(memind(2)), dble(loutaver), &
         dble(itime), dble(nspec), dble(maxpointspec_act), dble(nageclass) /), (/ 0d0 /))

    do iroutine = 1, 3
      call fill_case(icase)
      call delete_file('factor_drygrid')
      call delete_file('factor_drygrid_nest')
      call delete_file('factor_dryreceptor')
      open(unitoutrecept, file='rc_conc', form='unformatted', status='replace')
      open(unitoutreceptppt, file='rc_pptv', form='unformatted', status='replace')
      gridtotalunc = -9.
      wetgridtotalunc = -9.
      drygridtotalunc = -9.
      select case (iroutine)
      case (1)
        call concoutput(itime, outnum, gridtotalunc, wetgridtotalunc, drygridtotalunc)
      case (2)
        call concoutput_nest(itime, outnum)
      case (3)
        call concoutput_surf(itime, outnum, gridtotalunc, wetgridtotalunc, drygridtotalunc)
      end select
      close(unitoutrecept)
      close(unitoutreceptppt)
      call emit_dense(icase, iroutine)
      if (iroutine /= 2) then
        call emit('co.unc', (/ dble(variant), dble(icase), dble(iroutine) /), &
             (/ dble(gridtotalunc), dble(wetgridtotalunc), dble(drygridtotalunc) /))
      end if
      call emit_files(icase, iroutine, itime)
      ! The routines zero gridunc/griduncn and creceptor: record that too.
      call emit('co.reset', (/ dble(variant), dble(icase), dble(iroutine) /), &
           (/ dble(sum(abs(gridunc))), dble(sum(abs(griduncn))), dble(sum(abs(creceptor))) /))
    end do
    ios = 0
  end subroutine run_concout_case

  subroutine delete_file(fname)
    character(len=*), intent(in) :: fname
    logical :: ex
    inquire(file=fname, exist=ex)
    if (ex) then
      open(77, file=fname, status='old')
      close(77, status='delete')
    end if
  end subroutine delete_file

  ! Module arrays left by the routine: density, dry density, factor_drygrid,
  ! factor3d (what 1..4) per level, and the last slice's mean/sigma
  ! (what 5..10: grid, gridsigma, wetgrid, wetgridsigma, drygrid,
  ! drygridsigma) on the routine's own grid.
  subroutine emit_dense(icase, iroutine)
    integer, intent(in) :: icase, iroutine
    integer :: nx, ny, kz, ix, jy, n, iw
    double precision :: v(mxo*myo)
    if (iroutine == 2) then
      nx = mxn; ny = myn
    else
      nx = mxo; ny = myo
    end if
    do kz = 1, mzo
      do iw = 1, 6
        ! Densities and factors do not depend on the variant (V0 only); the
        ! last-slice mean/sigma are only informative with nclassunc > 1 (V1).
        if (iw <= 4 .and. variant /= 0) cycle
        if (iw >= 5 .and. variant /= 1) cycle
        n = 0
        do jy = 0, ny-1
          do ix = 0, nx-1
            n = n + 1
            select case (iw)
            case (1); v(n) = dble(densityoutgrid(ix,jy,kz))
            case (2); v(n) = dble(densitydrygrid(ix,jy,kz))
            case (3); v(n) = dble(factor_drygrid(ix,jy,kz))
            case (4); v(n) = dble(factor3d(ix,jy,kz))
            case (5); v(n) = dble(grid(ix,jy,kz))
            case (6); v(n) = dble(gridsigma(ix,jy,kz))
            end select
          end do
        end do
        call emit('co.dense', (/ dble(variant), dble(icase), dble(iroutine), dble(iw), &
             dble(kz) /), v(1:n))
      end do
    end do
    if (ldirect > 0 .and. variant == 1) then
      do iw = 7, 10
        if ((iw <= 8 .and. .not. WETDEP) .or. (iw >= 9 .and. .not. DRYDEP)) cycle
        n = 0
        do jy = 0, ny-1
          do ix = 0, nx-1
            n = n + 1
            select case (iw)
            case (7); v(n) = dble(wetgrid(ix,jy))
            case (8); v(n) = dble(wetgridsigma(ix,jy))
            case (9); v(n) = dble(drygrid(ix,jy))
            case (10); v(n) = dble(drygridsigma(ix,jy))
            end select
          end do
        end do
        call emit('co.dense', (/ dble(variant), dble(icase), dble(iroutine), dble(iw), &
             0d0 /), v(1:n))
      end do
    end if
  end subroutine emit_dense

  ! Read back every file the routine wrote. file: 1 grid_conc/grid_time,
  ! 2 grid_pptv, 3 factor_drygrid(_nest), 4 receptor_conc (unit 91),
  ! 5 receptor_pptv (unit 92), 6 factor_dryreceptor.
  subroutine emit_files(icase, iroutine, itime)
    integer, intent(in) :: icase, iroutine, itime
    character(len=8) :: adate
    character(len=6) :: atime
    character(len=3) :: anspec
    character(len=64) :: fname
    character(len=16) :: nest
    integer :: jjjjmmdd, ihmmss, ks
    real(kind=dp) :: jul
    jul = bdate + real(itime,kind=dp)/86400._dp
    call caldate(jul, jjjjmmdd, ihmmss)
    write(adate,'(i8.8)') jjjjmmdd
    write(atime,'(i6.6)') ihmmss
    nest = ''
    if (iroutine == 2) nest = 'nest_'
    do ks = 1, nspec
      write(anspec,'(i3.3)') ks
      if ((iout == 1) .or. (iout == 3) .or. (iout == 5)) then
        if (ldirect == 1) then
          fname = 'grid_conc_'//trim(nest)//adate//atime//'_'//anspec
        else
          fname = 'grid_time_'//trim(nest)//adate//atime//'_'//anspec
        end if
        call read_sparse(fname, .true., icase, iroutine, 100*ks + 1)
      end if
      if ((iout == 2) .or. (iout == 3)) then
        fname = 'grid_pptv_'//trim(nest)//adate//atime//'_'//anspec
        call read_sparse(fname, .true., icase, iroutine, 100*ks + 2)
      end if
    end do
    if (iroutine == 2) then
      call read_sparse('factor_drygrid_nest', .false., icase, iroutine, 3)
    else
      call read_sparse('factor_drygrid', .false., icase, iroutine, 3)
      call read_plain('rc_conc', icase, iroutine, 4)
      call read_plain('rc_pptv', icase, iroutine, 5)
      if (numreceptor > 0) call read_plain('factor_dryreceptor', icase, iroutine, 6)
    end if
  end subroutine emit_files

  ! A sparse file: [itime], then blocks of four records (count, indices,
  ! count, values). One row per block: n; i_1..i_n; m; v_1..v_m.
  subroutine read_sparse(fname, has_itime, icase, iroutine, ifile)
    character(len=*), intent(in) :: fname
    logical, intent(in) :: has_itime
    integer, intent(in) :: icase, iroutine, ifile
    integer :: it, n, m, i, ios, iblock
    integer :: ibuf(mxo*myo*mzo)
    real :: rbuf(mxo*myo*mzo)
    double precision :: o(2 + 2*mxo*myo*mzo)
    logical :: ex
    inquire(file=fname, exist=ex)
    if (.not. ex) then
      call emit('co.missing', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile) /), (/ 0d0 /))
      return
    end if
    open(78, file=fname, form='unformatted', status='old')
    if (has_itime) then
      read(78) it
      call emit('co.itime', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile) /), &
           (/ dble(it) /))
    end if
    iblock = 0
    do
      read(78, iostat=ios) n
      if (ios /= 0) exit
      read(78) (ibuf(i), i=1,n)
      read(78) m
      read(78) (rbuf(i), i=1,m)
      iblock = iblock + 1
      o(1) = dble(n)
      do i = 1, n
        o(1+i) = dble(ibuf(i))
      end do
      o(2+n) = dble(m)
      do i = 1, m
        o(2+n+i) = dble(rbuf(i))
      end do
      call emit('co.sparse', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile), &
           dble(iblock) /), o(1:2+n+m))
    end do
    close(78, status='delete')
  end subroutine read_sparse

  ! Receptor files: first record itime, then one record of numreceptor
  ! reals per species (factor_dryreceptor: one record).
  subroutine read_plain(fname, icase, iroutine, ifile)
    character(len=*), intent(in) :: fname
    integer, intent(in) :: icase, iroutine, ifile
    integer :: it, ios, irec, i
    real :: r(maxreceptor)
    logical :: ex
    inquire(file=fname, exist=ex)
    if (.not. ex) then
      call emit('co.missing', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile) /), (/ 0d0 /))
      return
    end if
    open(78, file=fname, form='unformatted', status='old')
    read(78, iostat=ios) it
    if (ios /= 0) then
      ! Empty file: nothing was written.
      call emit('co.recv', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile), &
           0d0 /), (/ -1d0 /))
      close(78, status='delete')
      return
    end if
    call emit('co.recv', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile), &
         0d0 /), (/ dble(it) /))
    irec = 0
    do
      read(78, iostat=ios) (r(i), i=1,numreceptor)
      if (ios /= 0) exit
      irec = irec + 1
      call emit('co.recv', (/ dble(variant), dble(icase), dble(iroutine), dble(ifile), &
           dble(irec) /), dble(r(1:numreceptor)))
    end do
    close(78, status='delete')
  end subroutine read_plain

  ! ========================================================================
  ! timemanager.f90 bookkeeping, extracted byte for byte (see file header).
  ! ========================================================================

  ! Output/sampling clock. Lines of timemanager.f90 copied verbatim are
  ! marked with their line number; `ev_*` assignments stand in for the CALL
  ! statements and are the only non-upstream statements inside the loop.
  subroutine tm_clock(isched)
    integer, intent(in) :: isched
    integer :: itime, loutnext, loutstart, loutend, ldeltat
    real :: outnum, weight
    integer :: ev_decay, ev_sample, ev_output, ev_resample, ev_split, ev_exit
    real :: ev_weight, ev_outnum_sample, ev_outnum_output

  loutnext=loutstep/2 !tm:118
  outnum=0. !tm:119
  loutstart=loutnext-loutaver/2 !tm:120
  loutend=loutnext+loutaver/2 !tm:121

  do itime=0,ideltas,lsynctime !tm:151
      ev_decay = 0; ev_sample = 0; ev_output = 0; ev_resample = 0
      ev_split = 0; ev_exit = 0; ev_weight = 0.; ev_outnum_sample = 0.
      ev_outnum_output = 0.; ldeltat = 0
    if (DEP.and.(itime.eq.loutnext).and.(ldirect.gt.0)) then !tm:264
      ev_decay = 1   ! the decay loop, timemanager.f90:265-298 (see tm_decaydep)
    endif !tm:299
    if ((ldirect*itime.ge.ldirect*loutstart).and. & !tm:345
         (ldirect*itime.le.ldirect*loutend)) then ! add to grid !tm:346
      if (mod(itime-loutstart,loutsample).eq.0) then !tm:347
        if ((itime.eq.loutstart).or.(itime.eq.loutend)) then !tm:353
          weight=0.5 !tm:354
        else !tm:355
          weight=1.0 !tm:356
        endif !tm:357
        outnum=outnum+weight !tm:358
        ev_sample = 1; ev_weight = weight; ev_outnum_sample = outnum   ! call conccalc, :359
      endif !tm:360
      if ((itime.eq.loutend).and.(outnum.gt.0.)) then !tm:371
        if ((iout.le.3.).or.(iout.eq.5)) then !tm:372
          ev_output = 1; ev_outnum_output = outnum   ! call concoutput*, :373-430
          outnum=0. !tm:431
        endif !tm:432
        loutnext=loutnext+loutstep !tm:452
        loutstart=loutnext-loutaver/2 !tm:453
        loutend=loutnext+loutaver/2 !tm:454
        if (itime.eq.loutstart) then !tm:455
          weight=0.5 !tm:456
          outnum=outnum+weight !tm:457
          ev_resample = 1   ! call conccalc, :458
        endif !tm:459
        if (ldirect*itime.ge.ldirect*itsplit) then !tm:468
          ev_split = 1   ! the split loop, :469-498 (see tm_split)
        endif !tm:499
      endif !tm:500
    endif !tm:501

    if (itime.eq.ideltas) ev_exit = 1   ! :504 is `if (itime.eq.ideltas) exit`; the exit is taken after the row below
    if (ev_exit == 0) then
    if (itime.lt.loutnext) then !tm:509
      ldeltat=itime-(loutnext-loutstep) !tm:510
    else                                  ! first half of next interval !tm:511
      ldeltat=itime-loutnext !tm:512
    endif !tm:513
    end if

      call emit('tm.clock', (/ dble(variant), dble(isched), dble(itime) /), &
           (/ dble(ev_decay), dble(ev_sample), dble(ev_weight), dble(ev_outnum_sample), &
           dble(ev_output), dble(ev_outnum_output), dble(ev_resample), dble(ev_split), &
           dble(ev_exit), dble(ldeltat), dble(outnum), dble(loutnext), dble(loutstart), &
           dble(loutend) /))
      if (ev_exit == 1) exit
    end do
  end subroutine tm_clock

  subroutine emit_clock()
    integer :: isched
    do isched = 1, 7
      DEP = .true.; iout = 1; itsplit = 999999999; ldirect = 1
      select case (isched)
      case (1)   ! the defaults of readcommand.f90: 3 h output, 900 s sync
        loutstep = 10800; loutaver = 10800; loutsample = 900; lsynctime = 900; ideltas = 43200
      case (2)   ! averaging shorter than the output step
        loutstep = 10800; loutaver = 3600; loutsample = 1800; lsynctime = 900; ideltas = 32400
      case (3)   ! backward run: readcommand.f90:629-631 negates the steps
        ldirect = -1
        loutstep = -10800; loutaver = -10800; loutsample = -900; lsynctime = -900; ideltas = -43200
      case (4)   ! odd half-intervals (integer division 2700/2 = 1350)
        loutstep = 8100; loutaver = 2700; loutsample = 900; lsynctime = 900; ideltas = 32400
      case (5)   ! iout = 4: plume trajectories only, outnum never reset
        iout = 4
        loutstep = 10800; loutaver = 10800; loutsample = 900; lsynctime = 900; ideltas = 32400
      case (6)   ! splitting switched on; DEP off
        DEP = .false.; itsplit = 14400
        loutstep = 7200; loutaver = 7200; loutsample = 1800; lsynctime = 600; ideltas = 28800
      case (7)   ! loutsample not dividing the window: loutend never sampled
        loutstep = 7200; loutaver = 5400; loutsample = 2700; lsynctime = 900; ideltas = 21600
      end select
      call emit('tm.sched', (/ dble(variant), dble(isched), dble(ldirect), &
           dble(loutstep), dble(loutaver), dble(loutsample), dble(lsynctime), &
           dble(ideltas), dble(itsplit), dble(iout), dble(merge(1,0,DEP)) /), (/ 0d0 /))
      call tm_clock(isched)
    end do
  end subroutine emit_clock

  ! ------------------------------------------------------------------------
  ! Decay of the deposition grids at the middle of the averaging interval.
  subroutine tm_decaydep(itime, loutnext)
    integer, intent(in) :: itime, loutnext
    integer :: ks, kp, nage, l, jy, ix

! >>> timemanager.f90:264-299 verbatim
    if (DEP.and.(itime.eq.loutnext).and.(ldirect.gt.0)) then
      do ks=1,nspec
      do kp=1,maxpointspec_act
        if (decay(ks).gt.0.) then
          do nage=1,nageclass
            do l=1,nclassunc
  ! Mother output grid
              do jy=0,numygrid-1
                do ix=0,numxgrid-1
                  wetgridunc(ix,jy,ks,kp,l,nage)= &
                       wetgridunc(ix,jy,ks,kp,l,nage)* &
                       exp(-1.*outstep*decay(ks))
                  drygridunc(ix,jy,ks,kp,l,nage)= &
                       drygridunc(ix,jy,ks,kp,l,nage)* &
                       exp(-1.*outstep*decay(ks))
                end do
              end do
  ! Nested output grid
              if (nested_output.eq.1) then
                do jy=0,numygridn-1
                  do ix=0,numxgridn-1
                    wetgriduncn(ix,jy,ks,kp,l,nage)= &
                         wetgriduncn(ix,jy,ks,kp,l,nage)* &
                         exp(-1.*outstep*decay(ks))
                    drygriduncn(ix,jy,ks,kp,l,nage)= &
                         drygriduncn(ix,jy,ks,kp,l,nage)* &
                         exp(-1.*outstep*decay(ks))
                  end do
                end do
              endif
            end do
          end do
        endif
      end do
      end do
    endif
! <<< end verbatim
  end subroutine tm_decaydep

  ! Cases: 1 decay due (mother + nest), 2 not at loutnext (no change),
  ! 3 nest output off, 4 backward (no decay), 5 a strong decay that
  ! underflows real(4)'s exp to 0 (exp(-108)).
  subroutine emit_decaydep()
    integer :: idc, itime, loutnext, ks, kp, l, nage, jy, ix, iarr, n
    double precision :: v(mxo*myo)
    do idc = 1, 5
      call fill_case(1)
      nspec = maxspec; maxpointspec_act = npt; nageclass = maxageclass
      DEP = .true.; ldirect = 1; nested_output = 1
      loutstep = 10800; outstep = real(abs(loutstep))
      loutnext = 16200; itime = 16200
      decay(1) = 2.0e-4
      if (maxspec >= 2) decay(2) = 0.
      select case (idc)
      case (2); itime = 15300
      case (3); nested_output = 0
      case (4); ldirect = -1
      case (5); decay(1) = 0.01
      end select
      call emit('tm.decaycase', (/ dble(variant), dble(idc), dble(itime), dble(loutnext), &
           dble(ldirect), dble(nested_output), dble(outstep), dble(decay(1:maxspec)) /), (/ 0d0 /))
      call tm_decaydep(itime, loutnext)
      ! Printed for the first uncertainty class and the last age class only
      ! (every species and release slot): the loop applies one factor per
      ! species to every class.
      do nage = nageclass, nageclass
        do l = 1, 1
          do kp = 1, maxpointspec_act
            do ks = 1, nspec
              do iarr = 1, 4
                n = 0
                if (iarr <= 2) then
                  do jy = 0, myo-1
                    do ix = 0, mxo-1
                      n = n + 1
                      if (iarr == 1) v(n) = dble(wetgridunc(ix,jy,ks,kp,l,nage))
                      if (iarr == 2) v(n) = dble(drygridunc(ix,jy,ks,kp,l,nage))
                    end do
                  end do
                else
                  do jy = 0, myn-1
                    do ix = 0, mxn-1
                      n = n + 1
                      if (iarr == 3) v(n) = dble(wetgriduncn(ix,jy,ks,kp,l,nage))
                      if (iarr == 4) v(n) = dble(drygriduncn(ix,jy,ks,kp,l,nage))
                    end do
                  end do
                end if
                call emit('tm.decaydep', (/ dble(variant), dble(idc), dble(iarr), dble(ks), &
                     dble(kp), dble(l), dble(nage) /), v(1:n))
              end do
            end do
          end do
        end do
      end do
    end do
    nested_output = 1
  end subroutine emit_decaydep

  ! ------------------------------------------------------------------------
  ! Before advance: release slot, age class, initialize trigger, backward
  ! scavenging initialisation. timemanager.f90:534-593 verbatim.
  subroutine tm_pre(itime, j, kp, nage)
    integer, intent(in) :: itime, j
    integer, intent(out) :: kp, nage
    integer :: ks, itage, idummy, loutnext
    real :: prob_rec(maxspec), grfraction(3), wetscav
    real :: xold, yold, zold

    loutnext = 10800    ! timemanager local; only passed on to get_wetscav
! >>> timemanager.f90:534-593 verbatim
        if (ioutputforeachrelease.eq.1) then
            kp=npoint(j)
        else
            kp=1
        endif
  ! Determine age class of the particle
        itage=abs(itra1(j)-itramem(j))
        do nage=1,nageclass
          if (itage.lt.lage(nage)) exit
        end do

  ! Initialize newly released particle
  !***********************************

        if ((itramem(j).eq.itime).or.(itime.eq.0)) &
             call initialize(itime,idt(j),uap(j),ucp(j),uzp(j), &
             us(j),vs(j),ws(j),xtra1(j),ytra1(j),ztra1(j),cbt(j))

  ! Memorize particle positions
  !****************************

        xold=xtra1(j)
        yold=ytra1(j)
        zold=ztra1(j)

   
  ! RECEPTOR: dry/wet depovel
  !****************************
  ! Before the particle is moved 
  ! the calculation of the scavenged mass shall only be done once after release
  ! xscav_frac1 was initialised with a negative value

      if  (DRYBKDEP) then
       do ks=1,nspec
         if  ((xscav_frac1(j,ks).lt.0)) then
            call get_vdep_prob(itime,xtra1(j),ytra1(j),ztra1(j),prob_rec)
            if (DRYDEPSPEC(ks)) then        ! dry deposition
               xscav_frac1(j,ks)=prob_rec(ks)
             else
                xmass1(j,ks)=0.
                xscav_frac1(j,ks)=0.
             endif
         endif
        enddo
       endif

       if (WETBKDEP) then 
       do ks=1,nspec
         if  ((xscav_frac1(j,ks).lt.0)) then
            call get_wetscav(itime,lsynctime,loutnext,j,ks,grfraction,idummy,idummy,wetscav)
            if (wetscav.gt.0) then
                xscav_frac1(j,ks)=wetscav* &
                       (zpoint2(npoint(j))-zpoint1(npoint(j)))*grfraction(1)
            else
                xmass1(j,ks)=0.
                xscav_frac1(j,ks)=0.
            endif
         endif
        enddo
       endif
! <<< end verbatim
    if (.false.) print *, xold, yold, zold
  end subroutine tm_pre

  subroutine emit_pre()
    integer :: ic, kp, nage, itime, j, ks
    double precision :: o(64)
    integer :: n
    nspec = maxspec
    nageclass = maxageclass
    lage(1) = 3600
    if (maxageclass >= 2) lage(2) = 86400
    lsynctime = 900
    zpoint1(1:2) = (/ 10., 100. /)
    zpoint2(1:2) = (/ 250., 100.5 /)
    do ic = 1, 12
      j = 7
      itime = 7200
      npoint(j) = 2
      itra1(j) = itime
      itramem(j) = 1800
      xtra1(j) = 3.25d0; ytra1(j) = 2.5d0; ztra1(j) = 120.
      xmass1(j,1:maxspec) = 0.75
      xscav_frac1(j,1:maxspec) = -1.
      ioutputforeachrelease = 0
      DRYBKDEP = .false.; WETBKDEP = .false.
      DRYDEPSPEC(1:maxspec) = .true.
      stub_prob_rec(1:maxspec) = 0.375
      if (maxspec >= 2) stub_prob_rec(2) = 0.625
      stub_grfraction = (/ 0.5, 0.25, 0.125 /)
      stub_wetscav = 3.0e-5
      select case (ic)
      case (2); ioutputforeachrelease = 1
      case (3); itramem(j) = 7200                 ! released now: initialize
      case (4); itime = 0; itra1(j) = 0; itramem(j) = 0
      case (5); itramem(j) = itime - 3600         ! itage == lage(1): class 2
      case (6); itramem(j) = itime - 90000        ! older than every class
      case (7); DRYBKDEP = .true.
      case (8); DRYBKDEP = .true.; DRYDEPSPEC(1) = .false.
      case (9); WETBKDEP = .true.
      case (10); WETBKDEP = .true.; stub_wetscav = 0.; npoint(j) = 1
      case (11); DRYBKDEP = .true.; WETBKDEP = .true.; xscav_frac1(j,1) = 0.5
      case (12); WETBKDEP = .true.; npoint(j) = 1; itramem(j) = itime + 3600  ! backward age
      end select
      itra1(j) = itime
      n_init = 0; n_vdep = 0; n_wetscav = 0
      call tm_pre(itime, j, kp, nage)
      n = 0
      call put(o, n, dble(kp)); call put(o, n, dble(nage)); call put(o, n, dble(n_init))
      call put(o, n, dble(n_vdep)); call put(o, n, dble(n_wetscav))
      do ks = 1, nspec
        call put(o, n, dble(xscav_frac1(j,ks)))
      end do
      do ks = 1, nspec
        call put(o, n, dble(xmass1(j,ks)))
      end do
      call emit('tm.pre', (/ dble(variant), dble(ic), dble(itime), dble(itra1(j)), &
           dble(itramem(j)), dble(npoint(j)), dble(ioutputforeachrelease), &
           dble(merge(1,0,DRYBKDEP)), dble(merge(1,0,WETBKDEP)), &
           dble(merge(1,0,DRYDEPSPEC(1))), dble(xscav_frac1_in(ic)), dble(stub_wetscav), &
           dble(stub_prob_rec(1:maxspec)), dble(stub_grfraction(1)), &
           dble(zpoint1(npoint(j))), dble(zpoint2(npoint(j))), dble(lage(1:nageclass)) /), &
           o(1:n))
    end do
    DRYBKDEP = .false.; WETBKDEP = .false.
  end subroutine emit_pre

  real function xscav_frac1_in(ic)
    integer, intent(in) :: ic
    if (ic == 11) then
      xscav_frac1_in = 0.5
    else
      xscav_frac1_in = -1.
    end if
  end function xscav_frac1_in

  subroutine put(o, n, x)
    double precision, intent(inout) :: o(:)
    integer, intent(inout) :: n
    double precision, intent(in) :: x
    n = n + 1
    o(n) = x
  end subroutine put

  ! ------------------------------------------------------------------------
  ! After advance. timemanager.f90:625-703 verbatim. `drydeposit` is a
  ! timemanager local that keeps its value from the previous particle when
  ! a species has no dry deposition; it is program-scope here for the same
  ! reason and is preset to a sentinel per case.
  subroutine tm_post(itime, j, nstop, prob, ldeltat, nage, kp, xmassfract)
    integer, intent(in) :: itime, j, nstop, ldeltat, nage, kp
    real, intent(in) :: prob(maxspec)
    real, intent(out) :: xmassfract
    integer :: ks
    real :: decfact

! >>> timemanager.f90:625-703 verbatim
        if (nstop.gt.1) then
          if (linit_cond.ge.1) call initial_cond_calc(itime,j)
          itra1(j)=-999999999
        else
          itra1(j)=itime+lsynctime


  ! Dry deposition and radioactive decay for each species
  ! Also check maximum (of all species) of initial mass remaining on the particle;
  ! if it is below a threshold value, terminate particle
  !*****************************************************************************

          xmassfract=0.
          do ks=1,nspec
            if (decay(ks).gt.0.) then             ! radioactive decay
              decfact=exp(-real(abs(lsynctime))*decay(ks))
            else
              decfact=1.
            endif

            if (DRYDEPSPEC(ks)) then        ! dry deposition
              drydeposit(ks)=xmass1(j,ks)*prob(ks)*decfact
              xmass1(j,ks)=xmass1(j,ks)*(1.-prob(ks))*decfact
              if (decay(ks).gt.0.) then   ! correct for decay (see wetdepo)
                drydeposit(ks)=drydeposit(ks)* &
                     exp(real(abs(ldeltat))*decay(ks))
              endif
            else                           ! no dry deposition
              xmass1(j,ks)=xmass1(j,ks)*decfact
            endif

! Skip check on mass fraction when npoint represents particle number
            if (mdomainfill.eq.0.and.mquasilag.eq.0) then
              if (xmass(npoint(j),ks).gt.0.) &
                   xmassfract=max(xmassfract,real(npart(npoint(j)))* &
                   xmass1(j,ks)/xmass(npoint(j),ks))
!ZHG 2015
                  !CGZ-lifetime: Check mass fraction left/save lifetime
                   ! if(real(npart(npoint(j)))*xmass1(j,ks)/xmass(npoint(j),ks).lt.e_inv.and.checklifetime(j,ks).eq.0.)then
                       !Mass below 1% of initial >register lifetime
                       ! checklifetime(j,ks)=abs(itra1(j)-itramem(j))
                       ! species_lifetime(ks,1)=species_lifetime(ks,1)+abs(itra1(j)-itramem(j))
                       ! species_lifetime(ks,2)= species_lifetime(ks,2)+1
                   ! endif
                   !CGZ-lifetime: Check mass fraction left/save lifetime
!ZHG 2015
            else
              xmassfract=1.0
            end if
          end do

          if (xmassfract.lt.minmass) then   ! terminate all particles carrying less mass
            itra1(j)=-999999999
            if (verbosity.gt.0) then
              print*,'terminated particle ',j,' for small mass'
            endif
          endif

  !        Sabine Eckhardt, June 2008
  !        don't create depofield for backward runs
          if (DRYDEP.AND.(ldirect.eq.1)) then
            call drydepokernel(nclass(j),drydeposit,real(xtra1(j)), &
                 real(ytra1(j)),nage,kp)
            if (nested_output.eq.1) call drydepokernel_nest( &
                 nclass(j),drydeposit,real(xtra1(j)),real(ytra1(j)), &
                 nage,kp)
          endif

  ! Terminate trajectories that are older than maximum allowed age
  !***************************************************************

          if (abs(itra1(j)-itramem(j)).ge.lage(nageclass)) then
            if (linit_cond.ge.1) call initial_cond_calc(itime+lsynctime,j)
            itra1(j)=-999999999
            if (verbosity.gt.0) then
              print*,'terminated particle ',j,' for age'
            endif
          endif
        endif
! <<< end verbatim
  end subroutine tm_post

  subroutine emit_post()
    integer :: ic, itime, j, nstop, ldeltat, nage, kp, ks, n
    real :: prob(maxspec), xmassfract, xmass1_in(maxspec)
    double precision :: o(64)
    double precision, parameter :: sentinel = -7d0
    nspec = maxspec
    nageclass = maxageclass
    j = 3
    do ic = 1, 16
      itime = 9000; nstop = 0; ldeltat = 900; nage = 1; kp = 1
      lsynctime = 900
      lage(1) = 172800
      if (maxageclass >= 2) then
        lage(1) = 3600; lage(2) = 172800
      end if
      itramem(j) = 0; npoint(j) = 2; nclass(j) = 3
      xtra1(j) = 4.375d0; ytra1(j) = 2.625d0
      xmass1(j,1:maxspec) = 0.5
      xmass(2,1:maxspec) = 1.0
      npart(2) = 1000
      prob(1:maxspec) = 0.25
      decay(1:maxspec) = 0.
      DRYDEPSPEC(1:maxspec) = .true.
      if (maxspec >= 2) then
        decay(2) = 2.0e-5; prob(2) = 0.0625; DRYDEPSPEC(2) = .false.
      end if
      mdomainfill = 0; mquasilag = 0; DRYDEP = .true.; ldirect = 1
      nested_output = 1; linit_cond = 0
      select case (ic)
      case (2);  decay(1) = 1.0e-5                       ! decay + deposit back-dating
      case (3);  decay(1) = 1.0e-5; ldeltat = 5400
      case (4);  decay(1) = 1.0e-5; ldeltat = -900       ! abs(ldeltat)
      case (5);  nstop = 3                                ! left the domain
      case (6);  nstop = 3; linit_cond = 1
      case (7);  xmass1(j,1:maxspec) = 6.0e-8            ! below minmass
      case (8);  xmass1(j,1:maxspec) = 2.5e-7            ! above minmass
      case (9);  xmass(2,1:maxspec) = 0.                 ! xmass 0: fraction stays 0
      case (10); xmass(2,1:maxspec) = 0.; mdomainfill = 1
      case (11); DRYDEP = .false.
      case (12); ldirect = -1; lsynctime = -900; itime = -9000
      case (13); nested_output = 0; prob(1) = 0.999
      case (14); itramem(j) = 9900 - lage(nageclass)       ! age limit reached
                 linit_cond = 1
      case (15); xmass1(j,1:maxspec) = 6.0e-8; linit_cond = 1
                 itramem(j) = 9900 - lage(nageclass)       ! minmass AND age
      case (16); decay(1) = 1.0e-3; lsynctime = 900; ldeltat = 3600
                 DRYDEPSPEC(1) = .false.
      end select
      xmass1_in = xmass1(j,:)
      itra1(j) = itime
      drydeposit = sentinel
      n_dry = 0; n_dryn = 0; n_icc = 0; icc_time = 0
      dry_dep = sentinel; dryn_dep = sentinel
      dry_x = 0.; dry_y = 0.; dryn_x = 0.; dryn_y = 0.
      dry_nunc = 0; dry_nage = 0; dry_kp = 0; dryn_nunc = 0; dryn_nage = 0; dryn_kp = 0
      xmassfract = -1.
      call tm_post(itime, j, nstop, prob, ldeltat, nage, kp, xmassfract)
      n = 0
      call put(o, n, dble(itra1(j)))
      do ks = 1, nspec
        call put(o, n, dble(xmass1(j,ks)))
      end do
      do ks = 1, nspec
        call put(o, n, dble(drydeposit(ks)))
      end do
      call put(o, n, dble(xmassfract))
      call put(o, n, dble(n_dry))
      call put(o, n, dble(dry_nunc)); call put(o, n, dble(dry_x)); call put(o, n, dble(dry_y))
      call put(o, n, dble(dry_nage)); call put(o, n, dble(dry_kp))
      do ks = 1, nspec
        call put(o, n, dble(dry_dep(ks)))
      end do
      call put(o, n, dble(n_dryn))
      call put(o, n, dble(dryn_nunc)); call put(o, n, dble(dryn_x)); call put(o, n, dble(dryn_y))
      do ks = 1, nspec
        call put(o, n, dble(dryn_dep(ks)))
      end do
      call put(o, n, dble(n_icc)); call put(o, n, dble(icc_time(1))); call put(o, n, dble(icc_time(2)))
      call emit('tm.post', (/ dble(variant), dble(ic), dble(itime), dble(nstop), dble(ldeltat), &
           dble(nage), dble(kp), dble(itramem(j)), dble(npoint(j)), dble(nclass(j)), &
           dble(xtra1(j)), dble(ytra1(j)), dble(lsynctime), dble(lage(nageclass)), &
           dble(nageclass), dble(npart(2)), dble(mdomainfill), dble(mquasilag), &
           dble(merge(1,0,DRYDEP)), dble(ldirect), dble(nested_output), dble(linit_cond), &
           dble(xmass1_in), dble(prob), dble(decay(1:maxspec)), &
           dble(merge(1,0,DRYDEPSPEC(1:maxspec))), dble(xmass(2,1:maxspec)) /), o(1:n))
    end do
  end subroutine emit_post

  ! ------------------------------------------------------------------------
  ! Particle splitting, timemanager.f90:468-499 verbatim.
  ! Operates on com_mod's numpart, as upstream does.
  subroutine tm_split(itime)
    integer, intent(in) :: itime
    integer :: n, j, ks

! >>> timemanager.f90:468-499 verbatim
        if (ldirect*itime.ge.ldirect*itsplit) then
          n=numpart
          do j=1,numpart
            if (ldirect*itime.ge.ldirect*itrasplit(j)) then
              if (n.lt.maxpart) then
                n=n+1
                itrasplit(j)=2*(itrasplit(j)-itramem(j))+itramem(j)
                itrasplit(n)=itrasplit(j)
                itramem(n)=itramem(j)
                itra1(n)=itra1(j)
                idt(n)=idt(j)
                npoint(n)=npoint(j)
                nclass(n)=nclass(j)
                xtra1(n)=xtra1(j)
                ytra1(n)=ytra1(j)
                ztra1(n)=ztra1(j)
                uap(n)=uap(j)
                ucp(n)=ucp(j)
                uzp(n)=uzp(j)
                us(n)=us(j)
                vs(n)=vs(j)
                ws(n)=ws(j)
                cbt(n)=cbt(j)
                do ks=1,nspec
                  xmass1(j,ks)=xmass1(j,ks)/2.
                  xmass1(n,ks)=xmass1(j,ks)
                end do
              endif
            endif
          end do
          numpart=n
        endif
! <<< end verbatim
  end subroutine tm_split

  ! Particles get itrasplit = itra1 + ldirect*itsplit at release
  ! (releaseparticles.f90:187); here set directly. Case 3 fills the particle
  ! store up to maxpart - 1 so the maxpart cap stops the second split.
  subroutine emit_split()
    integer :: ic, itime, np, j, j0, ks
    double precision :: o(64)
    integer :: n
    nspec = maxspec
    do ic = 1, 3
      ldirect = 1; itime = 14400; itsplit = 7200
      select case (ic)
      case (1); np = 5
      case (2); np = 4; ldirect = -1; itime = -14400; itsplit = 7200
      case (3); np = maxpart - 1
      end select
      j0 = max(1, np - 4)
      do j = 1, np
        itramem(j) = ldirect * 600 * mod(j, 4)
        itrasplit(j) = itramem(j) + ldirect * 3600 * (1 + mod(j, 5))
        if (ic == 3 .and. j < j0) itrasplit(j) = ldirect * 999999
        itra1(j) = itime; idt(j) = 60 + j; npoint(j) = 1 + mod(j, 2); nclass(j) = 1 + mod(j, 3)
        xtra1(j) = 1.25d0 + 0.5d0*dble(mod(j,7)); ytra1(j) = 2.0d0 + 0.25d0*dble(mod(j,3))
        ztra1(j) = 50. * real(1 + mod(j,4))
        uap(j) = 0.5; ucp(j) = -0.25; uzp(j) = 0.125
        us(j) = 1.; vs(j) = 2.; ws(j) = 0.5; cbt(j) = int(mod(j,2), kind=2)
        do ks = 1, nspec
          xmass1(j,ks) = 0.375 * real(ks) + 0.0625 * real(mod(j,5))
        end do
      end do
      call emit('tm.splitcase', (/ dble(variant), dble(ic), dble(ldirect), dble(itime), &
           dble(itsplit), dble(np), dble(maxpart) /), (/ 0d0 /))
      numpart = np
      call tm_split(itime)
      np = numpart
      do j = j0, np
        n = 0
        call put(o, n, dble(itrasplit(j))); call put(o, n, dble(itramem(j)))
        call put(o, n, dble(itra1(j))); call put(o, n, dble(idt(j)))
        call put(o, n, dble(npoint(j))); call put(o, n, dble(nclass(j)))
        call put(o, n, dble(xtra1(j))); call put(o, n, dble(ytra1(j)))
        call put(o, n, dble(ztra1(j))); call put(o, n, dble(cbt(j)))
        do ks = 1, nspec
          call put(o, n, dble(xmass1(j,ks)))
        end do
        call emit('tm.split', (/ dble(variant), dble(ic), dble(j), dble(np) /), o(1:n))
      end do
    end do
  end subroutine emit_split

end program flexpart_reference_concout

! --------------------------------------------------------------------------
! Recording stubs for the routines the extracted timemanager blocks call.
! They are NOT upstream; each records its arguments (see the file header).
subroutine drydepokernel(nunc,deposit,x,y,nage,kp)
  use par_mod, only: maxspec, dep_prec
  use concout_stub_mod
  implicit none
  real(dep_prec), dimension(maxspec) :: deposit
  real :: x,y
  integer :: nunc,nage,kp
  n_dry = n_dry + 1
  dry_nunc = nunc; dry_dep = deposit; dry_x = x; dry_y = y; dry_nage = nage; dry_kp = kp
end subroutine drydepokernel

subroutine drydepokernel_nest(nunc,deposit,x,y,nage,kp)
  use par_mod, only: maxspec, dep_prec
  use concout_stub_mod
  implicit none
  real(dep_prec), dimension(maxspec) :: deposit
  real :: x,y
  integer :: nunc,nage,kp
  n_dryn = n_dryn + 1
  dryn_nunc = nunc; dryn_dep = deposit; dryn_x = x; dryn_y = y; dryn_nage = nage; dryn_kp = kp
end subroutine drydepokernel_nest

subroutine initial_cond_calc(itime,i)
  use concout_stub_mod
  implicit none
  integer :: itime,i
  n_icc = n_icc + 1
  if (n_icc <= 4) icc_time(n_icc) = itime
  if (i < 0) stop 'initial_cond_calc stub'
end subroutine initial_cond_calc

subroutine initialize(itime,ldt,up,vp,wp,usigold,vsigold,wsigold,xt,yt,zt,icbt)
  use par_mod, only: dp
  use concout_stub_mod
  implicit none
  integer :: itime, ldt
  integer(kind=2) :: icbt
  real :: zt,up,vp,wp,usigold,vsigold,wsigold
  real(kind=dp) :: xt,yt
  n_init = n_init + 1
  if (itime < -2000000000) print *, ldt, icbt, zt, up, vp, wp, usigold, vsigold, wsigold, xt, yt
end subroutine initialize

subroutine get_vdep_prob(itime,xt,yt,zt,prob)
  use par_mod, only: dp, maxspec
  use concout_stub_mod
  implicit none
  integer :: itime
  real(kind=dp) :: xt,yt
  real :: zt, prob(maxspec)
  n_vdep = n_vdep + 1
  prob = stub_prob_rec
  if (itime < -2000000000) print *, xt, yt, zt
end subroutine get_vdep_prob

subroutine get_wetscav(itime,ltsample,loutnext,jpart,ks,grfraction,inc_count,blc_count,wetscav)
  use concout_stub_mod
  implicit none
  integer :: itime,ltsample,loutnext,jpart,ks
  integer :: inc_count, blc_count
  real :: grfraction(3), wetscav
  n_wetscav = n_wetscav + 1
  grfraction = stub_grfraction
  wetscav = stub_wetscav
  if (itime < -2000000000) print *, ltsample, loutnext, jpart, ks, inc_count, blc_count
end subroutine get_wetscav
