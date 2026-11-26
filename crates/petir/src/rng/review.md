# Review record: crates/petir/src/rng

```toml
[kovan]
id = "review-record-crates-petir-src-rng"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: prn (github:theodoreOnzGit)

```toml
[kovan]
id = "review-prn-theodoreonzgit-681ddf65"
kind = "review"
origin = "human"
created = "2026-11-26T10:45:44+08:00"
modified = "2026-11-26T12:51:52+08:00"
target = "fn:681ddf6544e7b60b"

[review]
path = "crates/petir/src/rng/lcg.rs::prn"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T12:51:53+08:00"
commit = "4388cb3b1acf65f713bd65b5b24e37036ab75095"
hash = "sha256:17638ea97982299a8f568290be2092c01c5d16f34183e2d73318e4c404ae4734"
doc_hash = "sha256:a4d8953a61b38a9b837f9d98df8be2e4afe68134124aa929247c69e8a094c5c0"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.callees]
"fn:753a37cf1d65784f" = "sha256:d299a82397336967bc828c6fc316000582411eb0d21e12313ad3f06bede79899"
"fn:902eafae69219a7e" = "sha256:8953807ced646a71ef314fdeb41301812fc0f1d781d673bbef3e2cadfdc6a8d2"
"fn:a4ea8cdd8a0e37b1" = "sha256:ec41a8e866d35a6b2c5c642f9811ac7956f21801b96a25397f154d7a8c0e774a"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "matches_upstream_no_error_return: openmc prn source code is the same logic"
independence = "someone_else"
limits_and_guards = "matches_upstream_no_guard: openmc prn source code is same"
maintainability = "yes"
numerical_hazards = "other: faithful upstream port, works mostly"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.signature]
key = "k1"
alg = "ed25519"
value = "P6MEUYj1C6PoeXHvdl0yMieeNfewFi8hm9siutxJBkJc41xo4Iff8ri8v59T0d9eD1Bscr1j78F13a1TGoNsAQ=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

in upstream, i saw  return static_cast<double>(result) * 0x1p-64
0x1 is 1,
p-64 is 2^-64
after consulting chatGPT, it brought me to the place it was defined (hexadecimal notation):
https://eel.is/c++draft/lex.fcon
looks legit.

I also checked 2^64 is 1.844E+19 in Libreoffice. 
Checks out. The bit shifting and all look about right
just like openmc source code

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit 4388cb3b1a.

# Review: MULT (github:theodoreOnzGit)

```toml
[kovan]
id = "review-mult-theodoreonzgit-753a37cf"
kind = "review"
origin = "human"
created = "2026-11-26T12:53:18+08:00"
modified = "2026-11-26T12:53:18+08:00"
target = "fn:753a37cf1d65784f"

[review]
path = "crates/petir/src/rng/lcg.rs::MULT"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-26"
signed_at = "2026-11-26T12:53:18+08:00"
commit = "f6ec6b6ab71aba17750c8c2e118683b919186697"
hash = "sha256:d299a82397336967bc828c6fc316000582411eb0d21e12313ad3f06bede79899"
doc_hash = "sha256:b322aaf628324c09b57586a330cafd0dfa0906059b481e3f1915a6d87c7de0ff"
cargo_lock = "sha256:f01119ef723c81b98b22a0bd1030fcc4965b939f2c757c626b5986870ad940ee"

[review.checklist]
coding_standards = "yes"
independence = "someone_else"
item_matches_description = "yes"
item_test_use = "used_and_checked"
item_value_source = "original_code"
maintainability = "yes"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "human"

[review.signature]
key = "k1"
alg = "ed25519"
value = "K+VZeTnW33dHkEXMesWujKo8y1jN3uASLC/A3RlQHn4kOnzS7jRBAMBHlkcFBeiAQu8I29XMi1ODz1VnFxT0Ag=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport/continuous-energy-collision-physics"
kind = "implements"
```

confirmed against upstream

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-26, commit f6ec6b6ab7.
