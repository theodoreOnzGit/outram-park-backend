! Differential driver for gh:#1490: NJOY2016's own `coul` and `ptlegc`
! (acefc.f90) on the LTP = 2 branch (residual cross-section expansion,
! acefc.f90:8198-8213). No held evaluation has an LTP = 2 record, so the nine
! LTP = 1 records of lists.txt are reused with c(3) set to 2: a code-path test
! of the arithmetic, not physics. Output as decimal and IEEE-754 bits, in the
! format of driver.f90 with the tags COUL2 and PTLEGC2.
program drv2
   use locale
   use endf
   use acefc
   implicit none
   character(32)::label
   real(kr)::awp,awt,spi,y
   integer::izai,iza,nw,i,ios,k,ncase
   real(kr)::c(2000),c0(2000)
   real(kr)::xs(5)
   integer(8)::ib

   xs=(/-0.96_kr,-0.5_kr,0.0_kr,0.5_kr,0.96_kr/)
   ncase=0
   c0=0
   open(10,file='lists.txt',status='old')
   do
      read(10,*,iostat=ios) label,awp,izai,awt,iza,spi,nw
      if (ios.ne.0) exit
      read(10,*) (c0(i),i=1,nw)
      c0(3)=2
      ncase=ncase+1
      do k=1,5
         c=0
         c(1:nw)=c0(1:nw)
         call coul(xs(k),y,c,awp,izai,awt,iza,spi)
         ib=transfer(y,ib)
         write(*,'(a,a,1x,i3,1x,es25.17,1x,z16.16)') 'COUL2 ',trim(label),k,y,ib
      enddo
      c=0
      c(1:nw)=c0(1:nw)
      call ptlegc(c,awp,izai,awt,iza,spi)
      write(*,'(a,a,1x,i3,1x,i6)') 'PTLEGC2 ',trim(label),ncase,nint(c(6))
      do i=1,nint(c(6))
         ib=transfer(c(5+2*i),ib)
         write(*,'(a,z16.16,1x,es25.17)',advance='no') ' ',ib,c(5+2*i)
         ib=transfer(c(6+2*i),ib)
         write(*,'(a,z16.16,1x,es25.17)') ' ',ib,c(6+2*i)
      enddo
   enddo
end program drv2
