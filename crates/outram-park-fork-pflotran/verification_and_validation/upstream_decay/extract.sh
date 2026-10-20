#!/usr/bin/env bash
# Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.
#
# Extract the decay solver of upstream PFLOTRAN's UFD_DECAY process model and
# the dense LU it calls, VERBATIM by line range, with only the mechanical
# substitutions listed below (gh:#1000). Re-run to regenerate *_extract.F90.
#
# Upstream: bitbucket.org/pflotran/pflotran, commit
#   2af572b8759c34aacc2216f56a906637f90af03e (2026-10-20), LGPL-3.0
#   (used under GPL-3.0, as this crate's NOTICE records for its combined work).
# Clone: see ../../upstream_source/README.md; check out the commit above.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src=${1:-$here/../../upstream_source/PFLOTRAN/src/pflotran}

# Fail loudly if the files are not the ones the line numbers refer to.
echo "5f3539e07ccb4c163787894b1ce3d4669b2fdb4bdf481753585357938518b1c0  $src/pm_ufd_decay.F90" | sha256sum -c -
echo "16c67eba5eed4395c6c516f8b5a18051c240af1cf53b8f8e63d7d71b8d5feea4  $src/utility.F90" | sha256sum -c -

# pm_ufd_decay.F90 lines 1868-1994: PMUFDDecaySolveISPDIAtCell from
# `if (.not.this%implicit_solution) then` to `max(mass_iso_tot1,1.d-90)`.
# Substitution: `this%` is deleted (the derived-type components become
# dummy arguments of the same name in driver.F90).
{
  echo "! EXTRACTED by extract.sh from PFLOTRAN 2af572b8 pm_ufd_decay.F90"
  echo "! lines 1868-1994 (PMUFDDecaySolveISPDIAtCell). Only edit: 'this%' deleted."
  sed -n '1868,1994p' "$src/pm_ufd_decay.F90" | sed 's/this%//g'
} > "$here/ufd_body_extract.F90"

# utility.F90 lines 542-699: LUDecomposition2, LUDecomposition1,
# LUBackSubstitution. Substitutions: PETSc kind macros -> intrinsic types;
# the three MPI calls on the stop_on_error path are deleted (the `print` and
# `stop` that follow them are kept, so a singular row still stops the run).
{
  echo "! EXTRACTED by extract.sh from PFLOTRAN 2af572b8 utility.F90"
  echo "! lines 542-699 (LUDecomposition2/1, LUBackSubstitution). Edits: PETSc"
  echo "! kinds -> intrinsic types; MPI_Comm_rank/MPI_Abort/MPI_Finalize deleted."
  sed -n '542,699p' "$src/utility.F90" |
    sed -e 's/PetscReal/real(8)/g' -e 's/PetscMPIInt/integer/g' \
      -e 's/PetscErrorCode/integer/g' -e 's/PetscInt/integer/g' \
      -e 's/PetscBool/logical/g' -e 's/PETSC_TRUE/.true./g' \
      -e '/MPI_Comm_rank\|MPI_Abort\|MPI_Finalize/d'
} > "$here/lu_extract.F90"
echo "extracted: ufd_body_extract.F90 lu_extract.F90"
