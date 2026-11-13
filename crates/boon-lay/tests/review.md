# Review record: crates/boon-lay/tests

```toml
[kovan]
id = "review-record-crates-boon-lay-tests"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: atops2026paper_against_the_papers_printed_releases (github:theodoreOnzGit)

```toml
[kovan]
id = "review-atops2026paper-against-the-papers-printed-releases-theodoreonzgit-9511bb6f"
kind = "review"
origin = "human"
created = "2026-11-03T16:49:28+08:00"
modified = "2026-11-03T16:49:28+08:00"
target = "fn:9511bb6f3a667d6e"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::atops2026paper_against_the_papers_printed_releases"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-03"
signed_at = "2026-11-03T16:49:29+08:00"
commit = "8f5f24a863b8ac7f74c53307aa48fa351484a45b"
hash = "sha256:0d9dafd6dd06d36a7b6612a14099543bd73b3d7b22d2e4177ffdc5144915f20a"
doc_hash = "sha256:02c4afe6bbdca37b16088976ccb16a16c7ab17acc4990f7a706e2fe9aa4ca24b"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:0a906b3f44704108" = "sha256:0477dcbfb31598d520ea9a9d75e03cdea025f049280a530db210ff1ea2e067e0"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:9ebac061e78bab77" = "sha256:6c3ed178717c77dab42ad2ed0faf5260d63634da983d3a19fc2353ad13b495e7"
"fn:b754ff5f79ad8f15" = "sha256:00edbc37903be951dbce5ac1e770eee50e2db7c005ea359b3a612e7b3cc237f4"
"fn:b872687bd30a967d" = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
"fn:cdad051795ac6be8" = "sha256:a9dec292637eb605ac407d9868d6cad55812ee390eb9f93ac35c12775f64271e"
"fn:e95381509718b938" = "sha256:bd2d0163823daca920081b40fa42e0e840f5ff475e31dca41e6b9e9e87f11010"
"fn:fb5529d08b5edfef" = "sha256:b8d4303f659750e972945ee910c028255321b71167786ea163a0198687360e69"
"fn:fe6fb33576d54327" = "sha256:46f138d56ce352b44596af49695c291d91336b112f064f5295c2d69556342815"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "4RBuqInqRcEiBOh2jwKz9WD7UJd8xXVfjiGEkclfuswBjYy7puQP4ZurxZPUzsuuNUNfG7h8bI/kqvOtL2MVCA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

This reproduces the test (and prints outputs) for the TRISO atops, reproducing 
paper results, it was found that it uses upstream TRISO ATOPS settings for decay 
rather than paper settings

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-03, commit 8f5f24a863.

# Review: score_case (github:theodoreOnzGit)

```toml
[kovan]
id = "review-score-case-theodoreonzgit-cdad0517"
kind = "review"
origin = "human"
created = "2026-11-04T10:41:23+08:00"
modified = "2026-11-04T10:41:23+08:00"
target = "fn:cdad051795ac6be8"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::score_case"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T10:41:23+08:00"
commit = "5e8606b71d8d774dec42bdfd16678c818d99e8d5"
hash = "sha256:a9dec292637eb605ac407d9868d6cad55812ee390eb9f93ac35c12775f64271e"
doc_hash = "sha256:5e580611dd19e71b56a8fd381e80d5377d0034441d779a8c1ad0a8fc806d1882"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:03f4100244ab1727" = "sha256:bea6414e8e99f94ebe6ab5c8bcd76ca692357209df5b465e98a9117c39e35b38"
"fn:1baee06ed91589b2" = "sha256:906c8fb45b9ce4759492ee1f3e113bcbf209fa53a912c0003642e686e8cfe2af"
"fn:518b88272fe2b144" = "sha256:cc5accd453a9fd750d6ebafbfc7459377ed9f51c9e3aaaa52f8bacc316f4db76"
"fn:51a1aaede9ebe26d" = "sha256:027488dd920548778e2241f4438bd0a61357d1e34b6d3e36c4ffd4e760191e6d"
"fn:9ebac061e78bab77" = "sha256:6c3ed178717c77dab42ad2ed0faf5260d63634da983d3a19fc2353ad13b495e7"
"fn:a8aed3662b6bb636" = "sha256:fe2fe2db2a651a96dcf41c4f61774a4498edba20a57549e54f4364ef72deddfe"
"fn:a9d6067f3b73c4cd" = "sha256:82cbbe2f03087f1695dd434babf4270b47fa9f192d2fa9b1265be0f0c784312d"
"fn:b872687bd30a967d" = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
"fn:bb7fcdf324b9356c" = "sha256:c390e6f85216eeb903eb29aaec79d521c2299aed1985dd1dafab5a5dedb7ecf2"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "vxxkfQ27vvJz+fr6Krwfof8fdf8FTTHRH6VRYqKqBzOhkrH1htAgFFPbrK+piLM3Q+ktuACeeJ9xmVQHdFBRDw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-particle-failure"
kind = "implements"
```

this is the driver file

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 5e8606b71d.

# Review: configurations (github:theodoreOnzGit)

```toml
[kovan]
id = "review-configurations-theodoreonzgit-0a906b3f"
kind = "review"
origin = "human"
created = "2026-11-04T10:50:33+08:00"
modified = "2026-11-04T10:50:33+08:00"
target = "fn:0a906b3f44704108"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::configurations"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T10:50:33+08:00"
commit = "a461c5d9deb7f2a9694e2b011ae3b2d766ab284d"
hash = "sha256:0477dcbfb31598d520ea9a9d75e03cdea025f049280a530db210ff1ea2e067e0"
doc_hash = "sha256:38ac9afbc266def61574a7dd8bb076fdfd0592c862f5ef1286c0c70679dd8706"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:04cbe7e951a3ef50" = "sha256:345ee09cfb571ad3eb7be4d63de0422ed36a5c74f207444c5530a4bd9e9ffb03"
"fn:05c53aa09e3542da" = "sha256:f6c29a369d635851bb790d1b7336f39216c5421501357505c0bc5161a12597eb"
"fn:24e4f2d2ea7f17f4" = "sha256:050f20680cf2ebc0fb4f0a7e8b3d49a74fc1fe359546321ecb13713346a7c00f"
"fn:3e752b3c15c4e4fe" = "sha256:d1f72246cf1efe75bc53507767c1fc7af7359636aa5b009d6fba94e241e5bf7a"
"fn:459395474c5875ce" = "sha256:97f8cd3ee0d1655194b4537bb7ef9d16c1f40e279be6fe6bf1b38d1b7b492abd"
"fn:51ce89820d2bb919" = "sha256:da5ea635a65fccdf1beba3add5e838fbf7b584f10204b36ac5169f80ef6c5d01"
"fn:720763535406a854" = "sha256:468363707eb6f8af68dd8e3d9901d7a76231693bf0a5b4e7e8bc5ad5ee6d5cc1"
"fn:7c47d7977a8d550b" = "sha256:e880f6106e2dfa3cf2d358f74dfd74e3fb91ec284083dadbcbab5330fd266bf1"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"
"fn:b31dc309195e5890" = "sha256:cb0506150f5656d4007ad3a0423685c53bc604ebe4185be3d7d919078226cab2"
"fn:b4300d995422f7cf" = "sha256:1ec2c617f1f0e953148c45600858c2e267e666ce1b816c307cb2cd67d91a5f3c"
"fn:b6670ce65d1534e4" = "sha256:508f9ad6180dc30fa3ca4821e7a704a5852bef8691ecee82828ec72da3d394fa"
"fn:b754ff5f79ad8f15" = "sha256:00edbc37903be951dbce5ac1e770eee50e2db7c005ea359b3a612e7b3cc237f4"
"fn:b872687bd30a967d" = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
"fn:d1e19251526ae55a" = "sha256:8b4d5df4512632190138615178de5e3e88d956fa7dedb0f19759c5041ae29adf"
"fn:e95381509718b938" = "sha256:bd2d0163823daca920081b40fa42e0e840f5ff475e31dca41e6b9e9e87f11010"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Jj6v2WY4iHVKqbdaI382TCtMeYFTupBM8p60c+XDBUG3ErwxF/2K9HsjHrok0B+yglk8uESFoxC/GM7fUomfAw=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel/triso-fission-product-release"
kind = "implements"
```

shows the configuration needed to run the case, 
whether to use paper or upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit a461c5d9de.

# Review: Setup (github:theodoreOnzGit)

```toml
[kovan]
id = "review-setup-theodoreonzgit-b872687b"
kind = "review"
origin = "human"
created = "2026-11-04T10:56:21+08:00"
modified = "2026-11-04T10:56:21+08:00"
target = "fn:b872687bd30a967d"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::Setup"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T10:56:21+08:00"
commit = "18111f33f14a7d49ed639c86e1188c694cd98d4c"
hash = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
doc_hash = "sha256:f5ec0b4732a50579359855fe23370d826281a6a3e2b50ae7bbf611758fb1f964"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"

[review.callees]
"fn:720763535406a854" = "sha256:468363707eb6f8af68dd8e3d9901d7a76231693bf0a5b4e7e8bc5ad5ee6d5cc1"
"fn:8a156c01e50ca527" = "sha256:d2f7f9d7c9313ca05aaba643ddb549000f656a5513204fe9d3821fed7183842b"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_holds_extra = "no"
item_impossible_values = "no_checked_when_built"
item_matches_description = "yes"
item_test_use = "is_test_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "ifGEtaq51b04+6s1f66M8sElcwGWJeXwuNkpW8YIoAibdTLW5MHzklXwYQQaY0vDt6aXqCyZ1HyROdDkwRD2CA=="

[[relation]]
target = "concept:02-nuclear-safety/fuel-system-design/triso-coated-particle-fuel"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 18111f33f1.

# Review: port_case_nodal_with (github:theodoreOnzGit)

```toml
[kovan]
id = "review-port-case-nodal-with-theodoreonzgit-bb7fcdf3"
kind = "review"
origin = "human"
created = "2026-11-04T11:08:22+08:00"
modified = "2026-11-04T11:08:22+08:00"
target = "fn:bb7fcdf324b9356c"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::port_case_nodal_with"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:08:22+08:00"
commit = "3d9371c0b57750f811ca5263eaace7ff1aabba63"
hash = "sha256:c390e6f85216eeb903eb29aaec79d521c2299aed1985dd1dafab5a5dedb7ecf2"
doc_hash = "sha256:c5c133236683cb5716ba66f30500586a82bcc9ddd34673363696d769cc82a210"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.callees]
"fn:03f4100244ab1727" = "sha256:bea6414e8e99f94ebe6ab5c8bcd76ca692357209df5b465e98a9117c39e35b38"
"fn:1baee06ed91589b2" = "sha256:906c8fb45b9ce4759492ee1f3e113bcbf209fa53a912c0003642e686e8cfe2af"
"fn:24b9b78394837b0c" = "sha256:c5e3fcdec29fdbc79637fd69a373b67e0c4ba62e4ac8aace394259ea282aacba"
"fn:2816dd8674f2b148" = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
"fn:2db20ac5ca82f2df" = "sha256:88f57a620fcad0a9ad2c3a1dd178c5e0c248335e82ec9259d7aae14506c5ada5"
"fn:34ff47a5448224c9" = "sha256:1565117e190704e250537a267fcc591cbd6f4f74e8c74c62d6c58955f5437378"
"fn:383734307cced06a" = "sha256:6e869b80d4bc8cf329bab6bad4e2ba9be8ef6411c675c75640bc46a6a70168fb"
"fn:3dab844bc23ea2bd" = "sha256:773d24db91ae5910200fd9060f3bedee67bce16d86c437bf02caea9ddccd8f9d"
"fn:44fd31aa536c0e7c" = "sha256:7104d2332e8fdac8384e558146e0fd17ddc6de5ff99c23c6ad619fc88331a963"
"fn:49cbfd358a6976d4" = "sha256:21bde6366bd14b2230818107f9f51697dcb40359f8c429329b0bea39dd191993"
"fn:4c0db62d31d996d2" = "sha256:1974b3b6d9593fc3081a314edfdf47d0314b8114563edac6d64dba38b1987cbb"
"fn:515e6933ab0603f9" = "sha256:e6d597d01f0fbf4906e858eed1b26c1868f061c52188509463e05e83cc78a311"
"fn:5a2cfb2f89e6446a" = "sha256:6f018e714c7040808b9b260d600f70aa1c59b7aa1fb8ebc33668a99f025e9654"
"fn:5a33f79b407cf341" = "sha256:4c2afe9cb508ba2c2933c0c9cd1e4cf01c88e57848b554e759d1854331c9e2a8"
"fn:745bd77be64f687a" = "sha256:443b83fe6b2801369330c415e78fe99fc27db37a4f85e98465880c10a1eec8f2"
"fn:79f7794a2eef219f" = "sha256:0701774b3dafa0c97c7952cad8b6d2857e2c582ee55cf3a1e9c6d8c77e83ab23"
"fn:8f6c972c90fdceff" = "sha256:9db560f8b8d5220a25ff12870e0702d97acedb8e3ad099a687d937d4bd35d990"
"fn:9a8a6835861d321b" = "sha256:35aa9d1c98f83def7d97ea998bcab736badd6b6b855a3d5be8e59ec40ed3de07"
"fn:9b0d7758d9e3ac90" = "sha256:59a5e8ce9a05349920ac13580febb3d2df80e6fac745f9b0053099aed23b0db3"
"fn:b872687bd30a967d" = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
"fn:dcb6acb5e841b5b0" = "sha256:24b2f6ca6faddbc1768e5afef5d98c2498f83bd3ad7a163cbe0edc7e13e24187"
"fn:e503a3d0722ff508" = "sha256:2064b2b67750abeeaa8a6697c2016cdb79cb79e9d76cf1bcf4d974387dfe881c"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW",
    "https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "iYujG9qf/XTi8kBSWqmYTwIWPu3J0fOJkqgY66Y8UzYvyd+XY2Hf/fxlrto0Bu/N1MjDBWHO35Gt5KbgpoiSAg=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 3d9371c0b5.

# Review: Nodal (github:theodoreOnzGit)

```toml
[kovan]
id = "review-nodal-theodoreonzgit-4c0db62d"
kind = "review"
origin = "human"
created = "2026-11-04T11:09:06+08:00"
modified = "2026-11-04T11:09:06+08:00"
target = "fn:4c0db62d31d996d2"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::Nodal"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:09:06+08:00"
commit = "11d1bb5478486cca893a1404999b5e17fd7025b6"
hash = "sha256:1974b3b6d9593fc3081a314edfdf47d0314b8114563edac6d64dba38b1987cbb"
doc_hash = "sha256:69b89cca399f885b06fdadedcd2ab7b74c535107ce093a3ad4110f0900da190b"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.callees]
"fn:dcb6acb5e841b5b0" = "sha256:24b2f6ca6faddbc1768e5afef5d98c2498f83bd3ad7a163cbe0edc7e13e24187"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "is_test_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "YGWLpu8XUr4kDWEPWuVjxKRuTeHXNgXDDIAp08b7xUa7+hvs5/Pe0CWuMldlm59fuzlu3YML21iQcmAOmbEqBw=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 11d1bb5478.

# Review: N_AXIAL (github:theodoreOnzGit)

```toml
[kovan]
id = "review-n-axial-theodoreonzgit-79f7794a"
kind = "review"
origin = "human"
created = "2026-11-04T11:13:19+08:00"
modified = "2026-11-04T11:13:19+08:00"
target = "fn:79f7794a2eef219f"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::N_AXIAL"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:13:19+08:00"
commit = "928aef156d4b270a6f31b7b5405514c510f8c643"
hash = "sha256:0701774b3dafa0c97c7952cad8b6d2857e2c582ee55cf3a1e9c6d8c77e83ab23"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "is_test_code"
item_value_source = "cited"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "NKSE3nSlyy3BchjYF/NfCRVVDAHtxVwXM2wBmrw2FkkLaSiC5jfZ3kWgvmPNUG4eCnMwyR362h3yez4J8gRuAg=="
```

checked against table 7 in TRISO ATOPS paper

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 928aef156d.

# Review: N_RADIAL (github:theodoreOnzGit)

```toml
[kovan]
id = "review-n-radial-theodoreonzgit-9b0d7758"
kind = "review"
origin = "human"
created = "2026-11-04T11:15:13+08:00"
modified = "2026-11-04T11:15:13+08:00"
target = "fn:9b0d7758d9e3ac90"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::N_RADIAL"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:15:13+08:00"
commit = "4004da6611650294e4bb19696bfffe87c923ef87"
hash = "sha256:59a5e8ce9a05349920ac13580febb3d2df80e6fac745f9b0053099aed23b0db3"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "plumbing"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "is_test_code"
item_value_source = "cited"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "Ysx+qsb9FTiQfpoPRQrMtghn3fVQSfAHGwwehhek1vTUUew+Wl/ErsuQsnKj571fCIe6YW1iUDPsntNS0qZ3Ag=="
```

checked against table 7

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 4004da6611.

# Review: upstream (github:theodoreOnzGit)

```toml
[kovan]
id = "review-upstream-theodoreonzgit-8d25b16b"
kind = "review"
origin = "human"
created = "2026-11-04T11:16:10+08:00"
modified = "2026-11-04T11:16:10+08:00"
target = "fn:8d25b16bb87e33c6"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::upstream"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:16:10+08:00"
commit = "e9be6819cfdcf6e1ae4fd5b2c853dfdfee832e11"
hash = "sha256:538b6eea075af3a5d7c39745773931c10cdf7b332f63a19603868f0b8449c583"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "ui_or_cli"

[review.callees]
"fn:03f4100244ab1727" = "sha256:bea6414e8e99f94ebe6ab5c8bcd76ca692357209df5b465e98a9117c39e35b38"
"fn:1baee06ed91589b2" = "sha256:906c8fb45b9ce4759492ee1f3e113bcbf209fa53a912c0003642e686e8cfe2af"
"fn:3dab844bc23ea2bd" = "sha256:773d24db91ae5910200fd9060f3bedee67bce16d86c437bf02caea9ddccd8f9d"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "KXLfwoYKbk9vfmJk3WN7dkpm9lfd3FGbWkXx2ziekS7Fm2l+hzLtWMdHx5CsMdSWDzGZo2Lzu43Mrdxv/NIqCQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit e9be6819cf.

# Review: table19 (github:theodoreOnzGit)

```toml
[kovan]
id = "review-table19-theodoreonzgit-fe6fb335"
kind = "review"
origin = "human"
created = "2026-11-04T11:23:50+08:00"
modified = "2026-11-04T11:23:50+08:00"
target = "fn:fe6fb33576d54327"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::table19"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:23:51+08:00"
commit = "8e9d9bff329b255690c56774efdb0f4416194c58"
hash = "sha256:46f138d56ce352b44596af49695c291d91336b112f064f5295c2d69556342815"
doc_hash = "sha256:81f35c539e8fba2e69f81190a3ec4c946517e6c11013b3bd8c1bd87e65b297dd"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.callees]
"fn:03f4100244ab1727" = "sha256:bea6414e8e99f94ebe6ab5c8bcd76ca692357209df5b465e98a9117c39e35b38"
"fn:1baee06ed91589b2" = "sha256:906c8fb45b9ce4759492ee1f3e113bcbf209fa53a912c0003642e686e8cfe2af"
"fn:24b9b78394837b0c" = "sha256:c5e3fcdec29fdbc79637fd69a373b67e0c4ba62e4ac8aace394259ea282aacba"
"fn:2816dd8674f2b148" = "sha256:3890d7f2c2bf5d78ac0a02d952c219265c6ded009ae509e8ceaaf6ce156eced5"
"fn:2db20ac5ca82f2df" = "sha256:88f57a620fcad0a9ad2c3a1dd178c5e0c248335e82ec9259d7aae14506c5ada5"
"fn:34ff47a5448224c9" = "sha256:1565117e190704e250537a267fcc591cbd6f4f74e8c74c62d6c58955f5437378"
"fn:383734307cced06a" = "sha256:6e869b80d4bc8cf329bab6bad4e2ba9be8ef6411c675c75640bc46a6a70168fb"
"fn:44fd31aa536c0e7c" = "sha256:7104d2332e8fdac8384e558146e0fd17ddc6de5ff99c23c6ad619fc88331a963"
"fn:49cbfd358a6976d4" = "sha256:21bde6366bd14b2230818107f9f51697dcb40359f8c429329b0bea39dd191993"
"fn:515e6933ab0603f9" = "sha256:e6d597d01f0fbf4906e858eed1b26c1868f061c52188509463e05e83cc78a311"
"fn:5a2cfb2f89e6446a" = "sha256:6f018e714c7040808b9b260d600f70aa1c59b7aa1fb8ebc33668a99f025e9654"
"fn:5a33f79b407cf341" = "sha256:4c2afe9cb508ba2c2933c0c9cd1e4cf01c88e57848b554e759d1854331c9e2a8"
"fn:745bd77be64f687a" = "sha256:443b83fe6b2801369330c415e78fe99fc27db37a4f85e98465880c10a1eec8f2"
"fn:8f6c972c90fdceff" = "sha256:9db560f8b8d5220a25ff12870e0702d97acedb8e3ad099a687d937d4bd35d990"
"fn:9a8a6835861d321b" = "sha256:35aa9d1c98f83def7d97ea998bcab736badd6b6b855a3d5be8e59ec40ed3de07"
"fn:b872687bd30a967d" = "sha256:a0ebdf17d87a0897d282f8e148ec15e3648cb75a06fbbb453256f89835cd2337"
"fn:dcb6acb5e841b5b0" = "sha256:24b2f6ca6faddbc1768e5afef5d98c2498f83bd3ad7a163cbe0edc7e13e24187"
"fn:e503a3d0722ff508" = "sha256:2064b2b67750abeeaa8a6697c2016cdb79cb79e9d76cf1bcf4d974387dfe881c"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "XjidLSamGsKT0d00qprY3LwcBltVqdiyDvdh3oTf08zpDTwWj3rEkOl7nSzvDn6mnGOM/3pjPC6c55kv7gn8DQ=="
```

parses table 19 csv (with table 16), didn't check too throughly, function wise
but looks correct.,.
in terms of the table19, and plant constants

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 8e9d9bff32.

# Review: table (github:theodoreOnzGit)

```toml
[kovan]
id = "review-table-theodoreonzgit-1baee06e"
kind = "review"
origin = "human"
created = "2026-11-04T11:24:41+08:00"
modified = "2026-11-04T11:24:41+08:00"
target = "fn:1baee06ed91589b2"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::table"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:24:42+08:00"
commit = "4154585966144f34dc82addd6fe233dd5de087b6"
hash = "sha256:906c8fb45b9ce4759492ee1f3e113bcbf209fa53a912c0003642e686e8cfe2af"
doc_hash = "sha256:9064bb0ecf34b62ab4b1dabf3f86db1fb9fa27dcc8002d3d8632a7eb4d88b7f0"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.callees]
"fn:8e508796566df4fe" = "sha256:1c65a7461c3900f956519fdccf236e0c993fc7a7382c9cdfd5dcab04fab35fce"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "lD7SPXyTqz3aZQhq7fQNJZmtSHtlv/zQZFtRRnW5K29LxU9nQTHSZxN3Jk0x6Q4ttkLyRxgpV6oOCp/+nB64CQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 4154585966.

# Review: f (github:theodoreOnzGit)

```toml
[kovan]
id = "review-f-theodoreonzgit-03f41002"
kind = "review"
origin = "human"
created = "2026-11-04T11:29:08+08:00"
modified = "2026-11-04T11:29:08+08:00"
target = "fn:03f4100244ab1727"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::f"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:29:08+08:00"
commit = "bd12783b37956ac75fb578ca26c59c26bffdd99f"
hash = "sha256:bea6414e8e99f94ebe6ab5c8bcd76ca692357209df5b465e98a9117c39e35b38"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "io_format"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "test_panics_on_failure"
independence = "someone_else"
limits_and_guards = "test_no_input"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "is_a_test"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "dSKEhFVjaAXMa1fqbpEYNi+Ovt0b+D43X5aJ/X6dMPsRIa7v2s5pp0k54mtDLf5wCIaqa4LL2XPLcy3kKNqoAA=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit bd12783b37.

# Review: YEAR_S (github:theodoreOnzGit)

```toml
[kovan]
id = "review-year-s-theodoreonzgit-24b9b783"
kind = "review"
origin = "human"
created = "2026-11-04T11:31:08+08:00"
modified = "2026-11-04T11:31:08+08:00"
target = "fn:24b9b78394837b0c"

[review]
path = "crates/boon-lay/tests/mhtgr_stoyer_workflow.rs::YEAR_S"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T11:31:09+08:00"
commit = "dea820e8ef1347f9eca6a877e12b5f48a642b1ad"
hash = "sha256:c5e3fcdec29fdbc79637fd69a373b67e0c4ba62e4ac8aace394259ea282aacba"
doc_hash = "sha256:03c61bcd9127d2e3f670731d9a5cbcbc5f9b95b01df0f2eb17b1a5c3fe1a44bd"
cargo_lock = "sha256:bc3449b029ba47af926e4ca500be454f3e3f738a3cf88c5ee3c1c3fddeaa5dfd"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "cited"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "is_a_test"

[review.authorship]
kind = "agent"
sessions = ["https://claude.ai/code/session_01EXPhstVwAqRb8o6YHVQoXW"]

[review.signature]
key = "k1"
alg = "ed25519"
value = "DE+gdHA0eNK4/oXWdWwi67XEkQ00hmlN8F8Ogi2zjS0Yucim6s98IGFjf3p5nfkK6Z/n3fXr5kxAVjGS++XdDw=="
```

just does unit conversion

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit dea820e8ef.
