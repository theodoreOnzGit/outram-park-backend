# Step 7 — Ray's equations: how this crate does it

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
>
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**Back:** [Step 5, the random walk](./05-random-walk.md) and
[step 6, the aggregate view](./06-eulerian-fresco.md).
**The question that led here:** the random walk is faithful but noisy;
the Eulerian family is smooth but needs strong assumptions or a mesh.
*How does boon-lay get a release answer that uses what each offers?*

**This page answers:** which equations does this crate use for that, and
where do they sit on the ladder?

---

## Where this sits on the ladder

```text
fewest assumptions, most expensive
   step 5   random walk (Lagrangian)        noisy, any geometry
        ...  <-- Ray's equations: placed by the maintainer
   step 6   multilayer Eulerian (FRESCO-type)  smooth, needs a mesh
   step 6   Booth, steady state, attenuation   closed form, coarse
cheapest, most judgement
```

The position between steps 5 and 6 is the maintainer's framing. This
page records the frame only.

---

> **[Maintainer to supply: Ray's equations (Zhern Ray), their source, and which boon-lay module implements them.]**

---

## What this page cannot yet say

The repository holds no record of these equations or of their author at
the time of this draft (2026-10-18). A search of the `boon-lay` source,
its docs and the workspace docs found nothing. So this page does **not**:

- state the equations;
- name a boon-lay module as their implementation;
- claim that any existing result was computed with them.

Until the maintainer supplies the source, readers should take steps 5
and 6 as what the code demonstrably does, and treat this step as
pending.

When it is filled in, it should follow the same shape as the other
pages: the question, the equations with every symbol defined, the
judgement that makes them acceptable, when they fail, a V&V record with
numbers (or a plain statement that none exists), "What we assumed", and
the code-review links for the implementing functions.

---

## What we assumed

Nothing yet. The assumptions of Ray's equations are to be listed once
the equations are supplied.

**Credit:** Zhern Ray

**Next question:** all of the above treats the particle as it is built.
What happens as the fission gas builds up pressure inside it?
[Step 8, gas, pressure and stress](./08-pressure-stress.md).
