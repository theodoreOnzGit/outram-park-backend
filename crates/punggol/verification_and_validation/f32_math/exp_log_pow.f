      program m
      implicit none
      real*4 x, y, a, b, c, d, e
      integer i, j
      integer*4 ia, ib, ic, id, ie
      equivalence (a, ia), (b, ib), (c, ic), (d, id), (e, ie)
      real*4 s
      s = 1.0
      do i = 1, 4000
        x = 1.0e-3 * (1.0 + 0.00731 * real(i)) ** 2
        y = -0.3333333
        a = x
        b = exp(x - 5.0)
        c = log(x)
        d = x ** y
        e = log10(x)
        write(6,'(Z8.8,1X,Z8.8,1X,Z8.8,1X,Z8.8,1X,Z8.8)') ia, ib, ic, id, ie
      end do
      end
