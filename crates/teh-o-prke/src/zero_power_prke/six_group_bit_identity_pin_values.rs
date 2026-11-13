// Recorded 2026-11-01 from the pre-gh:#1082 SixGroupPRKE (commit c9a3afcaf)
// by `print_six_group_fingerprints`. (FNV-1a hash of every returned value,
// final neutron density bits), for U-233, U-235, Pu-239.
//
// The explicit (RKF45, single fixed step) script diverges -- its final
// densities are ~1e88 and, for Pu-239, large and negative -- because the
// script deliberately includes steps far beyond the explicit stability limit
// (dt >> Lambda). That is the stiff behaviour the implicit solver exists to
// avoid; it is pinned anyway, because the pin is about arithmetic identity.
const PIN_IMPLICIT: [(u64, u64); 3] = [
    (0xf8f1_ad9a_0d8d_9c71, 0x4024_2fb1_709e_059d),
    (0x3df8_e3d2_b841_a895, 0x4012_07d8_d4aa_66eb),
    (0x0b23_418f_afb7_68da, 0x4045_9c07_4dd5_1215),
];
const PIN_EXPLICIT: [(u64, u64); 3] = [
    (0xb9af_af96_9e97_0b56, 0x5268_03ed_b0d0_af4a),
    (0x1fba_49f7_afc0_6a3b, 0x5584_ee95_4888_300f),
    (0xbc1d_4a3a_5c8f_b053, 0xd20a_c352_fa2d_0b96),
];
