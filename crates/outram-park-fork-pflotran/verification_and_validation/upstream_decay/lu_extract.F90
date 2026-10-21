! EXTRACTED by extract.sh from PFLOTRAN 2af572b8 utility.F90
! lines 542-699 (LUDecomposition2/1, LUBackSubstitution). Edits: PETSc
! kinds -> intrinsic types; MPI_Comm_rank/MPI_Abort/MPI_Finalize deleted.
subroutine LUDecomposition2(A,N,INDX,D)

  ! wrapper for more verbose LUDecomposition2

  implicit none

  integer :: N
  real(8) :: A(N,N)
  integer :: INDX(N)
  integer :: D

  integer :: idum

  call LUDecomposition(A,N,INDX,D,.true.,idum)

end subroutine LUDecomposition2

! ************************************************************************** !

subroutine LUDecomposition1(A,N,INDX,D,stop_on_error,ierror)
  !
  ! Given an NxN matrix A, with physical dimension NP, this routine replaces it
  ! by the LU decomposition of a rowwise permutation of itself.
  ! A and N are input. A is output; INDX is output vector which records the
  ! row permutation effected by the partial pivoting; D id output as +1 or -1
  ! depending on whether the number of row interchanges was odd or even,
  ! respectively. This routine is used in combination with LUBackSubstitution
  ! to solve linear equations or invert a matrix.
  !

  implicit none

  integer :: N
  real(8), parameter :: tiny=1.0d-20
  real(8) :: A(N,N),VV(N)
  integer :: INDX(N)
  integer :: D
  logical :: stop_on_error
  integer :: ierror

  integer :: i, j, k, imax
  real(8) :: aamax, sum, dum
  integer ::  rank
  integer :: ierr

  D=1
  do i=1,N
    aamax=0.d0
    do j=1,N
      if (abs(A(i,j)).gt.aamax) aamax=abs(A(i,j))
    enddo
    if (aamax <= 0.d0) then
      if (stop_on_error) then
        print *, "ERROR: Singular value encountered in LUDecomposition() on &
                 &processor: ", rank, ' aamax = ',aamax,' row = ',i
        do k = 1, N
          print *, "Jacobian: ",k,(j,A(k,j),j=1,N)
        enddo
        stop
      else
        ierror = 1
        return
      endif
    endif
    VV(i)=1./aamax
  enddo
  do j=1,N
    do i=1,j-1
      sum=A(i,j)
      do k=1,i-1
        sum=sum-A(i,k)*A(k,j)
      enddo
      A(i,j)=sum
    enddo
    aamax=0
    do i=j,N
      sum=A(i,j)
      do k=1,j-1
        sum=sum-A(i,k)*A(k,j)
      enddo
      A(i,j)=sum
      dum=VV(i)*abs(sum)
      if (dum.ge.aamax) then
        imax=i
        aamax=dum
      endif
    enddo
    if (j.ne.imax) then
      do k=1,N
        dum=A(imax,k)
        A(imax,k)=A(j,k)
        A(j,k)=dum
      enddo
      D=-D
      VV(imax)=VV(j)
    endif
    INDX(j)=imax
    if (A(j,j).eq.0.d0) A(j,j)=tiny
    if (j.ne.N) then
      dum=1.d0/A(j,j)
      do i=j+1,N
        A(i,j)=A(i,j)*dum
      enddo
    endif
  enddo
  return

end subroutine LUDecomposition1

! ************************************************************************** !

subroutine LUBackSubstitution(A,N,INDX,B)
  !
  ! Solves the set of N linear equations A.X=B. Here A is input, not as a matrix
  ! A but rather as its LU decomposition. INDX is the input as the permutation
  ! vector returned by LUDecomposition. B is input as the right-hand side
  ! vector B, and returns with the solution vector X.
  !

  implicit none

  integer :: N
  real(8) :: A(N,N),B(N)
  integer :: INDX(N)

  integer :: i, j, ii, ll
  real(8) :: sum


  ii=0
  do i=1,N
    ll=INDX(i)
    sum=B(ll)
    B(ll)=B(i)
    if (ii.ne.0) then
      do j=ii,i-1
        sum=sum-A(i,j)*B(j)
      enddo
    else if (sum.ne.0.d0) then
      ii=i
    endif
    B(i)=sum
  enddo
  do i=N,1,-1
    sum=B(i)
    if (i.lt.N) then
      do j=i+1,N
        sum=sum-A(i,j)*B(j)
      enddo
    endif
    B(i)=sum/A(i,i)
  enddo
  return

end subroutine LUBackSubstitution
