! SPDX-License-Identifier: GPL-3.0
!
! Code-to-code reference driver for `changi::flexpart::cmapf` and
! `changi::flexpart::coordtrafo`: Albion Taylor's (NOAA/ARL) conformal map
! library as distributed inside FLEXPART (cmapf_mod.f90, with A. Stohl's
! double-precision changes), and the release-point transformation
! coordtrafo.f90.
!
! Every routine called here is UPSTREAM FLEXPART v10.4 @ 3d7eebf, compiled from
! upstream_source/FLEXPART/src by dev/build_reference_cmapf.sh. Nothing below
! reimplements a projection: the driver only sets inputs and prints what
! upstream returns.
!
! TWO COPIES OF cmapf_mod ARE LINKED, and why. cmapf_mod.f90 declares
! `private` and makes only six routines public (cc2gll, cll2xy, cgszll, cxy2ll,
! stlmbr, stcm2p). The other thirteen (cspanf, eqvlat, stcm1p, cnllxy, cnxyll,
! cgszxy, cg2cll, cg2cxy, ccrvll, ccrvxy, cpolll, cpolxy) cannot be called
! from outside the module. So the build links
!   - cmapf_mod.f90 VERBATIM, used for the six public routines and for the
!     polar maps exactly as gridcheck_ecmwf.f90 builds them; and
!   - cmapf_open_mod, a copy generated at build time by a sed that renames the
!     module and deletes the single line `  private` -- nothing else. The
!     build script asserts that the diff is exactly those three lines.
! Every public routine is called through BOTH copies and the driver stops
! with an error if the two ever differ, which is the evidence that the copy
! computes what the verbatim module computes.
!
! PRECISION STRUCTURE (upstream's, reproduced by calling upstream): strcmp(9),
! latitudes, longitudes, x, y and the wind components are default `real`;
! A. Stohl made xi, eta (and xi0, eta0 in the xy routines) real(kind=dp) "to
! avoid problems at poles" -- but cnllxy's own xi, eta outputs, cll2xy's
! locals and cg2cxy's `radial` are still default real. The driver therefore
! passes default reals everywhere except cnxyll's xi, eta, which are dp.
! In the real(4) build the dp and real values coexist; in the real(8) build
! (-fdefault-real-8 -fdefault-double-8) everything is 8 bytes.
!
! Row format, one row per CALL:
!     function,a1;a2;...;an,o1;o2;...;om
! EVERY row is printed with rowvd (ES25.17E3) after dble(): rows mix default
! real and real(kind=dp) values, and a real(4) value widened to double prints
! exactly, so the test parses every function as f64 (all names are in its
! dp_functions list) and loses nothing.
!
! coordtrafo writes NOTICE lines with list-directed write(*,*); those start
! with a blank and are filtered out by the build script (grep -v '^ ').

! ============================================================================
! I/O helpers (copied from dev/flexpart_reference_physics.f90).
module cmapf_ref_io
  implicit none
contains

  subroutine fmtnum(x, buf)
    double precision, intent(in) :: x
    character(len=32), intent(out) :: buf
    if (kind(1.0) == 4) then
      write(buf,'(ES16.8E3)') x
    else
      write(buf,'(ES25.17E3)') x
    end if
    buf = adjustl(buf)
  end subroutine fmtnum

  subroutine rowv(name, args, nargs, outs, nouts)
    character(len=*), intent(in) :: name
    integer, intent(in) :: nargs, nouts
    real, intent(in) :: args(nargs), outs(nouts)
    character(len=8192) :: line
    character(len=32) :: buf
    integer :: i
    line = trim(name) // ','
    do i = 1, nargs
      call fmtnum(dble(args(i)), buf)
      line = trim(line) // trim(adjustl(buf))
      if (i < nargs) line = trim(line) // ';'
    end do
    line = trim(line) // ','
    do i = 1, nouts
      call fmtnum(dble(outs(i)), buf)
      line = trim(line) // trim(adjustl(buf))
      if (i < nouts) line = trim(line) // ';'
    end do
    write(*,'(A)') trim(line)
  end subroutine rowv

  subroutine rowvd(name, args, nargs, outs, nouts)
    character(len=*), intent(in) :: name
    integer, intent(in) :: nargs, nouts
    double precision, intent(in) :: args(nargs), outs(nouts)
    character(len=8192) :: line
    character(len=32) :: buf
    integer :: i
    line = trim(name) // ','
    do i = 1, nargs
      write(buf,'(ES25.17E3)') args(i)
      line = trim(line) // trim(adjustl(buf))
      if (i < nargs) line = trim(line) // ';'
    end do
    line = trim(line) // ','
    do i = 1, nouts
      write(buf,'(ES25.17E3)') outs(i)
      line = trim(line) // trim(adjustl(buf))
      if (i < nouts) line = trim(line) // ';'
    end do
    write(*,'(A)') trim(line)
  end subroutine rowvd

  ! Same value, NaN-aware (both NaN counts as the same).
  logical function same(a, b)
    real, intent(in) :: a, b
    same = (a == b) .or. ((a /= a) .and. (b /= b))
  end function same

  subroutine assert_same(what, a, b)
    character(len=*), intent(in) :: what
    real, intent(in) :: a, b
    if (.not. same(a, b)) then
      write(0,*) 'verbatim cmapf_mod and cmapf_open_mod disagree in ', what, a, b
      error stop 2
    end if
  end subroutine assert_same
end module cmapf_ref_io

! ============================================================================
! The nine maps every ll/xy routine is swept over (filled by build_maps).
module cmapf_ref_maps
  implicit none
  integer, parameter :: nmaps = 9
  real :: maps(9, nmaps)
  ! Latitudes straddling every pole guard: cnllxy's |sin| >= almst1 (|lat| >
  ! ~89.974), the ll routines' |lat| > 89.985 (89.985 itself is ON the guard),
  ! cgszxy's |ymerc| > 6 (|lat| > ~89.71), and stlmbr's +-89 radial bounds.
  integer, parameter :: nlat = 14
  real, parameter :: lats(nlat) = (/ -90., -89.99, -89.985, -89.9, -89.8, &
       -60., -15., 0., 45., 89.8, 89.9, 89.985, 89.99, 90. /)
  ! Longitude OFFSETS from the map's reference longitude strcmp(2): the cut of
  ! the map is at +-180 from it, so these straddle the cut and cspanf's wrap
  ! (365 = one full turn + 5). One absolute longitude near the dateline is
  ! added per map (crosses the dateline for maps whose cut is elsewhere).
  integer, parameter :: noff = 8
  real, parameter :: offs(noff) = (/ -180., -179.999, -60., 0., 0.004, &
       179.999, 180., 365. /)
  real, parameter :: dateline = 179.9995
contains
  ! Maps 7 to 9 are swept on a reduced set: 8 latitudes x 3 longitudes.
  logical function keep(m, i, j)
    integer, intent(in) :: m, i, j
    if (m <= 6) then
      keep = .true.
    else
      keep = any(i == (/ 1, 3, 4, 6, 9, 11, 12, 14 /)) .and. any(j == (/ 1, 4, 9 /))
    end if
  end function keep
end module cmapf_ref_maps

! ============================================================================
! cspanf: wrap edges (exactly on begin/end, one ulp-ish either side, several
! turns), both orders of (begin, end), and a span that is not a full turn.
subroutine emit_cspanf()
  use cmapf_ref_io
  use cmapf_open_mod, only: cspanf
  implicit none
  integer, parameter :: nv = 16, ns = 4
  real :: vals(nv), b(ns), e(ns), r
  integer :: i, j
  vals = (/ -900., -540., -360., -180.0001, -180., -179.9999, -0.0001, 0., &
       0.0001, 90., 179.9999, 180., 180.0001, 360., 540.25, 1.e4 /)
  b = (/ -180., 0., 180., -90. /)
  e = (/ 180., 360., -180., 90. /)
  do j = 1, ns
    do i = 1, nv
      r = cspanf(vals(i), b(j), e(j))
      call rowvd('cspanf', (/ dble(vals(i)), dble(b(j)), dble(e(j)) /), 3, &
           (/ dble(r) /), 1)
    end do
  end do
end subroutine emit_cspanf

! ============================================================================
! eqvlat: the |sin1 - sin2| > .001 log branch and the series branch, both
! hemispheres, reversed order, and the coincident-pole case.
subroutine emit_eqvlat()
  use cmapf_ref_io
  use cmapf_open_mod, only: eqvlat
  implicit none
  integer, parameter :: n = 11
  real :: l1(n), l2(n), r
  integer :: i
  l1 = (/ 30., 60., -30., 40., 40., 0., 10., 89., 90., -90., 33. /)
  l2 = (/ 60., 30., -60., 40., 40.05, 10., -10., 89.5, 90., -90., 45. /)
  do i = 1, n
    r = eqvlat(l1(i), l2(i))
    call rowvd('eqvlat', (/ dble(l1(i)), dble(l2(i)) /), 2, (/ dble(r) /), 1)
  end do
end subroutine emit_eqvlat

! ============================================================================
! stlmbr over tangent latitudes (poles, Mercator, near-Mercator, Lambert in
! both hemispheres) and reference longitudes needing cspanf's wrap.
subroutine emit_stlmbr()
  use cmapf_ref_io
  use cmapf_mod, only: stlmbr
  implicit none
  integer, parameter :: nt = 8, nl = 5
  real :: t(nt), xl(nl), s(9)
  integer :: i, j
  t = (/ 90., -90., 0., 0.4, -0.4, 45., -35., 89.99 /)
  xl = (/ 0., -100., 180., -180., 200. /)
  do i = 1, nt
    do j = 1, nl
      call stlmbr(s, t(i), xl(j))
      call rowvd('stlmbr', (/ dble(t(i)), dble(xl(j)) /), 2, dble(s), 9)
    end do
  end do
end subroutine emit_stlmbr

! ============================================================================
! Build the nine maps, emitting each stlmbr / stcm2p / stcm1p call.
!   1  northpolemap, exactly as gridcheck_ecmwf.f90:345-351 (dy = 0.5 deg)
!   2  southpolemap, exactly as gridcheck_ecmwf.f90:331-337
!   3  Lambert conformal, tangent eqvlat(30,60), stcm2p
!   4  southern Lambert (tangent -35), rotated grid, stcm1p (orient 30 deg)
!   5  Mercator (tangent 0), stcm2p
!   6  near-Mercator (tangent 0.4 deg, gamma ~ 7e-3), stcm1p, xlong 200
!   7  near-north-polar (tangent 89.5, gamma = 0.99996), stcm2p
!   8  near-south-polar (tangent -89.5), stcm1p
! Maps 7 and 8 put |gamma| strictly between cgszll's 0.9999 guard and 1, so
! the guard's placement (not only its existence) is exercised.
!   9  north polar stereographic straight from stlmbr (no stcm call): its
!      ccrvxy zero, x = 0, y = 1, is exactly representable, so ccrvxy's
!      `temp == 0` branch for |gamma| == 1 is reached. On the gridcheck polar
!      maps no x within 2e5 ulps of the pole makes xpolg exactly 0 (searched
!      with the port), so that branch is unreachable there.
subroutine build_maps()
  use par_mod, only: switchnorth, switchsouth
  use cmapf_ref_io
  use cmapf_ref_maps
  use cmapf_mod, only: stlmbr
  use cmapf_open_mod, only: eqvlat
  implicit none
  real :: s(9), dy, sizenorth, sizesouth, tl
  integer :: m

  dy = 0.5
  sizenorth = 6.*(90.-switchnorth)/dy
  call stlmbr(s, 90., 0.)
  call row_stlmbr(90., 0., s)
  call do_stcm2p(s, 0.,0.,switchnorth,0.,sizenorth,sizenorth,switchnorth,180.)
  maps(:,1) = s

  sizesouth = 6.*(switchsouth+90.)/dy
  call stlmbr(s, -90., 0.)
  call row_stlmbr(-90., 0., s)
  call do_stcm2p(s, 0.,0.,switchsouth,0.,sizesouth,sizesouth,switchsouth,180.)
  maps(:,2) = s

  tl = eqvlat(30., 60.)
  call stlmbr(s, tl, -100.)
  call row_stlmbr(tl, -100., s)
  call do_stcm2p(s, 1.,1., 20.,-125., 101.,81., 52.,-60.)
  maps(:,3) = s

  call stlmbr(s, -35., 150.)
  call row_stlmbr(-35., 150., s)
  call do_stcm1p(s, 10.,20., -30.,145., -35.,150., 25., 30.)
  maps(:,4) = s

  call stlmbr(s, 0., 120.)
  call row_stlmbr(0., 120., s)
  call do_stcm2p(s, 0.,0., -10.,100., 80.,40., 15.,140.)
  maps(:,5) = s

  call stlmbr(s, 0.4, 200.)
  call row_stlmbr(0.4, 200., s)
  call do_stcm1p(s, 0.,0., 0.,200., 0.,-160., 50., 0.)
  maps(:,6) = s

  call stlmbr(s, 89.5, 30.)
  call row_stlmbr(89.5, 30., s)
  call do_stcm2p(s, 0.,0., 80.,30., 50.,50., 80.,210.)
  maps(:,7) = s

  call stlmbr(s, -89.5, -30.)
  call row_stlmbr(-89.5, -30., s)
  call do_stcm1p(s, 0.,0., -80.,-30., -85.,-30., 20., 0.)
  maps(:,8) = s

  call stlmbr(s, 90., 0.)
  call row_stlmbr(90., 0., s)
  maps(:,9) = s

  do m = 1, nmaps
    call rowvd('map', (/ dble(m) /), 1, dble(maps(:,m)), 9)
  end do

contains

  subroutine row_stlmbr(t, xl, s)
    real, intent(in) :: t, xl, s(9)
    call rowvd('stlmbr', (/ dble(t), dble(xl) /), 2, dble(s), 9)
  end subroutine row_stlmbr

  subroutine do_stcm2p(s, x1,y1,xlat1,xlong1,x2,y2,xlat2,xlong2)
    use cmapf_mod, only: stcm2p
    real, intent(inout) :: s(9)
    real, intent(in) :: x1,y1,xlat1,xlong1,x2,y2,xlat2,xlong2
    real :: a(17)
    a(1:9) = s
    a(10:17) = (/ x1,y1,xlat1,xlong1,x2,y2,xlat2,xlong2 /)
    call stcm2p(s, x1,y1,xlat1,xlong1,x2,y2,xlat2,xlong2)
    call rowvd('stcm2p', dble(a), 17, dble(s), 9)
  end subroutine do_stcm2p

  subroutine do_stcm1p(s, x1,y1,xlat1,xlong1,xlatg,xlongg,gridsz,orient)
    use cmapf_open_mod, only: stcm1p
    real, intent(inout) :: s(9)
    real, intent(in) :: x1,y1,xlat1,xlong1,xlatg,xlongg,gridsz,orient
    real :: a(17)
    a(1:9) = s
    a(10:17) = (/ x1,y1,xlat1,xlong1,xlatg,xlongg,gridsz,orient /)
    call stcm1p(s, x1,y1,xlat1,xlong1,xlatg,xlongg,gridsz,orient)
    call rowvd('stcm1p', dble(a), 17, dble(s), 9)
  end subroutine do_stcm1p

end subroutine build_maps

! ============================================================================
! Routines taking a geographic position, on every map x every (lat, lon).
subroutine emit_ll()
  use cmapf_ref_io
  use cmapf_ref_maps
  use cmapf_mod, only: cll2xy, cgszll, cc2gll
  use cmapf_open_mod, only: cnllxy, cg2cll, ccrvll, cpolll, &
       cll2xy_o => cll2xy, cgszll_o => cgszll, cc2gll_o => cc2gll
  implicit none
  real :: s(9), lat, lon, xi, eta, x, y, xo, yo, g, go, ue, vn, ug, vg, &
       ugo, vgo, ue2, vn2, gx, gy, enx, eny, enz
  integer :: m, i, j
  do m = 1, nmaps
    s = maps(:,m)
    do i = 1, nlat
      do j = 1, noff + 1
        if (.not. keep(m, i, j)) cycle
        lat = lats(i)
        if (j <= noff) then
          lon = s(2) + offs(j)
        else
          lon = dateline
        end if
        ue = 10. + lat/10.
        vn = -4. + lon/50.

        call cnllxy(s, lat, lon, xi, eta)
        call rowvd('cnllxy', (/ dble(m), dble(lat), dble(lon) /), 3, &
             (/ dble(xi), dble(eta) /), 2)

        call cll2xy(s, lat, lon, x, y)
        call cll2xy_o(s, lat, lon, xo, yo)
        call assert_same('cll2xy x', x, xo)
        call assert_same('cll2xy y', y, yo)
        call rowvd('cll2xy', (/ dble(m), dble(lat), dble(lon) /), 3, &
             (/ dble(x), dble(y) /), 2)

        g = cgszll(s, lat, lon)
        go = cgszll_o(s, lat, lon)
        call assert_same('cgszll', g, go)
        call rowvd('cgszll', (/ dble(m), dble(lat), dble(lon) /), 3, &
             (/ dble(g) /), 1)

        call cc2gll(s, lat, lon, ue, vn, ug, vg)
        call cc2gll_o(s, lat, lon, ue, vn, ugo, vgo)
        call assert_same('cc2gll ug', ug, ugo)
        call assert_same('cc2gll vg', vg, vgo)
        call rowvd('cc2gll', (/ dble(m), dble(lat), dble(lon), dble(ue), dble(vn) /), 5, &
             (/ dble(ug), dble(vg) /), 2)

        call cg2cll(s, lat, lon, ue, vn, ue2, vn2)
        call rowvd('cg2cll', (/ dble(m), dble(lat), dble(lon), dble(ue), dble(vn) /), 5, &
             (/ dble(ue2), dble(vn2) /), 2)

        call ccrvll(s, lat, lon, gx, gy)
        call rowvd('ccrvll', (/ dble(m), dble(lat), dble(lon) /), 3, &
             (/ dble(gx), dble(gy) /), 2)

        call cpolll(s, lat, lon, enx, eny, enz)
        call rowvd('cpolll', (/ dble(m), dble(lat), dble(lon) /), 3, &
             (/ dble(enx), dble(eny), dble(enz) /), 3)
      end do
    end do
  end do
end subroutine emit_ll

! ============================================================================
! Routines taking map (x, y) or canonical (xi, eta) coordinates. The points
! are (a) the images of every (lat, lon) of emit_ll under cll2xy / cnllxy, and
! (b) a 5 x 5 scan around the image of the map's pole (x, y offsets of
! -20, -0.01, 0, 0.01, 20 grid units; xi, eta offsets of -0.3, -1e-4, 0,
! 1e-4, 0.3 around (0, 1/gamma)) -- the Mercator maps (gamma = 0) have no
! finite pole and are scanned around their origin instead.
subroutine emit_xy()
  use par_mod, only: dp
  use cmapf_ref_io
  use cmapf_ref_maps
  use cmapf_mod, only: cll2xy, cxy2ll
  use cmapf_open_mod, only: cnllxy, cnxyll, cgszxy, cg2cxy, ccrvxy, cpolxy, &
       cxy2ll_o => cxy2ll
  implicit none
  real :: s(9), lat, lon, xi, eta, x, y, xc, yc, xz, yz
  real(kind=dp) :: xic, etac, tz
  real :: dxy(5)
  real(kind=dp) :: dxe(5)
  integer :: m, i, j, k, l, n
  dxy = (/ -20., -0.01, 0., 0.01, 20. /)
  dxe = (/ -0.3_dp, -1.e-4_dp, 0._dp, 1.e-4_dp, 0.3_dp /)
  do m = 1, nmaps
    s = maps(:,m)
    n = 0
    do i = 1, nlat
      do j = 1, noff + 1
        if (.not. keep(m, i, j)) cycle
        lat = lats(i)
        if (j <= noff) then
          lon = s(2) + offs(j)
        else
          lon = dateline
        end if
        call cll2xy(s, lat, lon, x, y)
        call cnllxy(s, lat, lon, xi, eta)
        ! At |lat| >= 89.985, |sin(lat)| >= almst1 and cnllxy maps every
        ! longitude to the pole's image (0, 1/gamma): emit that point once.
        ! (Decided on the input latitude, so both builds emit the same rows.)
        if (abs(lat) >= 89.985 .and. j > 1) cycle
        n = n + 1
        call one_point(m, s, x, y, real(xi, dp), real(eta, dp), n)
      end do
    end do
    if (s(1) /= 0.) then
      call cll2xy(s, sign(90., s(1)), s(2), xc, yc)
      xic = 0._dp
      etac = 1._dp / s(1)
    else
      xc = s(3)
      yc = s(4)
      xic = 0._dp
      etac = 0._dp
    end if
    do k = 1, 5
      do l = 1, 5
        n = n + 1
        call one_point(m, s, xc + dxy(k), yc + dxy(l), xic + dxe(k), etac + dxe(l), n)
      end do
    end do
    ! The exact zero of ccrvxy's (xpolg, ypolg) -- x = s3 + s6/t,
    ! y = s4 + s5/t with t = gamma*s7/rearth -- so that ccrvxy's
    ! `temp == 0` branch is reached on maps where it is representable.
    if (s(1) /= 0.) then
      xc = s(3) + s(6) / (s(1) * s(7) / 6371.2)
      yc = s(4) + s(5) / (s(1) * s(7) / 6371.2)
      ! Search up to 64 ulps either side for an x (and a y) at which
      ! upstream's own expression, in this build's precision, is exactly 0:
      ! temp = s1*s7/rearth (default real, stored dp), s6 + temp*(s3 - x).
      tz = s(1) * s(7) / 6371.2
      xz = xc
      yz = yc
      do k = 0, 128
        x = xc
        do l = 1, k / 2
          x = nearest(x, sign(1., real(mod(k, 2)) - 0.5))
        end do
        if (s(6) + tz * (s(3) - x) == 0._dp) then
          xz = x
          exit
        end if
      end do
      do k = 0, 128
        y = yc
        do l = 1, k / 2
          y = nearest(y, sign(1., real(mod(k, 2)) - 0.5))
        end do
        if (s(5) + tz * (s(4) - y) == 0._dp) then
          yz = y
          exit
        end if
      end do
      n = n + 1
      call one_point(m, s, xz, yz, xic, etac, n)
    end if
  end do

contains

  subroutine one_point(m, s, x, y, xid, etad, n)
    integer, intent(in) :: m, n
    real, intent(in) :: s(9), x, y
    real(kind=dp), intent(in) :: xid, etad
    real :: lat, lon, lato, lono, g, ug, vg, ue, vn, gx, gy, enx, eny, enz
    real(kind=dp) :: xiv, etav

    ! cnllxy's dummies are dp; pass copies (cnxyll does not modify them,
    ! but the copies keep the driver's loop values safe regardless).
    xiv = xid
    etav = etad
    call cnxyll(s, xiv, etav, lat, lon)
    call rowvd('cnxyll', (/ dble(m), dble(xid), dble(etad) /), 3, &
         (/ dble(lat), dble(lon) /), 2)

    call cxy2ll(s, x, y, lat, lon)
    call cxy2ll_o(s, x, y, lato, lono)
    call assert_same('cxy2ll lat', lat, lato)
    call assert_same('cxy2ll lon', lon, lono)
    call rowvd('cxy2ll', (/ dble(m), dble(x), dble(y) /), 3, &
         (/ dble(lat), dble(lon) /), 2)

    g = cgszxy(s, x, y)
    call rowvd('cgszxy', (/ dble(m), dble(x), dble(y) /), 3, (/ dble(g) /), 1)

    ug = 5. + 0.1*real(mod(n, 17))
    vg = -3. + 0.2*real(mod(n, 11))
    call cg2cxy(s, x, y, ug, vg, ue, vn)
    call rowvd('cg2cxy', (/ dble(m), dble(x), dble(y), dble(ug), dble(vg) /), 5, &
         (/ dble(ue), dble(vn) /), 2)

    call ccrvxy(s, x, y, gx, gy)
    call rowvd('ccrvxy', (/ dble(m), dble(x), dble(y) /), 3, &
         (/ dble(gx), dble(gy) /), 2)

    call cpolxy(s, x, y, enx, eny, enz)
    call rowvd('cpolxy', (/ dble(m), dble(x), dble(y) /), 3, &
         (/ dble(enx), dble(eny), dble(enz) /), 3)
  end subroutine one_point

end subroutine emit_xy

! ============================================================================
! coordtrafo. Each scenario sets the com_mod grid (xlon0, ylat0, dx, dy,
! nxmin1, nymin1, xglobal, sglobal, nglobal) and point_mod release boxes
! (degrees), calls upstream, and prints the surviving boxes in grid units.
! zpoint1(i) carries the ORIGINAL index i, which coordtrafo shifts along with
! the box, so the row records which input survived.
!
!   ctgrid      scen; xlon0; ylat0; dx; dy; nxmin1; nymin1; xglobal; sglobal; nglobal
!   ctin        scen; i; xpoint1; ypoint1; xpoint2; ypoint2      (degrees)
!   coordtrafo_n scen -> numpoint after
!   coordtrafo  scen; k -> original index; xpoint1; ypoint1; xpoint2; ypoint2
module cmapf_ref_ct
  implicit none
contains
  subroutine run_scenario(scen, xl0, yl0, ddx, ddy, nx1, ny1, xg, sg, ng, np, &
       x1, y1, x2, y2, docall)
    use par_mod, only: dp
    use com_mod
    use point_mod
    use cmapf_ref_io
    implicit none
    integer, intent(in) :: scen, nx1, ny1, np
    real, intent(in) :: xl0, yl0, ddx, ddy, x1(np), y1(np), x2(np), y2(np)
    logical, intent(in) :: xg, sg, ng, docall
    integer :: i
    real :: lg(3)

    if (allocated(xpoint1)) then
      deallocate(xpoint1, xpoint2, ypoint1, ypoint2, zpoint1, zpoint2, &
           npart, kindz, ireleasestart, ireleaseend, xmass)
    end if
    allocate(xpoint1(np), xpoint2(np), ypoint1(np), ypoint2(np), &
         zpoint1(np), zpoint2(np), npart(np), kindz(np), &
         ireleasestart(np), ireleaseend(np), xmass(np, 1))

    xlon0 = xl0
    ylat0 = yl0
    dx = ddx
    dy = ddy
    nxmin1 = nx1
    nymin1 = ny1
    xglobal = xg
    sglobal = sg
    nglobal = ng
    nspec = 1
    numpoint = np
    lg = 0.
    if (xg) lg(1) = 1.
    if (sg) lg(2) = 1.
    if (ng) lg(3) = 1.
    call rowvd('ctgrid', (/ dble(scen), dble(xl0), dble(yl0), dble(ddx), &
         dble(ddy), dble(nx1), dble(ny1), dble(lg(1)), dble(lg(2)), dble(lg(3)) /), &
         10, (/ 0.d0 /), 0)
    do i = 1, np
      xpoint1(i) = x1(i)
      ypoint1(i) = y1(i)
      xpoint2(i) = x2(i)
      ypoint2(i) = y2(i)
      zpoint1(i) = real(i)
      zpoint2(i) = 100.
      npart(i) = 1000
      kindz(i) = 1
      ireleasestart(i) = 0
      ireleaseend(i) = 3600
      xmass(i, 1) = 1.
      write(compoint(i), '(A,I0)') 'point ', i
      call rowvd('ctin', (/ dble(scen), dble(i), dble(x1(i)), dble(y1(i)), &
           dble(x2(i)), dble(y2(i)) /), 6, (/ 0.d0 /), 0)
    end do

    if (.not. docall) return
    call coordtrafo()

    call rowvd('coordtrafo_n', (/ dble(scen) /), 1, (/ dble(numpoint) /), 1)
    do i = 1, numpoint
      call rowvd('coordtrafo', (/ dble(scen), dble(i) /), 2, &
           (/ dble(zpoint1(i)), dble(xpoint1(i)), dble(ypoint1(i)), &
              dble(xpoint2(i)), dble(ypoint2(i)) /), 5)
    end do
  end subroutine run_scenario
end module cmapf_ref_ct

subroutine emit_coordtrafo()
  use cmapf_ref_ct
  implicit none
  ! Scenario 1: regional domain lon -30..30, lat 20..60 (dx 0.5, dy 0.25).
  ! Inside; west of domain; ON the west edge (x = 0 < 1e-6, removed); east
  ! edge; south edge; north edge for ypoint2 (>= nymin1 - spacing); two
  ! consecutive removals (exercises the goto-15 restart); inside again.
  call run_scenario(1, -30., 20., 0.5, 0.25, 120, 160, .false., .false., .false., 10, &
       (/ 0., -31., -30., 29.9, 5., 5., -40., -50., -10., 29.999 /), &
       (/ 40., 40., 40., 40., 20., 50., 40., 40., 25., 59.99 /), &
       (/ 1., -29., -29., 30., 6., 6., -39., -49., -9., 29.9995 /), &
       (/ 41., 41., 41., 41., 21., 60., 41., 41., 26., 59.9999 /), .true.)

  ! Scenario 2: global 1-degree grid with both poles (xglobal, sglobal,
  ! nglobal). South-pole boxes are clamped to ypoint1 = 1e-6 and kept; a
  ! north-pole ypoint2 is clamped to nymin1 - 10*spacing; but a box whose
  ! ypoint1 is AT the north pole is removed (ypoint1 is never clamped).
  ! x is not checked when xglobal: west of xlon0 and beyond nxmin1 survive.
  call run_scenario(2, -180., -90., 1., 1., 360, 180, .true., .true., .true., 7, &
       (/ 0., -10., 100., -190., 185., 0., 50. /), &
       (/ -90., -95., 89., 10., 10., 90., -89.5 /), &
       (/ 10., 0., 110., -185., 190., 10., 51. /), &
       (/ -89., -80., 90., 20., 20., 90., 89.9 /), .true.)

  ! Scenario 5: the same global grid, boxes whose edge lies within 1e-5 deg
  ! of the north pole, i.e. ON the 1e-6 / 1e-5 / spacing margins of the
  ! domain test. Which side they fall is decided by the rounding of
  ! (lat - ylat0)/dy, so the outcome differs between the two builds; the
  ! real(8) build pins the port's decisions bit for bit.
  call run_scenario(5, -180., -90., 1., 1., 360, 180, .true., .true., .true., 4, &
       (/ 50., 0., 20., 30. /), &
       (/ -89.5, 89.99999, 89.999995, 10. /), &
       (/ 51., 1., 21., 31. /), &
       (/ 89.99999, 90., 90., 89.999995 /), .true.)

  ! Scenario 3: x-global but no poles (lat -80..80): the same pole boxes are
  ! now removed, and a box at lat -79.9999 (y = 1e-4) is kept.
  call run_scenario(3, 0., -80., 1., 1., 360, 160, .true., .false., .false., 6, &
       (/ 0., 10., 20., 30., 400., 5. /), &
       (/ -90., -79.9999, 10., 79.9, 0., 79.99999 /), &
       (/ 10., 11., 21., 31., 401., 6. /), &
       (/ -85., -79., 11., 80., 1., 80. /), .true.)

  ! Scenario 4: tiny nymin1 = 1 with nglobal, where 10*spacing(1.0) < 1e-5 so
  ! the north clamp's result itself still satisfies the clamp condition.
  call run_scenario(4, 0., 0., 1., 1., 10, 1, .false., .false., .true., 3, &
       (/ 1., 2., 3. /), &
       (/ 0.25, 0.5, 0.9999999 /), &
       (/ 2., 3., 4. /), &
       (/ 0.9999995, 1.5, 1. /), .true.)
end subroutine emit_coordtrafo

! Every point out of domain: upstream prints an error and executes `stop`.
! The inputs are printed; the build script checks the stop message and that
! no coordtrafo_n row followed, then records the outcome.
subroutine emit_coordtrafo_stop()
  use cmapf_ref_ct
  implicit none
  call run_scenario(9, -30., 20., 0.5, 0.25, 120, 160, .false., .false., .false., 2, &
       (/ -50., 40. /), (/ 40., 40. /), (/ -49., 41. /), (/ 41., 41. /), .true.)
end subroutine emit_coordtrafo_stop

! ============================================================================
program flexpart_reference_cmapf

  use cmapf_ref_io
  implicit none

  if (command_argument_count() > 0) then
    ! Separate process: coordtrafo's `stop` when every point is removed.
    call emit_coordtrafo_stop()
    stop
  end if

  write(*,'(A)') '# FLEXPART cmapf_mod + coordtrafo reference, generated by dev/flexpart_reference_cmapf.f90'
  write(*,'(A)') '# upstream: github.com/flexpart/flexpart v10.4 @ 3d7eebf (GPL-3.0-or-later)'
  write(*,'(A)') '# DO NOT EDIT BY HAND -- regenerate with dev/build_reference_cmapf.sh'
  write(*,'(A)') 'function,args,outputs'

  call emit_cspanf()
  call emit_eqvlat()
  call emit_stlmbr()
  call build_maps()
  call emit_ll()
  call emit_xy()
  call emit_coordtrafo()

end program flexpart_reference_cmapf
