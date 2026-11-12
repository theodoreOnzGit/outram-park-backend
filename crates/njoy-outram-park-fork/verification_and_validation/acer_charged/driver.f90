! Differential driver for gh:#1485: NJOY2016's own `coul`, `ptlegc` (acefc.f90)
! and `terp1` INT=6 (endf.f90), called on fixed inputs and printed as decimal
! and as IEEE-754 bit patterns, so a Rust test can compare bit for bit.
! Built by build_and_run.sh against an unmodified NJOY2016 source tree except
! that acefc's `public` line also exports ptlegc and coul.
program drv
   use locale
   use endf
   use acefc
   implicit none
   character(32)::label
   real(kr)::awp,awt,spi,y,x
   integer::izai,iza,nw,i,ios,k,ncase
   real(kr)::c(2000),c0(2000)
   real(kr)::xs(5),x1,y1,x2,y2
   integer(8)::ib

   ! terp1, INT = 6: the Coulomb penetrability law, threshold thr6 = 0
   ! (the value endf's module variable holds in ACER: nothing sets it there).
   thr6=0
   x1=1.0e3_kr; y1=1.0e-3_kr; x2=1.0e5_kr; y2=5.0_kr
   do i=1,5
      select case (i)
      case (1); x=2.0e3_kr
      case (2); x=1.0e4_kr
      case (3); x=5.0e4_kr
      case (4); x=1.0e3_kr
      case (5); x=1.0e5_kr
      end select
      call terp1(x1,y1,x2,y2,x,y,6)
      ib=transfer(y,ib)
      write(*,'(a,i2,1x,es25.17,1x,z16.16)') 'TERP6 ',i,y,ib
   enddo
   ! y1 = 0 returns y1; x2 = x1 returns y1
   call terp1(x1,0.0_kr,x2,y2,2.0e3_kr,y,6)
   ib=transfer(y,ib)
   write(*,'(a,i2,1x,es25.17,1x,z16.16)') 'TERP6 ',6,y,ib

   xs=(/-0.96_kr,-0.5_kr,0.0_kr,0.5_kr,0.96_kr/)
   ncase=0
   open(10,file='lists.txt',status='old')
   do
      read(10,*,iostat=ios) label,awp,izai,awt,iza,spi,nw
      if (ios.ne.0) exit
      read(10,*) (c0(i),i=1,nw)
      ncase=ncase+1
      ! coul at five cosines
      do k=1,5
         c(1:nw)=c0(1:nw)
         call coul(xs(k),y,c,awp,izai,awt,iza,spi)
         ib=transfer(y,ib)
         write(*,'(a,a,1x,i3,1x,es25.17,1x,z16.16)') 'COUL ',trim(label),k,y,ib
      enddo
      ! ptlegc reconstruction
      c(1:nw)=c0(1:nw)
      call ptlegc(c,awp,izai,awt,iza,spi)
      write(*,'(a,a,1x,i3,1x,i6)') 'PTLEGC ',trim(label),ncase,nint(c(6))
      do i=1,nint(c(6))
         ib=transfer(c(5+2*i),ib)
         write(*,'(a,z16.16,1x,es25.17)',advance='no') ' ',ib,c(5+2*i)
         ib=transfer(c(6+2*i),ib)
         write(*,'(a,z16.16,1x,es25.17)') ' ',ib,c(6+2*i)
      enddo
   enddo
end program drv
