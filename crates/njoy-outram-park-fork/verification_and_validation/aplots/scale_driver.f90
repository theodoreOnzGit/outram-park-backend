! Differential driver for ascle, ascll and mtname of NJOY2016.80 (acefc.f90, acecm.f90),
! extracted unchanged by extract.sh. Every input and output double is printed as its
! 16 hex digits, so the Rust test compares bit patterns.
program scale_driver
  use scale_mod
  use acecm
  use endf
  implicit none
  integer,parameter::dp=kind(1.d0)
  integer::m,major,minor,i,k,mt,iv,idx
  real(dp)::z1,z2,a1,a2,lo,hi
  integer(8)::seed
  real(dp)::hand(2,40)
  integer::hm(40)
  integer,dimension(6)::izs=(/1,1001,1002,1003,2003,2004/)
  integer,dimension(4)::ivs=(/-1,4,5,6/)
  character(10)::name

  ! --- hand-picked cases: every branch of ascle
  hm=(/4,4,4,0,21,1,4,4,4,4,4,4,4,4,4,4,4,2,1,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4/)
  hand(:,1)=(/0.d0,0.d0/)
  hand(:,2)=(/5.d0,5.d0/)
  hand(:,3)=(/6.d0,5.d0/)
  hand(:,4)=(/0.d0,10.d0/)
  hand(:,5)=(/0.d0,10.d0/)
  hand(:,6)=(/0.d0,1.d0/)
  hand(:,7)=(/0.d0,12.5d0/)
  hand(:,8)=(/1.d-3,2.d1/)
  hand(:,9)=(/1.d0,1.00001d0/)
  hand(:,10)=(/-5.d0,5.d0/)
  hand(:,11)=(/-5.d0,-1.d0/)
  hand(:,12)=(/2.d0,2000.d0/)
  hand(:,13)=(/2.d0,2.5d3/)
  hand(:,14)=(/1.d-9,1.d0/)
  hand(:,15)=(/1.d3,1.0001d3/)
  hand(:,16)=(/2.d0,20.d0/)
  hand(:,17)=(/0.d0,4.d4/)
  hand(:,18)=(/0.d0,2.d9/)
  hand(:,19)=(/1.d-6,1.d-5/)
  hand(:,20)=(/0.d0,0.0001d0/)
  hand(:,21)=(/0.d0,20.d0/)
  hand(:,22)=(/0.d0,2.01d0/)
  hand(:,23)=(/0.d0,1.99d0/)
  hand(:,24)=(/0.d0,4.99d0/)
  hand(:,25)=(/0.d0,5.d0/)
  hand(:,26)=(/0.d0,0.5d0/)
  hand(:,27)=(/0.d0,0.0201d0/)
  hand(:,28)=(/0.d0,0.0499d0/)
  hand(:,29)=(/3.d0,1.d7/)
  hand(:,30)=(/0.99d0,1.01d0/)
  hand(:,31)=(/0.d0,142.8833d0/)
  hand(:,32)=(/0.2d0,20.d0/)
  hand(:,33)=(/-1.d0,0.d0/)
  hand(:,34)=(/0.d0,3.d-12/)
  hand(:,35)=(/1.d-12,1.d-11/)
  hand(:,36)=(/0.d0,6.0221d23/)
  hand(:,37)=(/2.d0,3.d0/)
  hand(:,38)=(/0.2d0,0.2000001d0/)
  hand(:,39)=(/1.5d0,2.5d0/)
  hand(:,40)=(/0.d0,7.d0/)
  do i=1,40
     m=hm(i); z1=hand(1,i); z2=hand(2,i); a1=z1; a2=z2
     call ascle(m,a1,a2,major,minor)
     call pr_ascle(m,z1,z2,a1,a2,major,minor)
  enddo
  ! --- pseudo-random cases (a 64-bit LCG; the Rust test reads the inputs back)
  seed=20261112_8
  do i=1,3000
     m=1+int(next(seed)*6.d0)
     lo=-6.d0+12.d0*next(seed)
     z1=0.d0
     if (next(seed).gt.0.3d0) z1=sign(1.d0,next(seed)-0.8d0)*10.d0**lo
     hi=-6.d0+12.d0*next(seed)
     z2=10.d0**hi
     if (next(seed).gt.0.9d0) z2=z1*(1.d0+10.d0**(-8.d0*next(seed)))
     a1=z1; a2=z2
     call ascle(m,a1,a2,major,minor)
     call pr_ascle(m,z1,z2,a1,a2,major,minor)
  enddo
  ! --- ascll: positive pairs over 24 decades, with every mantissa threshold
  do i=1,3000
     lo=-12.d0+24.d0*next(seed)
     hi=-12.d0+24.d0*next(seed)
     z1=10.d0**lo
     z2=10.d0**hi
     if (mod(i,7).eq.0) z2=z1
     if (mod(i,11).eq.0) z2=z1*1.0000001d0
     if (mod(i,13).eq.0) then
        z1=10.d0**int(lo)*1.d0
        z2=10.d0**int(hi)*2.d0
     endif
     a1=z1; a2=z2
     call ascll(a1,a2)
     write(*,'(a,4(1x,z16.16))') 'ascll',z1,z2,a1,a2
  enddo
  ! --- mtname: every MT 1..1000 with a defined table entry, every izai and iverf
  do iv=1,4
     iverf=ivs(iv)
     do k=1,6
        do mt=1,1000
           ! Upstream indexes hndf(500) out of range for some MT when iverf >= 6 or
           ! -1 (undefined behaviour, no real file has such an MT): skip those.
           if (iverf.ge.6.or.iverf.eq.-1) then
              idx=mt
              if (idx.gt.999) idx=idx-1000*(idx/1000)
              if (idx.ge.600) idx=idx-399
              if (.not.((idx.ge.1.and.idx.le.500).or.(mt.ge.201.and.mt.le.207).or.mt.eq.444)) cycle
           endif
           call mtname(mt,name,izs(k))
           write(*,'(a,3(1x,i0),1x,a,a,a)') 'mtname',iverf,izs(k),mt,'[',name,']'
        enddo
     enddo
  enddo
contains
  real(dp) function next(s)
    integer(8),intent(inout)::s
    s=s*6364136223846793005_8+1442695040888963407_8
    next=real(ibits(s,11,53),dp)/9007199254740992.d0
  end function next
  subroutine pr_ascle(m,z1,z2,a1,a2,major,minor)
    integer::m,major,minor
    real(dp)::z1,z2,a1,a2
    write(*,'(a,1x,i0,4(1x,z16.16),2(1x,i0))') 'ascle',m,z1,z2,a1,a2,major,minor
  end subroutine pr_ascle
end program scale_driver
