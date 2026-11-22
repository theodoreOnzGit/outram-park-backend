! Copyright (C) 2026 Theodore Ong and the outram-park contributors. AGPL-3.0-only.
! gh:#1366: NJOY2016 mathm `e1` (SLATEC de1) and `gami` (SLATEC dgami) as hex bits.
! Links NJOY2016's own mathm.f90 UNMODIFIED (the two routines are public there).
! Grid: 770 log-spaced points u = 1e-6 * 10**(k/100), k=0..769 (to 49.6), plus the branch
! boundaries 1, 4 and their neighbours; a = 0.5, 1.5, 2.5 for gami.
program drv
   use locale
   use mathm, only: e1,gami
   implicit none
   integer::k,j
   real(kr)::u
   real(kr),parameter::extra(9)=(/0.999999_kr,1.0_kr,1.000001_kr,3.999999_kr,4.0_kr,4.000001_kr,&
        0.5_kr,50.0_kr,25.0_kr/)
   real(kr),parameter::av(3)=(/0.5_kr,1.5_kr,2.5_kr/)
   do k=0,769
      u=1.0e-6_kr*10.0_kr**(real(k,kr)/100.0_kr)
      call emit(u)
   enddo
   do k=1,size(extra)
      call emit(extra(k))
   enddo
contains
   subroutine emit(x)
      real(kr),intent(in)::x
      write(*,'(a,1x,z16.16,1x,z16.16,3(1x,z16.16))') 'E',tr(x),tr(e1(x)),&
         tr(gami(av(1),x)),tr(gami(av(2),x)),tr(gami(av(3),x))
   end subroutine emit
   function tr(x) result(i)
      real(kr),intent(in)::x
      integer(8)::i
      i=transfer(x,i)
   end function tr
end program drv
