# Source this to get the OpenFOAM-dev (852527377c7e, dummy Pstream, no MPI) environment.
export FOAM_INST_DIR=/home/user/upstream-build
set +u
. /home/user/upstream-build/OpenFOAM-dev/etc/bashrc WM_MPLIB=DUMMY WM_COMPILE_OPTION=Opt WM_LABEL_SIZE=32 WM_PRECISION_OPTION=DP WM_NCOMPPROCS=2
export WM_NCOMPPROCS=2
export OMP_NUM_THREADS=1
