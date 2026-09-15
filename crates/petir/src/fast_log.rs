// Ported from ARM optimized-routines `math/log.c` and `math/log_data.c`,
// commit f2e4faf58c6c671154f472a76eaaa977bb36c870 (2026-09-09), read
// 2026-09-14.
//
// Copyright (c) 2018-2025, Arm Limited.                      (upstream)
// SPDX-License-Identifier: MIT OR Apache-2.0 WITH LLVM-exception
// Copyright (C) 2026 Theodore Ong and the outram-park contributors (this port)
//
// ARM optimized-routines is MIT OR Apache-2.0 WITH LLVM-exception -- both
// permissive into GPL-3.0. This derivative work is GPL-3.0-only.

//! A **fast**, deterministic natural logarithm — ported from ARM's
//! optimized-routines, the implementation glibc itself ships.
//!
//! The companion to [`crate::fast_exp`]; read that module's docs for why this
//! exists, what the FMA-contraction caveat is, and why the port deliberately
//! tracks upstream's *portable* (non-FMA) form rather than glibc's contracted
//! one.
//!
//! **`ln` is the weakest of the three on speed, and that is worth stating up
//! front.** Measured from Rust over 2 000 000 calls
//! (`tests/fast_math_speed.rs`, five runs), this buys **1.09-1.18x** over the
//! musl-derived `libm` route, against ~1.8x for `exp` and ~2.1x for `powf`.
//! Two structural reasons: the `libm` crate's `log` is already close to
//! optimal, and `HAVE_FAST_FMA == 0` costs this function more than it costs
//! `exp` — it forces a second table (`tab2`, doubling the cache footprint) and
//! a longer near-1 branch. Adopt it for the **determinism**, which is
//! unconditional; the speed is a modest bonus here.
//!
//! It is nonetheless [`crate::real::ln`]'s default route, for consistency: one
//! deterministic implementation across all three functions is worth more than
//! a per-function speed optimum.
//!
//! # Fidelity
//!
//! `log` (`math/log.c:31-155`) transcribed, with the preprocessor variants
//! resolved as the baseline x86-64 build resolves them — these were read off
//! the *compiled* object rather than chosen by eye:
//!
//! ```text
//!   LOG_TABLE_BITS   7      (N = 128)
//!   LOG_POLY_ORDER   6
//!   LOG_POLY1_ORDER  12
//!   HAVE_FAST_FMA    0
//! ```
//!
//! `HAVE_FAST_FMA == 0` is the load-bearing one: it selects the `tab2`
//! (`chi`/`clo`) argument reduction `r = (z - chi - clo) * invc` over
//! `fma(z, invc, -1.0)`, and with `LOG_POLY1_ORDER == 12` and `N > 64` it
//! selects the near-1 branch's `rhi`/`rlo` splitting form. Upstream defines it
//! as `FP_FAST_FMA || __aarch64__`, and GCC does not define `FP_FAST_FMA` for
//! generic x86-64 — so this is the branch a portable build takes, and the one
//! a Rust transcription can reproduce (Rust never contracts, and `mul_add`
//! lowers to a libcall without hardware FMA).
//!
//! The 256 `tab` entries, 256 `tab2` entries, and 16 polynomial coefficients
//! were **not** parsed out of `log_data.c` — that file selects between table
//! sizes with preprocessor conditionals, and resolving those by hand is how
//! transcription errors get in. The compiled object was linked into a dumper
//! that printed the resolved `__log_data` fields as bit patterns, and the Rust
//! tables were generated from its output.

use crate::error::{PetirError, Result};

/// `LOG_TABLE_BITS`.
const TABLE_BITS: u32 = 7;
/// `N = 1 << LOG_TABLE_BITS`.
const N: u64 = 1 << TABLE_BITS;
/// `OFF` (`log.c:22`) — the interval split point, `0x3fe6000000000000`.
const OFF: u64 = 0x3fe6_0000_0000_0000;

/// `asuint64(1.0 - 0x1p-4)` / `asuint64(1.0 + 0x1.09p-4)` (`log.c:45-46`), the
/// near-1 window selected by `LOG_POLY1_ORDER == 12`. Rust has no hex-float
/// literals, so `0x1.09p-4 = (1 + 0x09/0x100) * 2^-4` is written out.
const LO: u64 = (1.0f64 - 0.0625).to_bits();
const HI: u64 = (1.0f64 + 0.064_697_265_625).to_bits();

/// `__log_data.ln2hi` / `ln2lo` — `ln 2` split for accuracy.
const LN2_HI: f64 = f64::from_bits(0x3fe62e42fefa3800);
const LN2_LO: f64 = f64::from_bits(0x3d2ef35793c76730);

/// `__log_data.poly[0..5]` — the main-path coefficients (`LOG_POLY_ORDER == 6`).
const POLY: [f64; 5] = [
    f64::from_bits(0xbfe0000000000001),
    f64::from_bits(0x3fd555555551305b),
    f64::from_bits(0xbfcfffffffeb4590),
    f64::from_bits(0x3fc999b324f10111),
    f64::from_bits(0xbfc55575e506c89f),
];

/// `__log_data.poly1[0..11]` — the near-1 coefficients (`LOG_POLY1_ORDER == 12`).
/// `POLY1[0] == -0.5` exactly, which the algorithm relies on.
const POLY1: [f64; 11] = [
    f64::from_bits(0xbfe0000000000000),
    f64::from_bits(0x3fd5555555555577),
    f64::from_bits(0xbfcffffffffffdcb),
    f64::from_bits(0x3fc999999995dd0c),
    f64::from_bits(0xbfc55555556745a7),
    f64::from_bits(0x3fc24924a344de30),
    f64::from_bits(0xbfbfffffa4423d65),
    f64::from_bits(0x3fbc7184282ad6ca),
    f64::from_bits(0xbfb999eb43b068ff),
    f64::from_bits(0x3fb78182f7afd085),
    f64::from_bits(0xbfb5521375d145cd),
];

/// `__log_data.tab` — 2*N entries, `(invc_bits, logc_bits)` per subinterval.
const TAB: [u64; 256] = [
    0x3ff734f0c3e0de9f,
    0xbfd7cc7f79e69000,
    0x3ff713786a2ce91f,
    0xbfd76feec20d0000,
    0x3ff6f26008fab5a0,
    0xbfd713e31351e000,
    0x3ff6d1a61f138c7d,
    0xbfd6b85b38287800,
    0x3ff6b1490bc5b4d1,
    0xbfd65d5590807800,
    0x3ff69147332f0cba,
    0xbfd602d076180000,
    0x3ff6719f18224223,
    0xbfd5a8ca86909000,
    0x3ff6524f99a51ed9,
    0xbfd54f4356035000,
    0x3ff63356aa8f24c4,
    0xbfd4f637c36b4000,
    0x3ff614b36b9ddc14,
    0xbfd49da7fda85000,
    0x3ff5f66452c65c4c,
    0xbfd445923989a800,
    0x3ff5d867b5912c4f,
    0xbfd3edf439b0b800,
    0x3ff5babccb5b90de,
    0xbfd396ce448f7000,
    0x3ff59d61f2d91a78,
    0xbfd3401e17bda000,
    0x3ff5805612465687,
    0xbfd2e9e2ef468000,
    0x3ff56397cee76bd3,
    0xbfd2941b3830e000,
    0x3ff54725e2a77f93,
    0xbfd23ec58cda8800,
    0x3ff52aff42064583,
    0xbfd1e9e129279000,
    0x3ff50f22dbb2bddf,
    0xbfd1956d2b48f800,
    0x3ff4f38f4734ded7,
    0xbfd141679ab9f800,
    0x3ff4d843cfde2840,
    0xbfd0edd094ef9800,
    0x3ff4bd3ec078a3c8,
    0xbfd09aa518db1000,
    0x3ff4a27fc3e0258a,
    0xbfd047e65263b800,
    0x3ff4880524d48434,
    0xbfcfeb224586f000,
    0x3ff46dce1b192d0b,
    0xbfcf474a7517b000,
    0x3ff453d9d3391854,
    0xbfcea4443d103000,
    0x3ff43a2744b4845a,
    0xbfce020d44e9b000,
    0x3ff420b54115f8fb,
    0xbfcd60a22977f000,
    0x3ff40782da3ef4b1,
    0xbfccc00104959000,
    0x3ff3ee8f5d57fe8f,
    0xbfcc202956891000,
    0x3ff3d5d9a00b4ce9,
    0xbfcb81178d811000,
    0x3ff3bd60c010c12b,
    0xbfcae2c9ccd3d000,
    0x3ff3a5242b75dab8,
    0xbfca45402e129000,
    0x3ff38d22cd9fd002,
    0xbfc9a877681df000,
    0x3ff3755bc5847a1c,
    0xbfc90c6d69483000,
    0x3ff35dce49ad36e2,
    0xbfc87120a645c000,
    0x3ff34679984dd440,
    0xbfc7d68fb4143000,
    0x3ff32f5cceffcb24,
    0xbfc73cb83c627000,
    0x3ff3187775a10d49,
    0xbfc6a39a9b376000,
    0x3ff301c8373e3990,
    0xbfc60b3154b7a000,
    0x3ff2eb4ebb95f841,
    0xbfc5737d76243000,
    0x3ff2d50a0219a9d1,
    0xbfc4dc7b8fc23000,
    0x3ff2bef9a8b7fd2a,
    0xbfc4462c51d20000,
    0x3ff2a91c7a0c1bab,
    0xbfc3b08abc830000,
    0x3ff293726014b530,
    0xbfc31b996b490000,
    0x3ff27dfa5757a1f5,
    0xbfc2875490a44000,
    0x3ff268b39b1d3bbf,
    0xbfc1f3b9f879a000,
    0x3ff2539d838ff5bd,
    0xbfc160c8252ca000,
    0x3ff23eb7aac9083b,
    0xbfc0ce7f57f72000,
    0x3ff22a012ba940b6,
    0xbfc03cdc49fea000,
    0x3ff2157996cc4132,
    0xbfbf57bdbc4b8000,
    0x3ff201201dd2fc9b,
    0xbfbe370896404000,
    0x3ff1ecf4494d480b,
    0xbfbd17983ef94000,
    0x3ff1d8f5528f6569,
    0xbfbbf9674ed8a000,
    0x3ff1c52311577e7c,
    0xbfbadc79202f6000,
    0x3ff1b17c74cb26e9,
    0xbfb9c0c3e7288000,
    0x3ff19e010c2c1ab6,
    0xbfb8a646b372c000,
    0x3ff18ab07bb670bd,
    0xbfb78d01b3ac0000,
    0x3ff1778a25efbcb6,
    0xbfb674f145380000,
    0x3ff1648d354c31da,
    0xbfb55e0e6d878000,
    0x3ff151b990275fdd,
    0xbfb4485cdea1e000,
    0x3ff13f0ea432d24c,
    0xbfb333d94d6aa000,
    0x3ff12c8b7210f9da,
    0xbfb22079f8c56000,
    0x3ff11a3028ecb531,
    0xbfb10e4698622000,
    0x3ff107fbda8434af,
    0xbfaffa6c6ad20000,
    0x3ff0f5ee0f4e6bb3,
    0xbfadda8d4a774000,
    0x3ff0e4065d2a9fce,
    0xbfabbcece4850000,
    0x3ff0d244632ca521,
    0xbfa9a1894012c000,
    0x3ff0c0a77ce2981a,
    0xbfa788583302c000,
    0x3ff0af2f83c636d1,
    0xbfa5715e67d68000,
    0x3ff09ddb98a01339,
    0xbfa35c8a49658000,
    0x3ff08cabaf52e7df,
    0xbfa149e364154000,
    0x3ff07b9f2f4e28fb,
    0xbf9e72c082eb8000,
    0x3ff06ab58c358f19,
    0xbf9a55f152528000,
    0x3ff059eea5ecf92c,
    0xbf963d62cf818000,
    0x3ff04949cdd12c90,
    0xbf9228fb8caa0000,
    0x3ff038c6c6f0ada9,
    0xbf8c317b20f90000,
    0x3ff02865137932a9,
    0xbf8419355daa0000,
    0x3ff0182427ea7348,
    0xbf781203c2ec0000,
    0x3ff008040614b195,
    0xbf60040979240000,
    0x3fefe01ff726fa1a,
    0x3f6feff384900000,
    0x3fefa11cc261ea74,
    0x3f87dc41353d0000,
    0x3fef6310b081992e,
    0x3f93cea3c4c28000,
    0x3fef25f63ceeadcd,
    0x3f9b9fc114890000,
    0x3feee9c8039113e7,
    0x3fa1b0d8ce110000,
    0x3feeae8078cbb1ab,
    0x3fa58a5bd001c000,
    0x3fee741aa29d0c9b,
    0x3fa95c8340d88000,
    0x3fee3a91830a99b5,
    0x3fad276aef578000,
    0x3fee01e009609a56,
    0x3fb07598e598c000,
    0x3fedca01e577bb98,
    0x3fb253f5e30d2000,
    0x3fed92f20b7c9103,
    0x3fb42edd8b380000,
    0x3fed5cac66fb5cce,
    0x3fb606598757c000,
    0x3fed272caa5ede9d,
    0x3fb7da76356a0000,
    0x3fecf26e3e6b2ccd,
    0x3fb9ab434e1c6000,
    0x3fecbe6da2a77902,
    0x3fbb78c7bb0d6000,
    0x3fec8b266d37086d,
    0x3fbd431332e72000,
    0x3fec5894bd5d5804,
    0x3fbf0a3171de6000,
    0x3fec26b533bb9f8c,
    0x3fc067152b914000,
    0x3febf583eeece73f,
    0x3fc147858292b000,
    0x3febc4fd75db96c1,
    0x3fc2266ecdca3000,
    0x3feb951e0c864a28,
    0x3fc303d7a6c55000,
    0x3feb65e2c5ef3e2c,
    0x3fc3dfc33c331000,
    0x3feb374867c9888b,
    0x3fc4ba366b7a8000,
    0x3feb094b211d304a,
    0x3fc5933928d1f000,
    0x3feadbe885f2ef7e,
    0x3fc66acd2418f000,
    0x3feaaf1d31603da2,
    0x3fc740f8ec669000,
    0x3fea82e63fd358a7,
    0x3fc815c0f51af000,
    0x3fea5740ef09738b,
    0x3fc8e92954f68000,
    0x3fea2c2a90ab4b27,
    0x3fc9bb3602f84000,
    0x3fea01a01393f2d1,
    0x3fca8bed1c2c0000,
    0x3fe9d79f24db3c1b,
    0x3fcb5b515c01d000,
    0x3fe9ae2505c7b190,
    0x3fcc2967ccbcc000,
    0x3fe9852ef297ce2f,
    0x3fccf635d5486000,
    0x3fe95cbaeea44b75,
    0x3fcdc1bd3446c000,
    0x3fe934c69de74838,
    0x3fce8c01b8cfe000,
    0x3fe90d4f2f6752e6,
    0x3fcf5509c0179000,
    0x3fe8e6528effd79d,
    0x3fd00e6c121fb800,
    0x3fe8bfce9fcc007c,
    0x3fd071b80e93d000,
    0x3fe899c0dabec30e,
    0x3fd0d46b9e867000,
    0x3fe87427aa2317fb,
    0x3fd13687334bd000,
    0x3fe84f00acb39a08,
    0x3fd1980d67234800,
    0x3fe82a49e8653e55,
    0x3fd1f8ffe0cc8000,
    0x3fe8060195f40260,
    0x3fd2595fd7636800,
    0x3fe7e22563e0a329,
    0x3fd2b9300914a800,
    0x3fe7beb377dcb5ad,
    0x3fd3187210436000,
    0x3fe79baa679725c2,
    0x3fd377266dec1800,
    0x3fe77907f2170657,
    0x3fd3d54ffbaf3000,
    0x3fe756cadbd6130c,
    0x3fd432eee32fe000,
];

/// `__log_data.tab2` — 2*N entries, `(chi_bits, clo_bits)` per subinterval.
/// Present only when `HAVE_FAST_FMA == 0`, which is the branch this port takes.
const TAB2: [u64; 256] = [
    0x3fe61000014fb66b,
    0x3c7e026c91425b3c,
    0x3fe63000034db495,
    0x3c8dbfea48005d41,
    0x3fe650000d94d478,
    0x3c8e7fa786d6a5b7,
    0x3fe67000074e6fad,
    0x3c61fcea6b54254c,
    0x3fe68ffffedf0fae,
    0xbc7c7e274c590efd,
    0x3fe6b0000763c5bc,
    0xbc8ac16848dcda01,
    0x3fe6d0001e5cc1f6,
    0x3c833f1c9d499311,
    0x3fe6efffeb05f63e,
    0xbc7e80041ae22d53,
    0x3fe710000e869780,
    0x3c7bff6671097952,
    0x3fe72ffffc67e912,
    0x3c8c00e226bd8724,
    0x3fe74fffdf81116a,
    0xbc6e02916ef101d2,
    0x3fe770000f679c90,
    0xbc67fc71cd549c74,
    0x3fe78ffffa7ec835,
    0x3c81bec19ef50483,
    0x3fe7affffe20c2e6,
    0xbc707e1729cc6465,
    0x3fe7cfffed3fc900,
    0xbc808072087b8b1c,
    0x3fe7efffe9261a76,
    0x3c8dc0286d9df9ae,
    0x3fe81000049ca3e8,
    0x3c897fd251e54c33,
    0x3fe8300017932c8f,
    0xbc8afee9b630f381,
    0x3fe850000633739c,
    0x3c89bfbf6b6535bc,
    0x3fe87000204289c6,
    0xbc8bbf65f3117b75,
    0x3fe88fffebf57904,
    0xbc89006ea23dcb57,
    0x3fe8b00022bc04df,
    0xbc7d00df38e04b0a,
    0x3fe8cfffe50c1b8a,
    0xbc88007146ff9f05,
    0x3fe8effffc918e43,
    0x3c83817bd07a7038,
    0x3fe910001efa5fc7,
    0x3c893e9176dfb403,
    0x3fe9300013467bb9,
    0x3c7f804e4b980276,
    0x3fe94fffe6ee076f,
    0xbc8f7ef0d9ff622e,
    0x3fe96fffde3c12d1,
    0xbc7082aa962638ba,
    0x3fe98ffff4458a0d,
    0xbc87801b9164a8ef,
    0x3fe9afffdd982e3e,
    0xbc8740e08a5a9337,
    0x3fe9cfffed49fb66,
    0x3c3fce08c19be000,
    0x3fe9f00020f19c51,
    0xbc8a3faa27885b0a,
    0x3fea10001145b006,
    0x3c74ff489958da56,
    0x3fea300007bbf6fa,
    0x3c8cbeab8a2b6d18,
    0x3fea500010971d79,
    0x3c88fecadd787930,
    0x3fea70001df52e48,
    0xbc8f41763dd8abdb,
    0x3fea90001c593352,
    0xbc8ebf0284c27612,
    0x3feab0002a4f3e4b,
    0xbc69fd043cff3f5f,
    0x3feacfffd7ae1ed1,
    0xbc823ee7129070b4,
    0x3feaefffee510478,
    0x3c6a063ee00edea3,
    0x3feb0fffdb650d5b,
    0x3c5a06c8381f0ab9,
    0x3feb2ffffeaaca57,
    0xbc79011e74233c1d,
    0x3feb4fffd995badc,
    0xbc79ff1068862a9f,
    0x3feb7000249e659c,
    0x3c8aff45d0864f3e,
    0x3feb8ffff9871640,
    0x3c7cfe7796c2c3f9,
    0x3febafffd204cb4f,
    0xbc63ff27eef22bc4,
    0x3febcfffd2415c45,
    0xbc6cffb7ee3bea21,
    0x3febeffff86309df,
    0xbc814103972e0b5c,
    0x3fec0fffe1b57653,
    0x3c8bc16494b76a19,
    0x3fec2ffff1fa57e3,
    0xbc64feef8d30c6ed,
    0x3fec4fffdcbfe424,
    0xbc843f68bcec4775,
    0x3fec6fffed54b9f7,
    0x3c847ea3f053e0ec,
    0x3fec8fffeb998fd5,
    0x3c7383068df992f1,
    0x3fecb0002125219a,
    0xbc68fd8e64180e04,
    0x3feccfffdd94469c,
    0x3c8e7ebe1cc7ea72,
    0x3fecefffeafdc476,
    0x3c8ebe39ad9f88fe,
    0x3fed1000169af82b,
    0x3c757d91a8b95a71,
    0x3fed30000d0ff71d,
    0x3c89c1906970c7da,
    0x3fed4fffea790fc4,
    0xbc580e37c558fe0c,
    0x3fed70002edc87e5,
    0xbc7f80d64dc10f44,
    0x3fed900021dc82aa,
    0xbc747c8f94fd5c5c,
    0x3fedafffd86b0283,
    0x3c8c7f1dc521617e,
    0x3fedd000296c4739,
    0x3c88019eb2ffb153,
    0x3fedefffe54490f5,
    0x3c6e00d2c652cc89,
    0x3fee0fffcdabf694,
    0xbc7f8340202d69d2,
    0x3fee2fffdb52c8dd,
    0x3c7b00c1ca1b0864,
    0x3fee4ffff24216ef,
    0x3c72ffa8b094ab51,
    0x3fee6fffe88a5e11,
    0xbc57f673b1efbe59,
    0x3fee9000119eff0d,
    0xbc84808d5e0bc801,
    0x3feeafffdfa51744,
    0x3c780006d54320b5,
    0x3feed0001a127fa1,
    0xbc5002f860565c92,
    0x3feef00007babcc4,
    0xbc8540445d35e611,
    0x3fef0ffff57a8d02,
    0xbc4ffb3139ef9105,
    0x3fef30001ee58ac7,
    0x3c8a81acf2731155,
    0x3fef4ffff5823494,
    0x3c8a3f41d4d7c743,
    0x3fef6ffffca94c6b,
    0xbc6202f41c987875,
    0x3fef8fffe1f9c441,
    0x3c777dd1f477e74b,
    0x3fefafffd2e0e37e,
    0xbc6f01199a7ca331,
    0x3fefd0001c77e49e,
    0x3c7181ee4bceacb1,
    0x3fefeffff7e0c331,
    0xbc6e05370170875a,
    0x3ff00ffff465606e,
    0xbc8a7ead491c0ada,
    0x3ff02ffff3867a58,
    0xbc977f69c3fcb2e0,
    0x3ff04ffffdfc0d17,
    0x3c97bffe34cb945b,
    0x3ff0700003cd4d82,
    0x3c820083c0e456cb,
    0x3ff08ffff9f2cbe8,
    0xbc6dffdfbe37751a,
    0x3ff0b000010cda65,
    0xbc913f7faee626eb,
    0x3ff0d00001a4d338,
    0x3c807dfa79489ff7,
    0x3ff0effffadafdfd,
    0xbc77040570d66bc0,
    0x3ff110000bbafd96,
    0x3c8e80d4846d0b62,
    0x3ff12ffffae5f45d,
    0x3c9dbffa64fd36ef,
    0x3ff150000dd59ad9,
    0x3c9a0077701250ae,
    0x3ff170000f21559a,
    0x3c8dfdf9e2e3deee,
    0x3ff18ffffc275426,
    0x3c910030dc3b7273,
    0x3ff1b000123d3c59,
    0x3c997f7980030188,
    0x3ff1cffff8299eb7,
    0xbc65f932ab9f8c67,
    0x3ff1effff48ad400,
    0x3c937fbf9da75beb,
    0x3ff210000c8b86a4,
    0x3c9f806b91fd5b22,
    0x3ff2300003854303,
    0x3c93ffc2eb9fbf33,
    0x3ff24fffffbcf684,
    0x3c7601e77e2e2e72,
    0x3ff26ffff52921d9,
    0x3c7ffcbb767f0c61,
    0x3ff2900014933a3c,
    0xbc7202ca3c02412b,
    0x3ff2b00014556313,
    0xbc92808233f21f02,
    0x3ff2cfffebfe523b,
    0xbc88ff7e384fdcf2,
    0x3ff2f0000bb8ad96,
    0xbc85ff51503041c5,
    0x3ff30ffffb7ae2af,
    0xbc810071885e289d,
    0x3ff32ffffeac5f7f,
    0xbc91ff5d3fb7b715,
    0x3ff350000ca66756,
    0x3c957f82228b82bd,
    0x3ff3700011fbf721,
    0x3c8000bac40dd5cc,
    0x3ff38ffff9592fb9,
    0xbc943f9d2db2a751,
    0x3ff3b00004ddd242,
    0x3c857f6b707638e1,
    0x3ff3cffff5b2c957,
    0x3c7a023a10bf1231,
    0x3ff3efffeab0b418,
    0x3c987f6d66b152b0,
    0x3ff410001532aff4,
    0x3c67f8375f198524,
    0x3ff4300017478b29,
    0x3c8301e672dc5143,
    0x3ff44fffe795b463,
    0x3c89ff69b8b2895a,
    0x3ff46fffe80475e0,
    0xbc95c0b19bc2f254,
    0x3ff48fffef6fc1e7,
    0x3c9b4009f23a2a72,
    0x3ff4afffe5bea704,
    0xbc94ffb7bf0d7d45,
    0x3ff4d000171027de,
    0xbc99c06471dc6a3d,
    0x3ff4f0000ff03ee2,
    0x3c977f890b85531c,
    0x3ff5100012dc4bd1,
    0x3c6004657166a436,
    0x3ff530001605277a,
    0xbc96bfcece233209,
    0x3ff54fffecdb704c,
    0xbc8902720505a1d7,
    0x3ff56fffef5f54a9,
    0x3c9bbfe60ec96412,
    0x3ff5900017e61012,
    0x3c887ec581afef90,
    0x3ff5b00003c93e92,
    0xbc9f41080abf0cc0,
    0x3ff5d0001d4919bc,
    0xbc98812afb254729,
    0x3ff5efffe7b87a89,
    0xbc947eb780ed6904,
];

/// `top16(x)` (`log.c:25-28`) — the top 16 bits of the double's encoding.
#[inline]
fn top16(x: f64) -> u32 {
    (x.to_bits() >> 48) as u32
}

/// The transcription itself — `log` (`math/log.c:31`).
///
/// Returns `Err((error, ieee_value))` for the two special cases, carrying both
/// PETIR's error and the value upstream's C returns, so that [`ln`] and
/// [`ln_ieee`] are two views of one implementation rather than two
/// implementations.
///
/// # Verification
///
/// Checked against upstream's own compiled C over 16 751 probes crossing every
/// branch — every binade from the subnormal floor upward at six mantissas
/// each, the near-1 `poly1` window and both sides of its edges, and the
/// subnormal renormalisation path: **16 748 compared, 16 748 bit-identical
/// (100.000 %)**, 3 refused at `±0` and `-1.0` with upstream returning `-inf`
/// / NaN (`tests/fast_log_vs_arm_optimized_routines.rs`, 2026-09-14).
fn ln_inner(x: f64) -> core::result::Result<f64, (PetirError, f64)> {
    let mut ix = x.to_bits();
    let top = top16(x);

    // Inputs close to 1.0 get their own polynomial: the main path's
    // `k*Ln2 + log(c) + r` decomposition cancels catastrophically there.
    if ix.wrapping_sub(LO) < HI.wrapping_sub(LO) {
        // Fix the sign of zero under downward rounding when x == 1.
        if ix == 1.0f64.to_bits() {
            return Ok(0.0);
        }
        let r = x - 1.0;
        let r2 = r * r;
        let r3 = r * r2;
        // LOG_POLY1_ORDER == 12 (`log.c:73-77`).
        let mut y = r3
            * (POLY1[1]
                + r * POLY1[2]
                + r2 * POLY1[3]
                + r3 * (POLY1[4]
                    + r * POLY1[5]
                    + r2 * POLY1[6]
                    + r3 * (POLY1[7] + r * POLY1[8] + r2 * POLY1[9] + r3 * POLY1[10])));
        // N > 64, so the `# else` arm (`log.c:82-91`): split r so that
        // `rhi * rhi` is exact, which costs ~0.025 ulp of worst-case error.
        let w = r * f64::from_bits(0x41A0000000000000); // 0x1p27
        let rhi = r + w - w;
        let rlo = r - rhi;
        let w = rhi * rhi * POLY1[0]; // POLY1[0] == -0.5.
        let hi = r + w;
        let mut lo = r - hi + w;
        lo += POLY1[0] * rlo * (rhi + r);
        y += lo;
        y += hi;
        return Ok(y);
    }

    if top.wrapping_sub(0x0010) >= 0x7ff0u32.wrapping_sub(0x0010) {
        // x < 0x1p-1022, or inf, or nan.
        if ix.wrapping_mul(2) == 0 {
            // `__math_divzero(1)` returns -inf and raises divide-by-zero.
            return Err((PetirError::ZeroDivide, f64::NEG_INFINITY));
        }
        if ix == f64::INFINITY.to_bits() {
            return Ok(x); // log(inf) == inf
        }
        if top & 0x8000 != 0 || top & 0x7ff0 == 0x7ff0 {
            // `__math_invalid(x)` (`math_err.c`) returns `(x - x) / (x - x)`
            // and raises invalid-operand. That expression is `0.0 / 0.0`,
            // whose NaN *payload and sign are architecture-dependent*: x86-64
            // SSE produces the "indefinite" QNaN with the sign bit set
            // (0xfff8…), aarch64 the positive default QNaN (0x7ff8…).
            //
            // This port returns the positive quiet NaN on every target
            // instead. That is a deliberate divergence in payload only: the
            // value is a NaN either way, NaN payloads carry no numerical
            // meaning, and a platform-dependent one would defeat exactly the
            // determinism this module exists for. The V&V test therefore
            // checks NaN-ness here rather than the bit pattern, and says so.
            return Err((PetirError::Domain, f64::NAN));
        }
        // x is subnormal: normalise it.
        ix = (x * f64::from_bits(0x4330000000000000)).to_bits(); // 0x1p52
        ix -= 52u64 << 52;
    }

    // x = 2^k z, with z in [OFF, 2*OFF) and exact. The range is split into N
    // subintervals; the i-th contains z, and c is near its centre.
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> (52 - TABLE_BITS)) % N) as usize;
    let k = (tmp as i64) >> 52; // arithmetic shift
    let iz = ix.wrapping_sub(tmp & (0xfffu64 << 52));
    // `i = (... ) % N` and `TAB`/`TAB2` each hold `2 * N` entries, so every
    // read below is in range -- by arithmetic the compiler cannot follow.
    // `get` keeps the values, and therefore the bit-identity with upstream
    // pinned by tests/fast_log_vs_arm_optimized_routines.rs, exactly as they
    // were, while removing the panic the subscripts carried.
    let (Some(&tab_invc), Some(&tab_logc)) = (TAB.get(2 * i), TAB.get(2 * i + 1)) else {
        return Err((PetirError::Range, f64::NAN));
    };
    let invc = f64::from_bits(tab_invc);
    let logc = f64::from_bits(tab_logc);
    let z = f64::from_bits(iz);

    // log(x) = log1p(z/c - 1) + log(c) + k*Ln2, with |r| < 1/(2N).
    // HAVE_FAST_FMA == 0, so the tab2 form (`log.c:126`); rounding error
    // 0x1p-55/N + 0x1p-66 against 0x1p-55/N for the fma form.
    let (Some(&tab2_chi), Some(&tab2_clo)) = (TAB2.get(2 * i), TAB2.get(2 * i + 1)) else {
        return Err((PetirError::Range, f64::NAN));
    };
    let chi = f64::from_bits(tab2_chi);
    let clo = f64::from_bits(tab2_clo);
    let r = (z - chi - clo) * invc;
    let kd = k as f64;

    // hi + lo = r + log(c) + k*Ln2.
    let w = kd * LN2_HI + logc;
    let hi = w + r;
    let lo = w - hi + r + kd * LN2_LO;

    // log(x) = lo + (log1p(r) - r) + hi. LOG_POLY_ORDER == 6 (`log.c:146`).
    let r2 = r * r;
    Ok(lo + r2 * POLY[0] + r * r2 * (POLY[1] + r * POLY[2] + r2 * (POLY[3] + r * POLY[4])) + hi)
}

/// Fast `ln x`, refusing the inputs that have no finite real logarithm.
///
/// This is [`ln_ieee`] with upstream's two special returns turned into errors,
/// which is PETIR's convention elsewhere.
///
/// # Errors
///
/// [`PetirError::ZeroDivide`] at `x == ±0`, where upstream returns
/// `__math_divzero(1)` (`-inf`, with a divide-by-zero flag), and
/// [`PetirError::Domain`] for `x < 0` or NaN, where it returns
/// `__math_invalid(x)` (a NaN, with the invalid flag). `ln(+inf) == +inf` is
/// returned as `Ok`, as upstream returns it.
#[inline]
pub fn ln(x: f64) -> Result<f64> {
    ln_inner(x).map_err(|(e, _)| e)
}

/// Fast `ln x`, returning exactly what upstream's C returns — `-inf` at zero
/// and a NaN for a negative argument — rather than an error.
///
/// The faithful surface, and the one to prefer; see [`crate::fast_exp::exp_ieee`]
/// for why both exist.
#[inline]
pub fn ln_ieee(x: f64) -> f64 {
    ln_inner(x).unwrap_or_else(|(_, v)| v)
}
