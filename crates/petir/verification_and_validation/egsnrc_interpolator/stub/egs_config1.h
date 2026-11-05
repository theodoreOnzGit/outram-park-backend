// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Minimal stand-in for EGSnrc's HEN_HOUSE/pieces/egs_config1.h, which is a
// template (it holds `__f77_function1__` placeholders that EGSnrc's configure
// step fills in) and so cannot be compiled as it stands. Only the typedefs
// egs_interpolator.{h,cpp} reach, with the values the template selects for a
// default build (no SINGLE, no WIN32). Written for this harness, not copied.
#ifndef EGS_CONFIG1_
#define EGS_CONFIG1_
#ifdef __cplusplus
#define __extc__ "C"
#else
#define __extc__
#endif
typedef double EGS_Float;
typedef short EGS_I16;
typedef int EGS_I32;
typedef long long EGS_I64;
#endif
