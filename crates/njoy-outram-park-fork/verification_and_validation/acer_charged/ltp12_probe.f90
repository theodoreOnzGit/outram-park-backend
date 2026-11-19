! gh:#1490: why a driver that fed an LTP >= 12 (tabulated) record to `coul`
! crashed. `coul` holds `real(kr)::p(65)` and calls `legndr(x,p,np)` with
! np = 2*nint(c(6)); for LTP = 12, c(6) is NL, the number of tabulated cosines.
! Reads lists_ltp12.txt (extract_lists.py --ltp12), prints each record's NL and
! the highest p index legndr would write (np+1), then calls `coul` at the
! driver's five cosines. `legndr` takes `p(*)` (assumed size, never bounds
! checked), so for np+1 > 65 it writes past the end of `coul`'s local `p` into
! the locals stored after it (with gfortran 13.3, `np` itself): the loop bound
! is then a double's upper half, and the following `p(ip+1)` reads run off the
! stack (a bounds error under -fcheck=bounds, a segfault without it).
! `acecpe` never does this: it calls ptlegc only `if (ltp.lt.12)`
! (acefc.f90:6552).
program probe
   use locale
   use acefc
   implicit none
   character(32)::label
   real(kr)::awp,awt,spi,y
   integer::izai,iza,nw,i,ios,k
   real(kr)::c(2000),c0(2000)
   real(kr)::xs(5)

   xs=(/0.0_kr,-0.96_kr,-0.5_kr,0.5_kr,0.96_kr/)

   open(10,file='lists_ltp12.txt',status='old')
   do
      read(10,*,iostat=ios) label,awp,izai,awt,iza,spi,nw
      if (ios.ne.0) exit
      c0=0
      read(10,*) (c0(i),i=1,nw)
      write(*,'(a,a,a,es12.5,a,i4,a,i4)') 'PROBE ',trim(label),' E=',c0(2),&
        ' NL=',nint(c0(6)),' highest p index=',2*nint(c0(6))+1
      do k=1,5
         c=c0
         write(*,'(a,f6.2)',advance='no') '  x=',xs(k)
         flush(6)
         call coul(xs(k),y,c,awp,izai,awt,iza,spi)
         write(*,'(a,es25.17)') ' returned ',y
         flush(6)
      enddo
   enddo
end program probe
