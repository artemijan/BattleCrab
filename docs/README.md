# Documentation

> **Where the project is.** The port is finished — every login milestone
> (M0–M5) and every game milestone (G0–G34) landed, and the parity sweeps that
> followed them are closed. The project is now in its **testing phase**: play
> the server, find where it diverges from Java, fix it with a test that fails
> without the fix. Nothing in this directory describes work still to be ported;
> [PORTING_STATUS.md](PORTING_STATUS.md) is the record of what the port did,
> not a plan.

## Start here

| Document | What it answers |
|---|---|
| [PORTING_STATUS.md](PORTING_STATUS.md) | **What was ported, what is partial, what never will be.** One table for the whole port, plus the parity sweeps that audited it. |
| [THREADING_MODEL.md](THREADING_MODEL.md) | How the server is threaded, why it was designed that way, and what it costs. |
| [PROJECT_LAYOUT.md](PROJECT_LAYOUT.md) | Where code lives and where new code should go. Conventions. |
| [PROGRESS.md](PROGRESS.md) | The dated journal — what landed, when, and what broke on the way. Long. |

## Reference

| Document | What it answers |
|---|---|
| [DATABASE.md](DATABASE.md) | Fresh installs, adopting a live database, adding a migration, regenerating entities. |
| [LOGGING.md](LOGGING.md) | Diagnostics (droppable), audit records (never dropped) and metrics: why they are separate, where each file lands, how to query them, and every config key. |
| [CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md) | The long-form analysis behind the threading model: Java's thread inventory and task managers, construct-by-construct mapping, the ECS component split. |
| [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md) | Every place Java relies on something Rust does not have, and the decision taken for each. The numbered decisions other docs cite. |
| [DASHBOARD.md](DASHBOARD.md) | Design of the web dashboard and its API. |
| [CUSTOM_DIST_DEVIATIONS.md](CUSTOM_DIST_DEVIATIONS.md) | Where `dist/game/data` intentionally differs from upstream, by operator decision. |
| [ROADMAP.md](ROADMAP.md) | The historical milestone breakdown (G14→G33) and the scope gate that defined what is out of scope. Superseded for *status* by PORTING_STATUS.md. |

## Parity checklists

Mechanical Java-file-against-Rust-module diffs, kept as evidence for the gates
they closed:

- [LOGIN_SERVER_PARITY.md](LOGIN_SERVER_PARITY.md) — all 63 login-server files accounted for.
- [PARITY_LOGIN_SERVER.md](PARITY_LOGIN_SERVER.md) — the M5 acceptance pass over the same tree, with fuller per-file notes. Overlaps the above; neither is a superset.
- [PARITY_CHECKLIST_G33.md](PARITY_CHECKLIST_G33.md) — client-packet handlers diffed by opcode.

## What happened to the plans

This directory used to hold 172 `PLAN_*.md` documents, one per feature, written
before the work and never rewritten after it. They are deleted; the
[retired-plan index](PORTING_STATUS.md#retired-plans) lists every one with its
milestone and the `git show` incantation to read it. What actually landed is in
[PROGRESS.md](PROGRESS.md), which is dated and was written afterwards.

## A note on trusting these files

Prose about what remains has drifted into fiction twice here, both times
claiming work was outstanding that had already shipped. When a document and the
code disagree, the code wins. The `TODO(<tag>)` markers in the source are the
one status artefact enforced by a test
(`deferral_markers_match_the_recorded_inventory`, currently expecting exactly
one: `TODO(antharas-cc)`) — prefer them to any sentence written by hand,
including the ones in this directory. `DEFERRALS.md` used to inventory them and was deleted on 2026-08-07
once the inventory emptied; `PORTING_STATUS.md` says how to read it out of git.
