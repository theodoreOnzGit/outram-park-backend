// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Harness stand-in for EGSnrc's configure-generated `egs_config1.h`
// (template: HEN_HOUSE/pieces/egs_config1.h). Only the type definitions
// egs_rndm needs, with the template's default (non-SINGLE) choice:
// EGS_Float is double, EGS_I32 int, EGS_I64 long long.
#ifndef EGS_CONFIG1_
#define EGS_CONFIG1_
#define CONFIG_NAME "outram-park-egs-rndm-harness"
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
