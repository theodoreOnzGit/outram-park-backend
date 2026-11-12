! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
! gh:#1368: run the verbatim `acelf6` ismooth extension block (extracted from NJOY2016
! 2016.79 or 2016.80) on LAW=1/LANG=2 spectra, with and without a first outgoing energy
! of zero, and print inputs and outputs as hex bit patterns.
!   I id lep nd na n nx ; D <scr(7..6+nx) hex ...>     (input)
!   R id nx n          ; E <scr(7..6+nx) hex ...>      (output)
! scr(5) and scr(6) are set as in the ENDF record (nx, n); the LEP=2 branch rewrites them.
program drv
   use locale
   use smoothx
   implicit none
   integer,parameter::ncase=12
   real(kr)::scr(400)
   integer::ic,lep,nd,na,n,nx,ncyc,jscr,i
   real(kr)::row(60)
   do ic=1,ncase
      call getcase(ic,lep,nd,na,n,row)
      ncyc=na+2
      nx=n*ncyc
      scr=0
      scr(3)=nd; scr(4)=na; scr(5)=nx; scr(6)=n
      scr(7:6+nx)=row(1:nx)
      write(*,'(a,i3,5i4)') 'I',ic,lep,nd,na,n,nx
      write(*,'(a,60(1x,z16.16))') 'D',(tr(scr(6+i)),i=1,nx)
      jscr=0
      call smooth(scr,nx,n,nd,ncyc,na,1,2,lep,1.0_kr,1,5,1.0e6_kr,jscr)
      write(*,'(a,i3,2i5)') 'R',ic,nx,n
      write(*,'(a,400(1x,z16.16))') 'E',(tr(scr(6+i)),i=1,nx)
   enddo
contains
   subroutine getcase(ic,lep,nd,na,n,row)
      integer,intent(in)::ic
      integer,intent(out)::lep,nd,na,n
      real(kr),intent(out)::row(60)
      row=0; nd=0; na=0
      select case(ic)
      case(1)  ! LEP=1, first E'=0: the extension is active in both versions
         lep=1; n=6
         row(1:12)=(/0.0_kr,2.0e-6_kr, 100.0_kr,3.0e-6_kr, 5.0e3_kr,4.0e-6_kr, 2.0e5_kr,2.0e-6_kr, 1.0e6_kr,1.0e-6_kr, 2.0e7_kr,0.0_kr/)
      case(2)  ! LEP=1, first E'=1 keV > 0: 2016.80 must not extend
         lep=1; n=6
         row(1:12)=(/1.0e3_kr,2.0e-6_kr, 5.0e3_kr,3.0e-6_kr, 2.0e4_kr,4.0e-6_kr, 2.0e5_kr,2.0e-6_kr, 1.0e6_kr,1.0e-6_kr, 2.0e7_kr,0.0_kr/)
      case(3)  ! LEP=1, first E' = 1e-3 eV (eout1 = 1e-9 MeV, not zero)
         lep=1; n=5
         row(1:10)=(/1.0e-3_kr,2.0e-6_kr, 3.0e3_kr,3.0e-6_kr, 3.0e5_kr,4.0e-6_kr, 1.0e6_kr,1.0e-6_kr, 2.0e7_kr,0.0_kr/)
      case(4)  ! LEP=1, first E'=0, constant histogram so the merge loop runs
         lep=1; n=6
         row(1:12)=(/0.0_kr,1.0e-6_kr, 10.0_kr,1.0e-6_kr, 20.0_kr,1.0e-6_kr, 3.0e4_kr,5.0e-6_kr, 1.0e6_kr,1.0e-6_kr, 2.0e7_kr,0.0_kr/)
      case(5)  ! LEP=2, first E'=0
         lep=2; n=5
         row(1:10)=(/0.0_kr,0.0_kr, 100.0_kr,1.0e-6_kr, 1.0e3_kr,2.0e-6_kr, 1.0e5_kr,3.0e-6_kr, 1.0e6_kr,1.0e-6_kr/)
      case(6)  ! LEP=2, first E'=500 eV > ex: 2016.79 inserts a zero row and extends
         lep=2; n=5
         row(1:10)=(/500.0_kr,1.0e-6_kr, 1.0e3_kr,2.0e-6_kr, 1.0e4_kr,3.0e-6_kr, 1.0e5_kr,3.0e-6_kr, 1.0e6_kr,1.0e-6_kr/)
      case(7)  ! LEP=2, first E'=30 eV (< ex) but > 0
         lep=2; n=5
         row(1:10)=(/30.0_kr,1.0e-6_kr, 1.0e3_kr,2.0e-6_kr, 1.0e4_kr,3.0e-6_kr, 1.0e5_kr,3.0e-6_kr, 1.0e6_kr,1.0e-6_kr/)
      case(8)  ! LEP=2, first E'=1e-3 eV
         lep=2; n=5
         row(1:10)=(/1.0e-3_kr,0.0_kr, 1.0e3_kr,2.0e-6_kr, 1.0e4_kr,3.0e-6_kr, 1.0e5_kr,3.0e-6_kr, 1.0e6_kr,1.0e-6_kr/)
      case(9)  ! LEP=1, first E'=0, one angular parameter (NA=1): ncyc=3
         lep=1; na=1; n=5
         row(1:15)=(/0.0_kr,2.0e-6_kr,0.1_kr, 100.0_kr,3.0e-6_kr,0.2_kr, 5.0e3_kr,4.0e-6_kr,0.3_kr, &
                     1.0e6_kr,1.0e-6_kr,0.4_kr, 2.0e7_kr,0.0_kr,0.5_kr/)
      case(10) ! LEP=2, NA=1, first E'=700 eV
         lep=2; na=1; n=5
         row(1:15)=(/700.0_kr,2.0e-6_kr,0.1_kr, 3.0e3_kr,3.0e-6_kr,0.2_kr, 5.0e4_kr,4.0e-6_kr,0.3_kr, &
                     1.0e6_kr,1.0e-6_kr,0.4_kr, 2.0e7_kr,0.0_kr,0.5_kr/)
      case(11) ! discrete lines present (ND=1): neither version acts
         lep=1; nd=1; n=5
         row(1:10)=(/0.0_kr,2.0e-6_kr, 3.0e3_kr,3.0e-6_kr, 3.0e5_kr,4.0e-6_kr, 1.0e6_kr,1.0e-6_kr, 2.0e7_kr,0.0_kr/)
      case(12) ! LEP=1, first E'=0 but second point already below ex: nothing to insert
         lep=1; n=5
         row(1:10)=(/0.0_kr,2.0e-6_kr, 30.0_kr,3.0e-6_kr, 3.0e5_kr,4.0e-6_kr, 1.0e6_kr,1.0e-6_kr, 2.0e7_kr,0.0_kr/)
      end select
   end subroutine getcase
   function tr(x) result(i)
      real(kr),intent(in)::x
      integer(8)::i
      i=transfer(x,i)
   end function tr
end program drv
