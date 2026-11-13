// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Minimal stand-in for EGSnrc's GENERATED `egs_config1.h` (the template is
// HEN_HOUSE/pieces/egs_config1.h; EGSnrc's configure script fills it in).
// Only the typedefs `egs_alias_table.cpp` needs, with the template's
// double-precision values: EGS_Float = double, EGS_I64 = long long.
#ifndef EGS_CONFIG1_
#define EGS_CONFIG1_
typedef double EGS_Float;
typedef short EGS_I16;
typedef int EGS_I32;
typedef long long EGS_I64;
#endif
