# A first human review goes through an AI-built picture before the code (practice)

**Maintainer direction, 2026-10-05:** *"an initial human review of the code can
be done more efficiently via AI generated GUI, before looking into code."*

When ported or newly written code is ready for its **first** human review, the
AI first builds (or extends) an interactive view of what the code does: its
flows, pools, routing, and which inputs reach which outputs. The reviewer looks
at that first. The line-by-line code review comes after, aimed at whatever the
picture raised.

## Why

A reviewer reading code sees one function at a time, and every branch in it
looks reasonable on its own. A picture shows the whole model at once, and a
modelling choice that is wrong *as a whole* stands out.

**The case that produced this rule (2026-10-05, gh:#583).**
`boon_lay::triso_atops_fork::normal_operation::normal_operation_node` routes
the coolant removal rates in three match arms, and each one reads as sensible.
The port agrees with upstream TRISO-ATOPS code to code, as it should, because
the routing *is* upstream's. Then the TRISO-ATOPS demo's release rung drew all
19 HTR-10 nuclides through the primary circuit at once, coloured by transport
group. That made two things visible:

- iodine, caesium, strontium and silver all plate out at **one** rate;
- the purification system **never** takes a metal.

The maintainer flagged it as "fishy" within minutes of seeing it. HTR-10's own
source (Liu & Cao 2002) gives plate-out per cycle of 20 % for I and 50 % for
Cs, and 90 % purification of metals. No test could have caught this, because a
faithful port reproduces upstream's simplification exactly. Code-to-code
agreement certifies the translation, not the physics.

## How

1. **Drive the picture from the library's own calls.** No physics in the GUI;
   the picture shows what the code does, not what the author meant
   (`dhoby-ghaut`'s demos are the pattern: the engine calls the crate, the app
   only draws).
2. **Everything at once by default, then filter.** Show the whole population
   (every nuclide, every zone, every species) first, grouped and coloured by
   the dimension the physics should depend on, with a filter down to a group or
   a single item. Uniform treatment across a group is the thing to see, and it
   is only visible when the group is all on screen together.
3. **Draw every modelling choice, including the zeros.** A term the model sets
   to zero is drawn as an off path, labelled with the reason ("no clean-up: the
   HPS does not scrub metals"), never left out. A missing arrow invites no
   question; a labelled dashed one does.
4. **Label what is illustration and what is borrowed.** For example, dot speed
   is not physical; parameters taken from another plant's reference case are
   named as such.
5. **Follow the demo rules:**
   [mobile-first and no-lag](mobile-first-tutorials-and-demos.md), a headless
   mode, and
   [reactor geometry drawn for review](../../crates/dhoby-ghaut/CLAUDE.md).
6. **Record what the review raised** in the issue tracker, then do the code
   review on those questions.

## What it does not replace

This is an entry point for review, not verification or validation. A picture
shows a model's **structure**; it is not evidence that the numbers are right.
Code review, tests, code-to-code verification and validation against measured
data are all still required (`AI_USAGE.md`, "Required Human Review";
`VERIFICATION_AND_VALIDATION.md`).
