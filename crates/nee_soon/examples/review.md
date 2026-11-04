# Review record: crates/nee_soon/examples

```toml
[kovan]
id = "review-record-crates-nee-soon-examples"
kind = "note"
origin = "kovan"
created = "2026-10-11T15:00:45Z"
modified = "2026-10-11T15:00:45Z"
```

The review record of this folder's code, created empty by kovan's "Index fresh".
Reviews, needs-fix notes and annotations are added by kovan's review wizard.
This file is human-owned: kovan never overwrites it.

# Review: main (github:theodoreOnzGit)

```toml
[kovan]
id = "review-main-theodoreonzgit-edd74593"
kind = "review"
origin = "human"
created = "2026-11-04T15:55:31+08:00"
modified = "2026-11-04T15:55:31+08:00"
target = "fn:edd74593f66a1a8b"

[review]
path = "crates/nee_soon/examples/htr10_rmc_keff.rs::main"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T15:55:31+08:00"
commit = "66aa9561d1442d72e5acaa15f6b344de47f93352"
hash = "sha256:139f74ee3ba74878dc187bde0ffc959b9dfbf147625f46e488a6d72be4447446"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"

[review.callees]
"fn:040aba8efb432ef5" = "sha256:7c639956abcc9f80d87c115c539ff05c1cb365f813ef51ab84be1eb66627e398"
"fn:14a6fa25c0a1f183" = "sha256:677bb5b0352b5b8c7e8f75f07f58ac3eb65476c76538d9f12885c63644862d24"
"fn:155454d7f49046ef" = "sha256:7516a714c7bda78f587c8b17472045899e8299158f157f6ab8752e66eb13fc65"
"fn:1b352f5a3a0a3ecc" = "sha256:d97aaeaa66eefa8add43e2103d464bcad044375f94fc9d5d42f20fcd54fc7607"
"fn:287949e80288e01d" = "sha256:09d46fb130bf01f7420ff5147d689be3aa40cd03c2cbd38138a8b586eb386f8c"
"fn:2d465d97c9ae3ad9" = "sha256:85212436ce53317cbd32c5dfa37230d6b686fece65fd8dcae2b27d7c65eb4beb"
"fn:358439787cf5c0b0" = "sha256:98c4000260dc59bd865c851fd0b7701e147d967c5c2c5359a5630c406784cdb2"
"fn:3b46afdb213471df" = "sha256:d205251ce71527f72a213e6313a69360708b096ecd0626da6f273a7ad4c66706"
"fn:44465a7ba0c2cbf5" = "sha256:f717d838d760b1d53e2c633e5e19c8e40e3818fc743af988d767a3bdaa5b1f79"
"fn:47fd464da19fe10c" = "sha256:a42c663f3f896c304a4dccbae98c265fec0f9baac00d179dc55d5f355563e6a6"
"fn:4f76e6b26742b201" = "sha256:ddabaed033654303fd6f24a1172912bf468f4c7895a2eb05c5d71b7f47501410"
"fn:53ba2c2c758a17f4" = "sha256:2635e656eba4896697c5f63904bb21a39b2ce70b9aa325031779f7931d9d67a2"
"fn:57187da898b1ab5c" = "sha256:ff2bb3913bcd449c48d20402c059deb1fef944cc3f0b5da64e5cd8ef7aa8104f"
"fn:5772088e7feb74c1" = "sha256:748922f8c63cdbca594196c8ade01f8aa22179c8ceaf95ba21b3db97235caf64"
"fn:5c4cc3c4bfe8fcce" = "sha256:6034c6df8404b5568b22c5ae32dd6e9985a206f6395951af87a07ee79a831635"
"fn:5d1aca2fda1e8746" = "sha256:f801369edd11a557f0189e4668b3d2e42a47de375681738ad241a65193b6bf69"
"fn:6ede4a464f6c2056" = "sha256:e1cb1d09ca7a182778637cb70a11fa25744e27d2272c08abd1feb7fc23a6cd80"
"fn:770c5941dead0a7e" = "sha256:e7da85859203f919743199860493872328e541d23b2146b50d16d2ccd7055b85"
"fn:77b3f887f8a8f5de" = "sha256:cae6e55b141f9b3592ec81f08d6778a8ca23f30c94245ecf2e159b62f78e4c52"
"fn:7bd8aa8577c8f39c" = "sha256:032670e0f7c429300a055a61b0377f37f649f49f23ba7e44a6b862fc6623db2b"
"fn:805df5c70660485c" = "sha256:2967583e7dab70fd8563df37b399fb0c2984b08c9de21f3ee558edc0c16766c4"
"fn:87319dc849f544c5" = "sha256:3123ccc5a13488d549983446cc038c90b5ace3ae3f11dc1ec54639e1c613d35c"
"fn:8d583fbd38b39922" = "sha256:3a231cdf2e29f2cd161c71d9b59d4e2e37fd05dc1849b72ad3897096b7d1e697"
"fn:9095272bcb872261" = "sha256:126a28e3122e02fd404005eaed9a6611aba7361284f38b1f5f1aad9729b84a4d"
"fn:917974dac4409fe6" = "sha256:4bede823c6aa1672bd6a8b17091f7604d1f8d07cb9a24b59036a919a0f792412"
"fn:9603fb2b6435f47a" = "sha256:a9aa4cfa621ce3f81004c60b769d5461db278e7a97e10fc01ddd037f24c34bc1"
"fn:962ea2d7def98052" = "sha256:718b28eb903ea0dd8601ca61b243e9b5ee77615ff21b8ca79f0a17755d8302c4"
"fn:9a6b1a9088a3265c" = "sha256:a8a5679b51e85612647fc61b960c8f7ba53e7622d9ce0721ce50184aaad2d437"
"fn:9b1683fffae43795" = "sha256:9e495ac6345c3cdb37954d3d796fd20dd04461eef7a8f361967c9d2f867511d3"
"fn:9d1821b1c0933a90" = "sha256:d8e4882bb0d01b6f2c4b80bc12edbcc0d4673eaaeda332ae8c5eb8080a83a4f5"
"fn:a0ade86c3ebd834b" = "sha256:1dac041004127c14d7d361e0d34d3cdc5613d055bf5a8f4e91925827a9fdbd21"
"fn:a14b6faadb959f4d" = "sha256:a911f6145f2b03afd0f51dc1460fb4f33fde2f5b2b2c87bdcb03e82c5398373a"
"fn:a5d962c731a1e18c" = "sha256:92f63b94bf00046c848794c768f537c5478dafc5fa4b2c945aa87ce0bf5450e2"
"fn:a76aa0a8dc7b1249" = "sha256:201f7c07e7fca982fc4be380806ce9a2e5fee8c753a255858827abc251cd6ac3"
"fn:a806bd1baee256d6" = "sha256:f01009fc553b9a4052443afac8220b96995bc32f66f477c05c1c44bad7b5f596"
"fn:afd63c8186b7d3f0" = "sha256:8fde0942a8070435b9881325a9fa0496b88ae6a532c8369a786e6bd696068bac"
"fn:b3390d1660e08cdc" = "sha256:631611589e378c753db946482237b76fa08cfc4d3992a8bd9a65106a5d2bed1a"
"fn:b577f89733eb3af5" = "sha256:ac474a51f3e2d785fa3051c4a1cea69b146e26145fa675596dafbae6fae45677"
"fn:ba62f7263371ed0b" = "sha256:ff88f9c4950a9eea7d7baa6921f76f73d3ce94d8042acb018222785db27ddec5"
"fn:bda5c77a17ef0158" = "sha256:e49f326e3c6b9e4865df90b1a1f1885b13fa1fdf0b17b978be65465f60d02bc3"
"fn:c3f67069af8569a9" = "sha256:bdee6e1c7c556895193013fe5194b2b54b287e3c8e690bf14e5616d7da5e2cb8"
"fn:c5011e8cbdc0dc98" = "sha256:53f9232b86a235fe4df321edb8f69aefa34e46ac95678efd6e0cc829b575024b"
"fn:c60ee79ac0406f0e" = "sha256:92e5ce1065e6f0125c779c7592e9b90a8bc922ca8d5690cc00fde11b8b2e2326"
"fn:c8df696de2995cd0" = "sha256:af80a644f49de2cb1d4b599bf213ca66022da54929e7f1986712cab39321bf3e"
"fn:cee6d34201e1fcab" = "sha256:e2a0447fa59781cf2d8c97f51e882468f689b13dcc5e3fbd51b81faf613b344a"
"fn:d427b7e9ab874c91" = "sha256:078d70299b05a9aacaf52ee877469e86c3853e70a0578fa53283b5a00bfaf7d8"
"fn:ea4bcd9a08d3698d" = "sha256:c3fdaa1b6dd69b06ecd79a2e1b602d4fdf7b32b22b3992aa314f986838d6b5d5"
"fn:eb0d723f9cc75435" = "sha256:930c3139b60b57ac5864d5b70d8b8deef282cc35b55e962544c5eb57a54bc5fc"
"fn:f0947f9f757d92d2" = "sha256:2b2b1a7e8c23495765db50a8145fb6ee6fceb8c868f0f36da63a99511215bd72"
"fn:f4704f13ac6b36e7" = "sha256:001e35512003f8e0352e81dbdcea35b138206799852bb2fa7c37d4e720667a19"
"fn:f8a9269e53dc9d00" = "sha256:267e061d0f427a872d74298e75471817304b1ca11502da56af63ec69d3070478"
"fn:fabbfafa1d782ad1" = "sha256:79e1b309bcff3eb274fbb59e04731581dd759baefbff675eb87cebe42732f3b4"
"fn:ff4dfa6416cdd4fc" = "sha256:dbc35cfa9f5f6ca51932de3653592be4a995644f71dd8233f344d32be3f21a76"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: driver file (main)"
independence = "someone_else"
limits_and_guards = "no_limits: this is a driver file (main)"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "other: This is the test function, just as an example"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "reference_code_to_code"

[review.authorship]
kind = "mixed"
sessions = [
    "https://claude.ai/code/session_012qgX47fwybgAuf2wEiRvBu",
    "https://claude.ai/code/session_018Lj53tTBuzapVRRpCXnX6e",
    "https://claude.ai/code/session_01E6ityjcMTdcZrzYRk9tyxk",
    "https://claude.ai/code/session_01FbFYNpJxPtK8cotkuBZyPU",
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
    "https://claude.ai/code/session_01MCKcAXFCyB6muPnCcb2NpZ",
    "https://claude.ai/code/session_01MuN674Wnq45CwGurhQFz6G",
    "https://claude.ai/code/session_01RK4L9ruHdU2JHaWPW4ozsc",
    "https://claude.ai/code/session_01UKkBkWTc7hmuyRCZxyQUqZ",
    "https://claude.ai/code/session_01VAHwhC54ZrQ6NHBcjo5q9B",
    "https://claude.ai/code/session_01WBYPnuGxMa5jpjipjYPGGi",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "JoZv5nClc589o1LC19Wi81DqPbAG3HGQiYbqhhwAr/Ehm02Bul1VvdukbLE946ZpAPY954Z+JmEJevmPV8oGCg=="

[[relation]]
target = "concept:02-nuclear-safety/nuclear-design/neutron-transport"
kind = "implements"
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit 66aa9561d1.

# Review: env_usize (github:theodoreOnzGit)

```toml
[kovan]
id = "review-env-usize-theodoreonzgit-f8a9269e"
kind = "review"
origin = "human"
created = "2026-11-04T16:11:57+08:00"
modified = "2026-11-04T16:11:57+08:00"
target = "fn:f8a9269e53dc9d00"

[review]
path = "crates/nee_soon/examples/htr10_rmc_keff.rs::env_usize"
by = "github:theodoreOnzGit"
rung = 3
date = "2026-11-04"
signed_at = "2026-11-04T16:11:57+08:00"
commit = "bcfa50de5b2574dbf28e465564af9e342f7b8b52"
hash = "sha256:267e061d0f427a872d74298e75471817304b1ca11502da56af63ec69d3070478"
doc_hash = "sha256:cc0e49ab07658770bb77dbe62bacd5f3e59c094cee92645523d475f159add015"
cargo_lock = "sha256:a5440ad885b8bef2745cbba37122e65f7eaa556c34dbc4a462d93afb860c2895"
no_concept = "generic_numerics"

[review.checklist]
coding_standards = "yes"
doc_matches_behaviour = "yes"
error_handling = "panics_justified: this is data input"
independence = "someone_else"
limits_and_guards = "guard_waived_research: so this takes in environment variables, defaults to 12"
maintainability = "yes"
numerical_hazards = "none_found"
test_reach = "reached_and_checked"
unintended_function = "no"
vv_case_author = "agent_wrote_or_cowrote"
vv_evidence = "none_yet"

[review.authorship]
kind = "agent"
sessions = [
    "https://claude.ai/code/session_01LN5DvQqPQdehQ1YLqvGPXd",
    "https://claude.ai/code/session_01MCKcAXFCyB6muPnCcb2NpZ",
]

[review.signature]
key = "k1"
alg = "ed25519"
value = "8H5+/5ultOlX1RM4IeEWYRjtOwZFxd82fO1n9FOFomOuMrCqD3ldURjHJgKHwXkG5q3kXDeufGnR8kuiqj4sAQ=="
```

## Sign-off

Reviewed by github:theodoreOnzGit at rung 3 on 2026-11-04, commit bcfa50de5b.
