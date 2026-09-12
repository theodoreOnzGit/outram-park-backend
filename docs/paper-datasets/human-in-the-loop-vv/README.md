# Human-in-the-loop V&V of agentically ported code

**Source records:** `docs/human-in-the-loop-ciet-v2-case-study.md` (192 lines),
`docs/human-corrections-to-ai-work.md`, the kopi-beans issue corpus (1,490
issues, 623 closed), and the git history (1,559 commits, 2024-11-25 onward).

## The question this answers

Part I ([arXiv:2608.17504](https://arxiv.org/abs/2608.17504)) found that with
agentic porting, **V&V by human expertise rather than code generation becomes
the bottleneck.** It asserted this; it did not characterise it.

This paper asks the follow-up: **what is the bottleneck actually made of?**
Specifically — which classes of defect survive AI generation, AI testing and AI
review, and what kind of knowledge catches them?

That is a question the field needs answered and almost nobody can answer,
because it requires a large agentically-produced codebase whose defect history
was recorded *as it happened* rather than reconstructed. This project has one.

## Why this anchor case

`docs/human-in-the-loop-ciet-v2-case-study.md` is a worked record of building
the CIET v2 digital twin, with sections already written for:

- division of labour between human and agent
- **four interventions that changed the design**, quoted in the maintainer's own
  words
- the bug the port surfaced
- **where the AI agents were wrong, and what caught it**
- what was verified, and what was not

CIET is the right anchor for these authors: it is the Compact Integral Effects
Test at UC Berkeley, the subject of Ong (2023) and the validation target of the
TUAS article (Ong, Xiao & Peterson, 2025). The domain expertise being *exercised*
in the case study is the authors' own, documented expertise — which is what makes
the claim "this needed a practitioner" checkable rather than self-serving.

## The defining criterion (already written, keep it)

From `human-corrections-to-ai-work.md`, and it is the paper's thesis in one
sentence:

> A defect that an AI could not have caught by working harder, because the
> verification it would have written encodes the same misunderstanding as the
> code.

That is a **falsifiable** class, not a complaint about AI. It excludes ordinary
bugs, and it predicts where review effort should go.

## To extract (not yet done)

1. **Formalise the taxonomy.** `human-corrections-to-ai-work.md` has 2 fully
   written entries. The corpus behind it is much larger — mine closed beads and
   commit messages for the same pattern. Each entry already has the right
   fields: *what was reported* (usually accurate), *what was actually wrong*,
   *why the AI process missed it*, *what test would have caught it*. The last
   field is what converts a war story into a standing check, and it is the
   paper's practical contribution.
2. **Quantify, do not just narrate.** Candidate measures, all derivable from
   data already on disk: closed-bead counts by type/priority; interval from
   introduction to detection; how many defects were found by a human vs a test
   vs an agent; how many were found only when an *external* oracle was
   introduced.
3. **Use the two strongest in-repo exhibits**, both already documented:
   - the **cancelling-defects** case from the transport work — two errors of
     opposite sign produced an apparently excellent `+163 ± 316` pcm, and the
     agreement was coincidental (see `../part2-transport/README.md`). A green
     number is not evidence of a correct model.
   - the **energy-equation boundary conditions** case, where every agent report
     was individually true and the diagnosis was still wrong, because the
     verification encoded the same misunderstanding as the code.
4. **State the limits.** n = 1 project, one maintainer, one language, one
   domain, and the defect log is curated by the same person who fixed the
   defects. Say so plainly and early. The finding is a characterisation from a
   rich single case, not a population estimate — claiming otherwise is the
   fastest way to lose a referee.

## Why this is the lowest-risk of the three to defend

It requires no expertise the authors do not demonstrably have. The domain
content is CIET, and the methodological content is this project's own history.
Contrast the nuclear-data material, which is deliberately deferred — see
`../part2-transport/README.md`, "Scope decision".
