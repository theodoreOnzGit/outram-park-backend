# Beads recovery — 19 unpublished beads, 2026-08-12

A durable copy of 19 beads that existed **only** in a container-local store and
would otherwise have been lost. Kept in git because git is the only thing in a
remote-execution container that survives a restart — this session already lost
an entire agent fleet's work to one.

## What happened

These beads were filed in earlier sessions with **beads-rs (`bd`)**, which
cannot push the store ref to a non-local remote
([kopitiam#19](https://github.com/theodoreOnzGit/kopitiam/issues/19)). They
therefore accumulated in the container's local store and were never published.

Meanwhile `refs/heads/beads/store` on **origin** was migrated from
`format_version 1` to `format_version 2`. That migration rewrites history, so
the local and origin stores genuinely diverged: origin was strictly ahead in
content (828 issues, later 833) while the local store held 19 beads origin had
never seen.

`bd` 0.1.26 could not read the store at all by then, failing with
`sync_failed - JSON error: invalid type: map, expected a sequence at line 1
column 1868`. Installing **kopi-beans (`bn`) 0.1.3** fixed reading, and the
local ref was repointed at origin's v2 store after the 19 were exported here.

## What was done

1. Exported the 19 local-only beads to
   `local-only-beads-20260812.json` (this directory).
2. Snapshotted the entire pre-repoint local store at ref
   **`refs/beads/local-preserve-20260812`** — local only, not pushed.
3. Repointed `refs/heads/beads/store` at origin's migrated v2 store.
4. Re-created all 19 with **`bn create --id <original-id>`**, preserving their
   original identifiers, and restored their original `closed` / `in_progress`
   statuses.

Preserving the identifiers mattered: **`op-0xv` is referenced from 3 files and
`op-fxp` from 1** in this repository. Re-filing under fresh ids would have
turned those into dangling references pointing at nothing.

## Why this file still exists after the re-filing

**The re-created beads are not yet in the git ref.** `bn`'s daemon has been
unable to sync — `bn status` reports 77 consecutive failures with

```
fetch_error: The slotmap turned out to be too small with 35 entries, would need 2 more
```

and `bn sync` hangs (killed at 120 s). The 19 beads are durable in `bn`'s own
WAL under `/root/.local/share/`, which is **outside the repository and dies
with the container**. Until the daemon can flush them to
`refs/heads/beads/store` and that ref reaches origin, this JSON is the only
copy that survives a restart.

**Delete this directory once `bn show op-0xv` resolves against a store ref that
has been pushed to origin** — not before.

## The 19

| id | status at preservation | title |
|---|---|---|
| `op-0f9` | open | materials: MATPRO Zircaloy dnu/dT may have the WRONG SIGN — literature unread, egress blocked |
| `op-0xv` | open | fracture: FV vs FEM accuracy at the crack-tip singularity is an open research question |
| `op-5rg` | open | container egress blocks every literature host — validation work cannot source data |
| `op-6sl.12` | open | offbeat mechanics: compact stress recovery for material interfaces |
| `op-7od` | open | META_LEMA_ANI: the two hypothesis branches' shear triples are exchanged |
| `op-a7p.9` | closed | code_aster P1b: GDEF_LOG finite-strain wrapper around the small-strain kernels |
| `op-a7p.10` | open | VISCOCHAB follow-ups: implicit NEWTON path, calsig extras, aster/mod.rs re-exports |
| `op-b0x` | in_progress | code_aster: check the ported laws against astest comp0* decks |
| `op-cw7` | open | pimple_foam: phi boundary face fluxes are unconstrained after the PISO corrector |
| `op-fph` | closed | code_aster VISCOCHAB blocked: ODE solvers take `&dyn OdeSystem` |
| `op-fxp` | in_progress | code_aster: consolidate the two IsotropicHardening types |
| `op-gew` | open | outram-foam-basic-lib: 153 rustdoc warnings |
| `op-hm2` | closed | melt_foam solver: buoyant Boussinesq PIMPLE with phase change + Stefan/energy verification |
| `op-hud` | open | Zircaloy Poisson: possible WRONG SIGN in dnu/dT — highest priority check |
| `op-k3v` | open | astest harness: mixed strain/stress control + temperature-dependent properties |
| `op-kus` | open | beads store in this container is NOT backed by git refs |
| `op-nwe` | open | GAP: Gau & Viskanta (1986) gallium melt benchmark data unavailable (egress blocked) |
| `op-u3n` | closed | code_aster scoping doc assumes astest, which is absent from the clone |
| `op-vpg` | open | Melting/solidification: port remaining upstream models + gallium melt verification case |

`op-kus` — "beads store in this container is NOT backed by git refs" — is the
bead that predicted this exact situation, filed before it happened.
